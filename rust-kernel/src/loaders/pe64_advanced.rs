//! Advanced Windows PE32+ (PE64) Loader Engine for HybridOS Kernel.
//!
//! Provides deep binary inspection, Section Mapping, Data Directory Resolution
//! (Export, Import, Base Relocation), Import Address Table (IAT) binding with
//! kernel32/ntdll stub resolvers, and complete Thread/Process Environment Block
//! (TEB/PEB) context synthesis for Windows NT user-mode process execution.

#[allow(unused_imports)]
use alloc::string::{String, ToString};
#[allow(unused_imports)]
use alloc::vec::Vec;
#[allow(unused_imports)]
use alloc::boxed::Box;
#[allow(unused_imports)]
use crate::compat::FloatExt;
use std::collections::HashMap;
use std::fmt;
use crate::process::HybridProcess;
use crate::types::{Handle, NtStatus};

// ============================================================================
// PE32+ Architectural Constants and Signatures
// ============================================================================

pub const IMAGE_DOS_SIGNATURE: u16 = 0x5A4D;                 // "MZ"
pub const IMAGE_NT_SIGNATURE: u32 = 0x00004550;               // "PE\0\0"
pub const IMAGE_FILE_MACHINE_AMD64: u16 = 0x8664;             // x86_64 / AMD64
pub const IMAGE_FILE_MACHINE_ARM64: u16 = 0xAA64;             // ARM64
pub const OPTIONAL_HEADER_MAGIC_PE32_PLUS: u16 = 0x020B;      // PE32+ (64-bit)

// PE File Header Characteristics
pub const IMAGE_FILE_RELOCS_STRIPPED: u16 = 0x0001;
pub const IMAGE_FILE_EXECUTABLE_IMAGE: u16 = 0x0002;
pub const IMAGE_FILE_LARGE_ADDRESS_AWARE: u16 = 0x0020;
pub const IMAGE_FILE_DLL: u16 = 0x2000;

// Windows Subsystem IDs
pub const IMAGE_SUBSYSTEM_UNKNOWN: u16 = 0;
pub const IMAGE_SUBSYSTEM_NATIVE: u16 = 1;
pub const IMAGE_SUBSYSTEM_WINDOWS_GUI: u16 = 2;
pub const IMAGE_SUBSYSTEM_WINDOWS_CUI: u16 = 3;
pub const IMAGE_SUBSYSTEM_POSIX_CUI: u16 = 7;
pub const IMAGE_SUBSYSTEM_WINDOWS_CE_GUI: u16 = 9;
pub const IMAGE_SUBSYSTEM_EFI_APPLICATION: u16 = 10;

// Section Characteristics Flags
pub const IMAGE_SCN_CNT_CODE: u32 = 0x00000020;
pub const IMAGE_SCN_CNT_INITIALIZED_DATA: u32 = 0x00000040;
pub const IMAGE_SCN_CNT_UNINITIALIZED_DATA: u32 = 0x00000080;
pub const IMAGE_SCN_MEM_DISCARDABLE: u32 = 0x02000000;
pub const IMAGE_SCN_MEM_NOT_CACHED: u32 = 0x04000000;
pub const IMAGE_SCN_MEM_NOT_PAGED: u32 = 0x08000000;
pub const IMAGE_SCN_MEM_SHARED: u32 = 0x10000000;
pub const IMAGE_SCN_MEM_EXECUTE: u32 = 0x20000000;
pub const IMAGE_SCN_MEM_READ: u32 = 0x40000000;
pub const IMAGE_SCN_MEM_WRITE: u32 = 0x80000000;

// Standard PE Data Directory Entries (16 total)
pub const IMAGE_DIRECTORY_ENTRY_EXPORT: usize = 0;
pub const IMAGE_DIRECTORY_ENTRY_IMPORT: usize = 1;
pub const IMAGE_DIRECTORY_ENTRY_RESOURCE: usize = 2;
pub const IMAGE_DIRECTORY_ENTRY_EXCEPTION: usize = 3;
pub const IMAGE_DIRECTORY_ENTRY_SECURITY: usize = 4;
pub const IMAGE_DIRECTORY_ENTRY_BASERELOC: usize = 5;
pub const IMAGE_DIRECTORY_ENTRY_DEBUG: usize = 6;
pub const IMAGE_DIRECTORY_ENTRY_ARCHITECTURE: usize = 7;
pub const IMAGE_DIRECTORY_ENTRY_GLOBALPTR: usize = 8;
pub const IMAGE_DIRECTORY_ENTRY_TLS: usize = 9;
pub const IMAGE_DIRECTORY_ENTRY_LOAD_CONFIG: usize = 10;
pub const IMAGE_DIRECTORY_ENTRY_BOUND_IMPORT: usize = 11;
pub const IMAGE_DIRECTORY_ENTRY_IAT: usize = 12;
pub const IMAGE_DIRECTORY_ENTRY_DELAY_IMPORT: usize = 13;
pub const IMAGE_DIRECTORY_ENTRY_COM_DESCRIPTOR: usize = 14;

// Base Relocation Types
pub const IMAGE_REL_BASED_ABSOLUTE: u8 = 0;
pub const IMAGE_REL_BASED_HIGH: u8 = 1;
pub const IMAGE_REL_BASED_LOW: u8 = 2;
pub const IMAGE_REL_BASED_HIGHLOW: u8 = 3;
pub const IMAGE_REL_BASED_HIGHADJ: u8 = 4;
pub const IMAGE_REL_BASED_DIR64: u8 = 10;

// Ordinal Flag for PE64 (Bit 63)
pub const IMAGE_ORDINAL_FLAG64: u64 = 0x80000000_00000000;

// ============================================================================
// Error Types
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PeError {
    BufferTooSmall { required: usize, actual: usize },
    InvalidDosSignature(u16),
    InvalidPeOffset(usize),
    InvalidNtSignature(u32),
    InvalidMachine(u16),
    InvalidOptionalMagic(u16),
    InvalidHeaderSize,
    SectionOutOfBounds { index: usize, rva: u32, size: usize },
    InvalidRva(u32),
    CorruptedDirectory(&'static str),
    RelocationFailed(String),
    IatBindingFailed(String),
    MemoryAllocationError,
}

impl fmt::Display for PeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PeError::BufferTooSmall { required, actual } => {
                write!(f, "PE buffer too small: required {} bytes, got {}", required, actual)
            }
            PeError::InvalidDosSignature(sig) => write!(f, "Invalid DOS signature: 0x{:04X} (expected MZ)", sig),
            PeError::InvalidPeOffset(off) => write!(f, "Invalid PE NT header offset e_lfanew: 0x{:X}", off),
            PeError::InvalidNtSignature(sig) => write!(f, "Invalid NT signature: 0x{:08X} (expected PE\\0\\0)", sig),
            PeError::InvalidMachine(m) => write!(f, "Unsupported architecture machine: 0x{:04X}", m),
            PeError::InvalidOptionalMagic(m) => write!(f, "Invalid Optional Header magic: 0x{:04X} (expected PE32+)", m),
            PeError::InvalidHeaderSize => write!(f, "Invalid header size specified in optional header"),
            PeError::SectionOutOfBounds { index, rva, size } => {
                write!(f, "Section {} out of bounds: RVA 0x{:08X}, raw size {}", index, rva, size)
            }
            PeError::InvalidRva(rva) => write!(f, "Failed to resolve RVA 0x{:08X} to image data", rva),
            PeError::CorruptedDirectory(dir) => write!(f, "Corrupted or malformed data directory: {}", dir),
            PeError::RelocationFailed(msg) => write!(f, "Base relocation error: {}", msg),
            PeError::IatBindingFailed(msg) => write!(f, "IAT binding error: {}", msg),
            PeError::MemoryAllocationError => write!(f, "Failed to allocate memory for virtual PE image"),
        }
    }
}

impl std::error::Error for PeError {}

impl From<PeError> for NtStatus {
    fn from(err: PeError) -> Self {
        match err {
            PeError::BufferTooSmall { .. } | PeError::InvalidDosSignature(_) | PeError::InvalidNtSignature(_) => {
                NtStatus::INVALID_PARAMETER
            }
            PeError::InvalidMachine(_) => NtStatus::NOT_IMPLEMENTED,
            PeError::MemoryAllocationError => NtStatus::NO_MEMORY,
            _ => NtStatus::UNSUCCESSFUL,
        }
    }
}

// ============================================================================
// Raw PE Binary Structures
// ============================================================================

/// IMAGE_DOS_HEADER (64 bytes)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageDosHeader {
    pub e_magic: u16,
    pub e_cblp: u16,
    pub e_cp: u16,
    pub e_crlc: u16,
    pub e_cparhdr: u16,
    pub e_minalloc: u16,
    pub e_maxalloc: u16,
    pub e_ss: u16,
    pub e_sp: u16,
    pub e_csum: u16,
    pub e_ip: u16,
    pub e_cs: u16,
    pub e_lfarlc: u16,
    pub e_ovno: u16,
    pub e_res: [u16; 4],
    pub e_oemid: u16,
    pub e_oeminfo: u16,
    pub e_res2: [u16; 10],
    pub e_lfanew: u32,
}

impl ImageDosHeader {
    pub fn parse(buf: &[u8]) -> Result<Self, PeError> {
        if buf.len() < 64 {
            return Err(PeError::BufferTooSmall { required: 64, actual: buf.len() });
        }
        let e_magic = u16::from_le_bytes([buf[0], buf[1]]);
        if e_magic != IMAGE_DOS_SIGNATURE {
            return Err(PeError::InvalidDosSignature(e_magic));
        }

        let e_lfanew = u32::from_le_bytes([buf[60], buf[61], buf[62], buf[63]]);

        Ok(Self {
            e_magic,
            e_cblp: u16::from_le_bytes([buf[2], buf[3]]),
            e_cp: u16::from_le_bytes([buf[4], buf[5]]),
            e_crlc: u16::from_le_bytes([buf[6], buf[7]]),
            e_cparhdr: u16::from_le_bytes([buf[8], buf[9]]),
            e_minalloc: u16::from_le_bytes([buf[10], buf[11]]),
            e_maxalloc: u16::from_le_bytes([buf[12], buf[13]]),
            e_ss: u16::from_le_bytes([buf[14], buf[15]]),
            e_sp: u16::from_le_bytes([buf[16], buf[17]]),
            e_csum: u16::from_le_bytes([buf[18], buf[19]]),
            e_ip: u16::from_le_bytes([buf[20], buf[21]]),
            e_cs: u16::from_le_bytes([buf[22], buf[23]]),
            e_lfarlc: u16::from_le_bytes([buf[24], buf[25]]),
            e_ovno: u16::from_le_bytes([buf[26], buf[27]]),
            e_res: [
                u16::from_le_bytes([buf[28], buf[29]]),
                u16::from_le_bytes([buf[30], buf[31]]),
                u16::from_le_bytes([buf[32], buf[33]]),
                u16::from_le_bytes([buf[34], buf[35]]),
            ],
            e_oemid: u16::from_le_bytes([buf[36], buf[37]]),
            e_oeminfo: u16::from_le_bytes([buf[38], buf[39]]),
            e_res2: [0; 10], // Simplified zeroed
            e_lfanew,
        })
    }
}

/// IMAGE_FILE_HEADER (COFF Header, 20 bytes)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageFileHeader {
    pub machine: u16,
    pub number_of_sections: u16,
    pub time_date_stamp: u32,
    pub pointer_to_symbol_table: u32,
    pub number_of_symbols: u32,
    pub size_of_optional_header: u16,
    pub characteristics: u16,
}

impl ImageFileHeader {
    pub fn parse(buf: &[u8]) -> Result<Self, PeError> {
        if buf.len() < 20 {
            return Err(PeError::BufferTooSmall { required: 20, actual: buf.len() });
        }
        Ok(Self {
            machine: u16::from_le_bytes([buf[0], buf[1]]),
            number_of_sections: u16::from_le_bytes([buf[2], buf[3]]),
            time_date_stamp: u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]),
            pointer_to_symbol_table: u32::from_le_bytes([buf[8], buf[9], buf[10], buf[11]]),
            number_of_symbols: u32::from_le_bytes([buf[12], buf[13], buf[14], buf[15]]),
            size_of_optional_header: u16::from_le_bytes([buf[16], buf[17]]),
            characteristics: u16::from_le_bytes([buf[18], buf[19]]),
        })
    }
}

/// IMAGE_DATA_DIRECTORY (8 bytes)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ImageDataDirectory {
    pub virtual_address: u32,
    pub size: u32,
}

impl ImageDataDirectory {
    pub fn parse(buf: &[u8]) -> Self {
        if buf.len() < 8 {
            return Self::default();
        }
        Self {
            virtual_address: u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]),
            size: u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]),
        }
    }

    pub fn is_present(&self) -> bool {
        self.virtual_address != 0 && self.size != 0
    }
}

/// IMAGE_OPTIONAL_HEADER64 (PE32+, 240 bytes)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageOptionalHeader64 {
    pub magic: u16,
    pub major_linker_version: u8,
    pub minor_linker_version: u8,
    pub size_of_code: u32,
    pub size_of_initialized_data: u32,
    pub size_of_uninitialized_data: u32,
    pub address_of_entry_point: u32,
    pub base_of_code: u32,
    pub image_base: u64,
    pub section_alignment: u32,
    pub file_alignment: u32,
    pub major_operating_system_version: u16,
    pub minor_operating_system_version: u16,
    pub major_image_version: u16,
    pub minor_image_version: u16,
    pub major_subsystem_version: u16,
    pub minor_subsystem_version: u16,
    pub win32_version_value: u32,
    pub size_of_image: u32,
    pub size_of_headers: u32,
    pub check_sum: u32,
    pub subsystem: u16,
    pub dll_characteristics: u16,
    pub size_of_stack_reserve: u64,
    pub size_of_stack_commit: u64,
    pub size_of_heap_reserve: u64,
    pub size_of_heap_commit: u64,
    pub loader_flags: u32,
    pub number_of_rva_and_sizes: u32,
    pub data_directories: [ImageDataDirectory; 16],
}

impl ImageOptionalHeader64 {
    pub fn parse(buf: &[u8]) -> Result<Self, PeError> {
        if buf.len() < 112 {
            return Err(PeError::BufferTooSmall { required: 112, actual: buf.len() });
        }
        let magic = u16::from_le_bytes([buf[0], buf[1]]);
        if magic != OPTIONAL_HEADER_MAGIC_PE32_PLUS {
            return Err(PeError::InvalidOptionalMagic(magic));
        }

        let mut data_dirs = [ImageDataDirectory::default(); 16];
        let num_dirs = u32::from_le_bytes([buf[108], buf[109], buf[110], buf[111]]) as usize;
        let clamped_dirs = num_dirs.min(16);

        let dirs_start = 112;
        for i in 0..clamped_dirs {
            let offset = dirs_start + (i * 8);
            if offset + 8 <= buf.len() {
                data_dirs[i] = ImageDataDirectory::parse(&buf[offset..offset + 8]);
            }
        }

        Ok(Self {
            magic,
            major_linker_version: buf[2],
            minor_linker_version: buf[3],
            size_of_code: u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]),
            size_of_initialized_data: u32::from_le_bytes([buf[8], buf[9], buf[10], buf[11]]),
            size_of_uninitialized_data: u32::from_le_bytes([buf[12], buf[13], buf[14], buf[15]]),
            address_of_entry_point: u32::from_le_bytes([buf[16], buf[17], buf[18], buf[19]]),
            base_of_code: u32::from_le_bytes([buf[20], buf[21], buf[22], buf[23]]),
            image_base: u64::from_le_bytes([
                buf[24], buf[25], buf[26], buf[27],
                buf[28], buf[29], buf[30], buf[31],
            ]),
            section_alignment: u32::from_le_bytes([buf[32], buf[33], buf[34], buf[35]]),
            file_alignment: u32::from_le_bytes([buf[36], buf[37], buf[38], buf[39]]),
            major_operating_system_version: u16::from_le_bytes([buf[40], buf[41]]),
            minor_operating_system_version: u16::from_le_bytes([buf[42], buf[43]]),
            major_image_version: u16::from_le_bytes([buf[44], buf[45]]),
            minor_image_version: u16::from_le_bytes([buf[46], buf[47]]),
            major_subsystem_version: u16::from_le_bytes([buf[48], buf[49]]),
            minor_subsystem_version: u16::from_le_bytes([buf[50], buf[51]]),
            win32_version_value: u32::from_le_bytes([buf[52], buf[53], buf[54], buf[55]]),
            size_of_image: u32::from_le_bytes([buf[56], buf[57], buf[58], buf[59]]),
            size_of_headers: u32::from_le_bytes([buf[60], buf[61], buf[62], buf[63]]),
            check_sum: u32::from_le_bytes([buf[64], buf[65], buf[66], buf[67]]),
            subsystem: u16::from_le_bytes([buf[68], buf[69]]),
            dll_characteristics: u16::from_le_bytes([buf[70], buf[71]]),
            size_of_stack_reserve: u64::from_le_bytes([
                buf[72], buf[73], buf[74], buf[75],
                buf[76], buf[77], buf[78], buf[79],
            ]),
            size_of_stack_commit: u64::from_le_bytes([
                buf[80], buf[81], buf[82], buf[83],
                buf[84], buf[85], buf[86], buf[87],
            ]),
            size_of_heap_reserve: u64::from_le_bytes([
                buf[88], buf[89], buf[90], buf[91],
                buf[92], buf[93], buf[94], buf[95],
            ]),
            size_of_heap_commit: u64::from_le_bytes([
                buf[96], buf[97], buf[98], buf[99],
                buf[100], buf[101], buf[102], buf[103],
            ]),
            loader_flags: u32::from_le_bytes([buf[104], buf[105], buf[106], buf[107]]),
            number_of_rva_and_sizes: num_dirs as u32,
            data_directories: data_dirs,
        })
    }
}

/// IMAGE_NT_HEADERS64 (Signature + FileHeader + OptionalHeader)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageNtHeaders64 {
    pub signature: u32,
    pub file_header: ImageFileHeader,
    pub optional_header: ImageOptionalHeader64,
}

impl ImageNtHeaders64 {
    pub fn parse(buf: &[u8]) -> Result<Self, PeError> {
        if buf.len() < 24 {
            return Err(PeError::BufferTooSmall { required: 24, actual: buf.len() });
        }
        let signature = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
        if signature != IMAGE_NT_SIGNATURE {
            return Err(PeError::InvalidNtSignature(signature));
        }

        let file_header = ImageFileHeader::parse(&buf[4..24])?;
        if file_header.machine != IMAGE_FILE_MACHINE_AMD64 && file_header.machine != IMAGE_FILE_MACHINE_ARM64 {
            return Err(PeError::InvalidMachine(file_header.machine));
        }

        let opt_start = 24;
        let opt_end = opt_start + file_header.size_of_optional_header as usize;
        if opt_end > buf.len() {
            return Err(PeError::BufferTooSmall { required: opt_end, actual: buf.len() });
        }

        let optional_header = ImageOptionalHeader64::parse(&buf[opt_start..opt_end])?;

        Ok(Self {
            signature,
            file_header,
            optional_header,
        })
    }
}

/// IMAGE_SECTION_HEADER (40 bytes)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageSectionHeader {
    pub name_raw: [u8; 8],
    pub virtual_size: u32,
    pub virtual_address: u32,
    pub size_of_raw_data: u32,
    pub pointer_to_raw_data: u32,
    pub pointer_to_relocations: u32,
    pub pointer_to_linenumbers: u32,
    pub number_of_relocations: u16,
    pub number_of_linenumbers: u16,
    pub characteristics: u32,
}

impl ImageSectionHeader {
    pub fn parse(buf: &[u8]) -> Result<Self, PeError> {
        if buf.len() < 40 {
            return Err(PeError::BufferTooSmall { required: 40, actual: buf.len() });
        }
        let mut name_raw = [0u8; 8];
        name_raw.copy_from_slice(&buf[0..8]);

        Ok(Self {
            name_raw,
            virtual_size: u32::from_le_bytes([buf[8], buf[9], buf[10], buf[11]]),
            virtual_address: u32::from_le_bytes([buf[12], buf[13], buf[14], buf[15]]),
            size_of_raw_data: u32::from_le_bytes([buf[16], buf[17], buf[18], buf[19]]),
            pointer_to_raw_data: u32::from_le_bytes([buf[20], buf[21], buf[22], buf[23]]),
            pointer_to_relocations: u32::from_le_bytes([buf[24], buf[25], buf[26], buf[27]]),
            pointer_to_linenumbers: u32::from_le_bytes([buf[28], buf[29], buf[30], buf[31]]),
            number_of_relocations: u16::from_le_bytes([buf[32], buf[33]]),
            number_of_linenumbers: u16::from_le_bytes([buf[34], buf[35]]),
            characteristics: u32::from_le_bytes([buf[36], buf[37], buf[38], buf[39]]),
        })
    }

    pub fn name(&self) -> String {
        let trimmed = self.name_raw.split(|&b| b == 0).next().unwrap_or(&self.name_raw);
        String::from_utf8_lossy(trimmed).to_string()
    }

    pub fn is_executable(&self) -> bool {
        (self.characteristics & IMAGE_SCN_MEM_EXECUTE) != 0
    }

    pub fn is_readable(&self) -> bool {
        (self.characteristics & IMAGE_SCN_MEM_READ) != 0
    }

    pub fn is_writable(&self) -> bool {
        (self.characteristics & IMAGE_SCN_MEM_WRITE) != 0
    }

    pub fn is_code(&self) -> bool {
        (self.characteristics & IMAGE_SCN_CNT_CODE) != 0
    }

    pub fn is_initialized_data(&self) -> bool {
        (self.characteristics & IMAGE_SCN_CNT_INITIALIZED_DATA) != 0
    }
}

// ============================================================================
// Data Directory Tables: Export, Import, Relocation
// ============================================================================

/// IMAGE_EXPORT_DIRECTORY (40 bytes)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageExportDirectory {
    pub characteristics: u32,
    pub time_date_stamp: u32,
    pub major_version: u16,
    pub minor_version: u16,
    pub name: u32,
    pub base: u32,
    pub number_of_functions: u32,
    pub number_of_names: u32,
    pub address_of_functions: u32,
    pub address_of_names: u32,
    pub address_of_name_ordinals: u32,
}

impl ImageExportDirectory {
    pub fn parse(buf: &[u8]) -> Result<Self, PeError> {
        if buf.len() < 40 {
            return Err(PeError::BufferTooSmall { required: 40, actual: buf.len() });
        }
        Ok(Self {
            characteristics: u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]),
            time_date_stamp: u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]),
            major_version: u16::from_le_bytes([buf[8], buf[9]]),
            minor_version: u16::from_le_bytes([buf[10], buf[11]]),
            name: u32::from_le_bytes([buf[12], buf[13], buf[14], buf[15]]),
            base: u32::from_le_bytes([buf[16], buf[17], buf[18], buf[19]]),
            number_of_functions: u32::from_le_bytes([buf[20], buf[21], buf[22], buf[23]]),
            number_of_names: u32::from_le_bytes([buf[24], buf[25], buf[26], buf[27]]),
            address_of_functions: u32::from_le_bytes([buf[28], buf[29], buf[30], buf[31]]),
            address_of_names: u32::from_le_bytes([buf[32], buf[33], buf[34], buf[35]]),
            address_of_name_ordinals: u32::from_le_bytes([buf[36], buf[37], buf[38], buf[39]]),
        })
    }
}

/// Exported Function Metadata
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportSymbol {
    pub name: Option<String>,
    pub ordinal: u32,
    pub rva: u32,
    pub forwarder: Option<String>,
}

/// Parsed Export Directory Table
#[derive(Debug, Clone, Default)]
pub struct ExportTable {
    pub dll_name: String,
    pub ordinal_base: u32,
    pub symbols: Vec<ExportSymbol>,
}

impl ExportTable {
    pub fn find_by_name(&self, name: &str) -> Option<&ExportSymbol> {
        self.symbols.iter().find(|s| s.name.as_deref() == Some(name))
    }

    pub fn find_by_ordinal(&self, ordinal: u32) -> Option<&ExportSymbol> {
        self.symbols.iter().find(|s| s.ordinal == ordinal)
    }
}

/// IMAGE_IMPORT_DESCRIPTOR (20 bytes)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageImportDescriptor {
    pub original_first_thunk: u32, // RVA to Import Lookup Table (INT)
    pub time_date_stamp: u32,
    pub forwarder_chain: u32,
    pub name: u32,                 // RVA to DLL name string
    pub first_thunk: u32,          // RVA to Import Address Table (IAT)
}

impl ImageImportDescriptor {
    pub fn parse(buf: &[u8]) -> Result<Self, PeError> {
        if buf.len() < 20 {
            return Err(PeError::BufferTooSmall { required: 20, actual: buf.len() });
        }
        Ok(Self {
            original_first_thunk: u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]),
            time_date_stamp: u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]),
            forwarder_chain: u32::from_le_bytes([buf[8], buf[9], buf[10], buf[11]]),
            name: u32::from_le_bytes([buf[12], buf[13], buf[14], buf[15]]),
            first_thunk: u32::from_le_bytes([buf[16], buf[17], buf[18], buf[19]]),
        })
    }

    pub fn is_terminator(&self) -> bool {
        self.original_first_thunk == 0
            && self.name == 0
            && self.first_thunk == 0
            && self.time_date_stamp == 0
            && self.forwarder_chain == 0
    }
}

/// Resolved Import Specification
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportFunction {
    ByName { hint: u16, name: String },
    ByOrdinal(u16),
}

impl fmt::Display for ImportFunction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImportFunction::ByName { hint, name } => write!(f, "{} (hint: {})", name, hint),
            ImportFunction::ByOrdinal(ord) => write!(f, "Ordinal #{}", ord),
        }
    }
}

/// Single Import Entry to be bound into the IAT
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportEntry {
    pub function: ImportFunction,
    pub iat_rva: u32,
    pub bound_address: Option<u64>,
}

/// All imports required from a specific DLL
#[derive(Debug, Clone)]
pub struct ImportModule {
    pub dll_name: String,
    pub original_first_thunk: u32,
    pub first_thunk: u32,
    pub entries: Vec<ImportEntry>,
}

/// Single Relocation Fixup Record
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RelocationEntry {
    pub reloc_type: u8,
    pub offset: u16,
    pub rva: u32,
}

/// A Page Block in the `.reloc` Section
#[derive(Debug, Clone)]
pub struct RelocationBlock {
    pub page_rva: u32,
    pub block_size: u32,
    pub entries: Vec<RelocationEntry>,
}

// ============================================================================
// Deep PE32+ Parser Model
// ============================================================================

/// Complete in-memory representation of a parsed PE32+ executable
#[derive(Debug, Clone)]
pub struct Pe64Image {
    pub dos_header: ImageDosHeader,
    pub nt_headers: ImageNtHeaders64,
    pub sections: Vec<ImageSectionHeader>,
    pub export_table: Option<ExportTable>,
    pub import_modules: Vec<ImportModule>,
    pub relocation_blocks: Vec<RelocationBlock>,
}

impl Pe64Image {
    /// Deeply parses a 64-bit Windows PE binary buffer
    pub fn parse(buffer: &[u8]) -> Result<Self, PeError> {
        let dos_header = ImageDosHeader::parse(buffer)?;

        let e_lfanew = dos_header.e_lfanew as usize;
        if e_lfanew + 264 > buffer.len() {
            return Err(PeError::InvalidPeOffset(e_lfanew));
        }

        let nt_headers = ImageNtHeaders64::parse(&buffer[e_lfanew..])?;

        // Read Section Table
        let num_sections = nt_headers.file_header.number_of_sections as usize;
        let opt_size = nt_headers.file_header.size_of_optional_header as usize;
        let sec_start = e_lfanew + 24 + opt_size;

        if sec_start + (num_sections * 40) > buffer.len() {
            return Err(PeError::BufferTooSmall {
                required: sec_start + (num_sections * 40),
                actual: buffer.len(),
            });
        }

        let mut sections = Vec::with_capacity(num_sections);
        for i in 0..num_sections {
            let offset = sec_start + (i * 40);
            let sec = ImageSectionHeader::parse(&buffer[offset..offset + 40])?;
            sections.push(sec);
        }

        let mut pe = Self {
            dos_header,
            nt_headers,
            sections,
            export_table: None,
            import_modules: Vec::new(),
            relocation_blocks: Vec::new(),
        };

        // Parse Export Directory
        pe.parse_export_directory(buffer)?;

        // Parse Import Directory
        pe.parse_import_directory(buffer)?;

        // Parse Base Relocations
        pe.parse_base_relocations(buffer)?;

        Ok(pe)
    }

    /// Converts a Relative Virtual Address (RVA) to a raw file offset
    pub fn rva_to_offset(&self, rva: u32) -> Option<usize> {
        // Headers region check
        if rva < self.nt_headers.optional_header.size_of_headers {
            return Some(rva as usize);
        }

        for sec in &self.sections {
            let sec_va_start = sec.virtual_address;
            let sec_va_end = sec_va_start + sec.virtual_size.max(sec.size_of_raw_data);

            if rva >= sec_va_start && rva < sec_va_end {
                let delta = rva - sec_va_start;
                if delta < sec.size_of_raw_data {
                    return Some((sec.pointer_to_raw_data + delta) as usize);
                } else {
                    return None; // Inside uninitialized BSS portion
                }
            }
        }
        None
    }

    /// Reads a null-terminated ASCII C-string starting at given RVA
    pub fn read_cstring(&self, buffer: &[u8], rva: u32) -> Option<String> {
        let mut offset = self.rva_to_offset(rva)?;
        let mut bytes = Vec::new();
        while offset < buffer.len() {
            let b = buffer[offset];
            if b == 0 {
                break;
            }
            bytes.push(b);
            offset += 1;
        }
        String::from_utf8(bytes).ok()
    }

    /// Reads an exact byte slice starting at given RVA
    pub fn read_slice<'a>(&self, buffer: &'a [u8], rva: u32, len: usize) -> Option<&'a [u8]> {
        let offset = self.rva_to_offset(rva)?;
        if offset + len <= buffer.len() {
            Some(&buffer[offset..offset + len])
        } else {
            None
        }
    }

    /// Internal parser for IMAGE_DIRECTORY_ENTRY_EXPORT
    fn parse_export_directory(&mut self, buffer: &[u8]) -> Result<(), PeError> {
        let dir = &self.nt_headers.optional_header.data_directories[IMAGE_DIRECTORY_ENTRY_EXPORT];
        if !dir.is_present() {
            return Ok(());
        }

        let exp_slice = match self.read_slice(buffer, dir.virtual_address, 40) {
            Some(s) => s,
            None => return Err(PeError::CorruptedDirectory("Export Directory Header")),
        };

        let raw_exp = ImageExportDirectory::parse(exp_slice)?;
        let dll_name = self.read_cstring(buffer, raw_exp.name).unwrap_or_else(|| "UNKNOWN.DLL".into());

        let num_funcs = raw_exp.number_of_functions as usize;
        let num_names = raw_exp.number_of_names as usize;

        let mut symbols = Vec::with_capacity(num_funcs);

        // Read Functions RVA array
        let funcs_bytes = self.read_slice(buffer, raw_exp.address_of_functions, num_funcs * 4)
            .ok_or(PeError::CorruptedDirectory("Export Functions Table"))?;

        // Read Names RVA array
        let names_bytes = if raw_exp.address_of_names != 0 && num_names > 0 {
            self.read_slice(buffer, raw_exp.address_of_names, num_names * 4)
        } else {
            None
        };

        // Read Name Ordinals array
        let ordinals_bytes = if raw_exp.address_of_name_ordinals != 0 && num_names > 0 {
            self.read_slice(buffer, raw_exp.address_of_name_ordinals, num_names * 2)
        } else {
            None
        };

        // Map function index to name
        let mut name_map: HashMap<usize, String> = HashMap::new();
        if let (Some(nb), Some(ob)) = (names_bytes, ordinals_bytes) {
            for i in 0..num_names {
                let name_rva = u32::from_le_bytes([nb[i * 4], nb[i * 4 + 1], nb[i * 4 + 2], nb[i * 4 + 3]]);
                let ordinal_idx = u16::from_le_bytes([ob[i * 2], ob[i * 2 + 1]]) as usize;
                if let Some(name_str) = self.read_cstring(buffer, name_rva) {
                    name_map.insert(ordinal_idx, name_str);
                }
            }
        }

        let dir_start = dir.virtual_address;
        let dir_end = dir_start + dir.size;

        for i in 0..num_funcs {
            let func_rva = u32::from_le_bytes([
                funcs_bytes[i * 4],
                funcs_bytes[i * 4 + 1],
                funcs_bytes[i * 4 + 2],
                funcs_bytes[i * 4 + 3],
            ]);

            if func_rva == 0 {
                continue; // Unassigned slot
            }

            let ordinal = raw_exp.base + i as u32;
            let name = name_map.remove(&i);

            // Forwarder detection: If function RVA falls inside Export Directory range
            let forwarder = if func_rva >= dir_start && func_rva < dir_end {
                self.read_cstring(buffer, func_rva)
            } else {
                None
            };

            symbols.push(ExportSymbol {
                name,
                ordinal,
                rva: func_rva,
                forwarder,
            });
        }

        self.export_table = Some(ExportTable {
            dll_name,
            ordinal_base: raw_exp.base,
            symbols,
        });

        Ok(())
    }

    /// Internal parser for IMAGE_DIRECTORY_ENTRY_IMPORT
    fn parse_import_directory(&mut self, buffer: &[u8]) -> Result<(), PeError> {
        let dir = &self.nt_headers.optional_header.data_directories[IMAGE_DIRECTORY_ENTRY_IMPORT];
        if !dir.is_present() {
            return Ok(());
        }

        let mut desc_rva = dir.virtual_address;
        let mut modules = Vec::new();

        loop {
            let desc_bytes = match self.read_slice(buffer, desc_rva, 20) {
                Some(b) => b,
                None => break,
            };
            let desc = ImageImportDescriptor::parse(desc_bytes)?;
            if desc.is_terminator() {
                break;
            }

            let dll_name = self.read_cstring(buffer, desc.name).unwrap_or_else(|| "UNKNOWN.DLL".into());
            let thunk_rva = if desc.original_first_thunk != 0 {
                desc.original_first_thunk
            } else {
                desc.first_thunk
            };

            let mut entries = Vec::new();
            let mut curr_thunk_rva = thunk_rva;
            let mut curr_iat_rva = desc.first_thunk;

            loop {
                let thunk_bytes = match self.read_slice(buffer, curr_thunk_rva, 8) {
                    Some(b) => b,
                    None => break,
                };
                let thunk_val = u64::from_le_bytes([
                    thunk_bytes[0], thunk_bytes[1], thunk_bytes[2], thunk_bytes[3],
                    thunk_bytes[4], thunk_bytes[5], thunk_bytes[6], thunk_bytes[7],
                ]);

                if thunk_val == 0 {
                    break; // End of thunk table
                }

                let function = if (thunk_val & IMAGE_ORDINAL_FLAG64) != 0 {
                    let ord = (thunk_val & 0xFFFF) as u16;
                    ImportFunction::ByOrdinal(ord)
                } else {
                    let name_rva = (thunk_val & 0x7FFFFFFF) as u32;
                    let hint = match self.read_slice(buffer, name_rva, 2) {
                        Some(b) => u16::from_le_bytes([b[0], b[1]]),
                        None => 0,
                    };
                    let func_name = self.read_cstring(buffer, name_rva + 2).unwrap_or_default();
                    ImportFunction::ByName { hint, name: func_name }
                };

                entries.push(ImportEntry {
                    function,
                    iat_rva: curr_iat_rva,
                    bound_address: None,
                });

                curr_thunk_rva += 8;
                curr_iat_rva += 8;
            }

            modules.push(ImportModule {
                dll_name,
                original_first_thunk: desc.original_first_thunk,
                first_thunk: desc.first_thunk,
                entries,
            });

            desc_rva += 20;
        }

        self.import_modules = modules;
        Ok(())
    }

    /// Internal parser for IMAGE_DIRECTORY_ENTRY_BASERELOC
    fn parse_base_relocations(&mut self, buffer: &[u8]) -> Result<(), PeError> {
        let dir = &self.nt_headers.optional_header.data_directories[IMAGE_DIRECTORY_ENTRY_BASERELOC];
        if !dir.is_present() {
            return Ok(());
        }

        let mut offset_rva = dir.virtual_address;
        let end_rva = dir.virtual_address + dir.size;
        let mut blocks = Vec::new();

        while offset_rva < end_rva {
            let header_bytes = match self.read_slice(buffer, offset_rva, 8) {
                Some(b) => b,
                None => break,
            };

            let page_rva = u32::from_le_bytes([header_bytes[0], header_bytes[1], header_bytes[2], header_bytes[3]]);
            let block_size = u32::from_le_bytes([header_bytes[4], header_bytes[5], header_bytes[6], header_bytes[7]]);

            if block_size < 8 {
                break; // Corrupted relocation block
            }

            let num_entries = ((block_size - 8) / 2) as usize;
            let entries_bytes = match self.read_slice(buffer, offset_rva + 8, num_entries * 2) {
                Some(b) => b,
                None => break,
            };

            let mut entries = Vec::with_capacity(num_entries);
            for i in 0..num_entries {
                let entry_val = u16::from_le_bytes([entries_bytes[i * 2], entries_bytes[i * 2 + 1]]);
                let reloc_type = (entry_val >> 12) as u8;
                let offset = entry_val & 0x0FFF;
                let rva = page_rva + offset as u32;

                entries.push(RelocationEntry {
                    reloc_type,
                    offset,
                    rva,
                });
            }

            blocks.push(RelocationBlock {
                page_rva,
                block_size,
                entries,
            });

            offset_rva += block_size;
        }

        self.relocation_blocks = blocks;
        Ok(())
    }
}

// ============================================================================
// Relocation Engine (DIR64 & HIGHLOW)
// ============================================================================

/// Applies Windows Base Relocations (.reloc) to mapped virtual memory image
pub fn apply_base_relocations(
    image_buffer: &mut [u8],
    relocations: &[RelocationBlock],
    preferred_base: u64,
    actual_base: u64,
) -> Result<usize, PeError> {
    if preferred_base == actual_base {
        return Ok(0);
    }

    let delta = actual_base.wrapping_sub(preferred_base);
    let mut applied_count = 0;

    for block in relocations {
        for entry in &block.entries {
            let target_offset = entry.rva as usize;

            match entry.reloc_type {
                IMAGE_REL_BASED_ABSOLUTE => {
                    // Padding / no-op
                }
                IMAGE_REL_BASED_DIR64 => {
                    if target_offset + 8 > image_buffer.len() {
                        return Err(PeError::RelocationFailed(format!(
                            "DIR64 fixup RVA 0x{:08X} beyond image bounds",
                            entry.rva
                        )));
                    }
                    let current_val = u64::from_le_bytes([
                        image_buffer[target_offset],
                        image_buffer[target_offset + 1],
                        image_buffer[target_offset + 2],
                        image_buffer[target_offset + 3],
                        image_buffer[target_offset + 4],
                        image_buffer[target_offset + 5],
                        image_buffer[target_offset + 6],
                        image_buffer[target_offset + 7],
                    ]);
                    let rebased_val = current_val.wrapping_add(delta);
                    image_buffer[target_offset..target_offset + 8].copy_from_slice(&rebased_val.to_le_bytes());
                    applied_count += 1;
                }
                IMAGE_REL_BASED_HIGHLOW => {
                    if target_offset + 4 > image_buffer.len() {
                        return Err(PeError::RelocationFailed(format!(
                            "HIGHLOW fixup RVA 0x{:08X} beyond image bounds",
                            entry.rva
                        )));
                    }
                    let current_val = u32::from_le_bytes([
                        image_buffer[target_offset],
                        image_buffer[target_offset + 1],
                        image_buffer[target_offset + 2],
                        image_buffer[target_offset + 3],
                    ]);
                    let delta32 = delta as u32;
                    let rebased_val = current_val.wrapping_add(delta32);
                    image_buffer[target_offset..target_offset + 4].copy_from_slice(&rebased_val.to_le_bytes());
                    applied_count += 1;
                }
                other => {
                    println!("[PE Relocator] Skipped unsupported relocation type: {}", other);
                }
            }
        }
    }

    Ok(applied_count)
}

// ============================================================================
// Import Address Table (IAT) Binder & Win32 Stub Resolver
// ============================================================================

/// Synthetic Virtual Addresses assigned to system stubs in HybridOS
pub const STUB_BASE_KERNEL32: u64 = 0x00007FFF_00010000;
pub const STUB_BASE_NTDLL: u64    = 0x00007FFF_00020000;
pub const STUB_BASE_USER32: u64   = 0x00007FFF_00030000;
pub const STUB_BASE_DYNAMIC: u64  = 0x00007FFF_DEAD0000;

/// Report summarizing results of IAT binding
#[derive(Debug, Clone, Default)]
pub struct IatBindReport {
    pub total_bound: usize,
    pub known_stubs: usize,
    pub dynamic_stubs: usize,
    pub modules_bound: Vec<String>,
}

/// Dynamic IAT Binder and Windows System API Stub Resolver
#[derive(Debug, Clone)]
pub struct IatStubResolver {
    known_stubs: HashMap<(String, String), u64>,
    dynamic_counter: u64,
}

impl Default for IatStubResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl IatStubResolver {
    pub fn new() -> Self {
        let mut resolver = Self {
            known_stubs: HashMap::new(),
            dynamic_counter: 0,
        };
        resolver.register_default_stubs();
        resolver
    }

    /// Registers standard system API stubs for kernel32.dll and ntdll.dll
    fn register_default_stubs(&mut self) {
        // kernel32.dll stubs
        let kernel32_symbols = [
            ("ExitProcess",           STUB_BASE_KERNEL32 + 0x00),
            ("GetStdHandle",          STUB_BASE_KERNEL32 + 0x10),
            ("WriteFile",             STUB_BASE_KERNEL32 + 0x20),
            ("ReadFile",              STUB_BASE_KERNEL32 + 0x30),
            ("CreateFileA",           STUB_BASE_KERNEL32 + 0x40),
            ("CreateFileW",           STUB_BASE_KERNEL32 + 0x50),
            ("CloseHandle",           STUB_BASE_KERNEL32 + 0x60),
            ("VirtualAlloc",          STUB_BASE_KERNEL32 + 0x70),
            ("VirtualFree",           STUB_BASE_KERNEL32 + 0x80),
            ("GetLastError",          STUB_BASE_KERNEL32 + 0x90),
            ("SetLastError",          STUB_BASE_KERNEL32 + 0xA0),
            ("GetModuleHandleA",      STUB_BASE_KERNEL32 + 0xB0),
            ("GetModuleHandleW",      STUB_BASE_KERNEL32 + 0xC0),
            ("GetProcAddress",        STUB_BASE_KERNEL32 + 0xD0),
            ("GetCurrentProcessId",   STUB_BASE_KERNEL32 + 0xE0),
            ("GetCurrentThreadId",    STUB_BASE_KERNEL32 + 0xF0),
            ("Sleep",                 STUB_BASE_KERNEL32 + 0x100),
            ("GetCommandLineA",       STUB_BASE_KERNEL32 + 0x110),
            ("GetCommandLineW",       STUB_BASE_KERNEL32 + 0x120),
            ("GetEnvironmentStrings", STUB_BASE_KERNEL32 + 0x130),
            ("FreeEnvironmentStrings",STUB_BASE_KERNEL32 + 0x140),
            ("GetProcessHeap",        STUB_BASE_KERNEL32 + 0x150),
            ("HeapAlloc",             STUB_BASE_KERNEL32 + 0x160),
            ("HeapFree",              STUB_BASE_KERNEL32 + 0x170),
        ];

        for (sym, addr) in kernel32_symbols {
            self.register_stub("kernel32.dll", sym, addr);
        }

        // ntdll.dll stubs
        let ntdll_symbols = [
            ("NtWriteFile",               STUB_BASE_NTDLL + 0x00),
            ("NtReadFile",                STUB_BASE_NTDLL + 0x10),
            ("NtAllocateVirtualMemory",   STUB_BASE_NTDLL + 0x20),
            ("NtFreeVirtualMemory",       STUB_BASE_NTDLL + 0x30),
            ("NtClose",                   STUB_BASE_NTDLL + 0x40),
            ("NtTerminateProcess",        STUB_BASE_NTDLL + 0x50),
            ("RtlAllocateHeap",           STUB_BASE_NTDLL + 0x60),
            ("RtlFreeHeap",               STUB_BASE_NTDLL + 0x70),
            ("RtlInitUnicodeString",      STUB_BASE_NTDLL + 0x80),
            ("NtQuerySystemInformation",  STUB_BASE_NTDLL + 0x90),
            ("RtlCaptureContext",         STUB_BASE_NTDLL + 0xA0),
            ("NtProtectVirtualMemory",    STUB_BASE_NTDLL + 0xB0),
        ];

        for (sym, addr) in ntdll_symbols {
            self.register_stub("ntdll.dll", sym, addr);
        }
    }

    /// Registers a custom API stub address for a specific DLL and function
    pub fn register_stub(&mut self, dll: &str, function: &str, stub_address: u64) {
        self.known_stubs.insert((dll.to_ascii_lowercase(), function.to_string()), stub_address);
    }

    /// Resolves an import symbol to a synthetic stub address
    pub fn resolve(&mut self, dll: &str, symbol: &ImportFunction) -> (u64, bool) {
        let dll_key = dll.to_ascii_lowercase();

        let sym_name = match symbol {
            ImportFunction::ByName { name, .. } => name.clone(),
            ImportFunction::ByOrdinal(ord) => format!("Ordinal_{}", ord),
        };

        if let Some(&addr) = self.known_stubs.get(&(dll_key, sym_name)) {
            (addr, true)
        } else {
            // Allocate a synthetic dynamic stub
            let addr = STUB_BASE_DYNAMIC + (self.dynamic_counter * 0x10);
            self.dynamic_counter += 1;
            (addr, false)
        }
    }

    /// Binds all import entries across modules directly into the in-memory IAT
    pub fn bind_imports(
        &mut self,
        image_buffer: &mut [u8],
        import_modules: &mut [ImportModule],
    ) -> Result<IatBindReport, PeError> {
        let mut report = IatBindReport::default();

        for module in import_modules {
            report.modules_bound.push(module.dll_name.clone());

            for entry in &mut module.entries {
                let (stub_addr, is_known) = self.resolve(&module.dll_name, &entry.function);
                entry.bound_address = Some(stub_addr);

                let iat_offset = entry.iat_rva as usize;
                if iat_offset + 8 > image_buffer.len() {
                    return Err(PeError::IatBindingFailed(format!(
                        "IAT RVA 0x{:08X} outside image buffer for {}:{}",
                        entry.iat_rva, module.dll_name, entry.function
                    )));
                }

                image_buffer[iat_offset..iat_offset + 8].copy_from_slice(&stub_addr.to_le_bytes());

                report.total_bound += 1;
                if is_known {
                    report.known_stubs += 1;
                } else {
                    report.dynamic_stubs += 1;
                }
            }
        }

        Ok(report)
    }
}

// ============================================================================
// Windows TEB / PEB User-Mode Context Preparation
// ============================================================================

/// UNICODE_STRING on x86_64
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct UnicodeString64 {
    pub length: u16,
    pub maximum_length: u16,
    pub _padding: u32,
    pub buffer: u64,
}

/// RTL_USER_PROCESS_PARAMETERS on 64-bit Windows NT
#[derive(Debug, Clone)]
pub struct RtlUserProcessParameters64 {
    pub maximum_length: u32,
    pub length: u32,
    pub flags: u32,
    pub debug_flags: u32,
    pub console_handle: u64,
    pub console_flags: u32,
    pub standard_input: u64,
    pub standard_output: u64,
    pub standard_error: u64,
    pub current_directory: UnicodeString64,
    pub dll_path: UnicodeString64,
    pub image_path_name: UnicodeString64,
    pub command_line: UnicodeString64,
    pub environment: u64,
}

impl Default for RtlUserProcessParameters64 {
    fn default() -> Self {
        Self {
            maximum_length: 0x400,
            length: 0x400,
            flags: 1,
            debug_flags: 0,
            console_handle: 0,
            console_flags: 0,
            standard_input: 0,
            standard_output: 1,
            standard_error: 2,
            current_directory: UnicodeString64::default(),
            dll_path: UnicodeString64::default(),
            image_path_name: UnicodeString64::default(),
            command_line: UnicodeString64::default(),
            environment: 0,
        }
    }
}

/// Windows 64-bit Process Environment Block (PEB)
#[derive(Debug, Clone)]
pub struct Peb64 {
    pub inherited_address_space: u8,
    pub read_image_file_exec_options: u8,
    pub being_debugged: u8,
    pub bit_field: u8,
    pub mutant: u64,
    pub image_base_address: u64,
    pub ldr: u64,
    pub process_parameters: u64,
    pub sub_system_data: u64,
    pub process_heap: u64,
    pub fast_peb_lock: u64,
    pub atl_thunk_s_list_ptr: u64,
    pub ifeo_key: u64,
    pub cross_process_flags: u32,
    pub kernel_callback_table: u64,
    pub number_of_processors: u32,
    pub nt_global_flag: u32,
    pub os_major_version: u32,
    pub os_minor_version: u32,
    pub os_build_number: u16,
    pub os_csd_version: u16,
    pub os_platform_id: u32,
    pub session_id: u32,
}

impl Default for Peb64 {
    fn default() -> Self {
        Self {
            inherited_address_space: 0,
            read_image_file_exec_options: 0,
            being_debugged: 0,
            bit_field: 0,
            mutant: 0,
            image_base_address: 0,
            ldr: 0,
            process_parameters: 0,
            sub_system_data: 0,
            process_heap: 0x00000180_00000000,
            fast_peb_lock: 0,
            atl_thunk_s_list_ptr: 0,
            ifeo_key: 0,
            cross_process_flags: 0,
            kernel_callback_table: 0,
            number_of_processors: 8,
            nt_global_flag: 0,
            os_major_version: 10,
            os_minor_version: 0,
            os_build_number: 19045, // Windows 10 22H2
            os_csd_version: 0,
            os_platform_id: 2,      // VER_PLATFORM_WIN32_NT
            session_id: 1,
        }
    }
}

/// Windows 64-bit Thread Environment Block (TEB)
#[derive(Debug, Clone)]
pub struct Teb64 {
    // NT_TIB64 (0x00 - 0x38)
    pub exception_list: u64,               // 0x00
    pub stack_base: u64,                   // 0x08
    pub stack_limit: u64,                  // 0x10
    pub subsystem_tib: u64,                // 0x18
    pub fiber_data: u64,                   // 0x20
    pub arbitrary_user_pointer: u64,       // 0x28
    pub self_teb: u64,                     // 0x30 (GS:[0x30] pointer to TEB)
    // TEB specific
    pub environment_pointer: u64,          // 0x38
    pub client_id_pid: u64,                // 0x40
    pub client_id_tid: u64,                // 0x48
    pub active_rpc_handle: u64,            // 0x50
    pub thread_local_storage_pointer: u64, // 0x58
    pub process_environment_block: u64,    // 0x60 (GS:[0x60] pointer to PEB)
    pub last_error_value: u32,             // 0x68
    pub count_of_owned_critical_sections: u32, // 0x6C
}

impl Default for Teb64 {
    fn default() -> Self {
        Self {
            exception_list: 0,
            stack_base: 0x000000C0_00000000,
            stack_limit: 0x000000BF_FFE00000,
            subsystem_tib: 0,
            fiber_data: 0,
            arbitrary_user_pointer: 0,
            self_teb: 0,
            environment_pointer: 0,
            client_id_pid: 0,
            client_id_tid: 1,
            active_rpc_handle: 0,
            thread_local_storage_pointer: 0,
            process_environment_block: 0,
            last_error_value: 0,
            count_of_owned_critical_sections: 0,
        }
    }
}

/// Fully synthesized Windows User-Mode Context
#[derive(Debug, Clone)]
pub struct WindowsUserContext {
    pub peb_address: u64,
    pub teb_address: u64,
    pub process_params_address: u64,
    pub peb: Peb64,
    pub teb: Teb64,
    pub process_parameters: RtlUserProcessParameters64,
    pub initial_rip: u64,
    pub initial_rsp: u64,
    pub initial_gs_base: u64,
    pub serialized_memory: HashMap<u64, Vec<u8>>,
}

/// Prepares the full PEB/TEB context and serialized memory layout for a Windows process
pub fn prepare_windows_user_context(
    proc: &HybridProcess,
    image_base: u64,
    entry_point: u64,
    command_line: &str,
) -> WindowsUserContext {
    let peb_address = 0x00007FFE_00000000u64;
    let teb_address = 0x00007FFE_00002000u64;
    let params_address = 0x00007FFE_00004000u64;
    let cmdline_str_address = 0x00007FFE_00005000u64;
    let imgpath_str_address = 0x00007FFE_00006000u64;

    let mut peb = Peb64::default();
    peb.image_base_address = image_base;
    peb.process_parameters = params_address;

    let mut teb = Teb64::default();
    teb.self_teb = teb_address;
    teb.process_environment_block = peb_address;
    teb.client_id_pid = proc.pid as u64;
    teb.client_id_tid = 1;

    let mut params = RtlUserProcessParameters64::default();
    params.standard_input = Handle::STDIN.0 as u64;
    params.standard_output = Handle::STDOUT.0 as u64;
    params.standard_error = Handle::STDERR.0 as u64;

    // Convert strings to UTF-16LE
    let cmdline_utf16: Vec<u16> = command_line.encode_utf16().chain(std::iter::once(0)).collect();
    let imgpath_utf16: Vec<u16> = proc.name.encode_utf16().chain(std::iter::once(0)).collect();

    params.command_line = UnicodeString64 {
        length: ((cmdline_utf16.len() - 1) * 2) as u16,
        maximum_length: (cmdline_utf16.len() * 2) as u16,
        _padding: 0,
        buffer: cmdline_str_address,
    };

    params.image_path_name = UnicodeString64 {
        length: ((imgpath_utf16.len() - 1) * 2) as u16,
        maximum_length: (imgpath_utf16.len() * 2) as u16,
        _padding: 0,
        buffer: imgpath_str_address,
    };

    let mut serialized_memory = HashMap::new();

    // Serialize PEB (512 bytes)
    let mut peb_buf = vec![0u8; 512];
    peb_buf[0] = peb.inherited_address_space;
    peb_buf[1] = peb.read_image_file_exec_options;
    peb_buf[2] = peb.being_debugged;
    peb_buf[3] = peb.bit_field;
    peb_buf[0x10..0x18].copy_from_slice(&peb.image_base_address.to_le_bytes());
    peb_buf[0x18..0x20].copy_from_slice(&peb.ldr.to_le_bytes());
    peb_buf[0x20..0x28].copy_from_slice(&peb.process_parameters.to_le_bytes());
    peb_buf[0x30..0x38].copy_from_slice(&peb.process_heap.to_le_bytes());
    peb_buf[0xB8..0xBC].copy_from_slice(&peb.number_of_processors.to_le_bytes());
    peb_buf[0xBC..0xC0].copy_from_slice(&peb.nt_global_flag.to_le_bytes());
    peb_buf[0x118..0x11C].copy_from_slice(&peb.os_major_version.to_le_bytes());
    peb_buf[0x11C..0x120].copy_from_slice(&peb.os_minor_version.to_le_bytes());
    peb_buf[0x120..0x122].copy_from_slice(&peb.os_build_number.to_le_bytes());
    peb_buf[0x124..0x128].copy_from_slice(&peb.os_platform_id.to_le_bytes());
    peb_buf[0x1D0..0x1D4].copy_from_slice(&peb.session_id.to_le_bytes());
    serialized_memory.insert(peb_address, peb_buf);

    // Serialize TEB (4096 bytes)
    let mut teb_buf = vec![0u8; 4096];
    teb_buf[0x00..0x08].copy_from_slice(&teb.exception_list.to_le_bytes());
    teb_buf[0x08..0x10].copy_from_slice(&teb.stack_base.to_le_bytes());
    teb_buf[0x10..0x18].copy_from_slice(&teb.stack_limit.to_le_bytes());
    teb_buf[0x30..0x38].copy_from_slice(&teb.self_teb.to_le_bytes()); // GS:[0x30]
    teb_buf[0x40..0x48].copy_from_slice(&teb.client_id_pid.to_le_bytes());
    teb_buf[0x48..0x50].copy_from_slice(&teb.client_id_tid.to_le_bytes());
    teb_buf[0x60..0x68].copy_from_slice(&teb.process_environment_block.to_le_bytes()); // GS:[0x60]
    teb_buf[0x68..0x6C].copy_from_slice(&teb.last_error_value.to_le_bytes());
    serialized_memory.insert(teb_address, teb_buf);

    // Serialize Process Parameters (512 bytes)
    let mut params_buf = vec![0u8; 512];
    params_buf[0x00..0x04].copy_from_slice(&params.maximum_length.to_le_bytes());
    params_buf[0x04..0x08].copy_from_slice(&params.length.to_le_bytes());
    params_buf[0x08..0x0C].copy_from_slice(&params.flags.to_le_bytes());
    params_buf[0x20..0x28].copy_from_slice(&params.standard_input.to_le_bytes());
    params_buf[0x28..0x30].copy_from_slice(&params.standard_output.to_le_bytes());
    params_buf[0x30..0x38].copy_from_slice(&params.standard_error.to_le_bytes());
    // image_path_name at 0x60
    params_buf[0x60..0x62].copy_from_slice(&params.image_path_name.length.to_le_bytes());
    params_buf[0x62..0x64].copy_from_slice(&params.image_path_name.maximum_length.to_le_bytes());
    params_buf[0x68..0x70].copy_from_slice(&params.image_path_name.buffer.to_le_bytes());
    // command_line at 0x70
    params_buf[0x70..0x72].copy_from_slice(&params.command_line.length.to_le_bytes());
    params_buf[0x72..0x74].copy_from_slice(&params.command_line.maximum_length.to_le_bytes());
    params_buf[0x78..0x80].copy_from_slice(&params.command_line.buffer.to_le_bytes());
    serialized_memory.insert(params_address, params_buf);

    // Serialize CommandLine string
    let mut cmdline_bytes = Vec::new();
    for u in cmdline_utf16 {
        cmdline_bytes.extend_from_slice(&u.to_le_bytes());
    }
    serialized_memory.insert(cmdline_str_address, cmdline_bytes);

    // Serialize ImagePath string
    let mut imgpath_bytes = Vec::new();
    for u in imgpath_utf16 {
        imgpath_bytes.extend_from_slice(&u.to_le_bytes());
    }
    serialized_memory.insert(imgpath_str_address, imgpath_bytes);

    WindowsUserContext {
        peb_address,
        teb_address,
        process_params_address: params_address,
        peb,
        teb,
        process_parameters: params,
        initial_rip: entry_point,
        initial_rsp: 0x000000C0_00000000u64 - 0x28, // 32 bytes shadow store + 8 bytes return alignment
        initial_gs_base: teb_address,
        serialized_memory,
    }
}

// ============================================================================
// End-to-End Loaded Image Result & Main Loader Entry Point
// ============================================================================

/// Complete representation of an instantiated and bound Windows PE32+ Image in HybridOS
#[derive(Debug, Clone)]
pub struct Pe64LoadedImage {
    pub pe: Pe64Image,
    pub target_base: u64,
    pub entry_point: u64,
    pub virtual_size: usize,
    pub mapped_image: Vec<u8>,
    pub relocations_applied: usize,
    pub iat_report: IatBindReport,
    pub user_context: WindowsUserContext,
}

/// Primary loader routine: Deeply parses, maps, rebases, binds IAT, and preps PEB/TEB.
pub fn load_pe64_advanced(
    proc: &mut HybridProcess,
    raw_buffer: &[u8],
    preferred_target_base: Option<u64>,
) -> Result<Pe64LoadedImage, PeError> {
    // 1. Deeply parse headers and data directories
    let mut pe = Pe64Image::parse(raw_buffer)?;

    let original_base = pe.nt_headers.optional_header.image_base;
    let target_base = preferred_target_base.unwrap_or(original_base);
    let size_of_image = pe.nt_headers.optional_header.size_of_image as usize;
    let size_of_headers = pe.nt_headers.optional_header.size_of_headers as usize;

    if size_of_image < size_of_headers {
        return Err(PeError::InvalidHeaderSize);
    }

    // 2. Allocate contiguous virtual memory buffer for the mapped image
    let mut mapped_image = vec![0u8; size_of_image];

    // 3. Map PE Headers (DOS + NT + Sections)
    let headers_to_copy = size_of_headers.min(raw_buffer.len());
    mapped_image[0..headers_to_copy].copy_from_slice(&raw_buffer[0..headers_to_copy]);

    // 4. Map each Section according to VirtualAddress and SizeOfRawData
    for sec in &pe.sections {
        let sec_va = sec.virtual_address as usize;
        let sec_raw_ptr = sec.pointer_to_raw_data as usize;
        let sec_raw_size = sec.size_of_raw_data as usize;
        let sec_virt_size = sec.virtual_size as usize;

        if sec_raw_size == 0 || sec_raw_ptr == 0 {
            // Uninitialized section (e.g. .bss) - remains zeroed
            continue;
        }

        let copy_size = sec_raw_size.min(sec_virt_size);
        if sec_raw_ptr + copy_size > raw_buffer.len() {
            return Err(PeError::SectionOutOfBounds {
                index: 0,
                rva: sec.virtual_address,
                size: copy_size,
            });
        }

        if sec_va + copy_size > mapped_image.len() {
            return Err(PeError::SectionOutOfBounds {
                index: 0,
                rva: sec.virtual_address,
                size: copy_size,
            });
        }

        mapped_image[sec_va..sec_va + copy_size]
            .copy_from_slice(&raw_buffer[sec_raw_ptr..sec_raw_ptr + copy_size]);
    }

    // 5. Apply Base Relocations if rebasing occurred
    let relocations_applied = if target_base != original_base {
        apply_base_relocations(&mut mapped_image, &pe.relocation_blocks, original_base, target_base)?
    } else {
        0
    };

    // 6. Bind Import Address Table (IAT) using Stub Resolver
    let mut resolver = IatStubResolver::new();
    let iat_report = resolver.bind_imports(&mut mapped_image, &mut pe.import_modules)?;

    // 7. Calculate final Entry Point
    let entry_point = target_base + pe.nt_headers.optional_header.address_of_entry_point as u64;

    // 8. Update HybridProcess kernel control block
    proc.image_base = target_base;
    proc.entry_point = entry_point;
    proc.image_size = size_of_image;

    // 9. Synthesize Windows PEB / TEB context
    let command_line = format!("{} /auto", proc.name);
    let user_context = prepare_windows_user_context(proc, target_base, entry_point, &command_line);

    println!("[PE64 Advanced Engine] Binary Successfully Loaded:");
    println!("                       Image Base:       0x{:016X} (Original: 0x{:016X})", target_base, original_base);
    println!("                       Entry Point:      0x{:016X}", entry_point);
    println!("                       Size of Image:    {} bytes ({} KB)", size_of_image, size_of_image / 1024);
    println!("                       Sections Count:   {}", pe.sections.len());
    println!("                       Relocs Applied:   {} records", relocations_applied);
    println!("                       IAT Stubs Bound:  {} (Known: {}, Dynamic: {})",
        iat_report.total_bound, iat_report.known_stubs, iat_report.dynamic_stubs);
    println!("                       PEB Address:      0x{:016X}", user_context.peb_address);
    println!("                       TEB Address:      0x{:016X} (GS Base)", user_context.teb_address);

    Ok(Pe64LoadedImage {
        pe,
        target_base,
        entry_point,
        virtual_size: size_of_image,
        mapped_image,
        relocations_applied,
        iat_report,
        user_context,
    })
}

// ============================================================================
// Unit and Integration Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::personality::ExecutionPersonality;

    /// Helper to construct a synthetic PE32+ binary with DOS, NT headers, sections,
    /// exports, imports, and relocation tables.
    fn create_synthetic_pe64() -> Vec<u8> {
        let mut buf = vec![0u8; 4096];

        // 1. DOS Header
        buf[0] = b'M';
        buf[1] = b'Z';
        let pe_offset = 128usize;
        buf[60..64].copy_from_slice(&(pe_offset as u32).to_le_bytes());

        // 2. NT Signature ("PE\0\0")
        buf[pe_offset..pe_offset + 4].copy_from_slice(&IMAGE_NT_SIGNATURE.to_le_bytes());

        // 3. File Header (20 bytes)
        let fh_off = pe_offset + 4;
        buf[fh_off..fh_off + 2].copy_from_slice(&IMAGE_FILE_MACHINE_AMD64.to_le_bytes()); // Machine
        buf[fh_off + 2..fh_off + 4].copy_from_slice(&3u16.to_le_bytes());                  // 3 sections (.text, .rdata, .reloc)
        buf[fh_off + 16..fh_off + 18].copy_from_slice(&240u16.to_le_bytes());             // Optional header size = 240
        buf[fh_off + 18..fh_off + 20].copy_from_slice(&(IMAGE_FILE_EXECUTABLE_IMAGE | IMAGE_FILE_LARGE_ADDRESS_AWARE).to_le_bytes());

        // 4. Optional Header (240 bytes)
        let opt_off = fh_off + 20;
        buf[opt_off..opt_off + 2].copy_from_slice(&OPTIONAL_HEADER_MAGIC_PE32_PLUS.to_le_bytes()); // Magic PE32+
        buf[opt_off + 16..opt_off + 20].copy_from_slice(&0x1000u32.to_le_bytes());                 // EntryPoint RVA = 0x1000
        buf[opt_off + 24..opt_off + 32].copy_from_slice(&0x0000000140000000u64.to_le_bytes());    // Preferred ImageBase
        buf[opt_off + 32..opt_off + 36].copy_from_slice(&0x1000u32.to_le_bytes());                 // SectionAlignment = 4096
        buf[opt_off + 36..opt_off + 40].copy_from_slice(&0x200u32.to_le_bytes());                  // FileAlignment = 512
        buf[opt_off + 56..opt_off + 60].copy_from_slice(&0x4000u32.to_le_bytes());                 // SizeOfImage = 16384 (0x4000)
        buf[opt_off + 60..opt_off + 64].copy_from_slice(&0x400u32.to_le_bytes());                  // SizeOfHeaders = 1024 (0x400)
        buf[opt_off + 68..opt_off + 70].copy_from_slice(&IMAGE_SUBSYSTEM_WINDOWS_CUI.to_le_bytes());
        buf[opt_off + 108..opt_off + 112].copy_from_slice(&16u32.to_le_bytes());                  // NumberOfRvaAndSizes = 16

        // Data Directories (16 * 8 bytes starting at opt_off + 112)
        let dd_off = opt_off + 112;
        // Import Table at index 1: RVA = 0x2000, Size = 40 bytes
        let import_dd = dd_off + (IMAGE_DIRECTORY_ENTRY_IMPORT * 8);
        buf[import_dd..import_dd + 4].copy_from_slice(&0x2000u32.to_le_bytes());
        buf[import_dd + 4..import_dd + 8].copy_from_slice(&40u32.to_le_bytes());

        // Relocation Table at index 5: RVA = 0x3000, Size = 16 bytes
        let reloc_dd = dd_off + (IMAGE_DIRECTORY_ENTRY_BASERELOC * 8);
        buf[reloc_dd..reloc_dd + 4].copy_from_slice(&0x3000u32.to_le_bytes());
        buf[reloc_dd + 4..reloc_dd + 8].copy_from_slice(&16u32.to_le_bytes());

        // 5. Section Headers (3 * 40 bytes)
        let sec_off = opt_off + 240;

        // Section 1: .text (VA: 0x1000, Raw: 0x400, Size: 0x400)
        buf[sec_off..sec_off + 5].copy_from_slice(b".text");
        buf[sec_off + 8..sec_off + 12].copy_from_slice(&0x200u32.to_le_bytes()); // VirtSize
        buf[sec_off + 12..sec_off + 16].copy_from_slice(&0x1000u32.to_le_bytes()); // VA
        buf[sec_off + 16..sec_off + 20].copy_from_slice(&0x400u32.to_le_bytes()); // RawSize
        buf[sec_off + 20..sec_off + 24].copy_from_slice(&0x400u32.to_le_bytes()); // RawPtr
        buf[sec_off + 36..sec_off + 40].copy_from_slice(&(IMAGE_SCN_CNT_CODE | IMAGE_SCN_MEM_EXECUTE | IMAGE_SCN_MEM_READ).to_le_bytes());

        // Section 2: .rdata (VA: 0x2000, Raw: 0x800, Size: 0x400)
        let sec2 = sec_off + 40;
        buf[sec2..sec2 + 6].copy_from_slice(b".rdata");
        buf[sec2 + 8..sec2 + 12].copy_from_slice(&0x400u32.to_le_bytes());
        buf[sec2 + 12..sec2 + 16].copy_from_slice(&0x2000u32.to_le_bytes()); // VA 0x2000
        buf[sec2 + 16..sec2 + 20].copy_from_slice(&0x400u32.to_le_bytes());
        buf[sec2 + 20..sec2 + 24].copy_from_slice(&0x800u32.to_le_bytes()); // RawPtr 0x800
        buf[sec2 + 36..sec2 + 40].copy_from_slice(&(IMAGE_SCN_CNT_INITIALIZED_DATA | IMAGE_SCN_MEM_READ).to_le_bytes());

        // Section 3: .reloc (VA: 0x3000, Raw: 0xC00, Size: 0x200)
        let sec3 = sec_off + 80;
        buf[sec3..sec3 + 6].copy_from_slice(b".reloc");
        buf[sec3 + 8..sec3 + 12].copy_from_slice(&0x200u32.to_le_bytes());
        buf[sec3 + 12..sec3 + 16].copy_from_slice(&0x3000u32.to_le_bytes()); // VA 0x3000
        buf[sec3 + 16..sec3 + 20].copy_from_slice(&0x200u32.to_le_bytes());
        buf[sec3 + 20..sec3 + 24].copy_from_slice(&0xC00u32.to_le_bytes()); // RawPtr 0xC00
        buf[sec3 + 36..sec3 + 40].copy_from_slice(&(IMAGE_SCN_CNT_INITIALIZED_DATA | IMAGE_SCN_MEM_DISCARDABLE | IMAGE_SCN_MEM_READ).to_le_bytes());

        // Payload in .text (Raw 0x400): Contains an absolute pointer needing relocation
        let text_raw = 0x400;
        buf[text_raw] = 0x90; // NOP
        buf[text_raw + 1] = 0x90; // NOP
        // At 0x402 (RVA 0x1002), store an absolute 64-bit address: 0x0000000140002050 (points into preferred base)
        buf[text_raw + 2..text_raw + 10].copy_from_slice(&0x0000000140002050u64.to_le_bytes());

        // Payload in .rdata (Raw 0x800): Import Directory Table + DLL name + Thunks
        let rdata_raw = 0x800;
        // Import Descriptor (20 bytes) at RVA 0x2000 (offset 0x800)
        // OriginalFirstThunk at RVA 0x2030 (offset 0x830)
        buf[rdata_raw..rdata_raw + 4].copy_from_slice(&0x2030u32.to_le_bytes());
        // Name at RVA 0x2050 (offset 0x850) -> "KERNEL32.dll"
        buf[rdata_raw + 12..rdata_raw + 16].copy_from_slice(&0x2050u32.to_le_bytes());
        // FirstThunk at RVA 0x2070 (offset 0x870)
        buf[rdata_raw + 16..rdata_raw + 20].copy_from_slice(&0x2070u32.to_le_bytes());
        // Terminating Import Descriptor at 0x814 is all zeros (default)

        // OriginalFirstThunk at 0x830 (RVA 0x2030): Points to ImportByName at RVA 0x2090 (offset 0x890)
        buf[rdata_raw + 0x30..rdata_raw + 0x38].copy_from_slice(&0x2090u64.to_le_bytes());
        // Terminating 0 u64 at 0x838

        // DLL Name string at 0x850: "KERNEL32.dll\0"
        let dll_bytes = b"KERNEL32.dll\0";
        buf[rdata_raw + 0x50..rdata_raw + 0x50 + dll_bytes.len()].copy_from_slice(dll_bytes);

        // FirstThunk (IAT) at 0x870: Initialized to point to ImportByName at RVA 0x2090
        buf[rdata_raw + 0x70..rdata_raw + 0x78].copy_from_slice(&0x2090u64.to_le_bytes());

        // ImportByName at 0x890: Hint (2 bytes: 0) + "ExitProcess\0"
        let hint_name = b"\x00\x00ExitProcess\0";
        buf[rdata_raw + 0x90..rdata_raw + 0x90 + hint_name.len()].copy_from_slice(hint_name);

        // Payload in .reloc (Raw 0xC00): 1 block covering page RVA 0x1000
        let reloc_raw = 0xC00;
        let page_rva = 0x1000u32;
        let block_size = 8 + 4; // 8 bytes header + 2 entries (each 2 bytes) = 12 bytes
        buf[reloc_raw..reloc_raw + 4].copy_from_slice(&page_rva.to_le_bytes());
        buf[reloc_raw + 4..reloc_raw + 8].copy_from_slice(&(block_size as u32).to_le_bytes());
        // Entry 1: IMAGE_REL_BASED_DIR64 (10) at offset 0x002 -> (10 << 12) | 0x002 = 0xA002
        let entry1 = (10u16 << 12) | 0x002;
        buf[reloc_raw + 8..reloc_raw + 10].copy_from_slice(&entry1.to_le_bytes());
        // Entry 2: IMAGE_REL_BASED_ABSOLUTE (0) padding -> 0x0000
        buf[reloc_raw + 10..reloc_raw + 12].copy_from_slice(&0u16.to_le_bytes());

        buf
    }

    #[test]
    fn test_pe64_parsing() {
        let raw = create_synthetic_pe64();
        let pe = Pe64Image::parse(&raw).expect("Failed to parse synthetic PE32+");

        assert_eq!(pe.dos_header.e_magic, IMAGE_DOS_SIGNATURE);
        assert_eq!(pe.nt_headers.signature, IMAGE_NT_SIGNATURE);
        assert_eq!(pe.nt_headers.file_header.machine, IMAGE_FILE_MACHINE_AMD64);
        assert_eq!(pe.nt_headers.optional_header.magic, OPTIONAL_HEADER_MAGIC_PE32_PLUS);
        assert_eq!(pe.sections.len(), 3);
        assert_eq!(pe.sections[0].name(), ".text");
        assert_eq!(pe.sections[1].name(), ".rdata");
        assert_eq!(pe.sections[2].name(), ".reloc");

        // Verify imports
        assert_eq!(pe.import_modules.len(), 1);
        let mod0 = &pe.import_modules[0];
        assert_eq!(mod0.dll_name, "KERNEL32.dll");
        assert_eq!(mod0.entries.len(), 1);
        match &mod0.entries[0].function {
            ImportFunction::ByName { name, .. } => assert_eq!(name, "ExitProcess"),
            _ => panic!("Expected import by name"),
        }

        // Verify relocations
        assert_eq!(pe.relocation_blocks.len(), 1);
        assert_eq!(pe.relocation_blocks[0].page_rva, 0x1000);
        assert_eq!(pe.relocation_blocks[0].entries.len(), 2);
        assert_eq!(pe.relocation_blocks[0].entries[0].reloc_type, IMAGE_REL_BASED_DIR64);
        assert_eq!(pe.relocation_blocks[0].entries[0].offset, 0x002);
    }

    #[test]
    fn test_pe64_rebasing_and_relocation() {
        let raw = create_synthetic_pe64();
        let pe = Pe64Image::parse(&raw).unwrap();

        let original_base = pe.nt_headers.optional_header.image_base;
        let new_base = 0x0000000200000000u64; // Rebase to 2.00000000

        let mut image_mem = vec![0u8; 0x4000];
        // Copy .text section to VA 0x1000
        image_mem[0x1000..0x1400].copy_from_slice(&raw[0x400..0x800]);

        // Prior to reloc, value at RVA 0x1002 is 0x140002050
        let original_val = u64::from_le_bytes(image_mem[0x1002..0x100A].try_into().unwrap());
        assert_eq!(original_val, 0x0000000140002050);

        let count = apply_base_relocations(&mut image_mem, &pe.relocation_blocks, original_base, new_base)
            .expect("Relocation failed");
        assert_eq!(count, 1);

        // After reloc, delta is (0x200000000 - 0x140000000) = 0xC0000000
        let rebased_val = u64::from_le_bytes(image_mem[0x1002..0x100A].try_into().unwrap());
        assert_eq!(rebased_val, 0x0000000200002050);
    }

    #[test]
    fn test_iat_stub_binding() {
        let raw = create_synthetic_pe64();
        let mut pe = Pe64Image::parse(&raw).unwrap();

        let mut image_mem = vec![0u8; 0x4000];
        // Copy .rdata section to VA 0x2000
        image_mem[0x2000..0x2400].copy_from_slice(&raw[0x800..0xC00]);

        let mut resolver = IatStubResolver::new();
        let report = resolver.bind_imports(&mut image_mem, &mut pe.import_modules)
            .expect("IAT binding failed");

        assert_eq!(report.total_bound, 1);
        assert_eq!(report.known_stubs, 1);

        // FirstThunk was at RVA 0x2070. ExitProcess should be bound to STUB_BASE_KERNEL32
        let bound_addr = u64::from_le_bytes(image_mem[0x2070..0x2078].try_into().unwrap());
        assert_eq!(bound_addr, STUB_BASE_KERNEL32);
    }

    #[test]
    fn test_peb_teb_preparation() {
        let proc = HybridProcess::new(4242, "calc.exe", ExecutionPersonality::WindowsNt);
        let image_base = 0x0000000140000000u64;
        let entry_point = 0x0000000140001000u64;
        let ctx = prepare_windows_user_context(&proc, image_base, entry_point, "calc.exe /test");

        assert_eq!(ctx.peb.image_base_address, image_base);
        assert_eq!(ctx.teb.self_teb, ctx.teb_address);
        assert_eq!(ctx.teb.process_environment_block, ctx.peb_address);
        assert_eq!(ctx.teb.client_id_pid, 4242);
        assert_eq!(ctx.initial_rip, entry_point);
        assert_eq!(ctx.initial_gs_base, ctx.teb_address);

        // Check serialized TEB at offset 0x30 points to self_teb
        let teb_buf = ctx.serialized_memory.get(&ctx.teb_address).unwrap();
        let self_ptr = u64::from_le_bytes(teb_buf[0x30..0x38].try_into().unwrap());
        assert_eq!(self_ptr, ctx.teb_address);

        // Check serialized TEB at offset 0x60 points to PEB
        let peb_ptr = u64::from_le_bytes(teb_buf[0x60..0x68].try_into().unwrap());
        assert_eq!(peb_ptr, ctx.peb_address);
    }

    #[test]
    fn test_full_load_pe64_advanced() {
        let raw = create_synthetic_pe64();
        let mut proc = HybridProcess::new(5000, "sample_app.exe", ExecutionPersonality::WindowsNt);

        let target_base = 0x0000000150000000u64; // Rebase by +0x10000000
        let loaded = load_pe64_advanced(&mut proc, &raw, Some(target_base))
            .expect("load_pe64_advanced failed");

        assert_eq!(proc.image_base, target_base);
        assert_eq!(proc.entry_point, target_base + 0x1000);
        assert_eq!(loaded.relocations_applied, 1);
        assert_eq!(loaded.iat_report.total_bound, 1);
        assert_eq!(loaded.user_context.peb.image_base_address, target_base);

        // Verify that in mapped_image at RVA 0x1002 the DIR64 rebase occurred
        let val_at_rva = u64::from_le_bytes(loaded.mapped_image[0x1002..0x100A].try_into().unwrap());
        assert_eq!(val_at_rva, 0x0000000150002050);

        // Verify that in mapped_image at IAT RVA 0x2070 ExitProcess stub is written
        let iat_val = u64::from_le_bytes(loaded.mapped_image[0x2070..0x2078].try_into().unwrap());
        assert_eq!(iat_val, STUB_BASE_KERNEL32);
    }
}
