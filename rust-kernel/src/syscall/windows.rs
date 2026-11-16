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
use crate::mm;

pub const SYS_NT_READ_FILE: u64                 = 0x0006;
pub const SYS_NT_WRITE_FILE: u64                = 0x0008;
pub const SYS_NT_CLOSE: u64                     = 0x000F;
pub const SYS_NT_ALLOCATE_VM: u64               = 0x0018;
pub const SYS_NT_TERMINATE_PROCESS: u64         = 0x002C;
pub const SYS_NT_QUERY_SYSTEM_INFORMATION: u64  = 0x0036;
pub const SYS_NT_YIELD_EXECUTION: u64           = 0x0046;
pub const SYS_NT_CREATE_FILE: u64               = 0x0055;

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct IoStatusBlock {
    pub status: u64,
    pub information: usize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SystemBasicInformation {
    pub reserved: u32,
    pub timer_resolution: u32,
    pub page_size: u32,
    pub number_of_physical_pages: u32,
    pub lowest_physical_page_number: u32,
    pub highest_physical_page_number: u32,
    pub allocation_granularity: u32,
    pub minimum_user_mode_address: u64,
    pub maximum_user_mode_address: u64,
    pub active_processors_affinity_mask: u64,
    pub number_of_processors: u32,
}

pub fn dispatch_windows_syscall(
    proc: &mut HybridProcess,
    ssdt_index: u64,
    a1: u64, // R10
    a2: u64, // RDX
    a3: u64, // R8
    a4: u64, // R9
    a5: u64, // Stack Arg 5 ([RSP + 40])
    a6: u64, // Stack Arg 6 ([RSP + 48])
) -> NtStatus {
    match ssdt_index {
        // 1. NtCreateFile (0x0055)
        SYS_NT_CREATE_FILE => {
            if a1 == 0 {
                return NtStatus::INVALID_PARAMETER;
            }

            let file_name = if a3 != 0 {
                let obj_attr = a3 as *const u64;
                let unicode_str_ptr = unsafe {
                    if let Ok(ptr) = crate::security::copy_from_user(obj_attr.add(1)) {
                        ptr as *const u16
                    } else {
                        core::ptr::null()
                    }
                };
                if !unicode_str_ptr.is_null() {
                    "C:\\HybridOS\\file.dat".to_string()
                } else {
                    "C:\\HybridOS\\unnamed.bin".to_string()
                }
            } else {
                "C:\\HybridOS\\default.bin".to_string()
            };

            let obj = KernelObject {
                obj_type: ObjectType::File {
                    name: file_name,
                    offset: 0,
                },
                access_mask: a2 as u32,
            };
            let handle = proc.allocate_handle(obj);

            let out_ptr = a1 as *mut usize;
            unsafe {
                if crate::security::copy_to_user(out_ptr, &handle.0).is_err() {
                    return NtStatus::ACCESS_VIOLATION;
                }
            }

            if a4 != 0 {
                let isb = IoStatusBlock {
                    status: 0, // STATUS_SUCCESS
                    information: 1, // FILE_OPENED
                };
                let isb_ptr = a4 as *mut IoStatusBlock;
                unsafe {
                    let _ = crate::security::copy_to_user(isb_ptr, &isb);
                }
            }

            NtStatus::SUCCESS
        }

        // 2. NtWriteFile (0x0008)
        SYS_NT_WRITE_FILE => {
            let handle = Handle(a1 as usize);
            let buf_ptr = if a6 != 0 { a6 as *const u8 } else { a4 as *const u8 };
            let isb_ptr = a5 as *mut IoStatusBlock;

            if handle == Handle::STDOUT || handle == Handle::STDERR {
                if !buf_ptr.is_null() {
                    if let Ok(slice) = unsafe { crate::security::copy_str_from_user(buf_ptr, 2048) } {
                        crate::serial_print!("{}", slice);
                    }
                }
                if !isb_ptr.is_null() {
                    let isb = IoStatusBlock { status: 0, information: 1 };
                    unsafe { let _ = crate::security::copy_to_user(isb_ptr, &isb); }
                }
                NtStatus::SUCCESS
            } else if let Some(obj) = proc.handles.get_mut(&handle) {
                if let Ok(mut guard) = obj.lock() {
                    if let ObjectType::File { ref mut offset, .. } = guard.obj_type {
                        *offset += 1;
                    }
                }
                NtStatus::SUCCESS
            } else {
                NtStatus::INVALID_HANDLE
            }
        }

        // 3. NtReadFile (0x0006)
        SYS_NT_READ_FILE => {
            let handle = Handle(a1 as usize);
            let buf_ptr = a6 as *mut u8;
            let isb_ptr = a5 as *mut IoStatusBlock;

            if buf_ptr.is_null() {
                return NtStatus::INVALID_PARAMETER;
            }

            if handle == Handle::STDIN {
                let isb = IoStatusBlock { status: 0, information: 0 };
                if !isb_ptr.is_null() {
                    unsafe { let _ = crate::security::copy_to_user(isb_ptr, &isb); }
                }
                NtStatus::SUCCESS
            } else if proc.handles.contains_key(&handle) {
                let isb = IoStatusBlock { status: 0, information: 0 };
                if !isb_ptr.is_null() {
                    unsafe { let _ = crate::security::copy_to_user(isb_ptr, &isb); }
                }
                NtStatus::SUCCESS
            } else {
                NtStatus::INVALID_HANDLE
            }
        }

        // 4. NtAllocateVirtualMemory (0x0018)
        SYS_NT_ALLOCATE_VM => {
            let base_ptr = a2 as *mut u64;
            let size_ptr = a4 as *mut usize;

            if base_ptr.is_null() || size_ptr.is_null() {
                return NtStatus::INVALID_PARAMETER;
            }

            let raw_size = unsafe {
                match crate::security::copy_from_user(size_ptr) {
                    Ok(s) if s > 0 => s,
                    _ => return NtStatus::INVALID_PARAMETER,
                }
            };

            let aligned_size = (raw_size + 4095) & !4095;
            let mut base_addr = unsafe {
                crate::security::copy_from_user(base_ptr).unwrap_or(0)
            };

            if base_addr == 0 {
                base_addr = 0x0000_0001_8000_0000;
            }

            let pages = aligned_size / 4096;
            let hhdm_offset = crate::boot::get_hhdm_offset();

            {
                let mut alloc_guard = mm::GLOBAL_FRAME_ALLOCATOR.lock();
                if let Some(ref mut alloc) = *alloc_guard {
                    for p in 0..pages {
                        let _page_vaddr = base_addr + (p as u64 * 4096);
                        if let Some(frame) = alloc.allocate_frame() {
                            let frame_hhdm = (frame.start_address().as_u64() + hhdm_offset) as *mut u8;
                            unsafe { core::ptr::write_bytes(frame_hhdm, 0, 4096); }
                        }
                    }
                }
            }

            unsafe {
                let _ = crate::security::copy_to_user(base_ptr, &base_addr);
                let _ = crate::security::copy_to_user(size_ptr, &aligned_size);
            }

            NtStatus::SUCCESS
        }

        // 5. NtQuerySystemInformation (0x0036)
        SYS_NT_QUERY_SYSTEM_INFORMATION => {
            let sys_class = a1 as u32;
            let info_ptr = a2 as *mut u8;
            let info_len = a3 as usize;
            let ret_ptr = a4 as *mut u32;

            if info_ptr.is_null() {
                return NtStatus::INVALID_PARAMETER;
            }

            match sys_class {
                0 => { // SystemBasicInformation
                    let s_size = core::mem::size_of::<SystemBasicInformation>();
                    if info_len < s_size {
                        return NtStatus::BUFFER_TOO_SMALL;
                    }
                    let basic = SystemBasicInformation {
                        reserved: 0,
                        timer_resolution: 156250, // 15.625 ms
                        page_size: 4096,
                        number_of_physical_pages: 1048576, // 4GB
                        lowest_physical_page_number: 1,
                        highest_physical_page_number: 1048576,
                        allocation_granularity: 65536, // 64KB
                        minimum_user_mode_address: 0x0001_0000,
                        maximum_user_mode_address: 0x0000_7FFF_FFFF_0000,
                        active_processors_affinity_mask: 0x01,
                        number_of_processors: 1,
                    };
                    unsafe {
                        let src = &basic as *const SystemBasicInformation as *const u8;
                        let slice = core::slice::from_raw_parts(src, s_size);
                        if crate::security::copy_to_user_slice(info_ptr, slice).is_err() {
                            return NtStatus::ACCESS_VIOLATION;
                        }
                    }
                    if !ret_ptr.is_null() {
                        unsafe { let _ = crate::security::copy_to_user(ret_ptr, &(s_size as u32)); }
                    }
                    NtStatus::SUCCESS
                }
                _ => NtStatus::NOT_IMPLEMENTED,
            }
        }

        SYS_NT_CLOSE => {
            let handle = Handle(a1 as usize);
            if proc.close_handle(handle) {
                NtStatus::SUCCESS
            } else {
                NtStatus::INVALID_HANDLE
            }
        }

        SYS_NT_YIELD_EXECUTION => {
            NtStatus::SUCCESS
        }

        SYS_NT_TERMINATE_PROCESS => {
            proc.is_terminated = true;
            proc.exit_code = a2 as i32;
            NtStatus::SUCCESS
        }

        _ => {
            super::windows_extended::dispatch_extended_windows_syscall(
                proc, ssdt_index, a1, a2, a3, a4, a5, a6,
            )
        }
    }
}
