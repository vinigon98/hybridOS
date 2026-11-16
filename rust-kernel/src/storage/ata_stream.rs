//! Real Bare-Metal ATA Block Device & Stream Adapter for HybridOS Storage.
//!
//! Exposes physical ATA controller I/O (`GLOBAL_ATA`) as a standard `BlockDevice`
//! and provides synchronous sequential streaming with mandatory `0xE7` Cache Flush.

use alloc::sync::Arc;
use core::cmp::min;
use crate::drivers::ata::{GLOBAL_ATA, SECTOR_SIZE};
use super::{BlockDevice, StorageError, PartitionBlockDevice};

/// BlockDevice implementation over the primary physical ATA controller.
pub struct AtaBlockDevice {
    pub is_master: bool,
    pub total_sectors: u64,
}

impl AtaBlockDevice {
    pub fn new(is_master: bool) -> Result<Self, StorageError> {
        let total_sectors = {
            let mut ata = GLOBAL_ATA.lock();
            let mut buf = [0u8; SECTOR_SIZE];
            // Read Sector 0 to verify device connectivity
            ata.read_sector(0, &mut buf).map_err(|e| StorageError::IoError(e))?;
            // ATA LBA28 standard maximum addressable sectors: 2^28 = 268,435,456 (128 GiB)
            0x1000_0000u64
        };

        Ok(Self {
            is_master,
            total_sectors,
        })
    }
}

impl BlockDevice for AtaBlockDevice {
    fn sector_size(&self) -> usize {
        SECTOR_SIZE
    }

    fn total_sectors(&self) -> u64 {
        self.total_sectors
    }

    fn read_sectors(&self, start_lba: u64, buf: &mut [u8]) -> Result<usize, StorageError> {
        if start_lba >= self.total_sectors {
            return Err(StorageError::IoError("ATA read LBA out of bounds"));
        }
        let sec_count = buf.len() / SECTOR_SIZE;
        let mut ata = GLOBAL_ATA.lock();
        ata.read_sectors(start_lba as u32, sec_count, buf)
            .map_err(|e| StorageError::IoError(e))?;
        Ok(buf.len())
    }

    fn write_sectors(&self, start_lba: u64, buf: &[u8]) -> Result<usize, StorageError> {
        if start_lba >= self.total_sectors {
            return Err(StorageError::IoError("ATA write LBA out of bounds"));
        }
        let sec_count = buf.len() / SECTOR_SIZE;
        let mut ata = GLOBAL_ATA.lock();
        // write_sectors performs write + synchronous 0xE7 Cache Flush per sector
        ata.write_sectors(start_lba as u32, sec_count, buf)
            .map_err(|e| StorageError::IoError(e))?;
        Ok(buf.len())
    }
}

/// High-performance byte-level stream adapter bound to an ATA partition.
pub struct AtaStream {
    partition_dev: Arc<PartitionBlockDevice>,
    cursor: u64,
    size_bytes: u64,
}

impl AtaStream {
    pub fn new(partition_dev: Arc<PartitionBlockDevice>) -> Self {
        let size_bytes = partition_dev.total_sectors() * (partition_dev.sector_size() as u64);
        Self {
            partition_dev,
            cursor: 0,
            size_bytes,
        }
    }

    pub fn position(&self) -> u64 {
        self.cursor
    }

    pub fn seek(&mut self, pos: u64) -> Result<u64, StorageError> {
        if pos > self.size_bytes {
            return Err(StorageError::IoError("Stream seek out of partition range"));
        }
        self.cursor = pos;
        Ok(self.cursor)
    }

    pub fn read(&mut self, buf: &mut [u8]) -> Result<usize, StorageError> {
        if self.cursor >= self.size_bytes || buf.is_empty() {
            return Ok(0);
        }
        let to_read = min(buf.len() as u64, self.size_bytes - self.cursor) as usize;
        self.partition_dev.read_bytes(self.cursor, &mut buf[..to_read])?;
        self.cursor += to_read as u64;
        Ok(to_read)
    }

    pub fn write(&mut self, buf: &[u8]) -> Result<usize, StorageError> {
        if buf.is_empty() {
            return Ok(0);
        }
        if self.cursor + (buf.len() as u64) > self.size_bytes {
            return Err(StorageError::NoSpace);
        }
        self.partition_dev.write_bytes(self.cursor, buf)?;
        self.cursor += buf.len() as u64;
        Ok(buf.len())
    }
}
