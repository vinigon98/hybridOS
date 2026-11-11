//! Hardware CPU Model-Specific Registers (MSRs), Control Registers, and Port I/O.
//!
//! Provides bare-metal Ring 0 instructions for:
//! - CPU Port I/O: inb/outb, inw/outw, inl/outl
//! - MSR manipulation: rdmsr/wrmsr for fast hardware SYSCALL/SYSRET
//! - Control Registers: CR0, CR2, CR3 (PML4 paging base), CR4

use core::arch::asm;

// MSR Addresses for x86_64 Architecture
pub const IA32_EFER: u32        = 0xC000_0080;
pub const IA32_STAR: u32        = 0xC000_0081;
pub const IA32_LSTAR: u32       = 0xC000_0082;
pub const IA32_CSTAR: u32       = 0xC000_0083;
pub const IA32_SFMASK: u32      = 0xC000_0084;
pub const IA32_FS_BASE: u32     = 0xC000_0100;
pub const IA32_GS_BASE: u32     = 0xC000_0101;
pub const IA32_KERNEL_GS_BASE: u32 = 0xC000_0102;

// EFER Bits
pub const EFER_SCE: u64 = 1 << 0; // System Call Extensions (enables SYSCALL/SYSRET)
pub const EFER_LME: u64 = 1 << 8; // Long Mode Enable
pub const EFER_LMA: u64 = 1 << 10; // Long Mode Active
pub const EFER_NXE: u64 = 1 << 11; // No-Execute Enable

// RFLAGS Bits for SFMASK (masks interrupts and traps upon SYSCALL entry)
pub const RFLAGS_IF: u64 = 1 << 9;  // Interrupt Flag
pub const RFLAGS_TF: u64 = 1 << 8;  // Trap Flag
pub const RFLAGS_DF: u64 = 1 << 10; // Direction Flag

// --- Port I/O Instructions ---

#[inline]
pub unsafe fn outb(port: u16, val: u8) {
    asm!("out dx, al", in("dx") port, in("al") val, options(nomem, nostack, preserves_flags));
}

#[inline]
pub unsafe fn inb(port: u16) -> u8 {
    let val: u8;
    asm!("in al, dx", out("al") val, in("dx") port, options(nomem, nostack, preserves_flags));
    val
}

#[inline]
pub unsafe fn outw(port: u16, val: u16) {
    asm!("out dx, ax", in("dx") port, in("ax") val, options(nomem, nostack, preserves_flags));
}

#[inline]
pub unsafe fn inw(port: u16) -> u16 {
    let val: u16;
    asm!("in ax, dx", out("ax") val, in("dx") port, options(nomem, nostack, preserves_flags));
    val
}

#[inline]
pub unsafe fn outl(port: u16, val: u32) {
    asm!("out dx, eax", in("dx") port, in("eax") val, options(nomem, nostack, preserves_flags));
}

#[inline]
pub unsafe fn inl(port: u16) -> u32 {
    let val: u32;
    asm!("in eax, dx", out("eax") val, in("dx") port, options(nomem, nostack, preserves_flags));
    val
}

#[inline]
pub unsafe fn io_wait() {
    outb(0x80, 0);
}

// --- MSR Instructions ---

#[inline]
pub unsafe fn rdmsr(msr: u32) -> u64 {
    let low: u32;
    let high: u32;
    asm!("rdmsr", in("ecx") msr, out("eax") low, out("edx") high, options(nomem, nostack, preserves_flags));
    ((high as u64) << 32) | (low as u64)
}

#[inline]
pub unsafe fn wrmsr(msr: u32, value: u64) {
    let low = value as u32;
    let high = (value >> 32) as u32;
    asm!("wrmsr", in("ecx") msr, in("eax") low, in("edx") high, options(nomem, nostack, preserves_flags));
}

// --- Control Registers ---

#[inline]
pub unsafe fn read_cr0() -> u64 {
    let cr0: u64;
    asm!("mov {}, cr0", out(reg) cr0, options(nomem, nostack, preserves_flags));
    cr0
}

#[inline]
pub unsafe fn write_cr0(val: u64) {
    asm!("mov cr0, {}", in(reg) val, options(nomem, nostack, preserves_flags));
}

#[inline]
pub unsafe fn read_cr2() -> u64 {
    let cr2: u64;
    asm!("mov {}, cr2", out(reg) cr2, options(nomem, nostack, preserves_flags));
    cr2
}

#[inline]
pub unsafe fn read_cr3() -> u64 {
    let cr3: u64;
    asm!("mov {}, cr3", out(reg) cr3, options(nomem, nostack, preserves_flags));
    cr3
}

#[inline]
pub unsafe fn write_cr3(val: u64) {
    asm!("mov cr3, {}", in(reg) val, options(nomem, nostack, preserves_flags));
}

#[inline]
pub unsafe fn read_cr4() -> u64 {
    let cr4: u64;
    asm!("mov {}, cr4", out(reg) cr4, options(nomem, nostack, preserves_flags));
    cr4
}

#[inline]
pub unsafe fn write_cr4(val: u64) {
    asm!("mov cr4, {}", in(reg) val, options(nomem, nostack, preserves_flags));
}

/// Programs the x86_64 CPU hardware MSRs for fast SYSCALL / SYSRET instructions.
///
/// Configures:
/// - STAR: Kernel CS/SS (0x08/0x10) in bits 32..47, User CS/SS (0x1B/0x23) in bits 48..63.
/// - LSTAR: 64-bit rip target of the kernel syscall trampoline.
/// - SFMASK: RFLAGS mask (disables IF and TF during syscall entry).
/// - EFER: Ensures SCE (System Call Extensions) is enabled.
pub unsafe fn init_syscall_msrs(syscall_entry_rip: u64) {
    // 1. Enable SCE in EFER
    let efer = rdmsr(IA32_EFER);
    wrmsr(IA32_EFER, efer | EFER_SCE | EFER_NXE);

    // 2. Setup STAR:
    // Bits [47:32]: Kernel CS (0x08) -> SS is CS + 8 (0x10)
    // Bits [63:48]: User CS base for SYSRET:
    // For 64-bit sysretq: target CS is (base + 16) | 3 = 0x23, target SS is (base + 8) | 3 = 0x1B.
    // So base is set to 0x13 (which yields CS=0x23, SS=0x1B).
    let star: u64 = ((0x0013u64) << 48) | ((0x0008u64) << 32);
    wrmsr(IA32_STAR, star);

    // 3. Setup LSTAR: Target address for 64-bit SYSCALL
    wrmsr(IA32_LSTAR, syscall_entry_rip);

    // 4. Setup SFMASK: Mask interrupts (IF) and debug trap (TF) on syscall entry
    wrmsr(IA32_SFMASK, RFLAGS_IF | RFLAGS_TF | RFLAGS_DF);
}
