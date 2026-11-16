//! Interrupt Descriptor Table (IDT) for x86_64 Long Mode.
//!
//! Manages 256 interrupt and exception vectors with real assembly stubs,
//! full register preservation, proper CPU error code handling, CR2 reading on #PF,
//! hardware timer (PIT/PIC), and real return via `iretq`.

use core::mem::size_of;
use core::sync::atomic::{AtomicU64, Ordering};
use crate::arch::gdt::KERNEL_CS;
use crate::serial_println;

/// Global system timer tick counter (100 Hz, 10ms per tick).
pub static SYSTEM_TICKS: AtomicU64 = AtomicU64::new(0);

/// Full processor context saved on the kernel stack during an interrupt or exception.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct InterruptContext {
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub r11: u64,
    pub r10: u64,
    pub r9: u64,
    pub r8: u64,
    pub rbp: u64,
    pub rdi: u64,
    pub rsi: u64,
    pub rdx: u64,
    pub rcx: u64,
    pub rbx: u64,
    pub rax: u64,
    pub vector: u64,
    pub error_code: u64,
    // Hardware frame pushed automatically by the x86_64 CPU
    pub rip: u64,
    pub cs: u64,
    pub rflags: u64,
    pub rsp: u64,
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

    #[inline]
    pub fn is_protection_violation(&self) -> bool {
        (self.0 & Self::PRESENT) != 0
    }

    #[inline]
    pub fn is_write(&self) -> bool {
        (self.0 & Self::WRITE) != 0
    }

    #[inline]
    pub fn is_user_mode(&self) -> bool {
        (self.0 & Self::USER) != 0
    }

    #[inline]
    pub fn is_instruction_fetch(&self) -> bool {
        (self.0 & Self::INSTRUCTION) != 0
    }
}

/// 16-byte (128-bit) IDT Gate Descriptor for x86_64 Long Mode.
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct IdtEntry {
    pub pointer_low: u16,    // Offset bits 0..15
    pub gdt_selector: u16,   // Code Segment selector in GDT
    pub options: u16,        // IST index, Gate Type (Interrupt/Trap), DPL, Present bit
    pub pointer_middle: u16, // Offset bits 16..31
    pub pointer_high: u32,   // Offset bits 32..63
    pub reserved: u32,       // Reserved by CPU (must be 0)
}

impl IdtEntry {
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

        let mut opts = (ist_index as u16) & 0x07;
        let gate_type = if is_trap { 0x0F } else { 0x0E };
        opts |= gate_type << 8;
        opts |= ((privilege_level as u16) & 0x03) << 13;
        opts |= 1 << 15; // Present bit

        self.options = opts;
    }

    #[inline]
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
    pub const fn new() -> Self {
        Self {
            entries: [IdtEntry::missing(); 256],
        }
    }

    /// Initializes hardware exception stubs, timer IRQ, and syscall gates.
    pub fn init_default_handlers(&mut self) {
        let cs = KERNEL_CS.as_u16();

        // CPU Exceptions (0..21)
        self.entries[0].set_handler(isr_stub_0 as *const () as usize, cs, 0, 0, false);
        self.entries[1].set_handler(isr_stub_1 as *const () as usize, cs, 0, 0, false);
        self.entries[2].set_handler(isr_stub_2 as *const () as usize, cs, 0, 0, false);
        self.entries[3].set_handler(isr_stub_3 as *const () as usize, cs, 0, 3, true); // Breakpoint (DPL=3)
        self.entries[4].set_handler(isr_stub_4 as *const () as usize, cs, 0, 0, false);
        self.entries[5].set_handler(isr_stub_5 as *const () as usize, cs, 0, 0, false);
        self.entries[6].set_handler(isr_stub_6 as *const () as usize, cs, 0, 0, false);
        self.entries[7].set_handler(isr_stub_7 as *const () as usize, cs, 0, 0, false);
        self.entries[8].set_handler(isr_stub_8 as *const () as usize, cs, 1, 0, false); // Double Fault (IST=1)
        self.entries[9].set_handler(isr_stub_9 as *const () as usize, cs, 0, 0, false);
        self.entries[10].set_handler(isr_stub_10 as *const () as usize, cs, 0, 0, false);
        self.entries[11].set_handler(isr_stub_11 as *const () as usize, cs, 0, 0, false);
        self.entries[12].set_handler(isr_stub_12 as *const () as usize, cs, 0, 0, false);
        self.entries[13].set_handler(isr_stub_13 as *const () as usize, cs, 0, 0, false); // GP Fault
        self.entries[14].set_handler(isr_stub_14 as *const () as usize, cs, 0, 0, false); // Page Fault
        self.entries[15].set_handler(isr_stub_15 as *const () as usize, cs, 0, 0, false);
        self.entries[16].set_handler(isr_stub_16 as *const () as usize, cs, 0, 0, false);
        self.entries[17].set_handler(isr_stub_17 as *const () as usize, cs, 0, 0, false);
        self.entries[18].set_handler(isr_stub_18 as *const () as usize, cs, 0, 0, false);
        self.entries[19].set_handler(isr_stub_19 as *const () as usize, cs, 0, 0, false);
        self.entries[20].set_handler(isr_stub_20 as *const () as usize, cs, 0, 0, false);
        self.entries[21].set_handler(isr_stub_21 as *const () as usize, cs, 0, 0, false);

        // Hardware IRQs (re-mapped PIC)
        self.entries[32].set_handler(isr_stub_32 as *const () as usize, cs, 0, 0, false); // IRQ0: Timer
        self.entries[33].set_handler(isr_stub_33 as *const () as usize, cs, 0, 0, false); // IRQ1: PS/2 Keyboard

        // Legacy Syscall Software Gates (DPL=3 so Ring 3 can invoke them)
        self.entries[0x2E].set_handler(isr_stub_46 as *const () as usize, cs, 0, 3, true);  // Windows NT
        self.entries[0x80].set_handler(isr_stub_128 as *const () as usize, cs, 0, 3, true); // Linux POSIX
    }

    pub fn pointer(&self) -> IdtDescriptor {
        IdtDescriptor {
            limit: (size_of::<[IdtEntry; 256]>() - 1) as u16,
            base: self.entries.as_ptr() as u64,
        }
    }

    pub fn load(&'static self) {
        let ptr = self.pointer();
        unsafe {
            core::arch::asm!("lidt [{}]", in(reg) &ptr, options(readonly, nostack, preserves_flags));
        }
    }
}

// ============================================================================
// Assembly Stubs and Real Hardware Return via IRETQ
// ============================================================================

core::arch::global_asm!(r#"
.macro ISR_NOERRCODE vec
.global isr_stub_\vec
isr_stub_\vec:
    push 0          /* dummy error code */
    push \vec       /* vector number */
    jmp common_isr_entry
.endm

.macro ISR_ERRCODE vec
.global isr_stub_\vec
isr_stub_\vec:
    /* CPU already pushed error code */
    push \vec       /* vector number */
    jmp common_isr_entry
.endm

common_isr_entry:
    /* Push general purpose registers in order */
    push rax
    push rbx
    push rcx
    push rdx
    push rsi
    push rdi
    push rbp
    push r8
    push r9
    push r10
    push r11
    push r12
    push r13
    push r14
    push r15

    /* First argument in RDI (System V AMD64 ABI): pointer to InterruptContext */
    mov rdi, rsp
    cld
    call idt_dispatch_interrupt

    /* Restore general purpose registers */
    pop r15
    pop r14
    pop r13
    pop r12
    pop r11
    pop r10
    pop r9
    pop r8
    pop rbp
    pop rdi
    pop rsi
    pop rdx
    pop rcx
    pop rbx
    pop rax

    /* Discard vector and error code from stack */
    add rsp, 16

    /* Real x86_64 hardware interrupt return */
    iretq

/* Stubs for exceptions */
ISR_NOERRCODE 0
ISR_NOERRCODE 1
ISR_NOERRCODE 2
ISR_NOERRCODE 3
ISR_NOERRCODE 4
ISR_NOERRCODE 5
ISR_NOERRCODE 6
ISR_NOERRCODE 7
ISR_ERRCODE   8
ISR_NOERRCODE 9
ISR_ERRCODE   10
ISR_ERRCODE   11
ISR_ERRCODE   12
ISR_ERRCODE   13
ISR_ERRCODE   14
ISR_NOERRCODE 15
ISR_NOERRCODE 16
ISR_ERRCODE   17
ISR_NOERRCODE 18
ISR_NOERRCODE 19
ISR_NOERRCODE 20
ISR_ERRCODE   21

/* Hardware IRQ stubs */
ISR_NOERRCODE 32
ISR_NOERRCODE 33
ISR_NOERRCODE 46
ISR_NOERRCODE 128
"#);

extern "C" {
    fn isr_stub_0();
    fn isr_stub_1();
    fn isr_stub_2();
    fn isr_stub_3();
    fn isr_stub_4();
    fn isr_stub_5();
    fn isr_stub_6();
    fn isr_stub_7();
    fn isr_stub_8();
    fn isr_stub_9();
    fn isr_stub_10();
    fn isr_stub_11();
    fn isr_stub_12();
    fn isr_stub_13();
    fn isr_stub_14();
    fn isr_stub_15();
    fn isr_stub_16();
    fn isr_stub_17();
    fn isr_stub_18();
    fn isr_stub_19();
    fn isr_stub_20();
    fn isr_stub_21();
    fn isr_stub_32();
    fn isr_stub_33();
    fn isr_stub_46();
    fn isr_stub_128();
}

/// Central interrupt dispatcher invoked by `common_isr_entry`.
#[no_mangle]
pub extern "C" fn idt_dispatch_interrupt(ctx: &mut InterruptContext) {
    match ctx.vector {
        0 => serial_println!("[CPU EXCEPTION #DE] Divide-by-Zero at RIP: 0x{:016X}", ctx.rip),
        1 => serial_println!("[CPU EXCEPTION #DB] Debug Trap at RIP: 0x{:016X}", ctx.rip),
        2 => serial_println!("[CPU EXCEPTION #NMI] Non-Maskable Interrupt at RIP: 0x{:016X}", ctx.rip),
        3 => serial_println!("[CPU EXCEPTION #BP] Breakpoint Hit at RIP: 0x{:016X}", ctx.rip),
        6 => serial_println!("[CPU EXCEPTION #UD] Invalid Opcode at RIP: 0x{:016X}", ctx.rip),
        8 => {
            serial_println!("================================================================");
            serial_println!("!!! [CRITICAL KERNEL PANIC] DOUBLE FAULT (#DF) !!!");
            serial_println!("Error Code:        0x{:016X}", ctx.error_code);
            serial_println!("Instruction (RIP): 0x{:016X} | CS: 0x{:04X}", ctx.rip, ctx.cs);
            serial_println!("RFLAGS:            0x{:016X}", ctx.rflags);
            serial_println!("Stack (RSP):       0x{:016X} | SS: 0x{:04X}", ctx.rsp, ctx.ss);
            serial_println!("================================================================");
            loop {
                unsafe { core::arch::asm!("cli; hlt"); }
            }
        }
        13 => {
            serial_println!("[CPU EXCEPTION #GP] General Protection Fault! Error Code: 0x{:016X}, RIP: 0x{:016X}", ctx.error_code, ctx.rip);
            loop {
                unsafe { core::arch::asm!("cli; hlt"); }
            }
        }
        14 => {
            // Read CR2 immediately to capture the exact faulting linear address
            let faulting_addr: u64;
            unsafe {
                core::arch::asm!("mov {}, cr2", out(reg) faulting_addr, options(nomem, nostack, preserves_flags));
            }
            let hhdm = crate::boot::get_hhdm_offset();
            let resolved = unsafe {
                crate::mm::cow::handle_page_fault(
                    crate::mm::paging::VirtAddr::new_truncate(faulting_addr),
                    ctx.error_code,
                    hhdm,
                )
            };
            if resolved {
                return; // Return via IRETQ to re-execute faulting instruction!
            }

            let err = PageFaultErrorCode(ctx.error_code);
            let pf_gs = unsafe { crate::arch::msr::rdmsr(crate::arch::msr::IA32_GS_BASE) };
            let pf_kgs = unsafe { crate::arch::msr::rdmsr(crate::arch::msr::IA32_KERNEL_GS_BASE) };
            serial_println!("[CPU EXCEPTION #PF] Page Fault at Linear Address: 0x{:016X} (Faulting RIP: 0x{:016X})", faulting_addr, ctx.rip);
            serial_println!("      ProtectionViolation={}, Write={}, UserMode={}, InstructionFetch={}",
                err.is_protection_violation(),
                err.is_write(),
                err.is_user_mode(),
                err.is_instruction_fetch()
            );
            serial_println!("      [#PF MSR STATE] GS_BASE=0x{:016X}, KERNEL_GS_BASE=0x{:016X}", pf_gs, pf_kgs);
            loop {
                unsafe { core::arch::asm!("cli; hlt"); }
            }
        }
        32 => {
            // IRQ0: Hardware Timer Tick
            SYSTEM_TICKS.fetch_add(1, Ordering::Relaxed); let _ = crate::sched::tick();
            unsafe {
                // Acknowledge Master PIC
                crate::arch::msr::outb(0x20, 0x20);
            }
        }
        33 => {
            // IRQ1: PS/2 Keyboard Controller
            let _ = crate::drivers::poll_ps2_controller();
            unsafe {
                crate::arch::msr::outb(0x20, 0x20);
            }
        }
        44 => {
            // IRQ12: PS/2 Mouse Controller
            let _ = crate::drivers::poll_ps2_controller();
            unsafe {
                crate::arch::msr::outb(0xA0, 0x20);
                crate::arch::msr::outb(0x20, 0x20);
            }
        }
        0x2E => {
            serial_println!("[CPU SYSCALL GATE] INT 0x2E Triggered (Windows NT Fast Trap) from RIP: 0x{:016X}", ctx.rip);
        }
        0x80 => {
            serial_println!("[CPU SYSCALL GATE] INT 0x80 Triggered (Linux POSIX Trap) from RIP: 0x{:016X}", ctx.rip);
        }
        other => {
            serial_println!("[CPU INTERRUPT] Vector {} (Error: 0x{:X}) at RIP: 0x{:016X}", other, ctx.error_code, ctx.rip);
            if other >= 32 && other < 48 {
                unsafe {
                    if other >= 40 {
                        crate::arch::msr::outb(0xA0, 0x20);
                    }
                    crate::arch::msr::outb(0x20, 0x20);
                }
            }
        }
    }
}

/// Remaps 8259 PIC vectors to 0x20-0x2F and sets up PIT Channel 0 at 100 Hz.
pub fn init_pic_and_pit() {
    unsafe {
        // ICW1: Start initialization
        crate::arch::msr::outb(0x20, 0x11);
        crate::arch::msr::io_wait();
        crate::arch::msr::outb(0xA0, 0x11);
        crate::arch::msr::io_wait();

        // ICW2: Vector offset 0x20 for Master PIC (IRQs 0..7), 0x28 for Slave PIC (IRQs 8..15)
        crate::arch::msr::outb(0x21, 0x20);
        crate::arch::msr::io_wait();
        crate::arch::msr::outb(0xA1, 0x28);
        crate::arch::msr::io_wait();

        // ICW3: Cascading setup
        crate::arch::msr::outb(0x21, 0x04);
        crate::arch::msr::io_wait();
        crate::arch::msr::outb(0xA1, 0x02);
        crate::arch::msr::io_wait();

        // ICW4: 8086/88 (MCS-80/85) mode
        crate::arch::msr::outb(0x21, 0x01);
        crate::arch::msr::io_wait();
        crate::arch::msr::outb(0xA1, 0x01);
        crate::arch::msr::io_wait();

        // Program PIT Channel 0 for 100 Hz timer ticks (1193182 / 100 = 11932)
        let divisor: u16 = 11932;
        crate::arch::msr::outb(0x43, 0x36); // Channel 0, lobyte/hibyte, rate generator (mode 3)
        crate::arch::msr::outb(0x40, (divisor & 0xFF) as u8);
        crate::arch::msr::outb(0x40, (divisor >> 8) as u8);

        // Unmask IRQ0 (Timer, bit 0) and IRQ1 (Keyboard, bit 1) on Master PIC
        crate::arch::msr::outb(0x21, 0xFC);
        // Mask all on Slave PIC
        crate::arch::msr::outb(0xA1, 0xFF);
    }
}
