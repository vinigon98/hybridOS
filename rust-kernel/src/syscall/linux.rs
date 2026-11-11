//! Linux System V AMD64 Syscall Implementation in Rust.

#[allow(unused_imports)]
use alloc::string::{String, ToString};
#[allow(unused_imports)]
use alloc::vec::Vec;
#[allow(unused_imports)]
use alloc::boxed::Box;
#[allow(unused_imports)]
use crate::compat::FloatExt;
use crate::process::{HybridProcess, KernelObject, ObjectType};
use crate::types::{PosixError, Handle};

pub const SYS_LINUX_READ: u64   = 0;
pub const SYS_LINUX_WRITE: u64  = 1;
pub const SYS_LINUX_OPEN: u64   = 2;
pub const SYS_LINUX_CLOSE: u64  = 3;
pub const SYS_LINUX_MMAP: u64   = 9;
pub const SYS_LINUX_GETPID: u64 = 39;
pub const SYS_LINUX_EXIT: u64   = 60;

pub fn dispatch_linux_syscall(
    proc: &mut HybridProcess,
    syscall_nr: u64,
    a1: u64,
    a2: u64,
    a3: u64,
    _a4: u64,
    _a5: u64,
    _a6: u64,
) -> Result<u64, PosixError> {
    match syscall_nr {
        SYS_LINUX_WRITE => {
            let fd = Handle(a1 as usize);
            let buf_ptr = a2 as *const u8;
            let count = a3 as usize;

            if fd == Handle::STDOUT || fd == Handle::STDERR {
                if !buf_ptr.is_null() && count > 0 {
                    let slice = unsafe { std::slice::from_raw_parts(buf_ptr, count) };
                    print!("[Linux stdout/stderr]: {}", String::from_utf8_lossy(slice));
                }
                Ok(count as u64)
            } else {
                Ok(count as u64)
            }
        }

        SYS_LINUX_READ => {
            println!("[Linux POSIX] sys_read(fd={}, count={})", a1, a3);
            Ok(0)
        }

        SYS_LINUX_OPEN => {
            let obj = KernelObject {
                obj_type: ObjectType::File {
                    name: "sample_linux_file".to_string(),
                    offset: 0,
                },
                access_mask: 0x3,
            };
            let fd = proc.allocate_handle(obj);
            println!("[Linux POSIX] sys_open -> Assigned FD: {}", fd.0);
            Ok(fd.0 as u64)
        }

        SYS_LINUX_CLOSE => {
            let fd = Handle(a1 as usize);
            if proc.close_handle(fd) {
                println!("[Linux POSIX] sys_close: Closed FD {}", fd.0);
                Ok(0)
            } else {
                Err(PosixError::EBADF)
            }
        }

        SYS_LINUX_MMAP => {
            let length = a2 as usize;
            println!("[Linux POSIX] sys_mmap: Allocated {} bytes", length);
            Ok(0x00007FFF0000)
        }

        SYS_LINUX_GETPID => {
            Ok(proc.pid as u64)
        }

        SYS_LINUX_EXIT => {
            proc.is_terminated = true;
            proc.exit_code = a1 as i32;
            println!("[Linux POSIX] sys_exit: PID={} exited with status {}", proc.pid, a1);
            Ok(0)
        }

        _ => {
            super::linux_extended::dispatch_extended_syscall(proc, syscall_nr, a1, a2, a3, _a4, _a5, _a6)
        }
    }
}
