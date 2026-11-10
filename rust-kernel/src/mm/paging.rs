//! 4-Level x86_64 Paging Architecture for HybridOS Kernel.
//!
//! Implements PML4 (Level 4), PDPT (Level 3), PD (Level 2), and PT (Level 1) structures,
//! page table entry flags (PRESENT, WRITABLE, USER, NO_EXECUTE, etc.),
//! virtual-to-physical address translation, and virtual memory page mapping.

use core::fmt;
use core::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Not, Index, IndexMut};
use std::collections::HashMap;

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
        // Sign-extend bit 47 to bits 48..63
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
    /// Page is present in physical memory.
    pub const PRESENT: Self = Self(1 << 0);
    /// Page is writable (read/write). If clear, page is read-only.
    pub const WRITABLE: Self = Self(1 << 1);
    /// Page is accessible from User Mode (Ring 3). If clear, Supervisor Mode (Ring 0-2) only.
    pub const USER: Self = Self(1 << 2);
    /// Write-through caching enabled.
    pub const WRITE_THROUGH: Self = Self(1 << 3);
    /// Cache disable (uncached memory, e.g. MMIO registers).
    pub const NO_CACHE: Self = Self(1 << 4);
    /// CPU has read or written to this page.
    pub const ACCESSED: Self = Self(1 << 5);
    /// CPU has written to this page.
    pub const DIRTY: Self = Self(1 << 6);
    /// Huge Page (2MB in PD, 1GB in PDPT).
    pub const HUGE_PAGE: Self = Self(1 << 7);
    /// Global page (TLB not flushed on CR3 reload).
    pub const GLOBAL: Self = Self(1 << 8);
    /// No-Execute (NX / XD bit 63). Prevents code execution on data pages.
    pub const NO_EXECUTE: Self = Self(1 << 63);

    /// Standard Kernel Read-Only: PRESENT
    pub const KERNEL_RO: Self = Self::PRESENT;
    /// Standard Kernel Read-Write: PRESENT | WRITABLE
    pub const KERNEL_RW: Self = Self(Self::PRESENT.0 | Self::WRITABLE.0);
    /// Standard Kernel Code: PRESENT | WRITABLE (or ro) without NX
    pub const KERNEL_CODE: Self = Self::PRESENT;
    /// Standard Kernel Data: PRESENT | WRITABLE | NO_EXECUTE
    pub const KERNEL_DATA: Self = Self(Self::PRESENT.0 | Self::WRITABLE.0 | Self::NO_EXECUTE.0);

    /// Standard User Read-Only: PRESENT | USER
    pub const USER_RO: Self = Self(Self::PRESENT.0 | Self::USER.0);
    /// Standard User Read-Write: PRESENT | WRITABLE | USER | NO_EXECUTE
    pub const USER_RW: Self = Self(Self::PRESENT.0 | Self::WRITABLE.0 | Self::USER.0 | Self::NO_EXECUTE.0);
    /// Standard User Code: PRESENT | USER
    pub const USER_CODE: Self = Self(Self::PRESENT.0 | Self::USER.0);
    /// Standard User Data / Stack / Heap: PRESENT | WRITABLE | USER | NO_EXECUTE
    pub const USER_DATA: Self = Self(Self::PRESENT.0 | Self::WRITABLE.0 | Self::USER.0 | Self::NO_EXECUTE.0);

    /// Creates an empty set of flags.
    #[inline]
    pub const fn empty() -> Self {
        Self(0)
    }

    /// Returns raw 64-bit bitmask.
    #[inline]
    pub const fn bits(&self) -> u64 {
        self.0
    }

    /// Checks if a specific flag is set.
    #[inline]
    pub const fn contains(&self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Inserts flags.
    #[inline]
    pub fn insert(&mut self, other: Self) {
        self.0 |= other.0;
    }

    /// Removes flags.
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
        let mut list = Vec::new();
        if self.contains(Self::PRESENT) { list.push("PRESENT"); }
        if self.contains(Self::WRITABLE) { list.push("WRITABLE"); }
        if self.contains(Self::USER) { list.push("USER"); }
        if self.contains(Self::WRITE_THROUGH) { list.push("WRITE_THROUGH"); }
        if self.contains(Self::NO_CACHE) { list.push("NO_CACHE"); }
        if self.contains(Self::ACCESSED) { list.push("ACCESSED"); }
        if self.contains(Self::DIRTY) { list.push("DIRTY"); }
        if self.contains(Self::HUGE_PAGE) { list.push("HUGE_PAGE"); }
        if self.contains(Self::GLOBAL) { list.push("GLOBAL"); }
        if self.contains(Self::NO_EXECUTE) { list.push("NO_EXECUTE"); }

        write!(f, "PageTableFlags({:?})", list)
    }
}

/// A 64-bit Entry in an x86_64 Page Table.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
#[repr(transparent)]
pub struct PageTableEntry(pub u64);

impl PageTableEntry {
    /// Creates an unused page table entry (0x0).
    #[inline]
    pub const fn new() -> Self {
        Self(0)
    }

    /// Checks if this entry is empty / unused.
    #[inline]
    pub const fn is_unused(&self) -> bool {
        self.0 == 0
    }

    /// Marks this entry as unused (cleared to 0).
    #[inline]
    pub fn set_unused(&mut self) {
        self.0 = 0;
    }

    /// Extracts the flags from this entry.
    #[inline]
    pub fn flags(&self) -> PageTableFlags {
        PageTableFlags(self.0 & !PHYSICAL_ADDRESS_MASK)
    }

    /// Extracts the physical frame address mapped by this entry.
    #[inline]
    pub fn addr(&self) -> PhysAddr {
        PhysAddr::new(self.0 & PHYSICAL_ADDRESS_MASK)
    }

    /// Sets the mapped physical address and flags.
    #[inline]
    pub fn set_addr(&mut self, addr: PhysAddr, flags: PageTableFlags) {
        assert!(addr.is_page_aligned(), "Physical address must be 4KB aligned");
        self.0 = (addr.as_u64() & PHYSICAL_ADDRESS_MASK) | flags.bits();
    }

    /// Updates flags while keeping existing physical address.
    #[inline]
    pub fn set_flags(&mut self, flags: PageTableFlags) {
        let addr_bits = self.0 & PHYSICAL_ADDRESS_MASK;
        self.0 = addr_bits | flags.bits();
    }

    /// Convenience check for PRESENT bit.
    #[inline]
    pub fn is_present(&self) -> bool {
        self.flags().contains(PageTableFlags::PRESENT)
    }

    /// Convenience check for WRITABLE bit.
    #[inline]
    pub fn is_writable(&self) -> bool {
        self.flags().contains(PageTableFlags::WRITABLE)
    }

    /// Convenience check for USER bit.
    #[inline]
    pub fn is_user(&self) -> bool {
        self.flags().contains(PageTableFlags::USER)
    }

    /// Convenience check for NO_EXECUTE bit.
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
            write!(f, "PageTableEntry(Addr: {}, Flags: {:?})", self.addr(), self.flags())
        }
    }
}

/// An x86_64 Page Table structure containing 512 64-bit entries (4096 bytes total).
#[derive(Clone)]
#[repr(align(4096))]
pub struct PageTable {
    pub entries: [PageTableEntry; ENTRY_COUNT],
}

impl PageTable {
    /// Creates a zeroed, empty page table.
    pub fn new() -> Self {
        Self {
            entries: [PageTableEntry::new(); ENTRY_COUNT],
        }
    }

    /// Clears all 512 entries to unused.
    pub fn zero(&mut self) {
        for entry in self.entries.iter_mut() {
            entry.set_unused();
        }
    }

    /// Returns an iterator over entries.
    pub fn iter(&self) -> core::slice::Iter<'_, PageTableEntry> {
        self.entries.iter()
    }

    /// Returns a mutable iterator over entries.
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
    /// Virtual address is non-canonical or invalid.
    InvalidAddress(u64),
    /// Page is already mapped in this address space.
    PageAlreadyMapped(VirtAddr),
    /// Page is not mapped in this address space.
    PageNotMapped(VirtAddr),
    /// Physical frame allocator failed to allocate a table frame.
    FrameAllocationFailed,
    /// Huge pages (2MB / 1GB) traversal not expanded.
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

impl std::error::Error for PagingError {}

/// Page Table Manager for an Address Space.
///
/// Manages the full 4-level x86_64 page table hierarchy (PML4 -> PDPT -> PD -> PT).
/// Provides virtual-to-physical address translation, single page mapping/unmapping,
/// range mapping, and permission updates.
pub struct PageTableManager {
    /// Physical frame address of the root Level 4 PML4 table (CR3 register value).
    pml4_frame: PhysFrame,
    /// Storage for page tables in this address space, indexed by their physical address.
    tables: HashMap<PhysAddr, PageTable>,
}

impl PageTableManager {
    /// Initializes a new page table manager, allocating the root PML4 from the given frame allocator.
    pub fn new(allocator: &mut BitmapFrameAllocator) -> Result<Self, PagingError> {
        let pml4_frame = allocator.allocate_frame().ok_or(PagingError::FrameAllocationFailed)?;
        let mut tables = HashMap::new();
        tables.insert(pml4_frame.start_address(), PageTable::new());

        Ok(Self {
            pml4_frame,
            tables,
        })
    }

    /// Returns the physical frame of the root PML4 table (value loaded into CR3).
    #[inline]
    pub fn root_frame(&self) -> PhysFrame {
        self.pml4_frame
    }

    /// Maps a 4KB virtual page to a physical frame with specified flags.
    /// Allocates intermediary page tables (PDPT, PD, PT) on demand.
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

        // 1. Get or create Level 3 (PDPT) table
        let pdpt_addr = self.get_or_create_subtable(self.pml4_frame.start_address(), p4_idx, allocator)?;

        // 2. Get or create Level 2 (PD) table
        let pd_addr = self.get_or_create_subtable(pdpt_addr, p3_idx, allocator)?;

        // 3. Get or create Level 1 (PT) table
        let pt_addr = self.get_or_create_subtable(pd_addr, p2_idx, allocator)?;

        // 4. Map the physical frame in the PT entry
        let pt = self.tables.get_mut(&pt_addr).expect("PT table missing from storage");
        let entry = &mut pt[p1_idx];

        if entry.is_present() {
            return Err(PagingError::PageAlreadyMapped(va));
        }

        entry.set_addr(frame.start_address(), flags | PageTableFlags::PRESENT);
        Ok(())
    }

    /// Helper to get or create a next-level table.
    fn get_or_create_subtable(
        &mut self,
        parent_addr: PhysAddr,
        index: usize,
        allocator: &mut BitmapFrameAllocator,
    ) -> Result<PhysAddr, PagingError> {
        let parent_table = self.tables.get(&parent_addr).expect("Parent table missing");
        let entry = parent_table[index];

        if entry.is_present() {
            Ok(entry.addr())
        } else {
            // Allocate a new frame for the subtable
            let sub_frame = allocator.allocate_frame().ok_or(PagingError::FrameAllocationFailed)?;
            let sub_addr = sub_frame.start_address();

            // Insert new zeroed page table
            self.tables.insert(sub_addr, PageTable::new());

            // Update parent entry with PRESENT | WRITABLE | USER permissions so lower tables can restrict
            let parent_mut = self.tables.get_mut(&parent_addr).unwrap();
            parent_mut[index].set_addr(
                sub_addr,
                PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER,
            );

            Ok(sub_addr)
        }
    }

    /// Unmaps a 4KB virtual page, returning the previously mapped physical frame.
    pub fn unmap_page(&mut self, va: VirtAddr) -> Result<PhysFrame, PagingError> {
        let p4_idx = va.p4_index();
        let p3_idx = va.p3_index();
        let p2_idx = va.p2_index();
        let p1_idx = va.p1_index();

        let pml4 = self.tables.get(&self.pml4_frame.start_address()).unwrap();
        if !pml4[p4_idx].is_present() {
            return Err(PagingError::PageNotMapped(va));
        }
        let pdpt_addr = pml4[p4_idx].addr();

        let pdpt = self.tables.get(&pdpt_addr).ok_or(PagingError::PageNotMapped(va))?;
        if !pdpt[p3_idx].is_present() {
            return Err(PagingError::PageNotMapped(va));
        }
        let pd_addr = pdpt[p3_idx].addr();

        let pd = self.tables.get(&pd_addr).ok_or(PagingError::PageNotMapped(va))?;
        if !pd[p2_idx].is_present() {
            return Err(PagingError::PageNotMapped(va));
        }
        let pt_addr = pd[p2_idx].addr();

        let pt = self.tables.get_mut(&pt_addr).ok_or(PagingError::PageNotMapped(va))?;
        let entry = &mut pt[p1_idx];

        if !entry.is_present() {
            return Err(PagingError::PageNotMapped(va));
        }

        let frame = PhysFrame::containing_address(entry.addr());
        entry.set_unused();

        Ok(frame)
    }

    /// Translates a virtual address to its corresponding physical address.
    /// Returns `Some(PhysAddr)` if mapped, or `None` if unmapped / not present.
    pub fn translate(&self, va: VirtAddr) -> Option<PhysAddr> {
        let p4_idx = va.p4_index();
        let p3_idx = va.p3_index();
        let p2_idx = va.p2_index();
        let p1_idx = va.p1_index();

        // Level 4 (PML4)
        let pml4 = self.tables.get(&self.pml4_frame.start_address())?;
        let p4_entry = pml4[p4_idx];
        if !p4_entry.is_present() {
            return None;
        }

        // Level 3 (PDPT)
        let pdpt = self.tables.get(&p4_entry.addr())?;
        let p3_entry = pdpt[p3_idx];
        if !p3_entry.is_present() {
            return None;
        }
        if p3_entry.flags().contains(PageTableFlags::HUGE_PAGE) {
            // 1GB huge page
            let base = p3_entry.addr().as_u64();
            let offset = va.as_u64() & 0x3FFF_FFFF;
            return Some(PhysAddr::new(base + offset));
        }

        // Level 2 (PD)
        let pd = self.tables.get(&p3_entry.addr())?;
        let p2_entry = pd[p2_idx];
        if !p2_entry.is_present() {
            return None;
        }
        if p2_entry.flags().contains(PageTableFlags::HUGE_PAGE) {
            // 2MB huge page
            let base = p2_entry.addr().as_u64();
            let offset = va.as_u64() & 0x1F_FFFF;
            return Some(PhysAddr::new(base + offset));
        }

        // Level 1 (PT)
        let pt = self.tables.get(&p2_entry.addr())?;
        let p1_entry = pt[p1_idx];
        if !p1_entry.is_present() {
            return None;
        }

        // 4KB page: Physical address + offset
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

        let pml4 = self.tables.get(&self.pml4_frame.start_address())?;
        let p4_entry = pml4[p4_idx];
        if !p4_entry.is_present() { return None; }

        let pdpt = self.tables.get(&p4_entry.addr())?;
        let p3_entry = pdpt[p3_idx];
        if !p3_entry.is_present() { return None; }

        let pd = self.tables.get(&p3_entry.addr())?;
        let p2_entry = pd[p2_idx];
        if !p2_entry.is_present() { return None; }

        let pt = self.tables.get(&p2_entry.addr())?;
        let p1_entry = pt[p1_idx];
        if !p1_entry.is_present() { return None; }

        Some(p1_entry.flags())
    }

    /// Updates flags for an already mapped virtual page.
    pub fn update_flags(&mut self, va: VirtAddr, flags: PageTableFlags) -> Result<(), PagingError> {
        let p4_idx = va.p4_index();
        let p3_idx = va.p3_index();
        let p2_idx = va.p2_index();
        let p1_idx = va.p1_index();

        let pml4 = self.tables.get(&self.pml4_frame.start_address()).unwrap();
        if !pml4[p4_idx].is_present() { return Err(PagingError::PageNotMapped(va)); }

        let pdpt = self.tables.get(&pml4[p4_idx].addr()).ok_or(PagingError::PageNotMapped(va))?;
        if !pdpt[p3_idx].is_present() { return Err(PagingError::PageNotMapped(va)); }

        let pd = self.tables.get(&pdpt[p3_idx].addr()).ok_or(PagingError::PageNotMapped(va))?;
        if !pd[p2_idx].is_present() { return Err(PagingError::PageNotMapped(va)); }

        let pt = self.tables.get_mut(&pd[p2_idx].addr()).ok_or(PagingError::PageNotMapped(va))?;
        let entry = &mut pt[p1_idx];

        if !entry.is_present() {
            return Err(PagingError::PageNotMapped(va));
        }

        entry.set_flags(flags | PageTableFlags::PRESENT);
        Ok(())
    }

    /// Maps a range of contiguous virtual pages to contiguous physical frames.
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

    /// Identity-maps a physical memory range (Virtual Address == Physical Address).
    /// Used for kernel startup, framebuffer, and early device drivers.
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

    /// Returns the total number of page tables currently allocated in this address space.
    pub fn table_count(&self) -> usize {
        self.tables.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_virt_addr_indices() {
        // Test an address with known index bit patterns:
        // P4 = 1, P3 = 2, P2 = 3, P1 = 4, offset = 0x567
        let raw: u64 = (1 << 39) | (2 << 30) | (3 << 21) | (4 << 12) | 0x567;
        let va = VirtAddr::new(raw).unwrap();

        assert_eq!(va.p4_index(), 1);
        assert_eq!(va.p3_index(), 2);
        assert_eq!(va.p2_index(), 3);
        assert_eq!(va.p1_index(), 4);
        assert_eq!(va.page_offset(), 0x567);
    }

    #[test]
    fn test_page_table_mapping_and_translation() {
        let mut allocator = BitmapFrameAllocator::new(PhysAddr::new(0x100000), 1024 * PAGE_SIZE);
        let mut manager = PageTableManager::new(&mut allocator).expect("Failed to create PageTableManager");

        let va = VirtAddr::new(0x4000_1000).unwrap();
        let target_frame = PhysFrame::from_number(500); // 500 * 4096 = 0x1F4000
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER;

        manager.map_page(va, target_frame, flags, &mut allocator).expect("Failed to map page");

        // Translate
        let translated = manager.translate(va).expect("Failed to translate mapped address");
        assert_eq!(translated, target_frame.start_address());

        // Check flags
        let trans_flags = manager.translate_flags(va).expect("Failed to get flags");
        assert!(trans_flags.contains(PageTableFlags::PRESENT));
        assert!(trans_flags.contains(PageTableFlags::WRITABLE));
        assert!(trans_flags.contains(PageTableFlags::USER));

        // Unmap
        let unmapped = manager.unmap_page(va).expect("Failed to unmap page");
        assert_eq!(unmapped, target_frame);
        assert_eq!(manager.translate(va), None);
    }

    #[test]
    fn test_identity_map_range() {
        let mut allocator = BitmapFrameAllocator::new(PhysAddr::new(0x200000), 1024 * PAGE_SIZE);
        let mut manager = PageTableManager::new(&mut allocator).unwrap();

        let start_pa = PhysAddr::new(0x0000);
        let pages = 16;
        manager.identity_map_range(start_pa, pages, PageTableFlags::KERNEL_RW, &mut allocator).unwrap();

        for i in 0..pages {
            let addr = (i * PAGE_SIZE) as u64;
            let va = VirtAddr::new(addr).unwrap();
            let pa = manager.translate(va).expect("Page not translated");
            assert_eq!(pa.as_u64(), addr);
        }
    }
}
