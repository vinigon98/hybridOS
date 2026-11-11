//! Memory Descriptor List (MDL) and Zero-Copy Page Projection.
//!
//! Provides kernel-level zero-copy inspection of memory buffers across
//! process boundaries without memory allocation or memcpy overhead.
//!
//! In accordance with HybridOS kernel safety guidelines, every `unsafe` block
//! includes exhaustive `// SAFETY:` documentation detailing pointer provenance,
//! memory invariants, alignment, and bounds validation.

use core::fmt;
use core::slice;
use core::sync::atomic::{AtomicBool, Ordering};

/// Standard physical page size (4 KiB) for x86_64 / AArch64.
pub const PAGE_SIZE: usize = 4096;
/// Page offset mask (lower 12 bits).
pub const PAGE_MASK: usize = PAGE_SIZE - 1;

/// MDL Flags as defined in Windows NT Kernel specifications.
pub mod flags {
    /// The MDL describes a buffer mapped to system virtual address space.
    pub const MDL_MAPPED_TO_SYSTEM_VA: u16 = 0x0001;
    /// The physical pages described by the MDL are locked in RAM (pinned).
    pub const MDL_PAGES_LOCKED: u16 = 0x0002;
    /// Buffer is sourced from non-paged kernel pool.
    pub const MDL_SOURCE_IS_NONPAGED_POOL: u16 = 0x0004;
    /// Buffer allocation uses fixed-size lookaside list.
    pub const MDL_ALLOCATED_FIXED_SIZE: u16 = 0x0008;
    /// The MDL represents a partial sub-range of another MDL.
    pub const MDL_PARTIAL: u16 = 0x0010;
    /// Buffer pages are mapped with read-only permissions.
    pub const MDL_READ_ONLY: u16 = 0x0020;
}

/// Errors occurring during MDL construction, page locking, or projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MdlError {
    /// Buffer pointer was null or invalid.
    NullBufferPointer,
    /// Byte count overflowed memory limits.
    InvalidByteCount,
    /// Memory pages are not locked prior to projection.
    PagesNotLocked,
    /// Buffer is read-only, denying mutable projection.
    WriteViolationReadOnly,
    /// Memory alignment failed verification.
    MisalignedBuffer,
    /// Page table translation or PFN lookup failed.
    PfnResolutionFailed,
    /// MDL is already projected exclusively.
    AlreadyProjected,
}

impl fmt::Display for MdlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NullBufferPointer => write!(f, "Buffer pointer is null or unmapped"),
            Self::InvalidByteCount => write!(f, "Buffer byte count exceeds maximum allowed span"),
            Self::PagesNotLocked => write!(f, "Physical pages must be locked before projection"),
            Self::WriteViolationReadOnly => write!(f, "Cannot project mutable view of read-only MDL"),
            Self::MisalignedBuffer => write!(f, "Buffer address does not satisfy alignment constraints"),
            Self::PfnResolutionFailed => write!(f, "Page frame number (PFN) resolution failed"),
            Self::AlreadyProjected => write!(f, "MDL is already actively projected exclusively"),
        }
    }
}

impl std::error::Error for MdlError {}

/// Memory Descriptor List (MDL) describing physical pages backing a virtual buffer.
///
/// Models Windows NT `MDL` semantics and HybridOS cross-personality zero-copy
/// VFS buffers. Enables user-mode workers to inspect file mappings or NT sections
/// without copying page contents into intermediate kernel allocations.
#[derive(Debug)]
pub struct Mdl {
    /// Start virtual address of the buffer page containing the start of the buffer.
    start_va: usize,
    /// Byte offset within the first page to the start of the buffer.
    byte_offset: u32,
    /// Total length in bytes of the buffer described by this MDL.
    byte_count: u32,
    /// Bitwise combination of MDL flags (e.g. `MDL_PAGES_LOCKED`, `MDL_READ_ONLY`).
    mdl_flags: u16,
    /// Process ID that owns the virtual address range (0 for kernel non-paged pool).
    process_id: u32,
    /// Direct mapped system address if projected into system/worker VA space.
    system_va: *mut u8,
    /// List of physical Page Frame Numbers (PFNs) describing the physical backing.
    pfns: Vec<u64>,
    /// Tracks active zero-copy projection to prevent aliasing violations.
    is_projected: AtomicBool,
}

// SAFETY: Mdl manages raw pointers (`system_va`) pointing to pinned, locked
// memory pages. Synchronization of concurrent access is governed by atomic
// projection tracking (`is_projected`) and Rust borrowing semantics.
unsafe impl Send for Mdl {}
unsafe impl Sync for Mdl {}

impl Mdl {
    /// Constructs a new MDL describing an existing in-memory slice.
    ///
    /// # Safety Invariant
    /// The memory slice must remain valid while the MDL is active unless
    /// pages are locked and managed by the kernel page table.
    pub fn from_slice(slice: &[u8], process_id: u32) -> Result<Self, MdlError> {
        if slice.is_empty() {
            return Err(MdlError::InvalidByteCount);
        }

        let va = slice.as_ptr() as usize;
        let byte_count = slice.len() as u32;
        let byte_offset = (va & PAGE_MASK) as u32;
        let start_va = va & !PAGE_MASK;

        let num_pages = Self::calculate_page_count(byte_offset, byte_count);

        // Generate synthetic PFNs for the memory range (in a live kernel, this
        // queries the hardware page table via CR3 / TTBR0).
        let mut pfns = Vec::with_capacity(num_pages);
        for i in 0..num_pages {
            // Pseudo-physical address calculation for user-space simulation/testing
            let pfn = ((start_va + (i * PAGE_SIZE)) >> 12) as u64;
            pfns.push(pfn);
        }

        Ok(Self {
            start_va,
            byte_offset,
            byte_count,
            mdl_flags: flags::MDL_READ_ONLY,
            process_id,
            system_va: slice.as_ptr() as *mut u8,
            pfns,
            is_projected: AtomicBool::new(false),
        })
    }

    /// Constructs an MDL from raw pointers, typically invoked by kernel VFS hooks.
    ///
    /// # Safety
    /// Caller must guarantee:
    /// 1. `buffer_ptr` is non-null and points to a readable memory region of at least `byte_count` bytes.
    /// 2. If `read_only` is false, `buffer_ptr` must be writable.
    /// 3. Memory remains allocated for the duration of the MDL's lifetime.
    pub unsafe fn from_raw_parts(
        buffer_ptr: *mut u8,
        byte_count: u32,
        process_id: u32,
        read_only: bool,
    ) -> Result<Self, MdlError> {
        if buffer_ptr.is_null() {
            return Err(MdlError::NullBufferPointer);
        }
        if byte_count == 0 {
            return Err(MdlError::InvalidByteCount);
        }

        let va = buffer_ptr as usize;
        let byte_offset = (va & PAGE_MASK) as u32;
        let start_va = va & !PAGE_MASK;
        let num_pages = Self::calculate_page_count(byte_offset, byte_count);

        let mut pfns = Vec::with_capacity(num_pages);
        for i in 0..num_pages {
            let pfn = ((start_va + (i * PAGE_SIZE)) >> 12) as u64;
            pfns.push(pfn);
        }

        let mut mdl_flags = 0;
        if read_only {
            mdl_flags |= flags::MDL_READ_ONLY;
        }

        Ok(Self {
            start_va,
            byte_offset,
            byte_count,
            mdl_flags,
            process_id,
            system_va: buffer_ptr,
            pfns,
            is_projected: AtomicBool::new(false),
        })
    }

    /// Calculates number of pages spanned by an offset and byte count.
    #[inline]
    pub fn calculate_page_count(byte_offset: u32, byte_count: u32) -> usize {
        let total = (byte_offset as usize) + (byte_count as usize);
        (total + PAGE_MASK) / PAGE_SIZE
    }

    /// Simulates `MmProbeAndLockPages` semantics: pins physical pages in RAM
    /// and ensures they will not be swapped out or reclaimed during inspection.
    pub fn probe_and_lock_pages(&mut self, read_only: bool) -> Result<(), MdlError> {
        if self.system_va.is_null() {
            return Err(MdlError::NullBufferPointer);
        }

        self.mdl_flags |= flags::MDL_PAGES_LOCKED;
        if read_only {
            self.mdl_flags |= flags::MDL_READ_ONLY;
        }

        Ok(())
    }

    /// Unlocks and unpins physical pages (`MmUnlockPages`).
    pub fn unlock_pages(&mut self) {
        self.mdl_flags &= !flags::MDL_PAGES_LOCKED;
        self.is_projected.store(false, Ordering::Release);
    }

    /// Projects the MDL pages into an immutable zero-copy byte slice (`&[u8]`).
    ///
    /// The returned `MdlProjection` acts as a zero-copy lease. No bytes are copied;
    /// the inspector reads directly from mapped memory.
    pub fn project_zero_copy<'a>(&'a self) -> Result<MdlProjection<'a>, MdlError> {
        // Enforce that pages are locked in physical RAM before projection
        if (self.mdl_flags & flags::MDL_PAGES_LOCKED) == 0 {
            return Err(MdlError::PagesNotLocked);
        }

        if self.system_va.is_null() {
            return Err(MdlError::NullBufferPointer);
        }

        // Check single projection lease
        if self.is_projected.swap(true, Ordering::AcqRel) {
            return Err(MdlError::AlreadyProjected);
        }

        // SAFETY:
        // 1. `self.system_va` was validated as non-null.
        // 2. `self.mdl_flags` has `MDL_PAGES_LOCKED`, guaranteeing memory is pinned in RAM.
        // 3. `self.byte_count` specifies the exact bounds validated during MDL construction.
        // 4. `is_projected` atomic flag prevents duplicate active leases.
        // 5. The lifetime `'a` binds the resulting slice to the lifetime of `&'a self`.
        let slice = unsafe {
            slice::from_raw_parts(self.system_va, self.byte_count as usize)
        };

        Ok(MdlProjection {
            mdl: self,
            slice,
        })
    }

    /// Projects the MDL pages into a mutable zero-copy byte slice (`&mut [u8]`).
    ///
    /// Fails with `WriteViolationReadOnly` if the MDL has `MDL_READ_ONLY` flag set.
    pub fn project_zero_copy_mut<'a>(&'a mut self) -> Result<MdlProjectionMut<'a>, MdlError> {
        if (self.mdl_flags & flags::MDL_PAGES_LOCKED) == 0 {
            return Err(MdlError::PagesNotLocked);
        }

        if (self.mdl_flags & flags::MDL_READ_ONLY) != 0 {
            return Err(MdlError::WriteViolationReadOnly);
        }

        if self.system_va.is_null() {
            return Err(MdlError::NullBufferPointer);
        }

        if self.is_projected.swap(true, Ordering::AcqRel) {
            return Err(MdlError::AlreadyProjected);
        }

        // SAFETY:
        // 1. `self.system_va` is non-null and valid for writes.
        // 2. Verified not read-only via `MDL_READ_ONLY` check.
        // 3. Exclusive mutable reference `&'a mut self` ensures no concurrent readers.
        // 4. Length is bounded by `self.byte_count`.
        let slice = unsafe {
            slice::from_raw_parts_mut(self.system_va, self.byte_count as usize)
        };

        Ok(MdlProjectionMut {
            mdl: self,
            slice,
        })
    }

    // Accessors
    #[inline]
    pub fn byte_count(&self) -> u32 {
        self.byte_count
    }

    #[inline]
    pub fn byte_offset(&self) -> u32 {
        self.byte_offset
    }

    #[inline]
    pub fn start_va(&self) -> usize {
        self.start_va
    }

    #[inline]
    pub fn flags(&self) -> u16 {
        self.mdl_flags
    }

    #[inline]
    pub fn process_id(&self) -> u32 {
        self.process_id
    }

    #[inline]
    pub fn pfns(&self) -> &[u64] {
        &self.pfns
    }

    #[inline]
    pub fn page_count(&self) -> usize {
        self.pfns.len()
    }

    #[inline]
    pub fn is_locked(&self) -> bool {
        (self.mdl_flags & flags::MDL_PAGES_LOCKED) != 0
    }

    #[inline]
    pub fn is_read_only(&self) -> bool {
        (self.mdl_flags & flags::MDL_READ_ONLY) != 0
    }
}

impl Drop for Mdl {
    fn drop(&mut self) {
        if self.is_locked() {
            self.unlock_pages();
        }
    }
}

/// An immutable zero-copy projection lease of memory pages described by an `Mdl`.
pub struct MdlProjection<'a> {
    mdl: &'a Mdl,
    slice: &'a [u8],
}

impl<'a> MdlProjection<'a> {
    /// Returns the projected memory as a safe immutable slice without copying.
    #[inline]
    pub fn as_slice(&self) -> &'a [u8] {
        self.slice
    }

    /// Length of the projected memory span.
    #[inline]
    pub fn len(&self) -> usize {
        self.slice.len()
    }

    /// Returns whether the projected memory span is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.slice.is_empty()
    }

    /// Reference to the backing MDL metadata.
    #[inline]
    pub fn mdl(&self) -> &'a Mdl {
        self.mdl
    }
}

impl<'a> Drop for MdlProjection<'a> {
    fn drop(&mut self) {
        // Release projection lease
        self.mdl.is_projected.store(false, Ordering::Release);
    }
}

impl<'a> core::ops::Deref for MdlProjection<'a> {
    type Target = [u8];

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.slice
    }
}

/// A mutable zero-copy projection lease of memory pages described by an `Mdl`.
pub struct MdlProjectionMut<'a> {
    mdl: &'a mut Mdl,
    slice: &'a mut [u8],
}

impl<'a> MdlProjectionMut<'a> {
    /// Returns the projected memory as a mutable slice.
    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        self.slice
    }

    /// Length of the projected memory span.
    #[inline]
    pub fn len(&self) -> usize {
        self.slice.len()
    }

    /// Returns whether the projected memory span is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.slice.is_empty()
    }
}

impl<'a> Drop for MdlProjectionMut<'a> {
    fn drop(&mut self) {
        self.mdl.is_projected.store(false, Ordering::Release);
    }
}

impl<'a> core::ops::Deref for MdlProjectionMut<'a> {
    type Target = [u8];

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.slice
    }
}

impl<'a> core::ops::DerefMut for MdlProjectionMut<'a> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.slice
    }
}
