//! HybridOS Task, Thread, and Process Control Blocks.
//! Supports dual-personality (Windows NT and Linux POSIX) execution environments.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use crate::personality::ExecutionPersonality;
use crate::process::{HybridProcess, KernelObject};
use crate::trap::TrapFrame;
use crate::types::Handle;

/// Execution state of a thread / task within the kernel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    /// Thread is created and ready to be scheduled.
    Ready,
    /// Thread is currently executing on a CPU core.
    Running,
    /// Thread is waiting on an event, I/O, sleep timer, or synchronization primitive.
    Blocked,
    /// Thread execution has terminated and resources are pending reclamation.
    Terminated,
}

impl TaskState {
    pub fn is_ready(&self) -> bool {
        matches!(self, TaskState::Ready)
    }

    pub fn is_running(&self) -> bool {
        matches!(self, TaskState::Running)
    }

    pub fn is_blocked(&self) -> bool {
        matches!(self, TaskState::Blocked)
    }

    pub fn is_terminated(&self) -> bool {
        matches!(self, TaskState::Terminated)
    }
}

/// Reason a thread entered the `Blocked` state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockReason {
    /// Cooperatively yielding CPU.
    Yield,
    /// Sleeping until target timer tick.
    Sleep(u64),
    /// Waiting for asynchronous I/O completion.
    IoWait,
    /// Contending on a mutex or spinlock.
    MutexLock,
    /// Waiting on an event / condition variable / futex.
    EventWait,
    /// Waiting on child process or thread termination (e.g. waitpid / WaitForSingleObject).
    WaitChild(u64),
    /// Waiting on an IPC channel or message queue.
    IpcChannel,
    /// Custom / user-defined blocking reason code.
    Custom(u32),
}

/// Priority representation across HybridOS subsystems.
///
/// Supports 32 priority levels (0..31):
/// - 0: System Idle
/// - 1..7: Low priority / background batch
/// - 8..15: Below normal
/// - 16..23: Normal interactive priority (default = 16)
/// - 24..27: High priority
/// - 28..30: Real-time soft priority
/// - 31: Hard Real-time critical priority
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Priority(pub u8);

impl Priority {
    pub const IDLE: Priority = Priority(0);
    pub const LOWEST: Priority = Priority(1);
    pub const LOW: Priority = Priority(8);
    pub const NORMAL: Priority = Priority(16);
    pub const HIGH: Priority = Priority(24);
    pub const HIGHEST: Priority = Priority(28);
    pub const REALTIME: Priority = Priority(31);

    pub const NUM_PRIORITIES: usize = 32;
    pub const DEFAULT: Priority = Priority(16);

    pub fn new(level: u8) -> Self {
        Priority(level.min(31))
    }

    pub fn as_usize(&self) -> usize {
        self.0 as usize
    }

    /// Boost priority by delta (used when unblocking from I/O or interactive events).
    pub fn boost(&self, delta: u8) -> Self {
        Priority((self.0 + delta).min(31))
    }

    /// Decay dynamic priority towards base priority after quantum expiration.
    pub fn decay(&self, delta: u8, base: Priority) -> Self {
        let dec = self.0.saturating_sub(delta);
        Priority(dec.max(base.0))
    }
}

/// High-level priority classes mirroring Windows NT and POSIX niceness ranges.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PriorityClass {
    Idle,
    BelowNormal,
    Normal,
    AboveNormal,
    High,
    Realtime,
}

impl PriorityClass {
    pub fn base_priority(&self) -> Priority {
        match self {
            Self::Idle => Priority::IDLE,
            Self::BelowNormal => Priority::LOW,
            Self::Normal => Priority::NORMAL,
            Self::AboveNormal => Priority::new(20),
            Self::High => Priority::HIGH,
            Self::Realtime => Priority::REALTIME,
        }
    }
}

/// x86_64 CPU Context containing full architectural register state.
///
/// Supports both System V AMD64 ABI (Linux) and Microsoft x64 Calling Convention (Windows NT),
/// along with segment registers and Thread Local Storage (FS base for Linux, GS base for Windows).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct CpuContext {
    // General purpose registers
    pub rax: u64,
    pub rbx: u64,
    pub rcx: u64, // Win64 Arg 1
    pub rdx: u64, // Win64 Arg 2 / Linux Arg 3
    pub rsi: u64, // Linux Arg 2
    pub rdi: u64, // Linux Arg 1
    pub rbp: u64,
    pub rsp: u64,
    pub r8:  u64, // Win64 Arg 3 / Linux Arg 5
    pub r9:  u64, // Win64 Arg 4 / Linux Arg 6
    pub r10: u64, // Linux Arg 4
    pub r11: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,

    // Instruction pointer and CPU flags
    pub rip: u64,
    pub rflags: u64,

    // Segment selectors
    pub cs: u64,
    pub ss: u64,

    // Thread Local Storage base registers:
    // Linux POSIX uses FS_BASE for pthread_self / TLS
    // Windows NT uses GS_BASE for TEB (Thread Environment Block)
    pub fs_base: u64,
    pub gs_base: u64,

    // Page table root (CR3 register)
    pub cr3: u64,
}

impl Default for CpuContext {
    fn default() -> Self {
        Self {
            rax: 0,
            rbx: 0,
            rcx: 0,
            rdx: 0,
            rsi: 0,
            rdi: 0,
            rbp: 0,
            rsp: 0,
            r8: 0,
            r9: 0,
            r10: 0,
            r11: 0,
            r12: 0,
            r13: 0,
            r14: 0,
            r15: 0,
            rip: 0,
            rflags: 0x202, // IF (Interrupt Enable Flag) set
            cs: 0x23,      // Default Ring 3 code selector
            ss: 0x2B,      // Default Ring 3 data selector
            fs_base: 0,
            gs_base: 0,
            cr3: 0,
        }
    }
}

impl CpuContext {
    /// Initialize a new execution context for a thread entry point.
    pub fn new_entry(
        entry_point: u64,
        stack_top: u64,
        arg: u64,
        personality: ExecutionPersonality,
        cr3: u64,
    ) -> Self {
        let mut ctx = Self::default();
        ctx.rip = entry_point;
        ctx.rsp = stack_top;
        ctx.rbp = stack_top;
        ctx.cr3 = cr3;

        match personality {
            ExecutionPersonality::WindowsNt => {
                // Windows x64 calling convention: First arg in RCX
                ctx.rcx = arg;
                // Reserve 32-byte shadow space on stack
                if ctx.rsp >= 32 {
                    ctx.rsp -= 32;
                }
            }
            ExecutionPersonality::LinuxPosix | ExecutionPersonality::Android => {
                // System V AMD64 ABI: First arg in RDI
                ctx.rdi = arg;
            }
            ExecutionPersonality::HybridNative => {
                ctx.rdi = arg;
                ctx.cs = 0x10; // Kernel mode code segment
                ctx.ss = 0x18; // Kernel mode data segment
            }
        }

        ctx
    }

    /// Construct `CpuContext` from a kernel `TrapFrame`.
    pub fn from_trap_frame(tf: &TrapFrame, cr3: u64) -> Self {
        Self {
            rax: tf.rax,
            rbx: tf.rbx,
            rcx: tf.rcx,
            rdx: tf.rdx,
            rsi: tf.rsi,
            rdi: tf.rdi,
            rbp: tf.rbp,
            rsp: tf.rsp,
            r8:  tf.r8,
            r9:  tf.r9,
            r10: tf.r10,
            r11: tf.r11,
            r12: tf.r12,
            r13: tf.r13,
            r14: tf.r14,
            r15: tf.r15,
            rip: tf.rip,
            rflags: tf.rflags,
            cs: tf.cs,
            ss: tf.ss,
            fs_base: 0,
            gs_base: 0,
            cr3,
        }
    }

    /// Export `CpuContext` into a `TrapFrame`.
    pub fn to_trap_frame(&self) -> TrapFrame {
        TrapFrame {
            rax: self.rax,
            rbx: self.rbx,
            rcx: self.rcx,
            rdx: self.rdx,
            rsi: self.rsi,
            rdi: self.rdi,
            rbp: self.rbp,
            rsp: self.rsp,
            r8:  self.r8,
            r9:  self.r9,
            r10: self.r10,
            r11: self.r11,
            r12: self.r12,
            r13: self.r13,
            r14: self.r14,
            r15: self.r15,
            rip: self.rip,
            rflags: self.rflags,
            cs: self.cs,
            ss: self.ss,
        }
    }
}

/// Thread Control Block (TCB) managing a schedulable execution entity.
#[derive(Debug, Clone)]
pub struct ThreadControlBlock {
    /// Unique thread identifier.
    pub tid: u64,
    /// Parent process identifier.
    pub pid: u64,
    /// Thread descriptor name.
    pub name: String,
    /// Current execution state.
    pub state: TaskState,
    /// Reason if thread is blocked.
    pub block_reason: Option<BlockReason>,

    /// Statically configured base priority.
    pub base_priority: Priority,
    /// Dynamically adjusted runtime priority.
    pub current_priority: Priority,

    /// Complete CPU register and context state.
    pub context: CpuContext,

    /// Kernel execution stack top and size.
    pub kernel_stack_top: u64,
    pub kernel_stack_size: usize,
    /// User mode stack top and size.
    pub user_stack_top: u64,
    pub user_stack_size: usize,

    /// Allocated time quantum per schedule in timer ticks.
    pub time_slice_quantum: u32,
    /// Remaining ticks in the current quantum.
    pub remaining_ticks: u32,
    /// Lifetime ticks executed on CPU.
    pub total_ticks_run: u64,
    /// Total context switches into this thread.
    pub switch_count: u64,
    /// Optional timestamp/tick when sleeping thread should be awakened.
    pub wake_tick: Option<u64>,

    /// Subsystem execution personality (Windows NT vs Linux POSIX).
    pub personality: ExecutionPersonality,
    /// Target CPU core affinity bitmask (default: all cores 0xFFFF_FFFF_FFFF_FFFF).
    pub cpu_affinity: u64,
    /// Return / exit status upon termination.
    pub exit_code: i32,
}

impl ThreadControlBlock {
    pub const DEFAULT_QUANTUM_TICKS: u32 = 10;

    /// Create a new thread control block.
    pub fn new(
        tid: u64,
        pid: u64,
        name: &str,
        personality: ExecutionPersonality,
        entry_point: u64,
        stack_top: u64,
        priority: Priority,
        cr3: u64,
    ) -> Self {
        let context = CpuContext::new_entry(entry_point, stack_top, 0, personality, cr3);

        Self {
            tid,
            pid,
            name: name.to_string(),
            state: TaskState::Ready,
            block_reason: None,
            base_priority: priority,
            current_priority: priority,
            context,
            kernel_stack_top: 0,
            kernel_stack_size: 16 * 1024, // 16 KB default kernel stack
            user_stack_top: stack_top,
            user_stack_size: 2 * 1024 * 1024, // 2 MB default user stack
            time_slice_quantum: Self::DEFAULT_QUANTUM_TICKS,
            remaining_ticks: Self::DEFAULT_QUANTUM_TICKS,
            total_ticks_run: 0,
            switch_count: 0,
            wake_tick: None,
            personality,
            cpu_affinity: u64::MAX,
            exit_code: 0,
        }
    }

    /// Consume a tick from the current quantum.
    /// Returns `true` if the quantum is exhausted.
    pub fn tick(&mut self) -> bool {
        if self.remaining_ticks > 0 {
            self.remaining_ticks -= 1;
        }
        self.remaining_ticks == 0
    }

    /// Reset remaining ticks back to the allocated quantum.
    pub fn reset_quantum(&mut self) {
        self.remaining_ticks = self.time_slice_quantum;
    }

    /// Boost dynamic priority by delta up to `Priority::REALTIME`.
    pub fn boost_priority(&mut self, delta: u8) {
        self.current_priority = self.current_priority.boost(delta);
    }

    /// Gradually decay dynamic priority towards base priority.
    pub fn decay_priority(&mut self) {
        self.current_priority = self.current_priority.decay(1, self.base_priority);
    }

    /// Transition to Blocked state with a reason.
    pub fn block(&mut self, reason: BlockReason) {
        self.state = TaskState::Blocked;
        self.block_reason = Some(reason);
    }

    /// Transition from Blocked to Ready state.
    pub fn wake(&mut self) {
        self.state = TaskState::Ready;
        self.block_reason = None;
        self.wake_tick = None;
    }

    /// Terminate the thread with an exit code.
    pub fn terminate(&mut self, exit_code: i32) {
        self.state = TaskState::Terminated;
        self.exit_code = exit_code;
    }
}

/// Lifecycle state of a process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessState {
    Active,
    Terminated,
    Zombie,
}

/// Process Control Block (PCB) representing a complete application instance.
#[derive(Debug)]
pub struct ProcessControlBlock {
    pub pid: u64,
    pub ppid: u64,
    pub name: String,
    pub personality: ExecutionPersonality,
    pub priority_class: PriorityClass,
    pub state: ProcessState,
    pub threads: Vec<u64>,
    pub main_thread_tid: u64,

    // Virtual memory space
    pub page_table_base: u64, // CR3
    pub image_base: u64,
    pub entry_point: u64,
    pub image_size: usize,

    // Object and Handle table
    pub handles: HashMap<Handle, Arc<Mutex<KernelObject>>>,
    pub next_handle: usize,

    // Credentials & Security
    pub uid: u32,
    pub gid: u32,
    pub win_token_id: u64,

    // Termination
    pub exit_code: i32,

    // Metrics
    pub creation_tick: u64,
    pub total_cpu_ticks: u64,
}

impl ProcessControlBlock {
    pub fn new(
        pid: u64,
        ppid: u64,
        name: &str,
        personality: ExecutionPersonality,
        priority_class: PriorityClass,
    ) -> Self {
        let mut pcb = Self {
            pid,
            ppid,
            name: name.to_string(),
            personality,
            priority_class,
            state: ProcessState::Active,
            threads: Vec::new(),
            main_thread_tid: 0,
            page_table_base: 0x1000_0000,
            image_base: 0,
            entry_point: 0,
            image_size: 0,
            handles: HashMap::new(),
            next_handle: 3,
            uid: 1000,
            gid: 1000,
            win_token_id: 1,
            exit_code: 0,
            creation_tick: 0,
            total_cpu_ticks: 0,
        };

        // Standard stream descriptors: 0 = stdin, 1 = stdout, 2 = stderr
        pcb.handles.insert(
            Handle::STDIN,
            Arc::new(Mutex::new(KernelObject {
                obj_type: crate::process::ObjectType::Device {
                    name: "console_in".into(),
                },
                access_mask: 0x1,
            })),
        );
        pcb.handles.insert(
            Handle::STDOUT,
            Arc::new(Mutex::new(KernelObject {
                obj_type: crate::process::ObjectType::Device {
                    name: "console_out".into(),
                },
                access_mask: 0x2,
            })),
        );
        pcb.handles.insert(
            Handle::STDERR,
            Arc::new(Mutex::new(KernelObject {
                obj_type: crate::process::ObjectType::Device {
                    name: "console_err".into(),
                },
                access_mask: 0x2,
            })),
        );

        pcb
    }

    /// Bridge constructor from existing `HybridProcess`.
    pub fn from_hybrid_process(proc: HybridProcess) -> Self {
        Self {
            pid: proc.pid as u64,
            ppid: 0,
            name: proc.name,
            personality: proc.personality,
            priority_class: PriorityClass::Normal,
            state: if proc.is_terminated {
                ProcessState::Terminated
            } else {
                ProcessState::Active
            },
            threads: Vec::new(),
            main_thread_tid: 0,
            page_table_base: 0x1000_0000,
            image_base: proc.image_base,
            entry_point: proc.entry_point,
            image_size: proc.image_size,
            handles: proc.handles,
            next_handle: proc.next_handle,
            uid: 1000,
            gid: 1000,
            win_token_id: 1,
            exit_code: proc.exit_code,
            creation_tick: 0,
            total_cpu_ticks: 0,
        }
    }

    pub fn add_thread(&mut self, tid: u64) {
        if !self.threads.contains(&tid) {
            self.threads.push(tid);
            if self.main_thread_tid == 0 {
                self.main_thread_tid = tid;
            }
        }
    }

    pub fn remove_thread(&mut self, tid: u64) {
        self.threads.retain(|&t| t != tid);
    }

    pub fn allocate_handle(&mut self, obj: KernelObject) -> Handle {
        let handle = Handle(self.next_handle);
        self.next_handle += 1;
        self.handles.insert(handle, Arc::new(Mutex::new(obj)));
        handle
    }

    pub fn get_object(&self, handle: Handle) -> Option<Arc<Mutex<KernelObject>>> {
        self.handles.get(&handle).cloned()
    }

    pub fn close_handle(&mut self, handle: Handle) -> bool {
        self.handles.remove(&handle).is_some()
    }

    pub fn terminate(&mut self, exit_code: i32) {
        self.state = ProcessState::Terminated;
        self.exit_code = exit_code;
    }

    pub fn is_active(&self) -> bool {
        matches!(self.state, ProcessState::Active)
    }

    pub fn is_terminated(&self) -> bool {
        matches!(self.state, ProcessState::Terminated)
    }
}
