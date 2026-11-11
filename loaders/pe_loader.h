#ifndef HYBRIDOS_PE_LOADER_H
#define HYBRIDOS_PE_LOADER_H

#include "win32/pe.h"
#include "kernel/process.h"

NTSTATUS pe_load_image(hybrid_process_t* proc, const u8* file_buffer, size_t file_size);

#endif /* HYBRIDOS_PE_LOADER_H */
