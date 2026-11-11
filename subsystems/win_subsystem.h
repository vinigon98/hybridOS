#ifndef HYBRIDOS_WIN_SUBSYSTEM_H
#define HYBRIDOS_WIN_SUBSYSTEM_H

#include "kernel/types.h"
#include "kernel/syscall.h"

void win_subsystem_init(void);

NTSTATUS sys_NtCreateFile(u64 FileHandlePtr, u64 DesiredAccess, u64 ObjectAttributes,
                          u64 IoStatusBlock, u64 AllocationSize, u64 FileAttributes);
NTSTATUS sys_NtReadFile(u64 FileHandle, u64 Event, u64 ApcRoutine,
                        u64 ApcContext, u64 IoStatusBlock, u64 Buffer);
NTSTATUS sys_NtWriteFile(u64 FileHandle, u64 Event, u64 ApcRoutine,
                         u64 ApcContext, u64 IoStatusBlock, u64 Buffer);
NTSTATUS sys_NtAllocateVirtualMemory(u64 ProcessHandle, u64 BaseAddressPtr, u64 ZeroBits,
                                     u64 RegionSizePtr, u64 AllocationType, u64 Protect);
NTSTATUS sys_NtFreeVirtualMemory(u64 ProcessHandle, u64 BaseAddressPtr, u64 RegionSizePtr,
                                 u64 FreeType, u64 a5, u64 a6);
NTSTATUS sys_NtClose(u64 Handle, u64 a2, u64 a3, u64 a4, u64 a5, u64 a6);
NTSTATUS sys_NtTerminateProcess(u64 ProcessHandle, u64 ExitStatus, u64 a3, u64 a4, u64 a5, u64 a6);

#endif /* HYBRIDOS_WIN_SUBSYSTEM_H */
