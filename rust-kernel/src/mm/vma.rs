//! Virtual Memory Area (VMA) and Address Space Management.

use alloc::vec::Vec;
use crate::mm::paging::{VirtAddr, PageTableFlags};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmaType {
    Anonymous,
    FileBacked,
    Stack,
    Heap,
    DemandZero,
}

#[derive(Debug, Clone)]
pub struct VirtualMemoryArea {
    pub start: VirtAddr,
    pub end: VirtAddr,
    pub flags: PageTableFlags,
    pub vma_type: VmaType,
    pub is_cow: bool,
    pub is_shared: bool,
}

impl VirtualMemoryArea {
    pub fn new(start: VirtAddr, end: VirtAddr, flags: PageTableFlags, vma_type: VmaType) -> Self {
        assert!(start < end);
        assert!(start.is_page_aligned() && end.is_page_aligned());
        Self {
            start,
            end,
            flags,
            vma_type,
            is_cow: false,
            is_shared: false,
        }
    }

    #[inline]
    pub fn contains(&self, addr: VirtAddr) -> bool {
        addr >= self.start && addr < self.end
    }

    #[inline]
    pub fn size(&self) -> u64 {
        self.end.0 - self.start.0
    }
}

pub struct AddressSpaceVmas {
    pub vmas: Vec<VirtualMemoryArea>,
}

impl AddressSpaceVmas {
    pub const fn new() -> Self {
        Self { vmas: Vec::new() }
    }

    pub fn insert(&mut self, vma: VirtualMemoryArea) {
        self.vmas.push(vma);
        self.vmas.sort_by_key(|v| v.start);
    }

    pub fn find_vma(&self, addr: VirtAddr) -> Option<&VirtualMemoryArea> {
        self.vmas.iter().find(|v| v.contains(addr))
    }

    pub fn find_vma_mut(&mut self, addr: VirtAddr) -> Option<&mut VirtualMemoryArea> {
        self.vmas.iter_mut().find(|v| v.contains(addr))
    }
}
