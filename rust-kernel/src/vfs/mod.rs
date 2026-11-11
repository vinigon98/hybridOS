//! HybridOS Virtual File System (VFS) Module
//!
//! Provides a unified, thread-safe, high-performance VFS architecture for
//! both Windows NT (PE32+, SSDT, Win32 paths) and Linux (ELF64, POSIX, Unix paths).
//!
//! Submodules:
//! - [`inode`]: Inode, InodeType, FileDescriptor, Permissions, and FileOperations trait.
//! - [`path`]: Unified Path Resolver mapping Unix and Windows NT paths.
//! - [`ramfs`]: In-memory filesystem implementation.

#[allow(unused_imports)]
use alloc::string::{String, ToString};
#[allow(unused_imports)]
use alloc::vec::Vec;
#[allow(unused_imports)]
use alloc::boxed::Box;
#[allow(unused_imports)]
use crate::compat::FloatExt;
pub mod inode;
pub mod path;
pub mod ramfs;

pub use inode::{
    DirEntry, FileDescriptor, FileOperations, Inode, InodeMetadata, InodeType,
    OpenFlags, Permissions, SeekWhence,
};
pub use path::{CanonicalPath, PathStyle, UnifiedPathResolver};
pub use ramfs::{
    RamFileSystem, RamfsConsoleDeviceOps, RamfsNullDeviceOps, RamfsZeroDeviceOps,
};

use std::collections::HashMap;
use std::fmt;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock, RwLock};

use crate::types::{NtStatus, PosixError};

/// Comprehensive VFS error codes mappable to both POSIX errno and Windows NTSTATUS.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VfsError {
    NotFound,
    AlreadyExists,
    PermissionDenied,
    NotADirectory,
    IsADirectory,
    DirectoryNotEmpty,
    InvalidArgument,
    IoError,
    NotSupported,
    TooManyOpenFiles,
    BadFileDescriptor,
    InvalidPath,
    NoSpace,
    IsDevice,
}

impl fmt::Display for VfsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VfsError::NotFound => write!(f, "File or directory not found"),
            VfsError::AlreadyExists => write!(f, "Object already exists"),
            VfsError::PermissionDenied => write!(f, "Access permission denied"),
            VfsError::NotADirectory => write!(f, "Not a directory"),
            VfsError::IsADirectory => write!(f, "Is a directory"),
            VfsError::DirectoryNotEmpty => write!(f, "Directory not empty"),
            VfsError::InvalidArgument => write!(f, "Invalid argument"),
            VfsError::IoError => write!(f, "I/O subsystem error"),
            VfsError::NotSupported => write!(f, "Operation not supported"),
            VfsError::TooManyOpenFiles => write!(f, "Too many open files"),
            VfsError::BadFileDescriptor => write!(f, "Bad file descriptor"),
            VfsError::InvalidPath => write!(f, "Invalid path syntax"),
            VfsError::NoSpace => write!(f, "No space left on device"),
            VfsError::IsDevice => write!(f, "Target is a device node"),
        }
    }
}

impl std::error::Error for VfsError {}

impl From<VfsError> for PosixError {
    fn from(err: VfsError) -> Self {
        match err {
            VfsError::NotFound => PosixError::ENOENT,
            VfsError::AlreadyExists => PosixError::EPERM,
            VfsError::PermissionDenied => PosixError::EACCES,
            VfsError::NotADirectory => PosixError::EINVAL,
            VfsError::IsADirectory => PosixError::EINVAL,
            VfsError::DirectoryNotEmpty => PosixError::EINVAL,
            VfsError::InvalidArgument => PosixError::EINVAL,
            VfsError::IoError => PosixError::EIO,
            VfsError::NotSupported => PosixError::ENOSYS,
            VfsError::TooManyOpenFiles => PosixError::ENOMEM,
            VfsError::BadFileDescriptor => PosixError::EBADF,
            VfsError::InvalidPath => PosixError::EINVAL,
            VfsError::NoSpace => PosixError::ENOMEM,
            VfsError::IsDevice => PosixError::ENODEV,
        }
    }
}

impl From<VfsError> for NtStatus {
    fn from(err: VfsError) -> Self {
        match err {
            VfsError::NotFound => NtStatus::FILE_NOT_FOUND,
            VfsError::AlreadyExists => NtStatus(-1073741771), // 0xC0000035 STATUS_OBJECT_NAME_COLLISION
            VfsError::PermissionDenied => NtStatus::ACCESS_DENIED,
            VfsError::NotADirectory => NtStatus(-1073741768), // 0xC0000038 STATUS_OBJECT_TYPE_MISMATCH
            VfsError::IsADirectory => NtStatus(-1073741768),
            VfsError::DirectoryNotEmpty => NtStatus(-1073741639), // 0xC00000B9 STATUS_DIRECTORY_NOT_EMPTY
            VfsError::InvalidArgument => NtStatus::INVALID_PARAMETER,
            VfsError::IoError => NtStatus::UNSUCCESSFUL,
            VfsError::NotSupported => NtStatus::NOT_IMPLEMENTED,
            VfsError::TooManyOpenFiles => NtStatus(-1073741753), // 0xC0000047 STATUS_TOO_MANY_OPENED_FILES
            VfsError::BadFileDescriptor => NtStatus::INVALID_HANDLE,
            VfsError::InvalidPath => NtStatus(-1073741767), // 0xC0000039 STATUS_OBJECT_PATH_INVALID
            VfsError::NoSpace => NtStatus(-1073741801), // STATUS_NO_MEMORY
            VfsError::IsDevice => NtStatus::INVALID_PARAMETER,
        }
    }
}

/// Central Virtual File System manager.
pub struct VirtualFileSystem {
    root_fs: Arc<RamFileSystem>,
    resolver: UnifiedPathResolver,
    open_files: RwLock<HashMap<usize, Arc<FileDescriptor>>>,
    next_fd: AtomicUsize,
}

impl VirtualFileSystem {
    /// Create a new VirtualFileSystem with an empty RAMFS root.
    pub fn new() -> Self {
        let root_fs = RamFileSystem::new();
        let resolver = UnifiedPathResolver::new();

        Self {
            root_fs,
            resolver,
            open_files: RwLock::new(HashMap::new()),
            next_fd: AtomicUsize::new(10), // User file descriptors start at 10
        }
    }

    /// Access the underlying RAM filesystem.
    pub fn ramfs(&self) -> &Arc<RamFileSystem> {
        &self.root_fs
    }

    /// Access the unified path resolver.
    pub fn resolver(&self) -> &UnifiedPathResolver {
        &self.resolver
    }

    /// Resolve an arbitrary path string to a CanonicalPath.
    pub fn resolve_path(&self, raw_path: &str, cwd: &str) -> Result<CanonicalPath, VfsError> {
        self.resolver.resolve(raw_path, cwd)
    }

    /// Look up an Inode from a canonical path.
    pub fn lookup_node(&self, canon: &CanonicalPath) -> Result<Arc<Inode>, VfsError> {
        let mut curr = self.root_fs.root();

        let mut symlink_hops = 0;
        let max_hops = 16;

        let mut idx = 0;
        let mut components = canon.components.clone();

        while idx < components.len() {
            let comp = &components[idx];
            let next_node = curr.lookup(comp)?;

            if next_node.ino_type == InodeType::Symlink {
                symlink_hops += 1;
                if symlink_hops > max_hops {
                    return Err(VfsError::IoError); // Symlink cycle
                }

                let target = next_node.readlink()?;
                let target_canon = self.resolver.resolve(&target, &curr_path_str(&components[..idx]))?;

                // Restart from root with resolved symlink components prepended to remainder
                curr = self.root_fs.root();
                let mut new_comps = target_canon.components;
                new_comps.extend_from_slice(&components[idx + 1..]);
                components = new_comps;
                idx = 0;
                continue;
            }

            curr = next_node;
            idx += 1;
        }

        Ok(curr)
    }

    /// Look up the parent directory Inode and leaf name for a canonical path.
    pub fn lookup_parent(&self, canon: &CanonicalPath) -> Result<(Arc<Inode>, String), VfsError> {
        if canon.is_root() {
            return Err(VfsError::InvalidArgument);
        }

        let leaf_name = canon.file_name().ok_or(VfsError::InvalidPath)?.to_string();
        let parent_canon = canon.parent().unwrap_or_else(CanonicalPath::root);
        let parent_node = self.lookup_node(&parent_canon)?;

        if parent_node.ino_type != InodeType::Directory {
            return Err(VfsError::NotADirectory);
        }

        Ok((parent_node, leaf_name))
    }

    /// Open a file or directory with specified OpenFlags, returning an FD handle.
    pub fn open(&self, raw_path: &str, flags: OpenFlags) -> Result<usize, VfsError> {
        let canon = self.resolve_path(raw_path, "/")?;

        let inode = match self.lookup_node(&canon) {
            Ok(existing) => {
                if flags.exclusive && flags.create {
                    return Err(VfsError::AlreadyExists);
                }
                if flags.directory && existing.ino_type != InodeType::Directory {
                    return Err(VfsError::NotADirectory);
                }
                if !flags.directory && existing.ino_type == InodeType::Directory && flags.write {
                    return Err(VfsError::IsADirectory);
                }
                if flags.truncate && flags.write {
                    existing.truncate(0)?;
                }
                existing
            }
            Err(VfsError::NotFound) => {
                if !flags.create {
                    return Err(VfsError::NotFound);
                }
                let (parent_dir, leaf) = self.lookup_parent(&canon)?;
                let perms = Permissions::default_file();
                parent_dir.create(&leaf, InodeType::File, perms)?
            }
            Err(e) => return Err(e),
        };

        let fd_id = self.next_fd.fetch_add(1, Ordering::SeqCst);
        let fd = Arc::new(FileDescriptor::new(
            fd_id,
            inode,
            flags,
            canon.as_unix_path(),
        ));

        let mut files = self.open_files.write().unwrap();
        files.insert(fd_id, fd);

        Ok(fd_id)
    }

    /// Read data from an open file descriptor.
    pub fn read(&self, fd: usize, buf: &mut [u8]) -> Result<usize, VfsError> {
        let desc = {
            let files = self.open_files.read().unwrap();
            files.get(&fd).cloned().ok_or(VfsError::BadFileDescriptor)?
        };
        desc.read(buf)
    }

    /// Write data to an open file descriptor.
    pub fn write(&self, fd: usize, buf: &[u8]) -> Result<usize, VfsError> {
        let desc = {
            let files = self.open_files.read().unwrap();
            files.get(&fd).cloned().ok_or(VfsError::BadFileDescriptor)?
        };
        desc.write(buf)
    }

    /// Reposition file descriptor offset.
    pub fn seek(&self, fd: usize, whence: SeekWhence) -> Result<u64, VfsError> {
        let desc = {
            let files = self.open_files.read().unwrap();
            files.get(&fd).cloned().ok_or(VfsError::BadFileDescriptor)?
        };
        desc.seek(whence)
    }

    /// Close an open file descriptor.
    pub fn close(&self, fd: usize) -> Result<(), VfsError> {
        let mut files = self.open_files.write().unwrap();
        if files.remove(&fd).is_some() {
            Ok(())
        } else {
            Err(VfsError::BadFileDescriptor)
        }
    }

    /// Create a directory at the given path.
    pub fn mkdir(&self, raw_path: &str, perms: Permissions) -> Result<Arc<Inode>, VfsError> {
        let canon = self.resolve_path(raw_path, "/")?;
        let (parent_dir, leaf) = self.lookup_parent(&canon)?;
        parent_dir.mkdir(&leaf, perms)
    }

    /// Create all missing parent directories along a path.
    pub fn create_dir_all(&self, raw_path: &str, perms: Permissions) -> Result<Arc<Inode>, VfsError> {
        let canon = self.resolve_path(raw_path, "/")?;
        let mut curr = self.root_fs.root();

        for comp in &canon.components {
            match curr.lookup(comp) {
                Ok(child) => {
                    if child.ino_type != InodeType::Directory {
                        return Err(VfsError::NotADirectory);
                    }
                    curr = child;
                }
                Err(VfsError::NotFound) => {
                    curr = curr.mkdir(comp, perms)?;
                }
                Err(e) => return Err(e),
            }
        }

        Ok(curr)
    }

    /// Create and write a file in a single step.
    pub fn create_file(&self, raw_path: &str, data: &[u8], perms: Permissions) -> Result<Arc<Inode>, VfsError> {
        let canon = self.resolve_path(raw_path, "/")?;
        let (parent_dir, leaf) = self.lookup_parent(&canon)?;
        self.root_fs.create_file(&parent_dir, &leaf, data, perms)
    }

    /// Read the entire content of a file into memory.
    pub fn read_to_vec(&self, raw_path: &str) -> Result<Vec<u8>, VfsError> {
        let canon = self.resolve_path(raw_path, "/")?;
        let node = self.lookup_node(&canon)?;
        if node.ino_type != InodeType::File {
            return Err(VfsError::InvalidArgument);
        }

        let size = node.get_size() as usize;
        let mut buffer = vec![0u8; size];
        let bytes_read = node.read(0, &mut buffer)?;
        buffer.truncate(bytes_read);
        Ok(buffer)
    }

    /// Iterate over entries in a directory.
    pub fn readdir(&self, raw_path: &str) -> Result<Vec<DirEntry>, VfsError> {
        let canon = self.resolve_path(raw_path, "/")?;
        let node = self.lookup_node(&canon)?;
        node.readdir()
    }

    /// Remove a file or symbolic link.
    pub fn unlink(&self, raw_path: &str) -> Result<(), VfsError> {
        let canon = self.resolve_path(raw_path, "/")?;
        let (parent_dir, leaf) = self.lookup_parent(&canon)?;
        parent_dir.unlink(&leaf)
    }

    /// Remove an empty directory.
    pub fn rmdir(&self, raw_path: &str) -> Result<(), VfsError> {
        let canon = self.resolve_path(raw_path, "/")?;
        let (parent_dir, leaf) = self.lookup_parent(&canon)?;
        parent_dir.rmdir(&leaf)
    }

    /// Fetch metadata for a filesystem object.
    pub fn stat(&self, raw_path: &str) -> Result<InodeMetadata, VfsError> {
        let canon = self.resolve_path(raw_path, "/")?;
        let node = self.lookup_node(&canon)?;
        Ok(node.metadata())
    }
}

impl Default for VirtualFileSystem {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper function to reconstruct a path string from components.
fn curr_path_str(comps: &[String]) -> String {
    if comps.is_empty() {
        "/".to_string()
    } else {
        let mut s = String::new();
        for c in comps {
            s.push('/');
            s.push_str(c);
        }
        s
    }
}

// Global Virtual File System singleton
static GLOBAL_VFS: OnceLock<VirtualFileSystem> = OnceLock::new();

/// Access the global Virtual File System instance.
pub fn get_vfs() -> &'static VirtualFileSystem {
    GLOBAL_VFS.get().expect("VFS subsystem has not been initialized! Call init_vfs() first.")
}

/// Initialize the global Virtual File System and populate default filesystem hierarchies.
pub fn init_vfs() -> Result<(), VfsError> {
    let vfs = VirtualFileSystem::new();

    // 1. Setup Standard Unix Directory Hierarchy
    let dir_perms = Permissions::default_dir();
    vfs.create_dir_all("/bin", dir_perms)?;
    vfs.create_dir_all("/sbin", dir_perms)?;
    vfs.create_dir_all("/etc", dir_perms)?;
    vfs.create_dir_all("/dev", dir_perms)?;
    vfs.create_dir_all("/tmp", Permissions::from_mode(0o777))?;
    vfs.create_dir_all("/usr/bin", dir_perms)?;
    vfs.create_dir_all("/usr/lib", dir_perms)?;
    vfs.create_dir_all("/proc", dir_perms)?;
    vfs.create_dir_all("/sys", dir_perms)?;
    vfs.create_dir_all("/home", dir_perms)?;
    vfs.create_dir_all("/root", Permissions::from_mode(0o700))?;

    // 2. Setup Standard Windows NT Directory Hierarchy (Drive C: and D:)
    vfs.create_dir_all("/c/Windows/System32", dir_perms)?;
    vfs.create_dir_all("/c/Windows/SysWOW64", dir_perms)?;
    vfs.create_dir_all("/c/Program Files", dir_perms)?;
    vfs.create_dir_all("/c/Program Files (x86)", dir_perms)?;
    vfs.create_dir_all("/c/Users/Administrator", dir_perms)?;
    vfs.create_dir_all("/c/Users/Default", dir_perms)?;
    vfs.create_dir_all("/d", dir_perms)?;

    // 3. Register Core Character Device Nodes in /dev
    let dev_dir = vfs.lookup_node(&vfs.resolve_path("/dev", "/")?)?;
    vfs.ramfs().create_char_device(&dev_dir, "null", Arc::new(RamfsNullDeviceOps))?;
    vfs.ramfs().create_char_device(&dev_dir, "zero", Arc::new(RamfsZeroDeviceOps))?;
    vfs.ramfs().create_char_device(&dev_dir, "console", Arc::new(RamfsConsoleDeviceOps))?;

    // 4. Populate Mock System Identification Files
    let os_release_content = b"NAME=\"HybridOS\"\nVERSION=\"1.0.0 (Proprietary Dual-Personality)\"\nID=hybridos\nID_LIKE=\"windows linux\"\nPRETTY_NAME=\"HybridOS Dual-Personality Kernel (Windows NT + Linux POSIX)\"\n";
    vfs.create_file("/etc/os-release", os_release_content, Permissions::default_file())?;

    let win_version_content = b"[Version]\nSignature=\"$WINDOWS NT$\"\nProvider=\"HybridOS NT Kernel Subsystem\"\nProductVersion=\"10.0.22631\"\n";
    vfs.create_file("/c/Windows/System32/ntoskrnl.ini", win_version_content, Permissions::default_file())?;

    if GLOBAL_VFS.set(vfs).is_err() {
        return Err(VfsError::AlreadyExists);
    }

    println!("[VFS] Virtual File System initialized successfully with root RamFS & Dual-Personality Path Resolver.");
    Ok(())
}

// Global VFS convenience wrapper functions

pub fn vfs_open(path: &str, flags: OpenFlags) -> Result<usize, VfsError> {
    get_vfs().open(path, flags)
}

pub fn vfs_read(fd: usize, buf: &mut [u8]) -> Result<usize, VfsError> {
    get_vfs().read(fd, buf)
}

pub fn vfs_write(fd: usize, buf: &[u8]) -> Result<usize, VfsError> {
    get_vfs().write(fd, buf)
}

pub fn vfs_seek(fd: usize, whence: SeekWhence) -> Result<u64, VfsError> {
    get_vfs().seek(fd, whence)
}

pub fn vfs_close(fd: usize) -> Result<(), VfsError> {
    get_vfs().close(fd)
}

pub fn vfs_mkdir(path: &str, perms: Permissions) -> Result<Arc<Inode>, VfsError> {
    get_vfs().mkdir(path, perms)
}

pub fn vfs_create_dir_all(path: &str, perms: Permissions) -> Result<Arc<Inode>, VfsError> {
    get_vfs().create_dir_all(path, perms)
}

pub fn vfs_create_file(path: &str, data: &[u8], perms: Permissions) -> Result<Arc<Inode>, VfsError> {
    get_vfs().create_file(path, data, perms)
}

pub fn vfs_read_all(path: &str) -> Result<Vec<u8>, VfsError> {
    get_vfs().read_to_vec(path)
}

pub fn vfs_readdir(path: &str) -> Result<Vec<DirEntry>, VfsError> {
    get_vfs().readdir(path)
}

pub fn vfs_resolve_path(path: &str) -> Result<String, VfsError> {
    let canon = get_vfs().resolve_path(path, "/")?;
    Ok(canon.as_unix_path())
}

pub fn vfs_unlink(path: &str) -> Result<(), VfsError> {
    get_vfs().unlink(path)
}

pub fn vfs_stat(path: &str) -> Result<InodeMetadata, VfsError> {
    get_vfs().stat(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vfs_end_to_end_operations() {
        let vfs = VirtualFileSystem::new();

        // 1. Create directory hierarchy
        vfs.create_dir_all("/etc/network", Permissions::default_dir()).unwrap();

        // 2. Open / create a file
        let fd = vfs.open("/etc/network/interfaces", OpenFlags::create_or_truncate()).unwrap();
        let payload = b"auto eth0\niface eth0 inet dhcp\n";
        let written = vfs.write(fd, payload).unwrap();
        assert_eq!(written, payload.len());
        vfs.close(fd).unwrap();

        // 3. Read back via path API
        let content = vfs.read_to_vec("/etc/network/interfaces").unwrap();
        assert_eq!(&content, payload);

        // 4. Windows NT Path Access to the same file
        // /c/Windows/System32 is populated
        vfs.create_dir_all(r"C:\Windows\System32", Permissions::default_dir()).unwrap();
        vfs.create_file(r"C:\Windows\System32\cmd.exe", b"MZ_MOCK_PE", Permissions::default_executable()).unwrap();

        // Access via Windows NT Object Manager path
        let nt_fd = vfs.open(r"\Device\HarddiskVolume1\Windows\System32\cmd.exe", OpenFlags::read_only()).unwrap();
        let mut pe_header = [0u8; 10];
        let n = vfs.read(nt_fd, &mut pe_header).unwrap();
        assert_eq!(n, 10);
        assert_eq!(&pe_header, b"MZ_MOCK_PE");
        vfs.close(nt_fd).unwrap();

        // Access via \SystemRoot alias
        let sys_content = vfs.read_to_vec(r"\SystemRoot\System32\cmd.exe").unwrap();
        assert_eq!(&sys_content, b"MZ_MOCK_PE");

        // 5. Stat metadata
        let meta = vfs.stat(r"C:\Windows\System32\cmd.exe").unwrap();
        assert_eq!(meta.size, 10);
        assert_eq!(meta.ino_type, InodeType::File);

        // 6. Directory listing
        let dir_entries = vfs.readdir(r"C:\Windows\System32").unwrap();
        assert!(dir_entries.iter().any(|e| e.name == "cmd.exe"));
    }
}
