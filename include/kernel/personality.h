#ifndef HYBRIDOS_PERSONALITY_H
#define HYBRIDOS_PERSONALITY_H

/*
 * Execution personality defines which system semantics,
 * register calling convention, and system call table are applied
 * to a given process or thread.
 */
typedef enum execution_personality {
    PERSONALITY_NATIVE_HYBRID = 0,
    PERSONALITY_WINDOWS       = 1, /* Microsoft x64 ABI, SSDT dispatch, NT objects */
    PERSONALITY_LINUX         = 2, /* System V AMD64 ABI, POSIX syscalls, VFS FDs */
    PERSONALITY_ANDROID       = 3  /* Linux + Android Binder/HAL extensions */
} execution_personality_t;

static inline const char* personality_to_string(execution_personality_t p) {
    switch (p) {
        case PERSONALITY_NATIVE_HYBRID: return "Hybrid-Native";
        case PERSONALITY_WINDOWS:       return "Windows NT (x64)";
        case PERSONALITY_LINUX:         return "Linux POSIX (ELF64)";
        case PERSONALITY_ANDROID:       return "Android (Bionic/HAL)";
        default:                        return "Unknown";
    }
}

#endif /* HYBRIDOS_PERSONALITY_H */
