#include "kernel/process.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static u32 g_next_pid = 1000;
hybrid_process_t* g_current_process = NULL;
hybrid_thread_t*  g_current_thread = NULL;

hybrid_process_t* process_create(const char* name, execution_personality_t personality) {
    hybrid_process_t* proc = (hybrid_process_t*)calloc(1, sizeof(hybrid_process_t));
    if (!proc) {
        return NULL;
    }

    proc->pid = g_next_pid++;
    proc->personality = personality;
    strncpy(proc->name, name ? name : "hybrid_task", sizeof(proc->name) - 1);

    /*
     * Standard POSIX file descriptors (0 = stdin, 1 = stdout, 2 = stderr)
     * are initialized automatically. In Windows personality, these also map
     * to Standard Handle values:
     * STD_INPUT_HANDLE  = (HANDLE)0
     * STD_OUTPUT_HANDLE = (HANDLE)1
     * STD_ERROR_HANDLE  = (HANDLE)2
     */
    for (int i = 0; i < 3; i++) {
        kernel_object_t* std_obj = (kernel_object_t*)calloc(1, sizeof(kernel_object_t));
        std_obj->type = OBJ_TYPE_DEVICE;
        std_obj->ref_count = 1;

        proc->handles[i].object = std_obj;
        proc->handles[i].access_mask = 0x3; /* Read / Write */
    }

    printf("[Kernel Process] Created Process PID=%u, Name='%s', Personality='%s'\n",
           proc->pid, proc->name, personality_to_string(proc->personality));

    return proc;
}

void process_destroy(hybrid_process_t* proc) {
    if (!proc) return;

    for (int i = 0; i < MAX_HANDLES_PER_PROCESS; i++) {
        if (proc->handles[i].object) {
            proc->handles[i].object->ref_count--;
            if (proc->handles[i].object->ref_count == 0) {
                free(proc->handles[i].object);
            }
            proc->handles[i].object = NULL;
        }
    }

    free(proc);
}

i32 process_allocate_fd_or_handle(hybrid_process_t* proc, kernel_object_t* obj, u32 access) {
    if (!proc || !obj) return -1;

    for (int i = 3; i < MAX_HANDLES_PER_PROCESS; i++) {
        if (proc->handles[i].object == NULL) {
            proc->handles[i].object = obj;
            proc->handles[i].access_mask = access;
            obj->ref_count++;
            return i;
        }
    }
    return -1;
}

kernel_object_t* process_get_object(hybrid_process_t* proc, i32 handle_or_fd) {
    if (!proc || handle_or_fd < 0 || handle_or_fd >= MAX_HANDLES_PER_PROCESS) {
        return NULL;
    }
    return proc->handles[handle_or_fd].object;
}

void process_close_handle(hybrid_process_t* proc, i32 handle_or_fd) {
    if (!proc || handle_or_fd < 0 || handle_or_fd >= MAX_HANDLES_PER_PROCESS) {
        return;
    }

    kernel_object_t* obj = proc->handles[handle_or_fd].object;
    if (obj) {
        obj->ref_count--;
        if (obj->ref_count == 0) {
            free(obj);
        }
        proc->handles[handle_or_fd].object = NULL;
    }
}
