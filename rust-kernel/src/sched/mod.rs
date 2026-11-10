//! HybridOS Process & Thread Scheduler Subsystem.
//!
//! Provides multi-level priority queue scheduling, preemptive round-robin dispatch,
//! cooperative yielding, sleep timer management, and process/thread lifecycle tracking
//! for hybrid Windows NT and Linux POSIX execution.

pub mod task;
pub mod scheduler;

pub use task::{
    BlockReason, CpuContext, Priority, PriorityClass, ProcessControlBlock, ProcessState,
    TaskState, ThreadControlBlock,
};
pub use scheduler::{Scheduler, SchedulerStats};

use std::sync::{Arc, Mutex, OnceLock};
use crate::personality::ExecutionPersonality;

/// Global Kernel Scheduler singleton instance.
static SCHEDULER: OnceLock<Mutex<Scheduler>> = OnceLock::new();

/// Initialize the global scheduler subsystem.
///
/// Ensures the singleton scheduler is booted, registering the system idle task
/// and setting up ready queues. Returns a static reference to the scheduler mutex.
pub fn init_scheduler() -> &'static Mutex<Scheduler> {
    SCHEDULER.get_or_init(|| Mutex::new(Scheduler::new()))
}

/// Retrieve reference to the global scheduler instance.
pub fn get_scheduler() -> &'static Mutex<Scheduler> {
    init_scheduler()
}

/// Dispatch scheduling cycle on the global scheduler.
pub fn schedule() -> Option<u64> {
    get_scheduler().lock().unwrap().schedule()
}

/// Cooperatively yield remainder of current thread's quantum.
pub fn yield_cpu() -> Option<u64> {
    get_scheduler().lock().unwrap().yield_cpu()
}

/// Transition a task into the Blocked state with a reason.
pub fn block_task(tid: u64, reason: BlockReason) {
    get_scheduler().lock().unwrap().block_task(tid, reason);
}

/// Wake a blocked task and return it to the Ready queue with an interactive boost.
pub fn wake_task(tid: u64) -> bool {
    get_scheduler().lock().unwrap().wake_task(tid)
}

/// Advance timer tick on the global scheduler.
pub fn tick() -> bool {
    get_scheduler().lock().unwrap().tick()
}

/// Query the currently running thread ID.
pub fn current_tid() -> Option<u64> {
    get_scheduler().lock().unwrap().current_tid()
}

/// Create a new process under the global scheduler.
pub fn create_process(
    name: &str,
    personality: ExecutionPersonality,
    priority_class: PriorityClass,
) -> Arc<Mutex<ProcessControlBlock>> {
    get_scheduler()
        .lock()
        .unwrap()
        .create_process(name, personality, priority_class)
}

/// Create a new thread under the global scheduler.
pub fn create_thread(
    pid: u64,
    name: &str,
    entry_point: u64,
    stack_top: u64,
    priority: Priority,
) -> Result<Arc<Mutex<ThreadControlBlock>>, &'static str> {
    get_scheduler()
        .lock()
        .unwrap()
        .create_thread(pid, name, entry_point, stack_top, priority)
}

/// Terminate a specific thread on the global scheduler.
pub fn terminate_thread(tid: u64, exit_code: i32) -> bool {
    get_scheduler()
        .lock()
        .unwrap()
        .terminate_thread(tid, exit_code)
}

/// Terminate an entire process and its threads on the global scheduler.
pub fn terminate_process(pid: u64, exit_code: i32) -> bool {
    get_scheduler()
        .lock()
        .unwrap()
        .terminate_process(pid, exit_code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scheduler_init() {
        let mut sched = Scheduler::new();
        assert_eq!(sched.system_ticks(), 0);
        assert_eq!(sched.ready_count(), 1); // System Idle thread ready

        // Initial schedule should pick Idle thread (TID 0)
        let scheduled = sched.schedule();
        assert_eq!(scheduled, Some(0));
        assert_eq!(sched.current_tid(), Some(0));
    }

    #[test]
    fn test_process_and_thread_creation() {
        let mut sched = Scheduler::new();

        // Create Windows NT process
        let win_proc = sched.create_process(
            "explorer.exe",
            ExecutionPersonality::WindowsNt,
            PriorityClass::Normal,
        );
        let win_pid = win_proc.lock().unwrap().pid;

        // Create thread in Windows process
        let win_thread = sched
            .create_thread(
                win_pid,
                "win_main",
                0x14000_1000,
                0x0000_7FFF_FFFF_0000,
                Priority::NORMAL,
            )
            .expect("Failed to create thread");

        let win_tid = win_thread.lock().unwrap().tid;
        assert_eq!(win_thread.lock().unwrap().personality, ExecutionPersonality::WindowsNt);

        // Schedule: win_thread (Priority 16) should preempt idle thread (Priority 0)
        let scheduled = sched.schedule();
        assert_eq!(scheduled, Some(win_tid));
        assert_eq!(sched.current_tid(), Some(win_tid));
    }

    #[test]
    fn test_multi_priority_scheduling() {
        let mut sched = Scheduler::new();

        let proc = sched.create_process(
            "test_host",
            ExecutionPersonality::LinuxPosix,
            PriorityClass::Normal,
        );
        let pid = proc.lock().unwrap().pid;

        let low_t = sched
            .create_thread(pid, "low_worker", 0x401000, 0x7fff0000, Priority::LOW)
            .unwrap();
        let high_t = sched
            .create_thread(pid, "high_worker", 0x402000, 0x7fff1000, Priority::HIGH)
            .unwrap();
        let rt_t = sched
            .create_thread(pid, "rt_worker", 0x403000, 0x7fff2000, Priority::REALTIME)
            .unwrap();

        let low_tid = low_t.lock().unwrap().tid;
        let high_tid = high_t.lock().unwrap().tid;
        let rt_tid = rt_t.lock().unwrap().tid;

        // 1. Highest priority (Realtime) must be scheduled first
        assert_eq!(sched.schedule(), Some(rt_tid));

        // 2. High priority scheduled second
        sched.block_task(rt_tid, BlockReason::IoWait);
        assert_eq!(sched.schedule(), Some(high_tid));

        // 3. Low priority scheduled third
        sched.block_task(high_tid, BlockReason::MutexLock);
        assert_eq!(sched.schedule(), Some(low_tid));

        // 4. Idle scheduled when all others are blocked
        sched.block_task(low_tid, BlockReason::Sleep(100));
        assert_eq!(sched.schedule(), Some(0));
    }

    #[test]
    fn test_round_robin_cooperative_yield() {
        let mut sched = Scheduler::new();
        let proc = sched.create_process(
            "coop_app",
            ExecutionPersonality::HybridNative,
            PriorityClass::Normal,
        );
        let pid = proc.lock().unwrap().pid;

        let t1 = sched
            .create_thread(pid, "task_1", 0x1000, 0x2000, Priority::NORMAL)
            .unwrap();
        let t2 = sched
            .create_thread(pid, "task_2", 0x3000, 0x4000, Priority::NORMAL)
            .unwrap();
        let tid1 = t1.lock().unwrap().tid;
        let tid2 = t2.lock().unwrap().tid;

        // Schedule first task
        assert_eq!(sched.schedule(), Some(tid1));

        // Yield should rotate to second task
        assert_eq!(sched.yield_cpu(), Some(tid2));

        // Yield again should rotate back to first task
        assert_eq!(sched.yield_cpu(), Some(tid1));
    }

    #[test]
    fn test_preemption_and_quantum_expiration() {
        let mut sched = Scheduler::new();
        let proc = sched.create_process(
            "preempt_app",
            ExecutionPersonality::LinuxPosix,
            PriorityClass::Normal,
        );
        let pid = proc.lock().unwrap().pid;

        let t1 = sched
            .create_thread(pid, "task_a", 0x1000, 0x2000, Priority::NORMAL)
            .unwrap();
        let t2 = sched
            .create_thread(pid, "task_b", 0x3000, 0x4000, Priority::NORMAL)
            .unwrap();
        let tid1 = t1.lock().unwrap().tid;
        let tid2 = t2.lock().unwrap().tid;

        assert_eq!(sched.schedule(), Some(tid1));

        // Tick 9 times (quantum is 10) - no preemption yet
        for _ in 0..9 {
            let preempted = sched.tick();
            assert!(!preempted);
            assert_eq!(sched.current_tid(), Some(tid1));
        }

        // 10th tick exhausts quantum -> preemption triggers switch to task_b
        let preempted = sched.tick();
        assert!(preempted);
        assert_eq!(sched.current_tid(), Some(tid2));
    }

    #[test]
    fn test_block_and_wake_with_priority_boost() {
        let mut sched = Scheduler::new();
        let proc = sched.create_process(
            "io_app",
            ExecutionPersonality::WindowsNt,
            PriorityClass::Normal,
        );
        let pid = proc.lock().unwrap().pid;

        let t = sched
            .create_thread(pid, "worker", 0x1000, 0x2000, Priority::NORMAL)
            .unwrap();
        let tid = t.lock().unwrap().tid;

        sched.schedule();
        assert_eq!(sched.current_tid(), Some(tid));

        // Block task on I/O
        sched.block_task(tid, BlockReason::IoWait);
        assert!(t.lock().unwrap().state.is_blocked());

        // Scheduler should drop to idle thread
        assert_eq!(sched.current_tid(), Some(0));

        // Wake task -> receives priority boost (+2 from NORMAL 16 -> 18)
        let awakened = sched.wake_task(tid);
        assert!(awakened);
        assert!(t.lock().unwrap().state.is_ready());
        assert_eq!(t.lock().unwrap().current_priority, Priority(18));

        // Next schedule must pick the awakened boosted task
        assert_eq!(sched.schedule(), Some(tid));
    }

    #[test]
    fn test_sleeping_tasks() {
        let mut sched = Scheduler::new();
        let proc = sched.create_process(
            "timer_app",
            ExecutionPersonality::LinuxPosix,
            PriorityClass::Normal,
        );
        let pid = proc.lock().unwrap().pid;

        let t = sched
            .create_thread(pid, "sleeper", 0x1000, 0x2000, Priority::NORMAL)
            .unwrap();
        let tid = t.lock().unwrap().tid;

        sched.schedule();
        assert_eq!(sched.current_tid(), Some(tid));

        // Sleep current task for 5 ticks
        sched.sleep_current(5);
        assert!(t.lock().unwrap().state.is_blocked());

        // Ticks 1 to 4: task remains asleep, idle runs
        for _ in 1..=4 {
            sched.tick();
            assert!(t.lock().unwrap().state.is_blocked());
        }

        // 5th tick: sleep deadline reached, task woken and preempts/becomes ready
        let preempted = sched.tick();
        assert!(preempted);
        assert!(t.lock().unwrap().state.is_running());
        assert_eq!(sched.current_tid(), Some(tid));
    }

    #[test]
    fn test_process_termination() {
        let mut sched = Scheduler::new();
        let proc = sched.create_process(
            "terminator",
            ExecutionPersonality::HybridNative,
            PriorityClass::Normal,
        );
        let pid = proc.lock().unwrap().pid;

        let t1 = sched
            .create_thread(pid, "t1", 0x1000, 0x2000, Priority::NORMAL)
            .unwrap();
        let t2 = sched
            .create_thread(pid, "t2", 0x3000, 0x4000, Priority::NORMAL)
            .unwrap();
        let tid1 = t1.lock().unwrap().tid;
        let tid2 = t2.lock().unwrap().tid;

        sched.schedule();

        // Terminate entire process
        sched.terminate_process(pid, 42);

        assert!(proc.lock().unwrap().is_terminated());
        assert_eq!(proc.lock().unwrap().exit_code, 42);
        assert!(t1.lock().unwrap().state.is_terminated());
        assert!(t2.lock().unwrap().state.is_terminated());
        assert_eq!(sched.get_thread(tid1).unwrap().lock().unwrap().exit_code, 42);
        assert_eq!(sched.get_thread(tid2).unwrap().lock().unwrap().exit_code, 42);

        // All threads dead, scheduler defaults to idle thread
        assert_eq!(sched.schedule(), Some(0));
    }
}
