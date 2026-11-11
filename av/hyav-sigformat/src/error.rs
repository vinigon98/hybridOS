//! Error definitions for the HyAV binary format and parser suite.

use core::fmt;

/// Result alias for operations in `hyav-sigformat`.
pub type Result<T> = core::result::Result<T, ParserError>;

/// Detailed parser error enumeration with bounds check diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParserError {
    /// Attempted to read past buffer boundary.
    UnexpectedEof {
        offset: usize,
        requested: usize,
        available: usize,
    },
    /// Magic signature mismatch.
    InvalidMagic {
        expected: &'static [u8],
        found: Vec<u8>,
    },
    /// DOS header is invalid or corrupt.
    InvalidDosHeader(&'static str),
    /// PE signature is missing or corrupt at the specified offset.
    InvalidPeSignature(usize),
    /// Optional header magic does not match PE32+ (expected 0x020B).
    InvalidOptionalHeaderMagic(u16),
    /// ELF identification is invalid (magic, class, endianness, version).
    InvalidElfIdent(&'static str),
    /// Expected 64-bit ELF (class 2), found 32-bit (class 1) or other.
    UnsupportedElfClass(u8),
    /// Unsupported endianness in ELF identification.
    UnsupportedElfEndian(u8),
    /// Specified offset exceeds file boundary.
    OffsetOutOfBounds {
        offset: usize,
        len: usize,
    },
    /// Integer overflow occurred during offset or size calculations.
    IntegerOverflow,
    /// String is not null-terminated within allowed bounds.
    StringNotTerminated,
    /// String contains invalid UTF-8 bytes where UTF-8 was expected.
    InvalidStringEncoding,
    /// Relative Virtual Address (RVA) does not map to any valid section.
    UnmappedRva(u32),
    /// Section count or header size exceeds safety threshold.
    ResourceExhausted(&'static str),
    /// Corrupt data structure encountered during parsing.
    CorruptedStructure(&'static str),
}

impl fmt::Display for ParserError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedEof { offset, requested, available } => {
                write!(f, "Unexpected EOF: at offset {offset}, requested {requested} bytes, available {available}")
            }
            Self::InvalidMagic { expected, found } => {
                write!(f, "Invalid magic: expected {expected:?}, found {found:?}")
            }
            Self::InvalidDosHeader(msg) => write!(f, "Invalid DOS header: {msg}"),
            Self::InvalidPeSignature(off) => write!(f, "Invalid PE signature at offset {off}"),
            Self::InvalidOptionalHeaderMagic(m) => {
                write!(f, "Invalid OptionalHeader magic: 0x{m:04X} (expected 0x020B PE32+)")
            }
            Self::InvalidElfIdent(msg) => write!(f, "Invalid ELF header ident: {msg}"),
            Self::UnsupportedElfClass(c) => write!(f, "Unsupported ELF class: {c} (expected 2 for 64-bit)"),
            Self::UnsupportedElfEndian(e) => write!(f, "Unsupported ELF endian: {e} (1=LE, 2=BE)"),
            Self::OffsetOutOfBounds { offset, len } => {
                write!(f, "Offset out of bounds: offset {offset} in buffer of {len} bytes")
            }
            Self::IntegerOverflow => write!(f, "Arithmetic integer overflow during parsing"),
            Self::StringNotTerminated => write!(f, "String is not null-terminated within bounds"),
            Self::InvalidStringEncoding => write!(f, "Invalid string encoding (non-UTF8/corrupt)"),
            Self::UnmappedRva(rva) => write!(f, "RVA 0x{rva:08X} does not map to any section"),
            Self::ResourceExhausted(msg) => write!(f, "Safety limit exceeded: {msg}"),
            Self::CorruptedStructure(msg) => write!(f, "Corrupted structure: {msg}"),
        }
    }
}

impl std::error::Error for ParserError {}
