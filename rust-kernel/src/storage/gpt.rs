//! GUID Partition Table (GPT) & Protective MBR Parser for HybridOS.
//!
//! Provides full support for:
//! - Protective MBR at LBA 0 (Partition type 0xEE)
//! - GPT Header at LBA 1 (Signature "EFI PART", CRC32 verification, LBA ranges)
//! - Partition Entry parsing (Windows Basic Data, Linux Filesystem, EFI System, etc.)
//! - UTF-16LE partition name decoding
//! - Image creation and serialization for unit testing and disk provisioning.

use std::fmt;
use super::{BlockDevice, StorageError};

/// Standard IEEE 802.3 CRC32 calculation.
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            if (crc & 1) != 0 {
                crc = (crc >> 1) ^ 0xEDB8_8320;
            } else {
                crc >>= 1;
            }
        }
    }
    !crc
}

/// 128-bit Globally Unique Identifier (GUID) in UEFI mixed-endian format.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Guid(pub [u8; 16]);

impl Guid {
    /// Linux filesystem data GUID: `0FC63DAF-8483-4772-8E79-3D69D8477DE4`
    pub const LINUX_FILESYSTEM: Guid = Guid([
        0xAF, 0x3D, 0xC6, 0x0F, 0x83, 0x84, 0x72, 0x47,
        0x8E, 0x79, 0x3D, 0x69, 0xD8, 0x47, 0x7D, 0xE4,
    ]);

    /// Windows basic data partition GUID: `EBD0A0A2-B9E5-4433-87C0-68B6B72699C7`
    pub const WINDOWS_BASIC_DATA: Guid = Guid([
        0xA2, 0xA0, 0xD0, 0xEB, 0xE5, 0xB9, 0x33, 0x44,
        0x87, 0xC0, 0x68, 0xB6, 0xB7, 0x26, 0x99, 0xC7,
    ]);

    /// EFI System Partition GUID: `C12A7328-F81F-11D2-BA4B-00A0C93EC93B`
    pub const EFI_SYSTEM: Guid = Guid([
        0x28, 0x73, 0x2A, 0xC1, 0x1F, 0xF8, 0xD2, 0x11,
        0xBA, 0x4B, 0x00, 0xA0, 0xC9, 0x3E, 0xC9, 0x3B,
    ]);

    /// Linux swap partition GUID: `0657FD6D-A4AB-43C4-84E5-0933C84B4F4F`
    pub const LINUX_SWAP: Guid = Guid([
        0x6D, 0xFD, 0x57, 0x06, 0xAB, 0xA4, 0xC4, 0x43,
        0x84, 0xE5, 0x09, 0x33, 0xC8, 0x4B, 0x4F, 0x4F,
    ]);

    /// Microsoft Reserved Partition GUID: `E3C9E316-0B5C-4DB8-817D-F92DF00215AE`
    pub const MICROSOFT_RESERVED: Guid = Guid([
        0x16, 0xE3, 0xC9, 0xE3, 0x5C, 0x0B, 0xB8, 0x4D,
        0x81, 0x7D, 0xF9, 0x2D, 0xF0, 0x02, 0x15, 0xAE,
    ]);

    /// Linux x86-64 root partition GUID: `4F68BCE3-E8CD-4DB1-96E7-FBCAF984B709`
    pub const LINUX_ROOT_X86_64: Guid = Guid([
        0xE3, 0xBC, 0x68, 0x4F, 0xCD, 0xE8, 0xB1, 0x4D,
        0x96, 0xE7, 0xFB, 0xCA, 0xF9, 0x84, 0xB7, 0x09,
    ]);

    /// Unused / empty partition entry GUID: `00000000-0000-0000-0000-000000000000`
    pub const UNUSED: Guid = Guid([0; 16]);

    /// Construct GUID from raw 16 bytes.
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Guid(bytes)
    }

    /// Access raw bytes of the GUID.
    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }

    /// Check if GUID is all zeros.
    pub fn is_zero(&self) -> bool {
        self.0.iter().all(|&b| b == 0)
    }

    /// Parse a GUID string in standard format: `XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX`.
    pub fn parse(s: &str) -> Result<Self, StorageError> {
        let clean = s.trim();
        let parts: Vec<&str> = clean.split('-').collect();
        if parts.len() != 5 {
            return Err(StorageError::InvalidData("Invalid GUID format: expected 5 hyphen-separated groups"));
        }

        if parts[0].len() != 8 || parts[1].len() != 4 || parts[2].len() != 4 || parts[3].len() != 4 || parts[4].len() != 12 {
            return Err(StorageError::InvalidData("Invalid GUID group length"));
        }

        let d1 = u32::from_str_radix(parts[0], 16)
            .map_err(|_| StorageError::InvalidData("Invalid hex in GUID part 1"))?;
        let d2 = u16::from_str_radix(parts[1], 16)
            .map_err(|_| StorageError::InvalidData("Invalid hex in GUID part 2"))?;
        let d3 = u16::from_str_radix(parts[2], 16)
            .map_err(|_| StorageError::InvalidData("Invalid hex in GUID part 3"))?;

        let p4_bytes = hex_to_bytes(parts[3])?;
        let p5_bytes = hex_to_bytes(parts[4])?;

        let mut bytes = [0u8; 16];
        // UEFI format: first three components are little-endian
        bytes[0..4].copy_from_slice(&d1.to_le_bytes());
        bytes[4..6].copy_from_slice(&d2.to_le_bytes());
        bytes[6..8].copy_from_slice(&d3.to_le_bytes());
        bytes[8..10].copy_from_slice(&p4_bytes);
        bytes[10..16].copy_from_slice(&p5_bytes);

        Ok(Guid(bytes))
    }
}

fn hex_to_bytes(hex: &str) -> Result<Vec<u8>, StorageError> {
    if hex.len() % 2 != 0 {
        return Err(StorageError::InvalidData("Hex string must have even length"));
    }
    let mut bytes = Vec::with_capacity(hex.len() / 2);
    for i in (0..hex.len()).step_by(2) {
        let b = u8::from_str_radix(&hex[i..i + 2], 16)
            .map_err(|_| StorageError::InvalidData("Invalid hex character in GUID"))?;
        bytes.push(b);
    }
    Ok(bytes)
}

impl fmt::Display for Guid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let d1 = u32::from_le_bytes([self.0[0], self.0[1], self.0[2], self.0[3]]);
        let d2 = u16::from_le_bytes([self.0[4], self.0[5]]);
        let d3 = u16::from_le_bytes([self.0[6], self.0[7]]);
        write!(
            f,
            "{:08X}-{:04X}-{:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}",
            d1, d2, d3,
            self.0[8], self.0[9],
            self.0[10], self.0[11], self.0[12], self.0[13], self.0[14], self.0[15]
        )
    }
}

impl fmt::Debug for Guid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Guid({})", self)
    }
}

/// Protective MBR partition entry (16 bytes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MbrPartitionRecord {
    pub boot_indicator: u8,
    pub starting_chs: [u8; 3],
    pub os_type: u8,
    pub ending_chs: [u8; 3],
    pub starting_lba: u32,
    pub size_in_lba: u32,
}

impl MbrPartitionRecord {
    pub fn parse(buf: &[u8]) -> Result<Self, StorageError> {
        if buf.len() < 16 {
            return Err(StorageError::InvalidData("MBR record buffer too small"));
        }
        Ok(Self {
            boot_indicator: buf[0],
            starting_chs: [buf[1], buf[2], buf[3]],
            os_type: buf[4],
            ending_chs: [buf[5], buf[6], buf[7]],
            starting_lba: u32::from_le_bytes([buf[8], buf[9], buf[10], buf[11]]),
            size_in_lba: u32::from_le_bytes([buf[12], buf[13], buf[14], buf[15]]),
        })
    }

    pub fn serialize(&self) -> [u8; 16] {
        let mut buf = [0u8; 16];
        buf[0] = self.boot_indicator;
        buf[1..4].copy_from_slice(&self.starting_chs);
        buf[4] = self.os_type;
        buf[5..8].copy_from_slice(&self.ending_chs);
        buf[8..12].copy_from_slice(&self.starting_lba.to_le_bytes());
        buf[12..16].copy_from_slice(&self.size_in_lba.to_le_bytes());
        buf
    }
}

/// Protective MBR at LBA 0.
#[derive(Debug, Clone)]
pub struct ProtectiveMbr {
    pub boot_code: [u8; 446],
    pub partitions: [MbrPartitionRecord; 4],
    pub signature: u16,
}

impl ProtectiveMbr {
    pub const SIGNATURE: u16 = 0xAA55;
    pub const GPT_PROTECTIVE_TYPE: u8 = 0xEE;

    pub fn parse(buf: &[u8]) -> Result<Self, StorageError> {
        if buf.len() < 512 {
            return Err(StorageError::InvalidData("MBR buffer must be at least 512 bytes"));
        }

        let sig = u16::from_le_bytes([buf[510], buf[511]]);
        if sig != Self::SIGNATURE {
            return Err(StorageError::InvalidData("Invalid MBR boot signature (expected 0xAA55)"));
        }

        let mut boot_code = [0u8; 446];
        boot_code.copy_from_slice(&buf[0..446]);

        let p1 = MbrPartitionRecord::parse(&buf[446..462])?;
        let p2 = MbrPartitionRecord::parse(&buf[462..478])?;
        let p3 = MbrPartitionRecord::parse(&buf[478..494])?;
        let p4 = MbrPartitionRecord::parse(&buf[494..510])?;

        Ok(Self {
            boot_code,
            partitions: [p1, p2, p3, p4],
            signature: sig,
        })
    }

    /// Check if this MBR contains a valid Protective GPT entry (type 0xEE).
    pub fn is_protective_gpt(&self) -> bool {
        self.partitions.iter().any(|p| p.os_type == Self::GPT_PROTECTIVE_TYPE)
    }

    /// Create a standard Protective MBR covering the given disk sector count.
    pub fn create_protective(disk_sectors: u64) -> Self {
        let size_in_lba = if disk_sectors > 0xFFFF_FFFF {
            0xFFFF_FFFF
        } else {
            (disk_sectors.saturating_sub(1)) as u32
        };

        let protective_entry = MbrPartitionRecord {
            boot_indicator: 0x00,
            starting_chs: [0x00, 0x02, 0x00],
            os_type: Self::GPT_PROTECTIVE_TYPE,
            ending_chs: [0xFF, 0xFF, 0xFF],
            starting_lba: 1,
            size_in_lba,
        };

        let empty_entry = MbrPartitionRecord {
            boot_indicator: 0,
            starting_chs: [0; 3],
            os_type: 0,
            ending_chs: [0; 3],
            starting_lba: 0,
            size_in_lba: 0,
        };

        Self {
            boot_code: [0u8; 446],
            partitions: [protective_entry, empty_entry, empty_entry, empty_entry],
            signature: Self::SIGNATURE,
        }
    }

    /// Serialize MBR to 512-byte sector.
    pub fn serialize(&self) -> [u8; 512] {
        let mut buf = [0u8; 512];
        buf[0..446].copy_from_slice(&self.boot_code);
        for (i, p) in self.partitions.iter().enumerate() {
            let offset = 446 + i * 16;
            buf[offset..offset + 16].copy_from_slice(&p.serialize());
        }
        buf[510..512].copy_from_slice(&self.signature.to_le_bytes());
        buf
    }
}

/// GPT Header at LBA 1 (and backup LBA).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GptHeader {
    pub signature: [u8; 8],
    pub revision: u32,
    pub header_size: u32,
    pub header_crc32: u32,
    pub reserved: u32,
    pub current_lba: u64,
    pub backup_lba: u64,
    pub first_usable_lba: u64,
    pub last_usable_lba: u64,
    pub disk_guid: Guid,
    pub partition_entry_lba: u64,
    pub number_of_partition_entries: u32,
    pub size_of_partition_entry: u32,
    pub partition_entry_array_crc32: u32,
}

impl GptHeader {
    pub const EFI_SIGNATURE: &'static [u8; 8] = b"EFI PART";
    pub const REVISION_1_0: u32 = 0x00010000;
    pub const MIN_HEADER_SIZE: u32 = 92;

    pub fn parse(buf: &[u8]) -> Result<Self, StorageError> {
        if buf.len() < 92 {
            return Err(StorageError::InvalidData("GPT header buffer too small"));
        }

        if &buf[0..8] != Self::EFI_SIGNATURE {
            return Err(StorageError::InvalidData("Invalid GPT signature, expected 'EFI PART'"));
        }

        let revision = u32::from_le_bytes([buf[8], buf[9], buf[10], buf[11]]);
        let header_size = u32::from_le_bytes([buf[12], buf[13], buf[14], buf[15]]);
        if header_size < Self::MIN_HEADER_SIZE || (header_size as usize) > buf.len() {
            return Err(StorageError::InvalidData("Invalid GPT header size"));
        }

        let header_crc32 = u32::from_le_bytes([buf[16], buf[17], buf[18], buf[19]]);
        let reserved = u32::from_le_bytes([buf[20], buf[21], buf[22], buf[23]]);
        let current_lba = u64::from_le_bytes(buf[24..32].try_into().unwrap());
        let backup_lba = u64::from_le_bytes(buf[32..40].try_into().unwrap());
        let first_usable_lba = u64::from_le_bytes(buf[40..48].try_into().unwrap());
        let last_usable_lba = u64::from_le_bytes(buf[48..56].try_into().unwrap());

        let mut disk_guid_bytes = [0u8; 16];
        disk_guid_bytes.copy_from_slice(&buf[56..72]);
        let disk_guid = Guid(disk_guid_bytes);

        let partition_entry_lba = u64::from_le_bytes(buf[72..80].try_into().unwrap());
        let number_of_partition_entries = u32::from_le_bytes([buf[80], buf[81], buf[82], buf[83]]);
        let size_of_partition_entry = u32::from_le_bytes([buf[84], buf[85], buf[86], buf[87]]);
        let partition_entry_array_crc32 = u32::from_le_bytes([buf[88], buf[89], buf[90], buf[91]]);

        // Verify Header CRC32 (with crc field zeroed)
        let mut verify_buf = vec![0u8; header_size as usize];
        verify_buf.copy_from_slice(&buf[..header_size as usize]);
        verify_buf[16..20].fill(0);
        let computed_crc = crc32(&verify_buf);

        if computed_crc != header_crc32 {
            return Err(StorageError::CrcMismatch {
                expected: header_crc32,
                computed: computed_crc,
            });
        }

        Ok(Self {
            signature: *Self::EFI_SIGNATURE,
            revision,
            header_size,
            header_crc32,
            reserved,
            current_lba,
            backup_lba,
            first_usable_lba,
            last_usable_lba,
            disk_guid,
            partition_entry_lba,
            number_of_partition_entries,
            size_of_partition_entry,
            partition_entry_array_crc32,
        })
    }

    /// Serialize header to 92-byte buffer with computed CRC32.
    pub fn serialize(&self) -> [u8; 92] {
        let mut buf = [0u8; 92];
        buf[0..8].copy_from_slice(&self.signature);
        buf[8..12].copy_from_slice(&self.revision.to_le_bytes());
        buf[12..16].copy_from_slice(&self.header_size.to_le_bytes());
        buf[16..20].fill(0); // Temporary zero for CRC calculation
        buf[20..24].copy_from_slice(&self.reserved.to_le_bytes());
        buf[24..32].copy_from_slice(&self.current_lba.to_le_bytes());
        buf[32..40].copy_from_slice(&self.backup_lba.to_le_bytes());
        buf[40..48].copy_from_slice(&self.first_usable_lba.to_le_bytes());
        buf[48..56].copy_from_slice(&self.last_usable_lba.to_le_bytes());
        buf[56..72].copy_from_slice(self.disk_guid.as_bytes());
        buf[72..80].copy_from_slice(&self.partition_entry_lba.to_le_bytes());
        buf[80..84].copy_from_slice(&self.number_of_partition_entries.to_le_bytes());
        buf[84..88].copy_from_slice(&self.size_of_partition_entry.to_le_bytes());
        buf[88..92].copy_from_slice(&self.partition_entry_array_crc32.to_le_bytes());

        let computed_crc = crc32(&buf);
        buf[16..20].copy_from_slice(&computed_crc.to_le_bytes());
        buf
    }
}

/// GPT Partition Entry (128 bytes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GptPartitionEntry {
    pub partition_type_guid: Guid,
    pub unique_partition_guid: Guid,
    pub starting_lba: u64,
    pub ending_lba: u64,
    pub attributes: u64,
    pub partition_name_utf16: [u16; 36],
    pub name: String,
}

impl GptPartitionEntry {
    pub const ATTRIBUTE_SYSTEM_PARTITION: u64 = 1 << 0;
    pub const ATTRIBUTE_READ_ONLY: u64 = 1 << 60;
    pub const ATTRIBUTE_HIDDEN: u64 = 1 << 62;
    pub const ATTRIBUTE_NO_AUTOMOUNT: u64 = 1 << 63;

    pub fn parse(buf: &[u8]) -> Result<Self, StorageError> {
        if buf.len() < 128 {
            return Err(StorageError::InvalidData("Partition entry buffer too small (must be 128 bytes)"));
        }

        let mut type_guid_bytes = [0u8; 16];
        type_guid_bytes.copy_from_slice(&buf[0..16]);
        let partition_type_guid = Guid(type_guid_bytes);

        let mut unique_guid_bytes = [0u8; 16];
        unique_guid_bytes.copy_from_slice(&buf[16..32]);
        let unique_partition_guid = Guid(unique_guid_bytes);

        let starting_lba = u64::from_le_bytes(buf[32..40].try_into().unwrap());
        let ending_lba = u64::from_le_bytes(buf[40..48].try_into().unwrap());
        let attributes = u64::from_le_bytes(buf[48..56].try_into().unwrap());

        let mut partition_name_utf16 = [0u16; 36];
        for i in 0..36 {
            let offset = 56 + i * 2;
            partition_name_utf16[i] = u16::from_le_bytes([buf[offset], buf[offset + 1]]);
        }

        // Decode UTF-16LE string until first null terminator
        let name_units: Vec<u16> = partition_name_utf16
            .iter()
            .copied()
            .take_while(|&u| u != 0)
            .collect();
        let name = String::from_utf16_lossy(&name_units);

        Ok(Self {
            partition_type_guid,
            unique_partition_guid,
            starting_lba,
            ending_lba,
            attributes,
            partition_name_utf16,
            name,
        })
    }

    /// Check if the partition entry is active / allocated.
    pub fn is_used(&self) -> bool {
        !self.partition_type_guid.is_zero() && self.ending_lba >= self.starting_lba
    }

    /// Number of 512-byte sectors in this partition.
    pub fn sector_count(&self) -> u64 {
        if self.ending_lba >= self.starting_lba {
            self.ending_lba - self.starting_lba + 1
        } else {
            0
        }
    }

    /// Total size in bytes for a given sector size (default 512).
    pub fn size_in_bytes(&self, sector_size: usize) -> u64 {
        self.sector_count() * (sector_size as u64)
    }

    /// Check if partition is Windows Basic Data (NTFS / FAT32 / ReFS).
    pub fn is_windows_data(&self) -> bool {
        self.partition_type_guid == Guid::WINDOWS_BASIC_DATA
    }

    /// Check if partition is Linux Filesystem Data (ext4 / btrfs / xfs).
    pub fn is_linux_fs(&self) -> bool {
        self.partition_type_guid == Guid::LINUX_FILESYSTEM || self.partition_type_guid == Guid::LINUX_ROOT_X86_64
    }

    /// Check if partition is EFI System Partition (ESP).
    pub fn is_efi_system(&self) -> bool {
        self.partition_type_guid == Guid::EFI_SYSTEM
    }

    /// Serialize partition entry to 128 bytes.
    pub fn serialize(&self) -> [u8; 128] {
        let mut buf = [0u8; 128];
        buf[0..16].copy_from_slice(self.partition_type_guid.as_bytes());
        buf[16..32].copy_from_slice(self.unique_partition_guid.as_bytes());
        buf[32..40].copy_from_slice(&self.starting_lba.to_le_bytes());
        buf[40..48].copy_from_slice(&self.ending_lba.to_le_bytes());
        buf[48..56].copy_from_slice(&self.attributes.to_le_bytes());
        for (i, &unit) in self.partition_name_utf16.iter().enumerate() {
            let offset = 56 + i * 2;
            buf[offset..offset + 2].copy_from_slice(&unit.to_le_bytes());
        }
        buf
    }

    /// Construct a partition entry with given parameters and UTF-8 name.
    pub fn new(
        type_guid: Guid,
        unique_guid: Guid,
        starting_lba: u64,
        ending_lba: u64,
        attributes: u64,
        name: &str,
    ) -> Self {
        let mut partition_name_utf16 = [0u16; 36];
        for (i, u) in name.encode_utf16().take(36).enumerate() {
            partition_name_utf16[i] = u;
        }

        Self {
            partition_type_guid: type_guid,
            unique_partition_guid: unique_guid,
            starting_lba,
            ending_lba,
            attributes,
            partition_name_utf16,
            name: name.to_string(),
        }
    }
}

/// Parsed GPT Partition Table from a block device.
#[derive(Debug, Clone)]
pub struct GptDisk {
    pub protective_mbr: ProtectiveMbr,
    pub header: GptHeader,
    pub partitions: Vec<GptPartitionEntry>,
}

impl GptDisk {
    /// Parse GPT and Protective MBR from any device implementing BlockDevice.
    pub fn parse_from_device(device: &dyn BlockDevice) -> Result<Self, StorageError> {
        let sec_size = device.sector_size();
        if sec_size != 512 {
            return Err(StorageError::InvalidData("GPT driver currently supports 512-byte sector devices"));
        }

        // 1. Read Protective MBR at LBA 0
        let mut lba0 = vec![0u8; 512];
        device.read_sectors(0, &mut lba0)?;
        let protective_mbr = ProtectiveMbr::parse(&lba0)?;
        if !protective_mbr.is_protective_gpt() {
            return Err(StorageError::InvalidData("LBA 0 is not a valid GPT Protective MBR"));
        }

        // 2. Read Primary GPT Header at LBA 1
        let mut lba1 = vec![0u8; 512];
        device.read_sectors(1, &mut lba1)?;
        let header = GptHeader::parse(&lba1)?;

        // 3. Read Partition Entry Array
        let entry_size = header.size_of_partition_entry as usize;
        let num_entries = header.number_of_partition_entries as usize;
        let total_array_bytes = entry_size * num_entries;
        let sectors_for_array = (total_array_bytes + sec_size - 1) / sec_size;

        let mut array_buf = vec![0u8; sectors_for_array * sec_size];
        device.read_sectors(header.partition_entry_lba, &mut array_buf)?;

        // Verify Partition Array CRC32
        let actual_array_bytes = &array_buf[..total_array_bytes];
        let computed_array_crc = crc32(actual_array_bytes);
        if computed_array_crc != header.partition_entry_array_crc32 {
            return Err(StorageError::CrcMismatch {
                expected: header.partition_entry_array_crc32,
                computed: computed_array_crc,
            });
        }

        // 4. Parse individual partition entries
        let mut partitions = Vec::new();
        for i in 0..num_entries {
            let offset = i * entry_size;
            let entry_slice = &actual_array_bytes[offset..offset + entry_size];
            let entry = GptPartitionEntry::parse(entry_slice)?;
            if entry.is_used() {
                partitions.push(entry);
            }
        }

        Ok(Self {
            protective_mbr,
            header,
            partitions,
        })
    }

    /// Find an active partition by name (case-insensitive).
    pub fn find_partition_by_name(&self, name: &str) -> Option<&GptPartitionEntry> {
        self.partitions.iter().find(|p| p.name.eq_ignore_ascii_case(name))
    }

    /// Find all active partitions matching a partition type GUID.
    pub fn find_partitions_by_type(&self, type_guid: &Guid) -> Vec<&GptPartitionEntry> {
        self.partitions.iter().filter(|p| &p.partition_type_guid == type_guid).collect()
    }

    /// Helper to synthesize a complete GPT disk image buffer for testing.
    pub fn create_mock_gpt_image(
        total_sectors: u64,
        disk_guid: Guid,
        partitions_spec: &[(Guid, Guid, u64, u64, u64, &str)],
    ) -> Vec<u8> {
        let sec_size = 512;
        let mut image = vec![0u8; (total_sectors as usize) * sec_size];

        // LBA 0: Protective MBR
        let mbr = ProtectiveMbr::create_protective(total_sectors);
        image[0..512].copy_from_slice(&mbr.serialize());

        // Partition entries array
        let num_entries = 128usize;
        let entry_size = 128usize;
        let mut entries_buf = vec![0u8; num_entries * entry_size];

        for (i, spec) in partitions_spec.iter().enumerate() {
            if i >= num_entries {
                break;
            }
            let entry = GptPartitionEntry::new(spec.0, spec.1, spec.2, spec.3, spec.4, spec.5);
            let offset = i * entry_size;
            entries_buf[offset..offset + entry_size].copy_from_slice(&entry.serialize());
        }

        let array_crc = crc32(&entries_buf);

        // LBA 1: GPT Header
        let header = GptHeader {
            signature: *GptHeader::EFI_SIGNATURE,
            revision: GptHeader::REVISION_1_0,
            header_size: 92,
            header_crc32: 0,
            reserved: 0,
            current_lba: 1,
            backup_lba: total_sectors - 1,
            first_usable_lba: 34,
            last_usable_lba: total_sectors - 34,
            disk_guid,
            partition_entry_lba: 2,
            number_of_partition_entries: 128,
            size_of_partition_entry: 128,
            partition_entry_array_crc32: array_crc,
        };

        let header_bytes = header.serialize();
        image[512..512 + 92].copy_from_slice(&header_bytes);

        // LBA 2..33: Partition Array (32 sectors = 128 * 128 = 16384 bytes)
        let array_start = 2 * sec_size;
        image[array_start..array_start + entries_buf.len()].copy_from_slice(&entries_buf);

        image
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crc32_standard_vector() {
        assert_eq!(crc32(b"123456789"), 0xCBF43926);
    }

    #[test]
    fn test_guid_parse_and_display() {
        let guid_str = "EBD0A0A2-B9E5-4433-87C0-68B6B72699C7";
        let guid = Guid::parse(guid_str).unwrap();
        assert_eq!(guid, Guid::WINDOWS_BASIC_DATA);
        assert_eq!(guid.to_string(), guid_str);

        let linux_guid_str = "0FC63DAF-8483-4772-8E79-3D69D8477DE4";
        let linux_guid = Guid::parse(linux_guid_str).unwrap();
        assert_eq!(linux_guid, Guid::LINUX_FILESYSTEM);
        assert_eq!(linux_guid.to_string(), linux_guid_str);

        let esp_guid_str = "C12A7328-F81F-11D2-BA4B-00A0C93EC93B";
        let esp_guid = Guid::parse(esp_guid_str).unwrap();
        assert_eq!(esp_guid, Guid::EFI_SYSTEM);
        assert_eq!(esp_guid.to_string(), esp_guid_str);
    }

    #[test]
    fn test_protective_mbr_serialization() {
        let mbr = ProtectiveMbr::create_protective(204800);
        let bytes = mbr.serialize();
        let parsed = ProtectiveMbr::parse(&bytes).unwrap();
        assert!(parsed.is_protective_gpt());
        assert_eq!(parsed.partitions[0].os_type, 0xEE);
        assert_eq!(parsed.partitions[0].starting_lba, 1);
        assert_eq!(parsed.partitions[0].size_in_lba, 204799);
    }

    #[test]
    fn test_gpt_header_crc_and_serialization() {
        let header = GptHeader {
            signature: *GptHeader::EFI_SIGNATURE,
            revision: GptHeader::REVISION_1_0,
            header_size: 92,
            header_crc32: 0,
            reserved: 0,
            current_lba: 1,
            backup_lba: 204799,
            first_usable_lba: 34,
            last_usable_lba: 204766,
            disk_guid: Guid::WINDOWS_BASIC_DATA,
            partition_entry_lba: 2,
            number_of_partition_entries: 128,
            size_of_partition_entry: 128,
            partition_entry_array_crc32: 0x12345678,
        };

        let serialized = header.serialize();
        let parsed = GptHeader::parse(&serialized).unwrap();
        assert_eq!(parsed.current_lba, 1);
        assert_eq!(parsed.backup_lba, 204799);
        assert_eq!(parsed.disk_guid, Guid::WINDOWS_BASIC_DATA);
        assert_eq!(parsed.partition_entry_array_crc32, 0x12345678);
    }

    #[test]
    fn test_gpt_partition_entry_utf16_name() {
        let entry = GptPartitionEntry::new(
            Guid::WINDOWS_BASIC_DATA,
            Guid::parse("11223344-5566-7788-99AA-BBCCDDEEFF00").unwrap(),
            2048,
            4095,
            0,
            "Windows System Data",
        );

        let bytes = entry.serialize();
        let parsed = GptPartitionEntry::parse(&bytes).unwrap();
        assert_eq!(parsed.name, "Windows System Data");
        assert_eq!(parsed.starting_lba, 2048);
        assert_eq!(parsed.ending_lba, 4095);
        assert_eq!(parsed.sector_count(), 2048);
        assert_eq!(parsed.size_in_bytes(512), 2048 * 512);
        assert!(parsed.is_windows_data());
        assert!(!parsed.is_linux_fs());
    }
}
