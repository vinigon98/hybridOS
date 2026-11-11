//! HybridOS Kernel Memory Management (MM) Subsystem.
//!
//! Provides a unified, high-performance, and modular memory management architecture:
//! - **Physical Frame Allocator (`mm::frame`)**: 4KB bitmap allocator managing physical memory frames.
//! - **4-Level x86_64 Paging (`mm::paging`)**: PML4, PDPT, PD, and PT structures with full permission bits.
//! - **Kernel Dynamic Heap (`mm::heap`)**: Block-based free-list allocator with instant coalescing.
//! - **Global MM Core (`init_mm()`)**: Subsystem initialization and global singletons.

#[allow(unused_imports)]
use alloc::string::{String, ToString};
#[allow(unused_imports)]
use alloc::vec::Vec;
#[allow(unused_imports)]
use alloc::boxed::Box;
#[allow(unused_imports)]
use crate::compat::FloatExt;
pub mod frame;
pub mod paging;
pub mod heap;

use crate::sync::SpinLock;

// Re-export core types for easy kernel-wide access
pub use frame::{PhysAddr, PhysFrame, BitmapFrameAllocator, FrameError, PAGE_SIZE, PAGE_SHIFT, PAGE_MASK};
pub use paging::{VirtAddr, PageTableFlags, PageTableEntry, PageTable, PageTableManager, PagingError};
pub use heap::{KernelHeap, LockedHeap, HeapError, HeapStats};

/// Physical memory managed by default (128 Megabytes = 32,768 physical frames).
pub const DEFAULT_PHYSICAL_MEMORY_SIZE: usize = 128 * 1024 * 1024;

/// Kernel heap memory size (4 Megabytes).
pub const KERNEL_HEAP_SIZE: usize = 16 * 1024 * 1024;

/// Global Physical Frame Allocator singleton.
pub static GLOBAL_FRAME_ALLOCATOR: SpinLock<Option<BitmapFrameAllocator>> = SpinLock::new(None);

/// Global Kernel 4-Level Page Table Manager singleton.
pub static KERNEL_PAGE_TABLE: SpinLock<Option<PageTableManager>> = SpinLock::new(None);

/// Global Kernel Dynamic Heap Allocator singleton.
pub static GLOBAL_HEAP: LockedHeap = LockedHeap::new();

/// Backing storage container for the Kernel Dynamic Heap.
#[repr(align(16))]
struct HeapStorage {
    buffer: core::cell::UnsafeCell<[u8; KERNEL_HEAP_SIZE]>,
}

unsafe impl Sync for HeapStorage {}

/// Static backing memory buffer for the Kernel Dynamic Heap.
static KERNEL_HEAP_STORAGE: HeapStorage = HeapStorage {
    buffer: core::cell::UnsafeCell::new([0u8; KERNEL_HEAP_SIZE]),
};

/// Comprehensive summary of system memory status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryInfo {
    pub total_physical_bytes: usize,
    pub used_physical_bytes: usize,
    pub free_physical_bytes: usize,
    pub total_frames: usize,
    pub allocated_frames: usize,
    pub free_frames: usize,
    pub kernel_pml4_cr3: PhysAddr,
    pub heap_total_bytes: usize,
    pub heap_used_bytes: usize,
    pub heap_free_bytes: usize,
    pub heap_allocations: usize,
}

/// Initializes the HybridOS Kernel Memory Management subsystem.
///
/// 1. Initializes the Physical Frame Allocator (`BitmapFrameAllocator`) with 128MB RAM.
/// 2. Reserves low physical memory (0x0 - 0x100000 for BIOS, IVT, VGA, and early loader).
/// 3. Initializes 4-Level x86_64 Paging (`PageTableManager`) with root PML4 table.
/// 4. Identity-maps the lower 16MB of physical RAM (Kernel image, stacks, MM tables).
/// 5. Initializes the Kernel Dynamic Heap Allocator with 4MB capacity.
pub fn init_mm() -> Result<MemoryInfo, &'static str> {
    crate::serial_println!("[MM] ================================================================");
    crate::serial_println!("[MM] Initializing HybridOS Memory Management Subsystem (Rust MM)...");

    // 1. Kernel Dynamic Heap Initialization (4 MB) FIRST so allocators/Vec can function!
    let heap_ptr = KERNEL_HEAP_STORAGE.buffer.get() as *mut u8;
    unsafe {
        GLOBAL_HEAP.init(heap_ptr, KERNEL_HEAP_SIZE)
            .map_err(|_| "Failed to initialize Kernel Dynamic Heap")?;
    }
    let heap_stats = GLOBAL_HEAP.stats();
    crate::serial_println!("[MM] Kernel Dynamic Heap Allocator online:");
    crate::serial_println!("     - Base Address:  0x{:016X}", heap_ptr as usize);
    crate::serial_println!("     - Capacity:      {} KB ({} bytes)", heap_stats.total_bytes / 1024, heap_stats.total_bytes);
    crate::serial_println!("     - Alignment:     16 bytes (SIMD/AVX ready)");

    // 2. Physical Frame Allocator Initialization
    let mut frame_allocator = BitmapFrameAllocator::new(PhysAddr::new(0), DEFAULT_PHYSICAL_MEMORY_SIZE);

    // 3. Reserve low 1MB physical memory (IVT, BDA, EBDA, Video RAM, BIOS ROM)
    let low_mem_frames = 0x100000 / PAGE_SIZE; // 256 frames (1MB)
    for i in 0..low_mem_frames {
        let frame = PhysFrame::from_number(i);
        let _ = frame_allocator.mark_allocated(frame);
    }
    crate::serial_println!("[MM] Physical Frame Allocator online:");
    crate::serial_println!("     - Total Memory:  {} MB ({} frames)", DEFAULT_PHYSICAL_MEMORY_SIZE / (1024 * 1024), frame_allocator.total_frames());
    crate::serial_println!("     - Reserved:      {} frames (0x00000000 - 0x00100000)", low_mem_frames);
    crate::serial_println!("     - Free Frames:   {}", frame_allocator.free_frames());

    // 4. 4-Level x86_64 Paging Architecture Initialization
    let mut page_table = PageTableManager::new(&mut frame_allocator)
        .map_err(|_| "Failed to initialize root PML4 page table")?;
    let cr3_frame = page_table.root_frame();
    crate::serial_println!("[MM] 4-Level x86_64 Paging online:");
    crate::serial_println!("     - Root PML4 Table Frame (CR3): {}", cr3_frame);

    // 5. Identity-map first 16MB (4096 pages) for Kernel Code, Data, and Hardware
    let identity_pages = 16 * 1024 * 1024 / PAGE_SIZE; // 4096 pages
    page_table.identity_map_range(
        PhysAddr::new(0),
        identity_pages,
        PageTableFlags::KERNEL_RW,
        &mut frame_allocator,
    ).map_err(|_| "Failed to identity-map early kernel physical memory")?;
    crate::serial_println!("     - Identity Mapped Range: 0x00000000 - 0x01000000 ({} MB / {} pages)", 16, identity_pages);
    crate::serial_println!("     - Alignment:     16 bytes (SIMD/AVX ready)");

    let info = MemoryInfo {
        total_physical_bytes: frame_allocator.total_memory(),
        used_physical_bytes: frame_allocator.used_memory(),
        free_physical_bytes: frame_allocator.free_memory(),
        total_frames: frame_allocator.total_frames(),
        allocated_frames: frame_allocator.allocated_frames(),
        free_frames: frame_allocator.free_frames(),
        kernel_pml4_cr3: cr3_frame.start_address(),
        heap_total_bytes: heap_stats.total_bytes,
        heap_used_bytes: heap_stats.allocated_bytes,
        heap_free_bytes: heap_stats.free_bytes,
        heap_allocations: heap_stats.active_allocations,
    };

    // Store in global singletons
    *GLOBAL_FRAME_ALLOCATOR.lock() = Some(frame_allocator);
    *KERNEL_PAGE_TABLE.lock() = Some(page_table);

    crate::serial_println!("[MM] Memory Management Subsystem successfully online!");
    crate::serial_println!("[MM] ================================================================");

    Ok(info)
}

/// Allocates a single physical frame from the global frame allocator.
pub fn allocate_frame() -> Option<PhysFrame> {
    let mut guard = GLOBAL_FRAME_ALLOCATOR.lock();
    guard.as_mut().and_then(|allocator| allocator.allocate_frame())
}

/// Frees a physical frame back to the global frame allocator.
pub fn free_frame(frame: PhysFrame) -> Result<(), FrameError> {
    let mut guard = GLOBAL_FRAME_ALLOCATOR.lock();
    if let Some(ref mut allocator) = *guard {
        allocator.free_frame(frame)
    } else {
        Err(FrameError::OutOfMemory)
    }
}

/// Allocates memory from the global kernel dynamic heap.
pub fn kernel_allocate(size: usize, align: usize) -> Result<*mut u8, HeapError> {
    GLOBAL_HEAP.allocate(size, align)
}

/// Frees memory to the global kernel dynamic heap.
pub fn kernel_free(ptr: *mut u8) -> Result<(), HeapError> {
    GLOBAL_HEAP.deallocate(ptr)
}

/// Queries current snapshot of system memory info.
pub fn get_memory_info() -> Option<MemoryInfo> {
    let frame_guard = GLOBAL_FRAME_ALLOCATOR.lock();
    let page_guard = KERNEL_PAGE_TABLE.lock();

    let frame_allocator = frame_guard.as_ref()?;
    let page_table = page_guard.as_ref()?;
    let heap_stats = GLOBAL_HEAP.stats();

    Some(MemoryInfo {
        total_physical_bytes: frame_allocator.total_memory(),
        used_physical_bytes: frame_allocator.used_memory(),
        free_physical_bytes: frame_allocator.free_memory(),
        total_frames: frame_allocator.total_frames(),
        allocated_frames: frame_allocator.allocated_frames(),
        free_frames: frame_allocator.free_frames(),
        kernel_pml4_cr3: page_table.root_frame().start_address(),
        heap_total_bytes: heap_stats.total_bytes,
        heap_used_bytes: heap_stats.allocated_bytes,
        heap_free_bytes: heap_stats.free_bytes,
        heap_allocations: heap_stats.active_allocations,
    })
}
