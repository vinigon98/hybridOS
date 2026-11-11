//! VFS Gate Interception Engine, NTSTATUS Semantics, and Degradation Policy.
//!
//! Enforces synchronous and asynchronous security mediation for both Windows NT
//! and Linux POSIX personalities.
//!
//! Key requirements:
//! - Returns `STATUS_PENDING` (0x00000103) during asynchronous or staged inspection.
//! - Enforces degradation timeouts (Graceful degradation / Fail-Open vs Fail-Closed).
//! - Returns `STATUS_VIRUS_INFECTED` (0xC0000906) when malware is blocked during
//!   `SEC_IMAGE` (0x01000000) section creation.

use crate::mdl::Mdl;
use crate::ring::{entry_flags, RingError, SharedRingBuffer, SqEntry};
use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// NTSTATUS return codes as defined in the Windows NT Kernel specification.
pub mod ntstatus {
    /// Operation completed successfully.
    pub const STATUS_SUCCESS: u32 = 0x00000000;
    /// The operation is pending completion in an asynchronous worker.
    pub const STATUS_PENDING: u32 = 0x00000103;
    /// The request was canceled or timed out.
    pub const STATUS_TIMEOUT: u32 = 0x00000102;
    /// General access denied.
    pub const STATUS_ACCESS_DENIED: u32 = 0xC0000022;
    /// The file is infected with a virus and cannot be opened or mapped.
    /// Specifically returned for SEC_IMAGE execution gates.
    pub const STATUS_VIRUS_INFECTED: u32 = 0xC0000906;
    /// A required security validation failed.
    pub const STATUS_TRUST_FAILURE: u32 = 0xC00006B5;
}

/// Windows NT Section Creation Flags (`NtCreateSection`).
pub mod section_flags {
    /// Executable image mapping (`SEC_IMAGE`).
    /// Instructs the memory manager to map a PE image file with execute permissions.
    pub const SEC_IMAGE: u32 = 0x01000000;
    /// Normal committed memory section.
    pub const SEC_COMMIT: u32 = 0x08000000;
    /// Reserved memory space.
    pub const SEC_RESERVE: u32 = 0x04000000;
}

/// Inspection Verdict committed by workers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Verdict {
    /// Operation is safe; access allowed.
    Allow = 0,
    /// Threat detected; access denied.
    Block = 1,
    /// High-severity threat; file quarantined and tombstoned.
    Quarantine = 2,
    /// Degradation timeout occurred; fail-open allow applied.
    DegradedAllow = 3,
}

impl From<u8> for Verdict {
    fn from(val: u8) -> Self {
        match val {
            0 => Self::Allow,
            1 => Self::Block,
            2 => Self::Quarantine,
            3 => Self::DegradedAllow,
            _ => Self::Block,
        }
    }
}

/// System degradation mode in response to worker overload or timeouts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DegradationMode {
    /// High-availability profile: allow requests upon worker timeout to prevent system hang.
    FailOpen,
    /// High-security profile: deny access upon worker timeout.
    FailClosed,
}

/// System degradation state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DegradationState {
    /// Operating under nominal latency bounds.
    Normal,
    /// Intermittent worker timeouts observed.
    Degraded,
    /// Severe latency or worker disconnect; emergency mode.
    Critical,
}

/// Configuration options for the VFS Gate engine.
#[derive(Debug, Clone)]
pub struct GateConfig {
    /// Timeout in milliseconds before degrading request.
    pub timeout_ms: u64,
    /// Maximum consecutive timeouts before triggering `DegradationState::Critical`.
    pub max_consecutive_timeouts: u32,
    /// System degradation mode (FailOpen vs FailClosed).
    pub degradation_mode: DegradationMode,
}

impl Default for GateConfig {
    fn default() -> Self {
        Self {
            timeout_ms: 500,
            max_consecutive_timeouts: 3,
            degradation_mode: DegradationMode::FailClosed,
        }
    }
}

/// The VFS Gate mediation engine.
pub struct GateEngine {
    ring: Box<SharedRingBuffer>,
    config: GateConfig,
    next_request_id: AtomicU64,
    consecutive_timeouts: AtomicU32,
    total_intercepted: AtomicU64,
    total_blocked: AtomicU64,
    total_degraded: AtomicU64,
}

impl GateEngine {
    /// Initializes a new Gate Engine backed by a 128 KiB lockless ring buffer.
    pub fn new(config: GateConfig) -> Self {
        Self {
            ring: SharedRingBuffer::new(),
            config,
            next_request_id: AtomicU64::new(1),
            consecutive_timeouts: AtomicU32::new(0),
            total_intercepted: AtomicU64::new(0),
            total_blocked: AtomicU64::new(0),
            total_degraded: AtomicU64::new(0),
        }
    }

    /// Access the underlying 128 KB shared ring buffer.
    pub fn ring_mut(&mut self) -> &mut SharedRingBuffer {
        &mut self.ring
    }

    /// Returns the current degradation state based on recent timeouts.
    pub fn degradation_state(&self) -> DegradationState {
        let timeouts = self.consecutive_timeouts.load(Ordering::Relaxed);
        if timeouts >= self.config.max_consecutive_timeouts {
            DegradationState::Critical
        } else if timeouts > 0 {
            DegradationState::Degraded
        } else {
            DegradationState::Normal
        }
    }

    /// Intercepts `NtCreateSection` (`SEC_IMAGE` verification).
    ///
    /// When `section_flags & SEC_IMAGE != 0`, the target file is being mapped for execution.
    /// If malware is detected by the inspection engine, this function returns:
    /// `STATUS_VIRUS_INFECTED` (`0xC0000906`).
    pub fn intercept_create_section(
        &mut self,
        path: &str,
        section_attributes: u32,
        mdl: Option<&Mdl>,
        pid: u32,
        tid: u32,
    ) -> u32 {
        self.total_intercepted.fetch_add(1, Ordering::Relaxed);

        let is_sec_image = (section_attributes & section_flags::SEC_IMAGE) != 0;

        let request_id = self.next_request_id.fetch_add(1, Ordering::Relaxed);

        // Populate SQ entry
        let mut sqe = SqEntry::default();
        sqe.request_id = request_id;
        sqe.timestamp_ns = 0;
        sqe.pid = pid;
        sqe.tid = tid;
        sqe.subsystem = 0; // Windows NT
        sqe.hook_type = 2; // PreSection

        if is_sec_image {
            sqe.flags |= entry_flags::SEC_IMAGE;
            sqe.scan_reason = 0x0001; // EXEC_PE
        }

        // Attach zero-copy MDL parameters if provided
        if let Some(m) = mdl {
            sqe.flags |= entry_flags::HAS_MDL;
            sqe.mdl_va = m.start_va() as u64;
            sqe.mdl_byte_count = m.byte_count() as u64;
            sqe.mdl_page_count = m.page_count() as u32;
        }

        let path_bytes = path.as_bytes();
        let copy_len = path_bytes.len().min(sqe.path_bytes.len());
        sqe.path_bytes[..copy_len].copy_from_slice(&path_bytes[..copy_len]);
        sqe.path_len = copy_len as u16;
        sqe.timeout_ms = self.config.timeout_ms as u16;

        // Submit to lockless SQ ring
        if let Err(RingError::QueueFull) = self.ring.sq_submit(&sqe) {
            // If ring is full, handle under degradation policy
            return self.handle_timeout_degradation(is_sec_image);
        }

        // For synchronous execution gates (like SEC_IMAGE), we await the worker CQ response
        // subject to the degradation timeout.
        let start = Instant::now();
        let timeout = Duration::from_millis(self.config.timeout_ms);

        loop {
            // Check completion queue
            if let Some(cqe) = self.ring.cq_consume() {
                if cqe.request_id == request_id {
                    self.consecutive_timeouts.store(0, Ordering::Relaxed);
                    let verdict = Verdict::from(cqe.verdict);
                    return self.resolve_verdict_to_status(verdict, is_sec_image);
                }
            }

            // Check degradation timeout
            if start.elapsed() >= timeout {
                return self.handle_timeout_degradation(is_sec_image);
            }

            // Spin/yield in simulated wait state
            core::hint::spin_loop();
        }
    }

    /// Asynchronous submission returning `STATUS_PENDING` (0x00000103).
    ///
    /// Allows the calling thread/IRP to return immediately to the kernel dispatcher
    /// while the request is evaluated asynchronously by the worker pool.
    pub fn submit_async(
        &mut self,
        path: &str,
        is_sec_image: bool,
        mdl: Option<&Mdl>,
        pid: u32,
        tid: u32,
    ) -> Result<(u64, u32), u32> {
        self.total_intercepted.fetch_add(1, Ordering::Relaxed);

        let request_id = self.next_request_id.fetch_add(1, Ordering::Relaxed);

        let mut sqe = SqEntry::default();
        sqe.request_id = request_id;
        sqe.pid = pid;
        sqe.tid = tid;
        sqe.subsystem = 0;
        sqe.hook_type = if is_sec_image { 2 } else { 1 };

        if is_sec_image {
            sqe.flags |= entry_flags::SEC_IMAGE;
            sqe.scan_reason = 0x0001; // EXEC_PE
        }

        if let Some(m) = mdl {
            sqe.flags |= entry_flags::HAS_MDL;
            sqe.mdl_va = m.start_va() as u64;
            sqe.mdl_byte_count = m.byte_count() as u64;
            sqe.mdl_page_count = m.page_count() as u32;
        }

        let path_bytes = path.as_bytes();
        let copy_len = path_bytes.len().min(sqe.path_bytes.len());
        sqe.path_bytes[..copy_len].copy_from_slice(&path_bytes[..copy_len]);
        sqe.path_len = copy_len as u16;
        sqe.timeout_ms = self.config.timeout_ms as u16;

        self.ring.sq_submit(&sqe).map_err(|_| ntstatus::STATUS_TIMEOUT)?;

        // Return (request_id, STATUS_PENDING)
        Ok((request_id, ntstatus::STATUS_PENDING))
    }

    /// Polls asynchronous completion from the CQ ring.
    pub fn poll_async_completion(&mut self, request_id: u64, is_sec_image: bool) -> Option<u32> {
        if let Some(cqe) = self.ring.cq_consume() {
            if cqe.request_id == request_id {
                let verdict = Verdict::from(cqe.verdict);
                return Some(self.resolve_verdict_to_status(verdict, is_sec_image));
            }
        }
        None
    }

    /// Resolves an inspection verdict into an NTSTATUS code.
    ///
    /// CRITICAL RULE:
    /// If `is_sec_image` is true and verdict is Block or Quarantine,
    /// returns `STATUS_VIRUS_INFECTED` (0xC0000906).
    pub fn resolve_verdict_to_status(&self, verdict: Verdict, is_sec_image: bool) -> u32 {
        match verdict {
            Verdict::Allow | Verdict::DegradedAllow => ntstatus::STATUS_SUCCESS,
            Verdict::Block | Verdict::Quarantine => {
                self.total_blocked.fetch_add(1, Ordering::Relaxed);
                if is_sec_image {
                    ntstatus::STATUS_VIRUS_INFECTED
                } else {
                    ntstatus::STATUS_ACCESS_DENIED
                }
            }
        }
    }

    /// Handles timeout expiration according to degradation policy.
    fn handle_timeout_degradation(&self, is_sec_image: bool) -> u32 {
        self.consecutive_timeouts.fetch_add(1, Ordering::Relaxed);
        self.total_degraded.fetch_add(1, Ordering::Relaxed);

        match self.config.degradation_mode {
            DegradationMode::FailOpen => {
                // High availability: allow operation
                ntstatus::STATUS_SUCCESS
            }
            DegradationMode::FailClosed => {
                // High security: block operation
                if is_sec_image {
                    ntstatus::STATUS_VIRUS_INFECTED
                } else {
                    ntstatus::STATUS_ACCESS_DENIED
                }
            }
        }
    }

    // Telemetry getters
    pub fn total_intercepted(&self) -> u64 {
        self.total_intercepted.load(Ordering::Relaxed)
    }

    pub fn total_blocked(&self) -> u64 {
        self.total_blocked.load(Ordering::Relaxed)
    }

    pub fn total_degraded(&self) -> u64 {
        self.total_degraded.load(Ordering::Relaxed)
    }
}
