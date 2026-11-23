//! Windows NT Native API (SSDT) Syscall Primitives for HybridOS Ring 3.

use core::arch::asm;

// NT System Service Numbers (SSDT)
pub const SYS_NT_WAIT_FOR_SINGLE_OBJECT: u32    = 0x0004;
pub const SYS_NT_READ_FILE: u32                 = 0x0006;
pub const SYS_NT_DEVICE_IO_CONTROL_FILE: u32    = 0x0007;
pub const SYS_NT_WRITE_FILE: u32                = 0x0008;
pub const SYS_NT_SET_EVENT: u32                 = 0x000E;
pub const SYS_NT_CLOSE: u32                     = 0x000F;
pub const SYS_NT_QUERY_INFORMATION_FILE: u32    = 0x0011;
pub const SYS_NT_ALLOCATE_VIRTUAL_MEMORY: u32   = 0x0018;
pub const SYS_NT_ALLOCATE_VM: u32               = 0x0018;
pub const SYS_NT_RESET_EVENT: u32               = 0x001D;
pub const SYS_NT_FREE_VIRTUAL_MEMORY: u32       = 0x001E;
pub const SYS_NT_FREE_VM: u32                   = 0x001E;
pub const SYS_NT_QUERY_VIRTUAL_MEMORY: u32      = 0x0023;
pub const SYS_NT_TERMINATE_PROCESS: u32         = 0x002C;
pub const SYS_NT_OPEN_FILE: u32                 = 0x0033;
pub const SYS_NT_DELAY_EXECUTION: u32           = 0x0034;
pub const SYS_NT_QUERY_SYSTEM_INFORMATION: u32  = 0x0036;
pub const SYS_NT_YIELD_EXECUTION: u32           = 0x0046;
pub const SYS_NT_CREATE_EVENT: u32              = 0x0048;
pub const SYS_NT_CREATE_FILE: u32               = 0x0055;

// Win32k Shadow SSDT (0x1000..0x1200)
pub const SYS_NT_USER_CREATE_WINDOW_EX: u32     = 0x1000;
pub const SYS_NT_USER_DESTROY_WINDOW: u32       = 0x1001;
pub const SYS_NT_USER_SHOW_WINDOW: u32          = 0x1002;
pub const SYS_NT_USER_SET_WINDOW_POS: u32       = 0x1003;
pub const SYS_NT_USER_GET_MESSAGE: u32          = 0x1004;
pub const SYS_NT_USER_PEEK_MESSAGE: u32         = 0x1005;
pub const SYS_NT_USER_POST_MESSAGE: u32         = 0x1006;
pub const SYS_NT_USER_DISPATCH_MESSAGE: u32     = 0x1007;
pub const SYS_NT_USER_GET_DC: u32               = 0x1008;
pub const SYS_NT_USER_RELEASE_DC: u32           = 0x1009;
pub const SYS_NT_USER_GET_CLIENT_RECT: u32      = 0x100A;
pub const SYS_NT_USER_GET_WINDOW_RECT: u32      = 0x100B;
pub const SYS_NT_USER_FIND_WINDOW_EX: u32       = 0x100C;
pub const SYS_NT_USER_DEF_WINDOW_PROC: u32      = 0x100D;
pub const SYS_NT_USER_UPDATE_WINDOW: u32        = 0x100E;
pub const SYS_NT_USER_GET_DESKTOP_WINDOW: u32   = 0x100F;
pub const SYS_NT_GDI_CREATE_COMPATIBLE_DC: u32  = 0x1010;
pub const SYS_NT_GDI_DELETE_DC: u32             = 0x1011;
pub const SYS_NT_GDI_CREATE_BITMAP: u32         = 0x1012;
pub const SYS_NT_GDI_CREATE_COMPATIBLE_BITMAP: u32 = 0x1013;
pub const SYS_NT_GDI_SELECT_BITMAP: u32         = 0x1014;
pub const SYS_NT_GDI_BIT_BLT: u32               = 0x1015;
pub const SYS_NT_GDI_EXT_TEXT_OUT_W: u32        = 0x1016;
pub const SYS_NT_GDI_RECTANGLE: u32             = 0x1017;
pub const SYS_NT_GDI_SET_PIXEL: u32             = 0x1018;
pub const SYS_NT_GDI_GET_PIXEL: u32             = 0x1019;
pub const SYS_NT_GDI_CREATE_SOLID_BRUSH: u32    = 0x101A;
pub const SYS_NT_GDI_SELECT_BRUSH: u32          = 0x101B;
pub const SYS_NT_GDI_FILL_RECT: u32             = 0x101C;
pub const SYS_NT_GDI_LINE_TO: u32               = 0x101D;
pub const SYS_NT_USER_INVALIDATE_RECT: u32      = 0x101E;

// NT Status Codes
pub const STATUS_SUCCESS: u32                  = 0x00000000;
pub const STATUS_UNSUCCESSFUL: u32             = 0xC0000001;
pub const STATUS_NOT_IMPLEMENTED: u32          = 0xC0000002;
pub const STATUS_INVALID_HANDLE: u32           = 0xC0000008;
pub const STATUS_INVALID_PARAMETER: u32        = 0xC000000D;
pub const STATUS_NO_MEMORY: u32                = 0xC0000017;
pub const STATUS_ACCESS_DENIED: u32            = 0xC0000022;

// Memory Allocation Types
pub const MEM_COMMIT: u32                      = 0x00001000;
pub const MEM_RESERVE: u32                     = 0x00002000;
pub const MEM_RELEASE: u32                     = 0x00008000;

// Memory Protection Flags
pub const PAGE_NOACCESS: u32                   = 0x01;
pub const PAGE_READONLY: u32                   = 0x02;
pub const PAGE_READWRITE: u32                  = 0x04;
pub const PAGE_EXECUTE: u32                    = 0x10;
pub const PAGE_EXECUTE_READWRITE: u32          = 0x40;

/// Windows NT x64 Syscall Thunk:
/// mov r10, rcx
/// mov eax, <ssdt_id>
/// syscall
#[inline(always)]
pub unsafe fn nt_syscall0(ssdt_id: u32) -> u32 {
    let status: u32;
    asm!(
        "mov r10, rcx",
        "syscall",
        inlateout("eax") ssdt_id => status,
        out("r10") _,
        out("rcx") _,
        out("r11") _,
        options(nostack, preserves_flags)
    );
    status
}

#[inline(always)]
pub unsafe fn nt_syscall1(ssdt_id: u32, a1: u64) -> u32 {
    let status: u32;
    asm!(
        "mov r10, rcx",
        "syscall",
        inlateout("eax") ssdt_id => status,
        in("rcx") a1,
        out("r10") _,
        out("r11") _,
        options(nostack, preserves_flags)
    );
    status
}

#[inline(always)]
pub unsafe fn nt_syscall2(ssdt_id: u32, a1: u64, a2: u64) -> u32 {
    let status: u32;
    asm!(
        "mov r10, rcx",
        "syscall",
        inlateout("eax") ssdt_id => status,
        in("rcx") a1,
        in("rdx") a2,
        out("r10") _,
        out("r11") _,
        options(nostack, preserves_flags)
    );
    status
}

#[inline(always)]
pub unsafe fn nt_syscall3(ssdt_id: u32, a1: u64, a2: u64, a3: u64) -> u32 {
    let status: u32;
    asm!(
        "mov r10, rcx",
        "syscall",
        inlateout("eax") ssdt_id => status,
        in("rcx") a1,
        in("rdx") a2,
        in("r8") a3,
        out("r10") _,
        out("r11") _,
        options(nostack, preserves_flags)
    );
    status
}

#[inline(always)]
pub unsafe fn nt_syscall4(ssdt_id: u32, a1: u64, a2: u64, a3: u64, a4: u64) -> u32 {
    let status: u32;
    asm!(
        "mov r10, rcx",
        "syscall",
        inlateout("eax") ssdt_id => status,
        in("rcx") a1,
        in("rdx") a2,
        in("r8") a3,
        in("r9") a4,
        out("r10") _,
        out("r11") _,
        options(nostack, preserves_flags)
    );
    status
}

#[inline(always)]
pub unsafe fn nt_syscall6(
    ssdt_id: u32,
    a1: u64,
    a2: u64,
    a3: u64,
    a4: u64,
    a5: u64,
    a6: u64,
) -> u32 {
    let status: u32;
    // In Microsoft x64, arguments 5 and 6 are passed on the user stack frame.
    // The syscall router in HybridOS reads user stack at rsp+40 and rsp+48 (or 5th and 6th slots).
    // We push a6, then a5 to match standard stack layout.
    asm!(
        "push {arg6}",
        "push {arg5}",
        "mov r10, rcx",
        "syscall",
        "add rsp, 16",
        arg5 = in(reg) a5,
        arg6 = in(reg) a6,
        inlateout("eax") ssdt_id => status,
        in("rcx") a1,
        in("rdx") a2,
        in("r8") a3,
        in("r9") a4,
        out("r10") _,
        out("r11") _,
        options(preserves_flags)
    );
    status
}

/// NtWriteFile: writes buffer to an NT file handle
pub fn nt_write_file(
    file_handle: usize,
    event: usize,
    apc_routine: usize,
    _apc_context: usize,
    io_status_block: usize,
    buffer: &[u8],
) -> u32 {
    unsafe {
        nt_syscall6(
            SYS_NT_WRITE_FILE,
            file_handle as u64,
            event as u64,
            apc_routine as u64,
            buffer.as_ptr() as u64, // arg 4: buffer
            io_status_block as u64,
            buffer.len() as u64,   // arg 6: length
        )
    }
}

/// NtReadFile: reads from an NT file handle into buffer
pub fn nt_read_file(
    file_handle: usize,
    event: usize,
    apc_routine: usize,
    _apc_context: usize,
    io_status_block: usize,
    buffer: &mut [u8],
) -> u32 {
    unsafe {
        nt_syscall6(
            SYS_NT_READ_FILE,
            file_handle as u64,
            event as u64,
            apc_routine as u64,
            buffer.as_mut_ptr() as u64,
            io_status_block as u64,
            buffer.len() as u64,
        )
    }
}

/// NtCreateFile: creates or opens an NT file
pub fn nt_create_file(
    file_handle_out: &mut usize,
    desired_access: u32,
    object_attributes: usize,
) -> u32 {
    unsafe {
        nt_syscall4(
            SYS_NT_CREATE_FILE,
            file_handle_out as *mut usize as u64,
            desired_access as u64,
            object_attributes as u64,
            0,
        )
    }
}

/// NtOpenFile: opens an existing NT file
pub fn nt_open_file(
    file_handle_out: &mut usize,
    desired_access: u32,
    object_attributes: usize,
) -> u32 {
    unsafe {
        nt_syscall4(
            SYS_NT_OPEN_FILE,
            file_handle_out as *mut usize as u64,
            desired_access as u64,
            object_attributes as u64,
            0,
        )
    }
}

/// NtClose: closes an open NT handle
pub fn nt_close(handle: usize) -> u32 {
    unsafe {
        nt_syscall1(SYS_NT_CLOSE, handle as u64)
    }
}

/// NtAllocateVirtualMemory: commits or reserves virtual memory pages
pub fn nt_allocate_virtual_memory(
    process_handle: usize,
    base_address: &mut *mut u8,
    zero_bits: usize,
    region_size: &mut usize,
    allocation_type: u32,
    protect: u32,
) -> u32 {
    unsafe {
        nt_syscall6(
            SYS_NT_ALLOCATE_VM,
            process_handle as u64,
            base_address as *mut *mut u8 as u64,
            zero_bits as u64,
            region_size as *mut usize as u64,
            allocation_type as u64,
            protect as u64,
        )
    }
}

/// NtFreeVirtualMemory: frees or decommits virtual memory pages
pub fn nt_free_virtual_memory(
    process_handle: usize,
    base_address: &mut *mut u8,
    region_size: &mut usize,
    free_type: u32,
) -> u32 {
    unsafe {
        nt_syscall4(
            SYS_NT_FREE_VM,
            process_handle as u64,
            base_address as *mut *mut u8 as u64,
            region_size as *mut usize as u64,
            free_type as u64,
        )
    }
}

/// NtYieldExecution: yields CPU to another thread
pub fn nt_yield_execution() -> u32 {
    unsafe {
        nt_syscall0(SYS_NT_YIELD_EXECUTION)
    }
}

/// NtTerminateProcess: terminates process with exit status
pub fn nt_terminate_process(process_handle: usize, exit_status: u32) -> ! {
    unsafe {
        nt_syscall2(SYS_NT_TERMINATE_PROCESS, process_handle as u64, exit_status as u64);
        loop {
            asm!("ud2", options(noreturn));
        }
    }
}

/// NtQuerySystemInformation: queries system performance or environment
pub fn nt_query_system_information(
    info_class: u32,
    info_buf: *mut u8,
    info_len: usize,
    return_len: *mut usize,
) -> u32 {
    unsafe {
        nt_syscall4(
            SYS_NT_QUERY_SYSTEM_INFORMATION,
            info_class as u64,
            info_buf as u64,
            info_len as u64,
            return_len as u64,
        )
    }
}
