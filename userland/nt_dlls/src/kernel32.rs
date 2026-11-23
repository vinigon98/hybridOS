//! kernel32.dll: Win32 Subsystem Base API Library for HybridOS Ring 3.

use libhybrid_sys::windows::*;
use libhybrid_sys::nt_env::*;
use crate::ntdll::*;

pub const STD_INPUT_HANDLE: i32  = -10;
pub const STD_OUTPUT_HANDLE: i32 = -11;
pub const STD_ERROR_HANDLE: i32  = -12;

pub const GENERIC_READ: u32    = 0x80000000;
pub const GENERIC_WRITE: u32   = 0x40000000;
pub const GENERIC_EXECUTE: u32 = 0x20000000;
pub const GENERIC_ALL: u32     = 0x10000000;

pub const CREATE_NEW: u32        = 1;
pub const CREATE_ALWAYS: u32     = 2;
pub const OPEN_EXISTING: u32     = 3;
pub const OPEN_ALWAYS: u32       = 4;
pub const TRUNCATE_EXISTING: u32 = 5;

pub const FILE_ATTRIBUTE_NORMAL: u32 = 0x80;

#[no_mangle]
pub unsafe extern "win64" fn GetStdHandle(n_std_handle: i32) -> usize {
    match n_std_handle {
        STD_INPUT_HANDLE => 0,
        STD_OUTPUT_HANDLE => 1,
        STD_ERROR_HANDLE => 2,
        _ => usize::MAX,
    }
}

#[no_mangle]
pub unsafe extern "win64" fn WriteFile(
    h_file: usize,
    lp_buffer: *const u8,
    n_number_of_bytes_to_write: u32,
    lp_number_of_bytes_written: *mut u32,
    _lp_overlapped: usize,
) -> i32 {
    let mut io_status = [0u64; 2];
    let status = NtWriteFile(
        h_file,
        0,
        0,
        0,
        io_status.as_mut_ptr() as usize,
        lp_buffer,
        n_number_of_bytes_to_write,
        core::ptr::null(),
        core::ptr::null(),
    );

    if status == STATUS_SUCCESS {
        if !lp_number_of_bytes_written.is_null() {
            *lp_number_of_bytes_written = n_number_of_bytes_to_write;
        }
        1
    } else {
        SetLastError(status);
        0
    }
}

#[no_mangle]
pub unsafe extern "win64" fn ReadFile(
    h_file: usize,
    lp_buffer: *mut u8,
    n_number_of_bytes_to_read: u32,
    lp_number_of_bytes_read: *mut u32,
    _lp_overlapped: usize,
) -> i32 {
    let mut io_status = [0u64; 2];
    let status = NtReadFile(
        h_file,
        0,
        0,
        0,
        io_status.as_mut_ptr() as usize,
        lp_buffer,
        n_number_of_bytes_to_read,
        core::ptr::null(),
        core::ptr::null(),
    );

    if status == STATUS_SUCCESS {
        if !lp_number_of_bytes_read.is_null() {
            *lp_number_of_bytes_read = n_number_of_bytes_to_read;
        }
        1
    } else {
        SetLastError(status);
        0
    }
}

#[no_mangle]
pub unsafe extern "win64" fn CreateFileA(
    lp_file_name: *const u8,
    dw_desired_access: u32,
    dw_share_mode: u32,
    _lp_security_attributes: usize,
    dw_creation_disposition: u32,
    dw_flags_and_attributes: u32,
    _h_template_file: usize,
) -> usize {
    let mut handle = usize::MAX;
    let status = NtCreateFile(
        &mut handle,
        dw_desired_access,
        lp_file_name as usize,
        0,
        core::ptr::null(),
        dw_flags_and_attributes,
        dw_share_mode,
        dw_creation_disposition,
        0,
        core::ptr::null(),
        0,
    );

    if status == STATUS_SUCCESS {
        handle
    } else {
        SetLastError(status);
        usize::MAX
    }
}

#[no_mangle]
pub unsafe extern "win64" fn CreateFileW(
    lp_file_name: *const u16,
    dw_desired_access: u32,
    dw_share_mode: u32,
    _lp_security_attributes: usize,
    dw_creation_disposition: u32,
    dw_flags_and_attributes: u32,
    _h_template_file: usize,
) -> usize {
    let mut handle = usize::MAX;
    let status = NtCreateFile(
        &mut handle,
        dw_desired_access,
        lp_file_name as usize,
        0,
        core::ptr::null(),
        dw_flags_and_attributes,
        dw_share_mode,
        dw_creation_disposition,
        0,
        core::ptr::null(),
        0,
    );

    if status == STATUS_SUCCESS {
        handle
    } else {
        SetLastError(status);
        usize::MAX
    }
}

#[no_mangle]
pub unsafe extern "win64" fn CloseHandle(h_object: usize) -> i32 {
    let status = NtClose(h_object);
    if status == STATUS_SUCCESS { 1 } else { 0 }
}

#[no_mangle]
pub unsafe extern "win64" fn VirtualAlloc(
    lp_address: *mut u8,
    dw_size: usize,
    fl_allocation_type: u32,
    fl_protect: u32,
) -> *mut u8 {
    let mut base = lp_address;
    let mut region_size = dw_size;
    let status = NtAllocateVirtualMemory(
        usize::MAX,
        &mut base,
        0,
        &mut region_size,
        fl_allocation_type,
        fl_protect,
    );

    if status == STATUS_SUCCESS {
        base
    } else {
        SetLastError(status);
        core::ptr::null_mut()
    }
}

#[no_mangle]
pub unsafe extern "win64" fn VirtualFree(
    lp_address: *mut u8,
    dw_size: usize,
    dw_free_type: u32,
) -> i32 {
    let mut base = lp_address;
    let mut region_size = dw_size;
    let status = NtFreeVirtualMemory(usize::MAX, &mut base, &mut region_size, dw_free_type);
    if status == STATUS_SUCCESS { 1 } else { 0 }
}

#[no_mangle]
pub unsafe extern "win64" fn GetProcessHeap() -> usize {
    get_peb_address() as usize
}

#[no_mangle]
pub unsafe extern "win64" fn HeapAlloc(h_heap: usize, dw_flags: u32, dw_bytes: usize) -> *mut u8 {
    RtlAllocateHeap(h_heap, dw_flags, dw_bytes)
}

#[no_mangle]
pub unsafe extern "win64" fn HeapFree(h_heap: usize, dw_flags: u32, lp_mem: *mut u8) -> i32 {
    RtlFreeHeap(h_heap, dw_flags, lp_mem) as i32
}

#[no_mangle]
pub unsafe extern "win64" fn GetLastError() -> u32 {
    get_last_error()
}

#[no_mangle]
pub unsafe extern "win64" fn SetLastError(dw_err_code: u32) {
    set_last_error(dw_err_code);
}

#[no_mangle]
pub unsafe extern "win64" fn GetCurrentProcessId() -> u32 {
    get_current_process_id()
}

#[no_mangle]
pub unsafe extern "win64" fn GetCurrentThreadId() -> u32 {
    get_current_thread_id()
}

#[no_mangle]
pub unsafe extern "win64" fn GetTickCount64() -> u64 {
    get_tick_count64()
}

#[no_mangle]
pub unsafe extern "win64" fn ExitProcess(u_exit_code: u32) -> ! {
    NtTerminateProcess(usize::MAX, u_exit_code);
}

#[no_mangle]
pub unsafe extern "win64" fn Sleep(dw_milliseconds: u32) {
    // 100ns units negative for relative delay
    let intervals = -((dw_milliseconds as i64) * 10_000);
    let _ = nt_syscall2(SYS_NT_DELAY_EXECUTION, 0, intervals as u64);
}

#[no_mangle]
pub unsafe extern "win64" fn SwitchToThread() -> i32 {
    NtYieldExecution();
    1
}
