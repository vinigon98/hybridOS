#ifndef HYBRIDOS_ELF_LOADER_H
#define HYBRIDOS_ELF_LOADER_H

#include "linux/elf.h"
#include "kernel/process.h"

NTSTATUS elf_load_image(hybrid_process_t* proc, const u8* file_buffer, size_t file_size);

#endif /* HYBRIDOS_ELF_LOADER_H */
