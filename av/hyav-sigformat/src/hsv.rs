//! HyAV Signature Vault (.hsv) Binary Container Format definitions and parsers.
//!
//! Defined in `docs/SIGFORMAT.md`. Fully zero-copy safe pure-Rust parser with zero unsafe.

use crate::error::{ParserError, Result};
use crate::reader::ByteReader;

pub const HSV_MAGIC: [u8; 4] = [0x48, 0x59, 0x41, 0x56]; // b"HYAV"
pub const HSV_MAJOR_VERSION: u16 = 1;
pub const HSV_HEADER_SIZE: usize = 64;

/// HSV Container Header (64 bytes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HsvHeader {
    pub magic: [u8; 4],
    pub format_major: u16,
    pub format_minor: u16,
    pub db_type: u16,
    pub flags: u16,
    pub build_timestamp: u64,
    pub total_signatures: u32,
    pub section_count: u16,
    pub reserved: [u8; 10],
    pub signature_offset: u64,
    pub string_table_offset: u64,
}

impl HsvHeader {
    pub fn parse(reader: &ByteReader<'_>) -> Result<Self> {
        let magic = reader.read_array::<4>(0)?;
        if magic != HSV_MAGIC {
            return Err(ParserError::InvalidMagic {
                expected: &HSV_MAGIC,
                found: magic.to_vec(),
            });
        }

        let format_major = reader.read_u16_le(4)?;
        let format_minor = reader.read_u16_le(6)?;
        let db_type = reader.read_u16_le(8)?;
        let flags = reader.read_u16_le(10)?;
        let build_timestamp = reader.read_u64_le(12)?;
        let total_signatures = reader.read_u32_le(20)?;
        let section_count = reader.read_u16_le(24)?;
        let reserved = reader.read_array::<10>(26)?;
        let signature_offset = reader.read_u64_le(36)?;
        let string_table_offset = reader.read_u64_le(44)?;

        Ok(Self {
            magic,
            format_major,
            format_minor,
            db_type,
            flags,
            build_timestamp,
            total_signatures,
            section_count,
            reserved,
            signature_offset,
            string_table_offset,
        })
    }
}

/// Exact hash signature record (Type 0x01).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SigExactHash {
    pub hash_algo: u8,
    pub severity: u8,
    pub threat_name_offset: u32,
    pub target_size_min: u64,
    pub target_size_max: u64,
    pub hash_value: [u8; 32],
}

impl SigExactHash {
    pub fn parse(reader: &ByteReader<'_>, offset: usize) -> Result<Self> {
        let hash_algo = reader.read_u8(offset)?;
        let severity = reader.read_u8(offset + 1)?;
        let threat_name_offset = reader.read_u32_le(offset + 2)?;
        let target_size_min = reader.read_u64_le(offset + 6)?;
        let target_size_max = reader.read_u64_le(offset + 14)?;
        let hash_value = reader.read_array::<32>(offset + 22)?;

        Ok(Self {
            hash_algo,
            severity,
            threat_name_offset,
            target_size_min,
            target_size_max,
            hash_value,
        })
    }
}

/// Byte sequence / wildcard signature record (Type 0x02).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SigBytePattern {
    pub target_format: u8,
    pub min_offset: u32,
    pub max_offset: u32,
    pub pattern_len: u16,
    pub threat_name_offset: u32,
    pub pattern: Vec<u8>,
    pub mask: Vec<u8>,
}

impl SigBytePattern {
    pub fn parse(reader: &ByteReader<'_>, offset: usize) -> Result<(Self, usize)> {
        let target_format = reader.read_u8(offset)?;
        let min_offset = reader.read_u32_le(offset + 1)?;
        let max_offset = reader.read_u32_le(offset + 5)?;
        let pattern_len = reader.read_u16_le(offset + 9)?;
        let threat_name_offset = reader.read_u32_le(offset + 11)?;

        let len = pattern_len as usize;
        let pattern_start = offset + 15;
        let mask_start = pattern_start
            .checked_add(len)
            .ok_or(ParserError::IntegerOverflow)?;

        let pattern = reader.read_bytes(pattern_start, len)?.to_vec();
        let mask = reader.read_bytes(mask_start, len)?.to_vec();

        let total_size = 15usize
            .checked_add(len.checked_mul(2).ok_or(ParserError::IntegerOverflow)?)
            .ok_or(ParserError::IntegerOverflow)?;

        Ok((
            Self {
                target_format,
                min_offset,
                max_offset,
                pattern_len,
                threat_name_offset,
                pattern,
                mask,
            },
            total_size,
        ))
    }
}
