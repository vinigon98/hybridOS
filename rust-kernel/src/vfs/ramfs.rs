//! Virtual File System - RAMFS (In-Memory File System)
//!
//! Provides a full-featured, thread-safe in-memory filesystem:
//! - File creation, read, write, resize, append, and truncate.
//! - Subdirectory creation, hierarchy navigation, and recursive traversal.
//! - Directory iteration (`readdir`) supporting `.`, `..`, and children.
//! - Symbolic links (`symlink`, `readlink`).
//! - Character devices (`/dev/null`, `/dev/zero`, `/dev/console`).
//! - Case-insensitive lookup fallback for Windows NT personality compatibility.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock, Weak};
use std::cmp::min;

use super::inode::{DirEntry, FileOperations, Inode, InodeType, Permissions};
use super::VfsError;

/// In-memory regular file operations backed by a dynamic byte buffer.
pub struct RamfsFileOps {
    data: RwLock<Vec<u8>>,
}

impl RamfsFileOps {
    pub fn new(initial_data: Vec<u8>) -> Self {
        Self {
            data: RwLock::new(initial_data),
        }
    }

    pub fn len(&self) -> usize {
        self.data.read().unwrap().len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.read().unwrap().is_empty()
    }
}

impl FileOperations for RamfsFileOps {
    fn read(&self, offset: u64, buf: &mut [u8]) -> Result<usize, VfsError> {
        let data = self.data.read().unwrap();
        let file_len = data.len() as u64;

        if offset >= file_len {
            return Ok(0); // EOF
        }

        let available = &data[offset as usize..];
        let bytes_to_copy = min(buf.len(), available.len());
        buf[..bytes_to_copy].copy_from_slice(&available[..bytes_to_copy]);
        Ok(bytes_to_copy)
    }

    fn write(&self, offset: u64, buf: &[u8]) -> Result<usize, VfsError> {
        let mut data = self.data.write().unwrap();
        let end_offset = (offset as usize).checked_add(buf.len()).ok_or(VfsError::NoSpace)?;

        if end_offset > data.len() {
            data.resize(end_offset, 0);
        }

        data[offset as usize..end_offset].copy_from_slice(buf);
        Ok(buf.len())
    }

    fn truncate(&self, size: u64) -> Result<(), VfsError> {
        let mut data = self.data.write().unwrap();
        data.resize(size as usize, 0);
        Ok(())
    }
}

/// In-memory directory operations supporting child entries, lookup, and mutation.
pub struct RamfsDirOps {
    self_node: RwLock<Option<Weak<Inode>>>,
    parent_node: RwLock<Option<Weak<Inode>>>,
    entries: RwLock<BTreeMap<String, Arc<Inode>>>,
    ino_allocator: Arc<AtomicU64>,
}

impl RamfsDirOps {
    pub fn new(ino_allocator: Arc<AtomicU64>) -> Self {
        Self {
            self_node: RwLock::new(None),
            parent_node: RwLock::new(None),
            entries: RwLock::new(BTreeMap::new()),
            ino_allocator,
        }
    }

    pub fn set_self(&self, inode: &Arc<Inode>) {
        *self.self_node.write().unwrap() = Some(Arc::downgrade(inode));
    }

    pub fn set_parent(&self, parent: &Arc<Inode>) {
        *self.parent_node.write().unwrap() = Some(Arc::downgrade(parent));
    }
}

impl FileOperations for RamfsDirOps {
    fn lookup(&self, name: &str) -> Result<Arc<Inode>, VfsError> {
        if name == "." {
            let guard = self.self_node.read().unwrap();
            if let Some(weak) = guard.as_ref() {
                if let Some(strong) = weak.upgrade() {
                    return Ok(strong);
                }
            }
            return Err(VfsError::NotFound);
        }

        if name == ".." {
            let guard = self.parent_node.read().unwrap();
            if let Some(weak) = guard.as_ref() {
                if let Some(strong) = weak.upgrade() {
                    return Ok(strong);
                }
            }
            // Root's parent is root itself
            let self_guard = self.self_node.read().unwrap();
            if let Some(weak) = self_guard.as_ref() {
                if let Some(strong) = weak.upgrade() {
                    return Ok(strong);
                }
            }
            return Err(VfsError::NotFound);
        }

        let entries = self.entries.read().unwrap();

        // 1. Exact match (POSIX standard)
        if let Some(node) = entries.get(name) {
            return Ok(node.clone());
        }

        // 2. Case-insensitive fallback (Windows NT compatibility)
        for (key, node) in entries.iter() {
            if key.eq_ignore_ascii_case(name) {
                return Ok(node.clone());
            }
        }

        Err(VfsError::NotFound)
    }

    fn create(&self, name: &str, ino_type: InodeType, perms: Permissions) -> Result<Arc<Inode>, VfsError> {
        validate_filename(name)?;

        let mut entries = self.entries.write().unwrap();
        if entries.contains_key(name) {
            return Err(VfsError::AlreadyExists);
        }

        let ino_id = self.ino_allocator.fetch_add(1, Ordering::SeqCst);

        let ops: Arc<dyn FileOperations> = match ino_type {
            InodeType::File => Arc::new(RamfsFileOps::new(Vec::new())),
            InodeType::CharDevice => Arc::new(RamfsNullDeviceOps),
            _ => Arc::new(RamfsFileOps::new(Vec::new())),
        };

        let inode = Arc::new(Inode::new(ino_id, ino_type, perms, ops));
        entries.insert(name.to_string(), inode.clone());
        Ok(inode)
    }

    fn mkdir(&self, name: &str, perms: Permissions) -> Result<Arc<Inode>, VfsError> {
        validate_filename(name)?;

        let mut entries = self.entries.write().unwrap();
        if entries.contains_key(name) {
            return Err(VfsError::AlreadyExists);
        }

        let ino_id = self.ino_allocator.fetch_add(1, Ordering::SeqCst);

        let dir_ops = Arc::new(RamfsDirOps::new(self.ino_allocator.clone()));
        let child_inode = Arc::new(Inode::new(ino_id, InodeType::Directory, perms, dir_ops.clone()));

        // Establish self and parent links
        dir_ops.set_self(&child_inode);
        if let Some(self_weak) = self.self_node.read().unwrap().as_ref() {
            if let Some(self_strong) = self_weak.upgrade() {
                dir_ops.set_parent(&self_strong);
            }
        }

        entries.insert(name.to_string(), child_inode.clone());
        Ok(child_inode)
    }

    fn unlink(&self, name: &str) -> Result<(), VfsError> {
        if name == "." || name == ".." {
            return Err(VfsError::InvalidArgument);
        }

        let mut entries = self.entries.write().unwrap();
        let target_key = entries.iter()
            .find(|(k, _)| k.as_str() == name || k.eq_ignore_ascii_case(name))
            .map(|(k, _)| k.clone())
            .ok_or(VfsError::NotFound)?;

        if let Some(node) = entries.get(&target_key) {
            if node.ino_type == InodeType::Directory {
                return Err(VfsError::IsADirectory);
            }
        }

        entries.remove(&target_key);
        Ok(())
    }

    fn rmdir(&self, name: &str) -> Result<(), VfsError> {
        if name == "." || name == ".." {
            return Err(VfsError::InvalidArgument);
        }

        let mut entries = self.entries.write().unwrap();
        let target_key = entries.iter()
            .find(|(k, _)| k.as_str() == name || k.eq_ignore_ascii_case(name))
            .map(|(k, _)| k.clone())
            .ok_or(VfsError::NotFound)?;

        let target_node = entries.get(&target_key).ok_or(VfsError::NotFound)?;
        if target_node.ino_type != InodeType::Directory {
            return Err(VfsError::NotADirectory);
        }

        // Verify directory is empty (has no child entries)
        let child_entries = target_node.readdir()?;
        // readdir returns . and .. plus any children; length > 2 means not empty
        if child_entries.len() > 2 {
            return Err(VfsError::DirectoryNotEmpty);
        }

        entries.remove(&target_key);
        Ok(())
    }

    fn readdir(&self) -> Result<Vec<DirEntry>, VfsError> {
        let mut result = Vec::new();

        // 1. Current directory entry "."
        if let Some(weak) = self.self_node.read().unwrap().as_ref() {
            if let Some(strong) = weak.upgrade() {
                result.push(DirEntry {
                    name: ".".to_string(),
                    inode_id: strong.id,
                    ino_type: InodeType::Directory,
                    size: strong.get_size(),
                });
            }
        }

        // 2. Parent directory entry ".."
        if let Some(weak) = self.parent_node.read().unwrap().as_ref() {
            if let Some(strong) = weak.upgrade() {
                result.push(DirEntry {
                    name: "..".to_string(),
                    inode_id: strong.id,
                    ino_type: InodeType::Directory,
                    size: strong.get_size(),
                });
            }
        } else if let Some(self_weak) = self.self_node.read().unwrap().as_ref() {
            if let Some(self_strong) = self_weak.upgrade() {
                result.push(DirEntry {
                    name: "..".to_string(),
                    inode_id: self_strong.id,
                    ino_type: InodeType::Directory,
                    size: self_strong.get_size(),
                });
            }
        }

        // 3. User children entries
        let entries = self.entries.read().unwrap();
        for (name, node) in entries.iter() {
            result.push(DirEntry {
                name: name.clone(),
                inode_id: node.id,
                ino_type: node.ino_type,
                size: node.get_size(),
            });
        }

        Ok(result)
    }

    fn symlink(&self, name: &str, target: &str) -> Result<Arc<Inode>, VfsError> {
        validate_filename(name)?;

        let mut entries = self.entries.write().unwrap();
        if entries.contains_key(name) {
            return Err(VfsError::AlreadyExists);
        }

        let ino_id = self.ino_allocator.fetch_add(1, Ordering::SeqCst);

        let sym_ops = Arc::new(RamfsSymlinkOps {
            target: RwLock::new(target.to_string()),
        });

        let inode = Arc::new(Inode::new(
            ino_id,
            InodeType::Symlink,
            Permissions::default_file(),
            sym_ops,
        ));
        inode.set_size(target.len() as u64);

        entries.insert(name.to_string(), inode.clone());
        Ok(inode)
    }

    fn add_entry(&self, name: &str, node: Arc<Inode>) -> Result<(), VfsError> {
        validate_filename(name)?;
        let mut entries = self.entries.write().unwrap();
        if entries.contains_key(name) {
            return Err(VfsError::AlreadyExists);
        }
        entries.insert(name.to_string(), node);
        Ok(())
    }
}

/// Symbolic link operations storing destination path.
pub struct RamfsSymlinkOps {
    target: RwLock<String>,
}

impl FileOperations for RamfsSymlinkOps {
    fn readlink(&self) -> Result<String, VfsError> {
        Ok(self.target.read().unwrap().clone())
    }
}

/// `/dev/null` character device operations.
pub struct RamfsNullDeviceOps;

impl FileOperations for RamfsNullDeviceOps {
    fn read(&self, _offset: u64, _buf: &mut [u8]) -> Result<usize, VfsError> {
        Ok(0) // Null device returns EOF immediately
    }

    fn write(&self, _offset: u64, buf: &[u8]) -> Result<usize, VfsError> {
        Ok(buf.len()) // Discards all data written
    }
}

/// `/dev/zero` character device operations.
pub struct RamfsZeroDeviceOps;

impl FileOperations for RamfsZeroDeviceOps {
    fn read(&self, _offset: u64, buf: &mut [u8]) -> Result<usize, VfsError> {
        buf.fill(0);
        Ok(buf.len())
    }

    fn write(&self, _offset: u64, buf: &[u8]) -> Result<usize, VfsError> {
        Ok(buf.len())
    }
}

/// `/dev/console` character device operations.
pub struct RamfsConsoleDeviceOps;

impl FileOperations for RamfsConsoleDeviceOps {
    fn read(&self, _offset: u64, _buf: &mut [u8]) -> Result<usize, VfsError> {
        Ok(0)
    }

    fn write(&self, _offset: u64, buf: &[u8]) -> Result<usize, VfsError> {
        let msg = String::from_utf8_lossy(buf);
        print!("{}", msg);
        Ok(buf.len())
    }
}

/// Thread-safe in-memory filesystem instance.
pub struct RamFileSystem {
    next_ino: Arc<AtomicU64>,
    root: Arc<Inode>,
}

impl RamFileSystem {
    /// Instantiate a fresh RAM filesystem with an initialized root directory (`/`).
    pub fn new() -> Arc<Self> {
        let ino_allocator = Arc::new(AtomicU64::new(2));

        let root_dir_ops = Arc::new(RamfsDirOps::new(ino_allocator.clone()));

        let root_inode = Arc::new(Inode::new(
            1,
            InodeType::Directory,
            Permissions::default_dir(),
            root_dir_ops.clone(),
        ));

        root_dir_ops.set_self(&root_inode);

        Arc::new(Self {
            next_ino: ino_allocator,
            root: root_inode,
        })
    }

    /// Access the root inode of the filesystem.
    pub fn root(&self) -> Arc<Inode> {
        self.root.clone()
    }

    /// Allocate a unique Inode ID.
    pub fn allocate_ino(&self) -> u64 {
        self.next_ino.fetch_add(1, Ordering::SeqCst)
    }

    /// Helper to create a regular file inside an existing directory inode.
    pub fn create_file(
        &self,
        parent: &Arc<Inode>,
        name: &str,
        data: &[u8],
        perms: Permissions,
    ) -> Result<Arc<Inode>, VfsError> {
        let inode = parent.create(name, InodeType::File, perms)?;
        if !data.is_empty() {
            inode.write(0, data)?;
        }
        Ok(inode)
    }

    /// Helper to create a directory inside an existing directory inode.
    pub fn create_dir(
        &self,
        parent: &Arc<Inode>,
        name: &str,
        perms: Permissions,
    ) -> Result<Arc<Inode>, VfsError> {
        parent.mkdir(name, perms)
    }

    /// Helper to register a special character device in a directory inode.
    pub fn create_char_device(
        &self,
        parent: &Arc<Inode>,
        name: &str,
        ops: Arc<dyn FileOperations>,
    ) -> Result<Arc<Inode>, VfsError> {
        validate_filename(name)?;

        let ino_id = self.allocate_ino();
        let inode = Arc::new(Inode::new(
            ino_id,
            InodeType::CharDevice,
            Permissions::from_mode(0o666),
            ops,
        ));

        parent.add_entry(name, inode.clone())?;
        Ok(inode)
    }
}

/// Helper function to validate path segment filenames.
fn validate_filename(name: &str) -> Result<(), VfsError> {
    if name.is_empty() || name == "." || name == ".." || name.contains('/') || name.contains('\\') {
        return Err(VfsError::InvalidArgument);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ramfs_directory_and_file_ops() {
        let ramfs = RamFileSystem::new();
        let root = ramfs.root();

        // 1. Create subdirectory
        let bin_dir = ramfs.create_dir(&root, "bin", Permissions::default_dir()).unwrap();
        assert_eq!(bin_dir.ino_type, InodeType::Directory);

        // 2. Create file in subdirectory
        let sh_file = ramfs.create_file(&bin_dir, "sh", b"#!/bin/sh\necho OK", Permissions::default_file()).unwrap();
        assert_eq!(sh_file.ino_type, InodeType::File);
        assert_eq!(sh_file.get_size(), 17);

        // 3. Read back file contents
        let mut buf = [0u8; 17];
        let n = sh_file.read(0, &mut buf).unwrap();
        assert_eq!(n, 17);
        assert_eq!(&buf, b"#!/bin/sh\necho OK");

        // 4. Directory iteration
        let entries = bin_dir.readdir().unwrap();
        assert!(entries.iter().any(|e| e.name == "sh"));
        assert!(entries.iter().any(|e| e.name == "."));
        assert!(entries.iter().any(|e| e.name == ".."));

        // 5. Case-insensitive lookup (Windows-compatible)
        let found = bin_dir.lookup("SH").unwrap();
        assert_eq!(found.id, sh_file.id);

        // 6. Navigation with . and ..
        let dot = bin_dir.lookup(".").unwrap();
        assert_eq!(dot.id, bin_dir.id);
        let dotdot = bin_dir.lookup("..").unwrap();
        assert_eq!(dotdot.id, root.id);

        // 7. Unlink
        bin_dir.unlink("sh").unwrap();
        assert!(matches!(bin_dir.lookup("sh"), Err(VfsError::NotFound)));
    }

    #[test]
    fn test_ramfs_character_devices() {
        let ramfs = RamFileSystem::new();
        let root = ramfs.root();

        let null_dev = ramfs.create_char_device(&root, "null", Arc::new(RamfsNullDeviceOps)).unwrap();
        let mut buf = [1u8; 16];
        let n = null_dev.read(0, &mut buf).unwrap();
        assert_eq!(n, 0); // null read gives EOF

        let zero_dev = ramfs.create_char_device(&root, "zero", Arc::new(RamfsZeroDeviceOps)).unwrap();
        let n2 = zero_dev.read(0, &mut buf).unwrap();
        assert_eq!(n2, 16);
        assert_eq!(buf, [0u8; 16]);
    }
}
