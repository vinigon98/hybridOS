#include "subsystems/linux_subsystem.h"
#include "kernel/process.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

linux_syscall_fn_t g_linux_syscall_table[SYS_LINUX_MAX];

u64 linux_sys_read(u64 fd, u64 buf, u64 count, u64 a4, u64 a5, u64 a6) {
    (void)a4; (void)a5; (void)a6; (void)buf; (void)count;
    printf("[Linux Subsystem] sys_read(fd=%llu, count=%llu)\n", 
           (unsigned long long)fd, (unsigned long long)count);
    return 0;
}

u64 linux_sys_write(u64 fd, u64 buf, u64 count, u64 a4, u64 a5, u64 a6) {
    (void)a4; (void)a5; (void)a6;
    if (!g_current_process) return -EBADF;

    kernel_object_t* obj = process_get_object(g_current_process, (i32)fd);
    if (!obj) {
        return -EBADF;
    }

    /* Standard output/err */
    if (fd == 1 || fd == 2) {
        const char* str = (const char*)buf;
        if (str) {
            printf("[Linux stdout/stderr]: %.*s", (int)count, str);
        }
        return count;
    }

    return count;
}

u64 linux_sys_open(u64 filename, u64 flags, u64 mode, u64 a4, u64 a5, u64 a6) {
    (void)flags; (void)mode; (void)a4; (void)a5; (void)a6;
    const char* path = (const char*)filename;
    printf("[Linux Subsystem] sys_open('%s', flags=0x%llX)\n", path ? path : "null", (unsigned long long)flags);

    if (!g_current_process || !path) {
        return -EINVAL;
    }

    kernel_object_t* file_obj = (kernel_object_t*)calloc(1, sizeof(kernel_object_t));
    file_obj->type = OBJ_TYPE_FILE;
    file_obj->ref_count = 1;

    i32 fd = process_allocate_fd_or_handle(g_current_process, file_obj, 0x3);
    if (fd < 0) {
        free(file_obj);
        return -ENOMEM;
    }

    printf("[Linux Subsystem] -> Assigned FD: %d\n", fd);
    return (u64)fd;
}

u64 linux_sys_close(u64 fd, u64 a2, u64 a3, u64 a4, u64 a5, u64 a6) {
    (void)a2; (void)a3; (void)a4; (void)a5; (void)a6;
    if (!g_current_process) return -EBADF;

    process_close_handle(g_current_process, (i32)fd);
    printf("[Linux Subsystem] sys_close(fd=%llu)\n", (unsigned long long)fd);
    return 0;
}

u64 linux_sys_mmap(u64 addr, u64 length, u64 prot, u64 flags, u64 fd, u64 offset) {
    (void)addr; (void)prot; (void)flags; (void)fd; (void)offset;
    void* mem = malloc((size_t)length);
    if (!mem) return -ENOMEM;

    memset(mem, 0, (size_t)length);
    printf("[Linux Subsystem] sys_mmap: Allocated %llu bytes at %p\n",
           (unsigned long long)length, mem);
    return (u64)(uintptr_t)mem;
}

u64 linux_sys_munmap(u64 addr, u64 length, u64 a3, u64 a4, u64 a5, u64 a6) {
    (void)length; (void)a3; (void)a4; (void)a5; (void)a6;
    if (addr) {
        free((void*)(uintptr_t)addr);
        return 0;
    }
    return -EINVAL;
}

u64 linux_sys_getpid(u64 a1, u64 a2, u64 a3, u64 a4, u64 a5, u64 a6) {
    (void)a1; (void)a2; (void)a3; (void)a4; (void)a5; (void)a6;
    return g_current_process ? (u64)g_current_process->pid : 0;
}

u64 linux_sys_exit(u64 error_code, u64 a2, u64 a3, u64 a4, u64 a5, u64 a6) {
    (void)a2; (void)a3; (void)a4; (void)a5; (void)a6;
    if (g_current_process) {
        g_current_process->is_terminated = true;
        g_current_process->exit_code = (i32)error_code;
        printf("[Linux Subsystem] sys_exit: PID=%u exited with code %llu\n",
               g_current_process->pid, (unsigned long long)error_code);
    }
    return 0;
}

void linux_subsystem_init(void) {
    memset(g_linux_syscall_table, 0, sizeof(g_linux_syscall_table));

    g_linux_syscall_table[SYS_LINUX_READ]   = linux_sys_read;
    g_linux_syscall_table[SYS_LINUX_WRITE]  = linux_sys_write;
    g_linux_syscall_table[SYS_LINUX_OPEN]   = linux_sys_open;
    g_linux_syscall_table[SYS_LINUX_CLOSE]  = linux_sys_close;
    g_linux_syscall_table[SYS_LINUX_MMAP]   = linux_sys_mmap;
    g_linux_syscall_table[SYS_LINUX_MUNMAP] = linux_sys_munmap;
    g_linux_syscall_table[SYS_LINUX_GETPID] = linux_sys_getpid;
    g_linux_syscall_table[SYS_LINUX_EXIT]   = linux_sys_exit;

    printf("[Kernel Init] Linux POSIX Subsystem initialized (%d syscalls mapped).\n", SYS_LINUX_MAX);
}
