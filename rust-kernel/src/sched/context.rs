//! Architectural x86_64 Context Switching and State Preservation.
//!
//! Implements low-level register preservation, CR3 address space switching,
//! FS_BASE (Linux TLS) / GS_BASE (Windows NT TEB) manipulation, and stack transitions.

use core::arch::global_asm;
use crate::sched::task::CpuContext;

extern "C" {
    /// Performs an atomic hardware context switch from `prev_ctx` to `next_ctx`.
    ///
    /// Saves all callee-saved registers, return RIP, stack pointer, RFLAGS,
    /// FS_BASE, and KERNEL_GS_BASE to `prev_ctx`.
    /// Restores registers, swaps CR3 if distinct, and resumes execution at `next_ctx.rip`.
    pub fn switch_to(prev_ctx: *mut CpuContext, next_ctx: *const CpuContext);
}

global_asm!(
    r#"
.global switch_to
switch_to:
    # ------------------------------------------------------------
    # 1. Save execution state of prev_ctx (%rdi)
    # ------------------------------------------------------------
    # Callee-saved general-purpose registers
    mov [rdi + 0x08], rbx
    mov [rdi + 0x30], rbp
    mov [rdi + 0x60], r12
    mov [rdi + 0x68], r13
    mov [rdi + 0x70], r14
    mov [rdi + 0x78], r15

    # Return instruction pointer (currently on top of the stack)
    mov rax, [rsp]
    mov [rdi + 0x80], rax

    # Caller stack pointer (prior to return address push)
    lea rax, [rsp + 8]
    mov [rdi + 0x38], rax

    # RFLAGS
    pushfq
    pop rax
    mov [rdi + 0x88], rax

    # Save FS_BASE (IA32_FS_BASE = 0xC0000100)
    mov ecx, 0xC0000100
    rdmsr
    shl rdx, 32
    or rax, rdx
    mov [rdi + 0xA0], rax

    # Save User GS_BASE (IA32_KERNEL_GS_BASE = 0xC0000102)
    mov ecx, 0xC0000102
    rdmsr
    shl rdx, 32
    or rax, rdx
    mov [rdi + 0xA8], rax

    # Save CR3
    mov rax, cr3
    mov [rdi + 0xB0], rax

    # ------------------------------------------------------------
    # 2. Switch to next_ctx (%rsi)
    # ------------------------------------------------------------
    # Check if CR3 address space differs
    mov rax, [rsi + 0xB0]
    test rax, rax
    jz 1f
    mov rdx, cr3
    cmp rax, rdx
    je 1f
    mov cr3, rax        # Switch PML4 and invalidate TLB
1:

    # Restore FS_BASE (Linux TLS)
    mov rax, [rsi + 0xA0]
    mov edx, eax
    shr rax, 32
    xchg rax, rdx
    mov ecx, 0xC0000100
    wrmsr

    # Restore User GS_BASE (Windows NT TEB)
    mov rax, [rsi + 0xA8]
    mov edx, eax
    shr rax, 32
    xchg rax, rdx
    mov ecx, 0xC0000102
    wrmsr

    # Restore callee-saved registers
    mov rbx, [rsi + 0x08]
    mov rbp, [rsi + 0x30]
    mov r12, [rsi + 0x60]
    mov r13, [rsi + 0x68]
    mov r14, [rsi + 0x70]
    mov r15, [rsi + 0x78]

    # Load new stack pointer
    mov rsp, [rsi + 0x38]

    # Restore RFLAGS
    mov rax, [rsi + 0x88]
    push rax
    popfq

    # Jump to target RIP
    mov rax, [rsi + 0x80]
    push rax
    ret
    "#
);
