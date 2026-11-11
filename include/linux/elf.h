#ifndef HYBRIDOS_LINUX_ELF_H
#define HYBRIDOS_LINUX_ELF_H

#include "kernel/types.h"

#define EI_NIDENT       16
#define ELFMAG0         0x7F
#define ELFMAG1         'E'
#define ELFMAG2         'L'
#define ELFMAG3         'F'

#define ELFCLASS64      2
#define ELFDATA2LSB     1   /* Little-endian */
#define EV_CURRENT      1

#define ET_EXEC         2   /* Executable file */
#define ET_DYN          3   /* Shared object / Position-independent executable (PIE) */

#define EM_X86_64       62  /* AMD x86-64 architecture */

/* Segment Types */
#define PT_NULL         0
#define PT_LOAD         1
#define PT_DYNAMIC      2
#define PT_INTERP       3
#define PT_NOTE         4
#define PT_SHLIB        5
#define PT_PHDR         6
#define PT_TLS          7
#define PT_GNU_STACK    0x6474e551

/* Segment Flags */
#define PF_X            0x1 /* Executable */
#define PF_W            0x2 /* Writable */
#define PF_R            0x4 /* Readable */

typedef struct {
    u8  e_ident[EI_NIDENT];
    u16 e_type;
    u16 e_machine;
    u32 e_version;
    u64 e_entry;
    u64 e_phoff;
    u64 e_shoff;
    u32 e_flags;
    u16 e_ehsize;
    u16 e_phentsize;
    u16 e_phnum;
    u16 e_shentsize;
    u16 e_shnum;
    u16 e_shstrndx;
} Elf64_Ehdr;

typedef struct {
    u32 p_type;
    u32 p_flags;
    u64 p_offset;
    u64 p_vaddr;
    u64 p_paddr;
    u64 p_filesz;
    u64 p_memsz;
    u64 p_align;
} Elf64_Phdr;

#endif /* HYBRIDOS_LINUX_ELF_H */
