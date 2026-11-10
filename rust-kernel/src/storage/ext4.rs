//! Core Ext4 Filesystem Driver for HybridOS.
//!
//! Features:
//! - Superblock parsing: magic (0xEF53), block sizes, inode count, group geometry.
//! - Block Group Descriptors (block bitmap, inode bitmap, inode table).
//! - Inode table traversal (mode, permissions, timestamps, sizes, flags).
//! - Extent Tree parsing (`ext4_extent_header`, `ext4_extent_idx`, `ext4_extent` block mappings).
//! - Directory entry parsing (`ext4_dir_entry_2`) and path resolution.
//! - File reading and directory listing (`readdir`).
//! - In-memory formatting support for provisioning disks and automated unit testing.

use std::cmp::min;
use std::sync::Arc;
use super::{BlockDevice, StorageError};

pub const EXT4_SUPERBLOCK_OFFSET: u64 = 1024;
pub const EXT4_SUPERBLOCK_MAGIC: u16 = 0xEF53;
pub const EXT4_ROOT_INO: u32 = 2;
pub const EXT4_EXTENTS_MAGIC: u16 = 0xF30A;
pub const EXT4_EXTENTS_FL: u32 = 0x0008_0000;

/// Ext4 File Types in `ext4_dir_entry_2`.
pub mod ext4_file_types {
    pub const EXT4_FT_UNKNOWN: u8 = 0;
    pub const EXT4_FT_REG_FILE: u8 = 1;
    pub const EXT4_FT_DIR: u8 = 2;
    pub const EXT4_FT_CHRDEV: u8 = 3;
    pub const EXT4_FT_BLKDEV: u8 = 4;
    pub const EXT4_FT_FIFO: u8 = 5;
    pub const EXT4_FT_SOCK: u8 = 6;
    pub const EXT4_FT_SYMLINK: u8 = 7;
}

/// Inode Mode bits (standard POSIX).
pub mod ext4_mode {
    pub const S_IFMT: u16 = 0xF000;
    pub const S_IFSOCK: u16 = 0xC000;
    pub const S_IFLNK: u16 = 0xA000;
    pub const S_IFREG: u16 = 0x8000;
    pub const S_IFBLK: u16 = 0x6000;
    pub const S_IFDIR: u16 = 0x4000;
    pub const S_IFCHR: u16 = 0x2000;
    pub const S_IFIFO: u16 = 0x1000;
}

/// Ext4 Superblock (1024 bytes at offset 1024).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ext4Superblock {
    pub s_inodes_count: u32,
    pub s_blocks_count_lo: u32,
    pub s_r_blocks_count_lo: u32,
    pub s_free_blocks_count_lo: u32,
    pub s_free_inodes_count: u32,
    pub s_first_data_block: u32,
    pub s_log_block_size: u32,
    pub s_log_cluster_size: u32,
    pub s_blocks_per_group: u32,
    pub s_clusters_per_group: u32,
    pub s_inodes_per_group: u32,
    pub s_magic: u16,
    pub s_state: u16,
    pub s_errors: u16,
    pub s_minor_rev_level: u16,
    pub s_lastcheck: u32,
    pub s_checkinterval: u32,
    pub s_creator_os: u32,
    pub s_rev_level: u32,
    pub s_first_ino: u32,
    pub s_inode_size: u16,
    pub s_block_group_nr: u16,
    pub s_feature_compat: u32,
    pub s_feature_incompat: u32,
    pub s_feature_ro_compat: u32,
    pub s_uuid: [u8; 16],
    pub s_volume_name: String,
    pub s_desc_size: u16,
    pub s_blocks_count_hi: u32,
    pub s_free_blocks_count_hi: u32,
}

impl Ext4Superblock {
    pub fn parse(buf: &[u8]) -> Result<Self, StorageError> {
        if buf.len() < 1024 {
            return Err(StorageError::InvalidData("Superblock buffer too small (must be 1024 bytes)"));
        }

        let magic = u16::from_le_bytes([buf[0x38], buf[0x39]]);
        if magic != EXT4_SUPERBLOCK_MAGIC {
            return Err(StorageError::InvalidData("Invalid ext4 superblock magic (expected 0xEF53)"));
        }

        let s_inodes_count = u32::from_le_bytes(buf[0x00..0x04].try_into().unwrap());
        let s_blocks_count_lo = u32::from_le_bytes(buf[0x04..0x08].try_into().unwrap());
        let s_r_blocks_count_lo = u32::from_le_bytes(buf[0x08..0x0C].try_into().unwrap());
        let s_free_blocks_count_lo = u32::from_le_bytes(buf[0x0C..0x10].try_into().unwrap());
        let s_free_inodes_count = u32::from_le_bytes(buf[0x10..0x14].try_into().unwrap());
        let s_first_data_block = u32::from_le_bytes(buf[0x14..0x18].try_into().unwrap());
        let s_log_block_size = u32::from_le_bytes(buf[0x18..0x1C].try_into().unwrap());
        let s_log_cluster_size = u32::from_le_bytes(buf[0x1C..0x20].try_into().unwrap());
        let s_blocks_per_group = u32::from_le_bytes(buf[0x20..0x24].try_into().unwrap());
        let s_clusters_per_group = u32::from_le_bytes(buf[0x24..0x28].try_into().unwrap());
        let s_inodes_per_group = u32::from_le_bytes(buf[0x28..0x2C].try_into().unwrap());
        let s_state = u16::from_le_bytes([buf[0x3A], buf[0x3B]]);
        let s_errors = u16::from_le_bytes([buf[0x3C], buf[0x3D]]);
        let s_minor_rev_level = u16::from_le_bytes([buf[0x3E], buf[0x3F]]);
        let s_lastcheck = u32::from_le_bytes(buf[0x40..0x44].try_into().unwrap());
        let s_checkinterval = u32::from_le_bytes(buf[0x44..0x48].try_into().unwrap());
        let s_creator_os = u32::from_le_bytes(buf[0x48..0x4C].try_into().unwrap());
        let s_rev_level = u32::from_le_bytes(buf[0x4C..0x50].try_into().unwrap());
        let s_first_ino = u32::from_le_bytes(buf[0x54..0x58].try_into().unwrap());
        let s_inode_size = u16::from_le_bytes([buf[0x58], buf[0x59]]);
        let s_block_group_nr = u16::from_le_bytes([buf[0x5A], buf[0x5B]]);
        let s_feature_compat = u32::from_le_bytes(buf[0x5C..0x60].try_into().unwrap());
        let s_feature_incompat = u32::from_le_bytes(buf[0x60..0x64].try_into().unwrap());
        let s_feature_ro_compat = u32::from_le_bytes(buf[0x64..0x68].try_into().unwrap());

        let mut s_uuid = [0u8; 16];
        s_uuid.copy_from_slice(&buf[0x68..0x78]);

        let s_volume_name = String::from_utf8_lossy(&buf[0x78..0x88]).trim_end_matches('\0').trim().to_string();

        let s_desc_size = u16::from_le_bytes([buf[0xFE], buf[0xFF]]);
        let desc_size = if s_desc_size == 0 { 32 } else { s_desc_size };

        let s_blocks_count_hi = u32::from_le_bytes(buf[0x150..0x154].try_into().unwrap());
        let s_free_blocks_count_hi = u32::from_le_bytes(buf[0x158..0x15C].try_into().unwrap());

        Ok(Self {
            s_inodes_count,
            s_blocks_count_lo,
            s_r_blocks_count_lo,
            s_free_blocks_count_lo,
            s_free_inodes_count,
            s_first_data_block,
            s_log_block_size,
            s_log_cluster_size,
            s_blocks_per_group,
            s_clusters_per_group,
            s_inodes_per_group,
            s_magic: magic,
            s_state,
            s_errors,
            s_minor_rev_level,
            s_lastcheck,
            s_checkinterval,
            s_creator_os,
            s_rev_level,
            s_first_ino,
            s_inode_size: if s_inode_size == 0 { 128 } else { s_inode_size },
            s_block_group_nr,
            s_feature_compat,
            s_feature_incompat,
            s_feature_ro_compat,
            s_uuid,
            s_volume_name,
            s_desc_size: desc_size,
            s_blocks_count_hi,
            s_free_blocks_count_hi,
        })
    }

    pub fn block_size(&self) -> usize {
        1024usize << self.s_log_block_size
    }

    pub fn total_blocks(&self) -> u64 {
        ((self.s_blocks_count_hi as u64) << 32) | (self.s_blocks_count_lo as u64)
    }

    pub fn block_groups_count(&self) -> u32 {
        let total_blocks = self.total_blocks() as u32;
        (total_blocks + self.s_blocks_per_group - 1) / self.s_blocks_per_group
    }

    pub fn serialize(&self) -> [u8; 1024] {
        let mut buf = [0u8; 1024];
        buf[0x00..0x04].copy_from_slice(&self.s_inodes_count.to_le_bytes());
        buf[0x04..0x08].copy_from_slice(&self.s_blocks_count_lo.to_le_bytes());
        buf[0x08..0x0C].copy_from_slice(&self.s_r_blocks_count_lo.to_le_bytes());
        buf[0x0C..0x10].copy_from_slice(&self.s_free_blocks_count_lo.to_le_bytes());
        buf[0x10..0x14].copy_from_slice(&self.s_free_inodes_count.to_le_bytes());
        buf[0x14..0x18].copy_from_slice(&self.s_first_data_block.to_le_bytes());
        buf[0x18..0x1C].copy_from_slice(&self.s_log_block_size.to_le_bytes());
        buf[0x1C..0x20].copy_from_slice(&self.s_log_cluster_size.to_le_bytes());
        buf[0x20..0x24].copy_from_slice(&self.s_blocks_per_group.to_le_bytes());
        buf[0x24..0x28].copy_from_slice(&self.s_clusters_per_group.to_le_bytes());
        buf[0x28..0x2C].copy_from_slice(&self.s_inodes_per_group.to_le_bytes());
        buf[0x38..0x3A].copy_from_slice(&self.s_magic.to_le_bytes());
        buf[0x3A..0x3C].copy_from_slice(&self.s_state.to_le_bytes());
        buf[0x3C..0x3E].copy_from_slice(&self.s_errors.to_le_bytes());
        buf[0x3E..0x40].copy_from_slice(&self.s_minor_rev_level.to_le_bytes());
        buf[0x40..0x44].copy_from_slice(&self.s_lastcheck.to_le_bytes());
        buf[0x44..0x48].copy_from_slice(&self.s_checkinterval.to_le_bytes());
        buf[0x48..0x4C].copy_from_slice(&self.s_creator_os.to_le_bytes());
        buf[0x4C..0x50].copy_from_slice(&self.s_rev_level.to_le_bytes());
        buf[0x54..0x58].copy_from_slice(&self.s_first_ino.to_le_bytes());
        buf[0x58..0x5A].copy_from_slice(&self.s_inode_size.to_le_bytes());
        buf[0x5A..0x5C].copy_from_slice(&self.s_block_group_nr.to_le_bytes());
        buf[0x5C..0x60].copy_from_slice(&self.s_feature_compat.to_le_bytes());
        buf[0x60..0x64].copy_from_slice(&self.s_feature_incompat.to_le_bytes());
        buf[0x64..0x68].copy_from_slice(&self.s_feature_ro_compat.to_le_bytes());
        buf[0x68..0x78].copy_from_slice(&self.s_uuid);

        let name_bytes = self.s_volume_name.as_bytes();
        let copy_len = min(name_bytes.len(), 16);
        buf[0x78..0x78 + copy_len].copy_from_slice(&name_bytes[..copy_len]);

        buf[0xFE..0x100].copy_from_slice(&self.s_desc_size.to_le_bytes());
        buf[0x150..0x154].copy_from_slice(&self.s_blocks_count_hi.to_le_bytes());
        buf[0x158..0x15C].copy_from_slice(&self.s_free_blocks_count_hi.to_le_bytes());

        buf
    }
}

/// Block Group Descriptor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ext4BlockGroupDesc {
    pub bg_block_bitmap: u64,
    pub bg_inode_bitmap: u64,
    pub bg_inode_table: u64,
    pub bg_free_blocks_count: u32,
    pub bg_free_inodes_count: u32,
    pub bg_used_dirs_count: u32,
    pub bg_flags: u16,
}

impl Ext4BlockGroupDesc {
    pub fn parse(buf: &[u8], is_64bit: bool) -> Result<Self, StorageError> {
        if buf.len() < 32 {
            return Err(StorageError::InvalidData("Group descriptor buffer too small"));
        }

        let bg_block_bitmap_lo = u32::from_le_bytes(buf[0x00..0x04].try_into().unwrap());
        let bg_inode_bitmap_lo = u32::from_le_bytes(buf[0x04..0x08].try_into().unwrap());
        let bg_inode_table_lo = u32::from_le_bytes(buf[0x08..0x0C].try_into().unwrap());
        let bg_free_blocks_count_lo = u16::from_le_bytes([buf[0x0C], buf[0x0D]]);
        let bg_free_inodes_count_lo = u16::from_le_bytes([buf[0x0E], buf[0x0F]]);
        let bg_used_dirs_count_lo = u16::from_le_bytes([buf[0x10], buf[0x11]]);
        let bg_flags = u16::from_le_bytes([buf[0x12], buf[0x13]]);

        let (bg_block_bitmap, bg_inode_bitmap, bg_inode_table, bg_free_blocks, bg_free_inodes, bg_used_dirs) =
            if is_64bit && buf.len() >= 64 {
                let bb_hi = u32::from_le_bytes(buf[0x20..0x24].try_into().unwrap()) as u64;
                let ib_hi = u32::from_le_bytes(buf[0x24..0x28].try_into().unwrap()) as u64;
                let it_hi = u32::from_le_bytes(buf[0x28..0x2C].try_into().unwrap()) as u64;
                let fb_hi = u16::from_le_bytes([buf[0x2C], buf[0x2D]]) as u32;
                let fi_hi = u16::from_le_bytes([buf[0x2E], buf[0x2F]]) as u32;
                let ud_hi = u16::from_le_bytes([buf[0x30], buf[0x31]]) as u32;

                (
                    (bb_hi << 32) | (bg_block_bitmap_lo as u64),
                    (ib_hi << 32) | (bg_inode_bitmap_lo as u64),
                    (it_hi << 32) | (bg_inode_table_lo as u64),
                    (fb_hi << 16) | (bg_free_blocks_count_lo as u32),
                    (fi_hi << 16) | (bg_free_inodes_count_lo as u32),
                    (ud_hi << 16) | (bg_used_dirs_count_lo as u32),
                )
            } else {
                (
                    bg_block_bitmap_lo as u64,
                    bg_inode_bitmap_lo as u64,
                    bg_inode_table_lo as u64,
                    bg_free_blocks_count_lo as u32,
                    bg_free_inodes_count_lo as u32,
                    bg_used_dirs_count_lo as u32,
                )
            };

        Ok(Self {
            bg_block_bitmap,
            bg_inode_bitmap,
            bg_inode_table,
            bg_free_blocks_count: bg_free_blocks,
            bg_free_inodes_count: bg_free_inodes,
            bg_used_dirs_count: bg_used_dirs,
            bg_flags,
        })
    }

    pub fn serialize(&self) -> [u8; 32] {
        let mut buf = [0u8; 32];
        buf[0x00..0x04].copy_from_slice(&(self.bg_block_bitmap as u32).to_le_bytes());
        buf[0x04..0x08].copy_from_slice(&(self.bg_inode_bitmap as u32).to_le_bytes());
        buf[0x08..0x0C].copy_from_slice(&(self.bg_inode_table as u32).to_le_bytes());
        buf[0x0C..0x0E].copy_from_slice(&(self.bg_free_blocks_count as u16).to_le_bytes());
        buf[0x0E..0x10].copy_from_slice(&(self.bg_free_inodes_count as u16).to_le_bytes());
        buf[0x10..0x12].copy_from_slice(&(self.bg_used_dirs_count as u16).to_le_bytes());
        buf[0x12..0x14].copy_from_slice(&self.bg_flags.to_le_bytes());
        buf
    }
}

/// Ext4 Extent Header (12 bytes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ext4ExtentHeader {
    pub eh_magic: u16,
    pub eh_entries: u16,
    pub eh_max: u16,
    pub eh_depth: u16,
    pub eh_generation: u32,
}

impl Ext4ExtentHeader {
    pub fn parse(buf: &[u8]) -> Result<Self, StorageError> {
        if buf.len() < 12 {
            return Err(StorageError::InvalidData("Extent header buffer too small"));
        }
        let eh_magic = u16::from_le_bytes([buf[0], buf[1]]);
        if eh_magic != EXT4_EXTENTS_MAGIC {
            return Err(StorageError::InvalidData("Invalid extent tree magic (expected 0xF30A)"));
        }
        let eh_entries = u16::from_le_bytes([buf[2], buf[3]]);
        let eh_max = u16::from_le_bytes([buf[4], buf[5]]);
        let eh_depth = u16::from_le_bytes([buf[6], buf[7]]);
        let eh_generation = u32::from_le_bytes(buf[8..12].try_into().unwrap());

        Ok(Self {
            eh_magic,
            eh_entries,
            eh_max,
            eh_depth,
            eh_generation,
        })
    }

    pub fn serialize(&self) -> [u8; 12] {
        let mut buf = [0u8; 12];
        buf[0..2].copy_from_slice(&self.eh_magic.to_le_bytes());
        buf[2..4].copy_from_slice(&self.eh_entries.to_le_bytes());
        buf[4..6].copy_from_slice(&self.eh_max.to_le_bytes());
        buf[6..8].copy_from_slice(&self.eh_depth.to_le_bytes());
        buf[8..12].copy_from_slice(&self.eh_generation.to_le_bytes());
        buf
    }
}

/// Ext4 Leaf Extent (12 bytes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ext4Extent {
    pub ee_block: u32,
    pub ee_len: u16,
    pub ee_start_hi: u16,
    pub ee_start_lo: u32,
}

impl Ext4Extent {
    pub fn parse(buf: &[u8]) -> Self {
        Self {
            ee_block: u32::from_le_bytes(buf[0..4].try_into().unwrap()),
            ee_len: u16::from_le_bytes([buf[4], buf[5]]),
            ee_start_hi: u16::from_le_bytes([buf[6], buf[7]]),
            ee_start_lo: u32::from_le_bytes(buf[8..12].try_into().unwrap()),
        }
    }

    pub fn serialize(&self) -> [u8; 12] {
        let mut buf = [0u8; 12];
        buf[0..4].copy_from_slice(&self.ee_block.to_le_bytes());
        buf[4..6].copy_from_slice(&self.ee_len.to_le_bytes());
        buf[6..8].copy_from_slice(&self.ee_start_hi.to_le_bytes());
        buf[8..12].copy_from_slice(&self.ee_start_lo.to_le_bytes());
        buf
    }

    pub fn physical_start_block(&self) -> u64 {
        ((self.ee_start_hi as u64) << 32) | (self.ee_start_lo as u64)
    }

    pub fn length_in_blocks(&self) -> u16 {
        if self.ee_len > 32768 {
            self.ee_len - 32768
        } else {
            self.ee_len
        }
    }
}

/// Ext4 Extent Index Node (12 bytes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ext4ExtentIdx {
    pub ei_block: u32,
    pub ei_leaf_lo: u32,
    pub ei_leaf_hi: u16,
    pub ei_unused: u16,
}

impl Ext4ExtentIdx {
    pub fn parse(buf: &[u8]) -> Self {
        Self {
            ei_block: u32::from_le_bytes(buf[0..4].try_into().unwrap()),
            ei_leaf_lo: u32::from_le_bytes(buf[4..8].try_into().unwrap()),
            ei_leaf_hi: u16::from_le_bytes([buf[8], buf[9]]),
            ei_unused: u16::from_le_bytes([buf[10], buf[11]]),
        }
    }

    pub fn physical_leaf_block(&self) -> u64 {
        ((self.ei_leaf_hi as u64) << 32) | (self.ei_leaf_lo as u64)
    }
}

/// Parsed Ext4 Inode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ext4Inode {
    pub ino: u32,
    pub i_mode: u16,
    pub i_uid: u32,
    pub i_size: u64,
    pub i_atime: u32,
    pub i_ctime: u32,
    pub i_mtime: u32,
    pub i_dtime: u32,
    pub i_gid: u32,
    pub i_links_count: u16,
    pub i_blocks: u64,
    pub i_flags: u32,
    pub i_block: [u8; 60],
}

impl Ext4Inode {
    pub fn parse(ino: u32, buf: &[u8]) -> Result<Self, StorageError> {
        if buf.len() < 128 {
            return Err(StorageError::InvalidData("Inode buffer too small"));
        }

        let i_mode = u16::from_le_bytes([buf[0x00], buf[0x01]]);
        let i_uid_lo = u16::from_le_bytes([buf[0x02], buf[0x03]]);
        let i_size_lo = u32::from_le_bytes(buf[0x04..0x08].try_into().unwrap());
        let i_atime = u32::from_le_bytes(buf[0x08..0x0C].try_into().unwrap());
        let i_ctime = u32::from_le_bytes(buf[0x0C..0x10].try_into().unwrap());
        let i_mtime = u32::from_le_bytes(buf[0x10..0x14].try_into().unwrap());
        let i_dtime = u32::from_le_bytes(buf[0x14..0x18].try_into().unwrap());
        let i_gid_lo = u16::from_le_bytes([buf[0x18], buf[0x19]]);
        let i_links_count = u16::from_le_bytes([buf[0x1A], buf[0x1B]]);
        let i_blocks_lo = u32::from_le_bytes(buf[0x1C..0x20].try_into().unwrap());
        let i_flags = u32::from_le_bytes(buf[0x20..0x24].try_into().unwrap());

        let mut i_block = [0u8; 60];
        i_block.copy_from_slice(&buf[0x28..0x28 + 60]);

        let i_size_high = if buf.len() >= 0x70 {
            u32::from_le_bytes(buf[0x6C..0x70].try_into().unwrap())
        } else {
            0
        };

        let i_size = ((i_size_high as u64) << 32) | (i_size_lo as u64);

        Ok(Self {
            ino,
            i_mode,
            i_uid: i_uid_lo as u32,
            i_size,
            i_atime,
            i_ctime,
            i_mtime,
            i_dtime,
            i_gid: i_gid_lo as u32,
            i_links_count,
            i_blocks: i_blocks_lo as u64,
            i_flags,
            i_block,
        })
    }

    pub fn is_dir(&self) -> bool {
        (self.i_mode & ext4_mode::S_IFMT) == ext4_mode::S_IFDIR
    }

    pub fn is_file(&self) -> bool {
        (self.i_mode & ext4_mode::S_IFMT) == ext4_mode::S_IFREG
    }

    pub fn is_symlink(&self) -> bool {
        (self.i_mode & ext4_mode::S_IFMT) == ext4_mode::S_IFLNK
    }

    pub fn has_extents(&self) -> bool {
        (self.i_flags & EXT4_EXTENTS_FL) != 0
    }

    pub fn serialize(&self) -> [u8; 128] {
        let mut buf = [0u8; 128];
        buf[0x00..0x02].copy_from_slice(&self.i_mode.to_le_bytes());
        buf[0x02..0x04].copy_from_slice(&(self.i_uid as u16).to_le_bytes());
        buf[0x04..0x08].copy_from_slice(&(self.i_size as u32).to_le_bytes());
        buf[0x08..0x0C].copy_from_slice(&self.i_atime.to_le_bytes());
        buf[0x0C..0x10].copy_from_slice(&self.i_ctime.to_le_bytes());
        buf[0x10..0x14].copy_from_slice(&self.i_mtime.to_le_bytes());
        buf[0x14..0x18].copy_from_slice(&self.i_dtime.to_le_bytes());
        buf[0x18..0x1A].copy_from_slice(&(self.i_gid as u16).to_le_bytes());
        buf[0x1A..0x1C].copy_from_slice(&self.i_links_count.to_le_bytes());
        buf[0x1C..0x20].copy_from_slice(&(self.i_blocks as u32).to_le_bytes());
        buf[0x20..0x24].copy_from_slice(&self.i_flags.to_le_bytes());
        buf[0x28..0x28 + 60].copy_from_slice(&self.i_block);
        buf[0x6C..0x70].copy_from_slice(&((self.i_size >> 32) as u32).to_le_bytes());
        buf
    }
}

/// Ext4 Directory Entry (`ext4_dir_entry_2`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ext4DirEntry {
    pub inode: u32,
    pub rec_len: u16,
    pub name_len: u8,
    pub file_type: u8,
    pub name: String,
}

impl Ext4DirEntry {
    pub fn parse(buf: &[u8]) -> Result<Option<Self>, StorageError> {
        if buf.len() < 8 {
            return Ok(None);
        }

        let inode = u32::from_le_bytes(buf[0..4].try_into().unwrap());
        let rec_len = u16::from_le_bytes([buf[4], buf[5]]);
        let name_len = buf[6];
        let file_type = buf[7];

        if rec_len < 8 || (rec_len as usize) > buf.len() {
            return Ok(None);
        }

        if inode == 0 {
            // Unused/deleted entry
            return Ok(Some(Self {
                inode: 0,
                rec_len,
                name_len: 0,
                file_type: 0,
                name: String::new(),
            }));
        }

        let name_end = 8 + (name_len as usize);
        if name_end > buf.len() || name_end > (rec_len as usize) {
            return Err(StorageError::InvalidData("Invalid ext4 dir entry name bounds"));
        }

        let name = String::from_utf8_lossy(&buf[8..name_end]).to_string();

        Ok(Some(Self {
            inode,
            rec_len,
            name_len,
            file_type,
            name,
        }))
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = vec![0u8; self.rec_len as usize];
        buf[0..4].copy_from_slice(&self.inode.to_le_bytes());
        buf[4..6].copy_from_slice(&self.rec_len.to_le_bytes());
        buf[6] = self.name_len;
        buf[7] = self.file_type;
        let name_bytes = self.name.as_bytes();
        let copy_len = min(name_bytes.len(), self.name_len as usize);
        buf[8..8 + copy_len].copy_from_slice(&name_bytes[..copy_len]);
        buf
    }
}

/// Ext4 Core Driver and Filesystem Engine.
pub struct Ext4FileSystem {
    device: Arc<dyn BlockDevice>,
    sb: Ext4Superblock,
}

impl Ext4FileSystem {
    /// Mount an Ext4 filesystem from a block device.
    pub fn mount(device: Arc<dyn BlockDevice>) -> Result<Self, StorageError> {
        let mut sb_buf = vec![0u8; 1024];
        device.read_bytes(EXT4_SUPERBLOCK_OFFSET, &mut sb_buf)?;
        let sb = Ext4Superblock::parse(&sb_buf)?;

        Ok(Self { device, sb })
    }

    pub fn superblock(&self) -> &Ext4Superblock {
        &self.sb
    }

    pub fn device(&self) -> &Arc<dyn BlockDevice> {
        &self.device
    }

    /// Read Block Group Descriptor for a given group index.
    pub fn read_block_group_desc(&self, group: u32) -> Result<Ext4BlockGroupDesc, StorageError> {
        let block_size = self.sb.block_size();
        let desc_size = self.sb.s_desc_size as usize;

        // Group descriptors start immediately after superblock
        // If block_size == 1024, block 0 is boot block/superblock, group descs start at block 2
        // If block_size > 1024, block 0 contains superblock at offset 1024, group descs start at block 1
        let first_bg_block = if block_size == 1024 { 2u64 } else { 1u64 };

        let offset = (first_bg_block * block_size as u64) + (group as u64 * desc_size as u64);
        let mut buf = vec![0u8; desc_size];
        self.device.read_bytes(offset, &mut buf)?;

        let is_64bit = (self.sb.s_feature_incompat & 0x0080) != 0;
        Ext4BlockGroupDesc::parse(&buf, is_64bit)
    }

    /// Read an Inode by its 1-based inode number.
    pub fn read_inode(&self, ino: u32) -> Result<Ext4Inode, StorageError> {
        if ino == 0 || ino > self.sb.s_inodes_count {
            return Err(StorageError::NotFound);
        }

        let group = (ino - 1) / self.sb.s_inodes_per_group;
        let index_in_group = (ino - 1) % self.sb.s_inodes_per_group;

        let bg_desc = self.read_block_group_desc(group)?;
        let block_size = self.sb.block_size() as u64;
        let inode_size = self.sb.s_inode_size as u64;

        let itable_byte_offset = bg_desc.bg_inode_table * block_size;
        let inode_byte_offset = itable_byte_offset + (index_in_group as u64 * inode_size);

        let mut buf = vec![0u8; inode_size as usize];
        self.device.read_bytes(inode_byte_offset, &mut buf)?;

        Ext4Inode::parse(ino, &buf)
    }

    /// Traverse the extent tree for an Inode to locate the physical block for a given logical block.
    pub fn map_logical_block(&self, inode: &Ext4Inode, logical_block: u32) -> Result<Option<u64>, StorageError> {
        if !inode.has_extents() {
            // Fallback for legacy direct blocks in i_block
            if logical_block < 12 {
                let offset = (logical_block as usize) * 4;
                let phys = u32::from_le_bytes(inode.i_block[offset..offset + 4].try_into().unwrap());
                return Ok(if phys != 0 { Some(phys as u64) } else { None });
            }
            return Err(StorageError::InvalidData("Indirect ext2/3 block trees not required for extents"));
        }

        self.traverse_extent_tree(&inode.i_block, logical_block)
    }

    fn traverse_extent_tree(&self, extent_data: &[u8], logical_block: u32) -> Result<Option<u64>, StorageError> {
        let header = Ext4ExtentHeader::parse(&extent_data[0..12])?;

        if header.eh_depth == 0 {
            // Leaf node: array of Ext4Extent
            for i in 0..header.eh_entries {
                let offset = 12 + (i as usize * 12);
                let extent = Ext4Extent::parse(&extent_data[offset..offset + 12]);
                let start_blk = extent.ee_block;
                let count = extent.length_in_blocks() as u32;

                if logical_block >= start_blk && logical_block < start_blk + count {
                    let blk_offset = (logical_block - start_blk) as u64;
                    return Ok(Some(extent.physical_start_block() + blk_offset));
                }
            }
            Ok(None)
        } else {
            // Index node: array of Ext4ExtentIdx
            let mut target_child_block = None;
            for i in 0..header.eh_entries {
                let offset = 12 + (i as usize * 12);
                let idx = Ext4ExtentIdx::parse(&extent_data[offset..offset + 12]);
                if logical_block >= idx.ei_block {
                    target_child_block = Some(idx.physical_leaf_block());
                } else {
                    break;
                }
            }

            if let Some(child_blk) = target_child_block {
                let block_size = self.sb.block_size();
                let mut child_buf = vec![0u8; block_size];
                self.device.read_bytes(child_blk * block_size as u64, &mut child_buf)?;
                self.traverse_extent_tree(&child_buf, logical_block)
            } else {
                Ok(None)
            }
        }
    }

    /// Read data from an Inode at a specified byte offset.
    pub fn read_inode_data(
        &self,
        inode: &Ext4Inode,
        offset: u64,
        buf: &mut [u8],
    ) -> Result<usize, StorageError> {
        if offset >= inode.i_size || buf.is_empty() {
            return Ok(0);
        }

        let block_size = self.sb.block_size() as u64;
        let max_readable = min(buf.len() as u64, inode.i_size - offset) as usize;

        let mut bytes_read = 0usize;
        let mut curr_offset = offset;
        let mut blk_buf = vec![0u8; block_size as usize];

        while bytes_read < max_readable {
            let logical_block = (curr_offset / block_size) as u32;
            let offset_in_block = (curr_offset % block_size) as usize;
            let available = (block_size as usize) - offset_in_block;
            let to_read = min(max_readable - bytes_read, available);

            if let Some(phys_block) = self.map_logical_block(inode, logical_block)? {
                self.device.read_bytes(phys_block * block_size, &mut blk_buf)?;
                buf[bytes_read..bytes_read + to_read]
                    .copy_from_slice(&blk_buf[offset_in_block..offset_in_block + to_read]);
            } else {
                // Sparse block hole: read as zero
                buf[bytes_read..bytes_read + to_read].fill(0);
            }

            bytes_read += to_read;
            curr_offset += to_read as u64;
        }

        Ok(bytes_read)
    }

    /// Read all directory entries from a directory Inode.
    pub fn readdir(&self, dir_inode: &Ext4Inode) -> Result<Vec<Ext4DirEntry>, StorageError> {
        if !dir_inode.is_dir() {
            return Err(StorageError::NotADirectory);
        }

        let mut entries = Vec::new();
        let total_size = dir_inode.i_size as usize;
        let mut data = vec![0u8; total_size];
        let n = self.read_inode_data(dir_inode, 0, &mut data)?;
        data.truncate(n);

        let mut offset = 0usize;
        while offset + 8 <= data.len() {
            let chunk = &data[offset..];
            if let Some(entry) = Ext4DirEntry::parse(chunk)? {
                let rec_len = entry.rec_len as usize;
                if rec_len == 0 {
                    break;
                }
                if entry.inode != 0 {
                    entries.push(entry);
                }
                offset += rec_len;
            } else {
                break;
            }
        }

        Ok(entries)
    }

    /// Look up a path starting from root (inode 2). E.g. "/bin/bash" or "etc/passwd".
    pub fn lookup_path(&self, path: &str) -> Result<Ext4Inode, StorageError> {
        let clean = path.trim();
        let mut curr = self.read_inode(EXT4_ROOT_INO)?;

        for component in clean.split('/') {
            if component.is_empty() || component == "." {
                continue;
            }
            if component == ".." {
                // Lookup parent in directory entries
                let entries = self.readdir(&curr)?;
                let parent_entry = entries
                    .into_iter()
                    .find(|e| e.name == "..")
                    .ok_or(StorageError::NotFound)?;
                curr = self.read_inode(parent_entry.inode)?;
                continue;
            }

            let entries = self.readdir(&curr)?;
            let child_entry = entries
                .into_iter()
                .find(|e| e.name == component)
                .ok_or(StorageError::NotFound)?;

            curr = self.read_inode(child_entry.inode)?;
        }

        Ok(curr)
    }

    /// Read full content of a file specified by path.
    pub fn read_file_by_path(&self, path: &str) -> Result<Vec<u8>, StorageError> {
        let inode = self.lookup_path(path)?;
        if !inode.is_file() {
            return Err(StorageError::InvalidData("Path is not a regular file"));
        }

        let mut buf = vec![0u8; inode.i_size as usize];
        let n = self.read_inode_data(&inode, 0, &mut buf)?;
        buf.truncate(n);
        Ok(buf)
    }

    /// Format and provision a basic, valid Ext4 filesystem on a block device for tests and installation.
    pub fn format(device: Arc<dyn BlockDevice>, volume_name: &str) -> Result<Self, StorageError> {
        let total_sectors = device.total_sectors();
        let block_size = 4096usize; // Standard 4KB ext4 block size
        let sectors_per_block = (block_size / 512) as u64;
        let total_blocks = (total_sectors / sectors_per_block) as u32;

        if total_blocks < 256 {
            return Err(StorageError::InvalidData("Device too small for ext4 filesystem"));
        }

        let blocks_per_group = 32768u32;
        let inodes_per_group = 2048u32;
        let inode_size = 256u16;

        let sb = Ext4Superblock {
            s_inodes_count: inodes_per_group,
            s_blocks_count_lo: total_blocks,
            s_r_blocks_count_lo: total_blocks / 20,
            s_free_blocks_count_lo: total_blocks.saturating_sub(64),
            s_free_inodes_count: inodes_per_group.saturating_sub(12),
            s_first_data_block: 0, // 0 for 4KB blocks
            s_log_block_size: 2, // 1024 << 2 = 4096
            s_log_cluster_size: 2,
            s_blocks_per_group: blocks_per_group,
            s_clusters_per_group: blocks_per_group,
            s_inodes_per_group: inodes_per_group,
            s_magic: EXT4_SUPERBLOCK_MAGIC,
            s_state: 1, // Clean
            s_errors: 1, // Continue
            s_minor_rev_level: 0,
            s_lastcheck: 0x6200_0000,
            s_checkinterval: 0,
            s_creator_os: 0, // Linux
            s_rev_level: 1, // Dynamic revision
            s_first_ino: 11,
            s_inode_size: inode_size,
            s_block_group_nr: 0,
            s_feature_compat: 0x0000_0038, // dir_index, ext_attr, resize_inode
            s_feature_incompat: 0x0000_0042, // filetype (0x02) | extents (0x40)
            s_feature_ro_compat: 0x0000_0003, // sparse_super | large_file
            s_uuid: [0x55; 16],
            s_volume_name: volume_name.to_string(),
            s_desc_size: 32,
            s_blocks_count_hi: 0,
            s_free_blocks_count_hi: 0,
        };

        // 1. Write Superblock at byte offset 1024
        let sb_bytes = sb.serialize();
        device.write_bytes(EXT4_SUPERBLOCK_OFFSET, &sb_bytes)?;

        // Block layout for Group 0:
        // Block 0: Superblock (at offset 1024)
        // Block 1: Block Group Descriptors
        // Block 2: Block Bitmap
        // Block 3: Inode Bitmap
        // Block 4..131: Inode Table (2048 inodes * 256 bytes = 524288 bytes = 128 blocks)
        // Block 132: Root Directory Data Block (Inode 2)
        // Block 133: Test file data block (Inode 11)

        let bg_desc = Ext4BlockGroupDesc {
            bg_block_bitmap: 2,
            bg_inode_bitmap: 3,
            bg_inode_table: 4,
            bg_free_blocks_count: (total_blocks.saturating_sub(135)) as u32,
            bg_free_inodes_count: inodes_per_group.saturating_sub(12),
            bg_used_dirs_count: 2,
            bg_flags: 0,
        };

        let bg_bytes = bg_desc.serialize();
        let mut bg_block_buf = vec![0u8; block_size];
        bg_block_buf[0..32].copy_from_slice(&bg_bytes);
        device.write_bytes(1 * block_size as u64, &bg_block_buf)?;

        // 2. Initialize Root Inode (ino 2)
        // Set up Extent Header + Extent mapping logical block 0 -> physical block 132
        let mut root_extent_buf = [0u8; 60];
        let ext_header = Ext4ExtentHeader {
            eh_magic: EXT4_EXTENTS_MAGIC,
            eh_entries: 1,
            eh_max: 4,
            eh_depth: 0,
            eh_generation: 0,
        };
        root_extent_buf[0..12].copy_from_slice(&ext_header.serialize());

        let root_extent = Ext4Extent {
            ee_block: 0,
            ee_len: 1,
            ee_start_hi: 0,
            ee_start_lo: 132,
        };
        root_extent_buf[12..24].copy_from_slice(&root_extent.serialize());

        let root_inode = Ext4Inode {
            ino: EXT4_ROOT_INO,
            i_mode: ext4_mode::S_IFDIR | 0o755,
            i_uid: 0,
            i_size: block_size as u64,
            i_atime: 0x6200_0000,
            i_ctime: 0x6200_0000,
            i_mtime: 0x6200_0000,
            i_dtime: 0,
            i_gid: 0,
            i_links_count: 3,
            i_blocks: 8, // 8 * 512 = 4096
            i_flags: EXT4_EXTENTS_FL,
            i_block: root_extent_buf,
        };

        // Write root inode to Inode Table (block 4)
        // Inode 2 is at index 1 (ino - 1 = 1) -> offset 1 * 256 = 256 bytes in inode table
        let itable_byte_offset = 4 * block_size as u64;
        let mut inode_buf = [0u8; 256];
        inode_buf[0..128].copy_from_slice(&root_inode.serialize());
        device.write_bytes(itable_byte_offset + 256, &inode_buf)?;

        // 3. Initialize Root Directory Data Block (Block 132)
        // Contains "." (ino 2) and ".." (ino 2)
        let mut root_dir_buf = vec![0u8; block_size];
        let dot_entry = Ext4DirEntry {
            inode: EXT4_ROOT_INO,
            rec_len: 12,
            name_len: 1,
            file_type: ext4_file_types::EXT4_FT_DIR,
            name: ".".to_string(),
        };
        root_dir_buf[0..12].copy_from_slice(&dot_entry.serialize());

        // ".." takes up the rest of the block: 4096 - 12 = 4084 bytes
        let dotdot_entry = Ext4DirEntry {
            inode: EXT4_ROOT_INO,
            rec_len: (block_size - 12) as u16,
            name_len: 2,
            file_type: ext4_file_types::EXT4_FT_DIR,
            name: "..".to_string(),
        };
        root_dir_buf[12..block_size].copy_from_slice(&dotdot_entry.serialize());

        device.write_bytes(132 * block_size as u64, &root_dir_buf)?;

        Ok(Self { device, sb })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::RamDisk;

    #[test]
    fn test_ext4_superblock_parse_and_serialize() {
        let sb = Ext4Superblock {
            s_inodes_count: 2048,
            s_blocks_count_lo: 32768,
            s_r_blocks_count_lo: 1638,
            s_free_blocks_count_lo: 31000,
            s_free_inodes_count: 2000,
            s_first_data_block: 0,
            s_log_block_size: 2,
            s_log_cluster_size: 2,
            s_blocks_per_group: 32768,
            s_clusters_per_group: 32768,
            s_inodes_per_group: 2048,
            s_magic: EXT4_SUPERBLOCK_MAGIC,
            s_state: 1,
            s_errors: 1,
            s_minor_rev_level: 0,
            s_lastcheck: 0,
            s_checkinterval: 0,
            s_creator_os: 0,
            s_rev_level: 1,
            s_first_ino: 11,
            s_inode_size: 256,
            s_block_group_nr: 0,
            s_feature_compat: 0x38,
            s_feature_incompat: 0x42,
            s_feature_ro_compat: 0x03,
            s_uuid: [0x42; 16],
            s_volume_name: "rootfs".to_string(),
            s_desc_size: 32,
            s_blocks_count_hi: 0,
            s_free_blocks_count_hi: 0,
        };

        let bytes = sb.serialize();
        let parsed = Ext4Superblock::parse(&bytes).unwrap();
        assert_eq!(parsed.s_magic, EXT4_SUPERBLOCK_MAGIC);
        assert_eq!(parsed.block_size(), 4096);
        assert_eq!(parsed.s_volume_name, "rootfs");
        assert_eq!(parsed.s_inodes_count, 2048);
    }

    #[test]
    fn test_ext4_extent_tree_mapping() {
        let mut block_data = [0u8; 60];
        let header = Ext4ExtentHeader {
            eh_magic: EXT4_EXTENTS_MAGIC,
            eh_entries: 2,
            eh_max: 4,
            eh_depth: 0,
            eh_generation: 0,
        };
        block_data[0..12].copy_from_slice(&header.serialize());

        // Extent 1: logical block 0..10 -> physical block 500..510
        let ext1 = Ext4Extent {
            ee_block: 0,
            ee_len: 10,
            ee_start_hi: 0,
            ee_start_lo: 500,
        };
        block_data[12..24].copy_from_slice(&ext1.serialize());

        // Extent 2: logical block 10..15 -> physical block 800..805
        let ext2 = Ext4Extent {
            ee_block: 10,
            ee_len: 5,
            ee_start_hi: 0,
            ee_start_lo: 800,
        };
        block_data[24..36].copy_from_slice(&ext2.serialize());

        let inode = Ext4Inode {
            ino: 100,
            i_mode: ext4_mode::S_IFREG | 0o644,
            i_uid: 1000,
            i_size: 15 * 4096,
            i_atime: 0,
            i_ctime: 0,
            i_mtime: 0,
            i_dtime: 0,
            i_gid: 1000,
            i_links_count: 1,
            i_blocks: 120,
            i_flags: EXT4_EXTENTS_FL,
            i_block: block_data,
        };

        let ramdisk = Arc::new(RamDisk::new(65536));
        let fs = Ext4FileSystem::format(ramdisk.clone(), "rootfs").unwrap();

        assert_eq!(fs.map_logical_block(&inode, 0).unwrap(), Some(500));
        assert_eq!(fs.map_logical_block(&inode, 5).unwrap(), Some(505));
        assert_eq!(fs.map_logical_block(&inode, 10).unwrap(), Some(800));
        assert_eq!(fs.map_logical_block(&inode, 14).unwrap(), Some(804));
        assert_eq!(fs.map_logical_block(&inode, 20).unwrap(), None);
    }

    #[test]
    fn test_ext4_root_dir_traversal() {
        let ramdisk = Arc::new(RamDisk::new(65536));
        let fs = Ext4FileSystem::format(ramdisk.clone(), "hybrid_root").unwrap();

        let root_inode = fs.read_inode(EXT4_ROOT_INO).unwrap();
        assert!(root_inode.is_dir());

        let entries = fs.readdir(&root_inode).unwrap();
        assert!(entries.iter().any(|e| e.name == "."));
        assert!(entries.iter().any(|e| e.name == ".."));

        // Lookup root path
        let resolved = fs.lookup_path("/").unwrap();
        assert_eq!(resolved.ino, EXT4_ROOT_INO);
    }
}
