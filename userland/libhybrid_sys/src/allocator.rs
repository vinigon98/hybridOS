//! User-space Ring 3 Global Heap Allocator for HybridOS.
//!
//! Directly requests real 4 KiB/64 KiB memory pages from the HybridOS kernel
//! via Linux `sys_mmap` or Windows `NtAllocateVirtualMemory`.

use core::alloc::{GlobalAlloc, Layout};
use core::sync::atomic::{AtomicUsize, Ordering};

const CHUNK_SIZE: usize = 64 * 1024; // 64 KiB per kernel allocation request

pub struct HybridUserAllocator {
    current_page: AtomicUsize,
    offset: AtomicUsize,
}

impl HybridUserAllocator {
    pub const fn new() -> Self {
        Self {
            current_page: AtomicUsize::new(0),
            offset: AtomicUsize::new(0),
        }
    }

    fn request_pages(&self, size: usize) -> *mut u8 {
        let alloc_size = (size + 4095) & !4095;
        // Try Linux sys_mmap first
        let ptr = crate::linux::sys_mmap(
            0,
            alloc_size,
            crate::linux::PROT_READ | crate::linux::PROT_WRITE,
            crate::linux::MAP_PRIVATE | crate::linux::MAP_ANONYMOUS,
            usize::MAX,
            0,
        );
        if !ptr.is_null() && (ptr as usize) != usize::MAX {
            return ptr;
        }

        // Fallback to Windows NtAllocateVirtualMemory
        let mut base_addr: *mut u8 = core::ptr::null_mut();
        let mut region_size = alloc_size;
        let status = crate::windows::nt_allocate_virtual_memory(
            usize::MAX, // NtCurrentProcess()
            &mut base_addr,
            0,
            &mut region_size,
            crate::windows::MEM_COMMIT | crate::windows::MEM_RESERVE,
            crate::windows::PAGE_READWRITE,
        );
        if status == crate::windows::STATUS_SUCCESS && !base_addr.is_null() {
            base_addr
        } else {
            core::ptr::null_mut()
        }
    }
}

unsafe impl GlobalAlloc for HybridUserAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let align = layout.align();
        let size = layout.size();

        // Simple aligned bump allocation backed by 64 KiB kernel chunks
        let mut cur_offset = self.offset.load(Ordering::Relaxed);
        let mut cur_page = self.current_page.load(Ordering::Relaxed);

        let aligned_offset = (cur_offset + align - 1) & !(align - 1);
        if cur_page == 0 || (aligned_offset + size) > CHUNK_SIZE {
            // Allocate a new chunk from the kernel
            let new_chunk = self.request_pages(CHUNK_SIZE.max(size));
            if new_chunk.is_null() {
                return core::ptr::null_mut();
            }
            cur_page = new_chunk as usize;
            self.current_page.store(cur_page, Ordering::SeqCst);
            cur_offset = 0;
        }

        let alloc_ptr = (cur_page + cur_offset) as *mut u8;
        self.offset.store(cur_offset + size, Ordering::SeqCst);
        alloc_ptr
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        // Bump allocator: pages are reclaimed when process exits or resets
    }
}
