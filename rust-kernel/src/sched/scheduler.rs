//! HybridOS Round-Robin Multi-Level Priority Queue Scheduler.
//!
//! Provides cooperative and preemptive scheduling, dynamic priority boosting and decay,
//! sleep timers, and lifecycle management for dual-personality processes and threads.

#[allow(unused_imports)]
use alloc::string::{String, ToString};
#[allow(unused_imports)]
use alloc::vec::Vec;
#[allow(unused_imports)]
use alloc::boxed::Box;
#[allow(unused_imports)]
use crate::compat::FloatExt;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use crate::personality::ExecutionPersonality;
use crate::sched::task::{
    BlockReason, Priority, PriorityClass, ProcessControlBlock, TaskState,
    ThreadControlBlock,
};

/// Global scheduler metrics and execution statistics.
#[derive(Debug, Default, Clone)]
pub struct SchedulerStats {
    pub context_switches: u64,
    pub total_ticks: u64,
    pub created_processes: u64,
    pub created_threads: u64,
    pub terminated_threads: u64,
}

/// Core HybridOS Kernel Scheduler.
pub struct Scheduler {
    /// Multi-level priority queues (0..31).
    /// Index 31 = highest real-time priority, Index 0 = lowest / idle priority.
    ready_queues: [VecDeque<u64>; Priority::NUM_PRIORITIES],

    /// All threads indexed by their unique Thread ID (TID).
    threads: HashMap<u64, Arc<Mutex<ThreadControlBlock>>>,

    /// All processes indexed by their unique Process ID (PID).
    processes: HashMap<u64, Arc<Mutex<ProcessControlBlock>>>,

    /// Currently running thread ID on this scheduler core.
    current_tid: Option<u64>,

    /// Monotonically increasing next available TID.
    next_tid: u64,

    /// Monotonically increasing next available PID.
    next_pid: u64,

    /// Global timer tick counter.
    system_ticks: u64,

    /// Scheduler execution statistics.
    stats: SchedulerStats,

    /// System Idle Thread identifier.
    idle_tid: u64,

    /// System Idle Process identifier.
    idle_pid: u64,
}

impl Default for Scheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl Scheduler {
    /// Instantiate and initialize the scheduler with a System Idle task.
    pub fn new() -> Self {
        const EMPTY_QUEUE: VecDeque<u64> = VecDeque::new();
        let mut scheduler = Self {
            ready_queues: [EMPTY_QUEUE; Priority::NUM_PRIORITIES],
            threads: HashMap::new(),
            processes: HashMap::new(),
            current_tid: None,
            next_tid: 0,
            next_pid: 0,
            system_ticks: 0,
            stats: SchedulerStats::default(),
            idle_tid: 0,
            idle_pid: 0,
        };

        // Bootstrap the System Idle Process (PID 0)
        let idle_proc = Arc::new(Mutex::new(ProcessControlBlock::new(
            0,
            0,
            "System Idle Process",
            ExecutionPersonality::HybridNative,
            PriorityClass::Idle,
        )));
        scheduler.processes.insert(0, idle_proc);
        scheduler.next_pid = 1;

        // Bootstrap the System Idle Thread (TID 0) at Priority::IDLE (0)
        let idle_thread = Arc::new(Mutex::new(ThreadControlBlock::new(
            0,
            0,
            "system_idle_worker",
            ExecutionPersonality::HybridNative,
            0x0000_0000_FFFF_0000,
            0x0000_0000_FFFF_FFF0,
            Priority::IDLE,
            0x1000_0000,
        )));
        scheduler.threads.insert(0, idle_thread);
        scheduler.ready_queues[Priority::IDLE.as_usize()].push_back(0);
        scheduler.next_tid = 1;
        scheduler.idle_tid = 0;
        scheduler.idle_pid = 0;

        scheduler
    }

    /// Primary scheduling dispatch loop.
    ///
    /// Selects the next ready thread from the highest non-empty priority queue (31 down to 0).
    /// Performs round-robin rotation within identical priority levels.
    /// Transitions previous running task to Ready (if still active) and newly selected task to Running.
    pub fn schedule(&mut self) -> Option<u64> {
        let prev_tid = self.current_tid;

        // 1. If previous thread was Running, re-enqueue it as Ready at back of its priority queue
        if let Some(cur_id) = prev_tid {
            let mut requeue_prio = None;
            if let Some(tcb_arc) = self.threads.get(&cur_id) {
                let mut tcb = tcb_arc.lock().unwrap();
                if tcb.state == TaskState::Running {
                    tcb.state = TaskState::Ready;
                    requeue_prio = Some(tcb.current_priority.as_usize());
                }
            }
            if let Some(prio) = requeue_prio {
                self.ready_queues[prio].push_back(cur_id);
            }
        }

        // 2. Select next ready thread starting from highest priority down to idle
        let mut next_tid = None;
        for prio_idx in (0..Priority::NUM_PRIORITIES).rev() {
            while let Some(candidate_tid) = self.ready_queues[prio_idx].pop_front() {
                if let Some(tcb_arc) = self.threads.get(&candidate_tid) {
                    let mut tcb = tcb_arc.lock().unwrap();
                    if tcb.state == TaskState::Ready {
                        tcb.state = TaskState::Running;
                        tcb.reset_quantum();
                        tcb.switch_count += 1;
                        next_tid = Some(candidate_tid);
                        break;
                    }
                }
            }
            if next_tid.is_some() {
                break;
            }
        }

        // 3. Switch context
        self.current_tid = next_tid;
        if next_tid.is_some() {
            self.stats.context_switches += 1;
        }

        next_tid
    }

    /// Cooperative yield: the currently running thread forfeits remaining time slice.
    /// The thread is returned to the back of its priority queue and `schedule()` is invoked.
    pub fn yield_cpu(&mut self) -> Option<u64> {
        self.schedule()
    }

    /// Block a thread awaiting an event, I/O, sleep timer, or lock.
    /// Removes the task from all ready queues. If the blocked task was currently running,
    /// triggers an immediate rescheduling cycle.
    pub fn block_task(&mut self, tid: u64, reason: BlockReason) {
        if let Some(tcb_arc) = self.threads.get(&tid) {
            let mut tcb = tcb_arc.lock().unwrap();
            if tcb.state == TaskState::Terminated {
                return;
            }
            tcb.block(reason);
        }

        // Scrub from all ready queues
        for queue in &mut self.ready_queues {
            queue.retain(|&t| t != tid);
        }

        // If the currently running thread was blocked, reschedule immediately
        if self.current_tid == Some(tid) {
            self.current_tid = None;
            self.schedule();
        }
    }

    /// Wake a previously blocked thread.
    ///
    /// Transitions thread state back to `Ready`, applies an interactive priority boost
    /// (+2 priority levels, up to max non-realtime), and enqueues into ready queue.
    pub fn wake_task(&mut self, tid: u64) -> bool {
        let mut awakened_prio = None;
        if let Some(tcb_arc) = self.threads.get(&tid) {
            let mut tcb = tcb_arc.lock().unwrap();
            if tcb.state == TaskState::Blocked {
                tcb.wake();
                // Interactive boost for unblocked threads (bounded to HIGHEST to avoid unbounded real-time escalation)
                if tcb.current_priority < Priority::HIGHEST {
                    tcb.boost_priority(2);
                }
                let prio = tcb.current_priority.as_usize();
                awakened_prio = Some(prio);
            }
        }

        if let Some(prio) = awakened_prio {
            self.ready_queues[prio].push_back(tid);
            return true;
        }

        false
    }

    /// Timer tick handler for preemptive multi-tasking.
    ///
    /// 1. Advances global tick count.
    /// 2. Awakens sleeping threads whose timer deadlines have elapsed.
    /// 3. Consumes quantum tick from current running thread.
    /// 4. Reschedules if quantum expired or a higher-priority task became runnable.
    pub fn tick(&mut self) -> bool {
        self.system_ticks += 1;
        self.stats.total_ticks += 1;

        // Step 1: Check and wake sleeping threads
        let mut sleeping_wakeups = Vec::new();
        for (&tid, tcb_arc) in &self.threads {
            let tcb = tcb_arc.lock().unwrap();
            if tcb.state == TaskState::Blocked {
                if let Some(BlockReason::Sleep(target_tick)) = tcb.block_reason {
                    if self.system_ticks >= target_tick {
                        sleeping_wakeups.push(tid);
                    }
                }
            }
        }
        for tid in sleeping_wakeups {
            self.wake_task(tid);
        }

        // Step 2: Update currently running thread statistics & quantum
        let mut quantum_expired = false;
        if let Some(cur_id) = self.current_tid {
            if let Some(tcb_arc) = self.threads.get(&cur_id) {
                let mut tcb = tcb_arc.lock().unwrap();
                tcb.total_ticks_run += 1;

                let pid = tcb.pid;
                if let Some(pcb_arc) = self.processes.get(&pid) {
                    let mut pcb = pcb_arc.lock().unwrap();
                    pcb.total_cpu_ticks += 1;
                }

                quantum_expired = tcb.tick();
                if quantum_expired {
                    // Decay priority after full quantum exhaustion
                    tcb.decay_priority();
                }
            }
        }

        // Step 3: Reschedule if quantum expired
        if quantum_expired {
            self.schedule();
            return true;
        }

        // Step 4: Check if any higher priority thread became ready
        if let Some(cur_id) = self.current_tid {
            let cur_prio = self
                .threads
                .get(&cur_id)
                .map(|t| t.lock().unwrap().current_priority.as_usize())
                .unwrap_or(0);

            for p in (cur_prio + 1..Priority::NUM_PRIORITIES).rev() {
                if !self.ready_queues[p].is_empty() {
                    self.schedule();
                    return true;
                }
            }
        }

        false
    }

    /// Put currently running thread to sleep for given number of timer ticks.
    pub fn sleep_current(&mut self, ticks: u64) -> bool {
        if let Some(cur_id) = self.current_tid {
            let target_tick = self.system_ticks + ticks;
            self.block_task(cur_id, BlockReason::Sleep(target_tick));
            true
        } else {
            false
        }
    }

    /// Create a new process in the kernel process table.
    pub fn create_process(
        &mut self,
        name: &str,
        personality: ExecutionPersonality,
        priority_class: PriorityClass,
    ) -> Arc<Mutex<ProcessControlBlock>> {
        let pid = self.next_pid;
        self.next_pid += 1;

        let mut pcb = ProcessControlBlock::new(pid, 0, name, personality, priority_class);
        pcb.creation_tick = self.system_ticks;
        let pcb_arc = Arc::new(Mutex::new(pcb));

        self.processes.insert(pid, pcb_arc.clone());
        self.stats.created_processes += 1;
        pcb_arc
    }

    /// Create a new thread associated with a process, initialized and placed in Ready queue.
    pub fn create_thread(
        &mut self,
        pid: u64,
        name: &str,
        entry_point: u64,
        stack_top: u64,
        priority: Priority,
    ) -> Result<Arc<Mutex<ThreadControlBlock>>, &'static str> {
        let (personality, cr3) = match self.processes.get(&pid) {
            Some(pcb_arc) => {
                let pcb = pcb_arc.lock().unwrap();
                if pcb.is_terminated() {
                    return Err("Cannot create thread in terminated process");
                }
                (pcb.personality, pcb.page_table_base)
            }
            None => return Err("Target process PID not found"),
        };

        let tid = self.next_tid;
        self.next_tid += 1;

        let tcb = ThreadControlBlock::new(
            tid,
            pid,
            name,
            personality,
            entry_point,
            stack_top,
            priority,
            cr3,
        );
        let tcb_arc = Arc::new(Mutex::new(tcb));

        // Register in process
        if let Some(pcb_arc) = self.processes.get(&pid) {
            let mut pcb = pcb_arc.lock().unwrap();
            pcb.add_thread(tid);
        }

        // Register in scheduler thread map and ready queue
        self.threads.insert(tid, tcb_arc.clone());
        self.ready_queues[priority.as_usize()].push_back(tid);
        self.stats.created_threads += 1;

        Ok(tcb_arc)
    }

    /// Terminate a specific thread.
    pub fn terminate_thread(&mut self, tid: u64, exit_code: i32) -> bool {
        // Disallow killing idle thread
        if tid == self.idle_tid {
            return false;
        }

        let mut removed = false;
        if let Some(tcb_arc) = self.threads.get(&tid) {
            let mut tcb = tcb_arc.lock().unwrap();
            tcb.terminate(exit_code);
            removed = true;
        }

        if removed {
            for queue in &mut self.ready_queues {
                queue.retain(|&t| t != tid);
            }
            self.stats.terminated_threads += 1;

            if self.current_tid == Some(tid) {
                self.current_tid = None;
                self.schedule();
            }
            return true;
        }

        false
    }

    /// Terminate an entire process and all of its associated threads.
    pub fn terminate_process(&mut self, pid: u64, exit_code: i32) -> bool {
        // Disallow killing idle process
        if pid == self.idle_pid {
            return false;
        }

        let threads_to_kill = if let Some(pcb_arc) = self.processes.get(&pid) {
            let mut pcb = pcb_arc.lock().unwrap();
            pcb.terminate(exit_code);
            pcb.threads.clone()
        } else {
            return false;
        };

        for tid in threads_to_kill {
            self.terminate_thread(tid, exit_code);
        }

        true
    }

    /// Retrieve currently executing thread ID.
    pub fn current_tid(&self) -> Option<u64> {
        self.current_tid
    }

    /// Retrieve currently executing thread control block.
    pub fn current_thread(&self) -> Option<Arc<Mutex<ThreadControlBlock>>> {
        self.current_tid.and_then(|tid| self.threads.get(&tid).cloned())
    }

    /// Retrieve currently executing process control block.
    pub fn current_process(&self) -> Option<Arc<Mutex<ProcessControlBlock>>> {
        self.current_thread().and_then(|tcb_arc| {
            let pid = tcb_arc.lock().unwrap().pid;
            self.processes.get(&pid).cloned()
        })
    }

    /// Query thread control block by TID.
    pub fn get_thread(&self, tid: u64) -> Option<Arc<Mutex<ThreadControlBlock>>> {
        self.threads.get(&tid).cloned()
    }

    /// Query process control block by PID.
    pub fn get_process(&self, pid: u64) -> Option<Arc<Mutex<ProcessControlBlock>>> {
        self.processes.get(&pid).cloned()
    }

    /// Return snapshot of scheduler statistics.
    pub fn stats(&self) -> SchedulerStats {
        self.stats.clone()
    }

    /// Global tick count.
    pub fn system_ticks(&self) -> u64 {
        self.system_ticks
    }

    /// Count of currently active threads in ready queues.
    pub fn ready_count(&self) -> usize {
        self.ready_queues.iter().map(|q| q.len()).sum()
    }

    /// Format detailed ASCII diagnostic overview of scheduler state.
    pub fn dump_state(&self) -> String {
        let mut out = String::new();
        out.push_str("================================================================================\n");
        out.push_str("                   HYBRIDOS MULTI-QUEUE SCHEDULER OVERVIEW\n");
        out.push_str("================================================================================\n");
        out.push_str(&format!(
            "Ticks: {:<8} | Switches: {:<8} | Current TID: {:<6} | Ready Threads: {}\n",
            self.system_ticks,
            self.stats.context_switches,
            self.current_tid.map(|t| t.to_string()).unwrap_or_else(|| "None".into()),
            self.ready_count()
        ));
        out.push_str("--------------------------------------------------------------------------------\n");
        out.push_str("ACTIVE READY QUEUES (Highest to Lowest):\n");
        for prio_idx in (0..Priority::NUM_PRIORITIES).rev() {
            let q = &self.ready_queues[prio_idx];
            if !q.is_empty() {
                out.push_str(&format!("  [Priority {:2}]: {:?}\n", prio_idx, q));
            }
        }
        out.push_str("--------------------------------------------------------------------------------\n");
        out.push_str("PROCESSES:\n");
        for (&pid, pcb_arc) in &self.processes {
            let pcb = pcb_arc.lock().unwrap();
            out.push_str(&format!(
                "  PID {:<4} | {:<20} | ABI: {:<16} | Threads: {:?}\n",
                pid,
                pcb.name,
                pcb.personality.name(),
                pcb.threads
            ));
        }
        out.push_str("--------------------------------------------------------------------------------\n");
        out.push_str("THREADS:\n");
        for (&tid, tcb_arc) in &self.threads {
            let tcb = tcb_arc.lock().unwrap();
            out.push_str(&format!(
                "  TID {:<4} | PID {:<4} | {:<20} | State: {:<10} | Prio: {:2}/{:2} | Switches: {:<4} | Ticks: {:<4}\n",
                tid,
                tcb.pid,
                tcb.name,
                format!("{:?}", tcb.state),
                tcb.current_priority.0,
                tcb.base_priority.0,
                tcb.switch_count,
                tcb.total_ticks_run
            ));
        }
        out.push_str("================================================================================\n");
        out
    }
}
