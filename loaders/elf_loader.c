#include "loaders/elf_loader.h"
#include <stdio.h>
#include <string.h>

NTSTATUS elf_load_image(hybrid_process_t* proc, const u8* file_buffer, size_t file_size) {
    if (!proc || !file_buffer || file_size < sizeof(Elf64_Ehdr)) {
        return STATUS_INVALID_PARAMETER;
    }

    const Elf64_Ehdr* ehdr = (const Elf64_Ehdr*)file_buffer;

    /* Verify ELF Magic */
    if (ehdr->e_ident[0] != ELFMAG0 || ehdr->e_ident[1] != ELFMAG1 ||
        ehdr->e_ident[2] != ELFMAG2 || ehdr->e_ident[3] != ELFMAG3) {
        printf("[ELF Loader] Invalid ELF magic signature\n");
        return STATUS_INVALID_PARAMETER;
    }

    if (ehdr->e_ident[4] != ELFCLASS64) {
        printf("[ELF Loader] Only 64-bit ELF binaries are supported (class: %d)\n", ehdr->e_ident[4]);
        return STATUS_NOT_IMPLEMENTED;
    }

    if (ehdr->e_machine != EM_X86_64) {
        printf("[ELF Loader] Unsupported architecture: %d (expected x86_64: 62)\n", ehdr->e_machine);
        return STATUS_NOT_IMPLEMENTED;
    }

    proc->entry_point = ehdr->e_entry;

    printf("[ELF Loader] Loaded Linux ELF64 Binary:\n");
    printf("            Type:             %s\n", ehdr->e_type == ET_EXEC ? "EXEC (Executable)" : 
                                              ehdr->e_type == ET_DYN  ? "DYN (PIE / Shared Lib)" : "Other");
    printf("            Entry Point:      0x%016llX\n", (unsigned long long)proc->entry_point);
    printf("            Program Headers:  %u (offset 0x%llX)\n", ehdr->e_phnum, (unsigned long long)ehdr->e_phoff);

    /* Enumerate Program Headers */
    if (ehdr->e_phoff + (ehdr->e_phnum * sizeof(Elf64_Phdr)) <= file_size) {
        const Elf64_Phdr* phdr = (const Elf64_Phdr*)(file_buffer + ehdr->e_phoff);
        size_t total_mem_size = 0;
        virt_addr_t min_vaddr = (virt_addr_t)-1;

        for (u16 i = 0; i < ehdr->e_phnum; i++) {
            if (phdr[i].p_type == PT_LOAD) {
                if (phdr[i].p_vaddr < min_vaddr) {
                    min_vaddr = phdr[i].p_vaddr;
                }
                total_mem_size += phdr[i].p_memsz;

                printf("            - Segment PT_LOAD: VAddr=0x%016llX, FileSz=0x%llX, MemSz=0x%llX, Flags=[%c%c%c]\n",
                       (unsigned long long)phdr[i].p_vaddr,
                       (unsigned long long)phdr[i].p_filesz,
                       (unsigned long long)phdr[i].p_memsz,
                       (phdr[i].p_flags & PF_R) ? 'R' : '-',
                       (phdr[i].p_flags & PF_W) ? 'W' : '-',
                       (phdr[i].p_flags & PF_X) ? 'X' : '-');
            }
        }
        proc->image_base = (min_vaddr == (virt_addr_t)-1) ? 0 : min_vaddr;
        proc->image_size = total_mem_size;
    }

    return STATUS_SUCCESS;
}
