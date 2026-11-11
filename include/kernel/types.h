#ifndef HYBRIDOS_TYPES_H
#define HYBRIDOS_TYPES_H

#include <stdint.h>
#include <stddef.h>
#include <stdbool.h>
#include <errno.h>

/* Fixed-width primitive types */
typedef uint8_t   u8;
typedef uint16_t  u16;
typedef uint32_t  u32;
typedef uint64_t  u64;

typedef int8_t    i8;
typedef int16_t   i16;
typedef int32_t   i32;
typedef int64_t   i64;

typedef uintptr_t virt_addr_t;
typedef uintptr_t phys_addr_t;

/* Status codes: Windows NTSTATUS style */
typedef i32 NTSTATUS;
#define STATUS_SUCCESS                   ((NTSTATUS)0x00000000L)
#define STATUS_UNSUCCESSFUL              ((NTSTATUS)0xC0000001L)
#define STATUS_NOT_IMPLEMENTED           ((NTSTATUS)0xC0000002L)
#define STATUS_INVALID_PARAMETER         ((NTSTATUS)0xC000000DL)
#define STATUS_NO_MEMORY                 ((NTSTATUS)0xC0000017L)
#define STATUS_ACCESS_DENIED             ((NTSTATUS)0xC0000022L)
#define STATUS_INVALID_HANDLE            ((NTSTATUS)0xC0000008L)
#define STATUS_FILE_NOT_FOUND            ((NTSTATUS)0xC0000034L)

#define NT_SUCCESS(Status) (((NTSTATUS)(Status)) >= 0)

/* Status codes: POSIX style (Linux errno) */
#ifndef EPERM
#define EPERM    1
#endif
#ifndef ENOENT
#define ENOENT   2
#endif
#ifndef ESRCH
#define ESRCH    3
#endif
#ifndef EINTR
#define EINTR    4
#endif
#ifndef EIO
#define EIO      5
#endif
#ifndef ENXIO
#define ENXIO    6
#endif
#ifndef E2BIG
#define E2BIG    7
#endif
#ifndef ENOEXEC
#define ENOEXEC  8
#endif
#ifndef EBADF
#define EBADF    9
#endif
#ifndef ECHILD
#define ECHILD  10
#endif
#ifndef EAGAIN
#define EAGAIN  11
#endif
#ifndef ENOMEM
#define ENOMEM  12
#endif
#ifndef EACCES
#define EACCES  13
#endif
#ifndef EFAULT
#define EFAULT  14
#endif
#ifndef EEXIST
#define EEXIST  17
#endif
#ifndef ENODEV
#define ENODEV  19
#endif
#ifndef EINVAL
#define EINVAL  22
#endif
#ifndef ENOSYS
#define ENOSYS 38
#endif

/* Handle & Descriptor types */
typedef void* HANDLE;
#define INVALID_HANDLE_VALUE ((HANDLE)(intptr_t)-1)

#endif /* HYBRIDOS_TYPES_H */
