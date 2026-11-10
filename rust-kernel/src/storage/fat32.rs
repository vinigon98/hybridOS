//! Full FAT32 Filesystem Driver for HybridOS.
//!
//! Features:
//! - BPB (BIOS Parameter Block) & Extended BPB parsing and serialization.
//! - FAT table traversal: cluster chains, free clusters, bad clusters, and EOF markers.
//! - Directory entry parser: Short Directory Entry (8.3) and Long File Name (LFN) entries.
//! - File read, write, directory listing (`readdir`), cluster allocation, and directory creation.
//! - In-memory formatting support for provisioning disks and automated unit testing.

use std::cmp::min;
use std::sync::{Arc, Mutex};
use super::{BlockDevice, StorageError};

/// FAT entry special cluster markers.
pub const FAT32_FREE_CLUSTER: u32 = 0x0000_0000;
pub const FAT32_RESERVED_CLUSTER: u32 = 0x0000_0001;
pub const FAT32_BAD_CLUSTER: u32 = 0x0FFF_FFF7;
pub const FAT32_EOF_MIN: u32 = 0x0FFF_FFF8;
pub const FAT32_EOF_STANDARD: u32 = 0x0FFF_FFFF;
pub const FAT32_CLUSTER_MASK: u32 = 0x0FFF_FFFF;

/// FAT32 Directory Entry Attributes.
pub mod fat_attrs {
    pub const READ_ONLY: u8 = 0x01;
    pub const HIDDEN: u8 = 0x02;
    pub const SYSTEM: u8 = 0x04;
    pub const VOLUME_ID: u8 = 0x08;
    pub const DIRECTORY: u8 = 0x10;
    pub const ARCHIVE: u8 = 0x20;
    pub const LFN: u8 = READ_ONLY | HIDDEN | SYSTEM | VOLUME_ID; // 0x0F
}

/// Parsed FAT32 BIOS Parameter Block (BPB) and Extended BPB.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fat32Bpb {
    pub bytes_per_sector: u16,
    pub sectors_per_cluster: u8,
    pub reserved_sectors: u16,
    pub fats_count: u8,
    pub root_ent_cnt: u16,
    pub total_sectors_16: u16,
    pub media_type: u8,
    pub fat_sz_16: u16,
    pub sectors_per_track: u16,
    pub heads_count: u16,
    pub hidden_sectors: u32,
    pub total_sectors_32: u32,

    // Extended FAT32 Fields
    pub sectors_per_fat: u32,
    pub ext_flags: u16,
    pub fs_version: u16,
    pub root_cluster: u32,
    pub fs_info_sector: u16,
    pub backup_boot_sector: u16,
    pub drive_number: u8,
    pub boot_signature: u8,
    pub volume_id: u32,
    pub volume_label: String,
    pub fs_type: String,
}

impl Fat32Bpb {
    pub fn parse(buf: &[u8]) -> Result<Self, StorageError> {
        if buf.len() < 512 {
            return Err(StorageError::InvalidData("FAT32 boot sector buffer too small"));
        }

        let sig = u16::from_le_bytes([buf[510], buf[511]]);
        if sig != 0xAA55 {
            return Err(StorageError::InvalidData("Invalid boot signature (expected 0xAA55)"));
        }

        let bytes_per_sector = u16::from_le_bytes([buf[11], buf[12]]);
        if bytes_per_sector != 512 && bytes_per_sector != 1024 && bytes_per_sector != 2048 && bytes_per_sector != 4096 {
            return Err(StorageError::InvalidData("Invalid bytes_per_sector in BPB"));
        }

        let sectors_per_cluster = buf[13];
        if sectors_per_cluster == 0 || (sectors_per_cluster & (sectors_per_cluster - 1)) != 0 {
            return Err(StorageError::InvalidData("sectors_per_cluster must be a power of 2"));
        }

        let reserved_sectors = u16::from_le_bytes([buf[14], buf[15]]);
        let fats_count = buf[16];
        let root_ent_cnt = u16::from_le_bytes([buf[17], buf[18]]);
        let total_sectors_16 = u16::from_le_bytes([buf[19], buf[20]]);
        let media_type = buf[21];
        let fat_sz_16 = u16::from_le_bytes([buf[22], buf[23]]);
        let sectors_per_track = u16::from_le_bytes([buf[24], buf[25]]);
        let heads_count = u16::from_le_bytes([buf[26], buf[27]]);
        let hidden_sectors = u32::from_le_bytes([buf[28], buf[29], buf[30], buf[31]]);
        let total_sectors_32 = u32::from_le_bytes([buf[32], buf[33], buf[34], buf[35]]);

        // In FAT32: root_ent_cnt and fat_sz_16 must be 0
        if fat_sz_16 != 0 {
            return Err(StorageError::InvalidData("BPB fat_sz_16 is not 0: not a FAT32 volume"));
        }

        let sectors_per_fat = u32::from_le_bytes([buf[36], buf[37], buf[38], buf[39]]);
        let ext_flags = u16::from_le_bytes([buf[40], buf[41]]);
        let fs_version = u16::from_le_bytes([buf[42], buf[43]]);
        let root_cluster = u32::from_le_bytes([buf[44], buf[45], buf[46], buf[47]]);
        let fs_info_sector = u16::from_le_bytes([buf[48], buf[49]]);
        let backup_boot_sector = u16::from_le_bytes([buf[50], buf[51]]);
        let drive_number = buf[64];
        let boot_signature = buf[66];
        let volume_id = u32::from_le_bytes([buf[67], buf[68], buf[69], buf[70]]);

        let volume_label = String::from_utf8_lossy(&buf[71..82]).trim().to_string();
        let fs_type = String::from_utf8_lossy(&buf[82..90]).trim().to_string();

        Ok(Self {
            bytes_per_sector,
            sectors_per_cluster,
            reserved_sectors,
            fats_count,
            root_ent_cnt,
            total_sectors_16,
            media_type,
            fat_sz_16,
            sectors_per_track,
            heads_count,
            hidden_sectors,
            total_sectors_32,
            sectors_per_fat,
            ext_flags,
            fs_version,
            root_cluster,
            fs_info_sector,
            backup_boot_sector,
            drive_number,
            boot_signature,
            volume_id,
            volume_label,
            fs_type,
        })
    }

    /// Serialize BPB to 512-byte boot sector.
    pub fn serialize(&self) -> [u8; 512] {
        let mut buf = [0u8; 512];
        // JMP short 0x58, NOP
        buf[0] = 0xEB;
        buf[1] = 0x58;
        buf[2] = 0x90;
        buf[3..11].copy_from_slice(b"MSWIN4.1");

        buf[11..13].copy_from_slice(&self.bytes_per_sector.to_le_bytes());
        buf[13] = self.sectors_per_cluster;
        buf[14..16].copy_from_slice(&self.reserved_sectors.to_le_bytes());
        buf[16] = self.fats_count;
        buf[17..19].copy_from_slice(&self.root_ent_cnt.to_le_bytes());
        buf[19..21].copy_from_slice(&self.total_sectors_16.to_le_bytes());
        buf[21] = self.media_type;
        buf[22..24].copy_from_slice(&self.fat_sz_16.to_le_bytes());
        buf[24..26].copy_from_slice(&self.sectors_per_track.to_le_bytes());
        buf[26..28].copy_from_slice(&self.heads_count.to_le_bytes());
        buf[28..32].copy_from_slice(&self.hidden_sectors.to_le_bytes());
        buf[32..36].copy_from_slice(&self.total_sectors_32.to_le_bytes());

        // FAT32 Extended fields
        buf[36..40].copy_from_slice(&self.sectors_per_fat.to_le_bytes());
        buf[40..42].copy_from_slice(&self.ext_flags.to_le_bytes());
        buf[42..44].copy_from_slice(&self.fs_version.to_le_bytes());
        buf[44..48].copy_from_slice(&self.root_cluster.to_le_bytes());
        buf[48..50].copy_from_slice(&self.fs_info_sector.to_le_bytes());
        buf[50..52].copy_from_slice(&self.backup_boot_sector.to_le_bytes());
        buf[64] = self.drive_number;
        buf[66] = self.boot_signature;
        buf[67..71].copy_from_slice(&self.volume_id.to_le_bytes());

        let mut label_padded = [b' '; 11];
        let lab_bytes = self.volume_label.as_bytes();
        let copy_len = min(lab_bytes.len(), 11);
        label_padded[..copy_len].copy_from_slice(&lab_bytes[..copy_len]);
        buf[71..82].copy_from_slice(&label_padded);

        let mut type_padded = [b' '; 8];
        let type_bytes = self.fs_type.as_bytes();
        let copy_tlen = min(type_bytes.len(), 8);
        type_padded[..copy_tlen].copy_from_slice(&type_bytes[..copy_tlen]);
        buf[82..90].copy_from_slice(&type_padded);

        buf[510] = 0x55;
        buf[511] = 0xAA;
        buf
    }

    /// Total sectors on the volume (prefers total_sectors_32).
    pub fn total_sectors(&self) -> u32 {
        if self.total_sectors_32 != 0 {
            self.total_sectors_32
        } else {
            self.total_sectors_16 as u32
        }
    }

    /// Size of a cluster in bytes.
    pub fn cluster_bytes(&self) -> usize {
        (self.sectors_per_cluster as usize) * (self.bytes_per_sector as usize)
    }

    /// Sector index where FAT #0 begins.
    pub fn fat_start_sector(&self) -> u64 {
        self.reserved_sectors as u64
    }

    /// Sector index where data clusters (starting from cluster 2) begin.
    pub fn data_start_sector(&self) -> u64 {
        (self.reserved_sectors as u64) + ((self.fats_count as u64) * (self.sectors_per_fat as u64))
    }

    /// Convert cluster number (>= 2) to absolute LBA sector on device.
    pub fn cluster_to_lba(&self, cluster: u32) -> u64 {
        if cluster < 2 {
            self.data_start_sector()
        } else {
            self.data_start_sector() + ((cluster as u64 - 2) * (self.sectors_per_cluster as u64))
        }
    }

    /// Total number of clusters in the data area.
    pub fn total_clusters(&self) -> u32 {
        let total_sec = self.total_sectors() as u64;
        let data_sec = total_sec.saturating_sub(self.data_start_sector());
        (data_sec / (self.sectors_per_cluster as u64)) as u32
    }
}

/// Short 8.3 Directory Entry (32 bytes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawDirEntry {
    pub name: [u8; 11],
    pub attr: u8,
    pub nt_reserved: u8,
    pub crt_time_tenth: u8,
    pub crt_time: u16,
    pub crt_date: u16,
    pub lst_acc_date: u16,
    pub fst_clus_hi: u16,
    pub wrt_time: u16,
    pub wrt_date: u16,
    pub fst_clus_lo: u16,
    pub file_size: u32,
}

impl RawDirEntry {
    pub fn parse(buf: &[u8]) -> Self {
        let mut name = [0u8; 11];
        name.copy_from_slice(&buf[0..11]);
        Self {
            name,
            attr: buf[11],
            nt_reserved: buf[12],
            crt_time_tenth: buf[13],
            crt_time: u16::from_le_bytes([buf[14], buf[15]]),
            crt_date: u16::from_le_bytes([buf[16], buf[17]]),
            lst_acc_date: u16::from_le_bytes([buf[18], buf[19]]),
            fst_clus_hi: u16::from_le_bytes([buf[20], buf[21]]),
            wrt_time: u16::from_le_bytes([buf[22], buf[23]]),
            wrt_date: u16::from_le_bytes([buf[24], buf[25]]),
            fst_clus_lo: u16::from_le_bytes([buf[26], buf[27]]),
            file_size: u32::from_le_bytes([buf[28], buf[29], buf[30], buf[31]]),
        }
    }

    pub fn serialize(&self) -> [u8; 32] {
        let mut buf = [0u8; 32];
        buf[0..11].copy_from_slice(&self.name);
        buf[11] = self.attr;
        buf[12] = self.nt_reserved;
        buf[13] = self.crt_time_tenth;
        buf[14..16].copy_from_slice(&self.crt_time.to_le_bytes());
        buf[16..18].copy_from_slice(&self.crt_date.to_le_bytes());
        buf[18..20].copy_from_slice(&self.lst_acc_date.to_le_bytes());
        buf[20..22].copy_from_slice(&self.fst_clus_hi.to_le_bytes());
        buf[22..24].copy_from_slice(&self.wrt_time.to_le_bytes());
        buf[24..26].copy_from_slice(&self.wrt_date.to_le_bytes());
        buf[26..28].copy_from_slice(&self.fst_clus_lo.to_le_bytes());
        buf[28..32].copy_from_slice(&self.file_size.to_le_bytes());
        buf
    }

    pub fn is_end(&self) -> bool {
        self.name[0] == 0x00
    }

    pub fn is_deleted(&self) -> bool {
        self.name[0] == 0xE5
    }

    pub fn is_lfn(&self) -> bool {
        self.attr == fat_attrs::LFN
    }

    pub fn is_directory(&self) -> bool {
        (self.attr & fat_attrs::DIRECTORY) != 0
    }

    pub fn first_cluster(&self) -> u32 {
        ((self.fst_clus_hi as u32) << 16) | (self.fst_clus_lo as u32)
    }

    pub fn set_first_cluster(&mut self, cluster: u32) {
        self.fst_clus_hi = ((cluster >> 16) & 0xFFFF) as u16;
        self.fst_clus_lo = (cluster & 0xFFFF) as u16;
    }

    /// Format standard 8.3 name to string.
    pub fn format_short_name(&self) -> String {
        let mut raw_name = self.name;
        if raw_name[0] == 0x05 {
            raw_name[0] = 0xE5;
        }

        let base_bytes = &raw_name[0..8];
        let ext_bytes = &raw_name[8..11];

        let base = String::from_utf8_lossy(base_bytes).trim_end().to_string();
        let ext = String::from_utf8_lossy(ext_bytes).trim_end().to_string();

        if ext.is_empty() {
            base
        } else {
            format!("{}.{}", base, ext)
        }
    }

    /// Compute 8-bit checksum of short name required by LFN entries.
    pub fn lfn_checksum(&self) -> u8 {
        let mut sum = 0u8;
        for &b in &self.name {
            sum = ((sum & 1) << 7).wrapping_add(sum >> 1).wrapping_add(b);
        }
        sum
    }

    /// Create an 11-byte short 8.3 name array from an arbitrary name.
    pub fn make_short_name(name: &str) -> [u8; 11] {
        let mut short_name = [b' '; 11];
        let clean = name.trim();
        let parts: Vec<&str> = clean.split('.').collect();

        let base = parts[0];
        let ext = if parts.len() > 1 { parts[parts.len() - 1] } else { "" };

        let mut idx = 0;
        for ch in base.chars() {
            if idx >= 8 {
                break;
            }
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                short_name[idx] = ch.to_ascii_uppercase() as u8;
                idx += 1;
            }
        }

        let mut e_idx = 0;
        for ch in ext.chars() {
            if e_idx >= 3 {
                break;
            }
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                short_name[8 + e_idx] = ch.to_ascii_uppercase() as u8;
                e_idx += 1;
            }
        }

        short_name
    }
}

/// Parsed Long File Name (LFN) Entry (32 bytes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LfnEntry {
    pub order: u8,
    pub name1: [u16; 5],
    pub attr: u8,
    pub lfn_type: u8,
    pub checksum: u8,
    pub name2: [u16; 6],
    pub fst_clus_lo: u16,
    pub name3: [u16; 2],
}

impl LfnEntry {
    pub fn parse(buf: &[u8]) -> Self {
        let order = buf[0];
        let mut name1 = [0u16; 5];
        for i in 0..5 {
            name1[i] = u16::from_le_bytes([buf[1 + i * 2], buf[2 + i * 2]]);
        }
        let attr = buf[11];
        let lfn_type = buf[12];
        let checksum = buf[13];
        let mut name2 = [0u16; 6];
        for i in 0..6 {
            name2[i] = u16::from_le_bytes([buf[14 + i * 2], buf[15 + i * 2]]);
        }
        let fst_clus_lo = u16::from_le_bytes([buf[26], buf[27]]);
        let mut name3 = [0u16; 2];
        for i in 0..2 {
            name3[i] = u16::from_le_bytes([buf[28 + i * 2], buf[29 + i * 2]]);
        }

        Self {
            order,
            name1,
            attr,
            lfn_type,
            checksum,
            name2,
            fst_clus_lo,
            name3,
        }
    }

    pub fn serialize(&self) -> [u8; 32] {
        let mut buf = [0u8; 32];
        buf[0] = self.order;
        for i in 0..5 {
            buf[1 + i * 2..3 + i * 2].copy_from_slice(&self.name1[i].to_le_bytes());
        }
        buf[11] = self.attr;
        buf[12] = self.lfn_type;
        buf[13] = self.checksum;
        for i in 0..6 {
            buf[14 + i * 2..16 + i * 2].copy_from_slice(&self.name2[i].to_le_bytes());
        }
        buf[26..28].copy_from_slice(&self.fst_clus_lo.to_le_bytes());
        for i in 0..2 {
            buf[28 + i * 2..30 + i * 2].copy_from_slice(&self.name3[i].to_le_bytes());
        }
        buf
    }

    pub fn sequence_number(&self) -> u8 {
        self.order & 0x1F
    }

    pub fn is_last(&self) -> bool {
        (self.order & 0x40) != 0
    }

    pub fn collect_characters(&self) -> [u16; 13] {
        let mut chars = [0u16; 13];
        chars[0..5].copy_from_slice(&self.name1);
        chars[5..11].copy_from_slice(&self.name2);
        chars[11..13].copy_from_slice(&self.name3);
        chars
    }
}

/// High-level Directory Entry representation in FAT32.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fat32DirEntry {
    pub name: String,
    pub short_name: String,
    pub attributes: u8,
    pub first_cluster: u32,
    pub file_size: u32,
    pub is_directory: bool,
    /// Offset of the short entry (in bytes) within the directory cluster chain
    pub dir_entry_offset: u64,
}

impl Fat32DirEntry {
    pub fn is_readonly(&self) -> bool {
        (self.attributes & fat_attrs::READ_ONLY) != 0
    }

    pub fn is_hidden(&self) -> bool {
        (self.attributes & fat_attrs::HIDDEN) != 0
    }

    pub fn is_system(&self) -> bool {
        (self.attributes & fat_attrs::SYSTEM) != 0
    }
}

/// Full FAT32 Filesystem Engine.
pub struct Fat32FileSystem {
    device: Arc<dyn BlockDevice>,
    bpb: Fat32Bpb,
    fat_lock: Mutex<()>,
}

impl Fat32FileSystem {
    /// Mount an existing FAT32 filesystem from a block device.
    pub fn mount(device: Arc<dyn BlockDevice>) -> Result<Self, StorageError> {
        let mut boot_sector = vec![0u8; 512];
        device.read_sectors(0, &mut boot_sector)?;
        let bpb = Fat32Bpb::parse(&boot_sector)?;

        Ok(Self {
            device,
            bpb,
            fat_lock: Mutex::new(()),
        })
    }

    pub fn bpb(&self) -> &Fat32Bpb {
        &self.bpb
    }

    pub fn device(&self) -> &Arc<dyn BlockDevice> {
        &self.device
    }

    /// Read next cluster pointer from FAT table.
    pub fn get_next_cluster(&self, cluster: u32) -> Result<u32, StorageError> {
        let _guard = self.fat_lock.lock().unwrap();
        self.read_fat_entry_unlocked(cluster)
    }

    fn read_fat_entry_unlocked(&self, cluster: u32) -> Result<u32, StorageError> {
        let fat_offset = (cluster as u64) * 4;
        let sector_size = self.bpb.bytes_per_sector as u64;
        let fat_sector = self.bpb.fat_start_sector() + (fat_offset / sector_size);
        let offset_in_sector = (fat_offset % sector_size) as usize;

        let mut sector_buf = vec![0u8; self.bpb.bytes_per_sector as usize];
        self.device.read_sectors(fat_sector, &mut sector_buf)?;

        let raw_entry = u32::from_le_bytes([
            sector_buf[offset_in_sector],
            sector_buf[offset_in_sector + 1],
            sector_buf[offset_in_sector + 2],
            sector_buf[offset_in_sector + 3],
        ]);

        Ok(raw_entry & FAT32_CLUSTER_MASK)
    }

    /// Write next cluster pointer to FAT table across all FAT copies.
    pub fn set_next_cluster(&self, cluster: u32, next_cluster: u32) -> Result<(), StorageError> {
        let _guard = self.fat_lock.lock().unwrap();
        self.write_fat_entry_unlocked(cluster, next_cluster)
    }

    fn write_fat_entry_unlocked(&self, cluster: u32, next_cluster: u32) -> Result<(), StorageError> {
        let fat_offset = (cluster as u64) * 4;
        let sector_size = self.bpb.bytes_per_sector as u64;
        let rel_sector = fat_offset / sector_size;
        let offset_in_sector = (fat_offset % sector_size) as usize;

        let mut sector_buf = vec![0u8; self.bpb.bytes_per_sector as usize];

        // Update each FAT mirror copy
        for fat_idx in 0..self.bpb.fats_count {
            let fat_start = self.bpb.fat_start_sector() + (fat_idx as u64 * self.bpb.sectors_per_fat as u64);
            let target_sector = fat_start + rel_sector;

            self.device.read_sectors(target_sector, &mut sector_buf)?;

            // Preserve upper 4 bits
            let old_entry = u32::from_le_bytes([
                sector_buf[offset_in_sector],
                sector_buf[offset_in_sector + 1],
                sector_buf[offset_in_sector + 2],
                sector_buf[offset_in_sector + 3],
            ]);
            let new_entry = (old_entry & 0xF000_0000) | (next_cluster & FAT32_CLUSTER_MASK);

            sector_buf[offset_in_sector..offset_in_sector + 4].copy_from_slice(&new_entry.to_le_bytes());
            self.device.write_sectors(target_sector, &sector_buf)?;
        }

        Ok(())
    }

    /// Allocate a fresh, zeroed cluster from the FAT.
    pub fn allocate_cluster(&self, prev_cluster: Option<u32>) -> Result<u32, StorageError> {
        let _guard = self.fat_lock.lock().unwrap();
        let total_clusters = self.bpb.total_clusters();

        let mut allocated = None;
        for c in 2..(total_clusters + 2) {
            let entry = self.read_fat_entry_unlocked(c)?;
            if entry == FAT32_FREE_CLUSTER {
                allocated = Some(c);
                break;
            }
        }

        let new_cluster = allocated.ok_or(StorageError::NoSpace)?;

        // Mark as EOF
        self.write_fat_entry_unlocked(new_cluster, FAT32_EOF_STANDARD)?;

        // Link previous cluster if present
        if let Some(prev) = prev_cluster {
            self.write_fat_entry_unlocked(prev, new_cluster)?;
        }

        // Zero out data in the newly allocated cluster
        let cluster_bytes = self.bpb.cluster_bytes();
        let zero_buf = vec![0u8; cluster_bytes];
        let lba = self.bpb.cluster_to_lba(new_cluster);
        self.device.write_sectors(lba, &zero_buf)?;

        Ok(new_cluster)
    }

    /// Free a cluster chain starting at `start_cluster`.
    pub fn free_cluster_chain(&self, start_cluster: u32) -> Result<(), StorageError> {
        if start_cluster < 2 {
            return Ok(());
        }

        let _guard = self.fat_lock.lock().unwrap();
        let mut curr = start_cluster;

        while curr >= 2 && curr < FAT32_EOF_MIN {
            let next = self.read_fat_entry_unlocked(curr)?;
            self.write_fat_entry_unlocked(curr, FAT32_FREE_CLUSTER)?;
            curr = next;
        }

        Ok(())
    }

    /// Get all cluster IDs belonging to a chain.
    pub fn get_cluster_chain(&self, start_cluster: u32) -> Result<Vec<u32>, StorageError> {
        if start_cluster < 2 {
            return Ok(Vec::new());
        }

        let _guard = self.fat_lock.lock().unwrap();
        let mut chain = Vec::new();
        let mut curr = start_cluster;
        let mut hops = 0;
        let max_hops = self.bpb.total_clusters() as usize + 2;

        while curr >= 2 && curr < FAT32_EOF_MIN {
            chain.push(curr);
            hops += 1;
            if hops > max_hops {
                return Err(StorageError::InvalidData("Cluster chain loop detected"));
            }
            curr = self.read_fat_entry_unlocked(curr)?;
        }

        Ok(chain)
    }

    /// Read raw bytes of an entire cluster chain.
    pub fn read_cluster_chain(&self, start_cluster: u32) -> Result<Vec<u8>, StorageError> {
        let chain = self.get_cluster_chain(start_cluster)?;
        let cluster_bytes = self.bpb.cluster_bytes();
        let mut buf = vec![0u8; chain.len() * cluster_bytes];

        for (i, &cluster) in chain.iter().enumerate() {
            let lba = self.bpb.cluster_to_lba(cluster);
            let offset = i * cluster_bytes;
            self.device.read_sectors(lba, &mut buf[offset..offset + cluster_bytes])?;
        }

        Ok(buf)
    }

    /// Read directory entries from a directory's starting cluster.
    pub fn readdir(&self, dir_cluster: u32) -> Result<Vec<Fat32DirEntry>, StorageError> {
        let dir_data = self.read_cluster_chain(dir_cluster)?;
        let mut entries = Vec::new();
        let mut lfn_parts: Vec<(u8, [u16; 13])> = Vec::new();
        let mut expected_checksum: Option<u8> = None;

        let num_records = dir_data.len() / 32;

        for i in 0..num_records {
            let offset = i * 32;
            let record_bytes = &dir_data[offset..offset + 32];
            let raw = RawDirEntry::parse(record_bytes);

            if raw.is_end() {
                break; // End of directory entries
            }

            if raw.is_deleted() {
                lfn_parts.clear();
                expected_checksum = None;
                continue;
            }

            if raw.is_lfn() {
                let lfn = LfnEntry::parse(record_bytes);
                if lfn.is_last() {
                    lfn_parts.clear();
                    expected_checksum = Some(lfn.checksum);
                }
                lfn_parts.push((lfn.sequence_number(), lfn.collect_characters()));
                continue;
            }

            // Standard 8.3 entry encountered
            // Check if Volume ID (skip volume label entry)
            if (raw.attr & fat_attrs::VOLUME_ID) != 0 && (raw.attr & fat_attrs::DIRECTORY) == 0 {
                lfn_parts.clear();
                expected_checksum = None;
                continue;
            }

            let short_name = raw.format_short_name();

            let name = if let Some(csum) = expected_checksum {
                if csum == raw.lfn_checksum() && !lfn_parts.is_empty() {
                    // Sort LFN parts by sequence number ascending (1, 2, ...)
                    lfn_parts.sort_by_key(|&(seq, _)| seq);
                    let mut utf16_units = Vec::new();
                    for &(_, chars) in &lfn_parts {
                        for &u in &chars {
                            if u == 0 || u == 0xFFFF {
                                break;
                            }
                            utf16_units.push(u);
                        }
                    }
                    String::from_utf16_lossy(&utf16_units)
                } else {
                    short_name.clone()
                }
            } else {
                short_name.clone()
            };

            lfn_parts.clear();
            expected_checksum = None;

            entries.push(Fat32DirEntry {
                name,
                short_name,
                attributes: raw.attr,
                first_cluster: raw.first_cluster(),
                file_size: raw.file_size,
                is_directory: raw.is_directory(),
                dir_entry_offset: offset as u64,
            });
        }

        Ok(entries)
    }

    /// Read file content at a given offset into a buffer.
    pub fn read_file_at(
        &self,
        first_cluster: u32,
        offset: u64,
        buf: &mut [u8],
        file_size: u64,
    ) -> Result<usize, StorageError> {
        if offset >= file_size || buf.is_empty() {
            return Ok(0);
        }

        let max_readable = min(buf.len() as u64, file_size - offset) as usize;
        let chain = self.get_cluster_chain(first_cluster)?;
        let cluster_bytes = self.bpb.cluster_bytes() as u64;

        let mut bytes_read = 0usize;
        let mut curr_offset = offset;

        while bytes_read < max_readable {
            let cluster_idx = (curr_offset / cluster_bytes) as usize;
            if cluster_idx >= chain.len() {
                break;
            }

            let cluster_num = chain[cluster_idx];
            let offset_in_cluster = (curr_offset % cluster_bytes) as usize;
            let available_in_cluster = (cluster_bytes as usize) - offset_in_cluster;
            let to_read = min(max_readable - bytes_read, available_in_cluster);

            let lba = self.bpb.cluster_to_lba(cluster_num);
            let mut cluster_buf = vec![0u8; cluster_bytes as usize];
            self.device.read_sectors(lba, &mut cluster_buf)?;

            buf[bytes_read..bytes_read + to_read]
                .copy_from_slice(&cluster_buf[offset_in_cluster..offset_in_cluster + to_read]);

            bytes_read += to_read;
            curr_offset += to_read as u64;
        }

        Ok(bytes_read)
    }

    /// Read entire file content into a Vec.
    pub fn read_file(&self, first_cluster: u32, file_size: u64) -> Result<Vec<u8>, StorageError> {
        let mut buf = vec![0u8; file_size as usize];
        let n = self.read_file_at(first_cluster, 0, &mut buf, file_size)?;
        buf.truncate(n);
        Ok(buf)
    }

    /// Write data to an existing or empty file cluster chain, allocating clusters as needed.
    pub fn write_file_at(
        &self,
        first_cluster: u32,
        offset: u64,
        data: &[u8],
        current_file_size: u64,
    ) -> Result<(u32, u64), StorageError> {
        let cluster_bytes = self.bpb.cluster_bytes() as u64;
        let end_offset = offset + data.len() as u64;

        let mut actual_first_cluster = first_cluster;
        if actual_first_cluster < 2 {
            // Allocate initial cluster
            actual_first_cluster = self.allocate_cluster(None)?;
        }

        let mut chain = self.get_cluster_chain(actual_first_cluster)?;

        // Calculate how many clusters are needed to hold end_offset
        let needed_clusters = ((end_offset + cluster_bytes - 1) / cluster_bytes) as usize;
        let needed_clusters = if needed_clusters == 0 { 1 } else { needed_clusters };

        while chain.len() < needed_clusters {
            let last_cluster = *chain.last().unwrap();
            let new_cluster = self.allocate_cluster(Some(last_cluster))?;
            chain.push(new_cluster);
        }

        // Write data into clusters
        let mut bytes_written = 0usize;
        let mut curr_offset = offset;

        while bytes_written < data.len() {
            let cluster_idx = (curr_offset / cluster_bytes) as usize;
            let cluster_num = chain[cluster_idx];
            let offset_in_cluster = (curr_offset % cluster_bytes) as usize;
            let to_write = min(data.len() - bytes_written, (cluster_bytes as usize) - offset_in_cluster);

            let lba = self.bpb.cluster_to_lba(cluster_num);
            let mut cluster_buf = vec![0u8; cluster_bytes as usize];
            self.device.read_sectors(lba, &mut cluster_buf)?;

            cluster_buf[offset_in_cluster..offset_in_cluster + to_write]
                .copy_from_slice(&data[bytes_written..bytes_written + to_write]);

            self.device.write_sectors(lba, &cluster_buf)?;

            bytes_written += to_write;
            curr_offset += to_write as u64;
        }

        let new_file_size = current_file_size.max(end_offset);
        Ok((actual_first_cluster, new_file_size))
    }

    /// Create a new file entry in a parent directory.
    pub fn create_file(
        &self,
        dir_cluster: u32,
        name: &str,
        attributes: u8,
    ) -> Result<Fat32DirEntry, StorageError> {
        // Check if file already exists
        let existing = self.readdir(dir_cluster)?;
        if existing.iter().any(|e| e.name.eq_ignore_ascii_case(name)) {
            return Err(StorageError::AlreadyExists);
        }

        let short_name_bytes = RawDirEntry::make_short_name(name);
        let raw = RawDirEntry {
            name: short_name_bytes,
            attr: attributes & !fat_attrs::DIRECTORY,
            nt_reserved: 0,
            crt_time_tenth: 0,
            crt_time: 0x4800, // 09:00:00
            crt_date: 0x5421, // 2022-01-01
            lst_acc_date: 0x5421,
            fst_clus_hi: 0,
            wrt_time: 0x4800,
            wrt_date: 0x5421,
            fst_clus_lo: 0,
            file_size: 0,
        };

        // If long name, prepare LFN entries
        let is_pure_short = name.len() <= 12 && name.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '.' || c == '_');
        let lfn_entries = if !is_pure_short {
            Self::create_lfn_entries(name, raw.lfn_checksum())
        } else {
            Vec::new()
        };

        let total_slots = lfn_entries.len() + 1;
        let entry_offset = self.allocate_dir_slots(dir_cluster, total_slots)?;

        // Write LFN entries followed by short entry
        let dir_chain = self.get_cluster_chain(dir_cluster)?;

        for (idx, lfn) in lfn_entries.iter().enumerate() {
            let slot_offset = entry_offset + (idx as u64 * 32);
            self.write_dir_record(&dir_chain, slot_offset, &lfn.serialize())?;
        }

        let short_offset = entry_offset + (lfn_entries.len() as u64 * 32);
        self.write_dir_record(&dir_chain, short_offset, &raw.serialize())?;

        Ok(Fat32DirEntry {
            name: name.to_string(),
            short_name: raw.format_short_name(),
            attributes: raw.attr,
            first_cluster: 0,
            file_size: 0,
            is_directory: false,
            dir_entry_offset: short_offset,
        })
    }

    /// Create a new directory within a parent directory.
    pub fn mkdir(&self, dir_cluster: u32, name: &str) -> Result<Fat32DirEntry, StorageError> {
        let existing = self.readdir(dir_cluster)?;
        if existing.iter().any(|e| e.name.eq_ignore_ascii_case(name)) {
            return Err(StorageError::AlreadyExists);
        }

        // 1. Allocate a cluster for the new directory
        let new_dir_cluster = self.allocate_cluster(None)?;

        // 2. Initialize new directory with "." and ".." entries
        let mut dot_entry = RawDirEntry {
            name: *b".          ",
            attr: fat_attrs::DIRECTORY,
            nt_reserved: 0,
            crt_time_tenth: 0,
            crt_time: 0x4800,
            crt_date: 0x5421,
            lst_acc_date: 0x5421,
            fst_clus_hi: 0,
            wrt_time: 0x4800,
            wrt_date: 0x5421,
            fst_clus_lo: 0,
            file_size: 0,
        };
        dot_entry.set_first_cluster(new_dir_cluster);

        let mut dotdot_entry = RawDirEntry {
            name: *b"..         ",
            attr: fat_attrs::DIRECTORY,
            nt_reserved: 0,
            crt_time_tenth: 0,
            crt_time: 0x4800,
            crt_date: 0x5421,
            lst_acc_date: 0x5421,
            fst_clus_hi: 0,
            wrt_time: 0x4800,
            wrt_date: 0x5421,
            fst_clus_lo: 0,
            file_size: 0,
        };
        let parent_ptr = if dir_cluster == self.bpb.root_cluster { 0 } else { dir_cluster };
        dotdot_entry.set_first_cluster(parent_ptr);

        let cluster_bytes = self.bpb.cluster_bytes();
        let mut dir_init = vec![0u8; cluster_bytes];
        dir_init[0..32].copy_from_slice(&dot_entry.serialize());
        dir_init[32..64].copy_from_slice(&dotdot_entry.serialize());

        let lba = self.bpb.cluster_to_lba(new_dir_cluster);
        self.device.write_sectors(lba, &dir_init)?;

        // 3. Create entry in parent directory
        let short_name_bytes = RawDirEntry::make_short_name(name);
        let mut raw = RawDirEntry {
            name: short_name_bytes,
            attr: fat_attrs::DIRECTORY,
            nt_reserved: 0,
            crt_time_tenth: 0,
            crt_time: 0x4800,
            crt_date: 0x5421,
            lst_acc_date: 0x5421,
            fst_clus_hi: 0,
            wrt_time: 0x4800,
            wrt_date: 0x5421,
            fst_clus_lo: 0,
            file_size: 0,
        };
        raw.set_first_cluster(new_dir_cluster);

        let is_pure_short = name.len() <= 12 && name.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '.' || c == '_');
        let lfn_entries = if !is_pure_short {
            Self::create_lfn_entries(name, raw.lfn_checksum())
        } else {
            Vec::new()
        };

        let total_slots = lfn_entries.len() + 1;
        let entry_offset = self.allocate_dir_slots(dir_cluster, total_slots)?;

        let dir_chain = self.get_cluster_chain(dir_cluster)?;
        for (idx, lfn) in lfn_entries.iter().enumerate() {
            let slot_offset = entry_offset + (idx as u64 * 32);
            self.write_dir_record(&dir_chain, slot_offset, &lfn.serialize())?;
        }

        let short_offset = entry_offset + (lfn_entries.len() as u64 * 32);
        self.write_dir_record(&dir_chain, short_offset, &raw.serialize())?;

        Ok(Fat32DirEntry {
            name: name.to_string(),
            short_name: raw.format_short_name(),
            attributes: raw.attr,
            first_cluster: new_dir_cluster,
            file_size: 0,
            is_directory: true,
            dir_entry_offset: short_offset,
        })
    }

    /// Update short directory entry with updated size and first cluster.
    pub fn update_entry_metadata(
        &self,
        dir_cluster: u32,
        entry_offset: u64,
        first_cluster: u32,
        file_size: u32,
    ) -> Result<(), StorageError> {
        let dir_chain = self.get_cluster_chain(dir_cluster)?;
        let mut record_buf = [0u8; 32];
        self.read_dir_record(&dir_chain, entry_offset, &mut record_buf)?;

        let mut raw = RawDirEntry::parse(&record_buf);
        raw.set_first_cluster(first_cluster);
        raw.file_size = file_size;

        self.write_dir_record(&dir_chain, entry_offset, &raw.serialize())?;
        Ok(())
    }

    /// Mark an entry as deleted (0xE5).
    pub fn unlink(&self, dir_cluster: u32, name: &str) -> Result<(), StorageError> {
        let entries = self.readdir(dir_cluster)?;
        let target = entries
            .into_iter()
            .find(|e| e.name.eq_ignore_ascii_case(name))
            .ok_or(StorageError::NotFound)?;

        if target.is_directory {
            // Verify empty (only . and ..)
            let sub_entries = self.readdir(target.first_cluster)?;
            if sub_entries.len() > 2 {
                return Err(StorageError::DirectoryNotEmpty);
            }
            self.free_cluster_chain(target.first_cluster)?;
        } else if target.first_cluster >= 2 {
            self.free_cluster_chain(target.first_cluster)?;
        }

        // Mark short entry (and preceding LFN entries) deleted
        let dir_chain = self.get_cluster_chain(dir_cluster)?;
        let mut record_buf = [0u8; 32];
        self.read_dir_record(&dir_chain, target.dir_entry_offset, &mut record_buf)?;
        record_buf[0] = 0xE5;
        self.write_dir_record(&dir_chain, target.dir_entry_offset, &record_buf)?;

        Ok(())
    }

    // Helper functions for directory slot allocation and I/O

    fn allocate_dir_slots(&self, dir_cluster: u32, count: usize) -> Result<u64, StorageError> {
        let mut dir_chain = self.get_cluster_chain(dir_cluster)?;
        let dir_data = self.read_cluster_chain(dir_cluster)?;

        let num_slots = dir_data.len() / 32;
        let mut consecutive_free = 0usize;
        let mut start_idx = 0usize;

        for i in 0..num_slots {
            let offset = i * 32;
            let first_byte = dir_data[offset];
            if first_byte == 0x00 || first_byte == 0xE5 {
                if consecutive_free == 0 {
                    start_idx = i;
                }
                consecutive_free += 1;
                if consecutive_free == count {
                    return Ok((start_idx * 32) as u64);
                }
            } else {
                consecutive_free = 0;
            }
        }

        // Need to expand directory by allocating a new cluster
        let last_cluster = *dir_chain.last().unwrap();
        let new_cluster = self.allocate_cluster(Some(last_cluster))?;
        dir_chain.push(new_cluster);

        // Return starting offset in the newly expanded area
        let expanded_offset = (num_slots * 32) as u64;
        Ok(expanded_offset)
    }

    fn read_dir_record(&self, dir_chain: &[u32], byte_offset: u64, buf: &mut [u8; 32]) -> Result<(), StorageError> {
        let cluster_bytes = self.bpb.cluster_bytes() as u64;
        let cluster_idx = (byte_offset / cluster_bytes) as usize;
        let offset_in_cluster = (byte_offset % cluster_bytes) as usize;

        let cluster_num = dir_chain[cluster_idx];
        let lba = self.bpb.cluster_to_lba(cluster_num);
        let mut cluster_buf = vec![0u8; cluster_bytes as usize];
        self.device.read_sectors(lba, &mut cluster_buf)?;

        buf.copy_from_slice(&cluster_buf[offset_in_cluster..offset_in_cluster + 32]);
        Ok(())
    }

    fn write_dir_record(&self, dir_chain: &[u32], byte_offset: u64, buf: &[u8; 32]) -> Result<(), StorageError> {
        let cluster_bytes = self.bpb.cluster_bytes() as u64;
        let cluster_idx = (byte_offset / cluster_bytes) as usize;
        let offset_in_cluster = (byte_offset % cluster_bytes) as usize;

        let cluster_num = dir_chain[cluster_idx];
        let lba = self.bpb.cluster_to_lba(cluster_num);
        let mut cluster_buf = vec![0u8; cluster_bytes as usize];
        self.device.read_sectors(lba, &mut cluster_buf)?;

        cluster_buf[offset_in_cluster..offset_in_cluster + 32].copy_from_slice(buf);
        self.device.write_sectors(lba, &cluster_buf)?;
        Ok(())
    }

    fn create_lfn_entries(long_name: &str, checksum: u8) -> Vec<LfnEntry> {
        let mut utf16: Vec<u16> = long_name.encode_utf16().collect();
        // Pad with null terminator and 0xFFFF
        utf16.push(0);
        while utf16.len() % 13 != 0 {
            utf16.push(0xFFFF);
        }

        let num_entries = utf16.len() / 13;
        let mut entries = Vec::with_capacity(num_entries);

        // LFN entries are written in reverse order (N down to 1)
        for seq in (1..=num_entries).rev() {
            let chunk_idx = seq - 1;
            let chunk = &utf16[chunk_idx * 13..(chunk_idx + 1) * 13];

            let order = if seq == num_entries { (seq as u8) | 0x40 } else { seq as u8 };
            let mut name1 = [0u16; 5];
            name1.copy_from_slice(&chunk[0..5]);
            let mut name2 = [0u16; 6];
            name2.copy_from_slice(&chunk[5..11]);
            let mut name3 = [0u16; 2];
            name3.copy_from_slice(&chunk[11..13]);

            entries.push(LfnEntry {
                order,
                name1,
                attr: fat_attrs::LFN,
                lfn_type: 0,
                checksum,
                name2,
                fst_clus_lo: 0,
                name3,
            });
        }

        entries
    }

    /// Format a block device as a fresh, standard FAT32 volume.
    pub fn format(device: Arc<dyn BlockDevice>, volume_label: &str) -> Result<Self, StorageError> {
        let total_sectors = device.total_sectors() as u32;
        if total_sectors < 65536 {
            return Err(StorageError::InvalidData("Device is too small for FAT32 (minimum ~33MB / 65536 sectors)"));
        }

        let bytes_per_sector = 512u16;
        let sectors_per_cluster = 8u8; // 4KB cluster size
        let reserved_sectors = 32u16;
        let fats_count = 2u8;

        // Approximate FAT size calculation
        // Each cluster entry is 4 bytes.
        let approx_clusters = total_sectors / (sectors_per_cluster as u32);
        let fat_bytes = (approx_clusters + 2) * 4;
        let sectors_per_fat = ((fat_bytes + (bytes_per_sector as u32) - 1) / (bytes_per_sector as u32)) + 16;

        let bpb = Fat32Bpb {
            bytes_per_sector,
            sectors_per_cluster,
            reserved_sectors,
            fats_count,
            root_ent_cnt: 0,
            total_sectors_16: 0,
            media_type: 0xF8,
            fat_sz_16: 0,
            sectors_per_track: 63,
            heads_count: 255,
            hidden_sectors: 0,
            total_sectors_32: total_sectors,
            sectors_per_fat,
            ext_flags: 0,
            fs_version: 0,
            root_cluster: 2,
            fs_info_sector: 1,
            backup_boot_sector: 6,
            drive_number: 0x80,
            boot_signature: 0x29,
            volume_id: 0x1984_2026,
            volume_label: volume_label.to_string(),
            fs_type: "FAT32".to_string(),
        };

        // 1. Write Sector 0 (Boot Sector BPB) and Sector 6 (Backup)
        let boot_bytes = bpb.serialize();
        device.write_sectors(0, &boot_bytes)?;
        device.write_sectors(6, &boot_bytes)?;

        // 2. Initialize FAT1 and FAT2
        let mut fat_sector = vec![0u8; bytes_per_sector as usize];
        // Cluster 0: media type with upper bits set (0x0FFFFF00 | 0xF8 = 0x0FFFFFF8)
        fat_sector[0..4].copy_from_slice(&0x0FFF_FFF8u32.to_le_bytes());
        // Cluster 1: EOF marker (0x0FFFFFFF)
        fat_sector[4..8].copy_from_slice(&0x0FFF_FFFFu32.to_le_bytes());
        // Cluster 2: Root directory EOF marker (0x0FFFFFFF)
        fat_sector[8..12].copy_from_slice(&0x0FFF_FFFFu32.to_le_bytes());

        let fat0_lba = bpb.fat_start_sector();
        device.write_sectors(fat0_lba, &fat_sector)?;

        let fat1_lba = fat0_lba + (sectors_per_fat as u64);
        device.write_sectors(fat1_lba, &fat_sector)?;

        // Zero out rest of first FAT sector for cleanliness
        fat_sector.fill(0);
        for s in 1..sectors_per_fat {
            device.write_sectors(fat0_lba + (s as u64), &fat_sector)?;
            device.write_sectors(fat1_lba + (s as u64), &fat_sector)?;
        }

        // 3. Initialize Root Directory Cluster 2 with zeros
        let root_lba = bpb.cluster_to_lba(2);
        let cluster_zero = vec![0u8; bpb.cluster_bytes()];
        device.write_sectors(root_lba, &cluster_zero)?;

        Ok(Self {
            device,
            bpb,
            fat_lock: Mutex::new(()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::RamDisk;

    #[test]
    fn test_fat32_bpb_roundtrip() {
        let bpb = Fat32Bpb {
            bytes_per_sector: 512,
            sectors_per_cluster: 8,
            reserved_sectors: 32,
            fats_count: 2,
            root_ent_cnt: 0,
            total_sectors_16: 0,
            media_type: 0xF8,
            fat_sz_16: 0,
            sectors_per_track: 32,
            heads_count: 64,
            hidden_sectors: 0,
            total_sectors_32: 131072, // 64MB
            sectors_per_fat: 128,
            ext_flags: 0,
            fs_version: 0,
            root_cluster: 2,
            fs_info_sector: 1,
            backup_boot_sector: 6,
            drive_number: 0x80,
            boot_signature: 0x29,
            volume_id: 0xA1B2C3D4,
            volume_label: "SYSTEM_BOOT".to_string(),
            fs_type: "FAT32".to_string(),
        };

        let bytes = bpb.serialize();
        let parsed = Fat32Bpb::parse(&bytes).unwrap();
        assert_eq!(parsed.bytes_per_sector, 512);
        assert_eq!(parsed.sectors_per_cluster, 8);
        assert_eq!(parsed.root_cluster, 2);
        assert_eq!(parsed.volume_label, "SYSTEM_BOOT");
        assert_eq!(parsed.fs_type, "FAT32");
    }

    #[test]
    fn test_fat32_end_to_end_file_operations() {
        // Create 64MB RamDisk (131072 sectors)
        let ramdisk = Arc::new(RamDisk::new(131072));
        let fs = Fat32FileSystem::format(ramdisk.clone(), "HYBRIDOS_C").unwrap();

        // 1. Create a subdirectory
        let dir_entry = fs.mkdir(fs.bpb().root_cluster, "Windows").unwrap();
        assert!(dir_entry.is_directory);
        assert_eq!(dir_entry.name, "Windows");

        // 2. Create a file with long filename in "Windows"
        let file_entry = fs.create_file(dir_entry.first_cluster, "HybridKernelConfig.sys", 0).unwrap();
        assert_eq!(file_entry.name, "HybridKernelConfig.sys");

        // 3. Write data to the file
        let payload = b"HybridOS NT Core Configuration: Personality=Dual; VortexVDM=Active;\n";
        let (first_clus, new_sz) = fs.write_file_at(
            file_entry.first_cluster,
            0,
            payload,
            file_entry.file_size as u64,
        ).unwrap();
        fs.update_entry_metadata(dir_entry.first_cluster, file_entry.dir_entry_offset, first_clus, new_sz as u32).unwrap();

        // 4. Read back the file
        let read_back = fs.read_file(first_clus, new_sz).unwrap();
        assert_eq!(&read_back, payload);

        // 5. Read directory listing of "Windows"
        let list = fs.readdir(dir_entry.first_cluster).unwrap();
        assert!(list.iter().any(|e| e.name == "HybridKernelConfig.sys"));

        // 6. Delete file
        fs.unlink(dir_entry.first_cluster, "HybridKernelConfig.sys").unwrap();
        let list_after = fs.readdir(dir_entry.first_cluster).unwrap();
        assert!(!list_after.iter().any(|e| e.name == "HybridKernelConfig.sys"));
    }
}
