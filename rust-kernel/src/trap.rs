//! x86_64 CPU Trap Frame and calling convention register representations.

#[allow(unused_imports)]
use alloc::string::{String, ToString};
#[allow(unused_imports)]
use alloc::vec::Vec;
#[allow(unused_imports)]
use alloc::boxed::Box;
#[allow(unused_imports)]
use crate::compat::FloatExt;
#[derive(Debug, Default, Clone)]
#[repr(C)]
pub struct TrapFrame {
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub r11: u64, // Saved RFLAGS
    pub r10: u64, // Win64 Arg1, Linux Arg4
    pub r9:  u64, // Win64 Arg4, Linux Arg6
    pub r8:  u64, // Win64 Arg3, Linux Arg5
    pub rbp: u64,
    pub rdi: u64, // Linux Arg1
    pub rsi: u64, // Linux Arg2
    pub rdx: u64, // Win64 Arg2, Linux Arg3
    pub rcx: u64, // Saved RIP, Win64 standard Arg1
    pub rbx: u64,
    pub rax: u64, // Syscall ID & Return value

    pub rip: u64,
    pub cs: u64,
    pub rflags: u64,
    pub rsp: u64,
    pub ss: u64,
}
