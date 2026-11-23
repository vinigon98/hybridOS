//! ntdll.dll: Windows NT Native System API for HybridOS Ring 3.

#![no_std]

use libhybrid_sys::windows::*;
use libhybrid_sys::nt_env::*;

#[repr(C)]
pub struct UnicodeString {
    pub length: u16,
    pub maximum_length: u16,
    pub buffer: *mut u16,
}

// ----------------------------------------------------------------------------
// Nt* System Call Wrappers
// ----------------------------------------------------------------------------

#[no_mangle]
pub unsafe extern "win64" fn NtWriteFile(
    file_handle: usize,
    event: usize,
    apc_routine: usize,
    _apc_context: usize,
    io_status_block: usize,
    buffer: *const u8,
    length: u32,
    byte_offset: *const u64,
    key: *const u32,
) -> u32 {
    let _ = (byte_offset, key);
    nt_syscall6(
        SYS_NT_WRITE_FILE,
        file_handle as u64,
        event as u64,
        apc_routine as u64,
        buffer as u64,
        io_status_block as u64,
        length as u64,
    )
}

#[no_mangle]
pub unsafe extern "win64" fn NtReadFile(
    file_handle: usize,
    event: usize,
    apc_routine: usize,
    _apc_context: usize,
    io_status_block: usize,
    buffer: *mut u8,
    length: u32,
    byte_offset: *const u64,
    key: *const u32,
) -> u32 {
    let _ = (byte_offset, key);
    nt_syscall6(
        SYS_NT_READ_FILE,
        file_handle as u64,
        event as u64,
        apc_routine as u64,
        buffer as u64,
        io_status_block as u64,
        length as u64,
    )
}

#[no_mangle]
pub unsafe extern "win64" fn NtCreateFile(
    file_handle: *mut usize,
    desired_access: u32,
    object_attributes: usize,
    io_status_block: usize,
    allocation_size: *const u64,
    file_attributes: u32,
    share_access: u32,
    create_disposition: u32,
    create_options: u32,
    ea_buffer: *const u8,
    ea_length: u32,
) -> u32 {
    let _ = (
        io_status_block,
        allocation_size,
        file_attributes,
        share_access,
        create_disposition,
        create_options,
        ea_buffer,
        ea_length,
    );
    nt_syscall4(
        SYS_NT_CREATE_FILE,
        file_handle as u64,
        desired_access as u64,
        object_attributes as u64,
        0,
    )
}

#[no_mangle]
pub unsafe extern "win64" fn NtOpenFile(
    file_handle: *mut usize,
    desired_access: u32,
    object_attributes: usize,
    io_status_block: usize,
    share_access: u32,
    open_options: u32,
) -> u32 {
    let _ = (io_status_block, share_access, open_options);
    nt_syscall4(
        SYS_NT_OPEN_FILE,
        file_handle as u64,
        desired_access as u64,
        object_attributes as u64,
        0,
    )
}

#[no_mangle]
pub unsafe extern "win64" fn NtClose(handle: usize) -> u32 {
    nt_syscall1(SYS_NT_CLOSE, handle as u64)
}

#[no_mangle]
pub unsafe extern "win64" fn NtAllocateVirtualMemory(
    process_handle: usize,
    base_address: *mut *mut u8,
    zero_bits: usize,
    region_size: *mut usize,
    allocation_type: u32,
    protect: u32,
) -> u32 {
    nt_syscall6(
        SYS_NT_ALLOCATE_VIRTUAL_MEMORY,
        process_handle as u64,
        base_address as u64,
        zero_bits as u64,
        region_size as u64,
        allocation_type as u64,
        protect as u64,
    )
}

#[no_mangle]
pub unsafe extern "win64" fn NtFreeVirtualMemory(
    process_handle: usize,
    base_address: *mut *mut u8,
    region_size: *mut usize,
    free_type: u32,
) -> u32 {
    nt_syscall4(
        SYS_NT_FREE_VM,
        process_handle as u64,
        base_address as u64,
        region_size as u64,
        free_type as u64,
    )
}

#[no_mangle]
pub unsafe extern "win64" fn NtYieldExecution() -> u32 {
    nt_syscall0(SYS_NT_YIELD_EXECUTION)
}

#[no_mangle]
pub unsafe extern "win64" fn NtTerminateProcess(process_handle: usize, exit_status: u32) -> ! {
    nt_syscall2(
        SYS_NT_TERMINATE_PROCESS,
        process_handle as u64,
        exit_status as u64,
    );
    loop {
        core::arch::asm!("ud2", options(noreturn));
    }
}

#[no_mangle]
pub unsafe extern "win64" fn NtQuerySystemInformation(
    system_information_class: u32,
    system_information: *mut u8,
    system_information_length: usize,
    return_length: *mut usize,
) -> u32 {
    nt_syscall4(
        SYS_NT_QUERY_SYSTEM_INFORMATION,
        system_information_class as u64,
        system_information as u64,
        system_information_length as u64,
        return_length as u64,
    )
}

// ----------------------------------------------------------------------------
// Zw* Aliases (Identical to Nt* in user space)
// ----------------------------------------------------------------------------
#[no_mangle]
pub unsafe extern "win64" fn ZwWriteFile(
    file_handle: usize,
    event: usize,
    apc_routine: usize,
    apc_context: usize,
    io_status_block: usize,
    buffer: *const u8,
    length: u32,
    byte_offset: *const u64,
    key: *const u32,
) -> u32 {
    NtWriteFile(
        file_handle,
        event,
        apc_routine,
        apc_context,
        io_status_block,
        buffer,
        length,
        byte_offset,
        key,
    )
}

#[no_mangle]
pub unsafe extern "win64" fn ZwClose(handle: usize) -> u32 {
    NtClose(handle)
}

// ----------------------------------------------------------------------------
// Rtl* Runtime Library Functions
// ----------------------------------------------------------------------------

#[no_mangle]
pub unsafe extern "win64" fn RtlInitUnicodeString(
    destination_string: *mut UnicodeString,
    source_string: *const u16,
) {
    if destination_string.is_null() {
        return;
    }
    if source_string.is_null() {
        (*destination_string).length = 0;
        (*destination_string).maximum_length = 0;
        (*destination_string).buffer = core::ptr::null_mut();
        return;
    }

    let mut len = 0usize;
    while *source_string.add(len) != 0 {
        len += 1;
    }

    let byte_len = (len * 2) as u16;
    (*destination_string).length = byte_len;
    (*destination_string).maximum_length = byte_len + 2;
    (*destination_string).buffer = source_string as *mut u16;
}

#[no_mangle]
pub unsafe extern "win64" fn RtlZeroMemory(destination: *mut u8, length: usize) {
    core::ptr::write_bytes(destination, 0, length);
}

#[no_mangle]
pub unsafe extern "win64" fn RtlCopyMemory(destination: *mut u8, source: *const u8, length: usize) {
    core::ptr::copy_nonoverlapping(source, destination, length);
}

#[no_mangle]
pub unsafe extern "win64" fn RtlAllocateHeap(
    _heap_handle: usize,
    _flags: u32,
    size: usize,
) -> *mut u8 {
    let mut base_addr: *mut u8 = core::ptr::null_mut();
    let mut alloc_size = (size + 4095) & !4095;
    let status = NtAllocateVirtualMemory(
        usize::MAX,
        &mut base_addr,
        0,
        &mut alloc_size,
        MEM_COMMIT | MEM_RESERVE,
        PAGE_READWRITE,
    );
    if status == STATUS_SUCCESS {
        base_addr
    } else {
        core::ptr::null_mut()
    }
}

#[no_mangle]
pub unsafe extern "win64" fn RtlFreeHeap(
    _heap_handle: usize,
    _flags: u32,
    base_address: *mut u8,
) -> u32 {
    let mut addr = base_address;
    let mut size = 0usize;
    let status = NtFreeVirtualMemory(usize::MAX, &mut addr, &mut size, MEM_RELEASE);
    if status == STATUS_SUCCESS { 1 } else { 0 }
}

// ----------------------------------------------------------------------------
// Ldr* Loader Functions
// ----------------------------------------------------------------------------

#[no_mangle]
pub unsafe extern "win64" fn LdrLoadDll(
    _path_to_file: *const u16,
    _flags: *const u32,
    _module_file_name: *const UnicodeString,
    module_handle: *mut usize,
) -> u32 {
    if !module_handle.is_null() {
        *module_handle = get_peb_address() as usize;
    }
    STATUS_SUCCESS
}

#[no_mangle]
pub unsafe extern "win64" fn LdrGetProcedureAddress(
    _module_handle: usize,
    _procedure_name: *const u8,
    _procedure_number: u32,
    procedure_address: *mut usize,
) -> u32 {
    if !procedure_address.is_null() {
        *procedure_address = 0;
    }
    STATUS_SUCCESS
}
