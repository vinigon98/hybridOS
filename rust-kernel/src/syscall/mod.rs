//! Universal Dual-Personality Syscall Router.

pub mod windows;
pub mod windows_extended;
pub mod win32k;
pub mod linux;
pub mod linux_extended;
pub mod posix_advanced;

use crate::process::HybridProcess;
use crate::personality::ExecutionPersonality;
use crate::trap::TrapFrame;

pub struct SyscallRouter;

impl SyscallRouter {
    pub fn dispatch(proc: &mut HybridProcess, tf: &mut TrapFrame) -> u64 {
        match proc.personality {
            ExecutionPersonality::WindowsNt => {
                // Microsoft x64 ABI decoding:
                // Syscall ID: RAX
                // Arg 1: R10
                // Arg 2: RDX
                // Arg 3: R8
                // Arg 4: R9
                // Arg 5 & 6: Passed via user stack frame
                let a1 = tf.r10;
                let a2 = tf.rdx;
                let a3 = tf.r8;
                let a4 = tf.r9;
                let mut a5 = 0;
                let mut a6 = 0;

                if tf.rsp != 0 {
                    let user_stack = tf.rsp as *const u64;
                    unsafe {
                        a5 = *user_stack.add(5);
                        a6 = *user_stack.add(6);
                    }
                }

                let status = if tf.rax >= 0x1000 {
                    win32k::dispatch_win32k_syscall(proc, tf.rax, a1, a2, a3, a4, a5, a6)
                } else {
                    windows::dispatch_windows_syscall(proc, tf.rax, a1, a2, a3, a4, a5, a6)
                };
                tf.rax = status.0 as u64;
                tf.rax
            }

            ExecutionPersonality::LinuxPosix | ExecutionPersonality::Android => {
                // System V AMD64 ABI decoding:
                // Syscall ID: RAX
                // Arg 1: RDI
                // Arg 2: RSI
                // Arg 3: RDX
                // Arg 4: R10
                // Arg 5: R8
                // Arg 6: R9
                let a1 = tf.rdi;
                let a2 = tf.rsi;
                let a3 = tf.rdx;
                let a4 = tf.r10;
                let a5 = tf.r8;
                let a6 = tf.r9;

                match linux::dispatch_linux_syscall(proc, tf.rax, a1, a2, a3, a4, a5, a6) {
                    Ok(val) => {
                        tf.rax = val;
                        val
                    }
                    Err(err) => {
                        let ret = -(err as i64) as u64;
                        tf.rax = ret;
                        ret
                    }
                }
            }

            ExecutionPersonality::HybridNative => {
                println!("[Syscall Router] Native Hybrid Syscall invoked: {}", tf.rax);
                0
            }
        }
    }
}
