//! Real Bare-Metal ATA PIO Disk Driver for Primary IDE Controller.
//!
//! Operates directly on standard x86_64 legacy IDE I/O ports:
//! - Data Register: 0x1F0
//! - Error/Features: 0x1F1
//! - Sector Count: 0x1F2
//! - LBA Low/Mid/High: 0x1F3, 0x1F4, 0x1F5
//! - Device/Drive: 0x1F6
//! - Status/Command: 0x1F7
//! - Device Control: 0x3F6
//!
//! Implements real 28-bit LBA PIO sector reads and writes (512 bytes per sector) from the
//! physical QEMU disk drive. Zero simulation.

use alloc::vec::Vec;
use crate::sync::SpinLock;
use crate::arch::msr::{inb, inw, outb};

pub const ATA_DATA_PORT: u16           = 0x1F0;
pub const ATA_ERROR_PORT: u16          = 0x1F1;
pub const ATA_SECTOR_COUNT_PORT: u16   = 0x1F2;
pub const ATA_LBA_LOW_PORT: u16        = 0x1F3;
pub const ATA_LBA_MID_PORT: u16        = 0x1F4;
pub const ATA_LBA_HIGH_PORT: u16       = 0x1F5;
pub const ATA_DEVICE_PORT: u16         = 0x1F6;
pub const ATA_STATUS_COMMAND_PORT: u16 = 0x1F7;
pub const ATA_CONTROL_PORT: u16        = 0x3F6;

pub const ATA_STATUS_BSY: u8 = 0x80; // Busy
pub const ATA_STATUS_DRDY: u8 = 0x40; // Drive Ready
pub const ATA_STATUS_DF: u8   = 0x20; // Drive Fault
pub const ATA_STATUS_DRQ: u8  = 0x08; // Data Request Ready
pub const ATA_STATUS_ERR: u8  = 0x01; // Error

pub const ATA_CMD_READ_SECTORS: u8  = 0x20;
pub const ATA_CMD_WRITE_SECTORS: u8 = 0x30;
pub const ATA_CMD_IDENTIFY: u8      = 0xEC;

pub const SECTOR_SIZE: usize = 512;

pub struct AtaPioDriver {
    pub is_master: bool,
    pub total_sectors: u64,
    pub drive_name: &'static str,
}

impl AtaPioDriver {
    pub const fn new(is_master: bool) -> Self {
        Self {
            is_master,
            total_sectors: 0,
            drive_name: "Primary ATA Hard Disk",
        }
    }

    /// Short I/O delay (400ns) by reading the alternate status register 4 times.
    #[inline]
    fn io_delay(&self) {
        unsafe {
            let _ = inb(ATA_CONTROL_PORT);
            let _ = inb(ATA_CONTROL_PORT);
            let _ = inb(ATA_CONTROL_PORT);
            let _ = inb(ATA_CONTROL_PORT);
        }
    }

    /// Waits until the drive clears the BSY flag.
    fn wait_not_busy(&self) -> Result<u8, &'static str> {
        for _ in 0..100_000 {
            let status = unsafe { inb(ATA_STATUS_COMMAND_PORT) };
            if (status & ATA_STATUS_BSY) == 0 {
                return Ok(status);
            }
        }
        Err("ATA device timed out waiting for BSY to clear")
    }

    /// Waits until the drive sets the DRQ (Data Request) flag and clears BSY.
    fn wait_ready(&self) -> Result<u8, &'static str> {
        for _ in 0..100_000 {
            let status = unsafe { inb(ATA_STATUS_COMMAND_PORT) };
            if (status & ATA_STATUS_ERR) != 0 {
                return Err("ATA hardware reported I/O error");
            }
            if (status & ATA_STATUS_DF) != 0 {
                return Err("ATA hardware reported Drive Fault");
            }
            if (status & ATA_STATUS_BSY) == 0 && (status & ATA_STATUS_DRQ) != 0 {
                return Ok(status);
            }
        }
        Err("ATA device timed out waiting for DRQ data readiness")
    }

    /// Reads a single 512-byte sector at the specified 28-bit LBA address.
    pub fn read_sector(&mut self, lba: u32, buf: &mut [u8; SECTOR_SIZE]) -> Result<(), &'static str> {
        if lba >= 0x1000_0000 {
            return Err("ATA LBA28 out of bounds (>= 128 GiB)");
        }
        self.wait_not_busy()?;

        let drive_sel: u8 = if self.is_master { 0xE0 } else { 0xF0 } | (((lba >> 24) & 0x0F) as u8);

        unsafe {
            outb(ATA_DEVICE_PORT, drive_sel);
            self.io_delay();

            outb(ATA_SECTOR_COUNT_PORT, 1);
            outb(ATA_LBA_LOW_PORT, (lba & 0xFF) as u8);
            outb(ATA_LBA_MID_PORT, ((lba >> 8) & 0xFF) as u8);
            outb(ATA_LBA_HIGH_PORT, ((lba >> 16) & 0xFF) as u8);

            outb(ATA_STATUS_COMMAND_PORT, ATA_CMD_READ_SECTORS);
            self.io_delay();
        }

        self.wait_ready()?;

        unsafe {
            let buf16 = buf.as_mut_ptr() as *mut u16;
            for i in 0..256 {
                let word = inw(ATA_DATA_PORT);
                core::ptr::write_unaligned(buf16.add(i), word);
            }
        }

        Ok(())
    }

    /// Writes a single 512-byte sector at the specified 28-bit LBA address (Persistent Disk Write).
    pub fn write_sector(&mut self, lba: u32, buf: &[u8; SECTOR_SIZE]) -> Result<(), &'static str> {
        if lba >= 0x1000_0000 {
            return Err("ATA LBA28 out of bounds (>= 128 GiB)");
        }
        self.wait_not_busy()?;

        let drive_sel: u8 = if self.is_master { 0xE0 } else { 0xF0 } | (((lba >> 24) & 0x0F) as u8);

        unsafe {
            outb(ATA_DEVICE_PORT, drive_sel);
            self.io_delay();

            outb(ATA_SECTOR_COUNT_PORT, 1);
            outb(ATA_LBA_LOW_PORT, (lba & 0xFF) as u8);
            outb(ATA_LBA_MID_PORT, ((lba >> 8) & 0xFF) as u8);
            outb(ATA_LBA_HIGH_PORT, ((lba >> 16) & 0xFF) as u8);

            outb(ATA_STATUS_COMMAND_PORT, ATA_CMD_WRITE_SECTORS);
            self.io_delay();
        }

        self.wait_ready()?;

        unsafe {
            let buf16 = buf.as_ptr() as *const u16;
            for i in 0..256 {
                let word = core::ptr::read_unaligned(buf16.add(i));
                crate::arch::msr::outw(ATA_DATA_PORT, word);
            }
        }

        self.wait_not_busy()?;
        unsafe {
            outb(ATA_STATUS_COMMAND_PORT, 0xE7); // Cache flush
        }
        self.wait_not_busy()?;

        Ok(())
    }

    /// Reads contiguous sectors from the disk into a byte buffer.
    pub fn read_sectors(&mut self, start_lba: u32, count: usize, buf: &mut [u8]) -> Result<(), &'static str> {
        if buf.len() < count * SECTOR_SIZE {
            return Err("Buffer is smaller than the requested sector count");
        }

        let mut sector_buf = [0u8; SECTOR_SIZE];
        for i in 0..count {
            self.read_sector(start_lba + i as u32, &mut sector_buf)?;
            let offset = i * SECTOR_SIZE;
            buf[offset..offset + SECTOR_SIZE].copy_from_slice(&sector_buf);
        }

        Ok(())
    }

    /// Writes contiguous sectors from a byte buffer to the disk.
    pub fn write_sectors(&mut self, start_lba: u32, count: usize, buf: &[u8]) -> Result<(), &'static str> {
        if buf.len() < count * SECTOR_SIZE {
            return Err("Buffer is smaller than the requested sector count");
        }

        let mut sector_buf = [0u8; SECTOR_SIZE];
        for i in 0..count {
            let offset = i * SECTOR_SIZE;
            sector_buf.copy_from_slice(&buf[offset..offset + SECTOR_SIZE]);
            self.write_sector(start_lba + i as u32, &sector_buf)?;
        }

        Ok(())
    }
}

pub static GLOBAL_ATA: SpinLock<AtaPioDriver> = SpinLock::new(AtaPioDriver::new(true));

/// Initializes the primary ATA controller and verifies the drive presence.
pub fn init_ata() -> Result<(), &'static str> {
    let mut driver = GLOBAL_ATA.lock();
    let mut sector0 = [0u8; 512];
    driver.read_sector(0, &mut sector0)?;

    if sector0[510] == 0x55 && sector0[511] == 0xAA {
        crate::serial_println!("[ATA PIO] Primary IDE Master drive detected: Valid MBR Boot Sector (512 bytes). Disk Write Persistence Active.");
        Ok(())
    } else {
        crate::serial_println!("[ATA PIO] Warning: MBR signature 0x55AA not found on primary drive.");
        Ok(())
    }
}
