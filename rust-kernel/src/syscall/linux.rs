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
use crate::mm;

pub const SYS_LINUX_READ: u64        = 0;
pub const SYS_LINUX_WRITE: u64       = 1;
pub const SYS_LINUX_OPEN: u64        = 2;
pub const SYS_LINUX_CLOSE: u64       = 3;
pub const SYS_LINUX_MMAP: u64        = 9;
pub const SYS_LINUX_MUNMAP: u64      = 11;
pub const SYS_LINUX_WRITEV: u64      = 20;
pub const SYS_LINUX_SCHED_YIELD: u64 = 24;
pub const SYS_LINUX_GETPID: u64      = 39;
pub const SYS_LINUX_EXIT: u64        = 60;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct IoVec64 {
    pub iov_base: u64,
    pub iov_len: u64,
}

pub fn dispatch_linux_syscall(
    proc: &mut HybridProcess,
    syscall_nr: u64,
    a1: u64, // RDI
    a2: u64, // RSI
    a3: u64, // RDX
    _a4: u64, // R10
    _a5: u64, // R8
    _a6: u64, // R9
) -> Result<u64, PosixError> {
    match syscall_nr {
        // 1. sys_write (1)
        SYS_LINUX_WRITE => {
            let fd = Handle(a1 as usize);
            let buf_ptr = a2 as *const u8;
            let count = a3 as usize;

            if fd == Handle::STDOUT || fd == Handle::STDERR {
                if !buf_ptr.is_null() && count > 0 {
                    if let Ok(slice) = unsafe { crate::security::copy_from_user_slice(buf_ptr, count) } {
                        let text = String::from_utf8_lossy(&slice);
                        crate::serial_print!("{}", text);
                    }
                }
                Ok(count as u64)
            } else {
                Ok(count as u64)
            }
        }

        // 2. sys_open (2)
        SYS_LINUX_OPEN => {
            let path_ptr = a1 as *const u8;
            if path_ptr.is_null() {
                return Err(PosixError::EFAULT);
            }

            let filename = unsafe {
                crate::security::copy_str_from_user(path_ptr, 1024)
                    .map_err(|_| PosixError::EFAULT)?
            };

            let obj = KernelObject {
                obj_type: ObjectType::File {
                    name: filename,
                    offset: 0,
                },
                access_mask: a2 as u32,
            };
            let fd = proc.allocate_handle(obj);
            Ok(fd.0 as u64)
        }

        // 3. sys_mmap (9)
        SYS_LINUX_MMAP => {
            let len = a2 as usize;
            if len == 0 {
                return Err(PosixError::EINVAL);
            }

            let aligned_len = (len + 4095) & !4095;
            let mut vaddr = a1;

            if vaddr == 0 {
                vaddr = 0x0000_7000_0000_0000;
            }

            let num_pages = aligned_len / 4096;
            let hhdm_offset = crate::boot::get_hhdm_offset();

            {
                let mut alloc_guard = mm::GLOBAL_FRAME_ALLOCATOR.lock();
                if let Some(ref mut alloc) = *alloc_guard {
                    for _p in 0..num_pages {
                        if let Some(frame) = alloc.allocate_frame() {
                            let frame_hhdm = (frame.start_address().as_u64() + hhdm_offset) as *mut u8;
                            unsafe { core::ptr::write_bytes(frame_hhdm, 0, 4096); }
                        }
                    }
                }
            }

            Ok(vaddr)
        }

        // 4. sys_munmap (11)
        SYS_LINUX_MUNMAP => {
            let addr = a1;
            let len = a2 as usize;

            if addr & 4095 != 0 || len == 0 {
                return Err(PosixError::EINVAL);
            }

            Ok(0)
        }

        // 5. sys_getpid (39)
        SYS_LINUX_GETPID => {
            Ok(proc.pid as u64)
        }

        // 6. sys_writev (20) — Scatter-Gather I/O
        SYS_LINUX_WRITEV => {
            let fd = Handle(a1 as usize);
            let iov_ptr = a2 as *const IoVec64;
            let iovcnt = a3 as usize;

            if iov_ptr.is_null() || iovcnt == 0 || iovcnt > 1024 {
                return Err(PosixError::EINVAL);
            }

            let mut total_written = 0u64;

            for i in 0..iovcnt {
                let cur_iov_ptr = unsafe { iov_ptr.add(i) };
                let iov = unsafe {
                    crate::security::copy_from_user(cur_iov_ptr)
                        .map_err(|_| PosixError::EFAULT)?
                };

                if iov.iov_len == 0 {
                    continue;
                }

                let base_ptr = iov.iov_base as *const u8;
                let len = iov.iov_len as usize;

                if fd == Handle::STDOUT || fd == Handle::STDERR {
                    if let Ok(slice) = unsafe { crate::security::copy_from_user_slice(base_ptr, len) } {
                        let text = String::from_utf8_lossy(&slice);
                        crate::serial_print!("{}", text);
                    }
                }

                total_written += iov.iov_len;
            }

            Ok(total_written)
        }

        SYS_LINUX_READ => {
            Ok(0)
        }

        SYS_LINUX_CLOSE => {
            let fd = Handle(a1 as usize);
            if proc.close_handle(fd) {
                Ok(0)
            } else {
                Err(PosixError::EBADF)
            }
        }

        SYS_LINUX_SCHED_YIELD => {
            Ok(0)
        }

        SYS_LINUX_EXIT => {
            proc.is_terminated = true;
            proc.exit_code = a1 as i32;
            Ok(0)
        }

        _ => {
            super::linux_extended::dispatch_extended_syscall(
                proc, syscall_nr, a1, a2, a3, _a4, _a5, _a6,
            )
        }
    }
}
