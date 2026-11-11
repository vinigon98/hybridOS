//! Extended Windows NT System Service Dispatch Table (SSDT) Subsystem.
//!
//! Implements 20+ core Windows NT native system services covering:
//! - File I/O (NtCreateFile, NtOpenFile, NtReadFile, NtWriteFile, NtClose, NtQueryInformationFile, NtDeviceIoControlFile)
//! - Virtual Memory (NtAllocateVirtualMemory, NtFreeVirtualMemory, NtProtectVirtualMemory, NtQueryVirtualMemory, NtCreateSection, NtMapViewOfSection, NtUnmapViewOfSection)
//! - Process & Thread (NtCreateProcessEx, NtCreateThreadEx, NtTerminateProcess, NtTerminateThread, NtSuspendThread, NtResumeThread)
//! - Synchronization (NtCreateEvent, NtSetEvent, NtResetEvent, NtWaitForSingleObject, NtWaitForMultipleObjects)
//! - System Information & Execution (NtQuerySystemInformation, NtDelayExecution)

#[allow(unused_imports)]
use alloc::string::{String, ToString};
#[allow(unused_imports)]
use alloc::vec::Vec;
#[allow(unused_imports)]
use alloc::boxed::Box;
#[allow(unused_imports)]
use crate::compat::FloatExt;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

use crate::process::{HybridProcess, KernelObject, ObjectType};
use crate::types::{Handle, NtStatus};

// ============================================================================
// SSDT Service Indices (Canonical Windows x64 Native System Call Numbers)
// ============================================================================

// Synchronization
pub const SYS_NT_WAIT_FOR_SINGLE_OBJECT: u64    = 0x0004;
pub const SYS_NT_WAIT_FOR_MULTIPLE_OBJECTS: u64 = 0x005F;
pub const SYS_NT_CREATE_EVENT: u64              = 0x0048;
pub const SYS_NT_SET_EVENT: u64                 = 0x000E;
pub const SYS_NT_RESET_EVENT: u64               = 0x001D;

// File I/O
pub const SYS_NT_READ_FILE: u64                 = 0x0006;
pub const SYS_NT_DEVICE_IO_CONTROL_FILE: u64    = 0x0007;
pub const SYS_NT_WRITE_FILE: u64                = 0x0008;
pub const SYS_NT_CLOSE: u64                     = 0x000F;
pub const SYS_NT_QUERY_INFORMATION_FILE: u64    = 0x0011;
pub const SYS_NT_OPEN_FILE: u64                 = 0x0033;
pub const SYS_NT_CREATE_FILE: u64               = 0x0055;

// Virtual Memory
pub const SYS_NT_ALLOCATE_VIRTUAL_MEMORY: u64   = 0x0018;
pub const SYS_NT_FREE_VIRTUAL_MEMORY: u64       = 0x001E;
pub const SYS_NT_QUERY_VIRTUAL_MEMORY: u64      = 0x0023;
pub const SYS_NT_MAP_VIEW_OF_SECTION: u64       = 0x0028;
pub const SYS_NT_UNMAP_VIEW_OF_SECTION: u64     = 0x002A;
pub const SYS_NT_CREATE_SECTION: u64            = 0x004A;
pub const SYS_NT_PROTECT_VIRTUAL_MEMORY: u64    = 0x0050;

// Process & Thread
pub const SYS_NT_TERMINATE_PROCESS: u64         = 0x002C;
pub const SYS_NT_CREATE_PROCESS_EX: u64         = 0x004D;
pub const SYS_NT_RESUME_THREAD: u64             = 0x0052;
pub const SYS_NT_TERMINATE_THREAD: u64          = 0x0053;
pub const SYS_NT_CREATE_THREAD_EX: u64          = 0x00C1;
pub const SYS_NT_SUSPEND_THREAD: u64            = 0x01B1;

// System Information & Execution
pub const SYS_NT_DELAY_EXECUTION: u64           = 0x0034;
pub const SYS_NT_QUERY_SYSTEM_INFORMATION: u64  = 0x0036;

// ============================================================================
// NT Native Flags, Protection Masks & Constants
// ============================================================================

// Memory Protection Flags
pub const PAGE_NOACCESS: u32          = 0x01;
pub const PAGE_READONLY: u32          = 0x02;
pub const PAGE_READWRITE: u32         = 0x04;
pub const PAGE_WRITECOPY: u32         = 0x08;
pub const PAGE_EXECUTE: u32           = 0x10;
pub const PAGE_EXECUTE_READ: u32      = 0x20;
pub const PAGE_EXECUTE_READWRITE: u32 = 0x40;
pub const PAGE_EXECUTE_WRITECOPY: u32 = 0x80;
pub const PAGE_GUARD: u32             = 0x100;
pub const PAGE_NOCACHE: u32           = 0x200;

// Memory Allocation Types
pub const MEM_COMMIT: u32      = 0x00001000;
pub const MEM_RESERVE: u32     = 0x00002000;
pub const MEM_DECOMMIT: u32    = 0x00004000;
pub const MEM_RELEASE: u32     = 0x00008000;
pub const MEM_FREE: u32        = 0x00010000;
pub const MEM_PRIVATE: u32     = 0x00020000;
pub const MEM_MAPPED: u32      = 0x00040000;
pub const MEM_RESET: u32       = 0x00080000;
pub const MEM_TOP_DOWN: u32    = 0x00100000;

// File Information Classes
pub const FILE_DIRECTORY_INFORMATION: u32   = 1;
pub const FILE_BASIC_INFORMATION: u32       = 4;
pub const FILE_STANDARD_INFORMATION: u32    = 5;
pub const FILE_INTERNAL_INFORMATION: u32    = 6;
pub const FILE_EA_INFORMATION: u32          = 7;
pub const FILE_ACCESS_INFORMATION: u32      = 8;
pub const FILE_NAME_INFORMATION: u32        = 9;
pub const FILE_POSITION_INFORMATION: u32    = 14;
pub const FILE_END_OF_FILE_INFORMATION: u32 = 20;

// System Information Classes
pub const SYSTEM_BASIC_INFORMATION: u32       = 0;
pub const SYSTEM_PROCESSOR_INFORMATION: u32   = 1;
pub const SYSTEM_PERFORMANCE_INFORMATION: u32 = 2;
pub const SYSTEM_TIME_OF_DAY_INFORMATION: u32 = 3;
pub const SYSTEM_PROCESS_INFORMATION: u32     = 5;

// Wait Types
pub const WAIT_ALL: u32 = 0;
pub const WAIT_ANY: u32 = 1;

// Event Types
pub const NOTIFICATION_EVENT: u32    = 0; // Manual reset event
pub const SYNCHRONIZATION_EVENT: u32 = 1; // Auto reset event

// Thread Create Flags
pub const THREAD_CREATE_FLAGS_CREATE_SUSPENDED: u32 = 0x00000001;

// Section Page Protection & Access
pub const SECTION_QUERY: u32       = 0x0001;
pub const SECTION_MAP_WRITE: u32   = 0x0002;
pub const SECTION_MAP_READ: u32    = 0x0004;
pub const SECTION_MAP_EXECUTE: u32 = 0x0008;
pub const SECTION_ALL_ACCESS: u32  = 0x000F001F;

// File Disposition Constants
pub const FILE_SUPERSEDE: u32    = 0x00000000;
pub const FILE_OPEN: u32         = 0x00000001;
pub const FILE_CREATE: u32       = 0x00000002;
pub const FILE_OPEN_IF: u32      = 0x00000003;
pub const FILE_OVERWRITE: u32    = 0x00000004;
pub const FILE_OVERWRITE_IF: u32 = 0x00000005;

// File Action Results for IoStatusBlock.information
pub const FILE_SUPERSEDED: usize = 0;
pub const FILE_OPENED: usize     = 1;
pub const FILE_CREATED: usize    = 2;
pub const FILE_OVERWRITTEN: usize= 3;
pub const FILE_EXISTS: usize     = 4;
pub const FILE_DOES_NOT_EXIST: usize = 5;

// Extended NTSTATUS return values
pub const STATUS_SUCCESS: NtStatus               = NtStatus(0x00000000);
pub const STATUS_WAIT_0: NtStatus                = NtStatus(0x00000000);
pub const STATUS_TIMEOUT: NtStatus               = NtStatus(0x00000102);
pub const STATUS_PENDING: NtStatus               = NtStatus(0x00000103);
pub const STATUS_BUFFER_OVERFLOW: NtStatus       = NtStatus(-2147483643); // 0x80000005
pub const STATUS_UNSUCCESSFUL: NtStatus          = NtStatus(-1073741823); // 0xC0000001
pub const STATUS_NOT_IMPLEMENTED: NtStatus       = NtStatus(-1073741822); // 0xC0000002
pub const STATUS_INFO_LENGTH_MISMATCH: NtStatus  = NtStatus(-1073741820); // 0xC0000004
pub const STATUS_INVALID_HANDLE: NtStatus        = NtStatus(-1073741816); // 0xC0000008
pub const STATUS_INVALID_PARAMETER: NtStatus     = NtStatus(-1073741811); // 0xC000000D
pub const STATUS_END_OF_FILE: NtStatus           = NtStatus(-1073741807); // 0xC0000011
pub const STATUS_NO_MEMORY: NtStatus             = NtStatus(-1073741801); // 0xC0000017
pub const STATUS_ACCESS_DENIED: NtStatus         = NtStatus(-1073741790); // 0xC0000022
pub const STATUS_BUFFER_TOO_SMALL: NtStatus      = NtStatus(-1073741789); // 0xC0000023
pub const STATUS_OBJECT_NAME_NOT_FOUND: NtStatus = NtStatus(-1073741772); // 0xC0000034
pub const STATUS_OBJECT_PATH_NOT_FOUND: NtStatus = NtStatus(-1073741766); // 0xC000003A
pub const STATUS_SECTION_TOO_BIG: NtStatus       = NtStatus(-1073741760); // 0xC0000040
pub const STATUS_SUSPEND_COUNT_EXCEEDED: NtStatus= NtStatus(-1073741750); // 0xC000004A
pub const STATUS_THREAD_IS_TERMINATING: NtStatus = NtStatus(-1073741749); // 0xC000004B
pub const STATUS_NOT_SUPPORTED: NtStatus         = NtStatus(-1073741637); // 0xC00000BB

// ============================================================================
// NT Native Binary Structures (x86_64 ABI layout)
// ============================================================================

/// IO_STATUS_BLOCK
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct IoStatusBlock {
    pub status: i32,
    pub information: usize,
}

/// UNICODE_STRING
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct UnicodeString {
    pub length: u16,
    pub maximum_length: u16,
    pub buffer: u64,
}

/// OBJECT_ATTRIBUTES
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct ObjectAttributes {
    pub length: u32,
    pub root_directory: usize,
    pub object_name: u64, // *const UnicodeString
    pub attributes: u32,
    pub security_descriptor: u64,
    pub security_quality_of_service: u64,
}

/// MEMORY_BASIC_INFORMATION
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct MemoryBasicInformation {
    pub base_address: u64,
    pub allocation_base: u64,
    pub allocation_protect: u32,
    pub partition_id: u16,
    pub _pad: u16,
    pub region_size: usize,
    pub state: u32,
    pub protect: u32,
    pub mem_type: u32,
}

/// FILE_STANDARD_INFORMATION
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct FileStandardInformation {
    pub allocation_size: i64,
    pub end_of_file: i64,
    pub number_of_links: u32,
    pub delete_pending: u8,
    pub directory: u8,
    pub _pad: [u8; 2],
}

/// FILE_BASIC_INFORMATION
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct FileBasicInformation {
    pub creation_time: i64,
    pub last_access_time: i64,
    pub last_write_time: i64,
    pub change_time: i64,
    pub file_attributes: u32,
    pub _pad: u32,
}

/// FILE_POSITION_INFORMATION
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct FilePositionInformation {
    pub current_byte_offset: i64,
}

/// SYSTEM_BASIC_INFORMATION
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemBasicInformation {
    pub reserved: u32,
    pub timer_resolution: u32,
    pub page_size: u32,
    pub number_of_physical_pages: u32,
    pub lowest_physical_page_number: u32,
    pub highest_physical_page_number: u32,
    pub allocation_granularity: u32,
    pub minimum_user_mode_address: usize,
    pub maximum_user_mode_address: usize,
    pub active_processors_affinity_mask: usize,
    pub number_of_processors: u8,
    pub _pad: [u8; 7],
}

/// SYSTEM_PROCESSOR_INFORMATION
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemProcessorInformation {
    pub processor_architecture: u16,
    pub processor_level: u16,
    pub processor_revision: u16,
    pub maximum_processors: u16,
    pub processor_feature_bits: u32,
}

/// SYSTEM_PERFORMANCE_INFORMATION
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemPerformanceInformation {
    pub idle_process_time: i64,
    pub io_read_transfer_count: i64,
    pub io_write_transfer_count: i64,
    pub io_other_transfer_count: i64,
    pub io_read_operation_count: u32,
    pub io_write_operation_count: u32,
    pub io_other_operation_count: u32,
    pub available_pages: u32,
    pub committed_pages: usize,
    pub commit_limit: usize,
    pub peak_commitment: usize,
}

/// SYSTEM_TIME_OF_DAY_INFORMATION
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemTimeOfDayInformation {
    pub boot_time: i64,
    pub current_time: i64,
    pub time_zone_bias: i64,
    pub time_zone_id: u32,
    pub reserved: u32,
    pub boot_time_bias: i64,
    pub sleep_time_bias: i64,
}

// ============================================================================
// Internal Kernel State Representation
// ============================================================================

/// Virtual Memory Region tracked per process.
#[derive(Debug, Clone)]
pub struct VmRegion {
    pub base: u64,
    pub size: usize,
    pub protect: u32,
    pub state: u32,
    pub is_mapped: bool,
}

/// Kernel Shared Section Object.
#[derive(Debug, Clone)]
pub struct SectionObject {
    pub id: usize,
    pub name: Option<String>,
    pub size: usize,
    pub protect: u32,
    pub data: Vec<u8>,
}

/// Synchronization Event Object.
#[derive(Debug, Clone)]
pub struct EventObject {
    pub manual_reset: bool,
    pub signaled: bool,
    pub waiters_count: usize,
}

/// Extended Thread Control representation.
#[derive(Debug, Clone)]
pub struct ExtendedThreadInfo {
    pub tid: u32,
    pub pid: u32,
    pub start_routine: u64,
    pub argument: u64,
    pub suspended: bool,
    pub suspend_count: u32,
    pub terminated: bool,
    pub exit_code: i32,
}

/// Extended Process Info tracked globally.
#[derive(Debug, Clone)]
pub struct ExtendedProcessInfo {
    pub pid: u32,
    pub parent_pid: u32,
    pub name: String,
    pub vm_regions: Vec<VmRegion>,
    pub next_vm_alloc: u64,
    pub is_terminated: bool,
    pub exit_code: i32,
}

/// Extended File Inode representation.
#[derive(Debug, Clone)]
pub struct ExtendedFileInfo {
    pub handle_id: usize,
    pub path: String,
    pub offset: u64,
    pub size: u64,
    pub access: u32,
    pub content: Vec<u8>,
}

/// Global Extended SSDT Subsystem State.
pub struct ExtendedKernelState {
    pub processes: HashMap<u32, ExtendedProcessInfo>,
    pub threads: HashMap<u32, ExtendedThreadInfo>,
    pub handle_to_tid: HashMap<usize, u32>,
    pub handle_to_sec_id: HashMap<usize, usize>,
    pub sections: HashMap<usize, SectionObject>,
    pub events: HashMap<usize, EventObject>,
    pub files: HashMap<usize, ExtendedFileInfo>,
}

impl ExtendedKernelState {
    pub fn new() -> Self {
        Self {
            processes: HashMap::new(),
            threads: HashMap::new(),
            handle_to_tid: HashMap::new(),
            handle_to_sec_id: HashMap::new(),
            sections: HashMap::new(),
            events: HashMap::new(),
            files: HashMap::new(),
        }
    }

    pub fn get_or_create_process(&mut self, pid: u32, name: &str) -> &mut ExtendedProcessInfo {
        self.processes.entry(pid).or_insert_with(|| ExtendedProcessInfo {
            pid,
            parent_pid: 0,
            name: name.to_string(),
            vm_regions: Vec::new(),
            next_vm_alloc: 0x00000001_50000000,
            is_terminated: false,
            exit_code: 0,
        })
    }
}

/// Singleton instance for Extended NT Kernel State.
static EXTENDED_STATE: OnceLock<Mutex<ExtendedKernelState>> = OnceLock::new();
static NEXT_PID: AtomicU32 = AtomicU32::new(2000);
static NEXT_TID: AtomicU32 = AtomicU32::new(3000);
static NEXT_SECTION_ID: AtomicUsize = AtomicUsize::new(100);

pub fn get_extended_state() -> &'static Mutex<ExtendedKernelState> {
    EXTENDED_STATE.get_or_init(|| Mutex::new(ExtendedKernelState::new()))
}

// ============================================================================
// Safe User Pointer Primitives
// ============================================================================

/// Safely copy a typed struct to a user-provided memory pointer.
pub unsafe fn write_user<T: Copy>(ptr: u64, val: &T) -> bool {
    if ptr == 0 {
        return false;
    }
    let target = ptr as *mut T;
    core::ptr::write_unaligned(target, *val);
    true
}

/// Safely read a typed struct from a user-provided memory pointer.
pub unsafe fn read_user<T: Copy>(ptr: u64) -> Option<T> {
    if ptr == 0 {
        return None;
    }
    let target = ptr as *const T;
    Some(core::ptr::read_unaligned(target))
}

/// Safely copy a slice of bytes into user memory.
pub unsafe fn write_user_bytes(ptr: u64, src: &[u8]) -> bool {
    if ptr == 0 {
        return false;
    }
    let dst = ptr as *mut u8;
    core::ptr::copy_nonoverlapping(src.as_ptr(), dst, src.len());
    true
}

/// Safely read an array of values from user space.
pub unsafe fn read_user_slice<T: Copy>(ptr: u64, count: usize) -> Option<Vec<T>> {
    if ptr == 0 || count == 0 {
        return None;
    }
    let src = ptr as *const T;
    let mut vec = Vec::with_capacity(count);
    for i in 0..count {
        vec.push(core::ptr::read_unaligned(src.add(i)));
    }
    Some(vec)
}

/// Read a string from an ObjectAttributes pointer or direct string pointer.
pub unsafe fn read_object_name(object_attributes_ptr: u64) -> Option<String> {
    if object_attributes_ptr == 0 {
        return None;
    }

    let oa = read_user::<ObjectAttributes>(object_attributes_ptr)?;
    if oa.object_name == 0 {
        return None;
    }

    let us = read_user::<UnicodeString>(oa.object_name)?;
    if us.buffer == 0 || us.length == 0 {
        return None;
    }

    // Attempt to read UTF-16 characters
    let num_chars = (us.length / 2) as usize;
    if let Some(utf16_chars) = read_user_slice::<u16>(us.buffer, num_chars) {
        return String::from_utf16(&utf16_chars).ok();
    }

    None
}

// ============================================================================
// Group 1: File I/O System Services
// ============================================================================

/// NtCreateFile: Creates or opens a file or device.
pub fn nt_create_file(
    proc: &mut HybridProcess,
    file_handle_ptr: u64,
    desired_access: u32,
    object_attributes_ptr: u64,
    io_status_block_ptr: u64,
    _allocation_size_ptr: u64,
    _file_attributes: u32,
    _share_access: u32,
    create_disposition: u32,
    _create_options: u32,
) -> NtStatus {
    if file_handle_ptr == 0 {
        return STATUS_INVALID_PARAMETER;
    }

    let file_path = unsafe { read_object_name(object_attributes_ptr) }
        .unwrap_or_else(|| "C:\\HybridOS\\VirtualDisk\\file.bin".to_string());

    let obj = KernelObject {
        obj_type: ObjectType::File {
            name: file_path.clone(),
            offset: 0,
        },
        access_mask: desired_access,
    };
    let handle = proc.allocate_handle(obj);

    unsafe {
        let _ = write_user::<usize>(file_handle_ptr, &handle.0);
    }

    let is_created = create_disposition == FILE_CREATE || create_disposition == FILE_SUPERSEDE;
    let action = if is_created { FILE_CREATED } else { FILE_OPENED };

    if io_status_block_ptr != 0 {
        let isb = IoStatusBlock {
            status: 0,
            information: action,
        };
        unsafe {
            let _ = write_user::<IoStatusBlock>(io_status_block_ptr, &isb);
        }
    }

    // Register in extended file state
    let mut state = get_extended_state().lock().unwrap();
    state.files.insert(
        handle.0,
        ExtendedFileInfo {
            handle_id: handle.0,
            path: file_path,
            offset: 0,
            size: 0,
            access: desired_access,
            content: Vec::new(),
        },
    );

    STATUS_SUCCESS
}

/// NtOpenFile: Opens an existing file object.
pub fn nt_open_file(
    proc: &mut HybridProcess,
    file_handle_ptr: u64,
    desired_access: u32,
    object_attributes_ptr: u64,
    io_status_block_ptr: u64,
    _share_access: u32,
    _open_options: u32,
) -> NtStatus {
    if file_handle_ptr == 0 {
        return STATUS_INVALID_PARAMETER;
    }

    let file_path = unsafe { read_object_name(object_attributes_ptr) }
        .unwrap_or_else(|| "C:\\HybridOS\\System32\\ntdll.dll".to_string());

    let obj = KernelObject {
        obj_type: ObjectType::File {
            name: file_path.clone(),
            offset: 0,
        },
        access_mask: desired_access,
    };
    let handle = proc.allocate_handle(obj);

    unsafe {
        let _ = write_user::<usize>(file_handle_ptr, &handle.0);
    }

    if io_status_block_ptr != 0 {
        let isb = IoStatusBlock {
            status: 0,
            information: FILE_OPENED,
        };
        unsafe {
            let _ = write_user::<IoStatusBlock>(io_status_block_ptr, &isb);
        }
    }

    let mut state = get_extended_state().lock().unwrap();
    state.files.insert(
        handle.0,
        ExtendedFileInfo {
            handle_id: handle.0,
            path: file_path,
            offset: 0,
            size: 1024,
            access: desired_access,
            content: vec![0u8; 1024],
        },
    );

    STATUS_SUCCESS
}

/// NtReadFile: Reads data from an open file or device handle.
pub fn nt_read_file(
    proc: &mut HybridProcess,
    file_handle: Handle,
    event: Handle,
    _apc_routine: u64,
    _apc_context: u64,
    io_status_block_ptr: u64,
    buffer_ptr: u64,
    length: u32,
    _byte_offset_ptr: u64,
    _key_ptr: u64,
) -> NtStatus {
    if !proc.handles.contains_key(&file_handle) {
        return STATUS_INVALID_HANDLE;
    }

    if length > 0 && buffer_ptr == 0 {
        return STATUS_INVALID_PARAMETER;
    }

    let mut bytes_read = 0usize;
    if buffer_ptr != 0 && length > 0 {
        let mut state = get_extended_state().lock().unwrap();
        if let Some(file) = state.files.get_mut(&file_handle.0) {
            let available = if (file.offset as usize) < file.content.len() {
                file.content.len() - (file.offset as usize)
            } else {
                0
            };
            let to_read = (length as usize).min(available);
            if to_read > 0 {
                let start = file.offset as usize;
                let slice = &file.content[start..start + to_read];
                unsafe {
                    let _ = write_user_bytes(buffer_ptr, slice);
                }
                file.offset += to_read as u64;
                bytes_read = to_read;
            } else {
                // If content is empty/simulated, generate mock data
                let mock_data = vec![0xCCu8; length as usize];
                unsafe {
                    let _ = write_user_bytes(buffer_ptr, &mock_data);
                }
                bytes_read = length as usize;
                file.offset += length as u64;
            }
        } else {
            // Virtual handle / console stdin
            bytes_read = length as usize;
        }
    }

    if io_status_block_ptr != 0 {
        let isb = IoStatusBlock {
            status: 0,
            information: bytes_read,
        };
        unsafe {
            let _ = write_user::<IoStatusBlock>(io_status_block_ptr, &isb);
        }
    }

    // Signal completion event if provided
    if event != Handle::INVALID && event.0 != 0 {
        let _ = nt_set_event(proc, event, 0);
    }

    STATUS_SUCCESS
}

/// NtWriteFile: Writes data to an open file, console, or device handle.
pub fn nt_write_file(
    proc: &mut HybridProcess,
    file_handle: Handle,
    event: Handle,
    _apc_routine: u64,
    _apc_context: u64,
    io_status_block_ptr: u64,
    buffer_ptr: u64,
    length: u32,
    _byte_offset_ptr: u64,
    _key_ptr: u64,
) -> NtStatus {
    if !proc.handles.contains_key(&file_handle) {
        return STATUS_INVALID_HANDLE;
    }

    if length > 0 && buffer_ptr == 0 {
        return STATUS_INVALID_PARAMETER;
    }

    let mut bytes_written = length as usize;

    // Handle console output redirection
    if file_handle == Handle::STDOUT || file_handle == Handle::STDERR {
        if buffer_ptr != 0 {
            if length > 0 {
                if let Some(bytes) = unsafe { read_user_slice::<u8>(buffer_ptr, length as usize) } {
                    let text = String::from_utf8_lossy(&bytes);
                    print!("[Win32 Console OUT]: {}", text);
                }
            } else {
                let cstr = unsafe { std::ffi::CStr::from_ptr(buffer_ptr as *const i8) };
                print!("[Win32 Console OUT]: {}", cstr.to_string_lossy());
            }
        }
    } else {
        // Regular file write
        if buffer_ptr != 0 && length > 0 {
            if let Some(bytes) = unsafe { read_user_slice::<u8>(buffer_ptr, length as usize) } {
                let mut state = get_extended_state().lock().unwrap();
                if let Some(file) = state.files.get_mut(&file_handle.0) {
                    file.content.extend_from_slice(&bytes);
                    file.offset += bytes.len() as u64;
                    file.size = file.content.len() as u64;
                    bytes_written = bytes.len();
                }
            }
        }
    }

    if io_status_block_ptr != 0 {
        let isb = IoStatusBlock {
            status: 0,
            information: bytes_written,
        };
        unsafe {
            let _ = write_user::<IoStatusBlock>(io_status_block_ptr, &isb);
        }
    }

    // Signal completion event if provided
    if event != Handle::INVALID && event.0 != 0 {
        let _ = nt_set_event(proc, event, 0);
    }

    STATUS_SUCCESS
}

/// NtClose: Closes any open kernel object handle.
pub fn nt_close(proc: &mut HybridProcess, handle: Handle) -> NtStatus {
    if proc.close_handle(handle) {
        let mut state = get_extended_state().lock().unwrap();
        state.files.remove(&handle.0);
        state.events.remove(&handle.0);
        state.sections.remove(&handle.0);
        state.handle_to_tid.remove(&handle.0);
        state.handle_to_sec_id.remove(&handle.0);
        STATUS_SUCCESS
    } else {
        STATUS_INVALID_HANDLE
    }
}

/// NtQueryInformationFile: Queries detailed metadata and status of an open file.
pub fn nt_query_information_file(
    proc: &HybridProcess,
    file_handle: Handle,
    io_status_block_ptr: u64,
    file_info_ptr: u64,
    length: u32,
    file_information_class: u32,
) -> NtStatus {
    let obj_arc = match proc.get_object(file_handle) {
        Some(o) => o,
        None => return STATUS_INVALID_HANDLE,
    };

    if file_info_ptr == 0 {
        return STATUS_INVALID_PARAMETER;
    }

    let obj = obj_arc.lock().unwrap();
    let (name, offset) = match &obj.obj_type {
        ObjectType::File { name, offset } => (name.clone(), *offset),
        _ => ("VirtualDevice".to_string(), 0),
    };

    let written_bytes;

    match file_information_class {
        FILE_STANDARD_INFORMATION => {
            if (length as usize) < std::mem::size_of::<FileStandardInformation>() {
                return STATUS_INFO_LENGTH_MISMATCH;
            }
            let info = FileStandardInformation {
                allocation_size: 4096,
                end_of_file: 4096,
                number_of_links: 1,
                delete_pending: 0,
                directory: 0,
                _pad: [0, 0],
            };
            unsafe {
                let _ = write_user::<FileStandardInformation>(file_info_ptr, &info);
            }
            written_bytes = std::mem::size_of::<FileStandardInformation>();
        }

        FILE_BASIC_INFORMATION => {
            if (length as usize) < std::mem::size_of::<FileBasicInformation>() {
                return STATUS_INFO_LENGTH_MISMATCH;
            }
            let info = FileBasicInformation {
                creation_time: 132000000000000000,
                last_access_time: 132000000000000000,
                last_write_time: 132000000000000000,
                change_time: 132000000000000000,
                file_attributes: 0x80, // FILE_ATTRIBUTE_NORMAL
                _pad: 0,
            };
            unsafe {
                let _ = write_user::<FileBasicInformation>(file_info_ptr, &info);
            }
            written_bytes = std::mem::size_of::<FileBasicInformation>();
        }

        FILE_POSITION_INFORMATION => {
            if (length as usize) < std::mem::size_of::<FilePositionInformation>() {
                return STATUS_INFO_LENGTH_MISMATCH;
            }
            let info = FilePositionInformation {
                current_byte_offset: offset as i64,
            };
            unsafe {
                let _ = write_user::<FilePositionInformation>(file_info_ptr, &info);
            }
            written_bytes = std::mem::size_of::<FilePositionInformation>();
        }

        FILE_NAME_INFORMATION => {
            let name_bytes = name.as_bytes();
            let needed = 4 + name_bytes.len();
            if (length as usize) < 4 {
                return STATUS_INFO_LENGTH_MISMATCH;
            }
            unsafe {
                let _ = write_user::<u32>(file_info_ptr, &(name_bytes.len() as u32));
                if (length as usize) >= needed {
                    let _ = write_user_bytes(file_info_ptr + 4, name_bytes);
                    written_bytes = needed;
                } else {
                    written_bytes = 4;
                }
            }
        }

        _ => {
            // Default blank response for unhandled file classes
            let zero_buf = vec![0u8; length as usize];
            unsafe {
                let _ = write_user_bytes(file_info_ptr, &zero_buf);
            }
            written_bytes = length as usize;
        }
    }

    if io_status_block_ptr != 0 {
        let isb = IoStatusBlock {
            status: 0,
            information: written_bytes,
        };
        unsafe {
            let _ = write_user::<IoStatusBlock>(io_status_block_ptr, &isb);
        }
    }

    STATUS_SUCCESS
}

/// NtDeviceIoControlFile: Sends an IOCTL directly to a device driver or subsystem.
pub fn nt_device_io_control_file(
    proc: &mut HybridProcess,
    file_handle: Handle,
    event: Handle,
    _apc_routine: u64,
    _apc_context: u64,
    io_status_block_ptr: u64,
    _io_control_code: u32,
    _input_buffer_ptr: u64,
    _input_buffer_length: u32,
    output_buffer_ptr: u64,
    output_buffer_length: u32,
) -> NtStatus {
    if !proc.handles.contains_key(&file_handle) {
        return STATUS_INVALID_HANDLE;
    }

    // Zero out output buffer if requested
    if output_buffer_ptr != 0 && output_buffer_length > 0 {
        let zero_buf = vec![0u8; output_buffer_length as usize];
        unsafe {
            let _ = write_user_bytes(output_buffer_ptr, &zero_buf);
        }
    }

    if io_status_block_ptr != 0 {
        let isb = IoStatusBlock {
            status: 0,
            information: output_buffer_length as usize,
        };
        unsafe {
            let _ = write_user::<IoStatusBlock>(io_status_block_ptr, &isb);
        }
    }

    if event != Handle::INVALID && event.0 != 0 {
        let _ = nt_set_event(proc, event, 0);
    }

    STATUS_SUCCESS
}

// ============================================================================
// Group 2: Virtual Memory System Services
// ============================================================================

/// NtAllocateVirtualMemory: Reserves, commits, or alters virtual address regions.
pub fn nt_allocate_virtual_memory(
    proc: &mut HybridProcess,
    _process_handle: Handle,
    base_address_ptr: u64,
    _zero_bits: usize,
    region_size_ptr: u64,
    allocation_type: u32,
    protect: u32,
) -> NtStatus {
    if base_address_ptr == 0 || region_size_ptr == 0 {
        return STATUS_INVALID_PARAMETER;
    }

    let raw_size = match unsafe { read_user::<usize>(region_size_ptr) } {
        Some(s) if s > 0 => s,
        _ => return STATUS_INVALID_PARAMETER,
    };

    // Page align up (4096-byte boundaries)
    let aligned_size = (raw_size + 4095) & !4095;
    let mut base = match unsafe { read_user::<u64>(base_address_ptr) } {
        Some(b) => b & !4095,
        None => 0,
    };

    let mut state = get_extended_state().lock().unwrap();
    let proc_info = state.get_or_create_process(proc.pid, &proc.name);

    if base == 0 {
        base = proc_info.next_vm_alloc;
        proc_info.next_vm_alloc += (aligned_size + 0x10000) as u64; // Add guard gap
    }

    proc_info.vm_regions.push(VmRegion {
        base,
        size: aligned_size,
        protect,
        state: if allocation_type & MEM_COMMIT != 0 { MEM_COMMIT } else { MEM_RESERVE },
        is_mapped: false,
    });

    unsafe {
        let _ = write_user::<u64>(base_address_ptr, &base);
        let _ = write_user::<usize>(region_size_ptr, &aligned_size);
    }

    STATUS_SUCCESS
}

/// NtFreeVirtualMemory: Frees or decommits a region of virtual memory.
pub fn nt_free_virtual_memory(
    proc: &mut HybridProcess,
    _process_handle: Handle,
    base_address_ptr: u64,
    region_size_ptr: u64,
    free_type: u32,
) -> NtStatus {
    if base_address_ptr == 0 || region_size_ptr == 0 {
        return STATUS_INVALID_PARAMETER;
    }

    let base = match unsafe { read_user::<u64>(base_address_ptr) } {
        Some(b) => b,
        None => return STATUS_INVALID_PARAMETER,
    };

    let mut state = get_extended_state().lock().unwrap();
    let proc_info = state.get_or_create_process(proc.pid, &proc.name);

    let mut freed_size = 0usize;
    if free_type & MEM_RELEASE != 0 {
        proc_info.vm_regions.retain(|r| {
            if r.base == base {
                freed_size = r.size;
                false
            } else {
                true
            }
        });
    } else {
        // MEM_DECOMMIT
        for r in &mut proc_info.vm_regions {
            if r.base == base {
                r.state = MEM_RESERVE;
                freed_size = r.size;
                break;
            }
        }
    }

    unsafe {
        let _ = write_user::<usize>(region_size_ptr, &freed_size);
        let _ = write_user::<u64>(base_address_ptr, &0u64);
    }

    STATUS_SUCCESS
}

/// NtProtectVirtualMemory: Changes memory protection flags on virtual address space.
pub fn nt_protect_virtual_memory(
    proc: &mut HybridProcess,
    _process_handle: Handle,
    base_address_ptr: u64,
    region_size_ptr: u64,
    new_protect: u32,
    old_protect_ptr: u64,
) -> NtStatus {
    if base_address_ptr == 0 || region_size_ptr == 0 {
        return STATUS_INVALID_PARAMETER;
    }

    let base = match unsafe { read_user::<u64>(base_address_ptr) } {
        Some(b) => b,
        None => return STATUS_INVALID_PARAMETER,
    };

    let mut state = get_extended_state().lock().unwrap();
    let proc_info = state.get_or_create_process(proc.pid, &proc.name);

    let mut old_protect = PAGE_READWRITE;
    for r in &mut proc_info.vm_regions {
        if r.base == base || (base >= r.base && base < r.base + r.size as u64) {
            old_protect = r.protect;
            r.protect = new_protect;
            break;
        }
    }

    if old_protect_ptr != 0 {
        unsafe {
            let _ = write_user::<u32>(old_protect_ptr, &old_protect);
        }
    }

    STATUS_SUCCESS
}

/// NtQueryVirtualMemory: Retrieves information about a range of pages in address space.
pub fn nt_query_virtual_memory(
    proc: &HybridProcess,
    _process_handle: Handle,
    base_address: u64,
    memory_information_class: u32,
    memory_info_ptr: u64,
    memory_info_length: usize,
    return_length_ptr: u64,
) -> NtStatus {
    if memory_info_ptr == 0 {
        return STATUS_INVALID_PARAMETER;
    }

    if memory_information_class != 0 {
        return STATUS_INVALID_PARAMETER; // Only MemoryBasicInformation supported
    }

    if memory_info_length < std::mem::size_of::<MemoryBasicInformation>() {
        return STATUS_INFO_LENGTH_MISMATCH;
    }

    let state = get_extended_state().lock().unwrap();
    let proc_info = state.processes.get(&proc.pid);

    let mut found_region = None;
    if let Some(p) = proc_info {
        for r in &p.vm_regions {
            if base_address >= r.base && base_address < r.base + r.size as u64 {
                found_region = Some(r.clone());
                break;
            }
        }
    }

    let mbi = match found_region {
        Some(r) => MemoryBasicInformation {
            base_address: r.base,
            allocation_base: r.base,
            allocation_protect: r.protect,
            partition_id: 0,
            _pad: 0,
            region_size: r.size,
            state: r.state,
            protect: r.protect,
            mem_type: if r.is_mapped { MEM_MAPPED } else { MEM_PRIVATE },
        },
        None => MemoryBasicInformation {
            base_address: base_address & !4095,
            allocation_base: 0,
            allocation_protect: 0,
            partition_id: 0,
            _pad: 0,
            region_size: 0x10000,
            state: MEM_FREE,
            protect: PAGE_NOACCESS,
            mem_type: 0,
        },
    };

    unsafe {
        let _ = write_user::<MemoryBasicInformation>(memory_info_ptr, &mbi);
        if return_length_ptr != 0 {
            let _ = write_user::<usize>(return_length_ptr, &std::mem::size_of::<MemoryBasicInformation>());
        }
    }

    STATUS_SUCCESS
}

/// NtCreateSection: Creates a section object for memory mapping and IPC sharing.
pub fn nt_create_section(
    proc: &mut HybridProcess,
    section_handle_ptr: u64,
    desired_access: u32,
    object_attributes_ptr: u64,
    maximum_size_ptr: u64,
    section_page_protection: u32,
    _allocation_attributes: u32,
    _file_handle: Handle,
) -> NtStatus {
    if section_handle_ptr == 0 {
        return STATUS_INVALID_PARAMETER;
    }

    let section_name = unsafe { read_object_name(object_attributes_ptr) };
    let size = if maximum_size_ptr != 0 {
        unsafe { read_user::<u64>(maximum_size_ptr).unwrap_or(0x10000) as usize }
    } else {
        0x10000 // 64 KB default
    };

    let sec_id = NEXT_SECTION_ID.fetch_add(1, Ordering::SeqCst);
    let section = SectionObject {
        id: sec_id,
        name: section_name.clone(),
        size,
        protect: section_page_protection,
        data: vec![0u8; size],
    };

    let obj = KernelObject {
        obj_type: ObjectType::File {
            name: section_name.unwrap_or_else(|| format!("Section_{}", sec_id)),
            offset: size as u64,
        },
        access_mask: desired_access,
    };
    let handle = proc.allocate_handle(obj);

    let mut state = get_extended_state().lock().unwrap();
    state.sections.insert(sec_id, section);
    state.handle_to_sec_id.insert(handle.0, sec_id);

    unsafe {
        let _ = write_user::<usize>(section_handle_ptr, &handle.0);
    }

    STATUS_SUCCESS
}

/// NtMapViewOfSection: Maps a view of a section into the virtual address space.
pub fn nt_map_view_of_section(
    proc: &mut HybridProcess,
    section_handle: Handle,
    _process_handle: Handle,
    base_address_ptr: u64,
    _zero_bits: usize,
    _commit_size: usize,
    _section_offset_ptr: u64,
    view_size_ptr: u64,
    _inherit_disposition: u32,
    _allocation_type: u32,
    win32_protect: u32,
) -> NtStatus {
    if !proc.handles.contains_key(&section_handle) {
        return STATUS_INVALID_HANDLE;
    }

    if base_address_ptr == 0 || view_size_ptr == 0 {
        return STATUS_INVALID_PARAMETER;
    }

    let mut view_size = match unsafe { read_user::<usize>(view_size_ptr) } {
        Some(s) if s > 0 => s,
        _ => 0x10000,
    };
    view_size = (view_size + 4095) & !4095;

    let mut base = match unsafe { read_user::<u64>(base_address_ptr) } {
        Some(b) => b & !4095,
        None => 0,
    };

    let mut state = get_extended_state().lock().unwrap();
    let proc_info = state.get_or_create_process(proc.pid, &proc.name);

    if base == 0 {
        base = proc_info.next_vm_alloc;
        proc_info.next_vm_alloc += (view_size + 0x10000) as u64;
    }

    proc_info.vm_regions.push(VmRegion {
        base,
        size: view_size,
        protect: win32_protect,
        state: MEM_COMMIT,
        is_mapped: true,
    });

    unsafe {
        let _ = write_user::<u64>(base_address_ptr, &base);
        let _ = write_user::<usize>(view_size_ptr, &view_size);
    }

    STATUS_SUCCESS
}

/// NtUnmapViewOfSection: Unmaps a view of a section from the process address space.
pub fn nt_unmap_view_of_section(
    proc: &mut HybridProcess,
    _process_handle: Handle,
    base_address: u64,
) -> NtStatus {
    let mut state = get_extended_state().lock().unwrap();
    let proc_info = state.get_or_create_process(proc.pid, &proc.name);

    let initial_count = proc_info.vm_regions.len();
    proc_info.vm_regions.retain(|r| r.base != base_address || !r.is_mapped);

    if proc_info.vm_regions.len() < initial_count {
        STATUS_SUCCESS
    } else {
        STATUS_INVALID_PARAMETER
    }
}

// ============================================================================
// Group 3: Process & Thread System Services
// ============================================================================

/// NtCreateProcessEx: Creates a new user-mode execution process.
pub fn nt_create_process_ex(
    proc: &mut HybridProcess,
    process_handle_ptr: u64,
    desired_access: u32,
    object_attributes_ptr: u64,
    _parent_process: Handle,
    _flags: u32,
    _section_handle: Handle,
    _debug_port: Handle,
    _exception_port: Handle,
    _job_member_level: u32,
) -> NtStatus {
    if process_handle_ptr == 0 {
        return STATUS_INVALID_PARAMETER;
    }

    let proc_name = unsafe { read_object_name(object_attributes_ptr) }
        .unwrap_or_else(|| "child_process.exe".to_string());

    let new_pid = NEXT_PID.fetch_add(1, Ordering::SeqCst);

    let mut state = get_extended_state().lock().unwrap();
    state.processes.insert(
        new_pid,
        ExtendedProcessInfo {
            pid: new_pid,
            parent_pid: proc.pid,
            name: proc_name,
            vm_regions: Vec::new(),
            next_vm_alloc: 0x00000001_50000000,
            is_terminated: false,
            exit_code: 0,
        },
    );

    let obj = KernelObject {
        obj_type: ObjectType::Process { pid: new_pid },
        access_mask: desired_access,
    };
    let handle = proc.allocate_handle(obj);

    unsafe {
        let _ = write_user::<usize>(process_handle_ptr, &handle.0);
    }

    STATUS_SUCCESS
}

/// NtCreateThreadEx: Creates an executable thread within a target process.
pub fn nt_create_thread_ex(
    proc: &mut HybridProcess,
    thread_handle_ptr: u64,
    desired_access: u32,
    _object_attributes_ptr: u64,
    _process_handle: Handle,
    start_routine: u64,
    argument: u64,
    create_flags: u32,
    _zero_bits: usize,
    _stack_size: usize,
    _maximum_stack_size: usize,
    _attribute_list_ptr: u64,
) -> NtStatus {
    if thread_handle_ptr == 0 {
        return STATUS_INVALID_PARAMETER;
    }

    let new_tid = NEXT_TID.fetch_add(1, Ordering::SeqCst);
    let suspended = (create_flags & THREAD_CREATE_FLAGS_CREATE_SUSPENDED) != 0;

    let obj = KernelObject {
        obj_type: ObjectType::Process { pid: new_tid },
        access_mask: desired_access,
    };
    let handle = proc.allocate_handle(obj);

    let mut state = get_extended_state().lock().unwrap();
    state.threads.insert(
        new_tid,
        ExtendedThreadInfo {
            tid: new_tid,
            pid: proc.pid,
            start_routine,
            argument,
            suspended,
            suspend_count: if suspended { 1 } else { 0 },
            terminated: false,
            exit_code: 0,
        },
    );
    state.handle_to_tid.insert(handle.0, new_tid);

    unsafe {
        let _ = write_user::<usize>(thread_handle_ptr, &handle.0);
    }

    STATUS_SUCCESS
}

/// NtTerminateProcess: Terminates the specified process and all its threads.
pub fn nt_terminate_process(
    proc: &mut HybridProcess,
    process_handle: Handle,
    exit_status: i32,
) -> NtStatus {
    // Current process pseudo-handle (-1) or 0
    if process_handle == Handle(usize::MAX) || process_handle.0 == 0 || process_handle.0 == proc.pid as usize {
        proc.is_terminated = true;
        proc.exit_code = exit_status;

        let mut state = get_extended_state().lock().unwrap();
        if let Some(p) = state.processes.get_mut(&proc.pid) {
            p.is_terminated = true;
            p.exit_code = exit_status;
        }
        return STATUS_SUCCESS;
    }

    if let Some(obj_arc) = proc.get_object(process_handle) {
        let obj = obj_arc.lock().unwrap();
        if let ObjectType::Process { pid } = obj.obj_type {
            let mut state = get_extended_state().lock().unwrap();
            if let Some(p) = state.processes.get_mut(&pid) {
                p.is_terminated = true;
                p.exit_code = exit_status;
            }
            return STATUS_SUCCESS;
        }
    }

    STATUS_INVALID_HANDLE
}

/// NtTerminateThread: Terminates the target thread.
pub fn nt_terminate_thread(
    proc: &mut HybridProcess,
    thread_handle: Handle,
    exit_status: i32,
) -> NtStatus {
    if thread_handle == Handle(usize::MAX) || thread_handle.0 == 0 {
        return STATUS_SUCCESS;
    }

    let mut state = get_extended_state().lock().unwrap();
    let tid = state.handle_to_tid.get(&thread_handle.0).copied().unwrap_or(thread_handle.0 as u32);
    if let Some(t) = state.threads.get_mut(&tid) {
        t.terminated = true;
        t.exit_code = exit_status;
        return STATUS_SUCCESS;
    }

    if proc.handles.contains_key(&thread_handle) {
        return STATUS_SUCCESS;
    }

    STATUS_INVALID_HANDLE
}

/// NtSuspendThread: Increments thread suspend count and suspends scheduling.
pub fn nt_suspend_thread(
    proc: &mut HybridProcess,
    thread_handle: Handle,
    previous_suspend_count_ptr: u64,
) -> NtStatus {
    if !proc.handles.contains_key(&thread_handle) {
        return STATUS_INVALID_HANDLE;
    }

    let mut state = get_extended_state().lock().unwrap();
    let tid = state.handle_to_tid.get(&thread_handle.0).copied().unwrap_or(thread_handle.0 as u32);

    if let Some(t) = state.threads.get_mut(&tid) {
        let prev = t.suspend_count;
        t.suspend_count += 1;
        t.suspended = true;

        if previous_suspend_count_ptr != 0 {
            unsafe {
                let _ = write_user::<u32>(previous_suspend_count_ptr, &prev);
            }
        }
        STATUS_SUCCESS
    } else {
        // Fallback for thread handles created externally
        if previous_suspend_count_ptr != 0 {
            unsafe {
                let _ = write_user::<u32>(previous_suspend_count_ptr, &0);
            }
        }
        STATUS_SUCCESS
    }
}

/// NtResumeThread: Decrements thread suspend count and wakes thread if 0.
pub fn nt_resume_thread(
    proc: &mut HybridProcess,
    thread_handle: Handle,
    previous_suspend_count_ptr: u64,
) -> NtStatus {
    if !proc.handles.contains_key(&thread_handle) {
        return STATUS_INVALID_HANDLE;
    }

    let mut state = get_extended_state().lock().unwrap();
    let tid = state.handle_to_tid.get(&thread_handle.0).copied().unwrap_or(thread_handle.0 as u32);

    if let Some(t) = state.threads.get_mut(&tid) {
        let prev = t.suspend_count;
        if t.suspend_count > 0 {
            t.suspend_count -= 1;
            if t.suspend_count == 0 {
                t.suspended = false;
            }
        }

        if previous_suspend_count_ptr != 0 {
            unsafe {
                let _ = write_user::<u32>(previous_suspend_count_ptr, &prev);
            }
        }
        STATUS_SUCCESS
    } else {
        if previous_suspend_count_ptr != 0 {
            unsafe {
                let _ = write_user::<u32>(previous_suspend_count_ptr, &1);
            }
        }
        STATUS_SUCCESS
    }
}

// ============================================================================
// Group 4: Synchronization System Services
// ============================================================================

/// NtCreateEvent: Creates a notification (manual) or synchronization (auto) event.
pub fn nt_create_event(
    proc: &mut HybridProcess,
    event_handle_ptr: u64,
    desired_access: u32,
    _object_attributes_ptr: u64,
    event_type: u32,
    initial_state: u8,
) -> NtStatus {
    if event_handle_ptr == 0 {
        return STATUS_INVALID_PARAMETER;
    }

    let manual_reset = event_type == NOTIFICATION_EVENT;
    let signaled = initial_state != 0;

    let obj = KernelObject {
        obj_type: ObjectType::Event { signaled },
        access_mask: desired_access,
    };
    let handle = proc.allocate_handle(obj);

    let mut state = get_extended_state().lock().unwrap();
    state.events.insert(
        handle.0,
        EventObject {
            manual_reset,
            signaled,
            waiters_count: 0,
        },
    );

    unsafe {
        let _ = write_user::<usize>(event_handle_ptr, &handle.0);
    }

    STATUS_SUCCESS
}

/// NtSetEvent: Sets an event object to the signaled state.
pub fn nt_set_event(
    proc: &mut HybridProcess,
    event_handle: Handle,
    previous_state_ptr: u64,
) -> NtStatus {
    let obj_arc = match proc.get_object(event_handle) {
        Some(o) => o,
        None => return STATUS_INVALID_HANDLE,
    };

    let mut obj = obj_arc.lock().unwrap();
    if let ObjectType::Event { ref mut signaled } = obj.obj_type {
        let prev = if *signaled { 1i32 } else { 0i32 };
        *signaled = true;

        let mut state = get_extended_state().lock().unwrap();
        if let Some(e) = state.events.get_mut(&event_handle.0) {
            e.signaled = true;
        }

        if previous_state_ptr != 0 {
            unsafe {
                let _ = write_user::<i32>(previous_state_ptr, &prev);
            }
        }
        STATUS_SUCCESS
    } else {
        STATUS_INVALID_HANDLE
    }
}

/// NtResetEvent: Sets an event object to the non-signaled state.
pub fn nt_reset_event(
    proc: &mut HybridProcess,
    event_handle: Handle,
    previous_state_ptr: u64,
) -> NtStatus {
    let obj_arc = match proc.get_object(event_handle) {
        Some(o) => o,
        None => return STATUS_INVALID_HANDLE,
    };

    let mut obj = obj_arc.lock().unwrap();
    if let ObjectType::Event { ref mut signaled } = obj.obj_type {
        let prev = if *signaled { 1i32 } else { 0i32 };
        *signaled = false;

        let mut state = get_extended_state().lock().unwrap();
        if let Some(e) = state.events.get_mut(&event_handle.0) {
            e.signaled = false;
        }

        if previous_state_ptr != 0 {
            unsafe {
                let _ = write_user::<i32>(previous_state_ptr, &prev);
            }
        }
        STATUS_SUCCESS
    } else {
        STATUS_INVALID_HANDLE
    }
}

/// NtWaitForSingleObject: Waits until the object satisfies the wait or timeout expires.
pub fn nt_wait_for_single_object(
    proc: &mut HybridProcess,
    handle: Handle,
    _alertable: u8,
    timeout_ptr: u64,
) -> NtStatus {
    let obj_arc = match proc.get_object(handle) {
        Some(o) => o,
        None => return STATUS_INVALID_HANDLE,
    };

    let mut obj = obj_arc.lock().unwrap();
    match obj.obj_type {
        ObjectType::Event { ref mut signaled } => {
            if *signaled {
                let mut state = get_extended_state().lock().unwrap();
                let is_auto_reset = state
                    .events
                    .get(&handle.0)
                    .map(|e| !e.manual_reset)
                    .unwrap_or(false);

                if is_auto_reset {
                    *signaled = false;
                    if let Some(e) = state.events.get_mut(&handle.0) {
                        e.signaled = false;
                    }
                }
                STATUS_WAIT_0
            } else if timeout_ptr != 0 && unsafe { read_user::<i64>(timeout_ptr) == Some(0) } {
                STATUS_TIMEOUT
            } else {
                STATUS_WAIT_0
            }
        }

        ObjectType::Process { pid } => {
            let state = get_extended_state().lock().unwrap();
            let is_term = state.processes.get(&pid).map(|p| p.is_terminated).unwrap_or(false);
            if is_term {
                STATUS_WAIT_0
            } else if timeout_ptr != 0 && unsafe { read_user::<i64>(timeout_ptr) == Some(0) } {
                STATUS_TIMEOUT
            } else {
                STATUS_WAIT_0
            }
        }

        _ => STATUS_WAIT_0,
    }
}

/// NtWaitForMultipleObjects: Waits until one or all objects are signaled.
pub fn nt_wait_for_multiple_objects(
    proc: &mut HybridProcess,
    count: u32,
    handles_ptr: u64,
    wait_type: u32,
    _alertable: u8,
    timeout_ptr: u64,
) -> NtStatus {
    if handles_ptr == 0 || count == 0 || count > 64 {
        return STATUS_INVALID_PARAMETER;
    }

    let handles = match unsafe { read_user_slice::<usize>(handles_ptr, count as usize) } {
        Some(h) => h,
        None => return STATUS_INVALID_PARAMETER,
    };

    if wait_type == WAIT_ANY {
        for (i, &raw_handle) in handles.iter().enumerate() {
            let h = Handle(raw_handle);
            if let Some(obj_arc) = proc.get_object(h) {
                let mut obj = obj_arc.lock().unwrap();
                if let ObjectType::Event { ref mut signaled } = obj.obj_type {
                    if *signaled {
                        let mut state = get_extended_state().lock().unwrap();
                        let is_auto = state.events.get(&raw_handle).map(|e| !e.manual_reset).unwrap_or(false);
                        if is_auto {
                            *signaled = false;
                            if let Some(e) = state.events.get_mut(&raw_handle) {
                                e.signaled = false;
                            }
                        }
                        return NtStatus(i as i32); // STATUS_WAIT_0 + i
                    }
                }
            }
        }

        if timeout_ptr != 0 && unsafe { read_user::<i64>(timeout_ptr) == Some(0) } {
            STATUS_TIMEOUT
        } else {
            STATUS_WAIT_0
        }
    } else {
        // WAIT_ALL: Check that every handle is signaled
        let mut all_signaled = true;
        for &raw_handle in &handles {
            let h = Handle(raw_handle);
            if let Some(obj_arc) = proc.get_object(h) {
                let obj = obj_arc.lock().unwrap();
                if let ObjectType::Event { signaled } = obj.obj_type {
                    if !signaled {
                        all_signaled = false;
                        break;
                    }
                }
            } else {
                return STATUS_INVALID_HANDLE;
            }
        }

        if all_signaled {
            for &raw_handle in &handles {
                let h = Handle(raw_handle);
                if let Some(obj_arc) = proc.get_object(h) {
                    let mut obj = obj_arc.lock().unwrap();
                    if let ObjectType::Event { ref mut signaled } = obj.obj_type {
                        let mut state = get_extended_state().lock().unwrap();
                        let is_auto = state.events.get(&raw_handle).map(|e| !e.manual_reset).unwrap_or(false);
                        if is_auto {
                            *signaled = false;
                            if let Some(e) = state.events.get_mut(&raw_handle) {
                                e.signaled = false;
                            }
                        }
                    }
                }
            }
            STATUS_WAIT_0
        } else if timeout_ptr != 0 && unsafe { read_user::<i64>(timeout_ptr) == Some(0) } {
            STATUS_TIMEOUT
        } else {
            STATUS_WAIT_0
        }
    }
}

// ============================================================================
// Group 5: System Information & Execution Services
// ============================================================================

/// NtQuerySystemInformation: Returns global kernel stats, performance, and processor data.
pub fn nt_query_system_information(
    system_information_class: u32,
    system_info_ptr: u64,
    system_info_length: u32,
    return_length_ptr: u64,
) -> NtStatus {
    if system_info_ptr == 0 {
        return STATUS_INVALID_PARAMETER;
    }

    let written_bytes;

    match system_information_class {
        SYSTEM_BASIC_INFORMATION => {
            if (system_info_length as usize) < std::mem::size_of::<SystemBasicInformation>() {
                return STATUS_INFO_LENGTH_MISMATCH;
            }
            let info = SystemBasicInformation {
                reserved: 0,
                timer_resolution: 156250, // 15.625 ms in 100ns units
                page_size: 4096,
                number_of_physical_pages: 1048576, // 4 GB RAM
                lowest_physical_page_number: 1,
                highest_physical_page_number: 1048576,
                allocation_granularity: 65536, // 64 KB
                minimum_user_mode_address: 0x00010000,
                maximum_user_mode_address: 0x00007FFFFFFEFFFF,
                active_processors_affinity_mask: 0x000000FF, // 8 CPU cores
                number_of_processors: 8,
                _pad: [0; 7],
            };
            unsafe {
                let _ = write_user::<SystemBasicInformation>(system_info_ptr, &info);
            }
            written_bytes = std::mem::size_of::<SystemBasicInformation>();
        }

        SYSTEM_PROCESSOR_INFORMATION => {
            if (system_info_length as usize) < std::mem::size_of::<SystemProcessorInformation>() {
                return STATUS_INFO_LENGTH_MISMATCH;
            }
            let info = SystemProcessorInformation {
                processor_architecture: 9, // PROCESSOR_ARCHITECTURE_AMD64
                processor_level: 6,
                processor_revision: 0x9E09,
                maximum_processors: 8,
                processor_feature_bits: 0x7FFFFFFF,
            };
            unsafe {
                let _ = write_user::<SystemProcessorInformation>(system_info_ptr, &info);
            }
            written_bytes = std::mem::size_of::<SystemProcessorInformation>();
        }

        SYSTEM_PERFORMANCE_INFORMATION => {
            if (system_info_length as usize) < std::mem::size_of::<SystemPerformanceInformation>() {
                return STATUS_INFO_LENGTH_MISMATCH;
            }
            let info = SystemPerformanceInformation {
                idle_process_time: 1250000000,
                io_read_transfer_count: 512000,
                io_write_transfer_count: 256000,
                io_other_transfer_count: 128000,
                io_read_operation_count: 1024,
                io_write_operation_count: 512,
                io_other_operation_count: 256,
                available_pages: 800000,
                committed_pages: 248576,
                commit_limit: 1048576,
                peak_commitment: 300000,
            };
            unsafe {
                let _ = write_user::<SystemPerformanceInformation>(system_info_ptr, &info);
            }
            written_bytes = std::mem::size_of::<SystemPerformanceInformation>();
        }

        SYSTEM_TIME_OF_DAY_INFORMATION => {
            if (system_info_length as usize) < std::mem::size_of::<SystemTimeOfDayInformation>() {
                return STATUS_INFO_LENGTH_MISMATCH;
            }
            let info = SystemTimeOfDayInformation {
                boot_time: 132000000000000000,
                current_time: 132000050000000000,
                time_zone_bias: -180 * 60 * 10000000, // UTC-3 in 100ns
                time_zone_id: 1,
                reserved: 0,
                boot_time_bias: 0,
                sleep_time_bias: 0,
            };
            unsafe {
                let _ = write_user::<SystemTimeOfDayInformation>(system_info_ptr, &info);
            }
            written_bytes = std::mem::size_of::<SystemTimeOfDayInformation>();
        }

        _ => {
            let zero_buf = vec![0u8; system_info_length as usize];
            unsafe {
                let _ = write_user_bytes(system_info_ptr, &zero_buf);
            }
            written_bytes = system_info_length as usize;
        }
    }

    if return_length_ptr != 0 {
        unsafe {
            let _ = write_user::<u32>(return_length_ptr, &(written_bytes as u32));
        }
    }

    STATUS_SUCCESS
}

/// NtDelayExecution: Suspends the execution of the current thread for the specified duration.
pub fn nt_delay_execution(
    _alertable: u8,
    delay_interval_ptr: u64,
) -> NtStatus {
    if delay_interval_ptr == 0 {
        return STATUS_INVALID_PARAMETER;
    }

    let interval = match unsafe { read_user::<i64>(delay_interval_ptr) } {
        Some(i) => i,
        None => return STATUS_INVALID_PARAMETER,
    };

    // If interval < 0, it represents relative 100-nanosecond units
    if interval < 0 {
        let abs_units = interval.unsigned_abs();
        let nanos = abs_units * 100;
        let millis = nanos / 1_000_000;
        if millis > 0 {
            std::thread::sleep(std::time::Duration::from_millis(millis.min(50)));
        } else {
            std::thread::yield_now();
        }
    } else {
        std::thread::yield_now();
    }

    STATUS_SUCCESS
}

// ============================================================================
// Extended Windows NT SSDT Dispatcher
// ============================================================================

/// Dispatches extended Windows NT System Service calls to appropriate handlers.
pub fn dispatch_extended_windows_syscall(
    proc: &mut HybridProcess,
    ssdt_index: u64,
    a1: u64,
    a2: u64,
    a3: u64,
    a4: u64,
    a5: u64,
    a6: u64,
) -> NtStatus {
    match ssdt_index {
        // Group 1: File I/O
        SYS_NT_CREATE_FILE => {
            nt_create_file(
                proc,
                a1,
                a2 as u32,
                a3,
                a4,
                a5,
                0,
                0,
                (a6 & 0xFFFFFFFF) as u32,
                0,
            )
        }

        SYS_NT_OPEN_FILE => {
            nt_open_file(
                proc,
                a1,
                a2 as u32,
                a3,
                a4,
                a5 as u32,
                a6 as u32,
            )
        }

        SYS_NT_READ_FILE => {
            nt_read_file(
                proc,
                Handle(a1 as usize),
                Handle(a2 as usize),
                0,
                0,
                a4,
                a6,
                a5 as u32,
                0,
                0,
            )
        }

        SYS_NT_WRITE_FILE => {
            nt_write_file(
                proc,
                Handle(a1 as usize),
                Handle(a2 as usize),
                0,
                0,
                a4,
                a6,
                a5 as u32,
                0,
                0,
            )
        }

        SYS_NT_CLOSE => {
            nt_close(proc, Handle(a1 as usize))
        }

        SYS_NT_QUERY_INFORMATION_FILE => {
            nt_query_information_file(
                proc,
                Handle(a1 as usize),
                a2,
                a3,
                a4 as u32,
                a5 as u32,
            )
        }

        SYS_NT_DEVICE_IO_CONTROL_FILE => {
            nt_device_io_control_file(
                proc,
                Handle(a1 as usize),
                Handle(a2 as usize),
                0,
                0,
                a3,
                a4 as u32,
                a5,
                0,
                a6,
                0,
            )
        }

        // Group 2: Virtual Memory
        SYS_NT_ALLOCATE_VIRTUAL_MEMORY => {
            nt_allocate_virtual_memory(
                proc,
                Handle(a1 as usize),
                a2,
                a3 as usize,
                a4,
                a5 as u32,
                a6 as u32,
            )
        }

        SYS_NT_FREE_VIRTUAL_MEMORY => {
            nt_free_virtual_memory(
                proc,
                Handle(a1 as usize),
                a2,
                a3,
                a4 as u32,
            )
        }

        SYS_NT_PROTECT_VIRTUAL_MEMORY => {
            nt_protect_virtual_memory(
                proc,
                Handle(a1 as usize),
                a2,
                a3,
                a4 as u32,
                a5,
            )
        }

        SYS_NT_QUERY_VIRTUAL_MEMORY => {
            nt_query_virtual_memory(
                proc,
                Handle(a1 as usize),
                a2,
                a3 as u32,
                a4,
                a5 as usize,
                a6,
            )
        }

        SYS_NT_CREATE_SECTION => {
            nt_create_section(
                proc,
                a1,
                a2 as u32,
                a3,
                a4,
                a5 as u32,
                a6 as u32,
                Handle::INVALID,
            )
        }

        SYS_NT_MAP_VIEW_OF_SECTION => {
            nt_map_view_of_section(
                proc,
                Handle(a1 as usize),
                Handle(a2 as usize),
                a3,
                0,
                0,
                0,
                a5,
                0,
                0,
                a6 as u32,
            )
        }

        SYS_NT_UNMAP_VIEW_OF_SECTION => {
            nt_unmap_view_of_section(
                proc,
                Handle(a1 as usize),
                a2,
            )
        }

        // Group 3: Process & Thread
        SYS_NT_CREATE_PROCESS_EX => {
            nt_create_process_ex(
                proc,
                a1,
                a2 as u32,
                a3,
                Handle(a4 as usize),
                a5 as u32,
                Handle(a6 as usize),
                Handle::INVALID,
                Handle::INVALID,
                0,
            )
        }

        SYS_NT_CREATE_THREAD_EX => {
            nt_create_thread_ex(
                proc,
                a1,
                a2 as u32,
                a3,
                Handle(a4 as usize),
                a5,
                a6,
                0,
                0,
                0,
                0,
                0,
            )
        }

        SYS_NT_TERMINATE_PROCESS => {
            nt_terminate_process(
                proc,
                Handle(a1 as usize),
                a2 as i32,
            )
        }

        SYS_NT_TERMINATE_THREAD => {
            nt_terminate_thread(
                proc,
                Handle(a1 as usize),
                a2 as i32,
            )
        }

        SYS_NT_SUSPEND_THREAD => {
            nt_suspend_thread(
                proc,
                Handle(a1 as usize),
                a2,
            )
        }

        SYS_NT_RESUME_THREAD => {
            nt_resume_thread(
                proc,
                Handle(a1 as usize),
                a2,
            )
        }

        // Group 4: Synchronization
        SYS_NT_CREATE_EVENT => {
            nt_create_event(
                proc,
                a1,
                a2 as u32,
                a3,
                a4 as u32,
                a5 as u8,
            )
        }

        SYS_NT_SET_EVENT => {
            nt_set_event(
                proc,
                Handle(a1 as usize),
                a2,
            )
        }

        SYS_NT_RESET_EVENT => {
            nt_reset_event(
                proc,
                Handle(a1 as usize),
                a2,
            )
        }

        SYS_NT_WAIT_FOR_SINGLE_OBJECT => {
            nt_wait_for_single_object(
                proc,
                Handle(a1 as usize),
                a2 as u8,
                a3,
            )
        }

        SYS_NT_WAIT_FOR_MULTIPLE_OBJECTS => {
            nt_wait_for_multiple_objects(
                proc,
                a1 as u32,
                a2,
                a3 as u32,
                a4 as u8,
                a5,
            )
        }

        // Group 5: System Information & Execution
        SYS_NT_QUERY_SYSTEM_INFORMATION => {
            nt_query_system_information(
                a1 as u32,
                a2,
                a3 as u32,
                a4,
            )
        }

        SYS_NT_DELAY_EXECUTION => {
            nt_delay_execution(
                a1 as u8,
                a2,
            )
        }

        _ => {
            if ssdt_index >= 0x1000 {
                super::win32k::dispatch_win32k_syscall(
                    proc, ssdt_index, a1, a2, a3, a4, a5, a6,
                )
            } else {
                println!("[Win32 SSDT Ext] Unhandled SSDT syscall: 0x{:04X}", ssdt_index);
                STATUS_NOT_IMPLEMENTED
            }
        }
    }
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::personality::ExecutionPersonality;

    fn make_test_proc() -> HybridProcess {
        HybridProcess::new(1005, "test_win_app.exe", ExecutionPersonality::WindowsNt)
    }

    #[test]
    fn test_file_io_subsystem() {
        let mut proc = make_test_proc();

        // 1. NtCreateFile
        let mut file_handle = 0usize;
        let mut isb = IoStatusBlock::default();
        let status = nt_create_file(
            &mut proc,
            &mut file_handle as *mut usize as u64,
            0x10000000,
            0,
            &mut isb as *mut IoStatusBlock as u64,
            0,
            0,
            0,
            FILE_CREATE,
            0,
        );
        assert_eq!(status, STATUS_SUCCESS);
        assert_ne!(file_handle, 0);
        assert_eq!(isb.information, FILE_CREATED);

        let handle = Handle(file_handle);

        // 2. NtWriteFile
        let test_data = b"HybridOS Extended Windows NT File I/O Test Content";
        let mut write_isb = IoStatusBlock::default();
        let status = nt_write_file(
            &mut proc,
            handle,
            Handle::INVALID,
            0,
            0,
            &mut write_isb as *mut IoStatusBlock as u64,
            test_data.as_ptr() as u64,
            test_data.len() as u32,
            0,
            0,
        );
        assert_eq!(status, STATUS_SUCCESS);
        assert_eq!(write_isb.information, test_data.len());

        // 3. NtQueryInformationFile
        let mut std_info = FileStandardInformation::default();
        let mut query_isb = IoStatusBlock::default();
        let status = nt_query_information_file(
            &proc,
            handle,
            &mut query_isb as *mut IoStatusBlock as u64,
            &mut std_info as *mut FileStandardInformation as u64,
            std::mem::size_of::<FileStandardInformation>() as u32,
            FILE_STANDARD_INFORMATION,
        );
        assert_eq!(status, STATUS_SUCCESS);
        assert_eq!(query_isb.information, std::mem::size_of::<FileStandardInformation>());

        // 4. NtClose
        let status = nt_close(&mut proc, handle);
        assert_eq!(status, STATUS_SUCCESS);
        assert_eq!(proc.handles.contains_key(&handle), false);
    }

    #[test]
    fn test_virtual_memory_subsystem() {
        let mut proc = make_test_proc();

        // 1. NtAllocateVirtualMemory
        let mut base = 0u64;
        let mut size = 8192usize;
        let status = nt_allocate_virtual_memory(
            &mut proc,
            Handle::INVALID,
            &mut base as *mut u64 as u64,
            0,
            &mut size as *mut usize as u64,
            MEM_COMMIT,
            PAGE_READWRITE,
        );
        assert_eq!(status, STATUS_SUCCESS);
        assert_ne!(base, 0);
        assert_eq!(size, 8192);

        // 2. NtQueryVirtualMemory
        let mut mbi = MemoryBasicInformation::default();
        let mut return_len = 0usize;
        let status = nt_query_virtual_memory(
            &proc,
            Handle::INVALID,
            base,
            0,
            &mut mbi as *mut MemoryBasicInformation as u64,
            std::mem::size_of::<MemoryBasicInformation>(),
            &mut return_len as *mut usize as u64,
        );
        assert_eq!(status, STATUS_SUCCESS);
        assert_eq!(mbi.base_address, base);
        assert_eq!(mbi.region_size, 8192);
        assert_eq!(mbi.protect, PAGE_READWRITE);

        // 3. NtProtectVirtualMemory
        let mut old_protect = 0u32;
        let status = nt_protect_virtual_memory(
            &mut proc,
            Handle::INVALID,
            &mut base as *mut u64 as u64,
            &mut size as *mut usize as u64,
            PAGE_EXECUTE_READWRITE,
            &mut old_protect as *mut u32 as u64,
        );
        assert_eq!(status, STATUS_SUCCESS);
        assert_eq!(old_protect, PAGE_READWRITE);

        // 4. NtCreateSection and NtMapViewOfSection
        let mut sec_handle = 0usize;
        let mut max_size = 65536u64;
        let status = nt_create_section(
            &mut proc,
            &mut sec_handle as *mut usize as u64,
            SECTION_ALL_ACCESS,
            0,
            &mut max_size as *mut u64 as u64,
            PAGE_READWRITE,
            0,
            Handle::INVALID,
        );
        assert_eq!(status, STATUS_SUCCESS);
        assert_ne!(sec_handle, 0);

        let mut map_base = 0u64;
        let mut view_size = 65536usize;
        let status = nt_map_view_of_section(
            &mut proc,
            Handle(sec_handle),
            Handle::INVALID,
            &mut map_base as *mut u64 as u64,
            0,
            0,
            0,
            &mut view_size as *mut usize as u64,
            0,
            0,
            PAGE_READWRITE,
        );
        assert_eq!(status, STATUS_SUCCESS);
        assert_ne!(map_base, 0);

        // 5. NtUnmapViewOfSection
        let status = nt_unmap_view_of_section(&mut proc, Handle::INVALID, map_base);
        assert_eq!(status, STATUS_SUCCESS);

        // 6. NtFreeVirtualMemory
        let status = nt_free_virtual_memory(
            &mut proc,
            Handle::INVALID,
            &mut base as *mut u64 as u64,
            &mut size as *mut usize as u64,
            MEM_RELEASE,
        );
        assert_eq!(status, STATUS_SUCCESS);
    }

    #[test]
    fn test_process_thread_lifecycle() {
        let mut proc = make_test_proc();

        // 1. NtCreateProcessEx
        let mut proc_handle = 0usize;
        let status = nt_create_process_ex(
            &mut proc,
            &mut proc_handle as *mut usize as u64,
            0x10000000,
            0,
            Handle::INVALID,
            0,
            Handle::INVALID,
            Handle::INVALID,
            Handle::INVALID,
            0,
        );
        assert_eq!(status, STATUS_SUCCESS);
        assert_ne!(proc_handle, 0);

        // 2. NtCreateThreadEx
        let mut thread_handle = 0usize;
        let status = nt_create_thread_ex(
            &mut proc,
            &mut thread_handle as *mut usize as u64,
            0x10000000,
            0,
            Handle(proc_handle),
            0x140001000,
            0x140002000,
            THREAD_CREATE_FLAGS_CREATE_SUSPENDED,
            0,
            0,
            0,
            0,
        );
        assert_eq!(status, STATUS_SUCCESS);
        assert_ne!(thread_handle, 0);

        // 3. NtResumeThread & NtSuspendThread
        let mut prev_count = 0u32;
        let status = nt_resume_thread(
            &mut proc,
            Handle(thread_handle),
            &mut prev_count as *mut u32 as u64,
        );
        assert_eq!(status, STATUS_SUCCESS);
        assert_eq!(prev_count, 1);

        let status = nt_suspend_thread(
            &mut proc,
            Handle(thread_handle),
            &mut prev_count as *mut u32 as u64,
        );
        assert_eq!(status, STATUS_SUCCESS);
        assert_eq!(prev_count, 0);

        // 4. NtTerminateThread & NtTerminateProcess
        let status = nt_terminate_thread(&mut proc, Handle(thread_handle), 0);
        assert_eq!(status, STATUS_SUCCESS);

        let status = nt_terminate_process(&mut proc, Handle(proc_handle), 42);
        assert_eq!(status, STATUS_SUCCESS);
    }

    #[test]
    fn test_synchronization_primitives() {
        let mut proc = make_test_proc();

        // 1. NtCreateEvent (Manual Reset, Initial State: false)
        let mut ev_handle = 0usize;
        let status = nt_create_event(
            &mut proc,
            &mut ev_handle as *mut usize as u64,
            0x10000000,
            0,
            NOTIFICATION_EVENT,
            0,
        );
        assert_eq!(status, STATUS_SUCCESS);
        assert_ne!(ev_handle, 0);

        let handle = Handle(ev_handle);

        // 2. NtWaitForSingleObject with timeout 0 should timeout
        let mut timeout = 0i64;
        let status = nt_wait_for_single_object(
            &mut proc,
            handle,
            0,
            &mut timeout as *mut i64 as u64,
        );
        assert_eq!(status, STATUS_TIMEOUT);

        // 3. NtSetEvent
        let mut prev_state = 0i32;
        let status = nt_set_event(&mut proc, handle, &mut prev_state as *mut i32 as u64);
        assert_eq!(status, STATUS_SUCCESS);
        assert_eq!(prev_state, 0);

        // 4. NtWaitForSingleObject should now succeed
        let status = nt_wait_for_single_object(&mut proc, handle, 0, 0);
        assert_eq!(status, STATUS_WAIT_0);

        // 5. NtResetEvent
        let status = nt_reset_event(&mut proc, handle, &mut prev_state as *mut i32 as u64);
        assert_eq!(status, STATUS_SUCCESS);
        assert_eq!(prev_state, 1);

        // 6. NtWaitForMultipleObjects
        let mut ev2_handle = 0usize;
        let _ = nt_create_event(&mut proc, &mut ev2_handle as *mut usize as u64, 0x10000000, 0, NOTIFICATION_EVENT, 1);
        let handles = [ev_handle, ev2_handle];

        let status = nt_wait_for_multiple_objects(
            &mut proc,
            2,
            handles.as_ptr() as u64,
            WAIT_ANY,
            0,
            0,
        );
        assert_eq!(status, NtStatus(1)); // Second event is signaled
    }

    #[test]
    fn test_system_services() {
        // 1. NtQuerySystemInformation
        let mut sbi = SystemBasicInformation::default();
        let mut ret_len = 0u32;
        let status = nt_query_system_information(
            SYSTEM_BASIC_INFORMATION,
            &mut sbi as *mut SystemBasicInformation as u64,
            std::mem::size_of::<SystemBasicInformation>() as u32,
            &mut ret_len as *mut u32 as u64,
        );
        assert_eq!(status, STATUS_SUCCESS);
        assert_eq!(sbi.page_size, 4096);
        assert_eq!(sbi.number_of_processors, 8);

        // 2. NtDelayExecution
        let mut interval = -10000i64; // 1 ms relative
        let status = nt_delay_execution(0, &mut interval as *mut i64 as u64);
        assert_eq!(status, STATUS_SUCCESS);
    }

    #[test]
    fn test_ssdt_dispatcher_routing() {
        let mut proc = make_test_proc();

        // Dispatch NtQuerySystemInformation via SSDT
        let mut sbi = SystemBasicInformation::default();
        let mut ret_len = 0u32;
        let status = dispatch_extended_windows_syscall(
            &mut proc,
            SYS_NT_QUERY_SYSTEM_INFORMATION,
            SYSTEM_BASIC_INFORMATION as u64,
            &mut sbi as *mut SystemBasicInformation as u64,
            std::mem::size_of::<SystemBasicInformation>() as u64,
            &mut ret_len as *mut u32 as u64,
            0,
            0,
        );
        assert_eq!(status, STATUS_SUCCESS);
        assert_eq!(sbi.page_size, 4096);

        // Dispatch NtClose with invalid handle
        let status = dispatch_extended_windows_syscall(
            &mut proc,
            SYS_NT_CLOSE,
            99999,
            0,
            0,
            0,
            0,
            0,
        );
        assert_eq!(status, STATUS_INVALID_HANDLE);

        // Dispatch Win32k service >= 0x1000 fallback routing
        let status_desktop = dispatch_extended_windows_syscall(
            &mut proc,
            crate::syscall::win32k::SYS_NT_USER_GET_DESKTOP_WINDOW,
            0,
            0,
            0,
            0,
            0,
            0,
        );
        assert!(status_desktop.is_success());
        assert_eq!(status_desktop.0, crate::syscall::win32k::HWND::DESKTOP.0 as i32);
    }
}
