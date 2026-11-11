//! PE32+ (64-bit Portable Executable) Parser.
//!
//! Fully safe pure-Rust parser with zero unsafe code and exhaustive bounds checking.
//! Parses DOS header, COFF header, OptionalHeader64, SectionHeaders, Imports, and Exports.

use crate::error::{ParserError, Result};
use crate::reader::ByteReader;

/// DOS header magic `MZ`.
pub const DOS_MAGIC: u16 = 0x5A4D;

/// PE signature `PE\0\0`.
pub const PE_SIGNATURE: u32 = 0x00004550;

/// Optional Header Magic for PE32+ (64-bit).
pub const PE32_PLUS_MAGIC: u16 = 0x020B;

/// Bit flag indicating 64-bit import by ordinal (bit 63).
pub const IMAGE_ORDINAL_FLAG64: u64 = 0x8000_0000_0000_0000;

/// Maximum number of sections permitted for safety.
pub const MAX_SECTIONS_LIMIT: usize = 256;

/// Maximum string length permitted during parsing.
pub const MAX_STRING_LEN: usize = 1024;

/// Standard Data Directory indices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum DataDirectoryIndex {
    Export = 0,
    Import = 1,
    Resource = 2,
    Exception = 3,
    Certificate = 4,
    BaseReloc = 5,
    Debug = 6,
    Architecture = 7,
    GlobalPtr = 8,
    Tls = 9,
    LoadConfig = 10,
    BoundImport = 11,
    Iat = 12,
    DelayImport = 13,
    ClrRuntime = 14,
    Reserved = 15,
}

/// DOS Header (IMAGE_DOS_HEADER).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DosHeader {
    pub e_magic: u16,
    pub e_lfanew: u32,
}

impl DosHeader {
    pub fn parse(reader: &ByteReader<'_>) -> Result<Self> {
        let magic = reader.read_u16_le(0)?;
        if magic != DOS_MAGIC {
            return Err(ParserError::InvalidDosHeader("Missing 'MZ' magic"));
        }
        let e_lfanew = reader.read_u32_le(0x3C)?;
        if e_lfanew < 64 {
            return Err(ParserError::InvalidDosHeader("e_lfanew points inside DOS header"));
        }
        Ok(Self { e_magic: magic, e_lfanew })
    }
}

/// COFF File Header (IMAGE_FILE_HEADER).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoffHeader {
    pub machine: u16,
    pub number_of_sections: u16,
    pub time_date_stamp: u32,
    pub pointer_to_symbol_table: u32,
    pub number_of_symbols: u32,
    pub size_of_optional_header: u16,
    pub characteristics: u16,
}

impl CoffHeader {
    pub fn parse(reader: &ByteReader<'_>, offset: usize) -> Result<Self> {
        Ok(Self {
            machine: reader.read_u16_le(offset)?,
            number_of_sections: reader.read_u16_le(offset + 2)?,
            time_date_stamp: reader.read_u32_le(offset + 4)?,
            pointer_to_symbol_table: reader.read_u32_le(offset + 8)?,
            number_of_symbols: reader.read_u32_le(offset + 12)?,
            size_of_optional_header: reader.read_u16_le(offset + 16)?,
            characteristics: reader.read_u16_le(offset + 18)?,
        })
    }
}

/// Data Directory entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ImageDataDirectory {
    pub virtual_address: u32,
    pub size: u32,
}

/// Optional Header for PE32+ (IMAGE_OPTIONAL_HEADER64).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptionalHeader64 {
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

impl OptionalHeader64 {
    pub fn parse(reader: &ByteReader<'_>, offset: usize) -> Result<Self> {
        let magic = reader.read_u16_le(offset)?;
        if magic != PE32_PLUS_MAGIC {
            return Err(ParserError::InvalidOptionalHeaderMagic(magic));
        }

        let major_linker_version = reader.read_u8(offset + 2)?;
        let minor_linker_version = reader.read_u8(offset + 3)?;
        let size_of_code = reader.read_u32_le(offset + 4)?;
        let size_of_initialized_data = reader.read_u32_le(offset + 8)?;
        let size_of_uninitialized_data = reader.read_u32_le(offset + 12)?;
        let address_of_entry_point = reader.read_u32_le(offset + 16)?;
        let base_of_code = reader.read_u32_le(offset + 20)?;
        let image_base = reader.read_u64_le(offset + 24)?;
        let section_alignment = reader.read_u32_le(offset + 32)?;
        let file_alignment = reader.read_u32_le(offset + 36)?;
        let major_operating_system_version = reader.read_u16_le(offset + 40)?;
        let minor_operating_system_version = reader.read_u16_le(offset + 42)?;
        let major_image_version = reader.read_u16_le(offset + 44)?;
        let minor_image_version = reader.read_u16_le(offset + 46)?;
        let major_subsystem_version = reader.read_u16_le(offset + 48)?;
        let minor_subsystem_version = reader.read_u16_le(offset + 50)?;
        let win32_version_value = reader.read_u32_le(offset + 52)?;
        let size_of_image = reader.read_u32_le(offset + 56)?;
        let size_of_headers = reader.read_u32_le(offset + 60)?;
        let check_sum = reader.read_u32_le(offset + 64)?;
        let subsystem = reader.read_u16_le(offset + 68)?;
        let dll_characteristics = reader.read_u16_le(offset + 70)?;
        let size_of_stack_reserve = reader.read_u64_le(offset + 72)?;
        let size_of_stack_commit = reader.read_u64_le(offset + 80)?;
        let size_of_heap_reserve = reader.read_u64_le(offset + 88)?;
        let size_of_heap_commit = reader.read_u64_le(offset + 96)?;
        let loader_flags = reader.read_u32_le(offset + 104)?;
        let number_of_rva_and_sizes = reader.read_u32_le(offset + 108)?;

        let mut data_directories = [ImageDataDirectory::default(); 16];
        let num_dirs = (number_of_rva_and_sizes as usize).min(16);
        let dd_start = offset + 112;

        for i in 0..num_dirs {
            let entry_offset = dd_start
                .checked_add(i.checked_mul(8).ok_or(ParserError::IntegerOverflow)?)
                .ok_or(ParserError::IntegerOverflow)?;
            data_directories[i] = ImageDataDirectory {
                virtual_address: reader.read_u32_le(entry_offset)?,
                size: reader.read_u32_le(entry_offset + 4)?,
            };
        }

        Ok(Self {
            magic,
            major_linker_version,
            minor_linker_version,
            size_of_code,
            size_of_initialized_data,
            size_of_uninitialized_data,
            address_of_entry_point,
            base_of_code,
            image_base,
            section_alignment,
            file_alignment,
            major_operating_system_version,
            minor_operating_system_version,
            major_image_version,
            minor_image_version,
            major_subsystem_version,
            minor_subsystem_version,
            win32_version_value,
            size_of_image,
            size_of_headers,
            check_sum,
            subsystem,
            dll_characteristics,
            size_of_stack_reserve,
            size_of_stack_commit,
            size_of_heap_reserve,
            size_of_heap_commit,
            loader_flags,
            number_of_rva_and_sizes,
            data_directories,
        })
    }
}

/// PE Section Header (IMAGE_SECTION_HEADER).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionHeader {
    pub raw_name: [u8; 8],
    pub name: String,
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

impl SectionHeader {
    pub fn parse(reader: &ByteReader<'_>, offset: usize) -> Result<Self> {
        let raw_name = reader.read_array::<8>(offset)?;
        let name_end = raw_name.iter().position(|&b| b == 0).unwrap_or(8);
        let name = String::from_utf8_lossy(&raw_name[..name_end]).into_owned();

        Ok(Self {
            raw_name,
            name,
            virtual_size: reader.read_u32_le(offset + 8)?,
            virtual_address: reader.read_u32_le(offset + 12)?,
            size_of_raw_data: reader.read_u32_le(offset + 16)?,
            pointer_to_raw_data: reader.read_u32_le(offset + 20)?,
            pointer_to_relocations: reader.read_u32_le(offset + 24)?,
            pointer_to_linenumbers: reader.read_u32_le(offset + 28)?,
            number_of_relocations: reader.read_u16_le(offset + 32)?,
            number_of_linenumbers: reader.read_u16_le(offset + 34)?,
            characteristics: reader.read_u32_le(offset + 36)?,
        })
    }

    /// Checks if a given RVA falls within this section's virtual memory range.
    #[inline]
    pub fn contains_rva(&self, rva: u32) -> bool {
        let span = self.virtual_size.max(self.size_of_raw_data);
        if let Some(end) = self.virtual_address.checked_add(span) {
            rva >= self.virtual_address && rva < end
        } else {
            false
        }
    }

    /// Translates an RVA within this section to a file offset.
    pub fn rva_to_file_offset(&self, rva: u32) -> Option<usize> {
        if !self.contains_rva(rva) {
            return None;
        }
        let delta = rva.checked_sub(self.virtual_address)?;
        if delta < self.size_of_raw_data {
            let offset = (self.pointer_to_raw_data as usize).checked_add(delta as usize)?;
            Some(offset)
        } else {
            None
        }
    }
}

/// An imported symbol (either by name or by ordinal).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedSymbol {
    pub name: Option<String>,
    pub ordinal: Option<u16>,
    pub hint: Option<u16>,
}

/// An imported DLL module and its referenced symbols.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportDescriptor {
    pub dll_name: String,
    pub original_first_thunk: u32,
    pub time_date_stamp: u32,
    pub forwarder_chain: u32,
    pub name_rva: u32,
    pub first_thunk: u32,
    pub symbols: Vec<ImportedSymbol>,
}

/// An exported symbol with its RVA and name/ordinal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportedSymbol {
    pub name: Option<String>,
    pub ordinal: u16,
    pub rva: u32,
    pub forwarder: Option<String>,
}

/// Export Directory and exported functions table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportDirectory {
    pub characteristics: u32,
    pub time_date_stamp: u32,
    pub major_version: u16,
    pub minor_version: u16,
    pub name: String,
    pub ordinal_base: u32,
    pub number_of_functions: u32,
    pub number_of_names: u32,
    pub address_of_functions_rva: u32,
    pub address_of_names_rva: u32,
    pub address_of_name_ordinals_rva: u32,
    pub exports: Vec<ExportedSymbol>,
}

/// Parsed PE32+ (64-bit Windows executable) structure.
#[derive(Debug, Clone)]
pub struct Pe64File<'a> {
    reader: ByteReader<'a>,
    pub dos_header: DosHeader,
    pub coff_header: CoffHeader,
    pub optional_header: OptionalHeader64,
    pub sections: Vec<SectionHeader>,
}

impl<'a> Pe64File<'a> {
    /// Parses a complete PE32+ binary from raw bytes.
    pub fn parse(data: &'a [u8]) -> Result<Self> {
        let reader = ByteReader::new(data);

        // 1. DOS Header
        let dos_header = DosHeader::parse(&reader)?;

        // 2. PE Signature
        let pe_sig_offset = dos_header.e_lfanew as usize;
        let sig = reader.read_u32_le(pe_sig_offset)?;
        if sig != PE_SIGNATURE {
            return Err(ParserError::InvalidPeSignature(pe_sig_offset));
        }

        // 3. COFF File Header
        let coff_offset = pe_sig_offset
            .checked_add(4)
            .ok_or(ParserError::IntegerOverflow)?;
        let coff_header = CoffHeader::parse(&reader, coff_offset)?;

        // 4. Optional Header (PE32+)
        let opt_offset = coff_offset
            .checked_add(20)
            .ok_or(ParserError::IntegerOverflow)?;
        let optional_header = OptionalHeader64::parse(&reader, opt_offset)?;

        // 5. Section Headers
        let sections_offset = opt_offset
            .checked_add(coff_header.size_of_optional_header as usize)
            .ok_or(ParserError::IntegerOverflow)?;

        let num_sections = coff_header.number_of_sections as usize;
        if num_sections > MAX_SECTIONS_LIMIT {
            return Err(ParserError::ResourceExhausted("Excessive PE section count"));
        }

        let mut sections = Vec::with_capacity(num_sections);
        for i in 0..num_sections {
            let sec_offset = sections_offset
                .checked_add(i.checked_mul(40).ok_or(ParserError::IntegerOverflow)?)
                .ok_or(ParserError::IntegerOverflow)?;
            sections.push(SectionHeader::parse(&reader, sec_offset)?);
        }

        Ok(Self {
            reader,
            dos_header,
            coff_header,
            optional_header,
            sections,
        })
    }

    /// Resolves an RVA to a file offset using the section headers.
    pub fn rva_to_file_offset(&self, rva: u32) -> Result<usize> {
        for sec in &self.sections {
            if let Some(off) = sec.rva_to_file_offset(rva) {
                if off <= self.reader.len() {
                    return Ok(off);
                }
            }
        }
        Err(ParserError::UnmappedRva(rva))
    }

    /// Retrieves a section by its name (e.g., ".text").
    pub fn find_section(&self, name: &str) -> Option<&SectionHeader> {
        self.sections.iter().find(|s| s.name == name)
    }

    /// Gets raw slice of section data.
    pub fn section_data(&self, section: &SectionHeader) -> Result<&'a [u8]> {
        let offset = section.pointer_to_raw_data as usize;
        let size = section.size_of_raw_data as usize;
        self.reader.read_bytes(offset, size)
    }

    /// Parses all import descriptors and their imported symbols.
    pub fn parse_imports(&self) -> Result<Vec<ImportDescriptor>> {
        let import_dir = self.optional_header.data_directories[DataDirectoryIndex::Import as usize];
        if import_dir.virtual_address == 0 || import_dir.size == 0 {
            return Ok(Vec::new());
        }

        let mut descriptors = Vec::new();
        let dir_offset = self.rva_to_file_offset(import_dir.virtual_address)?;
        let mut curr_offset = dir_offset;

        const MAX_DESCRIPTORS: usize = 2048;
        for _ in 0..MAX_DESCRIPTORS {
            let original_first_thunk = self.reader.read_u32_le(curr_offset)?;
            let time_date_stamp = self.reader.read_u32_le(curr_offset + 4)?;
            let forwarder_chain = self.reader.read_u32_le(curr_offset + 8)?;
            let name_rva = self.reader.read_u32_le(curr_offset + 12)?;
            let first_thunk = self.reader.read_u32_le(curr_offset + 16)?;

            // Null terminator descriptor indicates end of array
            if original_first_thunk == 0 && name_rva == 0 && first_thunk == 0 {
                break;
            }

            let name_offset = self.rva_to_file_offset(name_rva)?;
            let dll_name = self.reader.read_cstring(name_offset, MAX_STRING_LEN)?.to_string();

            // Thunk parsing (ILT / IAT)
            let thunk_rva = if original_first_thunk != 0 {
                original_first_thunk
            } else {
                first_thunk
            };

            let mut symbols = Vec::new();
            if thunk_rva != 0 {
                if let Ok(mut thunk_offset) = self.rva_to_file_offset(thunk_rva) {
                    const MAX_THUNKS: usize = 8192;
                    for _ in 0..MAX_THUNKS {
                        let entry = self.reader.read_u64_le(thunk_offset)?;
                        if entry == 0 {
                            break;
                        }

                        if entry & IMAGE_ORDINAL_FLAG64 != 0 {
                            symbols.push(ImportedSymbol {
                                name: None,
                                ordinal: Some((entry & 0xFFFF) as u16),
                                hint: None,
                            });
                        } else {
                            let hint_name_rva = (entry & 0x7FFF_FFFF) as u32;
                            if let Ok(hint_offset) = self.rva_to_file_offset(hint_name_rva) {
                                let hint = self.reader.read_u16_le(hint_offset).ok();
                                let name = self
                                    .reader
                                    .read_cstring(hint_offset + 2, MAX_STRING_LEN)
                                    .ok()
                                    .map(|s| s.to_string());
                                symbols.push(ImportedSymbol { name, ordinal: None, hint });
                            }
                        }

                        thunk_offset = thunk_offset
                            .checked_add(8)
                            .ok_or(ParserError::IntegerOverflow)?;
                    }
                }
            }

            descriptors.push(ImportDescriptor {
                dll_name,
                original_first_thunk,
                time_date_stamp,
                forwarder_chain,
                name_rva,
                first_thunk,
                symbols,
            });

            curr_offset = curr_offset
                .checked_add(20)
                .ok_or(ParserError::IntegerOverflow)?;
        }

        Ok(descriptors)
    }

    /// Parses export directory and exported functions.
    pub fn parse_exports(&self) -> Result<Option<ExportDirectory>> {
        let export_dir = self.optional_header.data_directories[DataDirectoryIndex::Export as usize];
        if export_dir.virtual_address == 0 || export_dir.size == 0 {
            return Ok(None);
        }

        let dir_offset = self.rva_to_file_offset(export_dir.virtual_address)?;
        let characteristics = self.reader.read_u32_le(dir_offset)?;
        let time_date_stamp = self.reader.read_u32_le(dir_offset + 4)?;
        let major_version = self.reader.read_u16_le(dir_offset + 8)?;
        let minor_version = self.reader.read_u16_le(dir_offset + 10)?;
        let name_rva = self.reader.read_u32_le(dir_offset + 12)?;
        let ordinal_base = self.reader.read_u32_le(dir_offset + 16)?;
        let number_of_functions = self.reader.read_u32_le(dir_offset + 20)?;
        let number_of_names = self.reader.read_u32_le(dir_offset + 24)?;
        let address_of_functions_rva = self.reader.read_u32_le(dir_offset + 28)?;
        let address_of_names_rva = self.reader.read_u32_le(dir_offset + 32)?;
        let address_of_name_ordinals_rva = self.reader.read_u32_le(dir_offset + 36)?;

        let name = if name_rva != 0 {
            let name_offset = self.rva_to_file_offset(name_rva)?;
            self.reader.read_cstring(name_offset, MAX_STRING_LEN)?.to_string()
        } else {
            String::new()
        };

        let export_va_start = export_dir.virtual_address;
        let export_va_end = export_va_start
            .checked_add(export_dir.size)
            .ok_or(ParserError::IntegerOverflow)?;

        // Function addresses (EAT)
        let functions_offset = self.rva_to_file_offset(address_of_functions_rva)?;
        let func_count = (number_of_functions as usize).min(65536);
        let mut func_rvas = Vec::with_capacity(func_count);
        for i in 0..func_count {
            let off = functions_offset
                .checked_add(i.checked_mul(4).ok_or(ParserError::IntegerOverflow)?)
                .ok_or(ParserError::IntegerOverflow)?;
            func_rvas.push(self.reader.read_u32_le(off)?);
        }

        // Names and Name Ordinals
        let name_count = (number_of_names as usize).min(func_count);
        let mut name_map = std::collections::HashMap::new();

        if address_of_names_rva != 0 && address_of_name_ordinals_rva != 0 {
            let names_offset = self.rva_to_file_offset(address_of_names_rva)?;
            let ordinals_offset = self.rva_to_file_offset(address_of_name_ordinals_rva)?;

            for i in 0..name_count {
                let name_entry_off = names_offset
                    .checked_add(i.checked_mul(4).ok_or(ParserError::IntegerOverflow)?)
                    .ok_or(ParserError::IntegerOverflow)?;
                let ord_entry_off = ordinals_offset
                    .checked_add(i.checked_mul(2).ok_or(ParserError::IntegerOverflow)?)
                    .ok_or(ParserError::IntegerOverflow)?;

                let fn_name_rva = self.reader.read_u32_le(name_entry_off)?;
                let ord_idx = self.reader.read_u16_le(ord_entry_off)?;

                if fn_name_rva != 0 {
                    if let Ok(fn_name_off) = self.rva_to_file_offset(fn_name_rva) {
                        if let Ok(fn_name) = self.reader.read_cstring(fn_name_off, MAX_STRING_LEN) {
                            name_map.insert(ord_idx as usize, fn_name.to_string());
                        }
                    }
                }
            }
        }

        let mut exports = Vec::with_capacity(func_count);
        for (idx, &rva) in func_rvas.iter().enumerate() {
            if rva == 0 {
                continue;
            }
            let ordinal = (ordinal_base as usize + idx) as u16;
            let name = name_map.remove(&idx);

            // Check if RVA falls into export directory range (forwarder export)
            let forwarder = if rva >= export_va_start && rva < export_va_end {
                if let Ok(fwd_off) = self.rva_to_file_offset(rva) {
                    self.reader.read_cstring(fwd_off, MAX_STRING_LEN).ok().map(|s| s.to_string())
                } else {
                    None
                }
            } else {
                None
            };

            exports.push(ExportedSymbol {
                name,
                ordinal,
                rva,
                forwarder,
            });
        }

        Ok(Some(ExportDirectory {
            characteristics,
            time_date_stamp,
            major_version,
            minor_version,
            name,
            ordinal_base,
            number_of_functions,
            number_of_names,
            address_of_functions_rva,
            address_of_names_rva,
            address_of_name_ordinals_rva,
            exports,
        }))
    }

    /// Calculates Shannon entropy of a slice (useful for packing detection).
    pub fn calculate_entropy(data: &[u8]) -> f64 {
        if data.is_empty() {
            return 0.0;
        }
        let mut counts = [0usize; 256];
        for &b in data {
            counts[b as usize] += 1;
        }
        let len_f = data.len() as f64;
        let mut entropy = 0.0;
        for &c in &counts {
            if c > 0 {
                let p = c as f64 / len_f;
                entropy -= p * p.log2();
            }
        }
        entropy
    }
}
