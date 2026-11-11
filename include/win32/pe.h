#ifndef HYBRIDOS_WIN32_PE_H
#define HYBRIDOS_WIN32_PE_H

#include "kernel/types.h"

#define IMAGE_DOS_SIGNATURE                0x5A4D      /* MZ */
#define IMAGE_NT_SIGNATURE                 0x00004550  /* PE00 */
#define IMAGE_FILE_MACHINE_AMD64           0x8664      /* x64 */

#define IMAGE_NUMBEROF_DIRECTORY_ENTRIES   16
#define IMAGE_SIZEOF_SHORT_NAME            8

#pragma pack(push, 2)
typedef struct _IMAGE_DOS_HEADER {
    u16 e_magic;    /* Magic number: MZ (0x5A4D) */
    u16 e_cblp;
    u16 e_cp;
    u16 e_crlc;
    u16 e_cparhdr;
    u16 e_minalloc;
    u16 e_maxalloc;
    u16 e_ss;
    u16 e_sp;
    u16 e_csum;
    u16 e_ip;
    u16 e_cs;
    u16 e_lfarlc;
    u16 e_ovno;
    u16 e_res[4];
    u16 e_oemid;
    u16 e_oeminfo;
    u16 e_res2[10];
    i32 e_lfanew;   /* File address of new exe header (PE) */
} IMAGE_DOS_HEADER, *PIMAGE_DOS_HEADER;
#pragma pack(pop)

#pragma pack(push, 4)
typedef struct _IMAGE_FILE_HEADER {
    u16 Machine;
    u16 NumberOfSections;
    u32 TimeDateStamp;
    u32 PointerToSymbolTable;
    u32 NumberOfSymbols;
    u16 SizeOfOptionalHeader;
    u16 Characteristics;
} IMAGE_FILE_HEADER, *PIMAGE_FILE_HEADER;

typedef struct _IMAGE_DATA_DIRECTORY {
    u32 VirtualAddress;
    u32 Size;
} IMAGE_DATA_DIRECTORY, *PIMAGE_DATA_DIRECTORY;

typedef struct _IMAGE_OPTIONAL_HEADER64 {
    u16 Magic; /* 0x20B for PE32+ (64-bit) */
    u8  MajorLinkerVersion;
    u8  MinorLinkerVersion;
    u32 SizeOfCode;
    u32 SizeOfInitializedData;
    u32 SizeOfUninitializedData;
    u32 AddressOfEntryPoint;
    u32 BaseOfCode;
    u64 ImageBase;
    u32 SectionAlignment;
    u32 FileAlignment;
    u16 MajorOperatingSystemVersion;
    u16 MinorOperatingSystemVersion;
    u16 MajorImageVersion;
    u16 MinorImageVersion;
    u16 MajorSubsystemVersion;
    u16 MinorSubsystemVersion;
    u32 Win32VersionValue;
    u32 SizeOfImage;
    u32 SizeOfHeaders;
    u32 CheckSum;
    u16 Subsystem;
    u16 DllCharacteristics;
    u64 SizeOfStackReserve;
    u64 SizeOfStackCommit;
    u64 SizeOfHeapReserve;
    u64 SizeOfHeapCommit;
    u32 LoaderFlags;
    u32 NumberOfRvaAndSizes;
    IMAGE_DATA_DIRECTORY DataDirectory[IMAGE_NUMBEROF_DIRECTORY_ENTRIES];
} IMAGE_OPTIONAL_HEADER64, *PIMAGE_OPTIONAL_HEADER64;

typedef struct _IMAGE_NT_HEADERS64 {
    u32 Signature; /* "PE\0\0" */
    IMAGE_FILE_HEADER FileHeader;
    IMAGE_OPTIONAL_HEADER64 OptionalHeader;
} IMAGE_NT_HEADERS64, *PIMAGE_NT_HEADERS64;

typedef struct _IMAGE_SECTION_HEADER {
    u8  Name[IMAGE_SIZEOF_SHORT_NAME];
    union {
        u32 PhysicalAddress;
        u32 VirtualSize;
    } Misc;
    u32 VirtualAddress;
    u32 SizeOfRawData;
    u32 PointerToRawData;
    u32 PointerToRelocations;
    u32 PointerToLinenumbers;
    u16 NumberOfRelocations;
    u16 NumberOfLinenumbers;
    u32 Characteristics;
} IMAGE_SECTION_HEADER, *PIMAGE_SECTION_HEADER;
#pragma pack(pop)

#endif /* HYBRIDOS_WIN32_PE_H */
