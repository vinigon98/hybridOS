#ifndef HYBRIDOS_PROCESS_H
#define HYBRIDOS_PROCESS_H

#include "kernel/types.h"
#include "kernel/personality.h"

#define MAX_HANDLES_PER_PROCESS 256
#define MAX_THREADS_PER_PROCESS 32

/*
 * x86_64 Trap Frame saved on entry from userspace via SYSCALL or Interrupt
 */
typedef struct trap_frame {
    /* Saved General Purpose Registers */
    u64 r15;
    u64 r14;
    u64 r13;
    u64 r12;
    u64 r11; /* Saved RFLAGS on syscall */
    u64 r10; /* Used as arg4 in Linux, arg1 in Win64 */
    u64 r9;  /* Arg6 in Linux, arg4 in Win64 */
    u64 r8;  /* Arg5 in Linux, arg3 in Win64 */
    u64 rbp;
    u64 rdi; /* Arg1 in Linux */
    u64 rsi; /* Arg2 in Linux */
    u64 rdx; /* Arg3 in Linux, arg2 in Win64 */
    u64 rcx; /* Saved RIP on syscall, arg1 in Win64 calling convention */
    u64 rbx;
    u64 rax; /* Syscall ID and return value */

    /* Hardware interrupt context (if pushed by CPU or syscall trampoline) */
    u64 rip;
    u64 cs;
    u64 rflags;
    u64 rsp;
    u64 ss;
} trap_frame_t;

/*
 * Universal Kernel Object descriptor
 */
typedef enum {
    OBJ_TYPE_FREE = 0,
    OBJ_TYPE_FILE,
    OBJ_TYPE_DEVICE,
    OBJ_TYPE_PROCESS,
    OBJ_TYPE_THREAD,
    OBJ_TYPE_EVENT,
    OBJ_TYPE_MUTEX
} object_type_t;

typedef struct kernel_object {
    object_type_t type;
    u32 ref_count;
    void* internal_ptr; /* Points to vfs_node, driver_object, etc. */
} kernel_object_t;

/*
 * Hybrid Handle Entry
 * Maps interchangeably as a Windows HANDLE or a POSIX file descriptor (fd)
 */
typedef struct hybrid_handle_entry {
    kernel_object_t* object;
    u32 access_mask; /* Generic read/write/execute permissions */
    u32 flags;
} hybrid_handle_entry_t;

/*
 * Hybrid Process Structure
 */
typedef struct hybrid_process {
    u32 pid;
    execution_personality_t personality;
    char name[64];

    /* Virtual memory base and entry point */
    virt_addr_t image_base;
    virt_addr_t entry_point;
    size_t image_size;

    /* Handle & File Descriptor Table */
    hybrid_handle_entry_t handles[MAX_HANDLES_PER_PROCESS];

    /* Active threads */
    u32 thread_count;
    struct hybrid_thread* threads[MAX_THREADS_PER_PROCESS];

    /* Process state */
    bool is_terminated;
    i32 exit_code;
} hybrid_process_t;

/*
 * Hybrid Thread Structure
 */
typedef struct hybrid_thread {
    u32 tid;
    hybrid_process_t* process;
    virt_addr_t stack_base;
    virt_addr_t stack_top;
    trap_frame_t trap_frame;
    bool is_running;
} hybrid_thread_t;

/* Global kernel context */
extern hybrid_process_t* g_current_process;
extern hybrid_thread_t*  g_current_thread;

/* Function prototypes */
hybrid_process_t* process_create(const char* name, execution_personality_t personality);
void process_destroy(hybrid_process_t* proc);
i32 process_allocate_fd_or_handle(hybrid_process_t* proc, kernel_object_t* obj, u32 access);
kernel_object_t* process_get_object(hybrid_process_t* proc, i32 handle_or_fd);
void process_close_handle(hybrid_process_t* proc, i32 handle_or_fd);

#endif /* HYBRIDOS_PROCESS_H */
