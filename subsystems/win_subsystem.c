#include "subsystems/win_subsystem.h"
#include "kernel/process.h"
#include "kernel/driver.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

windows_syscall_fn_t g_windows_syscall_table[SYS_WIN_NT_MAX];

NTSTATUS sys_NtCreateFile(u64 FileHandlePtr, u64 DesiredAccess, u64 ObjectAttributes,
                          u64 IoStatusBlock, u64 AllocationSize, u64 FileAttributes) {
    (void)ObjectAttributes; (void)IoStatusBlock; (void)AllocationSize; (void)FileAttributes;
    printf("[Win32 Subsystem] NtCreateFile(DesiredAccess=0x%llX)\n", (unsigned long long)DesiredAccess);

    if (!g_current_process || !FileHandlePtr) {
        return STATUS_INVALID_PARAMETER;
    }

    kernel_object_t* file_obj = (kernel_object_t*)calloc(1, sizeof(kernel_object_t));
    file_obj->type = OBJ_TYPE_FILE;
    file_obj->ref_count = 1;

    i32 handle = process_allocate_fd_or_handle(g_current_process, file_obj, (u32)DesiredAccess);
    if (handle < 0) {
        free(file_obj);
        return STATUS_NO_MEMORY;
    }

    *(HANDLE*)FileHandlePtr = (HANDLE)(uintptr_t)handle;
    printf("[Win32 Subsystem] -> Assigned HANDLE: 0x%X\n", handle);
    return STATUS_SUCCESS;
}

NTSTATUS sys_NtWriteFile(u64 FileHandle, u64 Event, u64 ApcRoutine,
                         u64 ApcContext, u64 IoStatusBlock, u64 Buffer) {
    (void)Event; (void)ApcRoutine; (void)ApcContext;
    i32 handle = (i32)(uintptr_t)FileHandle;
    
    if (!g_current_process) return STATUS_INVALID_HANDLE;

    kernel_object_t* obj = process_get_object(g_current_process, handle);
    if (!obj) {
        return STATUS_INVALID_HANDLE;
    }

    /* Emulate writing to stdout / console */
    if (handle == 1 || handle == 2 || obj->type == OBJ_TYPE_DEVICE) {
        const char* str = (const char*)Buffer;
        if (str) {
            printf("[Win32 Console OUT]: %s", str);
        }
    }

    if (IoStatusBlock) {
        IO_STATUS_BLOCK* iosb = (IO_STATUS_BLOCK*)IoStatusBlock;
        iosb->Status = STATUS_SUCCESS;
        iosb->Information = Buffer ? strlen((const char*)Buffer) : 0;
    }

    return STATUS_SUCCESS;
}

NTSTATUS sys_NtReadFile(u64 FileHandle, u64 Event, u64 ApcRoutine,
                        u64 ApcContext, u64 IoStatusBlock, u64 Buffer) {
    (void)Event; (void)ApcRoutine; (void)ApcContext; (void)Buffer;
    printf("[Win32 Subsystem] NtReadFile(HANDLE=0x%llX)\n", (unsigned long long)FileHandle);
    if (IoStatusBlock) {
        IO_STATUS_BLOCK* iosb = (IO_STATUS_BLOCK*)IoStatusBlock;
        iosb->Status = STATUS_SUCCESS;
        iosb->Information = 0;
    }
    return STATUS_SUCCESS;
}

NTSTATUS sys_NtAllocateVirtualMemory(u64 ProcessHandle, u64 BaseAddressPtr, u64 ZeroBits,
                                     u64 RegionSizePtr, u64 AllocationType, u64 Protect) {
    (void)ProcessHandle; (void)ZeroBits; (void)AllocationType; (void)Protect;
    if (!RegionSizePtr) return STATUS_INVALID_PARAMETER;

    size_t size = *(size_t*)RegionSizePtr;
    void* mem = malloc(size);
    if (!mem) return STATUS_NO_MEMORY;

    memset(mem, 0, size);
    if (BaseAddressPtr) {
        *(void**)BaseAddressPtr = mem;
    }

    printf("[Win32 Subsystem] NtAllocateVirtualMemory: Allocated %zu bytes at %p\n", size, mem);
    return STATUS_SUCCESS;
}

NTSTATUS sys_NtFreeVirtualMemory(u64 ProcessHandle, u64 BaseAddressPtr, u64 RegionSizePtr,
                                 u64 FreeType, u64 a5, u64 a6) {
    (void)ProcessHandle; (void)RegionSizePtr; (void)FreeType; (void)a5; (void)a6;
    if (BaseAddressPtr && *(void**)BaseAddressPtr) {
        free(*(void**)BaseAddressPtr);
        *(void**)BaseAddressPtr = NULL;
        return STATUS_SUCCESS;
    }
    return STATUS_INVALID_PARAMETER;
}

NTSTATUS sys_NtClose(u64 Handle, u64 a2, u64 a3, u64 a4, u64 a5, u64 a6) {
    (void)a2; (void)a3; (void)a4; (void)a5; (void)a6;
    if (!g_current_process) return STATUS_INVALID_HANDLE;

    process_close_handle(g_current_process, (i32)(uintptr_t)Handle);
    printf("[Win32 Subsystem] NtClose(HANDLE=0x%llX)\n", (unsigned long long)Handle);
    return STATUS_SUCCESS;
}

NTSTATUS sys_NtTerminateProcess(u64 ProcessHandle, u64 ExitStatus, u64 a3, u64 a4, u64 a5, u64 a6) {
    (void)ProcessHandle; (void)a3; (void)a4; (void)a5; (void)a6;
    if (g_current_process) {
        g_current_process->is_terminated = true;
        g_current_process->exit_code = (i32)ExitStatus;
        printf("[Win32 Subsystem] NtTerminateProcess: PID=%u terminated with status 0x%08X\n",
               g_current_process->pid, (unsigned int)ExitStatus);
    }
    return STATUS_SUCCESS;
}

void win_subsystem_init(void) {
    memset(g_windows_syscall_table, 0, sizeof(g_windows_syscall_table));

    g_windows_syscall_table[SYS_WIN_NT_CREATE_FILE]       = sys_NtCreateFile;
    g_windows_syscall_table[SYS_WIN_NT_READ_FILE]         = sys_NtReadFile;
    g_windows_syscall_table[SYS_WIN_NT_WRITE_FILE]        = sys_NtWriteFile;
    g_windows_syscall_table[SYS_WIN_NT_ALLOCATE_VM]       = sys_NtAllocateVirtualMemory;
    g_windows_syscall_table[SYS_WIN_NT_FREE_VM]           = sys_NtFreeVirtualMemory;
    g_windows_syscall_table[SYS_WIN_NT_CLOSE]             = sys_NtClose;
    g_windows_syscall_table[SYS_WIN_NT_TERMINATE_PROCESS] = sys_NtTerminateProcess;

    printf("[Kernel Init] Windows NT SSDT Subsystem initialized (%d services mapped).\n", SYS_WIN_NT_MAX);
}
