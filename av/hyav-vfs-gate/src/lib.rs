//! # HyAV VFS Gate (`hyav-vfs-gate`)
//!
//! Kernel-level interception, lockless 128 KB shared-memory rings,
//! zero-copy MDL (Memory Descriptor List) page projection, and NT/POSIX mediation.
//!
//! ## Core Features
//! - **128 KB Lockless SQ/CQ IPC Rings**: High-performance single/multi-core IPC using
//!   `AtomicU32` with `Acquire`/`Release` memory ordering and cache-line separation.
//! - **Zero-Copy MDL Projection**: Direct inspection of physical page ranges without memcpy.
//! - **Windows NT Semantics**: Full support for `STATUS_PENDING` (0x00000103) async returns,
//!   degradation timeouts, and `STATUS_VIRUS_INFECTED` (0xC0000906) on `SEC_IMAGE` blocks.

pub mod gate;
pub mod mdl;
pub mod ring;

// Re-exports
pub use gate::{
    ntstatus, section_flags, DegradationMode, DegradationState, GateConfig, GateEngine, Verdict,
};
pub use mdl::{flags as mdl_flags, Mdl, MdlError, MdlProjection, MdlProjectionMut, PAGE_SIZE};
pub use ring::{
    entry_flags, CqEntry, RingError, RingHeader, SharedRingBuffer, SqConsumer, SqEntry,
    SqProducer, CQ_CAPACITY, RING_TOTAL_SIZE, SQ_CAPACITY,
};
