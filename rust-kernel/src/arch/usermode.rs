//! Architectural Ring 3 User-Mode Transition for x86_64.
//!
//! Provides the low-level hardware transition from Ring 0 (Kernel) to Ring 3 (User Mode)
//! using the x86_64 `iretq` instruction with GDT selectors `USER_CS` (0x23) and `USER_DS` (0x1B).

use core::arch::global_asm;

extern "C" {
    /// Performs an irrevocable transition from Ring 0 to Ring 3 via `iretq`.
    ///
    /// Arguments:
    /// - `entry_point`: Target 64-bit user instruction pointer (`RIP`)
    /// - `user_rsp`: Top of the allocated user stack (`RSP`)
    /// - `arg`: Primary argument passed to the process (in `RDI` for Linux, `RCX` for Windows)
    pub fn enter_user_mode(entry_point: u64, user_rsp: u64, arg: u64);
}

global_asm!(
    r#"
.global enter_user_mode
enter_user_mode:
    # Arguments (System V AMD64 ABI):
    # %rdi = entry_point
    # %rsi = user_rsp
    # %rdx = arg

    # 1. Pass primary argument in %rcx (Windows x64) and %rdi (Linux System V)
    # Save entry_point and user_rsp in scratch registers before clearing
    mov r8, rdi     # target RIP
    mov r9, rsi     # target RSP
    mov rcx, rdx    # Windows Win64 Arg 1
    mov rdi, rdx    # Linux System V Arg 1

    # 2. Clear remaining registers to prevent kernel information leaks to user space
    xor rax, rax
    xor rbx, rbx
    xor rdx, rdx
    xor rsi, rsi
    xor r10, r10
    xor r11, r11
    xor r12, r12
    xor r13, r13
    xor r14, r14
    xor r15, r15
    xor rbp, rbp

    # 3. Reload segment registers with USER_DS (0x1B: Index 3, RPL 3)
    mov ax, 0x1B
    mov ds, ax
    mov es, ax
    mov fs, ax

    # 4. Construct the x86_64 hardware iretq stack frame:
    # [SS]     <- 0x1B (User Data Segment)
    # [RSP]    <- user_rsp
    # [RFLAGS] <- 0x202 (IF = 1: Interrupts Enabled)
    # [CS]     <- 0x23 (User Code Segment, RPL 3)
    # [RIP]    <- entry_point
    push 0x1B
    push r9
    push 0x202
    push 0x23
    push r8

    # 5. Execute hardware transition to Ring 3
    iretq
    "#
);
