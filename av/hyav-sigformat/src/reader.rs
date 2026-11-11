//! Bounds-checked binary reader utility for parsing PE/ELF structures without unsafe code.

use crate::error::{ParserError, Result};

/// A zero-copy, bounds-checked binary reader over a byte slice.
#[derive(Debug, Clone, Copy)]
pub struct ByteReader<'a> {
    data: &'a [u8],
}

impl<'a> ByteReader<'a> {
    /// Creates a new `ByteReader` wrapping the provided slice.
    #[inline]
    pub fn new(data: &'a [u8]) -> Self {
        Self { data }
    }

    /// Total length of the underlying slice.
    #[inline]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Whether the underlying slice is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Returns the full underlying byte slice.
    #[inline]
    pub fn data(&self) -> &'a [u8] {
        self.data
    }

    /// Reads a sub-slice at `offset` of length `len` with strict bounds checking.
    pub fn read_bytes(&self, offset: usize, len: usize) -> Result<&'a [u8]> {
        let end = offset.checked_add(len).ok_or(ParserError::IntegerOverflow)?;
        if end > self.data.len() {
            return Err(ParserError::UnexpectedEof {
                offset,
                requested: len,
                available: self.data.len().saturating_sub(offset),
            });
        }
        self.data
            .get(offset..end)
            .ok_or(ParserError::OffsetOutOfBounds {
                offset,
                len: self.data.len(),
            })
    }

    /// Reads a fixed-size byte array at `offset`.
    pub fn read_array<const N: usize>(&self, offset: usize) -> Result<[u8; N]> {
        let bytes = self.read_bytes(offset, N)?;
        let mut arr = [0u8; N];
        arr.copy_from_slice(bytes);
        Ok(arr)
    }

    /// Reads a single byte `u8` at `offset`.
    pub fn read_u8(&self, offset: usize) -> Result<u8> {
        let b = self.read_bytes(offset, 1)?;
        Ok(b[0])
    }

    /// Reads a `u16` in little-endian format at `offset`.
    pub fn read_u16_le(&self, offset: usize) -> Result<u16> {
        let b = self.read_array::<2>(offset)?;
        Ok(u16::from_le_bytes(b))
    }

    /// Reads a `u16` in big-endian format at `offset`.
    pub fn read_u16_be(&self, offset: usize) -> Result<u16> {
        let b = self.read_array::<2>(offset)?;
        Ok(u16::from_be_bytes(b))
    }

    /// Reads a `u32` in little-endian format at `offset`.
    pub fn read_u32_le(&self, offset: usize) -> Result<u32> {
        let b = self.read_array::<4>(offset)?;
        Ok(u32::from_le_bytes(b))
    }

    /// Reads a `u32` in big-endian format at `offset`.
    pub fn read_u32_be(&self, offset: usize) -> Result<u32> {
        let b = self.read_array::<4>(offset)?;
        Ok(u32::from_be_bytes(b))
    }

    /// Reads a `u64` in little-endian format at `offset`.
    pub fn read_u64_le(&self, offset: usize) -> Result<u64> {
        let b = self.read_array::<8>(offset)?;
        Ok(u64::from_le_bytes(b))
    }

    /// Reads a `u64` in big-endian format at `offset`.
    pub fn read_u64_be(&self, offset: usize) -> Result<u64> {
        let b = self.read_array::<8>(offset)?;
        Ok(u64::from_be_bytes(b))
    }

    /// Reads an `i32` in little-endian format at `offset`.
    pub fn read_i32_le(&self, offset: usize) -> Result<i32> {
        let b = self.read_array::<4>(offset)?;
        Ok(i32::from_le_bytes(b))
    }

    /// Reads a null-terminated ASCII/UTF-8 C-string from `offset` up to `max_len` bytes.
    pub fn read_cstring(&self, offset: usize, max_len: usize) -> Result<&'a str> {
        if offset >= self.data.len() {
            return Err(ParserError::OffsetOutOfBounds {
                offset,
                len: self.data.len(),
            });
        }
        let available = self.data.len() - offset;
        let scan_len = available.min(max_len);
        let slice = &self.data[offset..offset + scan_len];

        if let Some(pos) = slice.iter().position(|&b| b == 0) {
            let str_slice = &slice[..pos];
            core::str::from_utf8(str_slice).map_err(|_| ParserError::InvalidStringEncoding)
        } else {
            Err(ParserError::StringNotTerminated)
        }
    }

    /// Reads a fixed-length string, trimming trailing null characters.
    pub fn read_fixed_str<const N: usize>(&self, offset: usize) -> Result<String> {
        let bytes = self.read_array::<N>(offset)?;
        let end = bytes.iter().position(|&b| b == 0).unwrap_or(N);
        let s = String::from_utf8_lossy(&bytes[..end]).into_owned();
        Ok(s)
    }
}
