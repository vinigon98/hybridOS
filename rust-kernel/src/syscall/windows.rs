//! Windows NT System Service Dispatch Table (SSDT) in Rust.

#[allow(unused_imports)]
use alloc::string::{String, ToString};
#[allow(unused_imports)]
use alloc::vec::Vec;
#[allow(unused_imports)]
use alloc::boxed::Box;
#[allow(unused_imports)]
use crate::compat::FloatExt;
use crate::process::{HybridProcess, KernelObject, ObjectType};
use crate::types::{NtStatus, Handle};

pub const SYS_NT_WRITE_FILE: u64           = 0x0008;
pub const SYS_NT_READ_FILE: u64            = 0x0006;
pub const SYS_NT_CREATE_FILE: u64          = 0x0055;
pub const SYS_NT_ALLOCATE_VM: u64          = 0x0018;
pub const SYS_NT_CLOSE: u64                = 0x000F;
pub const SYS_NT_TERMINATE_PROCESS: u64    = 0x002C;

pub fn dispatch_windows_syscall(
    proc: &mut HybridProcess,
    ssdt_index: u64,
    a1: u64,
    a2: u64,
    _a3: u64,
    _a4: u64,
    _a5: u64,
    a6: u64,
) -> NtStatus {
    match ssdt_index {
        SYS_NT_WRITE_FILE => {
            let handle = Handle(a1 as usize);
            if handle == Handle::STDOUT || handle == Handle::STDERR {
                let ptr = a6 as *const u8;
                if !ptr.is_null() {
                    let cstr = unsafe { std::ffi::CStr::from_ptr(ptr as *const i8) };
                    print!("[Win32 Console OUT]: {}", cstr.to_string_lossy());
                }
                NtStatus::SUCCESS
            } else {
                NtStatus::SUCCESS
            }
        }

        SYS_NT_CREATE_FILE => {
            let obj = KernelObject {
                obj_type: ObjectType::File {
                    name: "sample_file.txt".to_string(),
                    offset: 0,
                },
                access_mask: a2 as u32,
            };
            let handle = proc.allocate_handle(obj);
            let out_ptr = a1 as *mut usize;
            if !out_ptr.is_null() {
                unsafe { *out_ptr = handle.0 };
            }
            println!("[Win32 SSDT] NtCreateFile -> Assigned HANDLE: 0x{:X}", handle.0);
            NtStatus::SUCCESS
        }

        SYS_NT_ALLOCATE_VM => {
            let size = a2 as usize;
            println!("[Win32 SSDT] NtAllocateVirtualMemory: Allocated {} bytes", size);
            NtStatus::SUCCESS
        }

        SYS_NT_CLOSE => {
            let handle = Handle(a1 as usize);
            if proc.close_handle(handle) {
                println!("[Win32 SSDT] NtClose: Closed HANDLE 0x{:X}", handle.0);
                NtStatus::SUCCESS
            } else {
                NtStatus::INVALID_HANDLE
            }
        }

        SYS_NT_TERMINATE_PROCESS => {
            proc.is_terminated = true;
            proc.exit_code = a2 as i32;
            println!("[Win32 SSDT] NtTerminateProcess: PID={} terminated with code 0x{:X}", proc.pid, a2);
            NtStatus::SUCCESS
        }

        _ => {
            super::windows_extended::dispatch_extended_windows_syscall(
                proc, ssdt_index, a1, a2, _a3, _a4, _a5, a6,
            )
        }
    }
}
