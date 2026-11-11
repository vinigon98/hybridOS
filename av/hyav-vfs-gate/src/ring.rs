//! Lockless 128 KB Shared Memory Ring Buffer (Submission & Completion Queues).
//!
//! Implements high-throughput kernel-to-userland IPC using atomic Acquire/Release
//! memory ordering semantics over a contiguous 128 KiB shared memory region.
//!
//! Cache-line padding (64 bytes) is used on all atomic indices to completely
//! prevent false sharing between producer and consumer cores.

use core::fmt;
use core::sync::atomic::{AtomicU32, Ordering};

/// Total size of the shared memory ring buffer: exactly 128 KiB (131,072 bytes).
pub const RING_TOTAL_SIZE: usize = 128 * 1024;

/// Capacity of the Submission Queue (SQ) in entries. Power-of-two (256).
pub const SQ_CAPACITY: u32 = 256;
pub const SQ_MASK: u32 = SQ_CAPACITY - 1;

/// Capacity of the Completion Queue (CQ) in entries. Power-of-two (512).
pub const CQ_CAPACITY: u32 = 512;
pub const CQ_MASK: u32 = CQ_CAPACITY - 1;

/// Magic identifier for shared ring buffer ("HYSQ" = 0x48595351).
pub const RING_MAGIC: u32 = 0x48595351;
pub const RING_VERSION: u16 = 1;

/// Flags for submission entries.
pub mod entry_flags {
    /// Target is mapped as an executable NT binary image (`SEC_IMAGE`).
    pub const SEC_IMAGE: u8 = 0x01;
    /// Direct read-only inspection access requested.
    pub const READ_ONLY: u8 = 0x02;
    /// Urgent synchronous priority path.
    pub const REALTIME_PRIORITY: u8 = 0x04;
    /// Memory descriptor list (MDL) backing is attached.
    pub const HAS_MDL: u8 = 0x08;
}

/// Errors returned by lockless ring operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RingError {
    /// Ring submission queue is at maximum capacity.
    QueueFull,
    /// Ring completion queue has no available entries.
    QueueEmpty,
    /// Ring buffer memory alignment is invalid.
    InvalidAlignment,
    /// Ring buffer size does not match required 128 KiB.
    InvalidSize,
    /// Magic number or version mismatch in shared header.
    CorruptHeader,
}

impl fmt::Display for RingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::QueueFull => write!(f, "Shared ring queue is full"),
            Self::QueueEmpty => write!(f, "Shared ring queue is empty"),
            Self::InvalidAlignment => write!(f, "Ring buffer is not properly cache-line aligned"),
            Self::InvalidSize => write!(f, "Ring buffer size does not match required 128 KB"),
            Self::CorruptHeader => write!(f, "Ring header magic or version is invalid"),
        }
    }
}

impl std::error::Error for RingError {}

/// Submission Queue Entry (SQE) - exactly 256 bytes, cache-aligned.
#[repr(C, align(64))]
#[derive(Clone, Copy)]
pub struct SqEntry {
    /// Unique monotonically increasing request identifier.
    pub request_id: u64,
    /// High-resolution timestamp when kernel intercepted operation (ns).
    pub timestamp_ns: u64,
    /// Target virtual address of zero-copy projected MDL buffer.
    pub mdl_va: u64,
    /// Size of MDL buffer in bytes.
    pub mdl_byte_count: u64,
    /// Number of physical pages spanned by the MDL.
    pub mdl_page_count: u32,
    /// Process ID issuing intercepted VFS operation.
    pub pid: u32,
    /// Thread ID issuing operation.
    pub tid: u32,
    /// Subsystem personality: 0 = Windows NT, 1 = Linux POSIX.
    pub subsystem: u8,
    /// Interception hook type: 0 = PreExec, 1 = PreOpen, 2 = PreSection, 3 = PostClose.
    pub hook_type: u8,
    /// Entry flags (e.g. `entry_flags::SEC_IMAGE`).
    pub flags: u8,
    /// Operation opcode (0 = ScanMdl, 1 = ScanPath, 2 = InvalidateCache).
    pub opcode: u8,
    /// Scan reason taxonomy code (matches SCANREASON.md, e.g. 0x0001 = EXEC_PE).
    pub scan_reason: u16,
    /// Worker degradation timeout in milliseconds.
    pub timeout_ms: u16,
    /// Length of file path in `path_bytes`.
    pub path_len: u16,
    /// Reserved alignment padding.
    pub _pad0: [u8; 6],
    /// UTF-8 file path of intercepted object.
    pub path_bytes: [u8; 184],
}

impl Default for SqEntry {
    fn default() -> Self {
        Self {
            request_id: 0,
            timestamp_ns: 0,
            mdl_va: 0,
            mdl_byte_count: 0,
            mdl_page_count: 0,
            pid: 0,
            tid: 0,
            subsystem: 0,
            hook_type: 0,
            flags: 0,
            opcode: 0,
            scan_reason: 0,
            timeout_ms: 500,
            path_len: 0,
            _pad0: [0; 6],
            path_bytes: [0; 184],
        }
    }
}

/// Completion Queue Entry (CQE) - exactly 64 bytes (1 cache line), cache-aligned.
#[repr(C, align(64))]
#[derive(Clone, Copy)]
pub struct CqEntry {
    /// Corresponds to matching `SqEntry.request_id`.
    pub request_id: u64,
    /// High-resolution latency of inspection (ns).
    pub latency_ns: u64,
    /// NTSTATUS result (e.g. STATUS_SUCCESS, STATUS_PENDING, STATUS_VIRUS_INFECTED).
    pub status: u32,
    /// Inspection verdict: 0 = Allow, 1 = Block, 2 = Quarantine, 3 = DegradedAllow.
    pub verdict: u8,
    /// Response flags.
    pub flags: u8,
    /// Reason or detection category code.
    pub reason_code: u16,
    /// Heuristic threat score (0-1000).
    pub scan_score: u32,
    /// Threat detection identifier / signature hash.
    pub detection_id: [u8; 32],
}

impl Default for CqEntry {
    fn default() -> Self {
        Self {
            request_id: 0,
            latency_ns: 0,
            status: 0,
            verdict: 0,
            flags: 0,
            reason_code: 0,
            scan_score: 0,
            detection_id: [0; 32],
        }
    }
}

/// Header governing shared memory ring queues.
///
/// Designed with explicit 64-byte padding between atomic variables to prevent
/// multi-core cache line thrashing.
#[repr(C, align(64))]
pub struct RingHeader {
    pub magic: u32,
    pub version: u16,
    pub flags: u16,
    pub total_bytes: u32,
    pub sq_capacity: u32,
    pub cq_capacity: u32,
    pub sq_entry_size: u32,
    pub cq_entry_size: u32,
    pub _pad_head: [u8; 36],

    // SQ Head: consumer (worker) read index.
    pub sq_head: AtomicU32,
    pub _pad_sq_head: [u8; 60],

    // SQ Tail: producer (kernel gate) write index.
    pub sq_tail: AtomicU32,
    pub _pad_sq_tail: [u8; 60],

    // CQ Head: consumer (kernel gate) read index.
    pub cq_head: AtomicU32,
    pub _pad_cq_head: [u8; 60],

    // CQ Tail: producer (worker) write index.
    pub cq_tail: AtomicU32,
    pub _pad_cq_tail: [u8; 60],
}

impl Default for RingHeader {
    fn default() -> Self {
        Self {
            magic: RING_MAGIC,
            version: RING_VERSION,
            flags: 0,
            total_bytes: RING_TOTAL_SIZE as u32,
            sq_capacity: SQ_CAPACITY,
            cq_capacity: CQ_CAPACITY,
            sq_entry_size: core::mem::size_of::<SqEntry>() as u32,
            cq_entry_size: core::mem::size_of::<CqEntry>() as u32,
            _pad_head: [0; 36],
            sq_head: AtomicU32::new(0),
            _pad_sq_head: [0; 60],
            sq_tail: AtomicU32::new(0),
            _pad_sq_tail: [0; 60],
            cq_head: AtomicU32::new(0),
            _pad_cq_head: [0; 60],
            cq_tail: AtomicU32::new(0),
            _pad_cq_tail: [0; 60],
        }
    }
}

/// Self-contained 128 KiB lockless shared ring structure.
///
/// Contains:
/// 1. `RingHeader` with cache-separated atomic indices.
/// 2. SQ entries array: 256 * 256 bytes = 65,536 bytes.
/// 3. CQ entries array: 512 * 64 bytes = 32,768 bytes.
/// Total allocated structure is cache-aligned and fits cleanly within 128 KB.
#[repr(C, align(4096))]
pub struct SharedRingBuffer {
    pub header: RingHeader,
    pub sq_entries: [SqEntry; SQ_CAPACITY as usize],
    pub cq_entries: [CqEntry; CQ_CAPACITY as usize],
    /// Extra buffer space reserved up to full 128 KB.
    pub _reserved_tail: [u8; RING_TOTAL_SIZE
        - core::mem::size_of::<RingHeader>()
        - (SQ_CAPACITY as usize * core::mem::size_of::<SqEntry>())
        - (CQ_CAPACITY as usize * core::mem::size_of::<CqEntry>())],
}

// Compile-time check that SharedRingBuffer matches exactly 128 KB.
const _: () = assert!(core::mem::size_of::<SharedRingBuffer>() == RING_TOTAL_SIZE);
const _: () = assert!(core::mem::size_of::<SqEntry>() == 256);
const _: () = assert!(core::mem::size_of::<CqEntry>() == 64);

impl SharedRingBuffer {
    /// Creates a newly initialized ring buffer instance on the heap or stack.
    pub fn new() -> Box<Self> {
        let mut uninit: Box<Self> = Box::new(unsafe { core::mem::zeroed() });
        uninit.header = RingHeader::default();
        uninit
    }

    /// Submits a request to the SQ ring (Producer = Kernel Gate).
    ///
    /// Uses Atomic `Acquire` to load `sq_head` and `Release` to store `sq_tail`.
    pub fn sq_submit(&mut self, entry: &SqEntry) -> Result<u32, RingError> {
        let tail = self.header.sq_tail.load(Ordering::Acquire);
        let head = self.header.sq_head.load(Ordering::Acquire);

        // Check if queue is full (power of 2 arithmetic)
        if tail.wrapping_sub(head) >= SQ_CAPACITY {
            return Err(RingError::QueueFull);
        }

        let slot = (tail & SQ_MASK) as usize;
        self.sq_entries[slot] = *entry;

        // Release ordering ensures the entry write is visible before tail increments
        self.header.sq_tail.store(tail.wrapping_add(1), Ordering::Release);
        Ok(tail)
    }

    /// Consumes a request from the SQ ring (Consumer = Worker).
    ///
    /// Uses Atomic `Acquire` to load `sq_tail` and `Release` to store `sq_head`.
    pub fn sq_consume(&mut self) -> Option<SqEntry> {
        let head = self.header.sq_head.load(Ordering::Acquire);
        let tail = self.header.sq_tail.load(Ordering::Acquire);

        // Check if queue is empty
        if head == tail {
            return None;
        }

        let slot = (head & SQ_MASK) as usize;
        let entry = self.sq_entries[slot];

        // Release ordering notifies the producer that the slot is now free
        self.header.sq_head.store(head.wrapping_add(1), Ordering::Release);
        Some(entry)
    }

    /// Submits a completion response to the CQ ring (Producer = Worker).
    ///
    /// Uses Atomic `Acquire` to load `cq_head` and `Release` to store `cq_tail`.
    pub fn cq_submit(&mut self, entry: &CqEntry) -> Result<u32, RingError> {
        let tail = self.header.cq_tail.load(Ordering::Acquire);
        let head = self.header.cq_head.load(Ordering::Acquire);

        if tail.wrapping_sub(head) >= CQ_CAPACITY {
            return Err(RingError::QueueFull);
        }

        let slot = (tail & CQ_MASK) as usize;
        self.cq_entries[slot] = *entry;

        self.header.cq_tail.store(tail.wrapping_add(1), Ordering::Release);
        Ok(tail)
    }

    /// Consumes a completion response from the CQ ring (Consumer = Kernel Gate).
    ///
    /// Uses Atomic `Acquire` to load `cq_tail` and `Release` to store `cq_head`.
    pub fn cq_consume(&mut self) -> Option<CqEntry> {
        let head = self.header.cq_head.load(Ordering::Acquire);
        let tail = self.header.cq_tail.load(Ordering::Acquire);

        if head == tail {
            return None;
        }

        let slot = (head & CQ_MASK) as usize;
        let entry = self.cq_entries[slot];

        self.header.cq_head.store(head.wrapping_add(1), Ordering::Release);
        Some(entry)
    }

    /// Returns the number of currently pending SQ submissions.
    #[inline]
    pub fn sq_pending_count(&self) -> u32 {
        let tail = self.header.sq_tail.load(Ordering::Acquire);
        let head = self.header.sq_head.load(Ordering::Acquire);
        tail.wrapping_sub(head)
    }

    /// Returns the number of currently available CQ completions.
    #[inline]
    pub fn cq_available_count(&self) -> u32 {
        let tail = self.header.cq_tail.load(Ordering::Acquire);
        let head = self.header.cq_head.load(Ordering::Acquire);
        tail.wrapping_sub(head)
    }
}

/// SPSC lockless producer handle for kernel submission.
pub struct SqProducer<'a> {
    ring: &'a mut SharedRingBuffer,
}

impl<'a> SqProducer<'a> {
    pub fn new(ring: &'a mut SharedRingBuffer) -> Self {
        Self { ring }
    }

    pub fn submit(&mut self, entry: &SqEntry) -> Result<u32, RingError> {
        self.ring.sq_submit(entry)
    }
}

/// SPSC lockless consumer handle for worker processing.
pub struct SqConsumer<'a> {
    ring: &'a mut SharedRingBuffer,
}

impl<'a> SqConsumer<'a> {
    pub fn new(ring: &'a mut SharedRingBuffer) -> Self {
        Self { ring }
    }

    pub fn pop(&mut self) -> Option<SqEntry> {
        self.ring.sq_consume()
    }
}
