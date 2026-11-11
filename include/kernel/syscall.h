#ifndef HYBRIDOS_SYSCALL_H
#define HYBRIDOS_SYSCALL_H

#include "kernel/types.h"
#include "kernel/process.h"

/*
 * =========================================================================
 * 1. Linux x86_64 Syscall Numbers (Standard Linux System V ABI)
 * =========================================================================
 */
#define SYS_LINUX_READ                   0
#define SYS_LINUX_WRITE                  1
#define SYS_LINUX_OPEN                   2
#define SYS_LINUX_CLOSE                  3
#define SYS_LINUX_STAT                   4
#define SYS_LINUX_FSTAT                  5
#define SYS_LINUX_LSEEK                  8
#define SYS_LINUX_MMAP                   9
#define SYS_LINUX_MPROTECT              10
#define SYS_LINUX_MUNMAP                11
#define SYS_LINUX_BRK                   12
#define SYS_LINUX_IOCTL                 16
#define SYS_LINUX_GETPID                39
#define SYS_LINUX_EXIT                  60
#define SYS_LINUX_MAX                  128

/*
 * =========================================================================
 * 2. Windows x64 SSDT Indices (Windows NT 6.x / 10 compatible)
 * =========================================================================
 */
#define SYS_WIN_NT_CLOSE                0x000F
#define SYS_WIN_NT_CREATE_FILE          0x0055
#define SYS_WIN_NT_OPEN_FILE            0x0033
#define SYS_WIN_NT_READ_FILE            0x0006
#define SYS_WIN_NT_WRITE_FILE           0x0008
#define SYS_WIN_NT_ALLOCATE_VM          0x0018
#define SYS_WIN_NT_FREE_VM              0x001E
#define SYS_WIN_NT_TERMINATE_PROCESS    0x002C
#define SYS_WIN_NT_DEVICE_IO_CONTROL    0x0007
#define SYS_WIN_NT_QUERY_SYS_INFO       0x0036
#define SYS_WIN_NT_MAX                  0x0100

/* Function types for Syscall Dispatchers */
typedef u64 (*linux_syscall_fn_t)(u64 a1, u64 a2, u64 a3, u64 a4, u64 a5, u64 a6);
typedef NTSTATUS (*windows_syscall_fn_t)(u64 a1, u64 a2, u64 a3, u64 a4, u64 a5, u64 a6);

/* Subsystem tables */
extern linux_syscall_fn_t g_linux_syscall_table[SYS_LINUX_MAX];
extern windows_syscall_fn_t g_windows_syscall_table[SYS_WIN_NT_MAX];

/* Universal Dispatcher Entry Point */
void syscall_subsystems_init(void);
u64 hybrid_syscall_dispatcher(trap_frame_t* tf);

#endif /* HYBRIDOS_SYSCALL_H */
