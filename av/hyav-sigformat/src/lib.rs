//! HyAV Signature Format & Binary Parsing Suite (`hyav-sigformat`)
//!
//! Provides binary codec (.sig / .db / .hsv) for HybridOS Antivirus:
//! - Magic: `HYAVSIG\0` (8 bytes)
//! - Header: 64-byte aligned metadata
//! - Bloom Filter section for ultra-low latency negative lookups
//! - Records: HASH_EXACT, PATTERN, PATH_RULE, PE_HEADER, ELF_HEADER
//! - Trailer: SHA-256 integrity digest + Ed25519 signature + `HYAVEOF\0`
//!
//! Provides pure-Rust binary parsers:
//! - PE32+ (DOS, COFF, OptionalHeader64, SectionHeaders, Imports, Exports)
//! - ELF64 (Ehdr, Phdr, Shdr, Symtab, Strtab)
//! - HyAV Signature Vault (.hsv container format)
//!
//! Pure Rust implementation with zero external dependencies and `#![forbid(unsafe_code)]`.

#![forbid(unsafe_code)]

pub mod bloom;
pub mod codec;
pub mod crypto;
pub mod elf;
pub mod error;
pub mod header;
pub mod hsv;
pub mod pe;
pub mod reader;
pub mod record;
pub mod trailer;

pub use bloom::BloomFilter;
pub use codec::{CodecError, DatabaseBuilder, SignatureDatabase};
pub use elf::{
    Elf64File, Elf64Header, Elf64Phdr, Elf64Shdr, Elf64Strtab, Elf64Sym, Endian, ELFCLASS64,
    ELFDATA2LSB, ELFDATA2MSB, ELF_MAGIC,
};
pub use error::{ParserError, Result as ParserResult};
pub use header::{DbType, Header, HeaderFlags, SIG_MAGIC};
pub use hsv::{HsvHeader, SigBytePattern, SigExactHash, HSV_MAGIC};
pub use pe::{
    CoffHeader, DataDirectoryIndex, DosHeader, ExportDirectory, ExportedSymbol, ImageDataDirectory,
    ImportDescriptor, ImportedSymbol, OptionalHeader64, Pe64File, SectionHeader, DOS_MAGIC,
    IMAGE_ORDINAL_FLAG64, PE32_PLUS_MAGIC, PE_SIGNATURE,
};
pub use reader::ByteReader;
pub use record::{
    Action, ElfHeaderRecord, HashAlgo, HashExactRecord, PathRuleRecord, PatternRecord, PeHeaderRecord,
    RecordCodecError, RecordPayload, RecordType, Severity, SignatureRecord, SyntheticElfInfo,
    SyntheticPeInfo, SyntheticPeSection, TargetFormat,
};
pub use trailer::{Trailer, TrailerError, TRAILER_MAGIC};
