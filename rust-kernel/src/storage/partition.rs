//! Universal MBR & GPT Partition Table Parser for HybridOS.
//!
//! Provides bare-metal partition table discovery across ATA, NVMe, AHCI, and VirtIO disks.
//! Supports:
//! - MBR (Master Boot Record, Sector 0) with 4 primary partitions (FAT32, EXT4, NTFS)
//! - GPT (GUID Partition Table, Sector 1) with Protective MBR, CRC32 checks, and UTF-16 partition names
//! - Automatic boundary detection (Start LBA, Sector Count, End LBA)

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt;

use super::{BlockDevice, StorageError};
use super::gpt::{GptHeader, GptPartitionEntry, Guid, crc32};

/// Filesystem kind identified by partition indicators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartitionFsKind {
    Fat32,
    Ext4,
    Ntfs,
    LinuxSwap,
    Unknown(u8),
}

impl fmt::Display for PartitionFsKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PartitionFsKind::Fat32 => write!(f, "FAT32"),
            PartitionFsKind::Ext4 => write!(f, "EXT4"),
            PartitionFsKind::Ntfs => write!(f, "NTFS"),
            PartitionFsKind::LinuxSwap => write!(f, "Linux Swap"),
            PartitionFsKind::Unknown(t) => write!(f, "Unknown (0x{:02X})", t),
        }
    }
}

/// Unified Partition Descriptor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartitionInfo {
    pub index: usize,
    pub name: String,
    pub start_lba: u64,
    pub sector_count: u64,
    pub fs_kind: PartitionFsKind,
    pub is_bootable: bool,
    pub type_guid: Option<Guid>,
}

impl PartitionInfo {
    pub fn end_lba(&self) -> u64 {
        self.start_lba.saturating_add(self.sector_count).saturating_sub(1)
    }
}

/// Disk Partition Map holding all detected partitions on a block device.
#[derive(Debug, Clone)]
pub struct DiskPartitionMap {
    pub is_gpt: bool,
    pub disk_guid: Option<Guid>,
    pub partitions: Vec<PartitionInfo>,
}

impl DiskPartitionMap {
    /// Inspect Sector 0 (MBR) and Sector 1 (GPT) of any BlockDevice and extract all valid partitions.
    pub fn parse(device: &dyn BlockDevice) -> Result<Self, StorageError> {
        let sec_size = device.sector_size();
        if sec_size != 512 {
            return Err(StorageError::InvalidData("Universal partition parser requires 512-byte sectors"));
        }

        // 1. Read Sector 0 (MBR)
        let mut mbr_buf = [0u8; 512];
        device.read_sectors(0, &mut mbr_buf)?;

        let sig = u16::from_le_bytes([mbr_buf[510], mbr_buf[511]]);
        if sig != 0xAA55 {
            return Err(StorageError::InvalidData("Sector 0 lacks valid 0x55AA boot signature"));
        }

        // Check for Protective MBR (Partition type 0xEE indicating GPT)
        let mut has_protective_gpt = false;
        for i in 0..4 {
            let offset = 446 + (i * 16);
            let os_type = mbr_buf[offset + 4];
            if os_type == 0xEE {
                has_protective_gpt = true;
                break;
            }
        }

        if has_protective_gpt {
            // Attempt full GPT parse from LBA 1
            if let Ok(gpt_map) = Self::parse_gpt(device) {
                return Ok(gpt_map);
            }
        }

        // Fallback: Parse Classic MBR Partitions
        Self::parse_mbr(&mbr_buf)
    }

    /// Parse classic 4-entry MBR partition table from Sector 0.
    fn parse_mbr(mbr_buf: &[u8; 512]) -> Result<Self, StorageError> {
        let mut partitions = Vec::new();

        for i in 0..4 {
            let offset = 446 + (i * 16);
            let boot_indicator = mbr_buf[offset];
            let os_type = mbr_buf[offset + 4];
            let start_lba = u32::from_le_bytes([
                mbr_buf[offset + 8],
                mbr_buf[offset + 9],
                mbr_buf[offset + 10],
                mbr_buf[offset + 11],
            ]) as u64;
            let sector_count = u32::from_le_bytes([
                mbr_buf[offset + 12],
                mbr_buf[offset + 13],
                mbr_buf[offset + 14],
                mbr_buf[offset + 15],
            ]) as u64;

            if os_type == 0x00 || sector_count == 0 {
                continue;
            }

            let fs_kind = match os_type {
                0x0B | 0x0C | 0x01 | 0x06 | 0x0E => PartitionFsKind::Fat32,
                0x83 => PartitionFsKind::Ext4,
                0x07 => PartitionFsKind::Ntfs,
                0x82 => PartitionFsKind::LinuxSwap,
                other => PartitionFsKind::Unknown(other),
            };

            let name = alloc::format!("MBR_Part{}", i + 1);

            partitions.push(PartitionInfo {
                index: i + 1,
                name,
                start_lba,
                sector_count,
                fs_kind,
                is_bootable: (boot_indicator & 0x80) != 0,
                type_guid: None,
            });
        }

        Ok(Self {
            is_gpt: false,
            disk_guid: None,
            partitions,
        })
    }

    /// Parse GPT Header at LBA 1 and partition array.
    fn parse_gpt(device: &dyn BlockDevice) -> Result<Self, StorageError> {
        let mut lba1 = [0u8; 512];
        device.read_sectors(1, &mut lba1)?;
        let header = GptHeader::parse(&lba1)?;

        let entry_size = header.size_of_partition_entry as usize;
        let num_entries = header.number_of_partition_entries as usize;
        let total_bytes = entry_size * num_entries;
        let num_sectors = (total_bytes + 511) / 512;

        let mut array_buf = alloc::vec![0u8; num_sectors * 512];
        device.read_sectors(header.partition_entry_lba, &mut array_buf)?;

        let computed_crc = crc32(&array_buf[..total_bytes]);
        if computed_crc != header.partition_entry_array_crc32 {
            return Err(StorageError::CrcMismatch {
                expected: header.partition_entry_array_crc32,
                computed: computed_crc,
            });
        }

        let mut partitions = Vec::new();
        for i in 0..num_entries {
            let offset = i * entry_size;
            let entry_slice = &array_buf[offset..offset + entry_size];
            let entry = GptPartitionEntry::parse(entry_slice)?;

            if !entry.is_used() {
                continue;
            }

            let fs_kind = if entry.is_windows_data() || entry.is_efi_system() {
                PartitionFsKind::Fat32
            } else if entry.is_linux_fs() {
                PartitionFsKind::Ext4
            } else if entry.partition_type_guid == Guid::LINUX_SWAP {
                PartitionFsKind::LinuxSwap
            } else {
                PartitionFsKind::Unknown(0xFF)
            };

            partitions.push(PartitionInfo {
                index: i + 1,
                name: if entry.name.is_empty() { alloc::format!("GPT_Part{}", i + 1) } else { entry.name.clone() },
                start_lba: entry.starting_lba,
                sector_count: entry.sector_count(),
                fs_kind,
                is_bootable: (entry.attributes & GptPartitionEntry::ATTRIBUTE_SYSTEM_PARTITION) != 0,
                type_guid: Some(entry.partition_type_guid),
            });
        }

        Ok(Self {
            is_gpt: true,
            disk_guid: Some(header.disk_guid),
            partitions,
        })
    }

    /// Retrieve the first primary FAT32 partition.
    pub fn find_primary_fat32(&self) -> Option<&PartitionInfo> {
        self.partitions.iter().find(|p| p.fs_kind == PartitionFsKind::Fat32)
    }

    /// Retrieve the first Linux EXT4 partition.
    pub fn find_linux_ext4(&self) -> Option<&PartitionInfo> {
        self.partitions.iter().find(|p| p.fs_kind == PartitionFsKind::Ext4)
    }
}
