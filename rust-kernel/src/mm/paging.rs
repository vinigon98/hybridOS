//! 4-Level x86_64 Physical Paging Architecture for HybridOS Kernel.
//!
//! Implements PML4 (Level 4), PDPT (Level 3), PD (Level 2), and PT (Level 1) structures,
//! page table entry flags (PRESENT, WRITABLE, USER, NO_EXECUTE, etc.),
//! virtual-to-physical address translation, and virtual memory page mapping.
//!
//! All page tables are real 4096-byte aligned physical memory frames accessed
//! directly via the Limine Higher-Half Direct Map (HHDM). Zero BTreeMap simulation.

#[allow(unused_imports)]
use alloc::string::{String, ToString};
#[allow(unused_imports)]
use alloc::vec::Vec;
#[allow(unused_imports)]
use alloc::boxed::Box;
#[allow(unused_imports)]
use crate::compat::FloatExt;
use core::fmt;
use core::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Not, Index, IndexMut};

use crate::mm::frame::{PhysAddr, PhysFrame, BitmapFrameAllocator, PAGE_SIZE, PAGE_MASK, PAGE_SHIFT};

/// Number of entries in each x86_64 page table level (512 entries per table).
pub const ENTRY_COUNT: usize = 512;

/// Bitmask for the physical frame address in a PageTableEntry (bits 12..52).
pub const PHYSICAL_ADDRESS_MASK: u64 = 0x000F_FFFF_FFFF_F000;

/// Represents a 64-bit canonical virtual memory address.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[repr(transparent)]
pub struct VirtAddr(pub u64);

impl VirtAddr {
    /// Creates a canonical virtual address.
    /// Bits 47 to 63 must be identical (sign extension).
    #[inline]
    pub fn new(addr: u64) -> Result<Self, PagingError> {
        let sign_bit = (addr >> 47) & 1;
        let prefix = addr >> 48;
        if (sign_bit == 0 && prefix == 0) || (sign_bit == 1 && prefix == 0xFFFF) {
            Ok(Self(addr))
        } else {
            Err(PagingError::InvalidAddress(addr))
        }
    }

    /// Creates a canonical virtual address, truncating / sign-extending bit 47.
    #[inline]
    pub const fn new_truncate(addr: u64) -> Self {
        let sign_extended = (((addr << 16) as i64) >> 16) as u64;
        Self(sign_extended)
    }

    /// Returns the raw 64-bit virtual address.
    #[inline]
    pub const fn as_u64(&self) -> u64 {
        self.0
    }

    /// Returns the raw usize representation.
    #[inline]
    pub const fn as_usize(&self) -> usize {
        self.0 as usize
    }

    /// Casts this virtual address to a raw immutable pointer.
    #[inline]
    pub const fn as_ptr<T>(&self) -> *const T {
        self.0 as *const T
    }

    /// Casts this virtual address to a raw mutable pointer.
    #[inline]
    pub const fn as_mut_ptr<T>(&self) -> *mut T {
        self.0 as *mut T
    }

    /// Level 4 (PML4) entry index (bits 39..48).
    #[inline]
    pub const fn p4_index(&self) -> usize {
        ((self.0 >> 39) & 0x1FF) as usize
    }

    /// Level 3 (PDPT) entry index (bits 30..39).
    #[inline]
    pub const fn p3_index(&self) -> usize {
        ((self.0 >> 30) & 0x1FF) as usize
    }

    /// Level 2 (PD) entry index (bits 21..30).
    #[inline]
    pub const fn p2_index(&self) -> usize {
        ((self.0 >> 21) & 0x1FF) as usize
    }

    /// Level 1 (PT) entry index (bits 12..21).
    #[inline]
    pub const fn p1_index(&self) -> usize {
        ((self.0 >> 12) & 0x1FF) as usize
    }

    /// Offset within the 4KB page (bits 0..12).
    #[inline]
    pub const fn page_offset(&self) -> usize {
        (self.0 & PAGE_MASK) as usize
    }

    /// Returns true if the address is aligned to the given byte boundary.
    #[inline]
    pub const fn is_aligned_to(&self, align: u64) -> bool {
        (self.0 & (align - 1)) == 0
    }

    /// Returns true if the address is 4KB page aligned.
    #[inline]
    pub const fn is_page_aligned(&self) -> bool {
        self.is_aligned_to(PAGE_SIZE as u64)
    }

    /// Aligns address down to 4KB boundary.
    #[inline]
    pub const fn align_down(&self) -> Self {
        Self(self.0 & !PAGE_MASK)
    }

    /// Aligns address up to 4KB boundary.
    #[inline]
    pub const fn align_up(&self) -> Self {
        Self((self.0 + PAGE_MASK) & !PAGE_MASK)
    }
}

impl fmt::Debug for VirtAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "VirtAddr(0x{:016X})", self.0)
    }
}

impl fmt::Display for VirtAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x{:016X}", self.0)
    }
}

impl core::ops::Add<u64> for VirtAddr {
    type Output = Self;
    #[inline]
    fn add(self, rhs: u64) -> Self::Output {
        VirtAddr::new_truncate(self.0 + rhs)
    }
}

impl core::ops::Sub<u64> for VirtAddr {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: u64) -> Self::Output {
        VirtAddr::new_truncate(self.0 - rhs)
    }
}

/// Architectural x86_64 Page Table Entry Flags.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
#[repr(transparent)]
pub struct PageTableFlags(pub u64);

impl PageTableFlags {
    pub const PRESENT: Self       = Self(1 << 0);
    pub const WRITABLE: Self      = Self(1 << 1);
    pub const USER: Self          = Self(1 << 2);
    pub const WRITE_THROUGH: Self = Self(1 << 3);
    pub const NO_CACHE: Self      = Self(1 << 4);
    pub const ACCESSED: Self      = Self(1 << 5);
    pub const DIRTY: Self         = Self(1 << 6);
    pub const HUGE_PAGE: Self     = Self(1 << 7);
    pub const GLOBAL: Self        = Self(1 << 8);
    pub const COW: Self           = Self(1 << 9);
    pub const DEMAND: Self        = Self(1 << 10);
    pub const NO_EXECUTE: Self    = Self(1 << 63);

    pub const KERNEL_RO: Self   = Self::PRESENT;
    pub const KERNEL_RW: Self   = Self(Self::PRESENT.0 | Self::WRITABLE.0);
    pub const KERNEL_CODE: Self = Self::PRESENT;
    pub const KERNEL_DATA: Self = Self(Self::PRESENT.0 | Self::WRITABLE.0 | Self::NO_EXECUTE.0);

    pub const USER_RO: Self   = Self(Self::PRESENT.0 | Self::USER.0);
    pub const USER_RW: Self   = Self(Self::PRESENT.0 | Self::WRITABLE.0 | Self::USER.0 | Self::NO_EXECUTE.0);
    pub const USER_CODE: Self = Self(Self::PRESENT.0 | Self::USER.0);
    pub const USER_DATA: Self = Self(Self::PRESENT.0 | Self::WRITABLE.0 | Self::USER.0 | Self::NO_EXECUTE.0);

    #[inline]
    pub const fn empty() -> Self {
        Self(0)
    }

    #[inline]
    pub const fn bits(&self) -> u64 {
        self.0
    }

    #[inline]
    pub const fn contains(&self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    #[inline]
    pub fn insert(&mut self, other: Self) {
        self.0 |= other.0;
    }

    #[inline]
    pub fn remove(&mut self, other: Self) {
        self.0 &= !other.0;
    }
}

impl BitOr for PageTableFlags {
    type Output = Self;
    #[inline]
    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for PageTableFlags {
    #[inline]
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl BitAnd for PageTableFlags {
    type Output = Self;
    #[inline]
    fn bitand(self, rhs: Self) -> Self::Output {
        Self(self.0 & rhs.0)
    }
}

impl BitAndAssign for PageTableFlags {
    #[inline]
    fn bitand_assign(&mut self, rhs: Self) {
        self.0 &= rhs.0;
    }
}

impl BitXor for PageTableFlags {
    type Output = Self;
    #[inline]
    fn bitxor(self, rhs: Self) -> Self::Output {
        Self(self.0 ^ rhs.0)
    }
}

impl BitXorAssign for PageTableFlags {
    #[inline]
    fn bitxor_assign(&mut self, rhs: Self) {
        self.0 ^= rhs.0;
    }
}

impl Not for PageTableFlags {
    type Output = Self;
    #[inline]
    fn not(self) -> Self::Output {
        Self(!self.0)
    }
}

impl fmt::Debug for PageTableFlags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Flags(0x{:X})", self.0)
    }
}

/// A 64-bit Entry in an x86_64 Page Table.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
#[repr(transparent)]
pub struct PageTableEntry(pub u64);

impl PageTableEntry {
    #[inline]
    pub const fn new() -> Self {
        Self(0)
    }

    #[inline]
    pub const fn is_unused(&self) -> bool {
        self.0 == 0
    }

    #[inline]
    pub fn set_unused(&mut self) {
        self.0 = 0;
    }

    #[inline]
    pub fn flags(&self) -> PageTableFlags {
        PageTableFlags(self.0 & !PHYSICAL_ADDRESS_MASK)
    }

    #[inline]
    pub fn addr(&self) -> PhysAddr {
        PhysAddr::new(self.0 & PHYSICAL_ADDRESS_MASK)
    }

    #[inline]
    pub fn set_addr(&mut self, addr: PhysAddr, flags: PageTableFlags) {
        assert!(addr.is_page_aligned(), "Physical address must be 4KB aligned");
        self.0 = (addr.as_u64() & PHYSICAL_ADDRESS_MASK) | flags.bits();
    }

    #[inline]
    pub fn set_flags(&mut self, flags: PageTableFlags) {
        let addr_bits = self.0 & PHYSICAL_ADDRESS_MASK;
        self.0 = addr_bits | flags.bits();
    }

    #[inline]
    pub fn is_present(&self) -> bool {
        self.flags().contains(PageTableFlags::PRESENT)
    }

    #[inline]
    pub fn is_writable(&self) -> bool {
        self.flags().contains(PageTableFlags::WRITABLE)
    }

    #[inline]
    pub fn is_user(&self) -> bool {
        self.flags().contains(PageTableFlags::USER)
    }

    #[inline]
    pub fn is_no_execute(&self) -> bool {
        self.flags().contains(PageTableFlags::NO_EXECUTE)
    }
}

impl fmt::Debug for PageTableEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_unused() {
            write!(f, "PageTableEntry(UNUSED)")
        } else {
            write!(f, "PageTableEntry(Addr: {}, Flags: 0x{:X})", self.addr(), self.flags().bits())
        }
    }
}

/// A real 4096-byte x86_64 Page Table containing 512 64-bit entries.
#[derive(Clone)]
#[repr(C, align(4096))]
pub struct PageTable {
    pub entries: [PageTableEntry; ENTRY_COUNT],
}

impl PageTable {
    pub const fn new() -> Self {
        Self {
            entries: [PageTableEntry::new(); ENTRY_COUNT],
        }
    }

    pub fn zero(&mut self) {
        for entry in self.entries.iter_mut() {
            entry.set_unused();
        }
    }

    pub fn iter(&self) -> core::slice::Iter<'_, PageTableEntry> {
        self.entries.iter()
    }

    pub fn iter_mut(&mut self) -> core::slice::IterMut<'_, PageTableEntry> {
        self.entries.iter_mut()
    }
}

impl Default for PageTable {
    fn default() -> Self {
        Self::new()
    }
}

impl Index<usize> for PageTable {
    type Output = PageTableEntry;
    #[inline]
    fn index(&self, index: usize) -> &Self::Output {
        &self.entries[index]
    }
}

impl IndexMut<usize> for PageTable {
    #[inline]
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.entries[index]
    }
}

/// Paging errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PagingError {
    InvalidAddress(u64),
    PageAlreadyMapped(VirtAddr),
    PageNotMapped(VirtAddr),
    FrameAllocationFailed,
    HugePageNotSupported(VirtAddr),
}

impl fmt::Display for PagingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidAddress(a) => write!(f, "Virtual address 0x{:016X} is non-canonical", a),
            Self::PageAlreadyMapped(va) => write!(f, "Virtual page {} is already mapped", va),
            Self::PageNotMapped(va) => write!(f, "Virtual page {} is not mapped", va),
            Self::FrameAllocationFailed => write!(f, "Out of physical frames for page table allocation"),
            Self::HugePageNotSupported(va) => write!(f, "Huge page mapping encountered at {}", va),
        }
    }
}

/// Physical Page Table Manager for an Address Space.
///
/// Directly manages the 4-level x86_64 page table hierarchy in physical RAM
/// via the Higher-Half Direct Map (HHDM).
pub struct PageTableManager {
    /// Physical frame address of the root Level 4 PML4 table (loaded into CR3).
    pml4_frame: PhysFrame,
    /// HHDM offset used to translate physical frames into kernel virtual pointers.
    hhdm_offset: u64,
}

impl PageTableManager {
    /// Initializes a new page table manager, allocating the root PML4 from the given frame allocator.
    pub fn new(allocator: &mut BitmapFrameAllocator, hhdm_offset: u64) -> Result<Self, PagingError> {
        let pml4_frame = allocator.allocate_frame().ok_or(PagingError::FrameAllocationFailed)?;
        let manager = Self {
            pml4_frame,
            hhdm_offset,
        };
        // Zero out the newly allocated root PML4
        unsafe {
            let pml4 = manager.table_at_phys(pml4_frame.start_address());
            pml4.zero();
        }
        Ok(manager)
    }

    /// Attaches to an existing PML4 table in physical memory.
    pub fn from_existing(pml4_frame: PhysFrame, hhdm_offset: u64) -> Self {
        Self {
            pml4_frame,
            hhdm_offset,
        }
    }

    /// Traverses the 4-level page hierarchy and returns a mutable reference to the leaf PTE.
    pub fn get_pte_mut(&mut self, virt: VirtAddr) -> Option<&'static mut PageTableEntry> {
        unsafe {
            let pml4 = self.table_at_phys(self.pml4_frame.start_address());
            let p4_entry = &mut pml4.entries[virt.p4_index()];
            if !p4_entry.is_present() { return None; }

            let pdpt = self.table_at_phys(p4_entry.addr());
            let p3_entry = &mut pdpt.entries[virt.p3_index()];
            if !p3_entry.is_present() { return None; }

            let pd = self.table_at_phys(p3_entry.addr());
            let p2_entry = &mut pd.entries[virt.p2_index()];
            if !p2_entry.is_present() || p2_entry.flags().contains(PageTableFlags::HUGE_PAGE) { return None; }

            let pt = self.table_at_phys(p2_entry.addr());
            let p1_entry = &mut pt.entries[virt.p1_index()];
            if !p1_entry.is_present() { return None; }

            Some(p1_entry)
        }
    }

    /// Duplicates user-space mappings (PML4 entries 0..256) into a child PML4 applying Copy-on-Write.
    /// Writable user pages are made Read-Only and tagged with PageTableFlags::COW.
    pub fn duplicate_user_mappings_cow(
        &mut self,
        child_manager: &mut PageTableManager,
        allocator: &mut BitmapFrameAllocator,
        ref_table: &crate::mm::cow::FrameRefCountTable,
    ) -> Result<(), PagingError> {
        unsafe {
            let parent_pml4 = self.table_at_phys(self.pml4_frame.start_address());
            for p4_idx in 0..256 {
                let p4_entry = &mut parent_pml4.entries[p4_idx];
                if !p4_entry.is_present() { continue; }

                let parent_pdpt = self.table_at_phys(p4_entry.addr());
                for p3_idx in 0..512 {
                    let p3_entry = &mut parent_pdpt.entries[p3_idx];
                    if !p3_entry.is_present() { continue; }

                    let parent_pd = self.table_at_phys(p3_entry.addr());
                    for p2_idx in 0..512 {
                        let p2_entry = &mut parent_pd.entries[p2_idx];
                        if !p2_entry.is_present() || p2_entry.flags().contains(PageTableFlags::HUGE_PAGE) { continue; }

                        let parent_pt = self.table_at_phys(p2_entry.addr());
                        for p1_idx in 0..512 {
                            let p1_entry = &mut parent_pt.entries[p1_idx];
                            if !p1_entry.is_present() { continue; }

                            let frame_phys = p1_entry.addr();
                            let mut flags = p1_entry.flags();

                            if flags.contains(PageTableFlags::WRITABLE) {
                                // Mark as CoW in parent: remove WRITABLE, add COW
                                flags.remove(PageTableFlags::WRITABLE);
                                flags.insert(PageTableFlags::COW);
                                p1_entry.set_flags(flags);
                            }

                            // Increment frame reference count for child
                            ref_table.inc_ref(frame_phys);

                            // Map the identical frame into child PML4 with same CoW flags
                            let virt = VirtAddr::new_truncate(
                                ((p4_idx as u64) << 39) |
                                ((p3_idx as u64) << 30) |
                                ((p2_idx as u64) << 21) |
                                ((p1_idx as u64) << 12)
                            );
                            let child_frame = PhysFrame::from_start_address(frame_phys).unwrap();
                            child_manager.map_page(virt, child_frame, flags, allocator)?;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Creates a new user address space PML4 that inherits the Kernel Higher-Half (entries 256..512).
    pub fn new_with_kernel_higher_half(
        allocator: &mut BitmapFrameAllocator,
        hhdm_offset: u64,
    ) -> Result<Self, PagingError> {
        let pml4_frame = allocator.allocate_frame().ok_or(PagingError::FrameAllocationFailed)?;
        let manager = Self {
            pml4_frame,
            hhdm_offset,
        };

        unsafe {
            let new_pml4 = manager.table_at_phys(pml4_frame.start_address());
            new_pml4.zero();

            // Read current CR3 to find active kernel PML4
            let active_cr3 = crate::arch::msr::read_cr3() & 0x000F_FFFF_FFFF_F000;
            let active_pml4 = manager.table_at_phys(PhysAddr::new(active_cr3));

            // Clone upper 256 entries (Kernel space: HHDM, kernel image, device MMIO)
            for i in 256..512 {
                new_pml4.entries[i] = active_pml4.entries[i];
            }
        }

        Ok(manager)
    }

    /// Accesses a page table frame at a physical address via the HHDM.
    #[inline]
    pub unsafe fn table_at_phys(&self, phys: PhysAddr) -> &'static mut PageTable {
        let virt_ptr = (self.hhdm_offset + phys.as_u64()) as *mut PageTable;
        &mut *virt_ptr
    }

    /// Returns the physical frame of the root PML4 table (loaded into CR3).
    #[inline]
    pub fn root_frame(&self) -> PhysFrame {
        self.pml4_frame
    }

    /// Helper to get or allocate a next-level table.
    fn get_or_create_subtable(
        &mut self,
        parent_phys: PhysAddr,
        index: usize,
        allocator: &mut BitmapFrameAllocator,
    ) -> Result<PhysAddr, PagingError> {
        let parent = unsafe { self.table_at_phys(parent_phys) };
        let entry = &mut parent.entries[index];

        if entry.is_present() {
            Ok(entry.addr())
        } else {
            let sub_frame = allocator.allocate_frame().ok_or(PagingError::FrameAllocationFailed)?;
            let sub_addr = sub_frame.start_address();

            unsafe {
                let sub_table = self.table_at_phys(sub_addr);
                sub_table.zero();
            }

            // Intermediate directories receive PRESENT | WRITABLE | USER so leaf PT controls final rights
            entry.set_addr(
                sub_addr,
                PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER,
            );

            Ok(sub_addr)
        }
    }

    /// Maps a 4KB virtual page to a physical frame with specified flags and invalidates TLB.
    pub fn map_page(
        &mut self,
        va: VirtAddr,
        frame: PhysFrame,
        flags: PageTableFlags,
        allocator: &mut BitmapFrameAllocator,
    ) -> Result<(), PagingError> {
        let p4_idx = va.p4_index();
        let p3_idx = va.p3_index();
        let p2_idx = va.p2_index();
        let p1_idx = va.p1_index();

        // Level 4 -> Level 3 (PDPT)
        let pdpt_addr = self.get_or_create_subtable(self.pml4_frame.start_address(), p4_idx, allocator)?;

        // Level 3 -> Level 2 (PD)
        let pd_addr = self.get_or_create_subtable(pdpt_addr, p3_idx, allocator)?;

        // Level 2 -> Level 1 (PT)
        let pt_addr = self.get_or_create_subtable(pd_addr, p2_idx, allocator)?;

        // Level 1: Leaf Page Table Entry
        let pt = unsafe { self.table_at_phys(pt_addr) };
        let entry = &mut pt.entries[p1_idx];

        if entry.is_present() {
            return Err(PagingError::PageAlreadyMapped(va));
        }

        entry.set_addr(frame.start_address(), flags | PageTableFlags::PRESENT);

        // Hardware TLB Invalidation for this virtual address
        unsafe {
            core::arch::asm!("invlpg [{}]", in(reg) va.as_u64(), options(nostack, preserves_flags));
        }

        Ok(())
    }

    /// Unmaps a 4KB virtual page, returning the previously mapped physical frame, and invalidates TLB.
    pub fn unmap_page(&mut self, va: VirtAddr) -> Result<PhysFrame, PagingError> {
        let p4_idx = va.p4_index();
        let p3_idx = va.p3_index();
        let p2_idx = va.p2_index();
        let p1_idx = va.p1_index();

        let pml4 = unsafe { self.table_at_phys(self.pml4_frame.start_address()) };
        let p4_entry = &pml4.entries[p4_idx];
        if !p4_entry.is_present() {
            return Err(PagingError::PageNotMapped(va));
        }

        let pdpt = unsafe { self.table_at_phys(p4_entry.addr()) };
        let p3_entry = &pdpt.entries[p3_idx];
        if !p3_entry.is_present() {
            return Err(PagingError::PageNotMapped(va));
        }

        let pd = unsafe { self.table_at_phys(p3_entry.addr()) };
        let p2_entry = &pd.entries[p2_idx];
        if !p2_entry.is_present() {
            return Err(PagingError::PageNotMapped(va));
        }

        let pt = unsafe { self.table_at_phys(p2_entry.addr()) };
        let p1_entry = &mut pt.entries[p1_idx];
        if !p1_entry.is_present() {
            return Err(PagingError::PageNotMapped(va));
        }

        let frame = PhysFrame::containing_address(p1_entry.addr());
        p1_entry.set_unused();

        // Hardware TLB Invalidation
        unsafe {
            core::arch::asm!("invlpg [{}]", in(reg) va.as_u64(), options(nostack, preserves_flags));
        }

        Ok(frame)
    }

    /// Translates a virtual address to its corresponding physical address.
    pub fn translate(&self, va: VirtAddr) -> Option<PhysAddr> {
        let p4_idx = va.p4_index();
        let p3_idx = va.p3_index();
        let p2_idx = va.p2_index();
        let p1_idx = va.p1_index();

        let pml4 = unsafe { self.table_at_phys(self.pml4_frame.start_address()) };
        let p4_entry = &pml4.entries[p4_idx];
        if !p4_entry.is_present() {
            return None;
        }

        let pdpt = unsafe { self.table_at_phys(p4_entry.addr()) };
        let p3_entry = &pdpt.entries[p3_idx];
        if !p3_entry.is_present() {
            return None;
        }
        if p3_entry.flags().contains(PageTableFlags::HUGE_PAGE) {
            let base = p3_entry.addr().as_u64();
            let offset = va.as_u64() & 0x3FFF_FFFF;
            return Some(PhysAddr::new(base + offset));
        }

        let pd = unsafe { self.table_at_phys(p3_entry.addr()) };
        let p2_entry = &pd.entries[p2_idx];
        if !p2_entry.is_present() {
            return None;
        }
        if p2_entry.flags().contains(PageTableFlags::HUGE_PAGE) {
            let base = p2_entry.addr().as_u64();
            let offset = va.as_u64() & 0x1F_FFFF;
            return Some(PhysAddr::new(base + offset));
        }

        let pt = unsafe { self.table_at_phys(p2_entry.addr()) };
        let p1_entry = &pt.entries[p1_idx];
        if !p1_entry.is_present() {
            return None;
        }

        let page_phys = p1_entry.addr().as_u64();
        let page_offset = va.page_offset() as u64;
        Some(PhysAddr::new(page_phys + page_offset))
    }

    /// Queries the flags for a virtual page.
    pub fn translate_flags(&self, va: VirtAddr) -> Option<PageTableFlags> {
        let p4_idx = va.p4_index();
        let p3_idx = va.p3_index();
        let p2_idx = va.p2_index();
        let p1_idx = va.p1_index();

        let pml4 = unsafe { self.table_at_phys(self.pml4_frame.start_address()) };
        let p4_entry = &pml4.entries[p4_idx];
        if !p4_entry.is_present() { return None; }

        let pdpt = unsafe { self.table_at_phys(p4_entry.addr()) };
        let p3_entry = &pdpt.entries[p3_idx];
        if !p3_entry.is_present() { return None; }

        let pd = unsafe { self.table_at_phys(p3_entry.addr()) };
        let p2_entry = &pd.entries[p2_idx];
        if !p2_entry.is_present() { return None; }

        let pt = unsafe { self.table_at_phys(p2_entry.addr()) };
        let p1_entry = &pt.entries[p1_idx];
        if !p1_entry.is_present() { return None; }

        Some(p1_entry.flags())
    }

    /// Updates flags for an already mapped virtual page and invalidates TLB.
    pub fn update_flags(&mut self, va: VirtAddr, flags: PageTableFlags) -> Result<(), PagingError> {
        let p4_idx = va.p4_index();
        let p3_idx = va.p3_index();
        let p2_idx = va.p2_index();
        let p1_idx = va.p1_index();

        let pml4 = unsafe { self.table_at_phys(self.pml4_frame.start_address()) };
        let p4_entry = &pml4.entries[p4_idx];
        if !p4_entry.is_present() { return Err(PagingError::PageNotMapped(va)); }

        let pdpt = unsafe { self.table_at_phys(p4_entry.addr()) };
        let p3_entry = &pdpt.entries[p3_idx];
        if !p3_entry.is_present() { return Err(PagingError::PageNotMapped(va)); }

        let pd = unsafe { self.table_at_phys(p3_entry.addr()) };
        let p2_entry = &pd.entries[p2_idx];
        if !p2_entry.is_present() { return Err(PagingError::PageNotMapped(va)); }

        let pt = unsafe { self.table_at_phys(p2_entry.addr()) };
        let entry = &mut pt.entries[p1_idx];

        if !entry.is_present() {
            return Err(PagingError::PageNotMapped(va));
        }

        entry.set_flags(flags | PageTableFlags::PRESENT);

        // Hardware TLB Invalidation
        unsafe {
            core::arch::asm!("invlpg [{}]", in(reg) va.as_u64(), options(nostack, preserves_flags));
        }

        Ok(())
    }

    /// Maps a contiguous range of virtual pages to physical frames.
    pub fn map_range(
        &mut self,
        start_va: VirtAddr,
        start_frame: PhysFrame,
        page_count: usize,
        flags: PageTableFlags,
        allocator: &mut BitmapFrameAllocator,
    ) -> Result<(), PagingError> {
        for i in 0..page_count {
            let offset = (i as u64) << PAGE_SHIFT;
            let va = start_va + offset;
            let frame = PhysFrame::containing_address(start_frame.start_address() + offset);
            self.map_page(va, frame, flags, allocator)?;
        }
        Ok(())
    }

    /// Identity-maps a physical memory range.
    pub fn identity_map_range(
        &mut self,
        start_pa: PhysAddr,
        page_count: usize,
        flags: PageTableFlags,
        allocator: &mut BitmapFrameAllocator,
    ) -> Result<(), PagingError> {
        let start_frame = PhysFrame::from_start_address(start_pa)
            .map_err(|_| PagingError::InvalidAddress(start_pa.as_u64()))?;
        let start_va = VirtAddr::new_truncate(start_pa.as_u64());

        self.map_range(start_va, start_frame, page_count, flags, allocator)
    }
}
