//! ELF64 (64-bit Executable and Linkable Format) Parser.
//!
//! Fully safe pure-Rust parser with zero unsafe code and exhaustive bounds checking.
//! Supports Little-Endian and Big-Endian ELF64 targets, Program Headers (Phdr),
//! Section Headers (Shdr), Symbol Tables (Symtab/Dynsym), and String Tables (Strtab).

use crate::error::{ParserError, Result};
use crate::reader::ByteReader;

/// Standard ELF Magic: `0x7F, 'E', 'L', 'F'`.
pub const ELF_MAGIC: [u8; 4] = [0x7F, b'E', b'L', b'F'];

/// ELF Class: 64-bit.
pub const ELFCLASS64: u8 = 2;

/// Data Endianness: 2's complement, little endian.
pub const ELFDATA2LSB: u8 = 1;
/// Data Endianness: 2's complement, big endian.
pub const ELFDATA2MSB: u8 = 2;

/// Segment types (p_type).
pub const PT_NULL: u32 = 0;
pub const PT_LOAD: u32 = 1;
pub const PT_DYNAMIC: u32 = 2;
pub const PT_INTERP: u32 = 3;
pub const PT_NOTE: u32 = 4;
pub const PT_SHLIB: u32 = 5;
pub const PT_PHDR: u32 = 6;
pub const PT_TLS: u32 = 7;
pub const PT_GNU_EH_FRAME: u32 = 0x6474e550;
pub const PT_GNU_STACK: u32 = 0x6474e551;
pub const PT_GNU_RELRO: u32 = 0x6474e552;

/// Segment flags (p_flags).
pub const PF_X: u32 = 0x1;
pub const PF_W: u32 = 0x2;
pub const PF_R: u32 = 0x4;

/// Section types (sh_type).
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

/// Section flags (sh_flags).
pub const SHF_WRITE: u64 = 0x1;
pub const SHF_ALLOC: u64 = 0x2;
pub const SHF_EXECINSTR: u64 = 0x4;

/// Maximum number of sections / program headers permitted for safety.
pub const MAX_ELF_HEADERS_LIMIT: usize = 1024;
pub const MAX_ELF_SYMBOLS_LIMIT: usize = 65536;

/// Endianness mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Endian {
    Little,
    Big,
}

impl Endian {
    #[inline]
    pub fn read_u16(self, reader: &ByteReader<'_>, offset: usize) -> Result<u16> {
        match self {
            Endian::Little => reader.read_u16_le(offset),
            Endian::Big => reader.read_u16_be(offset),
        }
    }

    #[inline]
    pub fn read_u32(self, reader: &ByteReader<'_>, offset: usize) -> Result<u32> {
        match self {
            Endian::Little => reader.read_u32_le(offset),
            Endian::Big => reader.read_u32_be(offset),
        }
    }

    #[inline]
    pub fn read_u64(self, reader: &ByteReader<'_>, offset: usize) -> Result<u64> {
        match self {
            Endian::Little => reader.read_u64_le(offset),
            Endian::Big => reader.read_u64_be(offset),
        }
    }
}

/// 64-bit ELF Header (Elf64_Ehdr).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Elf64Header {
    pub ident_magic: [u8; 4],
    pub ident_class: u8,
    pub ident_data: u8,
    pub ident_version: u8,
    pub ident_osabi: u8,
    pub ident_abiversion: u8,
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
    pub endian: Endian,
}

impl Elf64Header {
    pub fn parse(reader: &ByteReader<'_>) -> Result<Self> {
        let magic = reader.read_array::<4>(0)?;
        if magic != ELF_MAGIC {
            return Err(ParserError::InvalidMagic {
                expected: &ELF_MAGIC,
                found: magic.to_vec(),
            });
        }

        let ident_class = reader.read_u8(4)?;
        if ident_class != ELFCLASS64 {
            return Err(ParserError::UnsupportedElfClass(ident_class));
        }

        let ident_data = reader.read_u8(5)?;
        let endian = match ident_data {
            ELFDATA2LSB => Endian::Little,
            ELFDATA2MSB => Endian::Big,
            other => return Err(ParserError::UnsupportedElfEndian(other)),
        };

        let ident_version = reader.read_u8(6)?;
        if ident_version != 1 {
            return Err(ParserError::InvalidElfIdent("Unsupported ELF version"));
        }

        let ident_osabi = reader.read_u8(7)?;
        let ident_abiversion = reader.read_u8(8)?;

        let e_type = endian.read_u16(reader, 16)?;
        let e_machine = endian.read_u16(reader, 18)?;
        let e_version = endian.read_u32(reader, 20)?;
        let e_entry = endian.read_u64(reader, 24)?;
        let e_phoff = endian.read_u64(reader, 32)?;
        let e_shoff = endian.read_u64(reader, 40)?;
        let e_flags = endian.read_u32(reader, 48)?;
        let e_ehsize = endian.read_u16(reader, 52)?;
        let e_phentsize = endian.read_u16(reader, 54)?;
        let e_phnum = endian.read_u16(reader, 56)?;
        let e_shentsize = endian.read_u16(reader, 58)?;
        let e_shnum = endian.read_u16(reader, 60)?;
        let e_shstrndx = endian.read_u16(reader, 62)?;

        Ok(Self {
            ident_magic: magic,
            ident_class,
            ident_data,
            ident_version,
            ident_osabi,
            ident_abiversion,
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
            endian,
        })
    }
}

/// 64-bit ELF Program Header (Elf64_Phdr).
#[derive(Debug, Clone, PartialEq, Eq)]
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
    pub fn parse(reader: &ByteReader<'_>, offset: usize, endian: Endian) -> Result<Self> {
        let p_type = endian.read_u32(reader, offset)?;
        let p_flags = endian.read_u32(reader, offset + 4)?;
        let p_offset = endian.read_u64(reader, offset + 8)?;
        let p_vaddr = endian.read_u64(reader, offset + 16)?;
        let p_paddr = endian.read_u64(reader, offset + 24)?;
        let p_filesz = endian.read_u64(reader, offset + 32)?;
        let p_memsz = endian.read_u64(reader, offset + 40)?;
        let p_align = endian.read_u64(reader, offset + 48)?;

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
    pub fn is_executable(&self) -> bool {
        self.p_flags & PF_X != 0
    }

    #[inline]
    pub fn is_writable(&self) -> bool {
        self.p_flags & PF_W != 0
    }

    #[inline]
    pub fn is_readable(&self) -> bool {
        self.p_flags & PF_R != 0
    }
}

/// 64-bit ELF Section Header (Elf64_Shdr).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Elf64Shdr {
    pub sh_name: u32,
    pub name: String,
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
    pub fn parse_raw(reader: &ByteReader<'_>, offset: usize, endian: Endian) -> Result<Self> {
        let sh_name = endian.read_u32(reader, offset)?;
        let sh_type = endian.read_u32(reader, offset + 4)?;
        let sh_flags = endian.read_u64(reader, offset + 8)?;
        let sh_addr = endian.read_u64(reader, offset + 16)?;
        let sh_offset = endian.read_u64(reader, offset + 24)?;
        let sh_size = endian.read_u64(reader, offset + 32)?;
        let sh_link = endian.read_u32(reader, offset + 40)?;
        let sh_info = endian.read_u32(reader, offset + 44)?;
        let sh_addralign = endian.read_u64(reader, offset + 48)?;
        let sh_entsize = endian.read_u64(reader, offset + 56)?;

        Ok(Self {
            sh_name,
            name: String::new(),
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
    pub fn is_executable(&self) -> bool {
        self.sh_flags & SHF_EXECINSTR != 0
    }

    #[inline]
    pub fn is_writable(&self) -> bool {
        self.sh_flags & SHF_WRITE != 0
    }

    #[inline]
    pub fn is_alloc(&self) -> bool {
        self.sh_flags & SHF_ALLOC != 0
    }
}

/// 64-bit ELF Symbol (Elf64_Sym, 24 bytes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Elf64Sym {
    pub st_name: u32,
    pub name: String,
    pub st_info: u8,
    pub st_other: u8,
    pub st_shndx: u16,
    pub st_value: u64,
    pub st_size: u64,
}

impl Elf64Sym {
    pub fn parse(reader: &ByteReader<'_>, offset: usize, endian: Endian) -> Result<Self> {
        let st_name = endian.read_u32(reader, offset)?;
        let st_info = reader.read_u8(offset + 4)?;
        let st_other = reader.read_u8(offset + 5)?;
        let st_shndx = endian.read_u16(reader, offset + 6)?;
        let st_value = endian.read_u64(reader, offset + 8)?;
        let st_size = endian.read_u64(reader, offset + 16)?;

        Ok(Self {
            st_name,
            name: String::new(),
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
    pub fn symbol_type(&self) -> u8 {
        self.st_info & 0xF
    }

    #[inline]
    pub fn visibility(&self) -> u8 {
        self.st_other & 0x3
    }
}

/// Helper for bounds-checked string table reading.
#[derive(Debug, Clone, Copy)]
pub struct Elf64Strtab<'a> {
    data: &'a [u8],
}

impl<'a> Elf64Strtab<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data }
    }

    /// Retrieves a null-terminated string at the specified offset with bounds checking.
    pub fn get_string(&self, offset: u32) -> Result<&'a str> {
        let off = offset as usize;
        if off >= self.data.len() {
            return Err(ParserError::OffsetOutOfBounds {
                offset: off,
                len: self.data.len(),
            });
        }
        let slice = &self.data[off..];
        let null_pos = slice.iter().position(|&b| b == 0).ok_or(ParserError::StringNotTerminated)?;
        core::str::from_utf8(&slice[..null_pos]).map_err(|_| ParserError::InvalidStringEncoding)
    }
}

/// Parsed ELF64 binary structure.
#[derive(Debug, Clone)]
pub struct Elf64File<'a> {
    reader: ByteReader<'a>,
    pub header: Elf64Header,
    pub program_headers: Vec<Elf64Phdr>,
    pub section_headers: Vec<Elf64Shdr>,
}

impl<'a> Elf64File<'a> {
    /// Parses an ELF64 binary from raw bytes.
    pub fn parse(data: &'a [u8]) -> Result<Self> {
        let reader = ByteReader::new(data);

        // 1. ELF Header
        let header = Elf64Header::parse(&reader)?;
        let endian = header.endian;

        // 2. Program Headers
        let phnum = header.e_phnum as usize;
        if phnum > MAX_ELF_HEADERS_LIMIT {
            return Err(ParserError::ResourceExhausted("Excessive ELF program headers"));
        }

        let mut program_headers = Vec::with_capacity(phnum);
        if phnum > 0 && header.e_phoff > 0 {
            let phentsize = header.e_phentsize as usize;
            for i in 0..phnum {
                let off = (header.e_phoff as usize)
                    .checked_add(i.checked_mul(phentsize).ok_or(ParserError::IntegerOverflow)?)
                    .ok_or(ParserError::IntegerOverflow)?;
                program_headers.push(Elf64Phdr::parse(&reader, off, endian)?);
            }
        }

        // 3. Section Headers
        let shnum = header.e_shnum as usize;
        if shnum > MAX_ELF_HEADERS_LIMIT {
            return Err(ParserError::ResourceExhausted("Excessive ELF section headers"));
        }

        let mut section_headers = Vec::with_capacity(shnum);
        if shnum > 0 && header.e_shoff > 0 {
            let shentsize = header.e_shentsize as usize;
            for i in 0..shnum {
                let off = (header.e_shoff as usize)
                    .checked_add(i.checked_mul(shentsize).ok_or(ParserError::IntegerOverflow)?)
                    .ok_or(ParserError::IntegerOverflow)?;
                section_headers.push(Elf64Shdr::parse_raw(&reader, off, endian)?);
            }
        }

        // 4. Resolve section names via .shstrtab
        let shstrndx = header.e_shstrndx as usize;
        if shstrndx < section_headers.len() {
            let shstrtab_sec = &section_headers[shstrndx];
            let offset = shstrtab_sec.sh_offset as usize;
            let size = shstrtab_sec.sh_size as usize;

            if let Ok(shstrtab_bytes) = reader.read_bytes(offset, size) {
                let strtab = Elf64Strtab::new(shstrtab_bytes);
                for sec in &mut section_headers {
                    if let Ok(name) = strtab.get_string(sec.sh_name) {
                        sec.name = name.to_string();
                    }
                }
            }
        }

        Ok(Self {
            reader,
            header,
            program_headers,
            section_headers,
        })
    }

    /// Finds a section by its name (e.g. ".symtab", ".text").
    pub fn find_section(&self, name: &str) -> Option<&Elf64Shdr> {
        self.section_headers.iter().find(|s| s.name == name)
    }

    /// Gets raw slice of section data.
    pub fn section_data(&self, section: &Elf64Shdr) -> Result<&'a [u8]> {
        if section.sh_type == SHT_NOBITS {
            return Ok(&[]);
        }
        let offset = section.sh_offset as usize;
        let size = section.sh_size as usize;
        self.reader.read_bytes(offset, size)
    }

    /// Parses symbols from a symbol table section (e.g. `.symtab` or `.dynsym`).
    pub fn parse_symbols(&self, sym_section: &Elf64Shdr) -> Result<Vec<Elf64Sym>> {
        if sym_section.sh_type != SHT_SYMTAB && sym_section.sh_type != SHT_DYNSYM {
            return Err(ParserError::CorruptedStructure("Section is not a symbol table"));
        }

        let entsize = if sym_section.sh_entsize > 0 {
            sym_section.sh_entsize as usize
        } else {
            24 // Standard Elf64_Sym size
        };

        let sym_count = (sym_section.sh_size as usize / entsize).min(MAX_ELF_SYMBOLS_LIMIT);
        let link_idx = sym_section.sh_link as usize;

        // Obtain the associated string table for symbol names
        let strtab = if link_idx < self.section_headers.len() {
            let strtab_sec = &self.section_headers[link_idx];
            let strtab_bytes = self.section_data(strtab_sec).unwrap_or(&[]);
            Some(Elf64Strtab::new(strtab_bytes))
        } else {
            None
        };

        let mut symbols = Vec::with_capacity(sym_count);
        let start_offset = sym_section.sh_offset as usize;

        for i in 0..sym_count {
            let entry_off = start_offset
                .checked_add(i.checked_mul(entsize).ok_or(ParserError::IntegerOverflow)?)
                .ok_or(ParserError::IntegerOverflow)?;

            let mut sym = Elf64Sym::parse(&self.reader, entry_off, self.header.endian)?;
            if let Some(ref st) = strtab {
                if let Ok(name) = st.get_string(sym.st_name) {
                    sym.name = name.to_string();
                }
            }
            symbols.push(sym);
        }

        Ok(symbols)
    }

    /// Returns the entry point virtual address.
    #[inline]
    pub fn entry_point(&self) -> u64 {
        self.header.e_entry
    }
}
