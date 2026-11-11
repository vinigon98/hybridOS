#include "kernel/syscall.h"
#include "subsystems/win_subsystem.h"
#include "subsystems/linux_subsystem.h"
#include <stdio.h>

void syscall_subsystems_init(void) {
    win_subsystem_init();
    linux_subsystem_init();
    printf("[Syscall Router] Dual-Personality ABI Router Ready.\n");
}

u64 hybrid_syscall_dispatcher(trap_frame_t* tf) {
    if (!tf) return (u64)-1;

    execution_personality_t personality = PERSONALITY_NATIVE_HYBRID;
    if (g_current_process) {
        personality = g_current_process->personality;
    }

    u64 sys_id = tf->rax;
    u64 result = 0;

    switch (personality) {
        case PERSONALITY_WINDOWS: {
            /*
             * Microsoft x64 Syscall Calling Convention:
             * RAX: System Service Index (SSDT index)
             * R10: First parameter (RCX was clobbered by SYSCALL instruction)
             * RDX: Second parameter
             * R8:  Third parameter
             * R9:  Fourth parameter
             * RSP: Stack arguments (parameters 5 and 6)
             */
            u64 a1 = tf->r10;
            u64 a2 = tf->rdx;
            u64 a3 = tf->r8;
            u64 a4 = tf->r9;
            u64 a5 = 0;
            u64 a6 = 0;

            if (tf->rsp) {
                /* Safe read from user stack frame */
                u64* user_stack = (u64*)tf->rsp;
                a5 = user_stack[5]; /* 5th argument: [rsp + 0x28] */
                a6 = user_stack[6]; /* 6th argument: [rsp + 0x30] */
            }

            if (sys_id < SYS_WIN_NT_MAX && g_windows_syscall_table[sys_id]) {
                result = (u64)g_windows_syscall_table[sys_id](a1, a2, a3, a4, a5, a6);
            } else {
                printf("[Dispatcher] Unimplemented Windows NT syscall: 0x%04llX\n", (unsigned long long)sys_id);
                result = (u64)STATUS_NOT_IMPLEMENTED;
            }
            break;
        }

        case PERSONALITY_LINUX:
        case PERSONALITY_ANDROID: {
            /*
             * Linux System V AMD64 Syscall Calling Convention:
             * RAX: Syscall number
             * RDI: Arg 1
             * RSI: Arg 2
             * RDX: Arg 3
             * R10: Arg 4 (RCX is used for RIP by CPU)
             * R8:  Arg 5
             * R9:  Arg 6
             */
            u64 a1 = tf->rdi;
            u64 a2 = tf->rsi;
            u64 a3 = tf->rdx;
            u64 a4 = tf->r10;
            u64 a5 = tf->r8;
            u64 a6 = tf->r9;

            if (sys_id < SYS_LINUX_MAX && g_linux_syscall_table[sys_id]) {
                result = g_linux_syscall_table[sys_id](a1, a2, a3, a4, a5, a6);
            } else {
                printf("[Dispatcher] Unimplemented Linux syscall: %llu\n", (unsigned long long)sys_id);
                result = (u64)-ENOSYS;
            }
            break;
        }

        default: {
            printf("[Dispatcher] Unknown process personality: %d\n", personality);
            result = (u64)-1;
            break;
        }
    }

    tf->rax = result;
    return result;
}
