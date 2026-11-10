//! Physical Frame Allocator for HybridOS Kernel.
//!
//! Manages 4KB physical memory frames using a high-performance bitmap allocator.
//! Tracks free and allocated physical frames, supporting single-frame and contiguous
//! physical allocations for kernel pages, page tables, and DMA buffers.

use core::fmt;

/// Standard page frame size for x86_64 architecture (4 Kilobytes).
pub const PAGE_SIZE: usize = 4096;

/// Page offset bit shift (2^12 = 4096 bytes).
pub const PAGE_SHIFT: usize = 12;

/// Page offset mask (lower 12 bits).
pub const PAGE_MASK: u64 = 0xFFF;

/// Maximum physical address supported in standard 4-level x86_64 (52 bits).
pub const MAX_PHYS_ADDR: u64 = (1u64 << 52) - 1;

/// Represents a 64-bit physical memory address.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[repr(transparent)]
pub struct PhysAddr(pub u64);

impl PhysAddr {
    /// Creates a new physical address.
    #[inline]
    pub const fn new(addr: u64) -> Self {
        assert!(addr <= MAX_PHYS_ADDR, "Physical address exceeds 52-bit limit");
        Self(addr)
    }

    /// Creates a new physical address without verifying the 52-bit limit.
    #[inline]
    pub const fn new_truncate(addr: u64) -> Self {
        Self(addr & MAX_PHYS_ADDR)
    }

    /// Returns the raw 64-bit value.
    #[inline]
    pub const fn as_u64(&self) -> u64 {
        self.0
    }

    /// Returns the address as a usize.
    #[inline]
    pub const fn as_usize(&self) -> usize {
        self.0 as usize
    }

    /// Returns true if the address is aligned to the given byte boundary.
    #[inline]
    pub const fn is_aligned_to(&self, align: u64) -> bool {
        assert!(align.is_power_of_two(), "Alignment must be a power of two");
        (self.0 & (align - 1)) == 0
    }

    /// Returns true if the address is 4KB page-aligned.
    #[inline]
    pub const fn is_page_aligned(&self) -> bool {
        self.is_aligned_to(PAGE_SIZE as u64)
    }

    /// Aligns the address up to the nearest multiple of `align`.
    #[inline]
    pub const fn align_up(&self, align: u64) -> Self {
        assert!(align.is_power_of_two(), "Alignment must be a power of two");
        Self((self.0 + align - 1) & !(align - 1))
    }

    /// Aligns the address down to the nearest multiple of `align`.
    #[inline]
    pub const fn align_down(&self, align: u64) -> Self {
        assert!(align.is_power_of_two(), "Alignment must be a power of two");
        Self(self.0 & !(align - 1))
    }

    /// Checks if this address is null (0x0).
    #[inline]
    pub const fn is_null(&self) -> bool {
        self.0 == 0
    }
}

impl fmt::Debug for PhysAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PhysAddr(0x{:016X})", self.0)
    }
}

impl fmt::Display for PhysAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x{:016X}", self.0)
    }
}

impl core::ops::Add<u64> for PhysAddr {
    type Output = Self;
    #[inline]
    fn add(self, rhs: u64) -> Self::Output {
        PhysAddr::new(self.0 + rhs)
    }
}

impl core::ops::Sub<u64> for PhysAddr {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: u64) -> Self::Output {
        PhysAddr::new(self.0 - rhs)
    }
}

/// Represents a 4KB-aligned physical page frame.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PhysFrame {
    start_address: PhysAddr,
}

impl PhysFrame {
    /// Creates a frame from a starting physical address. Must be 4KB aligned.
    #[inline]
    pub fn from_start_address(addr: PhysAddr) -> Result<Self, FrameError> {
        if !addr.is_page_aligned() {
            Err(FrameError::InvalidAlignment(addr.0))
        } else {
            Ok(Self { start_address: addr })
        }
    }

    /// Returns the frame that contains the given physical address.
    #[inline]
    pub const fn containing_address(addr: PhysAddr) -> Self {
        Self {
            start_address: PhysAddr(addr.0 & !PAGE_MASK),
        }
    }

    /// Creates a frame corresponding to a frame index (frame_number * 4096).
    #[inline]
    pub const fn from_number(number: usize) -> Self {
        Self {
            start_address: PhysAddr((number as u64) << PAGE_SHIFT),
        }
    }

    /// Returns the frame number (index).
    #[inline]
    pub const fn frame_number(&self) -> usize {
        (self.start_address.0 >> PAGE_SHIFT) as usize
    }

    /// Returns the start physical address of this frame.
    #[inline]
    pub const fn start_address(&self) -> PhysAddr {
        self.start_address
    }

    /// Returns the size of a physical frame in bytes (4096).
    #[inline]
    pub const fn size(&self) -> usize {
        PAGE_SIZE
    }
}

impl fmt::Debug for PhysFrame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PhysFrame(#{} @ 0x{:016X})", self.frame_number(), self.start_address.0)
    }
}

impl fmt::Display for PhysFrame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Frame(#{}, {})", self.frame_number(), self.start_address)
    }
}

/// Errors returned by the physical frame allocator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameError {
    /// Physical memory is fully exhausted.
    OutOfMemory,
    /// Physical address is not aligned to 4KB page boundary.
    InvalidAlignment(u64),
    /// Frame is out of managed bounds.
    FrameOutOfBounds(usize),
    /// Frame was already marked free (double free attempt).
    FrameAlreadyFree(usize),
    /// Requested contiguous frame range cannot be satisfied.
    ContiguousAllocationFailed(usize),
}

impl fmt::Display for FrameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutOfMemory => write!(f, "Physical frame allocator: out of memory"),
            Self::InvalidAlignment(addr) => write!(f, "Address 0x{:X} is not 4KB aligned", addr),
            Self::FrameOutOfBounds(num) => write!(f, "Frame #{} is outside managed memory range", num),
            Self::FrameAlreadyFree(num) => write!(f, "Frame #{} is already free (double free)", num),
            Self::ContiguousAllocationFailed(cnt) => {
                write!(f, "Failed to allocate {} contiguous physical frames", cnt)
            }
        }
    }
}

impl std::error::Error for FrameError {}

/// Bitmap-based Physical Page Frame Allocator.
///
/// Uses an array of 64-bit words as a bitset where each bit represents a 4KB frame.
/// Bit = 0 indicates free, Bit = 1 indicates allocated.
pub struct BitmapFrameAllocator {
    base_addr: PhysAddr,
    total_frames: usize,
    allocated_frames: usize,
    bitmap: Vec<u64>,
    next_hint: usize,
}

impl BitmapFrameAllocator {
    /// Initializes a new physical frame allocator managing memory from `base_addr` for `size` bytes.
    ///
    /// # Arguments
    /// * `base_addr` - Starting physical address (aligned to 4KB).
    /// * `size` - Total byte size of physical memory region to manage.
    pub fn new(base_addr: PhysAddr, size: usize) -> Self {
        assert!(base_addr.is_page_aligned(), "Base address must be 4KB aligned");
        let total_frames = size / PAGE_SIZE;
        let num_words = (total_frames + 63) / 64;

        Self {
            base_addr,
            total_frames,
            allocated_frames: 0,
            bitmap: vec![0u64; num_words],
            next_hint: 0,
        }
    }

    /// Allocates a single 4KB physical frame.
    ///
    /// Returns `Some(PhysFrame)` if available, or `None` if out of memory.
    pub fn allocate_frame(&mut self) -> Option<PhysFrame> {
        if self.allocated_frames >= self.total_frames {
            return None;
        }

        let num_words = self.bitmap.len();
        let start_word = self.next_hint / 64;

        // Search starting from next_hint for fast amortized allocation
        for i in 0..num_words {
            let word_idx = (start_word + i) % num_words;
            let word = self.bitmap[word_idx];

            if word != u64::MAX {
                // There is at least one free bit in this 64-bit word
                let trailing_ones = (!word).trailing_zeros() as usize;
                let frame_idx = word_idx * 64 + trailing_ones;

                if frame_idx < self.total_frames {
                    // Mark as allocated
                    self.bitmap[word_idx] |= 1u64 << trailing_ones;
                    self.allocated_frames += 1;
                    self.next_hint = (frame_idx + 1) % self.total_frames;

                    let frame_addr = self.base_addr + ((frame_idx as u64) << PAGE_SHIFT);
                    return Some(PhysFrame {
                        start_address: frame_addr,
                    });
                }
            }
        }

        None
    }

    /// Allocates `count` contiguous 4KB physical frames.
    ///
    /// Useful for DMA buffers, device hardware queues, and large page setups.
    pub fn allocate_contiguous(&mut self, count: usize) -> Option<PhysFrame> {
        if count == 0 {
            return None;
        }
        if count == 1 {
            return self.allocate_frame();
        }
        if self.allocated_frames + count > self.total_frames {
            return None;
        }

        let mut run_start = 0;
        let mut current_run = 0;

        for idx in 0..self.total_frames {
            let word_idx = idx / 64;
            let bit_idx = idx % 64;
            let is_used = (self.bitmap[word_idx] & (1u64 << bit_idx)) != 0;

            if !is_used {
                if current_run == 0 {
                    run_start = idx;
                }
                current_run += 1;
                if current_run == count {
                    // Found contiguous run! Mark all as allocated.
                    for f in run_start..(run_start + count) {
                        let w = f / 64;
                        let b = f % 64;
                        self.bitmap[w] |= 1u64 << b;
                    }
                    self.allocated_frames += count;
                    self.next_hint = (run_start + count) % self.total_frames;

                    let frame_addr = self.base_addr + ((run_start as u64) << PAGE_SHIFT);
                    return Some(PhysFrame {
                        start_address: frame_addr,
                    });
                }
            } else {
                current_run = 0;
            }
        }

        None
    }

    /// Frees a previously allocated physical frame.
    pub fn free_frame(&mut self, frame: PhysFrame) -> Result<(), FrameError> {
        let frame_addr = frame.start_address().as_u64();
        let base = self.base_addr.as_u64();

        if frame_addr < base {
            return Err(FrameError::FrameOutOfBounds(frame.frame_number()));
        }

        let offset = (frame_addr - base) as usize;
        let frame_idx = offset / PAGE_SIZE;

        if frame_idx >= self.total_frames {
            return Err(FrameError::FrameOutOfBounds(frame_idx));
        }

        let word_idx = frame_idx / 64;
        let bit_idx = frame_idx % 64;

        if (self.bitmap[word_idx] & (1u64 << bit_idx)) == 0 {
            return Err(FrameError::FrameAlreadyFree(frame_idx));
        }

        // Clear bit
        self.bitmap[word_idx] &= !(1u64 << bit_idx);
        self.allocated_frames -= 1;

        if frame_idx < self.next_hint {
            self.next_hint = frame_idx;
        }

        Ok(())
    }

    /// Frees `count` contiguous physical frames starting at `start_frame`.
    pub fn free_contiguous(&mut self, start_frame: PhysFrame, count: usize) -> Result<(), FrameError> {
        for i in 0..count {
            let frame = PhysFrame {
                start_address: start_frame.start_address() + ((i as u64) << PAGE_SHIFT),
            };
            self.free_frame(frame)?;
        }
        Ok(())
    }

    /// Marks a specific physical frame as allocated (reserved).
    /// Used during kernel bootstrap to reserve BIOS, MMIO, and kernel binary pages.
    pub fn mark_allocated(&mut self, frame: PhysFrame) -> Result<(), FrameError> {
        let frame_addr = frame.start_address().as_u64();
        let base = self.base_addr.as_u64();

        if frame_addr < base {
            return Err(FrameError::FrameOutOfBounds(frame.frame_number()));
        }

        let offset = (frame_addr - base) as usize;
        let frame_idx = offset / PAGE_SIZE;

        if frame_idx >= self.total_frames {
            return Err(FrameError::FrameOutOfBounds(frame_idx));
        }

        let word_idx = frame_idx / 64;
        let bit_idx = frame_idx % 64;

        if (self.bitmap[word_idx] & (1u64 << bit_idx)) == 0 {
            self.bitmap[word_idx] |= 1u64 << bit_idx;
            self.allocated_frames += 1;
        }

        Ok(())
    }

    /// Marks a range of physical memory as allocated (e.g., kernel image or ACPI tables).
    pub fn mark_range_allocated(&mut self, start: PhysAddr, size: usize) -> Result<(), FrameError> {
        let start_frame = PhysFrame::containing_address(start);
        let end_frame = PhysFrame::containing_address(start + (size.saturating_sub(1) as u64));

        let start_idx = start_frame.frame_number();
        let end_idx = end_frame.frame_number();

        for num in start_idx..=end_idx {
            let f = PhysFrame::from_number(num);
            let _ = self.mark_allocated(f);
        }

        Ok(())
    }

    /// Checks if a given physical frame is currently allocated.
    pub fn is_frame_allocated(&self, frame: PhysFrame) -> bool {
        let frame_addr = frame.start_address().as_u64();
        let base = self.base_addr.as_u64();

        if frame_addr < base {
            return false;
        }

        let offset = (frame_addr - base) as usize;
        let frame_idx = offset / PAGE_SIZE;

        if frame_idx >= self.total_frames {
            return false;
        }

        let word_idx = frame_idx / 64;
        let bit_idx = frame_idx % 64;

        (self.bitmap[word_idx] & (1u64 << bit_idx)) != 0
    }

    /// Returns the total number of 4KB frames managed by this allocator.
    #[inline]
    pub const fn total_frames(&self) -> usize {
        self.total_frames
    }

    /// Returns the number of currently allocated frames.
    #[inline]
    pub const fn allocated_frames(&self) -> usize {
        self.allocated_frames
    }

    /// Returns the number of free frames remaining.
    #[inline]
    pub const fn free_frames(&self) -> usize {
        self.total_frames - self.allocated_frames
    }

    /// Returns total physical memory managed in bytes.
    #[inline]
    pub const fn total_memory(&self) -> usize {
        self.total_frames * PAGE_SIZE
    }

    /// Returns allocated physical memory in bytes.
    #[inline]
    pub const fn used_memory(&self) -> usize {
        self.allocated_frames * PAGE_SIZE
    }

    /// Returns free physical memory in bytes.
    #[inline]
    pub const fn free_memory(&self) -> usize {
        self.free_frames() * PAGE_SIZE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_phys_addr_alignment() {
        let addr = PhysAddr::new(0x1000);
        assert!(addr.is_page_aligned());
        assert_eq!(addr.align_up(0x1000), PhysAddr::new(0x1000));
        assert_eq!(PhysAddr::new(0x1005).align_up(0x1000), PhysAddr::new(0x2000));
        assert_eq!(PhysAddr::new(0x1005).align_down(0x1000), PhysAddr::new(0x1000));
    }

    #[test]
    fn test_frame_alloc_and_free() {
        // Manage 1 MB = 256 frames
        let mut allocator = BitmapFrameAllocator::new(PhysAddr::new(0x100000), 256 * PAGE_SIZE);
        assert_eq!(allocator.total_frames(), 256);
        assert_eq!(allocator.free_frames(), 256);

        let f1 = allocator.allocate_frame().expect("Frame 1 allocation failed");
        assert_eq!(f1.start_address(), PhysAddr::new(0x100000));
        assert_eq!(allocator.allocated_frames(), 1);

        let f2 = allocator.allocate_frame().expect("Frame 2 allocation failed");
        assert_eq!(f2.start_address(), PhysAddr::new(0x101000));
        assert_eq!(allocator.allocated_frames(), 2);

        // Free f1
        allocator.free_frame(f1).expect("Failed to free f1");
        assert_eq!(allocator.allocated_frames(), 1);

        // Next allocation should re-use f1
        let f3 = allocator.allocate_frame().expect("Frame 3 allocation failed");
        assert_eq!(f3.start_address(), PhysAddr::new(0x100000));
    }

    #[test]
    fn test_contiguous_allocation() {
        let mut allocator = BitmapFrameAllocator::new(PhysAddr::new(0), 128 * PAGE_SIZE);
        let block = allocator.allocate_contiguous(4).expect("Contiguous alloc failed");
        assert_eq!(block.start_address(), PhysAddr::new(0));
        assert_eq!(allocator.allocated_frames(), 4);

        allocator.free_contiguous(block, 4).expect("Free contiguous failed");
        assert_eq!(allocator.allocated_frames(), 0);
    }

    #[test]
    fn test_reservation() {
        let mut allocator = BitmapFrameAllocator::new(PhysAddr::new(0), 64 * PAGE_SIZE);
        let reserved_frame = PhysFrame::from_start_address(PhysAddr::new(0x2000)).unwrap();
        allocator.mark_allocated(reserved_frame).unwrap();

        assert!(allocator.is_frame_allocated(reserved_frame));

        let f1 = allocator.allocate_frame().unwrap();
        let f2 = allocator.allocate_frame().unwrap();
        let f3 = allocator.allocate_frame().unwrap();

        // 0x0000, 0x1000 should be allocated, then 0x2000 was reserved, so f3 is 0x3000
        assert_eq!(f1.start_address(), PhysAddr::new(0x0000));
        assert_eq!(f2.start_address(), PhysAddr::new(0x1000));
        assert_eq!(f3.start_address(), PhysAddr::new(0x3000));
    }
}
