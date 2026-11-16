//! HybridOS Kernel Memory Management (MM) Subsystem.
//!
//! Provides a unified, high-performance, and bare-metal memory management architecture:
//! - **Physical Frame Allocator (`mm::frame`)**: Bitmap allocator consuming all Limine USABLE memory regions.
//! - **4-Level x86_64 Paging (`mm::paging`)**: Direct PML4, PDPT, PD, and PT structures mapped via HHDM and loaded into CR3.
//! - **Kernel Dynamic Heap (`mm::heap`)**: Block-based free-list allocator mapped into dedicated virtual memory.
//! - **Global MM Core (`init_mm()`)**: Subsystem bootstrap, page mapping, and CR3 hardware activation.

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
pub mod vma;
pub mod cow;

use crate::sync::SpinLock;

// Re-export core types for easy kernel-wide access
pub use frame::{PhysAddr, PhysFrame, BitmapFrameAllocator, FrameError, PAGE_SIZE, PAGE_SHIFT, PAGE_MASK};
pub use paging::{VirtAddr, PageTableFlags, PageTableEntry, PageTable, PageTableManager, PagingError};
pub use heap::{KernelHeap, LockedHeap, HeapError, HeapStats};

/// Virtual memory base address allocated for the Kernel Dynamic Heap.
pub const KERNEL_HEAP_VIRT_START: u64 = 0xFFFF_9000_0000_0000;

/// Kernel heap memory size (16 Megabytes).
pub const KERNEL_HEAP_SIZE: usize = 16 * 1024 * 1024;

/// Global Physical Frame Allocator singleton.
pub static GLOBAL_FRAME_ALLOCATOR: SpinLock<Option<BitmapFrameAllocator>> = SpinLock::new(None);

/// Global Kernel 4-Level Page Table Manager singleton.
pub static KERNEL_PAGE_TABLE: SpinLock<Option<PageTableManager>> = SpinLock::new(None);

/// Global Kernel Dynamic Heap Allocator singleton.
pub static GLOBAL_HEAP: LockedHeap = LockedHeap::new();

/// Static bitmap storage (65,536 x 64-bit words = 512 KiB) managing up to 16 GB of physical RAM.
static mut BITMAP_STORAGE: [u64; 65536] = [u64::MAX; 65536];

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

/// Initializes the HybridOS Kernel Memory Management subsystem on bare metal.
///
/// 1. Consumes all USABLE physical memory regions from Limine Memory Map via `BitmapFrameAllocator`.
/// 2. Allocates a new physical PML4 frame and copies Limine Higher-Half entries (256..512).
/// 3. Identity-maps the lower 16MB of physical RAM for early kernel execution.
/// 4. Maps 16MB of physical RAM into dedicated virtual heap space (`KERNEL_HEAP_VIRT_START`).
/// 5. Activates the new root page table in hardware by updating the `CR3` CPU register.
/// 6. Initializes `GLOBAL_HEAP` at `KERNEL_HEAP_VIRT_START`.
pub fn init_mm() -> Result<MemoryInfo, &'static str> {
    crate::serial_println!("[MM] ================================================================");
    crate::serial_println!("[MM] Initializing HybridOS Bare-Metal MMU Subsystem (x86_64 Long Mode)...");

    let hhdm_offset = crate::boot::get_hhdm_offset();
    crate::serial_println!("[MM] Limine HHDM Base Offset: 0x{:016X}", hhdm_offset);

    // 1. Initialize Physical Frame Allocator from Limine memory map
    let memmap_entries = crate::boot::get_memmap_entries()
        .ok_or("Failed to retrieve Limine physical memory map")?;

    let mut frame_allocator = BitmapFrameAllocator::init_from_limine(
        unsafe { &mut BITMAP_STORAGE },
        memmap_entries,
    );

    crate::serial_println!("[MM] Physical Frame Allocator online:");
    crate::serial_println!("     - Hardware RAM Detected: {} MB ({} bytes)",
        frame_allocator.detected_usable_memory() / (1024 * 1024), frame_allocator.detected_usable_memory());
    crate::serial_println!("     - Allocator Bitmap Limit: {} MB ({} frames)",
        frame_allocator.total_memory() / (1024 * 1024), frame_allocator.total_frames());
    crate::serial_println!("     - Usable Free RAM:        {} MB ({} frames)",
        frame_allocator.free_memory() / (1024 * 1024), frame_allocator.free_frames());

    // 2. Allocate new physical PML4 table
    let mut page_table = PageTableManager::new(&mut frame_allocator, hhdm_offset)
        .map_err(|_| "Failed to allocate root PML4 table")?;
    let new_pml4_frame = page_table.root_frame();

    crate::serial_println!("[MM] Allocated new physical Root PML4 frame at: 0x{:016X}", new_pml4_frame.start_address().as_u64());

    // 3. Inherit Higher-Half mappings (Kernel code, Limine HHDM, Framebuffer) from bootloader CR3
    let current_cr3 = (unsafe { crate::arch::msr::read_cr3() }) & 0x000F_FFFF_FFFF_F000;
    unsafe {
        let active_pml4 = page_table.table_at_phys(PhysAddr::new(current_cr3));
        let new_pml4 = page_table.table_at_phys(new_pml4_frame.start_address());

        // Copy Higher-Half entries (256..512)
        for i in 256..512 {
            new_pml4.entries[i] = active_pml4.entries[i];
        }
    }
    crate::serial_println!("[MM] Higher-Half direct mappings copied from initial bootloader CR3.");

    // 4. Identity-map first 16MB (4096 pages) for early hardware, VGA, and low kernel pages
    let identity_pages = 16 * 1024 * 1024 / PAGE_SIZE;
    page_table.identity_map_range(
        PhysAddr::new(0),
        identity_pages,
        PageTableFlags::KERNEL_RW,
        &mut frame_allocator,
    ).map_err(|_| "Failed to identity-map early physical memory")?;
    crate::serial_println!("[MM] Identity mapped lower 16MB (0x00000000 - 0x01000000).");

    // 5. Allocate physical frames and map dedicated Virtual Memory for Kernel Heap
    let heap_pages = KERNEL_HEAP_SIZE / PAGE_SIZE; // 4096 pages (16MB)
    for i in 0..heap_pages {
        let heap_frame = frame_allocator.allocate_frame()
            .ok_or("Out of memory allocating physical frames for Kernel Heap")?;
        let va = VirtAddr::new(KERNEL_HEAP_VIRT_START + (i as u64 * PAGE_SIZE as u64)).unwrap();
        page_table.map_page(
            va,
            heap_frame,
            PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::GLOBAL | PageTableFlags::NO_EXECUTE,
            &mut frame_allocator,
        ).map_err(|_| "Failed to map virtual page for Kernel Heap")?;
    }
    crate::serial_println!("[MM] Kernel Heap mapped at VA 0x{:016X} ({} MB, 4096 physical pages).",
        KERNEL_HEAP_VIRT_START, KERNEL_HEAP_SIZE / (1024 * 1024));

    // 6. ACTIVATE REAL CR3 REGISTER IN CPU HARDWARE & ENABLE CR0.WP!
    unsafe {
        crate::arch::msr::write_cr3(new_pml4_frame.start_address().as_u64());
        crate::arch::msr::enable_cr0_wp();
    }
    crate::serial_println!("[MM] >>> CR3 HARDWARE SWITCH COMPLETE (Active PML4: 0x{:016X}) & CR0.WP ENABLED! <<<",
        new_pml4_frame.start_address().as_u64());

    // 7. Initialize Kernel Dynamic Heap
    unsafe {
        GLOBAL_HEAP.init(KERNEL_HEAP_VIRT_START as *mut u8, KERNEL_HEAP_SIZE)
            .map_err(|_| "Failed to initialize Kernel Dynamic Heap")?;
    }
    let heap_stats = GLOBAL_HEAP.stats();
    crate::serial_println!("[MM] Kernel Dynamic Heap Allocator online at virtual address 0x{:016X} ({} KB).",
        KERNEL_HEAP_VIRT_START, heap_stats.total_bytes / 1024);

    // Initialize Copy-on-Write frame reference table AFTER heap is initialized
    let tracked_frames = frame_allocator.total_frames().min(cow::MAX_TRACKED_FRAMES).min(262144);
    cow::init_frame_ref_counts(tracked_frames);

    let info = MemoryInfo {
        total_physical_bytes: frame_allocator.detected_usable_memory(),
        used_physical_bytes: frame_allocator.used_memory(),
        free_physical_bytes: frame_allocator.free_memory(),
        total_frames: frame_allocator.total_frames(),
        allocated_frames: frame_allocator.allocated_frames(),
        free_frames: frame_allocator.free_frames(),
        kernel_pml4_cr3: new_pml4_frame.start_address(),
        heap_total_bytes: heap_stats.total_bytes,
        heap_used_bytes: heap_stats.allocated_bytes,
        heap_free_bytes: heap_stats.free_bytes,
        heap_allocations: heap_stats.active_allocations,
    };

    // Store in global singletons
    *GLOBAL_FRAME_ALLOCATOR.lock() = Some(frame_allocator);
    *KERNEL_PAGE_TABLE.lock() = Some(page_table);

    crate::serial_println!("[MM] Memory Management Subsystem successfully online with 100% real hardware paging!");
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

/// Creates a new user address space with higher-half kernel mappings cloned from the active PML4.
pub fn create_user_page_table() -> Result<(PageTableManager, PhysFrame), PagingError> {
    let mut guard = GLOBAL_FRAME_ALLOCATOR.lock();
    let alloc = guard.as_mut().ok_or(PagingError::FrameAllocationFailed)?;
    let hhdm_offset = crate::boot::get_hhdm_offset();
    let mgr = PageTableManager::new_with_kernel_higher_half(alloc, hhdm_offset)?;
    let pml4_frame = mgr.root_frame();
    Ok((mgr, pml4_frame))
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

/// Maps physical MMIO device frames into kernel higher-half virtual address space.
pub fn map_kernel_mmio(phys_base: u64, pages: usize) -> Result<u64, &'static str> {
    let mut page_guard = KERNEL_PAGE_TABLE.lock();
    let mut frame_guard = GLOBAL_FRAME_ALLOCATOR.lock();
    let page_table = page_guard.as_mut().ok_or("Kernel page table uninitialized")?;
    let allocator = frame_guard.as_mut().ok_or("Frame allocator uninitialized")?;
    let hhdm = crate::boot::get_hhdm_offset();
    let virt_base = hhdm + phys_base;

    for i in 0..pages {
        let paddr = PhysAddr::new(phys_base + (i as u64 * 4096));
        let vaddr = VirtAddr::new_truncate(virt_base + (i as u64 * 4096));
        let frame = PhysFrame::from_start_address(paddr).map_err(|_| "Unaligned MMIO frame")?;
        let flags = PageTableFlags::PRESENT
            | PageTableFlags::WRITABLE
            | PageTableFlags::NO_CACHE
            | PageTableFlags::WRITE_THROUGH
            | PageTableFlags::NO_EXECUTE;
        let _ = page_table.map_page(vaddr, frame, flags, allocator);
    }

    Ok(virt_base)
}
