//! Architecture & Hardware Core Module for x86_64.
//!
//! Exposes:
//! - Hardware Port I/O and MSR management (`arch::msr`)
//! - Serial COM1 UART driver for kernel diagnostic logging (`arch::serial`)
//! - Global Descriptor Table (GDT) & Task State Segment (TSS) (`arch::gdt`)
//! - Interrupt Descriptor Table (IDT) with exception handlers and syscall gates (`arch::idt`)
//! - Central hardware architecture initialization: `init_arch()`

#[allow(unused_imports)]
use alloc::string::{String, ToString};
#[allow(unused_imports)]
use alloc::vec::Vec;
#[allow(unused_imports)]
use alloc::boxed::Box;
#[allow(unused_imports)]
use crate::compat::FloatExt;
pub mod msr;
pub mod serial;
pub mod gdt;
pub mod idt;
pub mod percpu;
pub mod usermode;
pub mod acpi;
pub mod apic;
pub mod smp;

pub use self::serial::{SerialPort, COM1, COM1_BASE, COM2_BASE};
pub use self::gdt::{
    GlobalDescriptorTable, TaskStateSegment, SegmentSelector,
    KERNEL_CS, KERNEL_DS, USER_CS, USER_DS, TSS_SEL,
};
pub use self::idt::{InterruptDescriptorTable, InterruptContext, PageFaultErrorCode};
pub use self::percpu::{PerCpuData, init_percpu, get_bsp_cpu_data, update_current_cpu_task};
pub use self::usermode::enter_user_mode;
pub type ExceptionStackFrame = InterruptContext;

use crate::sync::OnceLock;

static ARCH_STATUS: OnceLock<ArchStatus> = OnceLock::new();

// Static buffers for GDT, TSS, IDT, and Emergency Double Fault IST1 stack
static mut DOUBLE_FAULT_STACK: [u8; 16384] = [0u8; 16384];
static mut TSS_INSTANCE: TaskStateSegment = TaskStateSegment::new();
static mut GDT_INSTANCE: GlobalDescriptorTable = GlobalDescriptorTable::new();
static mut IDT_INSTANCE: InterruptDescriptorTable = InterruptDescriptorTable::new();

/// Diagnostic status record for the architecture core subsystem.
#[derive(Debug, Clone, Copy)]
pub struct ArchStatus {
    pub serial_ready: bool,
    pub gdt_loaded: bool,
    pub idt_loaded: bool,
    pub tss_configured: bool,
}

/// Initializes the x86_64 hardware architecture and low-level subsystem:
/// 1. Initializes COM1 UART serial controller at 0x3F8 (115200 baud, 8N1).
/// 2. Configures Task State Segment (TSS) with dedicated Double Fault IST1 stack.
/// 3. Builds and loads the Global Descriptor Table (GDT) with Kernel/User code/data segments.
/// 4. Builds and loads the Interrupt Descriptor Table (IDT) with 256 vectors and syscall gates.
pub fn init_arch() -> ArchStatus {
    if let Some(status) = ARCH_STATUS.get() {
        return *status;
    }

    // Phase 1: Initialize Serial COM1 driver
    let mut serial_ready = false;
    let mut serial = COM1.lock();
    if serial.init().is_ok() {
        serial_ready = true;
    }
    drop(serial);

    crate::serial_println!("================================================================");
    crate::serial_println!(" [ARCH] Initializing HybridOS x86_64 Hardware Core Subsystem...");
    crate::serial_println!("================================================================");
    crate::serial_println!("[ARCH] Serial COM1 (port 0x3F8) initialized @ 115200 baud, 8N1.");

    unsafe {
        // Phase 2: Configure Task State Segment (TSS) and Double Fault Stack (16 KiB)
        let stack_top = DOUBLE_FAULT_STACK.as_ptr() as u64 + DOUBLE_FAULT_STACK.len() as u64;
        TSS_INSTANCE.set_ist(1, stack_top);
        crate::serial_println!("[ARCH] TSS initialized (IST1 Double Fault emergency stack configured @ 0x{:016X}).", stack_top);

        // Phase 3: Setup and Load Global Descriptor Table (GDT)
        GDT_INSTANCE.init_standard(&TSS_INSTANCE);
        GDT_INSTANCE.load();

        crate::serial_println!("[ARCH] GDT loaded into CPU (LGDT):");
        crate::serial_println!("       - Index 1: 0x08 -> Kernel Code 64 (Ring 0)");
        crate::serial_println!("       - Index 2: 0x10 -> Kernel Data 64 (Ring 0)");
        crate::serial_println!("       - Index 3: 0x1B -> User Data 64   (Ring 3)");
        crate::serial_println!("       - Index 4: 0x23 -> User Code 64   (Ring 3)");
        crate::serial_println!("       - Index 5: 0x28 -> Task State Segment (TSS)");

        // Phase 4: Setup and Load Interrupt Descriptor Table (IDT)
        IDT_INSTANCE.init_default_handlers();
        IDT_INSTANCE.load();

        // Phase 5: Remap PIC and Initialize Hardware Timer (PIT @ 100Hz)
        idt::init_pic_and_pit();

        // Phase 6: Initialize Hardware Per-CPU Data Block & GS MSRs
        init_percpu(0);

        crate::serial_println!("[ARCH] IDT loaded into CPU (LIDT) with 256 vectors:");
        crate::serial_println!("       - Exception Vectors (#DE, #DB, #NMI, #BP, #UD, #DF, #GP, #PF)");
        crate::serial_println!("       - Hardware IRQ0 Timer (PIT 100Hz) & IRQ1 Keyboard online");
        crate::serial_println!("       - Windows NT Fast Syscall Trap Gate (INT 0x2E, DPL 3)");
        crate::serial_println!("       - Linux POSIX Syscall Trap Gate (INT 0x80, DPL 3)");
    }

    let status = ArchStatus {
        serial_ready,
        gdt_loaded: true,
        idt_loaded: true,
        tss_configured: true,
    };

    crate::serial_println!("[ARCH] x86_64 Architecture initialization complete.\n");

    let _ = ARCH_STATUS.set(status);
    status
}

/// Loads the kernel IDT into the calling CPU core (LIDT).
pub unsafe fn load_kernel_idt() {
    IDT_INSTANCE.load();
}

/// Loads the kernel GDT into the calling CPU core (LGDT).
pub unsafe fn load_kernel_gdt() {
    GDT_INSTANCE.load();
}

/// Configures the TSS RSP0 privilege-level stack for Ring 3 -> Ring 0 transitions.
pub fn set_kernel_stack(stack_top: u64) {
    unsafe {
        TSS_INSTANCE.set_rsp0(stack_top);
    }
}

/// Returns whether the architecture module has been successfully initialized.
pub fn is_arch_initialized() -> bool {
    ARCH_STATUS.get().is_some()
}
