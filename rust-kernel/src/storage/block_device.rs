//! Unified Block Device Subsystem for HybridOS Kernel.
//!
//! Provides a hardware-agnostic block I/O layer:
//! - [`BlockDevice`]: Universal trait for NVMe, AHCI, VirtIO, and RAM storage.
//! - [`DeviceStatus`] & [`DeviceType`]: Operational telemetry and hardware classification.
//! - [`BlockDeviceRegistry`]: Thread-safe dynamic registry with global singleton access.
//! - [`RamBlockDevice`]: Memory-backed block device reference implementation.

use std::sync::{Arc, RwLock, OnceLock};
use std::sync::atomic::{AtomicU32, Ordering};
use std::collections::HashMap;
use std::fmt;

// ============================================================================
// Device Classification & Status
// ============================================================================

/// Hardware storage interface classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeviceType {
    /// Non-Volatile Memory Express (PCIe NVMe 1.4+)
    Nvme,
    /// Serial ATA Advanced Host Controller Interface (SATA AHCI 1.3+)
    AhciSata,
    /// VirtIO Paravirtualized Block Device (QEMU/KVM virtio-blk)
    VirtIoBlk,
    /// In-memory RAM Disk
    RamDisk,
    /// Legacy Integrated Drive Electronics / ATAPI
    Ide,
    /// Small Computer System Interface
    Scsi,
    /// Other / Custom block device
    Other(&'static str),
}

impl fmt::Display for DeviceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DeviceType::Nvme => write!(f, "NVMe"),
            DeviceType::AhciSata => write!(f, "AHCI SATA"),
            DeviceType::VirtIoBlk => write!(f, "VirtIO Block"),
            DeviceType::RamDisk => write!(f, "RAM Disk"),
            DeviceType::Ide => write!(f, "IDE/ATAPI"),
            DeviceType::Scsi => write!(f, "SCSI"),
            DeviceType::Other(s) => write!(f, "Other ({})", s),
        }
    }
}

/// Operational state of a block device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceStatus {
    /// Device is online and ready to accept read/write requests.
    Online,
    /// Device is offline or disconnected.
    Offline,
    /// Device is functioning in a degraded state (e.g. SMART alerts or RAID recovery).
    Degraded(String),
    /// Device is currently busy servicing high-priority command or queue flush.
    Busy,
    /// Device encountered an unrecoverable hardware or protocol error.
    Error(String),
}

impl DeviceStatus {
    pub fn is_ready(&self) -> bool {
        matches!(self, DeviceStatus::Online | DeviceStatus::Degraded(_))
    }
}

impl fmt::Display for DeviceStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DeviceStatus::Online => write!(f, "Online"),
            DeviceStatus::Offline => write!(f, "Offline"),
            DeviceStatus::Degraded(reason) => write!(f, "Degraded: {}", reason),
            DeviceStatus::Busy => write!(f, "Busy"),
            DeviceStatus::Error(err) => write!(f, "Error: {}", err),
        }
    }
}

/// Summary metadata describing a registered block device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockDeviceInfo {
    pub id: u32,
    pub name: String,
    pub device_type: DeviceType,
    pub status: DeviceStatus,
    pub block_size: u32,
    pub block_count: u64,
    pub total_bytes: u64,
    pub read_only: bool,
}

// ============================================================================
// Block Device Errors
// ============================================================================

/// Error conditions encountered during block storage operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockDeviceError {
    /// General I/O error
    IoError(String),
    /// Logical Block Address out of bounds
    InvalidLba { lba: u64, max_lba: u64 },
    /// Destination or source buffer is too small for the requested block transfer
    BufferTooSmall { required: usize, provided: usize },
    /// Buffer length is not a multiple of the device's logical block size
    UnalignedBuffer { buffer_len: usize, block_size: u32 },
    /// Device is currently busy
    DeviceBusy,
    /// Device is offline
    DeviceOffline,
    /// Attempted write to a read-only medium
    WriteProtected,
    /// Hardware controller or PHY error
    HardwareFault(String),
    /// Command execution timed out
    Timeout,
    /// Device ID or Name not found
    NotFound,
    /// Device ID or Name already registered
    AlreadyExists,
    /// Unsupported or invalid command
    InvalidCommand(String),
    /// Queue full
    QueueFull,
}

impl fmt::Display for BlockDeviceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BlockDeviceError::IoError(msg) => write!(f, "Block I/O Error: {}", msg),
            BlockDeviceError::InvalidLba { lba, max_lba } => {
                write!(f, "Invalid LBA {}: exceeds device capacity of {} blocks", lba, max_lba)
            }
            BlockDeviceError::BufferTooSmall { required, provided } => {
                write!(f, "Buffer too small: required {} bytes, provided {} bytes", required, provided)
            }
            BlockDeviceError::UnalignedBuffer { buffer_len, block_size } => {
                write!(f, "Buffer length {} is not a multiple of block size {}", buffer_len, block_size)
            }
            BlockDeviceError::DeviceBusy => write!(f, "Device is busy"),
            BlockDeviceError::DeviceOffline => write!(f, "Device is offline"),
            BlockDeviceError::WriteProtected => write!(f, "Device is write-protected"),
            BlockDeviceError::HardwareFault(e) => write!(f, "Hardware fault: {}", e),
            BlockDeviceError::Timeout => write!(f, "Command timed out"),
            BlockDeviceError::NotFound => write!(f, "Block device not found"),
            BlockDeviceError::AlreadyExists => write!(f, "Block device already exists"),
            BlockDeviceError::InvalidCommand(cmd) => write!(f, "Invalid command: {}", cmd),
            BlockDeviceError::QueueFull => write!(f, "Submission queue full"),
        }
    }
}

impl std::error::Error for BlockDeviceError {}

// ============================================================================
// Block Device Trait
// ============================================================================

/// Trait implemented by all block-level storage devices in HybridOS.
pub trait BlockDevice: Send + Sync {
    /// Returns the unique numeric ID assigned to this device.
    fn id(&self) -> u32;

    /// Returns the canonical device name (e.g., "nvme0n1", "sda", "ramdisk0").
    fn name(&self) -> &str;

    /// Returns the underlying hardware interface type.
    fn device_type(&self) -> DeviceType;

    /// Queries the operational status of the device.
    fn status(&self) -> DeviceStatus;

    /// Logical block size in bytes (e.g., 512 or 4096).
    fn block_size(&self) -> u32;

    /// Total number of logical blocks on this storage device.
    fn block_count(&self) -> u64;

    /// Total device capacity in bytes.
    fn total_bytes(&self) -> u64 {
        self.block_count().saturating_mul(self.block_size() as u64)
    }

    /// Returns true if the device is read-only.
    fn is_read_only(&self) -> bool {
        false
    }

    /// Synchronously or cooperatively reads contiguous blocks starting at `lba` into `buf`.
    ///
    /// The buffer length MUST be a multiple of `block_size()`.
    /// Returns the number of bytes read on success.
    fn read_blocks(&self, lba: u64, buf: &mut [u8]) -> Result<usize, BlockDeviceError>;

    /// Synchronously or cooperatively writes contiguous blocks from `buf` starting at `lba`.
    ///
    /// The buffer length MUST be a multiple of `block_size()`.
    /// Returns the number of bytes written on success.
    fn write_blocks(&self, lba: u64, buf: &[u8]) -> Result<usize, BlockDeviceError>;

    /// Flushes any internal volatile controller caches to persistent storage.
    fn flush(&self) -> Result<(), BlockDeviceError>;

    /// Returns a structured metadata snapshot of this device.
    fn info(&self) -> BlockDeviceInfo {
        BlockDeviceInfo {
            id: self.id(),
            name: self.name().to_string(),
            device_type: self.device_type(),
            status: self.status(),
            block_size: self.block_size(),
            block_count: self.block_count(),
            total_bytes: self.total_bytes(),
            read_only: self.is_read_only(),
        }
    }
}

// ============================================================================
// Block Device Registry
// ============================================================================

/// Thread-safe registry for block device discovery, enumeration, and routing.
pub struct BlockDeviceRegistry {
    devices: RwLock<HashMap<u32, Arc<dyn BlockDevice>>>,
    name_to_id: RwLock<HashMap<String, u32>>,
    next_auto_id: AtomicU32,
}

impl BlockDeviceRegistry {
    pub fn new() -> Self {
        Self {
            devices: RwLock::new(HashMap::new()),
            name_to_id: RwLock::new(HashMap::new()),
            next_auto_id: AtomicU32::new(1),
        }
    }

    /// Register a block device. If the device ID is 0, an auto-incremented ID is assigned.
    pub fn register(&self, device: Arc<dyn BlockDevice>) -> Result<u32, BlockDeviceError> {
        let name = device.name().to_string();
        let mut name_map = self.name_to_id.write().map_err(|_| BlockDeviceError::HardwareFault("Registry lock poisoned".into()))?;
        if name_map.contains_key(&name) {
            return Err(BlockDeviceError::AlreadyExists);
        }

        let id = if device.id() == 0 {
            self.next_auto_id.fetch_add(1, Ordering::SeqCst)
        } else {
            device.id()
        };

        let mut dev_map = self.devices.write().map_err(|_| BlockDeviceError::HardwareFault("Registry lock poisoned".into()))?;
        if dev_map.contains_key(&id) {
            return Err(BlockDeviceError::AlreadyExists);
        }

        name_map.insert(name, id);
        dev_map.insert(id, device);

        Ok(id)
    }

    /// Register a device with an explicit unique ID.
    pub fn register_with_id(&self, id: u32, device: Arc<dyn BlockDevice>) -> Result<(), BlockDeviceError> {
        let name = device.name().to_string();
        let mut name_map = self.name_to_id.write().map_err(|_| BlockDeviceError::HardwareFault("Registry lock poisoned".into()))?;
        if name_map.contains_key(&name) {
            return Err(BlockDeviceError::AlreadyExists);
        }

        let mut dev_map = self.devices.write().map_err(|_| BlockDeviceError::HardwareFault("Registry lock poisoned".into()))?;
        if dev_map.contains_key(&id) {
            return Err(BlockDeviceError::AlreadyExists);
        }

        name_map.insert(name, id);
        dev_map.insert(id, device);

        Ok(())
    }

    /// Unregisters and returns the device with the given ID.
    pub fn unregister(&self, id: u32) -> Result<Arc<dyn BlockDevice>, BlockDeviceError> {
        let mut dev_map = self.devices.write().map_err(|_| BlockDeviceError::HardwareFault("Registry lock poisoned".into()))?;
        let device = dev_map.remove(&id).ok_or(BlockDeviceError::NotFound)?;

        let mut name_map = self.name_to_id.write().map_err(|_| BlockDeviceError::HardwareFault("Registry lock poisoned".into()))?;
        name_map.remove(device.name());

        Ok(device)
    }

    /// Retrieves an Arc reference to the device by numeric ID.
    pub fn get_device(&self, id: u32) -> Option<Arc<dyn BlockDevice>> {
        let dev_map = self.devices.read().ok()?;
        dev_map.get(&id).cloned()
    }

    /// Retrieves an Arc reference to the device by canonical name.
    pub fn get_device_by_name(&self, name: &str) -> Option<Arc<dyn BlockDevice>> {
        let name_map = self.name_to_id.read().ok()?;
        let id = name_map.get(name)?;
        self.get_device(*id)
    }

    /// Returns a list of all registered block device metadata snapshots.
    pub fn list_devices(&self) -> Vec<BlockDeviceInfo> {
        let dev_map = match self.devices.read() {
            Ok(map) => map,
            Err(_) => return Vec::new(),
        };

        dev_map.values().map(|dev| dev.info()).collect()
    }

    /// Total count of registered devices.
    pub fn count(&self) -> usize {
        self.devices.read().map(|m| m.len()).unwrap_or(0)
    }

    /// Clears all registered devices from the registry.
    pub fn clear(&self) {
        if let Ok(mut dev_map) = self.devices.write() {
            dev_map.clear();
        }
        if let Ok(mut name_map) = self.name_to_id.write() {
            name_map.clear();
        }
    }
}

/// Global singleton block device registry.
static GLOBAL_REGISTRY: OnceLock<BlockDeviceRegistry> = OnceLock::new();

/// Returns a reference to the global kernel block device registry.
pub fn get_global_registry() -> &'static BlockDeviceRegistry {
    GLOBAL_REGISTRY.get_or_init(BlockDeviceRegistry::new)
}

// ============================================================================
// RAM Block Device Reference Implementation
// ============================================================================

/// In-memory RAM disk implementation of the [`BlockDevice`] trait.
pub struct RamBlockDevice {
    id: u32,
    name: String,
    block_size: u32,
    block_count: u64,
    read_only: bool,
    data: RwLock<Vec<u8>>,
    status: RwLock<DeviceStatus>,
}

impl RamBlockDevice {
    /// Creates a new RAM disk with the specified parameters.
    pub fn new(id: u32, name: &str, block_size: u32, block_count: u64) -> Self {
        assert!(block_size > 0 && (block_size & (block_size - 1)) == 0, "block_size must be a power of two");
        let total_size = (block_count as usize).saturating_mul(block_size as usize);
        Self {
            id,
            name: name.to_string(),
            block_size,
            block_count,
            read_only: false,
            data: RwLock::new(vec![0u8; total_size]),
            status: RwLock::new(DeviceStatus::Online),
        }
    }

    /// Creates a read-only RAM disk initialized with the given byte slice.
    pub fn from_image(id: u32, name: &str, block_size: u32, image: &[u8], read_only: bool) -> Self {
        assert!(block_size > 0 && (block_size & (block_size - 1)) == 0, "block_size must be a power of two");
        let block_count = ((image.len() + block_size as usize - 1) / block_size as usize) as u64;
        let mut data = vec![0u8; (block_count as usize) * (block_size as usize)];
        data[..image.len()].copy_from_slice(image);

        Self {
            id,
            name: name.to_string(),
            block_size,
            block_count,
            read_only,
            data: RwLock::new(data),
            status: RwLock::new(DeviceStatus::Online),
        }
    }

    /// Sets the device status.
    pub fn set_status(&self, status: DeviceStatus) {
        if let Ok(mut s) = self.status.write() {
            *s = status;
        }
    }
}

impl BlockDevice for RamBlockDevice {
    fn id(&self) -> u32 {
        self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn device_type(&self) -> DeviceType {
        DeviceType::RamDisk
    }

    fn status(&self) -> DeviceStatus {
        self.status.read().map(|s| s.clone()).unwrap_or(DeviceStatus::Error("Lock poisoned".into()))
    }

    fn block_size(&self) -> u32 {
        self.block_size
    }

    fn block_count(&self) -> u64 {
        self.block_count
    }

    fn is_read_only(&self) -> bool {
        self.read_only
    }

    fn read_blocks(&self, lba: u64, buf: &mut [u8]) -> Result<usize, BlockDeviceError> {
        let status = self.status();
        if !status.is_ready() {
            return Err(BlockDeviceError::DeviceOffline);
        }

        if buf.is_empty() {
            return Ok(0);
        }

        if buf.len() % (self.block_size as usize) != 0 {
            return Err(BlockDeviceError::UnalignedBuffer {
                buffer_len: buf.len(),
                block_size: self.block_size,
            });
        }

        let num_blocks = (buf.len() / self.block_size as usize) as u64;
        if lba.saturating_add(num_blocks) > self.block_count {
            return Err(BlockDeviceError::InvalidLba {
                lba: lba + num_blocks,
                max_lba: self.block_count,
            });
        }

        let start_byte = (lba as usize) * (self.block_size as usize);
        let end_byte = start_byte + buf.len();

        let storage = self.data.read().map_err(|_| BlockDeviceError::HardwareFault("RAM disk read lock poisoned".into()))?;
        buf.copy_from_slice(&storage[start_byte..end_byte]);

        Ok(buf.len())
    }

    fn write_blocks(&self, lba: u64, buf: &[u8]) -> Result<usize, BlockDeviceError> {
        let status = self.status();
        if !status.is_ready() {
            return Err(BlockDeviceError::DeviceOffline);
        }

        if self.read_only {
            return Err(BlockDeviceError::WriteProtected);
        }

        if buf.is_empty() {
            return Ok(0);
        }

        if buf.len() % (self.block_size as usize) != 0 {
            return Err(BlockDeviceError::UnalignedBuffer {
                buffer_len: buf.len(),
                block_size: self.block_size,
            });
        }

        let num_blocks = (buf.len() / self.block_size as usize) as u64;
        if lba.saturating_add(num_blocks) > self.block_count {
            return Err(BlockDeviceError::InvalidLba {
                lba: lba + num_blocks,
                max_lba: self.block_count,
            });
        }

        let start_byte = (lba as usize) * (self.block_size as usize);
        let end_byte = start_byte + buf.len();

        let mut storage = self.data.write().map_err(|_| BlockDeviceError::HardwareFault("RAM disk write lock poisoned".into()))?;
        storage[start_byte..end_byte].copy_from_slice(buf);

        Ok(buf.len())
    }

    fn flush(&self) -> Result<(), BlockDeviceError> {
        // RAM disk is always memory-coherent
        Ok(())
    }
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ram_block_device_lifecycle() {
        let ramdisk = RamBlockDevice::new(101, "ram0", 512, 128); // 64 KB
        assert_eq!(ramdisk.id(), 101);
        assert_eq!(ramdisk.name(), "ram0");
        assert_eq!(ramdisk.block_size(), 512);
        assert_eq!(ramdisk.block_count(), 128);
        assert_eq!(ramdisk.total_bytes(), 65536);
        assert_eq!(ramdisk.device_type(), DeviceType::RamDisk);
        assert_eq!(ramdisk.status(), DeviceStatus::Online);

        // Write to LBA 2
        let mut write_buf = vec![0xABu8; 512 * 2]; // 2 blocks
        write_buf[0..4].copy_from_slice(&[0x11, 0x22, 0x33, 0x44]);
        let written = ramdisk.write_blocks(2, &write_buf).unwrap();
        assert_eq!(written, 1024);

        // Read back from LBA 2
        let mut read_buf = vec![0u8; 1024];
        let bytes_read = ramdisk.read_blocks(2, &mut read_buf).unwrap();
        assert_eq!(bytes_read, 1024);
        assert_eq!(&read_buf[0..4], &[0x11, 0x22, 0x33, 0x44]);
        assert_eq!(read_buf[10], 0xAB);

        // Verify LBA bounds check
        let mut oob_buf = vec![0u8; 512];
        let err = ramdisk.read_blocks(128, &mut oob_buf).unwrap_err();
        match err {
            BlockDeviceError::InvalidLba { lba, max_lba } => {
                assert_eq!(lba, 129);
                assert_eq!(max_lba, 128);
            }
            _ => panic!("Expected InvalidLba error"),
        }
    }

    #[test]
    fn test_unaligned_buffer_error() {
        let ramdisk = RamBlockDevice::new(102, "ram1", 512, 10);
        let mut unaligned_buf = vec![0u8; 300];
        let err = ramdisk.read_blocks(0, &mut unaligned_buf).unwrap_err();
        assert_eq!(err, BlockDeviceError::UnalignedBuffer { buffer_len: 300, block_size: 512 });
    }

    #[test]
    fn test_block_device_registry() {
        let registry = BlockDeviceRegistry::new();
        let dev1 = Arc::new(RamBlockDevice::new(1, "disk_a", 512, 100));
        let dev2 = Arc::new(RamBlockDevice::new(2, "disk_b", 4096, 50));

        assert_eq!(registry.register(dev1).unwrap(), 1);
        assert_eq!(registry.register(dev2).unwrap(), 2);
        assert_eq!(registry.count(), 2);

        // Duplicate registration check
        let dev_dup = Arc::new(RamBlockDevice::new(1, "disk_a", 512, 10));
        assert_eq!(registry.register(dev_dup).unwrap_err(), BlockDeviceError::AlreadyExists);

        // Query by ID and Name
        let found1 = registry.get_device(1).expect("Device 1 must exist");
        assert_eq!(found1.name(), "disk_a");

        let found2 = registry.get_device_by_name("disk_b").expect("Device disk_b must exist");
        assert_eq!(found2.id(), 2);
        assert_eq!(found2.block_size(), 4096);

        // List devices
        let list = registry.list_devices();
        assert_eq!(list.len(), 2);

        // Unregister
        let unreg = registry.unregister(1).unwrap();
        assert_eq!(unreg.name(), "disk_a");
        assert_eq!(registry.count(), 1);
        assert!(registry.get_device(1).is_none());
        assert!(registry.get_device_by_name("disk_a").is_none());
    }

    #[test]
    fn test_read_only_ram_device() {
        let data = vec![0x55u8; 1024];
        let ro_disk = RamBlockDevice::from_image(103, "ro_img", 512, &data, true);
        assert!(ro_disk.is_read_only());

        let mut read_buf = vec![0u8; 512];
        ro_disk.read_blocks(0, &mut read_buf).unwrap();
        assert_eq!(read_buf[0], 0x55);

        let write_buf = vec![0xAAu8; 512];
        let err = ro_disk.write_blocks(0, &write_buf).unwrap_err();
        assert_eq!(err, BlockDeviceError::WriteProtected);
    }
}
