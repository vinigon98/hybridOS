//! Bare-Metal Copy-on-Write (CoW) and Demand Paging Implementation.

use core::sync::atomic::{AtomicU32, Ordering};
use alloc::vec::Vec;
use crate::mm::frame::{PhysAddr, PhysFrame, PAGE_SIZE};
use crate::mm::paging::{PageTableManager, PageTableFlags, VirtAddr, PageTableEntry};
use crate::sync::SpinLock;

pub const MAX_TRACKED_FRAMES: usize = 1048576; // 4GB of physical RAM / 4KB frames

pub struct FrameRefCountTable {
    counts: Vec<AtomicU32>,
}

impl FrameRefCountTable {
    pub fn new(max_frames: usize) -> Self {
        let mut counts = Vec::with_capacity(max_frames);
        for _ in 0..max_frames {
            counts.push(AtomicU32::new(0));
        }
        Self { counts }
    }

    #[inline]
    fn frame_index(&self, phys: PhysAddr) -> Option<usize> {
        let idx = (phys.as_u64() / PAGE_SIZE as u64) as usize;
        if idx < self.counts.len() {
            Some(idx)
        } else {
            None
        }
    }

    pub fn get_ref(&self, phys: PhysAddr) -> u32 {
        self.frame_index(phys)
            .map(|idx| self.counts[idx].load(Ordering::Acquire))
            .unwrap_or(0)
    }

    pub fn inc_ref(&self, phys: PhysAddr) {
        if let Some(idx) = self.frame_index(phys) {
            self.counts[idx].fetch_add(1, Ordering::SeqCst);
        }
    }

    pub fn dec_ref(&self, phys: PhysAddr) -> u32 {
        if let Some(idx) = self.frame_index(phys) {
            self.counts[idx].fetch_sub(1, Ordering::SeqCst)
        } else {
            0
        }
    }

    pub fn set_ref(&self, phys: PhysAddr, val: u32) {
        if let Some(idx) = self.frame_index(phys) {
            self.counts[idx].store(val, Ordering::SeqCst);
        }
    }
}

pub static FRAME_REF_COUNTS: SpinLock<Option<FrameRefCountTable>> = SpinLock::new(None);

pub fn init_frame_ref_counts(max_frames: usize) {
    let mut guard = FRAME_REF_COUNTS.lock();
    *guard = Some(FrameRefCountTable::new(max_frames));
}

/// Handles Page Fault (#PF, Vector 14) for CoW and Demand Paging.
/// Returns true if the fault was resolved and the instruction can be re-executed.
pub unsafe fn handle_page_fault(
    fault_addr: VirtAddr,
    error_code: u64,
    hhdm_offset: u64,
) -> bool {
    let is_present = (error_code & 1) != 0;
    let is_write = (error_code & 2) != 0;

    let cr3 = crate::arch::msr::read_cr3() & 0x000F_FFFF_FFFF_F000;
    let phys_frame = match PhysFrame::from_start_address(PhysAddr::new(cr3)) {
        Ok(f) => f,
        Err(_) => return false,
    };
    let mut pt_manager = PageTableManager::from_existing(phys_frame, hhdm_offset);

    // 1. Demand Paging: Page Not Present (bit 0 == 0)
    if !is_present {
        // Null pointer dereference protection: page 0 (0x0..0x1000) is strictly unmapped
        if fault_addr.as_u64() < 0x1000 {
            return false;
        }

        let mut alloc_guard = crate::mm::GLOBAL_FRAME_ALLOCATOR.lock();
        if let Some(ref mut alloc) = *alloc_guard {
            if let Some(new_frame) = alloc.allocate_frame() {
                let frame_hhdm = (new_frame.start_address().as_u64() + hhdm_offset) as *mut u8;
                core::ptr::write_bytes(frame_hhdm, 0, PAGE_SIZE);

                let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER | PageTableFlags::NO_EXECUTE;
                let page_aligned = fault_addr.align_down();
                let _ = pt_manager.map_page(page_aligned, new_frame, flags, alloc);

                if let Some(ref ref_table) = *FRAME_REF_COUNTS.lock() {
                    ref_table.set_ref(new_frame.start_address(), 1);
                }
                return true;
            }
        }
        return false;
    }

    // 2. Copy-on-Write: Page Present, Write Violation (bit 0 == 1 && bit 1 == 1)
    if is_present && is_write {
        if let Some(pte) = pt_manager.get_pte_mut(fault_addr) {
            let flags = pte.flags();
            if flags.contains(PageTableFlags::COW) {
                let old_phys = pte.addr();
                let ref_count = {
                    let guard = FRAME_REF_COUNTS.lock();
                    if let Some(ref table) = *guard {
                        table.get_ref(old_phys)
                    } else {
                        1
                    }
                };

                if ref_count > 1 {
                    // Frame is shared between parent and child -> allocate fresh frame and copy
                    let mut alloc_guard = crate::mm::GLOBAL_FRAME_ALLOCATOR.lock();
                    if let Some(ref mut alloc) = *alloc_guard {
                        if let Some(new_frame) = alloc.allocate_frame() {
                            let src = (old_phys.as_u64() + hhdm_offset) as *const u8;
                            let dst = (new_frame.start_address().as_u64() + hhdm_offset) as *mut u8;
                            core::ptr::copy_nonoverlapping(src, dst, PAGE_SIZE);

                            // Decrement ref count of old frame
                            if let Some(ref ref_table) = *FRAME_REF_COUNTS.lock() {
                                ref_table.dec_ref(old_phys);
                                ref_table.set_ref(new_frame.start_address(), 1);
                            }

                            // Update PTE: remove COW, restore WRITABLE
                            let mut new_flags = flags;
                            new_flags.remove(PageTableFlags::COW);
                            new_flags.insert(PageTableFlags::WRITABLE);
                            pte.set_addr(new_frame.start_address(), new_flags);

                            // Invalidate TLB for this virtual address
                            core::arch::asm!("invlpg [{}]", in(reg) fault_addr.0, options(nostack, preserves_flags));
                            return true;
                        }
                    }
                } else {
                    // Last owner of the frame -> promote directly to WRITABLE without allocating
                    let mut new_flags = flags;
                    new_flags.remove(PageTableFlags::COW);
                    new_flags.insert(PageTableFlags::WRITABLE);
                    pte.set_flags(new_flags);

                    core::arch::asm!("invlpg [{}]", in(reg) fault_addr.0, options(nostack, preserves_flags));
                    return true;
                }
            }
        }
    }

    false
}
