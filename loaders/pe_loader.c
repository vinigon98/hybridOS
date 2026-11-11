#include "loaders/pe_loader.h"
#include <stdio.h>
#include <string.h>
#include <stdlib.h>

NTSTATUS pe_load_image(hybrid_process_t* proc, const u8* file_buffer, size_t file_size) {
    if (!proc || !file_buffer || file_size < sizeof(IMAGE_DOS_HEADER)) {
        return STATUS_INVALID_PARAMETER;
    }

    const IMAGE_DOS_HEADER* dos_header = (const IMAGE_DOS_HEADER*)file_buffer;
    if (dos_header->e_magic != IMAGE_DOS_SIGNATURE) {
        printf("[PE Loader] Invalid DOS magic: 0x%04X (expected 0x5A4D)\n", dos_header->e_magic);
        return STATUS_INVALID_PARAMETER;
    }

    if (dos_header->e_lfanew <= 0 || (size_t)dos_header->e_lfanew + sizeof(IMAGE_NT_HEADERS64) > file_size) {
        printf("[PE Loader] Invalid e_lfanew offset: 0x%X\n", dos_header->e_lfanew);
        return STATUS_INVALID_PARAMETER;
    }

    const IMAGE_NT_HEADERS64* nt_headers = (const IMAGE_NT_HEADERS64*)(file_buffer + dos_header->e_lfanew);
    if (nt_headers->Signature != IMAGE_NT_SIGNATURE) {
        printf("[PE Loader] Invalid NT Signature: 0x%08X (expected 0x00004550)\n", nt_headers->Signature);
        return STATUS_INVALID_PARAMETER;
    }

    if (nt_headers->FileHeader.Machine != IMAGE_FILE_MACHINE_AMD64) {
        printf("[PE Loader] Unsupported machine architecture: 0x%04X (requires x86_64 0x8664)\n", 
               nt_headers->FileHeader.Machine);
        return STATUS_NOT_IMPLEMENTED;
    }

    if (nt_headers->OptionalHeader.Magic != 0x20B) {
        printf("[PE Loader] Not a PE32+ (64-bit) binary (Magic: 0x%04X)\n", nt_headers->OptionalHeader.Magic);
        return STATUS_INVALID_PARAMETER;
    }

    proc->image_base = nt_headers->OptionalHeader.ImageBase;
    proc->entry_point = proc->image_base + nt_headers->OptionalHeader.AddressOfEntryPoint;
    proc->image_size = nt_headers->OptionalHeader.SizeOfImage;

    printf("[PE Loader] Loaded Windows NT Executable (PE32+):\n");
    printf("            ImageBase:        0x%016llX\n", (unsigned long long)proc->image_base);
    printf("            AddressOfEntry:   0x%016llX\n", (unsigned long long)proc->entry_point);
    printf("            SizeOfImage:      0x%08X bytes\n", (unsigned int)proc->image_size);
    printf("            Sections:         %u\n", nt_headers->FileHeader.NumberOfSections);

    /* Enumerate and display sections */
    const IMAGE_SECTION_HEADER* section = (const IMAGE_SECTION_HEADER*)((const u8*)&nt_headers->OptionalHeader + 
                                          nt_headers->FileHeader.SizeOfOptionalHeader);

    for (u16 i = 0; i < nt_headers->FileHeader.NumberOfSections; i++) {
        char sec_name[9] = {0};
        memcpy(sec_name, section[i].Name, 8);
        printf("            - Section [%s]: VA=0x%08X, RawSize=0x%08X, Flags=0x%08X\n",
               sec_name, 
               section[i].VirtualAddress, 
               section[i].SizeOfRawData,
               section[i].Characteristics);
    }

    return STATUS_SUCCESS;
}
