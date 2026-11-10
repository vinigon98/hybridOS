//! HybridOS Storage Subsystem
//!
//! Provides comprehensive partitioning and physical/logical filesystem drivers:
//! - [`gpt`]: GUID Partition Table (GPT) & Protective MBR parser and disk layout.
//! - [`fat32`]: Full FAT32 driver (BPB, FAT chains, LFN, read, write, directory operations).
//! - [`ext4`]: Core ext4 driver (Superblock, Block Groups, Inodes, Extents, Directories).
//! - [`BlockDevice`]: Universal block storage trait implemented for disks, partitions, and RAM disks.
//! - VFS Integration: Direct mounting of real FAT32 and EXT4 volumes into the unified HybridOS VFS.

pub mod block_device;
pub mod nvme;
pub mod ahci;
pub mod gpt;
pub mod fat32;
pub mod ext4;

pub use block_device::{
    BlockDevice as HwBlockDevice, BlockDeviceError, BlockDeviceInfo, BlockDeviceRegistry,
    DeviceStatus, DeviceType, RamBlockDevice, get_global_registry,
};
pub use nvme::{
    NvmeDevice, MockNvmeController, NvmeQueuePair, NvmeSqe, NvmeCqe,
    NvmeIdentifyController, NvmeIdentifyNamespace, PrpBuilder,
};
pub use ahci::{
    AhciDevice, MockAhciController, AhciCommandHeader, AhciPrdtEntry,
    AhciCommandTable, FisRegH2D, AtaIdentifyInfo,
};

use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::collections::HashMap;

use crate::vfs::{
    DirEntry, FileOperations, Inode, InodeType, Permissions, VfsError, VirtualFileSystem,
};
use crate::drivers::hardware::VirtioBlockDevice;

/// Comprehensive Storage subsystem errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StorageError {
    NotFound,
    AlreadyExists,
    NoSpace,
    InvalidData(&'static str),
    IoError(&'static str),
    CrcMismatch { expected: u32, computed: u32 },
    NotADirectory,
    IsADirectory,
    DirectoryNotEmpty,
    Unsupported,
}

impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StorageError::NotFound => write!(f, "Storage entity not found"),
            StorageError::AlreadyExists => write!(f, "Storage entity already exists"),
            StorageError::NoSpace => write!(f, "No space remaining on storage device"),
            StorageError::InvalidData(msg) => write!(f, "Storage data invalid or corrupted: {}", msg),
            StorageError::IoError(msg) => write!(f, "Block I/O device error: {}", msg),
            StorageError::CrcMismatch { expected, computed } => {
                write!(f, "Storage CRC32 mismatch: expected 0x{:08X}, computed 0x{:08X}", expected, computed)
            }
            StorageError::NotADirectory => write!(f, "Target is not a directory"),
            StorageError::IsADirectory => write!(f, "Target is a directory"),
            StorageError::DirectoryNotEmpty => write!(f, "Directory is not empty"),
            StorageError::Unsupported => write!(f, "Operation unsupported on this filesystem"),
        }
    }
}

impl std::error::Error for StorageError {}

impl From<StorageError> for VfsError {
    fn from(err: StorageError) -> Self {
        match err {
            StorageError::NotFound => VfsError::NotFound,
            StorageError::AlreadyExists => VfsError::AlreadyExists,
            StorageError::NoSpace => VfsError::NoSpace,
            StorageError::InvalidData(_) => VfsError::IoError,
            StorageError::IoError(_) => VfsError::IoError,
            StorageError::CrcMismatch { .. } => VfsError::IoError,
            StorageError::NotADirectory => VfsError::NotADirectory,
            StorageError::IsADirectory => VfsError::IsADirectory,
            StorageError::DirectoryNotEmpty => VfsError::DirectoryNotEmpty,
            StorageError::Unsupported => VfsError::NotSupported,
        }
    }
}

/// Universal Block Device Trait.
pub trait BlockDevice: Send + Sync {
    /// Sector size in bytes (typically 512 or 4096).
    fn sector_size(&self) -> usize {
        512
    }

    /// Total capacity in sectors.
    fn total_sectors(&self) -> u64;

    /// Read raw sectors starting at `start_lba`.
    fn read_sectors(&self, start_lba: u64, buf: &mut [u8]) -> Result<usize, StorageError>;

    /// Write raw sectors starting at `start_lba`.
    fn write_sectors(&self, start_lba: u64, buf: &[u8]) -> Result<usize, StorageError>;

    /// Read arbitrary byte range from device (translating to sector operations).
    fn read_bytes(&self, byte_offset: u64, buf: &mut [u8]) -> Result<usize, StorageError> {
        let sec_size = self.sector_size() as u64;
        let start_sec = byte_offset / sec_size;
        let end_byte = byte_offset + (buf.len() as u64);
        let end_sec = (end_byte + sec_size - 1) / sec_size;
        let num_secs = end_sec - start_sec;

        let mut temp = vec![0u8; (num_secs * sec_size) as usize];
        self.read_sectors(start_sec, &mut temp)?;

        let offset_in_sec = (byte_offset % sec_size) as usize;
        buf.copy_from_slice(&temp[offset_in_sec..offset_in_sec + buf.len()]);
        Ok(buf.len())
    }

    /// Write arbitrary byte range to device (read-modify-write as needed).
    fn write_bytes(&self, byte_offset: u64, buf: &[u8]) -> Result<usize, StorageError> {
        let sec_size = self.sector_size() as u64;
        let start_sec = byte_offset / sec_size;
        let end_byte = byte_offset + (buf.len() as u64);
        let end_sec = (end_byte + sec_size - 1) / sec_size;
        let num_secs = end_sec - start_sec;

        let mut temp = vec![0u8; (num_secs * sec_size) as usize];
        self.read_sectors(start_sec, &mut temp)?;

        let offset_in_sec = (byte_offset % sec_size) as usize;
        temp[offset_in_sec..offset_in_sec + buf.len()].copy_from_slice(buf);
        self.write_sectors(start_sec, &temp)?;
        Ok(buf.len())
    }
}

/// Sparse In-Memory RAM Disk for testing and temporary block volumes.
pub struct RamDisk {
    sectors: RwLock<HashMap<u64, [u8; 512]>>,
    total_sectors: u64,
}

impl RamDisk {
    pub fn new(total_sectors: u64) -> Self {
        Self {
            sectors: RwLock::new(HashMap::new()),
            total_sectors,
        }
    }

    pub fn from_image(data: &[u8]) -> Self {
        let total_sectors = ((data.len() + 511) / 512) as u64;
        let mut map = HashMap::new();

        for (i, chunk) in data.chunks(512).enumerate() {
            let mut sec = [0u8; 512];
            sec[..chunk.len()].copy_from_slice(chunk);
            map.insert(i as u64, sec);
        }

        Self {
            sectors: RwLock::new(map),
            total_sectors,
        }
    }
}

impl BlockDevice for RamDisk {
    fn sector_size(&self) -> usize {
        512
    }

    fn total_sectors(&self) -> u64 {
        self.total_sectors
    }

    fn read_sectors(&self, start_lba: u64, buf: &mut [u8]) -> Result<usize, StorageError> {
        if buf.len() % 512 != 0 {
            return Err(StorageError::InvalidData("Buffer length must be multiple of 512"));
        }

        let num_sectors = (buf.len() / 512) as u64;
        if start_lba + num_sectors > self.total_sectors {
            return Err(StorageError::IoError("Sector address out of range"));
        }

        let guard = self.sectors.read().unwrap();
        for i in 0..num_sectors {
            let lba = start_lba + i;
            let offset = (i as usize) * 512;
            if let Some(sec) = guard.get(&lba) {
                buf[offset..offset + 512].copy_from_slice(sec);
            } else {
                buf[offset..offset + 512].fill(0);
            }
        }

        Ok(buf.len())
    }

    fn write_sectors(&self, start_lba: u64, buf: &[u8]) -> Result<usize, StorageError> {
        if buf.len() % 512 != 0 {
            return Err(StorageError::InvalidData("Buffer length must be multiple of 512"));
        }

        let num_sectors = (buf.len() / 512) as u64;
        if start_lba + num_sectors > self.total_sectors {
            return Err(StorageError::IoError("Sector address out of range"));
        }

        let mut guard = self.sectors.write().unwrap();
        for i in 0..num_sectors {
            let lba = start_lba + i;
            let offset = (i as usize) * 512;
            let mut sec = [0u8; 512];
            sec.copy_from_slice(&buf[offset..offset + 512]);
            guard.insert(lba, sec);
        }

        Ok(buf.len())
    }
}

/// Sub-range Partition Block Device exposing a partition as an independent BlockDevice.
pub struct PartitionBlockDevice {
    inner: Arc<dyn BlockDevice>,
    start_lba: u64,
    sector_count: u64,
}

impl PartitionBlockDevice {
    pub fn new(inner: Arc<dyn BlockDevice>, start_lba: u64, sector_count: u64) -> Self {
        Self {
            inner,
            start_lba,
            sector_count,
        }
    }

    pub fn start_lba(&self) -> u64 {
        self.start_lba
    }
}

impl BlockDevice for PartitionBlockDevice {
    fn sector_size(&self) -> usize {
        self.inner.sector_size()
    }

    fn total_sectors(&self) -> u64 {
        self.sector_count
    }

    fn read_sectors(&self, start_lba: u64, buf: &mut [u8]) -> Result<usize, StorageError> {
        let sec_size = self.sector_size();
        let num_sectors = (buf.len() / sec_size) as u64;
        if start_lba + num_sectors > self.sector_count {
            return Err(StorageError::IoError("Partition read out of bounds"));
        }
        self.inner.read_sectors(self.start_lba + start_lba, buf)
    }

    fn write_sectors(&self, start_lba: u64, buf: &[u8]) -> Result<usize, StorageError> {
        let sec_size = self.sector_size();
        let num_sectors = (buf.len() / sec_size) as u64;
        if start_lba + num_sectors > self.sector_count {
            return Err(StorageError::IoError("Partition write out of bounds"));
        }
        self.inner.write_sectors(self.start_lba + start_lba, buf)
    }
}

/// Implement BlockDevice for physical VirtIO block device.
impl BlockDevice for VirtioBlockDevice {
    fn sector_size(&self) -> usize {
        512
    }

    fn total_sectors(&self) -> u64 {
        self.config.capacity_sectors
    }

    fn read_sectors(&self, start_lba: u64, buf: &mut [u8]) -> Result<usize, StorageError> {
        VirtioBlockDevice::read_sectors(self, start_lba, buf)
            .map_err(|e| StorageError::IoError(e))
    }

    fn write_sectors(&self, start_lba: u64, buf: &[u8]) -> Result<usize, StorageError> {
        VirtioBlockDevice::write_sectors(self, start_lba, buf)
            .map_err(|e| StorageError::IoError(e))
    }
}

/// Adapter to expose any hardware [`HwBlockDevice`] as a filesystem [`BlockDevice`].
pub struct BlockDeviceFsAdapter {
    inner: Arc<dyn HwBlockDevice>,
}

impl BlockDeviceFsAdapter {
    pub fn new(inner: Arc<dyn HwBlockDevice>) -> Self {
        Self { inner }
    }
}

impl BlockDevice for BlockDeviceFsAdapter {
    fn sector_size(&self) -> usize {
        self.inner.block_size() as usize
    }

    fn total_sectors(&self) -> u64 {
        self.inner.block_count()
    }

    fn read_sectors(&self, start_lba: u64, buf: &mut [u8]) -> Result<usize, StorageError> {
        self.inner.read_blocks(start_lba, buf)
            .map_err(|_| StorageError::IoError("Hardware block read error"))
    }

    fn write_sectors(&self, start_lba: u64, buf: &[u8]) -> Result<usize, StorageError> {
        self.inner.write_blocks(start_lba, buf)
            .map_err(|_| StorageError::IoError("Hardware block write error"))
    }
}

// ============================================================================
// VFS Mounting Integration: FAT32 & EXT4 Bridges
// ============================================================================

/// VFS FileOperations implementation for a FAT32 file.
pub struct Fat32VfsFileOps {
    fs: Arc<fat32::Fat32FileSystem>,
    first_cluster: AtomicU64,
    size: AtomicU64,
    parent_dir_cluster: u32,
    dir_entry_offset: u64,
}

impl FileOperations for Fat32VfsFileOps {
    fn read(&self, offset: u64, buf: &mut [u8]) -> Result<usize, VfsError> {
        let first_clus = self.first_cluster.load(Ordering::SeqCst) as u32;
        let file_sz = self.size.load(Ordering::SeqCst);
        self.fs
            .read_file_at(first_clus, offset, buf, file_sz)
            .map_err(Into::into)
    }

    fn write(&self, offset: u64, buf: &[u8]) -> Result<usize, VfsError> {
        let first_clus = self.first_cluster.load(Ordering::SeqCst) as u32;
        let current_sz = self.size.load(Ordering::SeqCst);

        let (new_first_clus, new_sz) = self
            .fs
            .write_file_at(first_clus, offset, buf, current_sz)?;

        self.first_cluster.store(new_first_clus as u64, Ordering::SeqCst);
        self.size.store(new_sz, Ordering::SeqCst);

        // Update directory entry on disk
        let _ = self.fs.update_entry_metadata(
            self.parent_dir_cluster,
            self.dir_entry_offset,
            new_first_clus,
            new_sz as u32,
        );

        Ok(buf.len())
    }
}

/// VFS FileOperations implementation for a FAT32 directory.
pub struct Fat32VfsDirOps {
    fs: Arc<fat32::Fat32FileSystem>,
    cluster: u32,
    ino_allocator: Arc<AtomicU64>,
}

impl Fat32VfsDirOps {
    pub fn new(fs: Arc<fat32::Fat32FileSystem>, cluster: u32, ino_allocator: Arc<AtomicU64>) -> Self {
        Self {
            fs,
            cluster,
            ino_allocator,
        }
    }
}

impl FileOperations for Fat32VfsDirOps {
    fn readdir(&self) -> Result<Vec<DirEntry>, VfsError> {
        let fat_entries = self.fs.readdir(self.cluster)?;
        let mut result = Vec::new();

        for entry in fat_entries {
            let ino_type = if entry.is_directory {
                InodeType::Directory
            } else {
                InodeType::File
            };
            result.push(DirEntry {
                name: entry.name,
                inode_id: entry.first_cluster as u64,
                ino_type,
                size: entry.file_size as u64,
            });
        }

        Ok(result)
    }

    fn lookup(&self, name: &str) -> Result<Arc<Inode>, VfsError> {
        let fat_entries = self.fs.readdir(self.cluster)?;
        let target = fat_entries
            .into_iter()
            .find(|e| e.name.eq_ignore_ascii_case(name))
            .ok_or(VfsError::NotFound)?;

        let ino_id = self.ino_allocator.fetch_add(1, Ordering::SeqCst);

        if target.is_directory {
            let dir_ops = Arc::new(Fat32VfsDirOps::new(
                self.fs.clone(),
                target.first_cluster,
                self.ino_allocator.clone(),
            ));
            let inode = Arc::new(Inode::new(
                ino_id,
                InodeType::Directory,
                Permissions::default_dir(),
                dir_ops,
            ));
            Ok(inode)
        } else {
            let file_ops = Arc::new(Fat32VfsFileOps {
                fs: self.fs.clone(),
                first_cluster: AtomicU64::new(target.first_cluster as u64),
                size: AtomicU64::new(target.file_size as u64),
                parent_dir_cluster: self.cluster,
                dir_entry_offset: target.dir_entry_offset,
            });
            let inode = Arc::new(Inode::new(
                ino_id,
                InodeType::File,
                Permissions::default_file(),
                file_ops,
            ));
            inode.set_size(target.file_size as u64);
            Ok(inode)
        }
    }

    fn create(&self, name: &str, ino_type: InodeType, _perms: Permissions) -> Result<Arc<Inode>, VfsError> {
        if ino_type == InodeType::Directory {
            return self.mkdir(name, _perms);
        }

        let entry = self.fs.create_file(self.cluster, name, fat32::fat_attrs::ARCHIVE)?;
        let ino_id = self.ino_allocator.fetch_add(1, Ordering::SeqCst);

        let file_ops = Arc::new(Fat32VfsFileOps {
            fs: self.fs.clone(),
            first_cluster: AtomicU64::new(0),
            size: AtomicU64::new(0),
            parent_dir_cluster: self.cluster,
            dir_entry_offset: entry.dir_entry_offset,
        });

        Ok(Arc::new(Inode::new(
            ino_id,
            InodeType::File,
            Permissions::default_file(),
            file_ops,
        )))
    }

    fn mkdir(&self, name: &str, _perms: Permissions) -> Result<Arc<Inode>, VfsError> {
        let entry = self.fs.mkdir(self.cluster, name)?;
        let ino_id = self.ino_allocator.fetch_add(1, Ordering::SeqCst);

        let dir_ops = Arc::new(Fat32VfsDirOps::new(
            self.fs.clone(),
            entry.first_cluster,
            self.ino_allocator.clone(),
        ));

        Ok(Arc::new(Inode::new(
            ino_id,
            InodeType::Directory,
            Permissions::default_dir(),
            dir_ops,
        )))
    }

    fn unlink(&self, name: &str) -> Result<(), VfsError> {
        self.fs.unlink(self.cluster, name).map_err(Into::into)
    }

    fn rmdir(&self, name: &str) -> Result<(), VfsError> {
        self.fs.unlink(self.cluster, name).map_err(Into::into)
    }
}

/// VFS FileOperations implementation for an EXT4 file.
pub struct Ext4VfsFileOps {
    fs: Arc<ext4::Ext4FileSystem>,
    inode: ext4::Ext4Inode,
}

impl FileOperations for Ext4VfsFileOps {
    fn read(&self, offset: u64, buf: &mut [u8]) -> Result<usize, VfsError> {
        self.fs
            .read_inode_data(&self.inode, offset, buf)
            .map_err(Into::into)
    }
}

/// VFS FileOperations implementation for an EXT4 directory.
pub struct Ext4VfsDirOps {
    fs: Arc<ext4::Ext4FileSystem>,
    ino: u32,
    ino_allocator: Arc<AtomicU64>,
}

impl Ext4VfsDirOps {
    pub fn new(fs: Arc<ext4::Ext4FileSystem>, ino: u32, ino_allocator: Arc<AtomicU64>) -> Self {
        Self {
            fs,
            ino,
            ino_allocator,
        }
    }
}

impl FileOperations for Ext4VfsDirOps {
    fn readdir(&self) -> Result<Vec<DirEntry>, VfsError> {
        let dir_inode = self.fs.read_inode(self.ino)?;
        let entries = self.fs.readdir(&dir_inode)?;
        let mut result = Vec::new();

        for e in entries {
            let ino_type = if e.file_type == ext4::ext4_file_types::EXT4_FT_DIR {
                InodeType::Directory
            } else {
                InodeType::File
            };
            result.push(DirEntry {
                name: e.name,
                inode_id: e.inode as u64,
                ino_type,
                size: 0,
            });
        }

        Ok(result)
    }

    fn lookup(&self, name: &str) -> Result<Arc<Inode>, VfsError> {
        let dir_inode = self.fs.read_inode(self.ino)?;
        let entries = self.fs.readdir(&dir_inode)?;
        let target = entries
            .into_iter()
            .find(|e| e.name == name)
            .ok_or(VfsError::NotFound)?;

        let child_inode = self.fs.read_inode(target.inode)?;
        let vfs_ino_id = self.ino_allocator.fetch_add(1, Ordering::SeqCst);

        if child_inode.is_dir() {
            let dir_ops = Arc::new(Ext4VfsDirOps::new(
                self.fs.clone(),
                child_inode.ino,
                self.ino_allocator.clone(),
            ));
            Ok(Arc::new(Inode::new(
                vfs_ino_id,
                InodeType::Directory,
                Permissions::from_mode(child_inode.i_mode as u32),
                dir_ops,
            )))
        } else {
            let size = child_inode.i_size;
            let file_ops = Arc::new(Ext4VfsFileOps {
                fs: self.fs.clone(),
                inode: child_inode,
            });
            let node = Arc::new(Inode::new(
                vfs_ino_id,
                InodeType::File,
                Permissions::from_mode(0o644),
                file_ops,
            ));
            node.set_size(size);
            Ok(node)
        }
    }
}

/// Mount a FAT32 filesystem at the specified VFS path.
pub fn mount_fat32(
    vfs: &VirtualFileSystem,
    mount_path: &str,
    device: Arc<dyn BlockDevice>,
) -> Result<(), VfsError> {
    let fs = Arc::new(fat32::Fat32FileSystem::mount(device)?);
    let root_cluster = fs.bpb().root_cluster;

    let canon = vfs.resolve_path(mount_path, "/")?;
    if canon.is_root() {
        return Err(VfsError::InvalidArgument);
    }

    // Ensure parent directory exists
    let parent_canon = canon.parent().unwrap_or_else(crate::vfs::CanonicalPath::root);
    let _ = vfs.create_dir_all(&parent_canon.as_unix_path(), Permissions::default_dir());

    let (parent_dir, leaf) = vfs.lookup_parent(&canon)?;
    let _ = parent_dir.rmdir(&leaf);
    let _ = parent_dir.unlink(&leaf);

    let ino_allocator = Arc::new(AtomicU64::new(50000));
    let mount_ops = Arc::new(Fat32VfsDirOps::new(fs, root_cluster, ino_allocator));
    let mount_inode = Arc::new(Inode::new(
        40001,
        InodeType::Directory,
        Permissions::default_dir(),
        mount_ops,
    ));

    parent_dir.add_entry(&leaf, mount_inode)?;
    Ok(())
}

/// Mount an EXT4 filesystem at the specified VFS path.
pub fn mount_ext4(
    vfs: &VirtualFileSystem,
    mount_path: &str,
    device: Arc<dyn BlockDevice>,
) -> Result<(), VfsError> {
    let fs = Arc::new(ext4::Ext4FileSystem::mount(device)?);

    let canon = vfs.resolve_path(mount_path, "/")?;
    if canon.is_root() {
        return Err(VfsError::InvalidArgument);
    }

    let parent_canon = canon.parent().unwrap_or_else(crate::vfs::CanonicalPath::root);
    let _ = vfs.create_dir_all(&parent_canon.as_unix_path(), Permissions::default_dir());

    let (parent_dir, leaf) = vfs.lookup_parent(&canon)?;
    let _ = parent_dir.rmdir(&leaf);
    let _ = parent_dir.unlink(&leaf);

    let ino_allocator = Arc::new(AtomicU64::new(60000));
    let mount_ops = Arc::new(Ext4VfsDirOps::new(fs, ext4::EXT4_ROOT_INO, ino_allocator));
    let mount_inode = Arc::new(Inode::new(
        40002,
        InodeType::Directory,
        Permissions::default_dir(),
        mount_ops,
    ));

    parent_dir.add_entry(&leaf, mount_inode)?;
    Ok(())
}

/// Inspect GPT partitioning table and automatically mount supported filesystems.
pub fn mount_gpt_disk(
    vfs: &VirtualFileSystem,
    device: Arc<dyn BlockDevice>,
    base_mount_path: &str,
) -> Result<Vec<String>, VfsError> {
    let gpt_disk = gpt::GptDisk::parse_from_device(&*device)?;
    let mut mounted = Vec::new();

    for (idx, part) in gpt_disk.partitions.iter().enumerate() {
        let part_dev = Arc::new(PartitionBlockDevice::new(
            device.clone(),
            part.starting_lba,
            part.sector_count(),
        ));

        let part_mount = format!("{}/part{}", base_mount_path.trim_end_matches('/'), idx + 1);

        if part.is_windows_data() || part.is_efi_system() {
            // Attempt FAT32 mount
            if let Ok(()) = mount_fat32(vfs, &part_mount, part_dev.clone()) {
                mounted.push(part_mount);
                continue;
            }
        }

        if part.is_linux_fs() {
            // Attempt EXT4 mount
            if let Ok(()) = mount_ext4(vfs, &part_mount, part_dev.clone()) {
                mounted.push(part_mount);
                continue;
            }
        }
    }

    Ok(mounted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ramdisk_read_write() {
        let disk = RamDisk::new(1024);
        let payload = [0x5Au8; 512];
        disk.write_sectors(10, &payload).unwrap();

        let mut read_buf = [0u8; 512];
        disk.read_sectors(10, &mut read_buf).unwrap();
        assert_eq!(read_buf, payload);

        // Byte-level read/write test
        let greeting = b"HybridOS Universal Block Layer";
        disk.write_bytes(10 * 512 + 50, greeting).unwrap();

        let mut read_greeting = vec![0u8; greeting.len()];
        disk.read_bytes(10 * 512 + 50, &mut read_greeting).unwrap();
        assert_eq!(&read_greeting, greeting);
    }

    #[test]
    fn test_partition_block_device_bounds() {
        let disk = Arc::new(RamDisk::new(2048));
        let part = PartitionBlockDevice::new(disk.clone(), 100, 500);

        assert_eq!(part.total_sectors(), 500);
        let buf = [0xFFu8; 512];
        part.write_sectors(0, &buf).unwrap();

        // Verify write was offset by 100 on the parent disk
        let mut parent_buf = [0u8; 512];
        disk.read_sectors(100, &mut parent_buf).unwrap();
        assert_eq!(parent_buf, buf);

        // Out of bounds check
        let mut out_buf = [0u8; 1024];
        let err = part.read_sectors(499, &mut out_buf);
        assert!(err.is_err());
    }

    #[test]
    fn test_vfs_fat32_mount_and_file_io() {
        let vfs = VirtualFileSystem::new();
        let ramdisk = Arc::new(RamDisk::new(131072)); // 64MB
        let fs = fat32::Fat32FileSystem::format(ramdisk.clone(), "SYSTEM_VOL").unwrap();

        // Populate FAT32 disk with a file before mounting
        let dir = fs.mkdir(fs.bpb().root_cluster, "boot").unwrap();
        let file = fs.create_file(dir.first_cluster, "grub.cfg", 0).unwrap();
        let content = b"set timeout=5\nmenuentry 'HybridOS' { boot }\n";
        let (first_c, sz) = fs.write_file_at(file.first_cluster, 0, content, 0).unwrap();
        fs.update_entry_metadata(dir.first_cluster, file.dir_entry_offset, first_c, sz as u32).unwrap();

        // Mount FAT32 partition to VFS at /mnt/fat32
        mount_fat32(&vfs, "/mnt/fat32", ramdisk.clone()).unwrap();

        // Lookup and read file via VFS path
        let read_back = vfs.read_to_vec("/mnt/fat32/boot/grub.cfg").unwrap();
        assert_eq!(&read_back, content);

        // Create new file via VFS inside mounted FAT32 volume!
        let new_file_data = b"HYBRIDOS_LIVE_KERNEL_DATA";
        vfs.create_file("/mnt/fat32/boot/kernel.bin", new_file_data, Permissions::default_file()).unwrap();

        let vfs_read_new = vfs.read_to_vec("/mnt/fat32/boot/kernel.bin").unwrap();
        assert_eq!(&vfs_read_new, new_file_data);
    }

    #[test]
    fn test_vfs_ext4_mount_and_traversal() {
        let vfs = VirtualFileSystem::new();
        let ramdisk = Arc::new(RamDisk::new(65536));
        let _fs = ext4::Ext4FileSystem::format(ramdisk.clone(), "rootfs").unwrap();

        // Mount EXT4 partition to VFS at /mnt/ext4
        mount_ext4(&vfs, "/mnt/ext4", ramdisk.clone()).unwrap();

        // Stat the mount point
        let stat = vfs.stat("/mnt/ext4").unwrap();
        assert_eq!(stat.ino_type, InodeType::Directory);

        // Read directory entries via VFS
        let entries = vfs.readdir("/mnt/ext4").unwrap();
        assert!(entries.iter().any(|e| e.name == "."));
        assert!(entries.iter().any(|e| e.name == ".."));
    }

    #[test]
    fn test_nvme_and_ahci_hardware_drivers_and_registry() {
        let registry = block_device::BlockDeviceRegistry::new();

        // 1. Setup Mock NVMe Controller
        let nvme_mock = Arc::new(std::sync::Mutex::new(nvme::MockNvmeController::new(131072, 512))); // 64 MB
        let nvme_dev = Arc::new(nvme::NvmeDevice::new(1, "nvme0n1", nvme_mock).unwrap());
        registry.register(nvme_dev.clone()).unwrap();

        // 2. Setup Mock AHCI SATA Controller
        let ahci_mock = Arc::new(std::sync::Mutex::new(ahci::MockAhciController::new(2, 65536))); // 32 MB
        let ahci_dev = Arc::new(ahci::AhciDevice::new(2, "sda", ahci_mock, 0).unwrap());
        registry.register(ahci_dev.clone()).unwrap();

        assert_eq!(registry.count(), 2);
        assert_eq!(registry.get_device(1).unwrap().name(), "nvme0n1");
        assert_eq!(registry.get_device(2).unwrap().name(), "sda");

        // 3. Format NVMe as FAT32 filesystem using BlockDeviceFsAdapter!
        let nvme_fs_adapter = Arc::new(BlockDeviceFsAdapter::new(nvme_dev));
        let fat_fs = fat32::Fat32FileSystem::format(nvme_fs_adapter.clone(), "NVME_FAT").unwrap();
        let dir = fat_fs.mkdir(fat_fs.bpb().root_cluster, "system").unwrap();
        let file = fat_fs.create_file(dir.first_cluster, "os_image.bin", 0).unwrap();
        let os_data = b"HYBRIDOS_X64_CORE_IMAGE";
        let (first_c, sz) = fat_fs.write_file_at(file.first_cluster, 0, os_data, 0).unwrap();
        fat_fs.update_entry_metadata(dir.first_cluster, file.dir_entry_offset, first_c, sz as u32).unwrap();

        // 4. Mount NVMe FAT32 volume to VFS
        let vfs = VirtualFileSystem::new();
        mount_fat32(&vfs, "/mnt/nvme", nvme_fs_adapter).unwrap();
        let read_back = vfs.read_to_vec("/mnt/nvme/system/os_image.bin").unwrap();
        assert_eq!(&read_back, os_data);

        // 5. Format AHCI SATA as EXT4 filesystem using BlockDeviceFsAdapter!
        let ahci_fs_adapter = Arc::new(BlockDeviceFsAdapter::new(ahci_dev));
        let _ext_fs = ext4::Ext4FileSystem::format(ahci_fs_adapter.clone(), "ahci_root").unwrap();
        mount_ext4(&vfs, "/mnt/ahci", ahci_fs_adapter).unwrap();
        let stat = vfs.stat("/mnt/ahci").unwrap();
        assert_eq!(stat.ino_type, InodeType::Directory);
    }
}
