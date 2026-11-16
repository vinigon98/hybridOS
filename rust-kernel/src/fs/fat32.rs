//! Authentic, Bare-Metal FAT32 File System Parser for HybridOS.
//!
//! Reads directly from physical disk sectors via the Primary ATA PIO Controller.
//! Zero mocks: parses MBR, Volume Boot Record (VBR), File Allocation Tables (FAT1/FAT2),
//! Directory Clusters, and Cluster Chains to read authentic binary files from disk into memory.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use crate::drivers::ata::{GLOBAL_ATA, SECTOR_SIZE};

#[derive(Debug, Clone)]
pub struct Fat32Entry {
    pub name: String,
    pub is_directory: bool,
    pub first_cluster: u32,
    pub size: u32,
}

#[derive(Debug, Clone)]
pub struct Fat32FileSystem {
    pub partition_start_lba: u32,
    pub bytes_per_sector: u16,
    pub sectors_per_cluster: u8,
    pub reserved_sectors: u16,
    pub num_fats: u8,
    pub sectors_per_fat: u32,
    pub root_cluster: u32,
    pub fat_start_lba: u32,
    pub data_start_lba: u32,
}

impl Fat32FileSystem {
    /// Mounts and parses the first FAT32 partition from the primary ATA hard disk.
    pub fn mount_primary_drive() -> Result<Self, &'static str> {
        let mut mbr = [0u8; 512];
        {
            let mut ata = GLOBAL_ATA.lock();
            ata.read_sector(0, &mut mbr)?;
        }

        if mbr[510] != 0x55 || mbr[511] != 0xAA {
            return Err("Invalid MBR boot signature");
        }

        // Partition 1 entry starts at offset 0x1BE (446)
        let part_start_lba = u32::from_le_bytes([
            mbr[0x1BE + 8], mbr[0x1BE + 9], mbr[0x1BE + 10], mbr[0x1BE + 11],
        ]);
        let part_sectors = u32::from_le_bytes([
            mbr[0x1BE + 12], mbr[0x1BE + 13], mbr[0x1BE + 14], mbr[0x1BE + 15],
        ]);

        if part_start_lba == 0 || part_sectors == 0 {
            return Err("Primary partition table entry is empty");
        }

        // Read VBR (Volume Boot Record) at partition_start_lba
        let mut vbr = [0u8; 512];
        {
            let mut ata = GLOBAL_ATA.lock();
            ata.read_sector(part_start_lba, &mut vbr)?;
        }

        if vbr[510] != 0x55 || vbr[511] != 0xAA {
            return Err("Invalid FAT32 VBR boot signature");
        }

        let bytes_per_sector = u16::from_le_bytes([vbr[11], vbr[12]]);
        let sectors_per_cluster = vbr[13];
        let reserved_sectors = u16::from_le_bytes([vbr[14], vbr[15]]);
        let num_fats = vbr[16];
        let sectors_per_fat = u32::from_le_bytes([vbr[36], vbr[37], vbr[38], vbr[39]]);
        let root_cluster = u32::from_le_bytes([vbr[44], vbr[45], vbr[46], vbr[47]]);

        if bytes_per_sector != 512 || sectors_per_cluster == 0 || num_fats == 0 {
            return Err("Invalid FAT32 BPB parameters");
        }

        let fat_start_lba = part_start_lba + reserved_sectors as u32;
        let data_start_lba = fat_start_lba + (num_fats as u32 * sectors_per_fat);

        crate::serial_println!(
            "[FAT32] Mounted Partition @ LBA {}: ClusSize={} B, RootClus={}, DataLBA={}",
            part_start_lba,
            sectors_per_cluster as usize * 512,
            root_cluster,
            data_start_lba
        );

        Ok(Self {
            partition_start_lba: part_start_lba,
            bytes_per_sector,
            sectors_per_cluster,
            reserved_sectors,
            num_fats,
            sectors_per_fat,
            root_cluster,
            fat_start_lba,
            data_start_lba,
        })
    }

    /// Converts a cluster number into a 28-bit physical LBA sector address.
    #[inline]
    pub fn cluster_to_lba(&self, cluster: u32) -> u32 {
        self.data_start_lba + ((cluster.saturating_sub(2)) * self.sectors_per_cluster as u32)
    }

    /// Reads the next cluster pointer from the primary File Allocation Table (FAT1).
    pub fn next_cluster(&self, cluster: u32) -> Result<u32, &'static str> {
        let fat_offset_bytes = cluster * 4;
        let fat_sector = self.fat_start_lba + (fat_offset_bytes / 512);
        let byte_in_sector = (fat_offset_bytes % 512) as usize;

        let mut sector_buf = [0u8; 512];
        {
            let mut ata = GLOBAL_ATA.lock();
            ata.read_sector(fat_sector, &mut sector_buf)?;
        }

        let raw_val = u32::from_le_bytes([
            sector_buf[byte_in_sector],
            sector_buf[byte_in_sector + 1],
            sector_buf[byte_in_sector + 2],
            sector_buf[byte_in_sector + 3],
        ]);

        Ok(raw_val & 0x0FFF_FFFF)
    }

    /// Lists entries in a directory identified by its start cluster.
    pub fn list_directory(&self, start_cluster: u32) -> Result<Vec<Fat32Entry>, &'static str> {
        let mut entries = Vec::new();
        let mut curr_cluster = start_cluster;
        let cluster_bytes = self.sectors_per_cluster as usize * 512;
        let mut clus_buf = alloc::vec![0u8; cluster_bytes];

        let mut lfn_parts: Vec<(u8, String)> = Vec::new();

        while curr_cluster >= 2 && curr_cluster < 0x0FFF_FFF8 {
            let clus_lba = self.cluster_to_lba(curr_cluster);
            {
                let mut ata = GLOBAL_ATA.lock();
                ata.read_sectors(clus_lba, self.sectors_per_cluster as usize, &mut clus_buf)?;
            }

            for entry_idx in 0..(cluster_bytes / 32) {
                let off = entry_idx * 32;
                let first_byte = clus_buf[off];

                if first_byte == 0x00 {
                    return Ok(entries); // End of directory
                }
                if first_byte == 0xE5 {
                    lfn_parts.clear();
                    continue; // Deleted entry
                }

                let attr = clus_buf[off + 11];

                // Check for LFN entry (attr == 0x0F)
                if attr == 0x0F {
                    let seq = first_byte;
                    let mut name_part = String::new();
                    // LFN Unicode characters in offsets 1..11, 14..26, 28..32
                    let char_offsets = [1, 3, 5, 7, 9, 14, 16, 18, 20, 22, 24, 28, 30];
                    for &co in &char_offsets {
                        let c16 = u16::from_le_bytes([clus_buf[off + co], clus_buf[off + co + 1]]);
                        if c16 != 0 && c16 != 0xFFFF {
                            if let Some(ch) = core::char::from_u32(c16 as u32) {
                                name_part.push(ch);
                            }
                        }
                    }
                    lfn_parts.push((seq & 0x1F, name_part));
                    continue;
                }

                // Volume label
                if (attr & 0x08) != 0 {
                    lfn_parts.clear();
                    continue;
                }

                let is_directory = (attr & 0x10) != 0;
                let high_cluster = u16::from_le_bytes([clus_buf[off + 20], clus_buf[off + 21]]);
                let low_cluster = u16::from_le_bytes([clus_buf[off + 26], clus_buf[off + 27]]);
                let first_cluster = ((high_cluster as u32) << 16) | (low_cluster as u32);
                let size = u32::from_le_bytes([
                    clus_buf[off + 28], clus_buf[off + 29], clus_buf[off + 30], clus_buf[off + 31],
                ]);

                let entry_name = if !lfn_parts.is_empty() {
                    lfn_parts.sort_by_key(|k| k.0);
                    let mut full_name = String::new();
                    for (_, part) in &lfn_parts {
                        full_name.push_str(part);
                    }
                    lfn_parts.clear();
                    full_name
                } else {
                    let mut s = String::new();
                    let raw_name = &clus_buf[off..off + 8];
                    let raw_ext = &clus_buf[off + 8..off + 11];
                    let base = core::str::from_utf8(raw_name).unwrap_or("").trim_end();
                    let ext = core::str::from_utf8(raw_ext).unwrap_or("").trim_end();
                    s.push_str(base);
                    if !ext.is_empty() {
                        s.push('.');
                        s.push_str(ext);
                    }
                    s
                };

                entries.push(Fat32Entry {
                    name: entry_name,
                    is_directory,
                    first_cluster,
                    size,
                });
            }

            curr_cluster = self.next_cluster(curr_cluster)?;
        }

        Ok(entries)
    }

    /// Reads an entire file's payload from disk by following its cluster chain.
    pub fn read_file(&self, entry: &Fat32Entry) -> Result<Vec<u8>, &'static str> {
        let mut file_data = Vec::with_capacity(entry.size as usize);
        let mut curr_cluster = entry.first_cluster;
        let cluster_bytes = self.sectors_per_cluster as usize * 512;
        let mut clus_buf = alloc::vec![0u8; cluster_bytes];
        let mut remaining = entry.size as usize;

        while curr_cluster >= 2 && curr_cluster < 0x0FFF_FFF8 && remaining > 0 {
            let clus_lba = self.cluster_to_lba(curr_cluster);
            {
                let mut ata = GLOBAL_ATA.lock();
                ata.read_sectors(clus_lba, self.sectors_per_cluster as usize, &mut clus_buf)?;
            }

            let to_copy = remaining.min(cluster_bytes);
            file_data.extend_from_slice(&clus_buf[..to_copy]);
            remaining -= to_copy;

            curr_cluster = self.next_cluster(curr_cluster)?;
        }

        Ok(file_data)
    }

    /// Finds and reads a file by name starting from the root directory.
    pub fn read_root_file(&self, target_name: &str) -> Result<Vec<u8>, &'static str> {
        let entries = self.list_directory(self.root_cluster)?;
        for entry in entries {
            if entry.name.eq_ignore_ascii_case(target_name) {
                return self.read_file(&entry);
            }
        }
        Err("File not found on FAT32 partition")
    }
}
