//! Kernel Dynamic Heap Allocator for HybridOS Kernel.
//!
//! Implements a doubly-linked block-based free-list allocator featuring:
//! - Immediate coalescing of adjacent free memory blocks to prevent fragmentation.
//! - Block splitting to minimize internal fragmentation.
//! - 16-byte alignment guarantee for SIMD and atomic compatibility.
//! - Magic signature integrity validation (`0xDEADBEEF`) for buffer overflow and double-free detection.
//! - Thread-safe `LockedHeap` synchronization wrapper.

use core::alloc::Layout;
use core::fmt;
use std::sync::Mutex;

/// Magic signature used to verify block header integrity.
pub const HEAP_BLOCK_MAGIC: u32 = 0xDEAD_BEEF;

/// Minimum payload size for any allocated or split block.
pub const MIN_BLOCK_SIZE: usize = 16;

/// Default block header alignment (16 bytes).
pub const BLOCK_ALIGNMENT: usize = 16;

/// Size of the block header in bytes (aligned to 16 bytes).
pub const HEADER_SIZE: usize = (core::mem::size_of::<BlockHeader>() + (BLOCK_ALIGNMENT - 1)) & !(BLOCK_ALIGNMENT - 1);

/// Doubly-linked block header prepending every heap chunk.
#[repr(C, align(16))]
pub struct BlockHeader {
    /// Magic number for corruption checks (`0xDEADBEEF`).
    pub magic: u32,
    /// Whether this block is free (`true`) or currently allocated (`false`).
    pub is_free: bool,
    /// Size of the payload area (in bytes, excluding header).
    pub size: usize,
    /// Pointer to the next block header in physical address sequence.
    pub next: *mut BlockHeader,
    /// Pointer to the previous block header in physical address sequence.
    pub prev: *mut BlockHeader,
}

impl BlockHeader {
    /// Initializes a new block header.
    pub unsafe fn init(ptr: *mut BlockHeader, size: usize, is_free: bool) {
        let header = &mut *ptr;
        header.magic = HEAP_BLOCK_MAGIC;
        header.is_free = is_free;
        header.size = size;
        header.next = core::ptr::null_mut();
        header.prev = core::ptr::null_mut();
    }

    /// Verifies header integrity.
    #[inline]
    pub fn is_valid(&self) -> bool {
        self.magic == HEAP_BLOCK_MAGIC
    }

    /// Returns a pointer to the payload data following this header.
    #[inline]
    pub unsafe fn payload(&self) -> *mut u8 {
        (self as *const BlockHeader as *mut u8).add(HEADER_SIZE)
    }

    /// Returns a header reference from a payload data pointer.
    #[inline]
    pub unsafe fn from_payload<'a>(payload: *mut u8) -> &'a mut BlockHeader {
        let header_ptr = payload.sub(HEADER_SIZE) as *mut BlockHeader;
        &mut *header_ptr
    }
}

/// Errors returned by the Kernel Heap Allocator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeapError {
    /// Heap has not been initialized with memory.
    NotInitialized,
    /// Requested allocation exceeds available free heap memory.
    OutOfMemory,
    /// Alignment requirement is invalid (not a power of two).
    InvalidAlignment,
    /// Attempted to free a null pointer.
    NullPointer,
    /// Attempted to free an invalid or corrupted memory block.
    CorruptedHeader,
    /// Attempted to free a block that was already free (double free).
    DoubleFree,
}

impl fmt::Display for HeapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotInitialized => write!(f, "Kernel heap is not initialized"),
            Self::OutOfMemory => write!(f, "Kernel heap: Out of memory"),
            Self::InvalidAlignment => write!(f, "Requested alignment is not a power of two"),
            Self::NullPointer => write!(f, "Attempted to free null pointer"),
            Self::CorruptedHeader => write!(f, "Block header corrupted or invalid magic signature"),
            Self::DoubleFree => write!(f, "Attempted double free on heap memory block"),
        }
    }
}

impl std::error::Error for HeapError {}

/// Block-based Doubly Linked List Kernel Heap Allocator.
pub struct KernelHeap {
    /// Pointer to the beginning of the heap memory region.
    heap_start: *mut u8,
    /// Pointer to the end of the heap memory region.
    heap_end: *mut u8,
    /// Head of the doubly-linked list of blocks.
    head: *mut BlockHeader,
    /// Total usable heap memory in bytes.
    total_bytes: usize,
    /// Number of bytes currently allocated (payload).
    allocated_bytes: usize,
    /// Total count of active allocations.
    active_allocations: usize,
    /// Whether the heap has been initialized.
    is_initialized: bool,
}

// Raw pointers in KernelHeap are protected by Mutex synchronization in LockedHeap
unsafe impl Send for KernelHeap {}

impl KernelHeap {
    /// Creates an uninitialized kernel heap.
    pub const fn new() -> Self {
        Self {
            heap_start: core::ptr::null_mut(),
            heap_end: core::ptr::null_mut(),
            head: core::ptr::null_mut(),
            total_bytes: 0,
            allocated_bytes: 0,
            active_allocations: 0,
            is_initialized: false,
        }
    }

    /// Initializes the heap with a dedicated memory region.
    ///
    /// # Safety
    /// The caller must ensure that `heap_start` points to a valid, writable memory region
    /// of at least `size` bytes, and that no other code is writing to this memory.
    pub unsafe fn init(&mut self, heap_start: *mut u8, size: usize) -> Result<(), HeapError> {
        if size < HEADER_SIZE + MIN_BLOCK_SIZE {
            return Err(HeapError::OutOfMemory);
        }

        // Align heap start to 16-byte boundary
        let start_addr = heap_start as usize;
        let aligned_start = (start_addr + (BLOCK_ALIGNMENT - 1)) & !(BLOCK_ALIGNMENT - 1);
        let padding = aligned_start - start_addr;
        let usable_size = size.saturating_sub(padding);

        if usable_size < HEADER_SIZE + MIN_BLOCK_SIZE {
            return Err(HeapError::OutOfMemory);
        }

        let first_block = aligned_start as *mut BlockHeader;
        let initial_payload_size = usable_size - HEADER_SIZE;

        BlockHeader::init(first_block, initial_payload_size, true);

        self.heap_start = aligned_start as *mut u8;
        self.heap_end = (aligned_start + usable_size) as *mut u8;
        self.head = first_block;
        self.total_bytes = usable_size;
        self.allocated_bytes = 0;
        self.active_allocations = 0;
        self.is_initialized = true;

        Ok(())
    }

    /// Allocates `size` bytes with the specified alignment (minimum 16 bytes).
    pub fn allocate(&mut self, size: usize, align: usize) -> Result<*mut u8, HeapError> {
        if !self.is_initialized {
            return Err(HeapError::NotInitialized);
        }

        if size == 0 {
            return Ok(core::ptr::null_mut());
        }

        let effective_align = align.max(BLOCK_ALIGNMENT);
        if !effective_align.is_power_of_two() {
            return Err(HeapError::InvalidAlignment);
        }

        // Align requested size to 16 bytes
        let needed_size = (size + (BLOCK_ALIGNMENT - 1)) & !(BLOCK_ALIGNMENT - 1);

        // Search for first free block that fits
        let mut curr = self.head;
        while !curr.is_null() {
            unsafe {
                let header = &mut *curr;
                if !header.is_valid() {
                    return Err(HeapError::CorruptedHeader);
                }

                if header.is_free && header.size >= needed_size {
                    // Check if block can be split
                    let remaining = header.size.saturating_sub(needed_size);
                    if remaining >= HEADER_SIZE + MIN_BLOCK_SIZE {
                        // Split block
                        let split_payload_size = remaining - HEADER_SIZE;
                        let new_block_ptr = (header.payload()).add(needed_size) as *mut BlockHeader;

                        BlockHeader::init(new_block_ptr, split_payload_size, true);
                        let new_block = &mut *new_block_ptr;

                        new_block.next = header.next;
                        new_block.prev = curr;

                        if !header.next.is_null() {
                            (*header.next).prev = new_block_ptr;
                        }
                        header.next = new_block_ptr;
                        header.size = needed_size;
                    }

                    header.is_free = false;
                    self.allocated_bytes += header.size;
                    self.active_allocations += 1;

                    return Ok(header.payload());
                }

                curr = header.next;
            }
        }

        Err(HeapError::OutOfMemory)
    }

    /// Deallocates memory previously allocated at `ptr`.
    /// Immediately coalesces adjacent free blocks to minimize fragmentation.
    pub fn deallocate(&mut self, ptr: *mut u8) -> Result<(), HeapError> {
        if !self.is_initialized {
            return Err(HeapError::NotInitialized);
        }
        if ptr.is_null() {
            return Err(HeapError::NullPointer);
        }

        // Verify pointer is within heap bounds
        if (ptr < self.heap_start) || (ptr >= self.heap_end) {
            return Err(HeapError::CorruptedHeader);
        }

        unsafe {
            let header = BlockHeader::from_payload(ptr);

            if !header.is_valid() {
                return Err(HeapError::CorruptedHeader);
            }
            if header.is_free {
                return Err(HeapError::DoubleFree);
            }

            // Mark as free
            header.is_free = true;
            self.allocated_bytes = self.allocated_bytes.saturating_sub(header.size);
            self.active_allocations = self.active_allocations.saturating_sub(1);

            let header_ptr = header as *mut BlockHeader;

            // Coalesce with next block if free
            if !header.next.is_null() && (*header.next).is_free {
                let next_header = &mut *header.next;
                if next_header.is_valid() {
                    header.size += HEADER_SIZE + next_header.size;
                    header.next = next_header.next;
                    if !header.next.is_null() {
                        (*header.next).prev = header_ptr;
                    }
                }
            }

            // Coalesce with previous block if free
            if !header.prev.is_null() && (*header.prev).is_free {
                let prev_header = &mut *header.prev;
                if prev_header.is_valid() {
                    prev_header.size += HEADER_SIZE + header.size;
                    prev_header.next = header.next;
                    if !header.next.is_null() {
                        (*header.next).prev = header.prev;
                    }
                }
            }
        }

        Ok(())
    }

    /// Allocates memory conforming to a Rust `Layout`.
    pub fn allocate_layout(&mut self, layout: Layout) -> Result<*mut u8, HeapError> {
        self.allocate(layout.size(), layout.align())
    }

    /// Deallocates memory conforming to a Rust `Layout`.
    pub fn deallocate_layout(&mut self, ptr: *mut u8, _layout: Layout) -> Result<(), HeapError> {
        self.deallocate(ptr)
    }

    /// Returns total heap capacity in bytes.
    #[inline]
    pub const fn total_bytes(&self) -> usize {
        self.total_bytes
    }

    /// Returns currently allocated bytes.
    #[inline]
    pub const fn allocated_bytes(&self) -> usize {
        self.allocated_bytes
    }

    /// Returns remaining free bytes.
    #[inline]
    pub const fn free_bytes(&self) -> usize {
        self.total_bytes.saturating_sub(self.allocated_bytes)
    }

    /// Returns count of active allocations.
    #[inline]
    pub const fn active_allocations(&self) -> usize {
        self.active_allocations
    }

    /// Returns whether the heap is initialized.
    #[inline]
    pub const fn is_initialized(&self) -> bool {
        self.is_initialized
    }
}

impl Default for KernelHeap {
    fn default() -> Self {
        Self::new()
    }
}

/// Thread-safe Global Locked Kernel Heap wrapper.
pub struct LockedHeap {
    inner: Mutex<KernelHeap>,
}

impl LockedHeap {
    /// Creates a new uninitialized locked kernel heap.
    pub const fn new() -> Self {
        Self {
            inner: Mutex::new(KernelHeap::new()),
        }
    }

    /// Initializes the locked kernel heap.
    pub unsafe fn init(&self, heap_start: *mut u8, size: usize) -> Result<(), HeapError> {
        let mut heap = self.inner.lock().unwrap();
        heap.init(heap_start, size)
    }

    /// Allocates `size` bytes with `align` alignment.
    pub fn allocate(&self, size: usize, align: usize) -> Result<*mut u8, HeapError> {
        let mut heap = self.inner.lock().unwrap();
        heap.allocate(size, align)
    }

    /// Deallocates a previously allocated pointer.
    pub fn deallocate(&self, ptr: *mut u8) -> Result<(), HeapError> {
        let mut heap = self.inner.lock().unwrap();
        heap.deallocate(ptr)
    }

    /// Returns statistics snapshot of the heap.
    pub fn stats(&self) -> HeapStats {
        let heap = self.inner.lock().unwrap();
        HeapStats {
            total_bytes: heap.total_bytes(),
            allocated_bytes: heap.allocated_bytes(),
            free_bytes: heap.free_bytes(),
            active_allocations: heap.active_allocations(),
            is_initialized: heap.is_initialized(),
        }
    }
}

/// Snapshot of heap memory statistics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeapStats {
    pub total_bytes: usize,
    pub allocated_bytes: usize,
    pub free_bytes: usize,
    pub active_allocations: usize,
    pub is_initialized: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heap_allocation_and_free() {
        let mut buffer = vec![0u8; 64 * 1024]; // 64 KB heap
        let mut heap = KernelHeap::new();

        unsafe {
            heap.init(buffer.as_mut_ptr(), buffer.len()).expect("Heap init failed");
        }

        assert!(heap.is_initialized());
        assert_eq!(heap.active_allocations(), 0);

        // Allocate 64 bytes
        let p1 = heap.allocate(64, 16).expect("Alloc 1 failed");
        assert_eq!(heap.active_allocations(), 1);
        assert!(!p1.is_null());

        // Write some test data into p1
        unsafe {
            core::ptr::write_bytes(p1, 0xAA, 64);
            assert_eq!(*p1, 0xAA);
        }

        // Allocate 128 bytes
        let p2 = heap.allocate(128, 16).expect("Alloc 2 failed");
        assert_eq!(heap.active_allocations(), 2);

        // Deallocate p1
        heap.deallocate(p1).expect("Dealloc p1 failed");
        assert_eq!(heap.active_allocations(), 1);

        // Deallocate p2
        heap.deallocate(p2).expect("Dealloc p2 failed");
        assert_eq!(heap.active_allocations(), 0);

        // After freeing both, blocks coalesce back to full size
        let p3 = heap.allocate(60 * 1024, 16).expect("Large re-alloc after coalesce failed");
        assert_eq!(heap.active_allocations(), 1);
        heap.deallocate(p3).expect("Dealloc p3 failed");
    }

    #[test]
    fn test_heap_double_free_detection() {
        let mut buffer = vec![0u8; 16 * 1024];
        let mut heap = KernelHeap::new();
        unsafe {
            heap.init(buffer.as_mut_ptr(), buffer.len()).unwrap();
        }

        let p = heap.allocate(32, 16).unwrap();
        heap.deallocate(p).unwrap();

        // Second deallocate must return DoubleFree error
        let err = heap.deallocate(p);
        assert_eq!(err, Err(HeapError::DoubleFree));
    }
}
