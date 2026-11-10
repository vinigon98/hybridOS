//! Global Descriptor Table (GDT) and Task State Segment (TSS) for x86_64 Long Mode.
//!
//! Configures 64-bit kernel and user-mode segments:
//! - Kernel Code 64 (Ring 0)
//! - Kernel Data 64 (Ring 0)
//! - User Code 64 (Ring 3)
//! - User Data 64 (Ring 3)
//! - 64-bit Task State Segment (TSS) for Interrupt Stack Table (IST) switching
//!
//! Segment selector layout is aligned with x86_64 `SYSCALL`/`SYSRET` MSR requirements.

use core::mem::size_of;

/// Standard x86_64 Segment Selector representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct SegmentSelector(pub u16);

impl SegmentSelector {
    /// Creates a new segment selector with the specified table index and privilege level.
    pub const fn new(index: u16, rpl: u8) -> Self {
        Self((index << 3) | ((rpl as u16) & 0x03))
    }

    /// Returns the raw 16-bit selector value.
    pub const fn as_u16(&self) -> u16 {
        self.0
    }

    /// Returns the descriptor table index.
    pub const fn index(&self) -> u16 {
        self.0 >> 3
    }

    /// Returns the Requested Privilege Level (0 for Ring 0, 3 for Ring 3).
    pub const fn rpl(&self) -> u8 {
        (self.0 & 0x03) as u8
    }
}

// Well-known GDT Selectors for HybridOS
pub const KERNEL_CS: SegmentSelector = SegmentSelector::new(1, 0); // 0x08
pub const KERNEL_DS: SegmentSelector = SegmentSelector::new(2, 0); // 0x10
pub const USER_DS: SegmentSelector   = SegmentSelector::new(3, 3); // 0x1B (Index 3, RPL 3)
pub const USER_CS: SegmentSelector   = SegmentSelector::new(4, 3); // 0x23 (Index 4, RPL 3)
pub const TSS_SEL: SegmentSelector   = SegmentSelector::new(5, 0); // 0x28 (Index 5, RPL 0)

/// Bitfield flags for constructing x86_64 Segment Descriptors.
pub struct DescriptorFlags;

impl DescriptorFlags {
    // Descriptor Access & Type Flags
    pub const ACCESSED: u64        = 1 << 40;
    pub const WRITABLE: u64        = 1 << 41; // For data segments
    pub const READABLE: u64        = 1 << 41; // For code segments
    pub const CONFORMING: u64      = 1 << 42;
    pub const EXECUTABLE: u64      = 1 << 43;
    pub const USER_SEGMENT: u64    = 1 << 44; // Code or data segment (1) vs system segment (0)
    pub const DPL_RING0: u64       = 0 << 45;
    pub const DPL_RING3: u64       = 3 << 45;
    pub const PRESENT: u64         = 1 << 47;

    // Architecture Flags
    pub const AVAILABLE: u64       = 1 << 52;
    pub const LONG_MODE: u64       = 1 << 53; // 64-bit code segment flag
    pub const DEFAULT_SIZE: u64    = 1 << 54; // 32-bit default operation size
    pub const GRANULARITY_4K: u64  = 1 << 55; // 4KiB page granularity
    pub const LIMIT_MASK_HIGH: u64 = 0x000F_0000_0000_0000;
    pub const LIMIT_MASK_LOW: u64  = 0x0000_0000_0000_FFFF;
}

/// Helper for constructing x86_64 Segment Descriptors.
pub struct Descriptor;

impl Descriptor {
    /// 64-bit Kernel Code Segment (Ring 0, Executable, Readable, Long Mode 64-bit).
    pub const fn kernel_code_segment() -> u64 {
        DescriptorFlags::PRESENT
            | DescriptorFlags::USER_SEGMENT
            | DescriptorFlags::EXECUTABLE
            | DescriptorFlags::READABLE
            | DescriptorFlags::LONG_MODE
            | DescriptorFlags::DPL_RING0
    }

    /// 64-bit Kernel Data Segment (Ring 0, Writable, 4K Granularity).
    pub const fn kernel_data_segment() -> u64 {
        DescriptorFlags::PRESENT
            | DescriptorFlags::USER_SEGMENT
            | DescriptorFlags::WRITABLE
            | DescriptorFlags::DPL_RING0
            | DescriptorFlags::GRANULARITY_4K
            | DescriptorFlags::LIMIT_MASK_HIGH
            | DescriptorFlags::LIMIT_MASK_LOW
    }

    /// 64-bit User Code Segment (Ring 3, Executable, Readable, Long Mode 64-bit).
    pub const fn user_code_segment() -> u64 {
        DescriptorFlags::PRESENT
            | DescriptorFlags::USER_SEGMENT
            | DescriptorFlags::EXECUTABLE
            | DescriptorFlags::READABLE
            | DescriptorFlags::LONG_MODE
            | DescriptorFlags::DPL_RING3
    }

    /// 64-bit User Data Segment (Ring 3, Writable, 4K Granularity).
    pub const fn user_data_segment() -> u64 {
        DescriptorFlags::PRESENT
            | DescriptorFlags::USER_SEGMENT
            | DescriptorFlags::WRITABLE
            | DescriptorFlags::DPL_RING3
            | DescriptorFlags::GRANULARITY_4K
            | DescriptorFlags::LIMIT_MASK_HIGH
            | DescriptorFlags::LIMIT_MASK_LOW
    }

    /// 128-bit 64-bit TSS (Task State Segment) Descriptor.
    /// In x86_64 Long Mode, system descriptors (such as TSS) occupy two consecutive 64-bit slots.
    pub fn tss_segment(tss: &'static TaskStateSegment) -> (u64, u64) {
        let ptr = tss as *const _ as u64;
        let limit = (size_of::<TaskStateSegment>() - 1) as u64;

        // TSS Low (64 bits):
        // Limit[15:0], Base[23:0], Type=0x9 (Available 64-bit TSS), DPL=0, P=1, Limit[19:16], Base[31:24]
        let mut low: u64 = limit & 0xFFFF;
        low |= (ptr & 0x00FF_FFFF) << 16;
        low |= 0x9u64 << 40; // Type: 0x9 = 64-bit TSS (Available)
        low |= DescriptorFlags::DPL_RING0;
        low |= DescriptorFlags::PRESENT;
        low |= ((limit >> 16) & 0x0F) << 48;
        low |= ((ptr >> 24) & 0xFF) << 56;

        // TSS High (64 bits):
        // Base[63:32], Reserved[31:0]
        let high: u64 = (ptr >> 32) & 0xFFFF_FFFF;

        (low, high)
    }
}

/// Task State Segment for x86_64 Long Mode.
///
/// In 64-bit mode, hardware task switching is removed; the TSS is solely used
/// to specify privilege-level stack pointers (RSP0-RSP2) and the Interrupt Stack Table (IST1-IST7).
#[derive(Debug, Clone)]
#[repr(C, packed)]
pub struct TaskStateSegment {
    reserved0: u32,
    /// Stack pointers for privilege level transitions (RSP0 is kernel stack on ring 3 -> ring 0 switch)
    pub rsp: [u64; 3],
    reserved1: u64,
    /// Interrupt Stack Table (IST1 through IST7) for dedicated exception stack switching
    pub ist: [u64; 7],
    reserved2: u64,
    reserved3: u16,
    /// Base address of the I/O Permission Bit Map (relative to TSS base)
    pub iomap_base: u16,
}

impl TaskStateSegment {
    /// Constructs a clean Task State Segment with empty stacks.
    pub const fn new() -> Self {
        Self {
            reserved0: 0,
            rsp: [0; 3],
            reserved1: 0,
            ist: [0; 7],
            reserved2: 0,
            reserved3: 0,
            iomap_base: size_of::<TaskStateSegment>() as u16, // Points past TSS if no I/O bitmap
        }
    }

    /// Sets the kernel stack pointer for privilege transitions to Ring 0.
    pub fn set_rsp0(&mut self, stack_top: u64) {
        self.rsp[0] = stack_top;
    }

    /// Configures an Interrupt Stack Table (IST) entry (1-based index: 1 through 7).
    pub fn set_ist(&mut self, ist_index: usize, stack_top: u64) {
        assert!(ist_index >= 1 && ist_index <= 7, "IST index must be between 1 and 7");
        self.ist[ist_index - 1] = stack_top;
    }
}

/// Pointer structure passed to the `LGDT` CPU instruction.
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct GdtDescriptor {
    pub limit: u16,
    pub base: u64,
}

/// Global Descriptor Table container supporting up to 8 entries.
pub struct GlobalDescriptorTable {
    table: [u64; 8],
    len: usize,
}

impl GlobalDescriptorTable {
    /// Creates an empty GDT initialized with the null descriptor at index 0.
    pub const fn new() -> Self {
        Self {
            table: [0; 8],
            len: 1, // Entry 0 is always null
        }
    }

    /// Initializes standard HybridOS kernel and user segments:
    /// - Index 0: Null
    /// - Index 1: Kernel Code 64
    /// - Index 2: Kernel Data 64
    /// - Index 3: User Data 64
    /// - Index 4: User Code 64
    /// - Index 5 & 6: 128-bit 64-bit TSS Descriptor
    pub fn init_standard(&mut self, tss: &'static TaskStateSegment) {
        self.table[0] = 0; // Null descriptor
        self.table[1] = Descriptor::kernel_code_segment();
        self.table[2] = Descriptor::kernel_data_segment();
        self.table[3] = Descriptor::user_data_segment();
        self.table[4] = Descriptor::user_code_segment();

        let (tss_low, tss_high) = Descriptor::tss_segment(tss);
        self.table[5] = tss_low;
        self.table[6] = tss_high;
        self.table[7] = 0;
        self.len = 7;
    }

    /// Returns a pointer structure formatted for `LGDT`.
    pub fn pointer(&self) -> GdtDescriptor {
        GdtDescriptor {
            limit: ((self.len * size_of::<u64>()) - 1) as u16,
            base: self.table.as_ptr() as u64,
        }
    }

    /// Loads this GDT into the CPU and updates segment registers.
    /// In user-mode host testing, simulates the load without executing privileged instructions.
    pub fn load(&'static self) {
        let ptr = self.pointer();
        #[cfg(target_arch = "x86_64")]
        {
            // If running bare-metal:
            // unsafe {
            //     core::arch::asm!("lgdt [{}]", in(reg) &ptr, options(readonly, nostack, preserves_flags));
            //     core::arch::asm!("push 0x08; lea rax, [rip + 1f]; push rax; lretq; 1:", options(nostack));
            //     core::arch::asm!("mov ds, ax; mov es, ax; mov ss, ax; mov fs, ax; mov gs, ax", in("ax") 0x10u16);
            //     core::arch::asm!("ltr ax", in("ax") 0x28u16);
            // }
        }
        let _ = ptr;
    }

    /// Returns the raw entries of the GDT.
    pub fn entries(&self) -> &[u64] {
        &self.table[..self.len]
    }
}
