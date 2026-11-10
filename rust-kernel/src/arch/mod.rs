//! Architecture & Hardware Core Module for x86_64.
//!
//! Exposes:
//! - Serial COM1 UART driver for kernel diagnostic logging
//! - Global Descriptor Table (GDT) & Task State Segment (TSS)
//! - Interrupt Descriptor Table (IDT) with exception handlers and syscall gates
//! - Central hardware architecture initialization: `init_arch()`

pub mod serial;
pub mod gdt;
pub mod idt;

pub use self::serial::{SerialPort, COM1, COM1_BASE, COM2_BASE};
pub use self::gdt::{
    GlobalDescriptorTable, TaskStateSegment, SegmentSelector,
    KERNEL_CS, KERNEL_DS, USER_CS, USER_DS, TSS_SEL,
};
pub use self::idt::{InterruptDescriptorTable, ExceptionStackFrame, PageFaultErrorCode};

use std::sync::OnceLock;

static ARCH_STATUS: OnceLock<ArchStatus> = OnceLock::new();

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
    if let Ok(mut serial) = COM1.lock() {
        if serial.init().is_ok() {
            serial_ready = true;
        }
    }

    crate::serial_println!("================================================================");
    crate::serial_println!(" [ARCH] Initializing HybridOS x86_64 Hardware Core Subsystem...");
    crate::serial_println!("================================================================");
    crate::serial_println!("[ARCH] Serial COM1 (port 0x3F8) initialized @ 115200 baud, 8N1.");

    // Phase 2: Configure Task State Segment (TSS) and Double Fault Stack (16 KiB)
    let double_fault_stack: &'static mut [u8; 16384] = Box::leak(Box::new([0u8; 16384]));
    let stack_top = double_fault_stack.as_ptr() as u64 + double_fault_stack.len() as u64;

    let tss: &'static mut TaskStateSegment = Box::leak(Box::new(TaskStateSegment::new()));
    tss.set_ist(1, stack_top);
    crate::serial_println!("[ARCH] TSS initialized (IST1 Double Fault emergency stack configured @ 0x{:016X}).", stack_top);

    // Phase 3: Setup and Load Global Descriptor Table (GDT)
    let gdt: &'static mut GlobalDescriptorTable = Box::leak(Box::new(GlobalDescriptorTable::new()));
    gdt.init_standard(tss);
    gdt.load();

    crate::serial_println!("[ARCH] GDT loaded successfully:");
    crate::serial_println!("       - Index 1: 0x08 -> Kernel Code 64 (Ring 0)");
    crate::serial_println!("       - Index 2: 0x10 -> Kernel Data 64 (Ring 0)");
    crate::serial_println!("       - Index 3: 0x1B -> User Data 64   (Ring 3)");
    crate::serial_println!("       - Index 4: 0x23 -> User Code 64   (Ring 3)");
    crate::serial_println!("       - Index 5: 0x28 -> Task State Segment (TSS)");

    // Phase 4: Setup and Load Interrupt Descriptor Table (IDT)
    let idt: &'static mut InterruptDescriptorTable = Box::leak(Box::new(InterruptDescriptorTable::new()));
    idt.init_default_handlers();
    idt.load();

    crate::serial_println!("[ARCH] IDT loaded with 256 vectors:");
    crate::serial_println!("       - Exception Vectors (#DE, #DB, #NMI, #BP, #UD, #DF, #GP, #PF)");
    crate::serial_println!("       - Windows NT Fast Syscall Trap Gate (INT 0x2E, DPL 3)");
    crate::serial_println!("       - Linux POSIX Syscall Trap Gate (INT 0x80, DPL 3)");

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

/// Returns whether the architecture module has been successfully initialized.
pub fn is_arch_initialized() -> bool {
    ARCH_STATUS.get().is_some()
}
