#ifndef HYBRIDOS_LINUX_SUBSYSTEM_H
#define HYBRIDOS_LINUX_SUBSYSTEM_H

#include "kernel/types.h"
#include "kernel/syscall.h"

void linux_subsystem_init(void);

u64 linux_sys_read(u64 fd, u64 buf, u64 count, u64 a4, u64 a5, u64 a6);
u64 linux_sys_write(u64 fd, u64 buf, u64 count, u64 a4, u64 a5, u64 a6);
u64 linux_sys_open(u64 filename, u64 flags, u64 mode, u64 a4, u64 a5, u64 a6);
u64 linux_sys_close(u64 fd, u64 a2, u64 a3, u64 a4, u64 a5, u64 a6);
u64 linux_sys_mmap(u64 addr, u64 length, u64 prot, u64 flags, u64 fd, u64 offset);
u64 linux_sys_munmap(u64 addr, u64 length, u64 a3, u64 a4, u64 a5, u64 a6);
u64 linux_sys_getpid(u64 a1, u64 a2, u64 a3, u64 a4, u64 a5, u64 a6);
u64 linux_sys_exit(u64 error_code, u64 a2, u64 a3, u64 a4, u64 a5, u64 a6);

#endif /* HYBRIDOS_LINUX_SUBSYSTEM_H */
