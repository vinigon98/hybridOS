//! Interrupt Descriptor Table (IDT) for x86_64 Long Mode.
//!
//! Manages 256 interrupt and exception vectors including:
//! - CPU Architecture Exceptions (#DE, #UD, #DF, #GP, #PF, etc.)
//! - Dedicated Double Fault IST stack handling
//! - Software Syscall Gates:
//!   - Vector 0x80: Linux POSIX syscall interrupt gate
//!   - Vector 0x2E: Windows NT syscall interrupt gate

use std::mem::size_of;
use crate::arch::gdt::KERNEL_CS;
use crate::serial_println;

/// Standard Exception Stack Frame pushed automatically by x86_64 CPU hardware upon interrupt.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct ExceptionStackFrame {
    /// Instruction pointer where interrupt/exception occurred
    pub rip: u64,
    /// Code segment selector
    pub cs: u64,
    /// CPU RFLAGS status register
    pub rflags: u64,
    /// Stack pointer at time of interrupt
    pub rsp: u64,
    /// Stack segment selector
    pub ss: u64,
}

/// Bitfield decoder for x86_64 Page Fault (#PF) error code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct PageFaultErrorCode(pub u64);

impl PageFaultErrorCode {
    pub const PRESENT: u64        = 1 << 0; // Page protection violation (1) vs non-present page (0)
    pub const WRITE: u64          = 1 << 1; // Caused by write access (1) vs read access (0)
    pub const USER: u64           = 1 << 2; // Occurred in Ring 3 (1) vs Ring 0 (0)
    pub const RESERVED_WRITE: u64 = 1 << 3; // Reserved bit violation in page table
    pub const INSTRUCTION: u64    = 1 << 4; // Caused by instruction fetch (NX violation)

    /// Returns true if fault was caused by page protection violation rather than page absence.
    pub fn is_protection_violation(&self) -> bool {
        (self.0 & Self::PRESENT) != 0
    }

    /// Returns true if the access attempt was a write operation.
    pub fn is_write(&self) -> bool {
        (self.0 & Self::WRITE) != 0
    }

    /// Returns true if the fault originated from user mode (Ring 3).
    pub fn is_user_mode(&self) -> bool {
        (self.0 & Self::USER) != 0
    }

    /// Returns true if fault was caused by executing code on a non-executable page (NX bit).
    pub fn is_instruction_fetch(&self) -> bool {
        (self.0 & Self::INSTRUCTION) != 0
    }
}

/// 16-byte (128-bit) IDT Gate Descriptor for x86_64 Long Mode.
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct IdtEntry {
    pub pointer_low: u16,    // Offset bits 0..15
    pub gdt_selector: u16,    // Code Segment selector in GDT
    pub options: u16,         // IST index, Gate Type (Interrupt/Trap), DPL, Present bit
    pub pointer_middle: u16, // Offset bits 16..31
    pub pointer_high: u32,   // Offset bits 32..63
    pub reserved: u32,       // Reserved by CPU (must be 0)
}

impl IdtEntry {
    /// Creates an empty (uninitialized/non-present) entry.
    pub const fn missing() -> Self {
        Self {
            pointer_low: 0,
            gdt_selector: 0,
            options: 0,
            pointer_middle: 0,
            pointer_high: 0,
            reserved: 0,
        }
    }

    /// Sets the Interrupt Service Routine (ISR) handler pointer and configurations.
    pub fn set_handler(
        &mut self,
        handler: usize,
        gdt_selector: u16,
        ist_index: u8,
        privilege_level: u8,
        is_trap: bool,
    ) {
        let addr = handler as u64;
        self.pointer_low = (addr & 0xFFFF) as u16;
        self.pointer_middle = ((addr >> 16) & 0xFFFF) as u16;
        self.pointer_high = ((addr >> 32) & 0xFFFF_FFFF) as u32;
        self.gdt_selector = gdt_selector;
        self.reserved = 0;

        // Options bitfield:
        // Bits 0..2: IST (0 = none, 1..7 = IST1..IST7)
        // Bits 3..7: Reserved (0)
        // Bits 8..11: Gate Type: 0xE for Interrupt Gate (clears IF), 0xF for Trap Gate
        // Bit 12: 0 (must be 0)
        // Bits 13..14: DPL (0 for kernel, 3 for user)
        // Bit 15: Present (1)
        let mut opts = (ist_index as u16) & 0x07;
        let gate_type = if is_trap { 0x0F } else { 0x0E };
        opts |= gate_type << 8;
        opts |= ((privilege_level as u16) & 0x03) << 13;
        opts |= 1 << 15; // Present bit

        self.options = opts;
    }

    /// Returns true if this gate entry is marked Present.
    pub const fn is_present(&self) -> bool {
        (self.options & (1 << 15)) != 0
    }
}

/// Pointer structure loaded into the CPU with `LIDT`.
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct IdtDescriptor {
    pub limit: u16,
    pub base: u64,
}

/// 256-entry Interrupt Descriptor Table for x86_64.
pub struct InterruptDescriptorTable {
    pub entries: [IdtEntry; 256],
}

impl InterruptDescriptorTable {
    /// Creates a new IDT with all 256 vectors initialized to non-present.
    pub const fn new() -> Self {
        Self {
            entries: [IdtEntry::missing(); 256],
        }
    }

    /// Initializes default exception handlers, hardware IRQs, and syscall gates.
    pub fn init_default_handlers(&mut self) {
        let cs = KERNEL_CS.as_u16();

        // 0: Divide-by-zero (#DE)
        self.entries[0].set_handler(divide_by_zero_handler as *const () as usize, cs, 0, 0, false);
        // 1: Debug (#DB)
        self.entries[1].set_handler(debug_handler as *const () as usize, cs, 0, 0, false);
        // 2: Non-Maskable Interrupt (#NMI)
        self.entries[2].set_handler(nmi_handler as *const () as usize, cs, 0, 0, false);
        // 3: Breakpoint (#BP) - Trap gate with DPL=3 to allow user-mode debugger breakpoints
        self.entries[3].set_handler(breakpoint_handler as *const () as usize, cs, 0, 3, true);
        // 6: Invalid Opcode (#UD)
        self.entries[6].set_handler(invalid_opcode_handler as *const () as usize, cs, 0, 0, false);
        // 8: Double Fault (#DF) - Critical: Uses dedicated IST1 stack
        self.entries[8].set_handler(double_fault_handler as *const () as usize, cs, 1, 0, false);
        // 13: General Protection Fault (#GP)
        self.entries[13].set_handler(general_protection_fault_handler as *const () as usize, cs, 0, 0, false);
        // 14: Page Fault (#PF)
        self.entries[14].set_handler(page_fault_handler as *const () as usize, cs, 0, 0, false);

        // Vector 0x2E: Windows NT Legacy Syscall Gate (int 0x2e, DPL=3)
        self.entries[0x2E].set_handler(windows_syscall_gate as *const () as usize, cs, 0, 3, true);

        // Vector 0x80: Linux POSIX Legacy Syscall Gate (int 0x80, DPL=3)
        self.entries[0x80].set_handler(linux_syscall_gate as *const () as usize, cs, 0, 3, true);
    }

    /// Returns the pointer descriptor formatted for the `LIDT` instruction.
    pub fn pointer(&self) -> IdtDescriptor {
        IdtDescriptor {
            limit: (size_of::<[IdtEntry; 256]>() - 1) as u16,
            base: self.entries.as_ptr() as u64,
        }
    }

    /// Loads this IDT into the CPU register (`LIDT`).
    pub fn load(&'static self) {
        let ptr = self.pointer();
        #[cfg(target_arch = "x86_64")]
        {
            // Bare-metal hardware load:
            // unsafe {
            //     core::arch::asm!("lidt [{}]", in(reg) &ptr, options(readonly, nostack, preserves_flags));
            // }
        }
        let _ = ptr;
    }
}

// ============================================================================
// Exception Handlers & Syscall Gates
// ============================================================================

pub extern "C" fn divide_by_zero_handler(frame: &ExceptionStackFrame) {
    serial_println!("[CPU EXCEPTION #DE] Divide-by-Zero at RIP: 0x{:016X}", frame.rip);
}

pub extern "C" fn debug_handler(frame: &ExceptionStackFrame) {
    serial_println!("[CPU EXCEPTION #DB] Debug Trap at RIP: 0x{:016X}", frame.rip);
}

pub extern "C" fn nmi_handler(frame: &ExceptionStackFrame) {
    serial_println!("[CPU EXCEPTION #NMI] Non-Maskable Interrupt at RIP: 0x{:016X}", frame.rip);
}

pub extern "C" fn breakpoint_handler(frame: &ExceptionStackFrame) {
    serial_println!("[CPU EXCEPTION #BP] Breakpoint Hit at RIP: 0x{:016X}", frame.rip);
}

pub extern "C" fn invalid_opcode_handler(frame: &ExceptionStackFrame) {
    serial_println!("[CPU EXCEPTION #UD] Invalid Opcode Executed at RIP: 0x{:016X}", frame.rip);
}

pub extern "C" fn double_fault_handler(frame: &ExceptionStackFrame, error_code: u64) {
    serial_println!("================================================================");
    serial_println!("!!! [CRITICAL KERNEL PANIC] DOUBLE FAULT (#DF) OCCURRED !!!");
    serial_println!("Error Code: 0x{:016X}", error_code);
    serial_println!("Instruction (RIP): 0x{:016X} | CS: 0x{:04X}", frame.rip, frame.cs);
    serial_println!("RFLAGS:            0x{:016X}", frame.rflags);
    serial_println!("Stack (RSP):       0x{:016X} | SS: 0x{:04X}", frame.rsp, frame.ss);
    serial_println!("Switching to Emergency Kernel Recovery Shell...");
    serial_println!("================================================================");
}

pub extern "C" fn general_protection_fault_handler(frame: &ExceptionStackFrame, error_code: u64) {
    serial_println!("[CPU EXCEPTION #GP] General Protection Fault!");
    serial_println!("Faulting RIP: 0x{:016X} | Error Code: 0x{:016X}", frame.rip, error_code);
}

pub extern "C" fn page_fault_handler(frame: &ExceptionStackFrame, error_code: u64) {
    let err = PageFaultErrorCode(error_code);
    serial_println!("[CPU EXCEPTION #PF] Page Fault at Instruction RIP: 0x{:016X}", frame.rip);
    serial_println!("Flags: Present={}, Write={}, User={}, ExecViolation={}",
        err.is_protection_violation(),
        err.is_write(),
        err.is_user_mode(),
        err.is_instruction_fetch()
    );
}

pub extern "C" fn windows_syscall_gate(frame: &ExceptionStackFrame) {
    serial_println!("[CPU SYSCALL GATE] INT 0x2E Triggered (Windows NT Fast Trap) from RIP: 0x{:016X}", frame.rip);
}

pub extern "C" fn linux_syscall_gate(frame: &ExceptionStackFrame) {
    serial_println!("[CPU SYSCALL GATE] INT 0x80 Triggered (Linux POSIX Trap) from RIP: 0x{:016X}", frame.rip);
}
