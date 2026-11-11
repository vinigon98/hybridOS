//! FileId definition for HyAV.

use std::fmt;

/// Unique 128-bit identifier for filesystem objects across HybridOS.
///
/// Composed of:
/// - `volume_guid`: 64-bit partition or volume identifier.
/// - `inode`: 64-bit filesystem index node or file record number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct FileId {
    /// 64-bit partition or volume identifier.
    pub volume_guid: u64,
    /// 64-bit filesystem index node or file record number.
    pub inode: u64,
}

impl FileId {
    /// Creates a new `FileId` from explicit volume GUID and inode.
    #[inline]
    pub const fn new(volume_guid: u64, inode: u64) -> Self {
        Self { volume_guid, inode }
    }

    /// Constructs a `FileId` from a single 128-bit integer.
    /// The upper 64 bits represent the volume GUID, and lower 64 bits represent the inode.
    #[inline]
    pub const fn from_u128(val: u128) -> Self {
        Self {
            volume_guid: (val >> 64) as u64,
            inode: val as u64,
        }
    }

    /// Converts this `FileId` into a 128-bit integer representation.
    #[inline]
    pub const fn as_u128(&self) -> u128 {
        ((self.volume_guid as u128) << 64) | (self.inode as u128)
    }

    /// Constructs a `FileId` from 16 raw bytes in little-endian order.
    #[inline]
    pub fn from_le_bytes(bytes: [u8; 16]) -> Self {
        let volume_guid = u64::from_le_bytes(bytes[0..8].try_into().unwrap());
        let inode = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
        Self { volume_guid, inode }
    }

    /// Returns the 16-byte little-endian byte array representation.
    #[inline]
    pub fn to_le_bytes(&self) -> [u8; 16] {
        let mut buf = [0u8; 16];
        buf[0..8].copy_from_slice(&self.volume_guid.to_le_bytes());
        buf[8..16].copy_from_slice(&self.inode.to_le_bytes());
        buf
    }
}

impl fmt::Display for FileId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:016x}:{:016x}", self.volume_guid, self.inode)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_id_conversions() {
        let fid = FileId::new(0x1122334455667788, 0x99aabbccddeeff00);
        let u128_val = fid.as_u128();
        assert_eq!(FileId::from_u128(u128_val), fid);

        let bytes = fid.to_le_bytes();
        assert_eq!(FileId::from_le_bytes(bytes), fid);
    }
}
