//! Advanced Linux ELF64 Engine for HybridOS Kernel.
//!
//! Provides deep ELF64 binary parsing, section and symbol analysis, dynamic
//! linking inspection, Auxiliary Vector (Auxv) generation, and System V AMD64
//! ABI compliant initial user stack initialization.
//!
//! # Architecture
//! In HybridOS, Linux executables run side-by-side with Windows PE32+ binaries.
//! The Advanced ELF64 Engine provides the foundation for the Linux POSIX
//! personality by reconstructing the complete binary metadata:
//! - Deep ELF64 parsing: Ehdr, Phdr (PT_LOAD, PT_INTERP, PT_DYNAMIC, PT_PHDR, PT_TLS),
//!   Shdr (sections, symbol table, strtab).
//! - Linux Auxiliary Vector Generator (Auxv): AT_NULL, AT_ENTRY, AT_PHDR, AT_PHNUM,
//!   AT_PAGESZ, AT_BASE, AT_FLAGS, AT_RANDOM, AT_PLATFORM, AT_EXECFN, etc.
//! - Initial User Stack Construction conforming to the Linux System V AMD64 ABI:
//!   `argc`, `argv[]`, `envp[]`, `auxv[]`, and guaranteed 16-byte stack pointer alignment.

#[allow(unused_imports)]
use alloc::string::{String, ToString};
#[allow(unused_imports)]
use alloc::vec::Vec;
#[allow(unused_imports)]
use alloc::boxed::Box;
#[allow(unused_imports)]
use crate::compat::FloatExt;
use std::fmt;
use crate::process::HybridProcess;
use crate::types::NtStatus;

// ============================================================================
// Standard ELF64 Constants
// ============================================================================

/// Standard ELF Identification Magic: 0x7F, 'E', 'L', 'F'
pub const ELFMAG: [u8; 4] = [0x7F, b'E', b'L', b'F'];

/// ELF Header Identification Indices
pub const EI_MAG0: usize = 0;
pub const EI_MAG1: usize = 1;
pub const EI_MAG2: usize = 2;
pub const EI_MAG3: usize = 3;
pub const EI_CLASS: usize = 4;
pub const EI_DATA: usize = 5;
pub const EI_VERSION: usize = 6;
pub const EI_OSABI: usize = 7;
pub const EI_ABIVERSION: usize = 8;
pub const EI_NIDENT: usize = 16;

/// ELF File Classes (Bitness)
pub const ELFCLASSNONE: u8 = 0;
pub const ELFCLASS32: u8 = 1;
pub const ELFCLASS64: u8 = 2;

/// Data Endianness
pub const ELFDATANONE: u8 = 0;
pub const ELFDATA2LSB: u8 = 1; // Little Endian
pub const ELFDATA2MSB: u8 = 2; // Big Endian

/// ELF Version
pub const EV_NONE: u8 = 0;
pub const EV_CURRENT: u8 = 1;

/// Operating System and ABI Identification
pub const ELFOSABI_NONE: u8 = 0;
pub const ELFOSABI_SYSV: u8 = 0;
pub const ELFOSABI_LINUX: u8 = 3;
pub const ELFOSABI_STANDALONE: u8 = 255;

/// ELF File Types (`e_type`)
pub const ET_NONE: u16 = 0;        // No file type
pub const ET_REL: u16 = 1;         // Relocatable object file
pub const ET_EXEC: u16 = 2;        // Fixed-address executable file
pub const ET_DYN: u16 = 3;         // Position Independent Executable (PIE) / Shared Library
pub const ET_CORE: u16 = 4;        // Core file

/// Target Architectures (`e_machine`)
pub const EM_NONE: u16 = 0;
pub const EM_386: u16 = 3;
pub const EM_X86_64: u16 = 62;      // AMD x86-64
pub const EM_ARM: u16 = 40;
pub const EM_AARCH64: u16 = 183;
pub const EM_RISCV: u16 = 243;

/// Program Header Segment Types (`p_type`)
pub const PT_NULL: u32 = 0;         // Unused entry
pub const PT_LOAD: u32 = 1;         // Loadable segment
pub const PT_DYNAMIC: u32 = 2;      // Dynamic linking information
pub const PT_INTERP: u32 = 3;       // Interpreter pathname string
pub const PT_NOTE: u32 = 4;         // Auxiliary information / comments
pub const PT_SHLIB: u32 = 5;        // Reserved / unspecified
pub const PT_PHDR: u32 = 6;         // Location and size of program header table
pub const PT_TLS: u32 = 7;          // Thread-Local Storage template
pub const PT_GNU_EH_FRAME: u32 = 0x6474e550; // Exception handling frame
pub const PT_GNU_STACK: u32 = 0x6474e551;    // Stack execution permission
pub const PT_GNU_RELRO: u32 = 0x6474e552;    // Read-only after relocation
pub const PT_GNU_PROPERTY: u32 = 0x6474e553; // GNU Property notes

/// Program Header Flags (`p_flags`)
pub const PF_X: u32 = 1 << 0;       // Execute permission
pub const PF_W: u32 = 1 << 1;       // Write permission
pub const PF_R: u32 = 1 << 2;       // Read permission
pub const PF_MASKOS: u32 = 0x0ff00000;
pub const PF_MASKPROC: u32 = 0xf0000000;

/// Section Header Types (`sh_type`)
pub const SHT_NULL: u32 = 0;
pub const SHT_PROGBITS: u32 = 1;
pub const SHT_SYMTAB: u32 = 2;
pub const SHT_STRTAB: u32 = 3;
pub const SHT_RELA: u32 = 4;
pub const SHT_HASH: u32 = 5;
pub const SHT_DYNAMIC: u32 = 6;
pub const SHT_NOTE: u32 = 7;
pub const SHT_NOBITS: u32 = 8;
pub const SHT_REL: u32 = 9;
pub const SHT_SHLIB: u32 = 10;
pub const SHT_DYNSYM: u32 = 11;
pub const SHT_INIT_ARRAY: u32 = 14;
pub const SHT_FINI_ARRAY: u32 = 15;
pub const SHT_PREINIT_ARRAY: u32 = 16;
pub const SHT_GROUP: u32 = 17;
pub const SHT_SYMTAB_SHNDX: u32 = 18;
pub const SHT_GNU_HASH: u32 = 0x6ffffff6;
pub const SHT_GNU_VERDEF: u32 = 0x6ffffffd;
pub const SHT_GNU_VERNEED: u32 = 0x6ffffffe;
pub const SHT_GNU_VERSYM: u32 = 0x6fffffff;

/// Section Header Flags (`sh_flags`)
pub const SHF_WRITE: u64 = 0x1;
pub const SHF_ALLOC: u64 = 0x2;
pub const SHF_EXECINSTR: u64 = 0x4;
pub const SHF_MERGE: u64 = 0x10;
pub const SHF_STRINGS: u64 = 0x20;
pub const SHF_INFO_LINK: u64 = 0x40;
pub const SHF_LINK_ORDER: u64 = 0x80;
pub const SHF_OS_NONCONFORMING: u64 = 0x100;
pub const SHF_GROUP: u64 = 0x200;
pub const SHF_TLS: u64 = 0x400;

/// Symbol Bindings
pub const STB_LOCAL: u8 = 0;
pub const STB_GLOBAL: u8 = 1;
pub const STB_WEAK: u8 = 2;

/// Symbol Types
pub const STT_NOTYPE: u8 = 0;
pub const STT_OBJECT: u8 = 1;
pub const STT_FUNC: u8 = 2;
pub const STT_SECTION: u8 = 3;
pub const STT_FILE: u8 = 4;
pub const STT_COMMON: u8 = 5;
pub const STT_TLS: u8 = 6;

/// Symbol Visibility
pub const STV_DEFAULT: u8 = 0;
pub const STV_INTERNAL: u8 = 1;
pub const STV_HIDDEN: u8 = 2;
pub const STV_PROTECTED: u8 = 3;

/// Special Section Indices
pub const SHN_UNDEF: u16 = 0;
pub const SHN_LORESERVE: u16 = 0xff00;
pub const SHN_ABS: u16 = 0xfff1;
pub const SHN_COMMON: u16 = 0xfff2;
pub const SHN_HIRESERVE: u16 = 0xffff;

/// Dynamic Tag Types (`d_tag`)
pub const DT_NULL: i64 = 0;
pub const DT_NEEDED: i64 = 1;
pub const DT_PLTRELSZ: i64 = 2;
pub const DT_PLTGOT: i64 = 3;
pub const DT_HASH: i64 = 4;
pub const DT_STRTAB: i64 = 5;
pub const DT_SYMTAB: i64 = 6;
pub const DT_RELA: i64 = 7;
pub const DT_RELASZ: i64 = 8;
pub const DT_RELAENT: i64 = 9;
pub const DT_STRSZ: i64 = 10;
pub const DT_SYMENT: i64 = 11;
pub const DT_INIT: i64 = 12;
pub const DT_FINI: i64 = 13;
pub const DT_SONAME: i64 = 14;
pub const DT_RPATH: i64 = 15;
pub const DT_SYMBOLIC: i64 = 16;
pub const DT_REL: i64 = 17;
pub const DT_RELSZ: i64 = 18;
pub const DT_RELENT: i64 = 19;
pub const DT_PLTREL: i64 = 20;
pub const DT_DEBUG: i64 = 21;
pub const DT_TEXTREL: i64 = 22;
pub const DT_JMPREL: i64 = 23;
pub const DT_BIND_NOW: i64 = 24;
pub const DT_INIT_ARRAY: i64 = 25;
pub const DT_FINI_ARRAY: i64 = 26;
pub const DT_INIT_ARRAYSZ: i64 = 27;
pub const DT_FINI_ARRAYSZ: i64 = 28;
pub const DT_RUNPATH: i64 = 29;
pub const DT_FLAGS: i64 = 30;
pub const DT_GNU_HASH: i64 = 0x6ffffef5;
pub const DT_VERSYM: i64 = 0x6ffffff0;
pub const DT_FLAGS_1: i64 = 0x6ffffffb;

/// Linux Auxiliary Vector Types (`a_type`)
pub const AT_NULL: u64 = 0;           // End of auxiliary vector
pub const AT_IGNORE: u64 = 1;         // Entry should be ignored
pub const AT_EXECFD: u64 = 2;         // File descriptor of executable
pub const AT_PHDR: u64 = 3;           // Virtual address of program headers in memory
pub const AT_PHENT: u64 = 4;          // Size of one program header entry
pub const AT_PHNUM: u64 = 5;          // Total number of program headers
pub const AT_PAGESZ: u64 = 6;         // System virtual memory page size (typically 4096)
pub const AT_BASE: u64 = 7;           // Base virtual address of dynamic linker (or 0 for static)
pub const AT_FLAGS: u64 = 8;          // Execution flags
pub const AT_ENTRY: u64 = 9;          // Entry point address of program
pub const AT_NOTELF: u64 = 10;        // Program is not an ELF binary
pub const AT_UID: u64 = 11;           // Real User ID
pub const AT_EUID: u64 = 12;          // Effective User ID
pub const AT_GID: u64 = 13;           // Real Group ID
pub const AT_EGID: u64 = 14;          // Effective Group ID
pub const AT_PLATFORM: u64 = 15;      // Address of CPU architecture string (e.g., "x86_64")
pub const AT_HWCAP: u64 = 16;         // Hardware capability flags
pub const AT_CLKTCK: u64 = 17;        // Frequency of times() increments (100Hz)
pub const AT_SECURE: u64 = 23;        // Secure execution mode flag
pub const AT_BASE_PLATFORM: u64 = 24; // Real platform name
pub const AT_RANDOM: u64 = 25;        // Address of 16 cryptographically random bytes
pub const AT_HWCAP2: u64 = 26;        // Extension of AT_HWCAP
pub const AT_EXECFN: u64 = 31;        // Address of executable pathname string
pub const AT_SYSINFO_EHDR: u64 = 33;   // Address of Linux vDSO ELF header

// ============================================================================
// Error Types
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ElfError {
    BufferTooSmall { required: usize, actual: usize },
    InvalidMagic([u8; 4]),
    UnsupportedClass(u8),
    UnsupportedEndian(u8),
    UnsupportedVersion(u8),
    UnsupportedArchitecture(u16),
    InvalidHeader(String),
    InvalidProgramHeader(String),
    InvalidSectionHeader(String),
    StringTableError(String),
    SymbolTableError(String),
    DynamicSectionError(String),
    StackBuilderError(String),
}

impl fmt::Display for ElfError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BufferTooSmall { required, actual } => {
                write!(f, "ELF buffer too small: required {} bytes, got {}", required, actual)
            }
            Self::InvalidMagic(magic) => {
                write!(f, "Invalid ELF magic signature: {:02X?}", magic)
            }
            Self::UnsupportedClass(cls) => {
                write!(f, "Unsupported ELF bitness class: {} (expected ELFCLASS64 = 2)", cls)
            }
            Self::UnsupportedEndian(end) => {
                write!(f, "Unsupported ELF endianness: {} (expected ELFDATA2LSB = 1)", end)
            }
            Self::UnsupportedVersion(ver) => {
                write!(f, "Unsupported ELF version: {} (expected EV_CURRENT = 1)", ver)
            }
            Self::UnsupportedArchitecture(arch) => {
                write!(f, "Unsupported target architecture: {} (expected EM_X86_64 = 62)", arch)
            }
            Self::InvalidHeader(msg) => write!(f, "Invalid ELF header: {}", msg),
            Self::InvalidProgramHeader(msg) => write!(f, "Invalid program header: {}", msg),
            Self::InvalidSectionHeader(msg) => write!(f, "Invalid section header: {}", msg),
            Self::StringTableError(msg) => write!(f, "String table error: {}", msg),
            Self::SymbolTableError(msg) => write!(f, "Symbol table error: {}", msg),
            Self::DynamicSectionError(msg) => write!(f, "Dynamic section error: {}", msg),
            Self::StackBuilderError(msg) => write!(f, "Stack builder error: {}", msg),
        }
    }
}

impl std::error::Error for ElfError {}

impl From<ElfError> for NtStatus {
    fn from(err: ElfError) -> Self {
        match err {
            ElfError::BufferTooSmall { .. }
            | ElfError::InvalidMagic(_)
            | ElfError::InvalidHeader(_)
            | ElfError::InvalidProgramHeader(_)
            | ElfError::InvalidSectionHeader(_)
            | ElfError::StringTableError(_)
            | ElfError::SymbolTableError(_)
            | ElfError::DynamicSectionError(_)
            | ElfError::StackBuilderError(_) => NtStatus::INVALID_PARAMETER,
            ElfError::UnsupportedClass(_)
            | ElfError::UnsupportedEndian(_)
            | ElfError::UnsupportedVersion(_)
            | ElfError::UnsupportedArchitecture(_) => NtStatus::NOT_IMPLEMENTED,
        }
    }
}

// ============================================================================
// ELF64 Core Data Structures
// ============================================================================

/// Raw 64-bit ELF Executable and Linkable Format Header (64 bytes).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Elf64Ehdr {
    pub e_ident: [u8; EI_NIDENT],
    pub e_type: u16,
    pub e_machine: u16,
    pub e_version: u32,
    pub e_entry: u64,
    pub e_phoff: u64,
    pub e_shoff: u64,
    pub e_flags: u32,
    pub e_ehsize: u16,
    pub e_phentsize: u16,
    pub e_phnum: u16,
    pub e_shentsize: u16,
    pub e_shnum: u16,
    pub e_shstrndx: u16,
}

impl Elf64Ehdr {
    pub const SIZE: usize = 64;

    pub fn parse(buf: &[u8]) -> Result<Self, ElfError> {
        if buf.len() < Self::SIZE {
            return Err(ElfError::BufferTooSmall {
                required: Self::SIZE,
                actual: buf.len(),
            });
        }

        if &buf[0..4] != ELFMAG {
            return Err(ElfError::InvalidMagic([buf[0], buf[1], buf[2], buf[3]]));
        }

        if buf[EI_CLASS] != ELFCLASS64 {
            return Err(ElfError::UnsupportedClass(buf[EI_CLASS]));
        }

        if buf[EI_DATA] != ELFDATA2LSB {
            return Err(ElfError::UnsupportedEndian(buf[EI_DATA]));
        }

        if buf[EI_VERSION] != EV_CURRENT {
            return Err(ElfError::UnsupportedVersion(buf[EI_VERSION]));
        }

        let mut e_ident = [0u8; EI_NIDENT];
        e_ident.copy_from_slice(&buf[0..16]);

        let e_type = u16::from_le_bytes([buf[16], buf[17]]);
        let e_machine = u16::from_le_bytes([buf[18], buf[19]]);
        let e_version = u32::from_le_bytes([buf[20], buf[21], buf[22], buf[23]]);
        let e_entry = u64::from_le_bytes([
            buf[24], buf[25], buf[26], buf[27],
            buf[28], buf[29], buf[30], buf[31],
        ]);
        let e_phoff = u64::from_le_bytes([
            buf[32], buf[33], buf[34], buf[35],
            buf[36], buf[37], buf[38], buf[39],
        ]);
        let e_shoff = u64::from_le_bytes([
            buf[40], buf[41], buf[42], buf[43],
            buf[44], buf[45], buf[46], buf[47],
        ]);
        let e_flags = u32::from_le_bytes([buf[48], buf[49], buf[50], buf[51]]);
        let e_ehsize = u16::from_le_bytes([buf[52], buf[53]]);
        let e_phentsize = u16::from_le_bytes([buf[54], buf[55]]);
        let e_phnum = u16::from_le_bytes([buf[56], buf[57]]);
        let e_shentsize = u16::from_le_bytes([buf[58], buf[59]]);
        let e_shnum = u16::from_le_bytes([buf[60], buf[61]]);
        let e_shstrndx = u16::from_le_bytes([buf[62], buf[63]]);

        if e_machine != EM_X86_64 {
            return Err(ElfError::UnsupportedArchitecture(e_machine));
        }

        Ok(Self {
            e_ident,
            e_type,
            e_machine,
            e_version,
            e_entry,
            e_phoff,
            e_shoff,
            e_flags,
            e_ehsize,
            e_phentsize,
            e_phnum,
            e_shentsize,
            e_shnum,
            e_shstrndx,
        })
    }
}

/// Raw 64-bit ELF Program Header Entry (56 bytes).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Elf64Phdr {
    pub p_type: u32,
    pub p_flags: u32,
    pub p_offset: u64,
    pub p_vaddr: u64,
    pub p_paddr: u64,
    pub p_filesz: u64,
    pub p_memsz: u64,
    pub p_align: u64,
}

impl Elf64Phdr {
    pub const SIZE: usize = 56;

    pub fn parse(buf: &[u8]) -> Result<Self, ElfError> {
        if buf.len() < Self::SIZE {
            return Err(ElfError::BufferTooSmall {
                required: Self::SIZE,
                actual: buf.len(),
            });
        }

        let p_type = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
        let p_flags = u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]);
        let p_offset = u64::from_le_bytes([
            buf[8], buf[9], buf[10], buf[11],
            buf[12], buf[13], buf[14], buf[15],
        ]);
        let p_vaddr = u64::from_le_bytes([
            buf[16], buf[17], buf[18], buf[19],
            buf[20], buf[21], buf[22], buf[23],
        ]);
        let p_paddr = u64::from_le_bytes([
            buf[24], buf[25], buf[26], buf[27],
            buf[28], buf[29], buf[30], buf[31],
        ]);
        let p_filesz = u64::from_le_bytes([
            buf[32], buf[33], buf[34], buf[35],
            buf[36], buf[37], buf[38], buf[39],
        ]);
        let p_memsz = u64::from_le_bytes([
            buf[40], buf[41], buf[42], buf[43],
            buf[44], buf[45], buf[46], buf[47],
        ]);
        let p_align = u64::from_le_bytes([
            buf[48], buf[49], buf[50], buf[51],
            buf[52], buf[53], buf[54], buf[55],
        ]);

        Ok(Self {
            p_type,
            p_flags,
            p_offset,
            p_vaddr,
            p_paddr,
            p_filesz,
            p_memsz,
            p_align,
        })
    }

    #[inline]
    pub fn is_load(&self) -> bool {
        self.p_type == PT_LOAD
    }

    #[inline]
    pub fn is_interp(&self) -> bool {
        self.p_type == PT_INTERP
    }

    #[inline]
    pub fn is_dynamic(&self) -> bool {
        self.p_type == PT_DYNAMIC
    }

    #[inline]
    pub fn is_phdr(&self) -> bool {
        self.p_type == PT_PHDR
    }

    #[inline]
    pub fn is_tls(&self) -> bool {
        self.p_type == PT_TLS
    }

    #[inline]
    pub fn is_readable(&self) -> bool {
        (self.p_flags & PF_R) != 0
    }

    #[inline]
    pub fn is_writable(&self) -> bool {
        (self.p_flags & PF_W) != 0
    }

    #[inline]
    pub fn is_executable(&self) -> bool {
        (self.p_flags & PF_X) != 0
    }

    pub fn flags_str(&self) -> String {
        format!(
            "{}{}{}",
            if self.is_readable() { 'R' } else { '-' },
            if self.is_writable() { 'W' } else { '-' },
            if self.is_executable() { 'X' } else { '-' }
        )
    }

    pub fn type_name(&self) -> &'static str {
        match self.p_type {
            PT_NULL => "PT_NULL",
            PT_LOAD => "PT_LOAD",
            PT_DYNAMIC => "PT_DYNAMIC",
            PT_INTERP => "PT_INTERP",
            PT_NOTE => "PT_NOTE",
            PT_SHLIB => "PT_SHLIB",
            PT_PHDR => "PT_PHDR",
            PT_TLS => "PT_TLS",
            PT_GNU_EH_FRAME => "PT_GNU_EH_FRAME",
            PT_GNU_STACK => "PT_GNU_STACK",
            PT_GNU_RELRO => "PT_GNU_RELRO",
            PT_GNU_PROPERTY => "PT_GNU_PROPERTY",
            _ => "PT_UNKNOWN",
        }
    }
}

/// Raw 64-bit ELF Section Header Entry (64 bytes).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Elf64Shdr {
    pub sh_name: u32,
    pub sh_type: u32,
    pub sh_flags: u64,
    pub sh_addr: u64,
    pub sh_offset: u64,
    pub sh_size: u64,
    pub sh_link: u32,
    pub sh_info: u32,
    pub sh_addralign: u64,
    pub sh_entsize: u64,
}

impl Elf64Shdr {
    pub const SIZE: usize = 64;

    pub fn parse(buf: &[u8]) -> Result<Self, ElfError> {
        if buf.len() < Self::SIZE {
            return Err(ElfError::BufferTooSmall {
                required: Self::SIZE,
                actual: buf.len(),
            });
        }

        let sh_name = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
        let sh_type = u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]);
        let sh_flags = u64::from_le_bytes([
            buf[8], buf[9], buf[10], buf[11],
            buf[12], buf[13], buf[14], buf[15],
        ]);
        let sh_addr = u64::from_le_bytes([
            buf[16], buf[17], buf[18], buf[19],
            buf[20], buf[21], buf[22], buf[23],
        ]);
        let sh_offset = u64::from_le_bytes([
            buf[24], buf[25], buf[26], buf[27],
            buf[28], buf[29], buf[30], buf[31],
        ]);
        let sh_size = u64::from_le_bytes([
            buf[32], buf[33], buf[34], buf[35],
            buf[36], buf[37], buf[38], buf[39],
        ]);
        let sh_link = u32::from_le_bytes([buf[40], buf[41], buf[42], buf[43]]);
        let sh_info = u32::from_le_bytes([buf[44], buf[45], buf[46], buf[47]]);
        let sh_addralign = u64::from_le_bytes([
            buf[48], buf[49], buf[50], buf[51],
            buf[52], buf[53], buf[54], buf[55],
        ]);
        let sh_entsize = u64::from_le_bytes([
            buf[56], buf[57], buf[58], buf[59],
            buf[60], buf[61], buf[62], buf[63],
        ]);

        Ok(Self {
            sh_name,
            sh_type,
            sh_flags,
            sh_addr,
            sh_offset,
            sh_size,
            sh_link,
            sh_info,
            sh_addralign,
            sh_entsize,
        })
    }

    #[inline]
    pub fn is_allocated(&self) -> bool {
        (self.sh_flags & SHF_ALLOC) != 0
    }

    #[inline]
    pub fn is_writable(&self) -> bool {
        (self.sh_flags & SHF_WRITE) != 0
    }

    #[inline]
    pub fn is_executable(&self) -> bool {
        (self.sh_flags & SHF_EXECINSTR) != 0
    }

    #[inline]
    pub fn is_nobits(&self) -> bool {
        self.sh_type == SHT_NOBITS
    }

    pub fn type_name(&self) -> &'static str {
        match self.sh_type {
            SHT_NULL => "SHT_NULL",
            SHT_PROGBITS => "SHT_PROGBITS",
            SHT_SYMTAB => "SHT_SYMTAB",
            SHT_STRTAB => "SHT_STRTAB",
            SHT_RELA => "SHT_RELA",
            SHT_HASH => "SHT_HASH",
            SHT_DYNAMIC => "SHT_DYNAMIC",
            SHT_NOTE => "SHT_NOTE",
            SHT_NOBITS => "SHT_NOBITS",
            SHT_REL => "SHT_REL",
            SHT_SHLIB => "SHT_SHLIB",
            SHT_DYNSYM => "SHT_DYNSYM",
            SHT_INIT_ARRAY => "SHT_INIT_ARRAY",
            SHT_FINI_ARRAY => "SHT_FINI_ARRAY",
            SHT_GNU_HASH => "SHT_GNU_HASH",
            _ => "SHT_OTHER",
        }
    }
}

/// Raw 64-bit Symbol Table Entry (24 bytes).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Elf64Sym {
    pub st_name: u32,
    pub st_info: u8,
    pub st_other: u8,
    pub st_shndx: u16,
    pub st_value: u64,
    pub st_size: u64,
}

impl Elf64Sym {
    pub const SIZE: usize = 24;

    pub fn parse(buf: &[u8]) -> Result<Self, ElfError> {
        if buf.len() < Self::SIZE {
            return Err(ElfError::BufferTooSmall {
                required: Self::SIZE,
                actual: buf.len(),
            });
        }

        let st_name = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
        let st_info = buf[4];
        let st_other = buf[5];
        let st_shndx = u16::from_le_bytes([buf[6], buf[7]]);
        let st_value = u64::from_le_bytes([
            buf[8], buf[9], buf[10], buf[11],
            buf[12], buf[13], buf[14], buf[15],
        ]);
        let st_size = u64::from_le_bytes([
            buf[16], buf[17], buf[18], buf[19],
            buf[20], buf[21], buf[22], buf[23],
        ]);

        Ok(Self {
            st_name,
            st_info,
            st_other,
            st_shndx,
            st_value,
            st_size,
        })
    }

    #[inline]
    pub fn bind(&self) -> u8 {
        self.st_info >> 4
    }

    #[inline]
    pub fn sym_type(&self) -> u8 {
        self.st_info & 0x0F
    }

    #[inline]
    pub fn visibility(&self) -> u8 {
        self.st_other & 0x03
    }
}

/// Raw 64-bit Dynamic Array Entry (16 bytes).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Elf64Dyn {
    pub d_tag: i64,
    pub d_val: u64,
}

impl Elf64Dyn {
    pub const SIZE: usize = 16;

    pub fn parse(buf: &[u8]) -> Result<Self, ElfError> {
        if buf.len() < Self::SIZE {
            return Err(ElfError::BufferTooSmall {
                required: Self::SIZE,
                actual: buf.len(),
            });
        }

        let d_tag = i64::from_le_bytes([
            buf[0], buf[1], buf[2], buf[3],
            buf[4], buf[5], buf[6], buf[7],
        ]);
        let d_val = u64::from_le_bytes([
            buf[8], buf[9], buf[10], buf[11],
            buf[12], buf[13], buf[14], buf[15],
        ]);

        Ok(Self { d_tag, d_val })
    }
}

// ============================================================================
// High-Level Parsed Representations
// ============================================================================

/// Parsed Section with resolved string name and metadata.
#[derive(Debug, Clone)]
pub struct Elf64Section {
    pub index: usize,
    pub name: String,
    pub raw: Elf64Shdr,
}

/// Parsed Symbol with resolved name, binding, and type.
#[derive(Debug, Clone)]
pub struct Elf64Symbol {
    pub name: String,
    pub raw: Elf64Sym,
    pub bind: u8,
    pub sym_type: u8,
    pub visibility: u8,
}

impl Elf64Symbol {
    pub fn is_function(&self) -> bool {
        self.sym_type == STT_FUNC
    }

    pub fn is_object(&self) -> bool {
        self.sym_type == STT_OBJECT
    }

    pub fn is_global(&self) -> bool {
        self.bind == STB_GLOBAL
    }

    pub fn is_weak(&self) -> bool {
        self.bind == STB_WEAK
    }
}

/// Thread-Local Storage Segment Metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TlsSegment {
    pub vaddr: u64,
    pub filesz: u64,
    pub memsz: u64,
    pub align: u64,
    pub offset: u64,
}

impl TlsSegment {
    /// Size of uninitialized TLS zero space (.tbss).
    pub fn tbss_size(&self) -> u64 {
        if self.memsz > self.filesz {
            self.memsz - self.filesz
        } else {
            0
        }
    }
}

/// Dynamic Linking Metadata.
#[derive(Debug, Clone, Default)]
pub struct Elf64DynamicInfo {
    pub entries: Vec<Elf64Dyn>,
    pub needed_libraries: Vec<String>,
    pub soname: Option<String>,
    pub rpath: Option<String>,
    pub runpath: Option<String>,
}

/// Image loading report returned upon loading into a HybridProcess.
#[derive(Debug, Clone)]
pub struct Elf64ImageInfo {
    pub entry_point: u64,
    pub image_base: u64,
    pub image_size: usize,
    pub is_pie: bool,
    pub interpreter: Option<String>,
    pub tls: Option<TlsSegment>,
    pub needed_libraries: Vec<String>,
    pub phnum: usize,
    pub shnum: usize,
    pub symbols_count: usize,
}

// ============================================================================
// Safe String Table Helpers
// ============================================================================

/// Safely extract a null-terminated UTF-8 string from a byte buffer at an offset.
pub fn extract_string(buffer: &[u8], offset: usize) -> Result<&str, ElfError> {
    if offset >= buffer.len() {
        return Err(ElfError::StringTableError(format!(
            "String offset 0x{:X} exceeds table length {}",
            offset, buffer.len()
        )));
    }

    let slice = &buffer[offset..];
    let null_pos = slice.iter().position(|&b| b == 0).unwrap_or(slice.len());

    std::str::from_utf8(&slice[..null_pos]).map_err(|e| {
        ElfError::StringTableError(format!("Invalid UTF-8 in string table: {}", e))
    })
}

// ============================================================================
// High-Level ELF64 Parser
// ============================================================================

/// Complete parsed representation of a 64-bit ELF binary.
#[derive(Debug, Clone)]
pub struct Elf64File<'a> {
    raw_data: &'a [u8],
    ehdr: Elf64Ehdr,
    phdrs: Vec<Elf64Phdr>,
    sections: Vec<Elf64Section>,
    symbols: Vec<Elf64Symbol>,
    dynsymbols: Vec<Elf64Symbol>,
    interpreter: Option<String>,
    dynamic_info: Option<Elf64DynamicInfo>,
    tls_info: Option<TlsSegment>,
}

impl<'a> Elf64File<'a> {
    /// Deep-parse a 64-bit ELF binary from memory.
    pub fn parse(buffer: &'a [u8]) -> Result<Self, ElfError> {
        let ehdr = Elf64Ehdr::parse(buffer)?;

        // 1. Parse Program Headers
        let mut phdrs = Vec::with_capacity(ehdr.e_phnum as usize);
        let phoff = ehdr.e_phoff as usize;
        let phentsize = ehdr.e_phentsize as usize;
        let phnum = ehdr.e_phnum as usize;

        if phoff > 0 && phnum > 0 {
            if phoff + (phnum * phentsize) > buffer.len() {
                return Err(ElfError::InvalidProgramHeader(format!(
                    "Program header table out of bounds: offset 0x{:X}, total bytes {}",
                    phoff, phnum * phentsize
                )));
            }

            for i in 0..phnum {
                let entry_off = phoff + (i * phentsize);
                let phdr_bytes = &buffer[entry_off..entry_off + phentsize];
                let phdr = Elf64Phdr::parse(phdr_bytes)?;
                phdrs.push(phdr);
            }
        }

        // 2. Discover PT_INTERP and PT_TLS from Program Headers
        let mut interpreter = None;
        let mut tls_info = None;

        for phdr in &phdrs {
            if phdr.is_interp() {
                let off = phdr.p_offset as usize;
                let filesz = phdr.p_filesz as usize;
                if off + filesz <= buffer.len() && filesz > 0 {
                    let path_slice = &buffer[off..off + filesz];
                    let end = path_slice.iter().position(|&b| b == 0).unwrap_or(filesz);
                    if let Ok(path_str) = std::str::from_utf8(&path_slice[..end]) {
                        interpreter = Some(path_str.trim().to_string());
                    }
                }
            } else if phdr.is_tls() {
                tls_info = Some(TlsSegment {
                    vaddr: phdr.p_vaddr,
                    filesz: phdr.p_filesz,
                    memsz: phdr.p_memsz,
                    align: phdr.p_align,
                    offset: phdr.p_offset,
                });
            }
        }

        // 3. Parse Section Headers & Section Names String Table (.shstrtab)
        let shoff = ehdr.e_shoff as usize;
        let shentsize = ehdr.e_shentsize as usize;
        let shnum = ehdr.e_shnum as usize;
        let mut raw_shdrs = Vec::with_capacity(shnum);

        if shoff > 0 && shnum > 0 && shoff + (shnum * shentsize) <= buffer.len() {
            for i in 0..shnum {
                let entry_off = shoff + (i * shentsize);
                let shdr_bytes = &buffer[entry_off..entry_off + shentsize];
                let shdr = Elf64Shdr::parse(shdr_bytes)?;
                raw_shdrs.push(shdr);
            }
        }

        // Retrieve .shstrtab section bytes
        let mut shstrtab_data = &[][..];
        if (ehdr.e_shstrndx as usize) < raw_shdrs.len() {
            let shstr_shdr = &raw_shdrs[ehdr.e_shstrndx as usize];
            let start = shstr_shdr.sh_offset as usize;
            let size = shstr_shdr.sh_size as usize;
            if start + size <= buffer.len() {
                shstrtab_data = &buffer[start..start + size];
            }
        }

        // Build high-level sections list
        let mut sections = Vec::with_capacity(raw_shdrs.len());
        for (i, raw) in raw_shdrs.iter().enumerate() {
            let name = if !shstrtab_data.is_empty() {
                extract_string(shstrtab_data, raw.sh_name as usize)
                    .unwrap_or("")
                    .to_string()
            } else {
                String::new()
            };
            sections.push(Elf64Section {
                index: i,
                name,
                raw: *raw,
            });
        }

        // 4. Parse Symbol Tables (.symtab & .dynsym)
        let mut symbols = Vec::new();
        let mut dynsymbols = Vec::new();

        for sec in &sections {
            if sec.raw.sh_type == SHT_SYMTAB || sec.raw.sh_type == SHT_DYNSYM {
                let link_idx = sec.raw.sh_link as usize;
                let strtab_data = if link_idx < sections.len() {
                    let str_sec = &sections[link_idx];
                    let start = str_sec.raw.sh_offset as usize;
                    let size = str_sec.raw.sh_size as usize;
                    if start + size <= buffer.len() {
                        &buffer[start..start + size]
                    } else {
                        &[][..]
                    }
                } else {
                    &[][..]
                };

                let sym_start = sec.raw.sh_offset as usize;
                let sym_size = sec.raw.sh_size as usize;
                let entsize = if sec.raw.sh_entsize > 0 {
                    sec.raw.sh_entsize as usize
                } else {
                    Elf64Sym::SIZE
                };

                if sym_start + sym_size <= buffer.len() && entsize >= Elf64Sym::SIZE {
                    let count = sym_size / entsize;
                    let mut list = Vec::with_capacity(count);
                    for i in 0..count {
                        let off = sym_start + (i * entsize);
                        if let Ok(raw_sym) = Elf64Sym::parse(&buffer[off..off + entsize]) {
                            let name = if !strtab_data.is_empty() {
                                extract_string(strtab_data, raw_sym.st_name as usize)
                                    .unwrap_or("")
                                    .to_string()
                            } else {
                                String::new()
                            };

                            list.push(Elf64Symbol {
                                name,
                                raw: raw_sym,
                                bind: raw_sym.bind(),
                                sym_type: raw_sym.sym_type(),
                                visibility: raw_sym.visibility(),
                            });
                        }
                    }

                    if sec.raw.sh_type == SHT_SYMTAB {
                        symbols = list;
                    } else {
                        dynsymbols = list;
                    }
                }
            }
        }

        // 5. Parse Dynamic Segment / Section
        let mut dynamic_info = None;
        let mut dyn_offset = None;
        let mut dyn_size = None;

        for phdr in &phdrs {
            if phdr.is_dynamic() {
                dyn_offset = Some(phdr.p_offset as usize);
                dyn_size = Some(phdr.p_filesz as usize);
                break;
            }
        }

        if dyn_offset.is_none() {
            for sec in &sections {
                if sec.raw.sh_type == SHT_DYNAMIC {
                    dyn_offset = Some(sec.raw.sh_offset as usize);
                    dyn_size = Some(sec.raw.sh_size as usize);
                    break;
                }
            }
        }

        if let (Some(off), Some(sz)) = (dyn_offset, dyn_size) {
            if off + sz <= buffer.len() {
                let count = sz / Elf64Dyn::SIZE;
                let mut entries = Vec::with_capacity(count);
                let mut dyn_strtab_offset = None;

                for i in 0..count {
                    let entry_off = off + (i * Elf64Dyn::SIZE);
                    if let Ok(dyn_entry) = Elf64Dyn::parse(&buffer[entry_off..entry_off + Elf64Dyn::SIZE]) {
                        if dyn_entry.d_tag == DT_STRTAB {
                            // Virtual address or file offset of string table
                            for sec in &sections {
                                if sec.raw.sh_type == SHT_STRTAB && sec.raw.sh_addr == dyn_entry.d_val {
                                    dyn_strtab_offset = Some(sec.raw.sh_offset as usize);
                                    break;
                                }
                            }
                        }
                        let is_null = dyn_entry.d_tag == DT_NULL;
                        entries.push(dyn_entry);
                        if is_null {
                            break;
                        }
                    }
                }

                // Resolve needed libraries and tags if dynstr is accessible
                let mut needed_libraries = Vec::new();
                let mut soname = None;
                let mut rpath = None;
                let mut runpath = None;

                // If not found via sh_addr, search section by name ".dynstr"
                if dyn_strtab_offset.is_none() {
                    for sec in &sections {
                        if sec.name == ".dynstr" {
                            dyn_strtab_offset = Some(sec.raw.sh_offset as usize);
                            break;
                        }
                    }
                }

                if let Some(str_off) = dyn_strtab_offset {
                    if str_off < buffer.len() {
                        let strtab_slice = &buffer[str_off..];
                        for e in &entries {
                            match e.d_tag {
                                DT_NEEDED => {
                                    if let Ok(lib) = extract_string(strtab_slice, e.d_val as usize) {
                                        needed_libraries.push(lib.to_string());
                                    }
                                }
                                DT_SONAME => {
                                    if let Ok(so) = extract_string(strtab_slice, e.d_val as usize) {
                                        soname = Some(so.to_string());
                                    }
                                }
                                DT_RPATH => {
                                    if let Ok(rp) = extract_string(strtab_slice, e.d_val as usize) {
                                        rpath = Some(rp.to_string());
                                    }
                                }
                                DT_RUNPATH => {
                                    if let Ok(rnp) = extract_string(strtab_slice, e.d_val as usize) {
                                        runpath = Some(rnp.to_string());
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }

                dynamic_info = Some(Elf64DynamicInfo {
                    entries,
                    needed_libraries,
                    soname,
                    rpath,
                    runpath,
                });
            }
        }

        Ok(Self {
            raw_data: buffer,
            ehdr,
            phdrs,
            sections,
            symbols,
            dynsymbols,
            interpreter,
            dynamic_info,
            tls_info,
        })
    }

    #[inline]
    pub fn ehdr(&self) -> &Elf64Ehdr {
        &self.ehdr
    }

    #[inline]
    pub fn entry_point(&self) -> u64 {
        self.ehdr.e_entry
    }

    #[inline]
    pub fn is_pie(&self) -> bool {
        self.ehdr.e_type == ET_DYN
    }

    #[inline]
    pub fn is_dynamic(&self) -> bool {
        self.dynamic_info.is_some() || self.phdrs.iter().any(|p| p.is_dynamic())
    }

    #[inline]
    pub fn interpreter(&self) -> Option<&str> {
        self.interpreter.as_deref()
    }

    #[inline]
    pub fn tls(&self) -> Option<&TlsSegment> {
        self.tls_info.as_ref()
    }

    #[inline]
    pub fn program_headers(&self) -> &[Elf64Phdr] {
        &self.phdrs
    }

    pub fn load_segments(&self) -> impl Iterator<Item = &Elf64Phdr> {
        self.phdrs.iter().filter(|p| p.is_load())
    }

    #[inline]
    pub fn section_headers(&self) -> &[Elf64Section] {
        &self.sections
    }

    pub fn find_section(&self, name: &str) -> Option<&Elf64Section> {
        self.sections.iter().find(|s| s.name == name)
    }

    pub fn section_data(&self, sec: &Elf64Section) -> Result<&[u8], ElfError> {
        let start = sec.raw.sh_offset as usize;
        let size = sec.raw.sh_size as usize;
        if start + size > self.raw_data.len() {
            return Err(ElfError::InvalidSectionHeader(format!(
                "Section {} bounds [0x{:X}..0x{:X}] exceed file size {}",
                sec.name, start, start + size, self.raw_data.len()
            )));
        }
        Ok(&self.raw_data[start..start + size])
    }

    #[inline]
    pub fn symbols(&self) -> &[Elf64Symbol] {
        &self.symbols
    }

    pub fn find_symbol(&self, name: &str) -> Option<&Elf64Symbol> {
        self.symbols.iter().find(|s| s.name == name)
            .or_else(|| self.dynsymbols.iter().find(|s| s.name == name))
    }

    #[inline]
    pub fn dynsymbols(&self) -> &[Elf64Symbol] {
        &self.dynsymbols
    }

    pub fn needed_libraries(&self) -> &[String] {
        if let Some(ref d) = self.dynamic_info {
            &d.needed_libraries
        } else {
            &[]
        }
    }

    /// Calculate the memory footprint (min virtual address and total memory size).
    pub fn compute_memory_bounds(&self) -> (u64, usize) {
        let mut min_vaddr = u64::MAX;
        let mut max_vaddr = 0u64;
        let mut has_load = false;

        for seg in self.load_segments() {
            has_load = true;
            if seg.p_vaddr < min_vaddr {
                min_vaddr = seg.p_vaddr;
            }
            let seg_end = seg.p_vaddr.saturating_add(seg.p_memsz);
            if seg_end > max_vaddr {
                max_vaddr = seg_end;
            }
        }

        if !has_load || min_vaddr == u64::MAX {
            (0, 0)
        } else {
            let total_size = max_vaddr.saturating_sub(min_vaddr) as usize;
            (min_vaddr, total_size)
        }
    }

    /// Retrieve the virtual address of the program header table in memory.
    pub fn phdr_vaddr(&self) -> u64 {
        for phdr in &self.phdrs {
            if phdr.is_phdr() {
                return phdr.p_vaddr;
            }
        }
        // If no explicit PT_PHDR segment, find the PT_LOAD segment containing e_phoff
        for phdr in self.load_segments() {
            if self.ehdr.e_phoff >= phdr.p_offset
                && self.ehdr.e_phoff < phdr.p_offset + phdr.p_filesz
            {
                return phdr.p_vaddr + (self.ehdr.e_phoff - phdr.p_offset);
            }
        }
        0
    }

    /// Load binary image metadata into a HybridProcess.
    pub fn load_into_process(&self, proc: &mut HybridProcess) -> Result<Elf64ImageInfo, ElfError> {
        let (min_vaddr, total_size) = self.compute_memory_bounds();

        proc.entry_point = self.ehdr.e_entry;
        proc.image_base = min_vaddr;
        proc.image_size = total_size;

        println!("[ELF64 Advanced] Linux ELF64 Binary Process Mapping:");
        println!("                 PID:             {}", proc.pid);
        println!("                 Name:            {}", proc.name);
        println!("                 Type:            {}", if self.is_pie() { "ET_DYN (PIE)" } else { "ET_EXEC (Fixed)" });
        println!("                 Entry Point:     0x{:016X}", proc.entry_point);
        println!("                 Image Base:      0x{:016X}", proc.image_base);
        println!("                 Image Size:      0x{:X} bytes ({} KB)", proc.image_size, proc.image_size / 1024);
        println!("                 Program Headers: {} (offset 0x{:X})", self.ehdr.e_phnum, self.ehdr.e_phoff);
        println!("                 Section Headers: {} (offset 0x{:X})", self.ehdr.e_shnum, self.ehdr.e_shoff);

        if let Some(ref interp) = self.interpreter {
            println!("                 Interpreter:     \"{}\"", interp);
        }

        if let Some(ref tls) = self.tls_info {
            println!("                 TLS Segment:     VAddr=0x{:016X}, FileSz=0x{:X}, MemSz=0x{:X}, Align=0x{:X}",
                     tls.vaddr, tls.filesz, tls.memsz, tls.align);
        }

        for (idx, seg) in self.load_segments().enumerate() {
            println!("                 - PT_LOAD[{}]:    VAddr=0x{:016X}, FileSz=0x{:X}, MemSz=0x{:X}, Flags=[{}]",
                     idx, seg.p_vaddr, seg.p_filesz, seg.p_memsz, seg.flags_str());
        }

        if let Some(ref dyn_info) = self.dynamic_info {
            if !dyn_info.needed_libraries.is_empty() {
                println!("                 Dependencies:    {:?}", dyn_info.needed_libraries);
            }
        }

        Ok(Elf64ImageInfo {
            entry_point: proc.entry_point,
            image_base: proc.image_base,
            image_size: proc.image_size,
            is_pie: self.is_pie(),
            interpreter: self.interpreter.clone(),
            tls: self.tls_info,
            needed_libraries: self.needed_libraries().to_vec(),
            phnum: self.phdrs.len(),
            shnum: self.sections.len(),
            symbols_count: self.symbols.len() + self.dynsymbols.len(),
        })
    }
}

// ============================================================================
// Auxiliary Vector (Auxv) Generator
// ============================================================================

/// Single Auxiliary Vector Entry conforming to Linux ELF ABI (16 bytes).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuxvEntry {
    pub a_type: u64,
    pub a_val: u64,
}

impl AuxvEntry {
    pub const SIZE: usize = 16;

    pub fn new(a_type: u64, a_val: u64) -> Self {
        Self { a_type, a_val }
    }

    pub fn to_le_bytes(self) -> [u8; 16] {
        let mut bytes = [0u8; 16];
        bytes[0..8].copy_from_slice(&self.a_type.to_le_bytes());
        bytes[8..16].copy_from_slice(&self.a_val.to_le_bytes());
        bytes
    }
}

/// Linux Auxiliary Vector Builder.
#[derive(Debug, Clone, Default)]
pub struct AuxvBuilder {
    entries: Vec<AuxvEntry>,
}

impl AuxvBuilder {
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    pub fn add(&mut self, a_type: u64, a_val: u64) -> &mut Self {
        self.entries.push(AuxvEntry::new(a_type, a_val));
        self
    }

    pub fn set_entry(&mut self, entry_point: u64) -> &mut Self {
        self.add(AT_ENTRY, entry_point)
    }

    pub fn set_phdr(&mut self, phdr_vaddr: u64) -> &mut Self {
        self.add(AT_PHDR, phdr_vaddr)
    }

    pub fn set_phnum(&mut self, phnum: u16) -> &mut Self {
        self.add(AT_PHNUM, phnum as u64)
    }

    pub fn set_phent(&mut self, phent: u16) -> &mut Self {
        self.add(AT_PHENT, phent as u64)
    }

    pub fn set_pagesz(&mut self, pagesz: u64) -> &mut Self {
        self.add(AT_PAGESZ, pagesz)
    }

    pub fn set_base(&mut self, base: u64) -> &mut Self {
        self.add(AT_BASE, base)
    }

    pub fn set_flags(&mut self, flags: u64) -> &mut Self {
        self.add(AT_FLAGS, flags)
    }

    pub fn set_random(&mut self, random_ptr: u64) -> &mut Self {
        self.add(AT_RANDOM, random_ptr)
    }

    pub fn set_platform(&mut self, platform_ptr: u64) -> &mut Self {
        self.add(AT_PLATFORM, platform_ptr)
    }

    pub fn set_execfn(&mut self, execfn_ptr: u64) -> &mut Self {
        self.add(AT_EXECFN, execfn_ptr)
    }

    /// Convenience generator populating standard Linux auxv entries for an ELF binary.
    pub fn for_elf(
        elf: &Elf64File,
        interp_base: u64,
        page_size: u64,
        flags: u64,
    ) -> Self {
        let mut builder = Self::new();
        let phdr_vaddr = elf.phdr_vaddr();

        builder
            .set_phdr(phdr_vaddr)
            .set_phent(elf.ehdr.e_phentsize)
            .set_phnum(elf.ehdr.e_phnum)
            .set_pagesz(page_size)
            .set_base(interp_base)
            .set_flags(flags)
            .set_entry(elf.entry_point())
            .add(AT_UID, 1000)
            .add(AT_EUID, 1000)
            .add(AT_GID, 1000)
            .add(AT_EGID, 1000)
            .add(AT_CLKTCK, 100)
            .add(AT_HWCAP, 0x178BFBFF) // Standard x86_64 CPU features (SSE, SSE2, etc.)
            .add(AT_SECURE, 0);

        builder
    }

    /// Finalize auxiliary vector list ensuring termination with AT_NULL.
    pub fn build(mut self) -> Vec<AuxvEntry> {
        if !self.entries.iter().any(|e| e.a_type == AT_NULL) {
            self.entries.push(AuxvEntry::new(AT_NULL, 0));
        }
        self.entries
    }
}

// ============================================================================
// System V AMD64 ABI Initial User Stack Setup
// ============================================================================

/// Structured layout of the initial user stack image.
#[derive(Debug, Clone)]
pub struct LinuxUserStack {
    /// Initial value of the User %rsp register (guaranteed 16-byte aligned).
    pub initial_rsp: u64,
    /// High virtual memory bound of the stack.
    pub stack_top: u64,
    /// Argument count (argc).
    pub argc: u64,
    /// Pointers to argv[0..argc] in the user address space.
    pub argv_ptrs: Vec<u64>,
    /// Pointers to envp[0..envc] in the user address space.
    pub envp_ptrs: Vec<u64>,
    /// Pointer to the Auxiliary Vector array.
    pub auxv_ptr: u64,
    /// Virtual address of AT_RANDOM 16-byte seed.
    pub random_ptr: u64,
    /// Virtual address of AT_PLATFORM string.
    pub platform_ptr: u64,
    /// Virtual address of AT_EXECFN string.
    pub execfn_ptr: u64,
    /// Exact binary memory image to be mapped into the user address space
    /// from `initial_rsp` up to `stack_top`.
    pub memory_image: Vec<u8>,
}

impl LinuxUserStack {
    #[inline]
    pub fn len(&self) -> usize {
        self.memory_image.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.memory_image.is_empty()
    }

    #[inline]
    pub fn base_vaddr(&self) -> u64 {
        self.initial_rsp
    }

    /// Verify full compliance with System V AMD64 ABI requirements.
    pub fn verify_abi(&self) -> Result<(), &'static str> {
        // Rule 1: %rsp must be 16-byte aligned at process entry
        if self.initial_rsp % 16 != 0 {
            return Err("Misaligned initial stack pointer (%rsp must be 16-byte aligned)");
        }

        // Rule 2: %rsp points to argc (8 bytes)
        if self.memory_image.len() < 8 {
            return Err("Stack memory image is smaller than 8 bytes");
        }

        let first_word = u64::from_le_bytes(self.memory_image[0..8].try_into().unwrap());
        if first_word != self.argc {
            return Err("Stack top does not match argc");
        }

        // Rule 3: argv array has argc elements followed by NULL
        let argv_offset = 8;
        let argv_null_offset = argv_offset + (self.argc as usize * 8);
        if argv_null_offset + 8 > self.memory_image.len() {
            return Err("Stack image truncated during argv NULL check");
        }
        let argv_null = u64::from_le_bytes(self.memory_image[argv_null_offset..argv_null_offset + 8].try_into().unwrap());
        if argv_null != 0 {
            return Err("argv array is not terminated by NULL");
        }

        // Rule 4: envp array is terminated by NULL
        let envp_offset = argv_null_offset + 8;
        let envp_null_offset = envp_offset + (self.envp_ptrs.len() * 8);
        if envp_null_offset + 8 > self.memory_image.len() {
            return Err("Stack image truncated during envp NULL check");
        }
        let envp_null = u64::from_le_bytes(self.memory_image[envp_null_offset..envp_null_offset + 8].try_into().unwrap());
        if envp_null != 0 {
            return Err("envp array is not terminated by NULL");
        }

        Ok(())
    }
}

/// Linux System V AMD64 Initial User Stack Builder.
#[derive(Debug, Clone)]
pub struct LinuxUserStackBuilder {
    args: Vec<String>,
    envs: Vec<String>,
    auxv: Vec<AuxvEntry>,
    platform: String,
    execfn: String,
    random_bytes: [u8; 16],
}

impl Default for LinuxUserStackBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl LinuxUserStackBuilder {
    pub fn new() -> Self {
        Self {
            args: Vec::new(),
            envs: Vec::new(),
            auxv: Vec::new(),
            platform: "x86_64".to_string(),
            execfn: "/init".to_string(),
            random_bytes: [
                0x4A, 0x7B, 0x3C, 0x9D, 0x1E, 0x5F, 0x82, 0xA1,
                0x0F, 0x43, 0x78, 0x9B, 0xC2, 0xE4, 0x16, 0x3A,
            ],
        }
    }

    pub fn arg(&mut self, s: &str) -> &mut Self {
        self.args.push(s.to_string());
        self
    }

    pub fn args<I, S>(&mut self, iter: I) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        for item in iter {
            self.args.push(item.as_ref().to_string());
        }
        self
    }

    pub fn env(&mut self, s: &str) -> &mut Self {
        self.envs.push(s.to_string());
        self
    }

    pub fn envs<I, S>(&mut self, iter: I) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        for item in iter {
            self.envs.push(item.as_ref().to_string());
        }
        self
    }

    pub fn auxv_entries(&mut self, entries: &[AuxvEntry]) -> &mut Self {
        self.auxv.extend_from_slice(entries);
        self
    }

    pub fn platform(&mut self, s: &str) -> &mut Self {
        self.platform = s.to_string();
        self
    }

    pub fn execfn(&mut self, s: &str) -> &mut Self {
        self.execfn = s.to_string();
        self
    }

    pub fn random(&mut self, bytes: [u8; 16]) -> &mut Self {
        self.random_bytes = bytes;
        self
    }

    /// Construct the complete user stack byte image targeting `stack_top`.
    ///
    /// # Layout (High Address -> Low Address)
    /// [stack_top]
    /// - Platform String ("x86_64\0")
    /// - AT_RANDOM 16 random bytes
    /// - Executable Path String (execfn)
    /// - Environment Strings ("KEY=VALUE\0"...)
    /// - Argument Strings ("argv[i]\0"...)
    /// - Padding (0-15 bytes) to align RSP to 16 bytes
    /// - Auxiliary Vectors: `(type, val)..., (AT_NULL, 0)`
    /// - NULL (terminating envp)
    /// - `envp[envc-1]` ... `envp[0]` pointers
    /// - NULL (terminating argv)
    /// - `argv[argc-1]` ... `argv[0]` pointers
    /// - `argc` (u64) <- %rsp (16-byte aligned)
    pub fn build(self, stack_top: u64) -> Result<LinuxUserStack, ElfError> {
        if stack_top < 0x10000 {
            return Err(ElfError::StackBuilderError(format!(
                "Invalid stack_top address: 0x{:X}", stack_top
            )));
        }

        // 1. Pack string payloads from stack_top downward
        let mut string_arena: Vec<u8> = Vec::new();

        // Helper closure to push a slice and return its user virtual address
        let mut cur_vaddr = stack_top;

        // Platform string
        let plat_bytes = [self.platform.as_bytes(), &[0]].concat();
        cur_vaddr -= plat_bytes.len() as u64;
        let platform_ptr = cur_vaddr;
        string_arena.splice(0..0, plat_bytes);

        // 16 random bytes
        cur_vaddr -= 16;
        let random_ptr = cur_vaddr;
        string_arena.splice(0..0, self.random_bytes.to_vec());

        // Execfn string
        let execfn_bytes = [self.execfn.as_bytes(), &[0]].concat();
        cur_vaddr -= execfn_bytes.len() as u64;
        let execfn_ptr = cur_vaddr;
        string_arena.splice(0..0, execfn_bytes);

        // Environment strings
        let mut envp_ptrs = Vec::with_capacity(self.envs.len());
        for env in self.envs.iter().rev() {
            let bytes = [env.as_bytes(), &[0]].concat();
            cur_vaddr -= bytes.len() as u64;
            envp_ptrs.push(cur_vaddr);
            string_arena.splice(0..0, bytes);
        }
        envp_ptrs.reverse();

        // Argument strings
        let mut argv_ptrs = Vec::with_capacity(self.args.len());
        for arg in self.args.iter().rev() {
            let bytes = [arg.as_bytes(), &[0]].concat();
            cur_vaddr -= bytes.len() as u64;
            argv_ptrs.push(cur_vaddr);
            string_arena.splice(0..0, bytes);
        }
        argv_ptrs.reverse();

        let data_bottom_vaddr = cur_vaddr;

        // 2. Prepare Auxiliary Vector entries, updating pointer values
        let mut final_auxv = self.auxv;
        if !final_auxv.iter().any(|e| e.a_type == AT_RANDOM) {
            final_auxv.push(AuxvEntry::new(AT_RANDOM, random_ptr));
        } else {
            for e in final_auxv.iter_mut() {
                if e.a_type == AT_RANDOM && e.a_val == 0 {
                    e.a_val = random_ptr;
                }
            }
        }

        if !final_auxv.iter().any(|e| e.a_type == AT_PLATFORM) {
            final_auxv.push(AuxvEntry::new(AT_PLATFORM, platform_ptr));
        } else {
            for e in final_auxv.iter_mut() {
                if e.a_type == AT_PLATFORM && e.a_val == 0 {
                    e.a_val = platform_ptr;
                }
            }
        }

        if !final_auxv.iter().any(|e| e.a_type == AT_EXECFN) {
            final_auxv.push(AuxvEntry::new(AT_EXECFN, execfn_ptr));
        } else {
            for e in final_auxv.iter_mut() {
                if e.a_type == AT_EXECFN && e.a_val == 0 {
                    e.a_val = execfn_ptr;
                }
            }
        }

        // Ensure AT_NULL termination
        if !final_auxv.iter().any(|e| e.a_type == AT_NULL) {
            final_auxv.push(AuxvEntry::new(AT_NULL, 0));
        }

        // 3. Compute size of the pointer and auxv table:
        // - argc (1)
        // - argv pointers (argc)
        // - NULL (1)
        // - envp pointers (envc)
        // - NULL (1)
        // - auxv entries (auxv_count * 2 words)
        let argc = self.args.len() as u64;
        let envc = self.envs.len();
        let auxv_words = final_auxv.len() * 2;
        let total_words = 1 + (argc as usize) + 1 + envc + 1 + auxv_words;
        let table_size_bytes = total_words * 8;

        // 4. Align initial_rsp downwards to 16 bytes:
        // System V AMD64 ABI requires: %rsp % 16 == 0
        let target_unaligned_rsp = data_bottom_vaddr.checked_sub(table_size_bytes as u64)
            .ok_or_else(|| ElfError::StackBuilderError("Stack pointer underflow".into()))?;
        let initial_rsp = target_unaligned_rsp & !0x0F;

        // Zero padding between end of table and data_bottom
        let padding_bytes = (data_bottom_vaddr - (initial_rsp + table_size_bytes as u64)) as usize;

        // 5. Serialize pointer table into byte vector
        let mut table_bytes = Vec::with_capacity(table_size_bytes);

        // Push argc
        table_bytes.extend_from_slice(&argc.to_le_bytes());

        // Push argv pointers
        for &ptr in &argv_ptrs {
            table_bytes.extend_from_slice(&ptr.to_le_bytes());
        }
        // argv NULL terminator
        table_bytes.extend_from_slice(&0u64.to_le_bytes());

        // Push envp pointers
        for &ptr in &envp_ptrs {
            table_bytes.extend_from_slice(&ptr.to_le_bytes());
        }
        // envp NULL terminator
        table_bytes.extend_from_slice(&0u64.to_le_bytes());

        // Push auxiliary vectors
        let auxv_ptr = initial_rsp + table_bytes.len() as u64;
        for entry in &final_auxv {
            table_bytes.extend_from_slice(&entry.to_le_bytes());
        }

        // 6. Assemble complete memory image: [table] + [padding] + [strings]
        let mut memory_image = Vec::with_capacity(table_size_bytes + padding_bytes + string_arena.len());
        memory_image.extend_from_slice(&table_bytes);
        if padding_bytes > 0 {
            memory_image.resize(memory_image.len() + padding_bytes, 0);
        }
        memory_image.extend_from_slice(&string_arena);

        let stack = LinuxUserStack {
            initial_rsp,
            stack_top,
            argc,
            argv_ptrs,
            envp_ptrs,
            auxv_ptr,
            random_ptr,
            platform_ptr,
            execfn_ptr,
            memory_image,
        };

        // Self-validate compliance
        if let Err(e) = stack.verify_abi() {
            return Err(ElfError::StackBuilderError(e.to_string()));
        }

        Ok(stack)
    }
}

// ============================================================================
// Public Loader API
// ============================================================================

/// Load a 64-bit ELF executable into a HybridProcess using the Advanced Engine.
pub fn load_elf64_advanced(
    proc: &mut HybridProcess,
    buffer: &[u8],
) -> Result<Elf64ImageInfo, NtStatus> {
    let elf = Elf64File::parse(buffer)?;
    let info = elf.load_into_process(proc)?;
    Ok(info)
}

/// Load a 64-bit ELF executable and initialize its complete System V AMD64 user stack.
pub fn load_elf64_with_user_stack(
    proc: &mut HybridProcess,
    buffer: &[u8],
    args: &[&str],
    envs: &[&str],
    stack_top: u64,
) -> Result<(Elf64ImageInfo, LinuxUserStack), NtStatus> {
    let elf = Elf64File::parse(buffer)?;
    let info = elf.load_into_process(proc)?;

    let auxv = AuxvBuilder::for_elf(&elf, 0, 4096, 0).build();

    let mut stack_builder = LinuxUserStackBuilder::new();
    stack_builder.args(args);
    stack_builder.envs(envs);
    stack_builder.auxv_entries(&auxv);
    if let Some(first_arg) = args.first() {
        stack_builder.execfn(first_arg);
    }

    let stack = stack_builder.build(stack_top)?;

    println!("[ELF64 Advanced] System V AMD64 Initial User Stack Prepared:");
    println!("                 %rsp (Entry):    0x{:016X} (Aligned-16: {})",
             stack.initial_rsp, stack.initial_rsp % 16 == 0);
    println!("                 Stack Top:       0x{:016X}", stack.stack_top);
    println!("                 Image Size:      {} bytes", stack.len());
    println!("                 argc:            {}", stack.argc);
    println!("                 argv Pointers:   {} entries", stack.argv_ptrs.len());
    println!("                 envp Pointers:   {} entries", stack.envp_ptrs.len());
    println!("                 auxv Vector:     at 0x{:016X}", stack.auxv_ptr);

    Ok((info, stack))
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::personality::ExecutionPersonality;

    fn create_synthetic_elf64() -> Vec<u8> {
        let mut buf = vec![0u8; 1024];

        // 1. ELF Header
        buf[0] = 0x7F; buf[1] = b'E'; buf[2] = b'L'; buf[3] = b'F';
        buf[EI_CLASS] = ELFCLASS64;
        buf[EI_DATA] = ELFDATA2LSB;
        buf[EI_VERSION] = EV_CURRENT;
        buf[EI_OSABI] = ELFOSABI_LINUX;

        // e_type = ET_EXEC (2)
        buf[16] = 2; buf[17] = 0;
        // e_machine = EM_X86_64 (62)
        buf[18] = 62; buf[19] = 0;
        // e_version = 1
        buf[20] = 1;
        // e_entry = 0x401000
        buf[24..32].copy_from_slice(&0x401000u64.to_le_bytes());
        // e_phoff = 64
        buf[32..40].copy_from_slice(&64u64.to_le_bytes());
        // e_shoff = 256
        buf[40..48].copy_from_slice(&256u64.to_le_bytes());
        // e_ehsize = 64
        buf[52..54].copy_from_slice(&64u16.to_le_bytes());
        // e_phentsize = 56
        buf[54..56].copy_from_slice(&56u16.to_le_bytes());
        // e_phnum = 3 (PT_PHDR, PT_INTERP, PT_LOAD)
        buf[56..58].copy_from_slice(&3u16.to_le_bytes());
        // e_shentsize = 64
        buf[58..60].copy_from_slice(&64u16.to_le_bytes());
        // e_shnum = 3 (NULL, .text, .shstrtab)
        buf[60..62].copy_from_slice(&3u16.to_le_bytes());
        // e_shstrndx = 2
        buf[62..64].copy_from_slice(&2u16.to_le_bytes());

        // 2. Program Headers at offset 64
        // Phdr 0: PT_PHDR
        let ph0 = 64;
        buf[ph0..ph0 + 4].copy_from_slice(&PT_PHDR.to_le_bytes());
        buf[ph0 + 4..ph0 + 8].copy_from_slice(&PF_R.to_le_bytes());
        buf[ph0 + 8..ph0 + 16].copy_from_slice(&64u64.to_le_bytes());      // offset
        buf[ph0 + 16..ph0 + 24].copy_from_slice(&0x400040u64.to_le_bytes()); // vaddr
        buf[ph0 + 32..ph0 + 40].copy_from_slice(&168u64.to_le_bytes());   // filesz
        buf[ph0 + 40..ph0 + 48].copy_from_slice(&168u64.to_le_bytes());   // memsz
        buf[ph0 + 48..ph0 + 56].copy_from_slice(&8u64.to_le_bytes());     // align

        // Phdr 1: PT_INTERP
        let interp_str = b"/lib64/ld-linux-x86-64.so.2\0";
        let interp_off = 512;
        buf[interp_off..interp_off + interp_str.len()].copy_from_slice(interp_str);

        let ph1 = 64 + 56;
        buf[ph1..ph1 + 4].copy_from_slice(&PT_INTERP.to_le_bytes());
        buf[ph1 + 4..ph1 + 8].copy_from_slice(&PF_R.to_le_bytes());
        buf[ph1 + 8..ph1 + 16].copy_from_slice(&(interp_off as u64).to_le_bytes());
        buf[ph1 + 16..ph1 + 24].copy_from_slice(&0x400200u64.to_le_bytes());
        buf[ph1 + 32..ph1 + 40].copy_from_slice(&(interp_str.len() as u64).to_le_bytes());
        buf[ph1 + 40..ph1 + 48].copy_from_slice(&(interp_str.len() as u64).to_le_bytes());
        buf[ph1 + 48..ph1 + 56].copy_from_slice(&1u64.to_le_bytes());

        // Phdr 2: PT_LOAD
        let ph2 = 64 + 112;
        buf[ph2..ph2 + 4].copy_from_slice(&PT_LOAD.to_le_bytes());
        buf[ph2 + 4..ph2 + 8].copy_from_slice(&(PF_R | PF_X).to_le_bytes());
        buf[ph2 + 8..ph2 + 16].copy_from_slice(&0u64.to_le_bytes());
        buf[ph2 + 16..ph2 + 24].copy_from_slice(&0x400000u64.to_le_bytes());
        buf[ph2 + 32..ph2 + 40].copy_from_slice(&0x2000u64.to_le_bytes());
        buf[ph2 + 40..ph2 + 48].copy_from_slice(&0x4000u64.to_le_bytes());
        buf[ph2 + 48..ph2 + 56].copy_from_slice(&0x1000u64.to_le_bytes());

        // 3. Section Headers at offset 256
        // Shdr 0: SHT_NULL
        // Shdr 1: .text
        let sh1 = 256 + 64;
        buf[sh1..sh1 + 4].copy_from_slice(&1u32.to_le_bytes()); // sh_name offset in shstrtab
        buf[sh1 + 4..sh1 + 8].copy_from_slice(&SHT_PROGBITS.to_le_bytes());
        buf[sh1 + 8..sh1 + 16].copy_from_slice(&(SHF_ALLOC | SHF_EXECINSTR).to_le_bytes());
        buf[sh1 + 16..sh1 + 24].copy_from_slice(&0x401000u64.to_le_bytes());
        buf[sh1 + 24..sh1 + 32].copy_from_slice(&0x1000u64.to_le_bytes());
        buf[sh1 + 32..sh1 + 40].copy_from_slice(&0x200u64.to_le_bytes());

        // Shdr 2: .shstrtab
        let shstrtab_off = 600;
        let shstrtab_content = b"\0.text\0.shstrtab\0";
        buf[shstrtab_off..shstrtab_off + shstrtab_content.len()].copy_from_slice(shstrtab_content);

        let sh2 = 256 + 128;
        buf[sh2..sh2 + 4].copy_from_slice(&7u32.to_le_bytes()); // sh_name for .shstrtab
        buf[sh2 + 4..sh2 + 8].copy_from_slice(&SHT_STRTAB.to_le_bytes());
        buf[sh2 + 24..sh2 + 32].copy_from_slice(&(shstrtab_off as u64).to_le_bytes());
        buf[sh2 + 32..sh2 + 40].copy_from_slice(&(shstrtab_content.len() as u64).to_le_bytes());

        buf
    }

    #[test]
    fn test_elf64_parsing() {
        let binary = create_synthetic_elf64();
        let elf = Elf64File::parse(&binary).expect("Failed to parse synthetic ELF");

        assert_eq!(elf.entry_point(), 0x401000);
        assert!(!elf.is_pie());
        assert_eq!(elf.interpreter(), Some("/lib64/ld-linux-x86-64.so.2"));

        let (base, size) = elf.compute_memory_bounds();
        assert_eq!(base, 0x400000);
        assert_eq!(size, 0x4000);

        let text_sec = elf.find_section(".text");
        assert!(text_sec.is_some());
        let text = text_sec.unwrap();
        assert_eq!(text.raw.sh_addr, 0x401000);
        assert!(text.raw.is_executable());
        assert!(text.raw.is_allocated());
    }

    #[test]
    fn test_auxv_generation() {
        let binary = create_synthetic_elf64();
        let elf = Elf64File::parse(&binary).unwrap();

        let auxv = AuxvBuilder::for_elf(&elf, 0x7FFF0000, 4096, 0).build();

        assert!(auxv.iter().any(|e| e.a_type == AT_ENTRY && e.a_val == 0x401000));
        assert!(auxv.iter().any(|e| e.a_type == AT_BASE && e.a_val == 0x7FFF0000));
        assert!(auxv.iter().any(|e| e.a_type == AT_PAGESZ && e.a_val == 4096));
        assert!(auxv.iter().any(|e| e.a_type == AT_PHNUM && e.a_val == 3));
        assert!(auxv.iter().any(|e| e.a_type == AT_NULL && e.a_val == 0));
    }

    #[test]
    fn test_linux_user_stack_abi() {
        let stack_top = 0x7FFF_FFFF_0000u64;
        let mut builder = LinuxUserStackBuilder::new();
        builder
            .args(&["/bin/bash", "-c", "echo HybridOS"])
            .envs(&["PATH=/bin:/usr/bin", "HOME=/root", "TERM=linux"])
            .auxv_entries(&[
                AuxvEntry::new(AT_ENTRY, 0x401000),
                AuxvEntry::new(AT_PAGESZ, 4096),
                AuxvEntry::new(AT_NULL, 0),
            ]);

        let stack = builder.build(stack_top).expect("User stack build failed");

        // Verify System V AMD64 ABI 16-byte alignment
        assert_eq!(stack.initial_rsp % 16, 0, "%rsp must be 16-byte aligned");
        assert_eq!(stack.argc, 3);
        assert_eq!(stack.argv_ptrs.len(), 3);
        assert_eq!(stack.envp_ptrs.len(), 3);
        assert!(stack.initial_rsp < stack.stack_top);

        // Verify verification helper
        assert!(stack.verify_abi().is_ok());
    }

    #[test]
    fn test_load_into_process() {
        let binary = create_synthetic_elf64();
        let mut proc = HybridProcess::new(42, "test_proc", ExecutionPersonality::LinuxPosix);
        let info = load_elf64_advanced(&mut proc, &binary).expect("Failed to load into process");

        assert_eq!(proc.entry_point, 0x401000);
        assert_eq!(proc.image_base, 0x400000);
        assert_eq!(proc.image_size, 0x4000);
        assert_eq!(info.entry_point, 0x401000);
    }

    #[test]
    fn test_stack_alignment_permutations() {
        let stack_top = 0x7FFF_FFFF_0000u64;

        // Test multiple permutations of argument and environment counts
        for num_args in 0..=5 {
            for num_envs in 0..=4 {
                let args: Vec<String> = (0..num_args).map(|i| format!("arg{}", i)).collect();
                let envs: Vec<String> = (0..num_envs).map(|i| format!("ENV_{}=val", i)).collect();

                let mut builder = LinuxUserStackBuilder::new();
                builder.args(&args);
                builder.envs(&envs);
                builder.auxv_entries(&[
                    AuxvEntry::new(AT_ENTRY, 0x401000),
                    AuxvEntry::new(AT_PAGESZ, 4096),
                    AuxvEntry::new(AT_PHDR, 0x400040),
                    AuxvEntry::new(AT_PHNUM, 3),
                    AuxvEntry::new(AT_PHENT, 56),
                    AuxvEntry::new(AT_BASE, 0),
                    AuxvEntry::new(AT_FLAGS, 0),
                    AuxvEntry::new(AT_NULL, 0),
                ]);

                let stack = builder.build(stack_top).expect("Build permutation failed");

                // Check 16-byte alignment
                assert_eq!(
                    stack.initial_rsp % 16,
                    0,
                    "Failed 16-byte alignment with num_args={}, num_envs={}",
                    num_args,
                    num_envs
                );
                assert_eq!(stack.argc, num_args as u64);
                assert_eq!(stack.argv_ptrs.len(), num_args);
                assert_eq!(stack.envp_ptrs.len(), num_envs);
                assert!(stack.verify_abi().is_ok());
            }
        }
    }

    #[test]
    fn test_malformed_elf_error_handling() {
        // Truncated buffer (< 64 bytes)
        let short_buf = vec![0x7F, b'E', b'L', b'F'];
        assert!(matches!(Elf64File::parse(&short_buf), Err(ElfError::BufferTooSmall { .. })));

        // Bad magic
        let mut bad_magic = create_synthetic_elf64();
        bad_magic[0] = 0x00;
        assert!(matches!(Elf64File::parse(&bad_magic), Err(ElfError::InvalidMagic(_))));

        // 32-bit ELF (ELFCLASS32 = 1)
        let mut class32 = create_synthetic_elf64();
        class32[EI_CLASS] = ELFCLASS32;
        assert!(matches!(Elf64File::parse(&class32), Err(ElfError::UnsupportedClass(1))));

        // Big Endian (ELFDATA2MSB = 2)
        let mut big_endian = create_synthetic_elf64();
        big_endian[EI_DATA] = ELFDATA2MSB;
        assert!(matches!(Elf64File::parse(&big_endian), Err(ElfError::UnsupportedEndian(2))));

        // Non-x86_64 architecture (EM_386 = 3)
        let mut i386 = create_synthetic_elf64();
        i386[18] = 3; i386[19] = 0;
        assert!(matches!(Elf64File::parse(&i386), Err(ElfError::UnsupportedArchitecture(3))));
    }

    #[test]
    fn test_dynamic_and_tls_parsing() {
        let mut buf = vec![0u8; 2048];

        // ELF Header
        buf[0] = 0x7F; buf[1] = b'E'; buf[2] = b'L'; buf[3] = b'F';
        buf[EI_CLASS] = ELFCLASS64;
        buf[EI_DATA] = ELFDATA2LSB;
        buf[EI_VERSION] = EV_CURRENT;
        buf[EI_OSABI] = ELFOSABI_LINUX;
        buf[16] = 3; buf[17] = 0; // ET_DYN (PIE)
        buf[18] = 62; buf[19] = 0; // EM_X86_64
        buf[20] = 1;
        buf[24..32].copy_from_slice(&0x2000u64.to_le_bytes()); // e_entry
        buf[32..40].copy_from_slice(&64u64.to_le_bytes());    // e_phoff
        buf[40..48].copy_from_slice(&512u64.to_le_bytes());   // e_shoff
        buf[52..54].copy_from_slice(&64u16.to_le_bytes());
        buf[54..56].copy_from_slice(&56u16.to_le_bytes());
        buf[56..58].copy_from_slice(&3u16.to_le_bytes());     // 3 Phdrs: LOAD, DYNAMIC, TLS
        buf[58..60].copy_from_slice(&64u16.to_le_bytes());
        buf[60..62].copy_from_slice(&2u16.to_le_bytes());     // 2 Shdrs: NULL, .dynstr
        buf[62..64].copy_from_slice(&1u16.to_le_bytes());     // shstrndx

        // Phdr 0: PT_LOAD
        let ph0 = 64;
        buf[ph0..ph0 + 4].copy_from_slice(&PT_LOAD.to_le_bytes());
        buf[ph0 + 4..ph0 + 8].copy_from_slice(&(PF_R | PF_X).to_le_bytes());
        buf[ph0 + 8..ph0 + 16].copy_from_slice(&0u64.to_le_bytes());
        buf[ph0 + 16..ph0 + 24].copy_from_slice(&0x1000u64.to_le_bytes());
        buf[ph0 + 32..ph0 + 40].copy_from_slice(&0x1000u64.to_le_bytes());
        buf[ph0 + 40..ph0 + 48].copy_from_slice(&0x1000u64.to_le_bytes());
        buf[ph0 + 48..ph0 + 56].copy_from_slice(&0x1000u64.to_le_bytes());

        // Phdr 1: PT_DYNAMIC
        let dyn_off = 1024;
        let ph1 = 64 + 56;
        buf[ph1..ph1 + 4].copy_from_slice(&PT_DYNAMIC.to_le_bytes());
        buf[ph1 + 4..ph1 + 8].copy_from_slice(&(PF_R | PF_W).to_le_bytes());
        buf[ph1 + 8..ph1 + 16].copy_from_slice(&(dyn_off as u64).to_le_bytes());
        buf[ph1 + 16..ph1 + 24].copy_from_slice(&0x3000u64.to_le_bytes());
        buf[ph1 + 32..ph1 + 40].copy_from_slice(&48u64.to_le_bytes()); // 3 entries * 16
        buf[ph1 + 40..ph1 + 48].copy_from_slice(&48u64.to_le_bytes());
        buf[ph1 + 48..ph1 + 56].copy_from_slice(&8u64.to_le_bytes());

        // Dynamic entries:
        // Entry 0: DT_NEEDED, val: 1 (offset in .dynstr)
        buf[dyn_off..dyn_off + 8].copy_from_slice(&(DT_NEEDED as u64).to_le_bytes());
        buf[dyn_off + 8..dyn_off + 16].copy_from_slice(&1u64.to_le_bytes());
        // Entry 1: DT_SONAME, val: 11
        buf[dyn_off + 16..dyn_off + 24].copy_from_slice(&(DT_SONAME as u64).to_le_bytes());
        buf[dyn_off + 24..dyn_off + 32].copy_from_slice(&11u64.to_le_bytes());
        // Entry 2: DT_NULL
        buf[dyn_off + 32..dyn_off + 40].copy_from_slice(&0u64.to_le_bytes());
        buf[dyn_off + 40..dyn_off + 48].copy_from_slice(&0u64.to_le_bytes());

        // Phdr 2: PT_TLS
        let ph2 = 64 + 112;
        buf[ph2..ph2 + 4].copy_from_slice(&PT_TLS.to_le_bytes());
        buf[ph2 + 4..ph2 + 8].copy_from_slice(&PF_R.to_le_bytes());
        buf[ph2 + 8..ph2 + 16].copy_from_slice(&1200u64.to_le_bytes());
        buf[ph2 + 16..ph2 + 24].copy_from_slice(&0x5000u64.to_le_bytes());
        buf[ph2 + 32..ph2 + 40].copy_from_slice(&0x40u64.to_le_bytes()); // filesz (.tdata)
        buf[ph2 + 40..ph2 + 48].copy_from_slice(&0x100u64.to_le_bytes()); // memsz (.tdata + .tbss)
        buf[ph2 + 48..ph2 + 56].copy_from_slice(&16u64.to_le_bytes());

        // Shdr 1: .dynstr
        let dynstr_off = 1500;
        let dynstr_content = b"\0libc.so.6\0app.so\0";
        buf[dynstr_off..dynstr_off + dynstr_content.len()].copy_from_slice(dynstr_content);

        let sh1 = 512 + 64;
        buf[sh1 + 4..sh1 + 8].copy_from_slice(&SHT_STRTAB.to_le_bytes());
        buf[sh1 + 24..sh1 + 32].copy_from_slice(&(dynstr_off as u64).to_le_bytes());
        buf[sh1 + 32..sh1 + 40].copy_from_slice(&(dynstr_content.len() as u64).to_le_bytes());

        let elf = Elf64File::parse(&buf).expect("Failed to parse dynamic ELF");

        assert!(elf.is_pie());
        assert!(elf.is_dynamic());

        let tls = elf.tls().expect("Missing TLS info");
        assert_eq!(tls.vaddr, 0x5000);
        assert_eq!(tls.filesz, 0x40);
        assert_eq!(tls.memsz, 0x100);
        assert_eq!(tls.tbss_size(), 0xC0); // 0x100 - 0x40 = 0xC0
        assert_eq!(tls.align, 16);
    }
}

