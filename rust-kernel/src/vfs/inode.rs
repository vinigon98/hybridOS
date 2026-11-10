//! Virtual File System - Inode, FileDescriptor, and Permissions
//!
//! Provides the fundamental abstractions for HybridOS storage objects:
//! - [`Inode`]: Universal kernel filesystem node.
//! - [`InodeType`]: Inode classification (File, Directory, Devices, Symlinks).
//! - [`Permissions`]: Unified POSIX mode bits and Windows NT file attributes.
//! - [`FileDescriptor`]: Open file instance with per-handle offset and flags.
//! - [`FileOperations`]: Trait for filesystem driver operations.

use std::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::fmt;

use super::VfsError;

/// Inode classification supported across HybridOS personalities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InodeType {
    File,
    Directory,
    CharDevice,
    BlockDevice,
    Symlink,
    Fifo,
    Socket,
}

impl InodeType {
    #[inline]
    pub fn is_file(&self) -> bool {
        matches!(self, InodeType::File)
    }

    #[inline]
    pub fn is_dir(&self) -> bool {
        matches!(self, InodeType::Directory)
    }

    #[inline]
    pub fn is_device(&self) -> bool {
        matches!(self, InodeType::CharDevice | InodeType::BlockDevice)
    }

    #[inline]
    pub fn is_symlink(&self) -> bool {
        matches!(self, InodeType::Symlink)
    }
}

impl fmt::Display for InodeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InodeType::File => write!(f, "REGULAR_FILE"),
            InodeType::Directory => write!(f, "DIRECTORY"),
            InodeType::CharDevice => write!(f, "CHAR_DEVICE"),
            InodeType::BlockDevice => write!(f, "BLOCK_DEVICE"),
            InodeType::Symlink => write!(f, "SYMLINK"),
            InodeType::Fifo => write!(f, "FIFO"),
            InodeType::Socket => write!(f, "SOCKET"),
        }
    }
}

/// Unified permissions combining POSIX mode bits and Windows NT attributes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Permissions {
    pub mode: u32,
    pub readonly: bool,
    pub hidden: bool,
    pub system: bool,
    pub archive: bool,
}

impl Permissions {
    // POSIX Standard Mode Bits
    pub const S_ISUID: u32 = 0o4000;
    pub const S_ISGID: u32 = 0o2000;
    pub const S_ISVTX: u32 = 0o1000;
    pub const S_IRUSR: u32 = 0o0400;
    pub const S_IWUSR: u32 = 0o0200;
    pub const S_IXUSR: u32 = 0o0100;
    pub const S_IRWXU: u32 = 0o0700;
    pub const S_IRGRP: u32 = 0o0040;
    pub const S_IWGRP: u32 = 0o0020;
    pub const S_IXGRP: u32 = 0o0010;
    pub const S_IRWXG: u32 = 0o0070;
    pub const S_IROTH: u32 = 0o0004;
    pub const S_IWOTH: u32 = 0o0002;
    pub const S_IXOTH: u32 = 0o0001;
    pub const S_IRWXO: u32 = 0o0007;

    // Windows NT File Attributes
    pub const FILE_ATTRIBUTE_READONLY: u32  = 0x00000001;
    pub const FILE_ATTRIBUTE_HIDDEN: u32    = 0x00000002;
    pub const FILE_ATTRIBUTE_SYSTEM: u32    = 0x00000004;
    pub const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x00000010;
    pub const FILE_ATTRIBUTE_ARCHIVE: u32   = 0x00000020;
    pub const FILE_ATTRIBUTE_NORMAL: u32    = 0x00000080;

    /// Construct permissions from POSIX octal mode.
    pub fn from_mode(mode: u32) -> Self {
        let readonly = (mode & Self::S_IWUSR) == 0;
        Self {
            mode: mode & 0o7777,
            readonly,
            hidden: false,
            system: false,
            archive: true,
        }
    }

    /// Construct permissions from Windows NT attributes mask.
    pub fn from_nt_attributes(attrs: u32) -> Self {
        let readonly = (attrs & Self::FILE_ATTRIBUTE_READONLY) != 0;
        let hidden = (attrs & Self::FILE_ATTRIBUTE_HIDDEN) != 0;
        let system = (attrs & Self::FILE_ATTRIBUTE_SYSTEM) != 0;
        let archive = (attrs & Self::FILE_ATTRIBUTE_ARCHIVE) != 0;
        let mode = if readonly { 0o444 } else { 0o644 };

        Self {
            mode,
            readonly,
            hidden,
            system,
            archive,
        }
    }

    /// Standard default file permissions: 0o644 (rw-r--r--).
    pub fn default_file() -> Self {
        Self::from_mode(0o644)
    }

    /// Standard default directory permissions: 0o755 (rwxr-xr-x).
    pub fn default_dir() -> Self {
        let mut p = Self::from_mode(0o755);
        p.archive = false;
        p
    }

    /// Standard default executable permissions: 0o755 (rwxr-xr-x).
    pub fn default_executable() -> Self {
        Self::from_mode(0o755)
    }

    /// Read-only permissions: 0o444 (r--r--r--).
    pub fn readonly() -> Self {
        Self::from_mode(0o444)
    }

    pub fn is_readable(&self) -> bool {
        (self.mode & (Self::S_IRUSR | Self::S_IRGRP | Self::S_IROTH)) != 0
    }

    pub fn is_writable(&self) -> bool {
        !self.readonly && (self.mode & (Self::S_IWUSR | Self::S_IWGRP | Self::S_IWOTH)) != 0
    }

    pub fn is_executable(&self) -> bool {
        (self.mode & (Self::S_IXUSR | Self::S_IXGRP | Self::S_IXOTH)) != 0
    }

    /// Convert to Windows NT file attributes DWORD.
    pub fn to_nt_attributes(&self, is_dir: bool) -> u32 {
        let mut attrs = 0u32;
        if self.readonly {
            attrs |= Self::FILE_ATTRIBUTE_READONLY;
        }
        if self.hidden {
            attrs |= Self::FILE_ATTRIBUTE_HIDDEN;
        }
        if self.system {
            attrs |= Self::FILE_ATTRIBUTE_SYSTEM;
        }
        if self.archive {
            attrs |= Self::FILE_ATTRIBUTE_ARCHIVE;
        }
        if is_dir {
            attrs |= Self::FILE_ATTRIBUTE_DIRECTORY;
        }
        if attrs == 0 {
            attrs = Self::FILE_ATTRIBUTE_NORMAL;
        }
        attrs
    }
}

/// Directory entry representation returned during iteration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirEntry {
    pub name: String,
    pub inode_id: u64,
    pub ino_type: InodeType,
    pub size: u64,
}

/// Seek repositioning modes for file descriptors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeekWhence {
    Start(u64),
    Current(i64),
    End(i64),
}

/// Snapshot of an Inode's metadata for `stat` operations.
#[derive(Debug, Clone)]
pub struct InodeMetadata {
    pub id: u64,
    pub ino_type: InodeType,
    pub size: u64,
    pub permissions: Permissions,
    pub uid: u32,
    pub gid: u32,
    pub nlink: usize,
    pub atime: u64,
    pub mtime: u64,
    pub ctime: u64,
}

/// Trait defining lower-level filesystem operations on an Inode.
pub trait FileOperations: Send + Sync {
    fn read(&self, _offset: u64, _buf: &mut [u8]) -> Result<usize, VfsError> {
        Err(VfsError::NotSupported)
    }

    fn write(&self, _offset: u64, _buf: &[u8]) -> Result<usize, VfsError> {
        Err(VfsError::NotSupported)
    }

    fn truncate(&self, _size: u64) -> Result<(), VfsError> {
        Err(VfsError::NotSupported)
    }

    fn lookup(&self, _name: &str) -> Result<Arc<Inode>, VfsError> {
        Err(VfsError::NotADirectory)
    }

    fn create(&self, _name: &str, _ino_type: InodeType, _perms: Permissions) -> Result<Arc<Inode>, VfsError> {
        Err(VfsError::NotADirectory)
    }

    fn mkdir(&self, _name: &str, _perms: Permissions) -> Result<Arc<Inode>, VfsError> {
        Err(VfsError::NotADirectory)
    }

    fn unlink(&self, _name: &str) -> Result<(), VfsError> {
        Err(VfsError::NotADirectory)
    }

    fn rmdir(&self, _name: &str) -> Result<(), VfsError> {
        Err(VfsError::NotADirectory)
    }

    fn readdir(&self) -> Result<Vec<DirEntry>, VfsError> {
        Err(VfsError::NotADirectory)
    }

    fn symlink(&self, _name: &str, _target: &str) -> Result<Arc<Inode>, VfsError> {
        Err(VfsError::NotADirectory)
    }

    fn add_entry(&self, _name: &str, _node: Arc<Inode>) -> Result<(), VfsError> {
        Err(VfsError::NotADirectory)
    }

    fn readlink(&self) -> Result<String, VfsError> {
        Err(VfsError::InvalidArgument)
    }

    fn ioctl(&self, _cmd: u32, _arg: usize) -> Result<usize, VfsError> {
        Err(VfsError::NotSupported)
    }

    fn sync(&self) -> Result<(), VfsError> {
        Ok(())
    }
}

/// Universal Inode structure managed by the HybridOS VFS.
pub struct Inode {
    pub id: u64,
    pub ino_type: InodeType,
    pub permissions: RwLock<Permissions>,
    pub size: AtomicU64,
    pub uid: AtomicU32,
    pub gid: AtomicU32,
    pub nlink: AtomicUsize,
    pub atime: AtomicU64,
    pub mtime: AtomicU64,
    pub ctime: AtomicU64,
    pub ops: Arc<dyn FileOperations>,
}

impl Inode {
    /// Construct a new Inode with backing filesystem operations.
    pub fn new(
        id: u64,
        ino_type: InodeType,
        permissions: Permissions,
        ops: Arc<dyn FileOperations>,
    ) -> Self {
        Self {
            id,
            ino_type,
            permissions: RwLock::new(permissions),
            size: AtomicU64::new(0),
            uid: AtomicU32::new(0),
            gid: AtomicU32::new(0),
            nlink: AtomicUsize::new(1),
            atime: AtomicU64::new(0),
            mtime: AtomicU64::new(0),
            ctime: AtomicU64::new(0),
            ops,
        }
    }

    /// Retrieve a snapshot of the Inode's metadata.
    pub fn metadata(&self) -> InodeMetadata {
        InodeMetadata {
            id: self.id,
            ino_type: self.ino_type,
            size: self.size.load(Ordering::Relaxed),
            permissions: *self.permissions.read().unwrap(),
            uid: self.uid.load(Ordering::Relaxed),
            gid: self.gid.load(Ordering::Relaxed),
            nlink: self.nlink.load(Ordering::Relaxed),
            atime: self.atime.load(Ordering::Relaxed),
            mtime: self.mtime.load(Ordering::Relaxed),
            ctime: self.ctime.load(Ordering::Relaxed),
        }
    }

    pub fn get_size(&self) -> u64 {
        self.size.load(Ordering::Relaxed)
    }

    pub fn set_size(&self, size: u64) {
        self.size.store(size, Ordering::SeqCst);
    }

    /// Read data from the backing storage at a specific offset.
    pub fn read(&self, offset: u64, buf: &mut [u8]) -> Result<usize, VfsError> {
        let perms = *self.permissions.read().unwrap();
        if !perms.is_readable() {
            return Err(VfsError::PermissionDenied);
        }
        self.ops.read(offset, buf)
    }

    /// Write data to the backing storage at a specific offset.
    pub fn write(&self, offset: u64, buf: &[u8]) -> Result<usize, VfsError> {
        let perms = *self.permissions.read().unwrap();
        if !perms.is_writable() {
            return Err(VfsError::PermissionDenied);
        }
        let written = self.ops.write(offset, buf)?;
        let end_offset = offset + written as u64;

        // Atomically update file size if we wrote past current EOF
        let mut current_size = self.size.load(Ordering::Relaxed);
        while end_offset > current_size {
            match self.size.compare_exchange_weak(
                current_size,
                end_offset,
                Ordering::SeqCst,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(actual) => current_size = actual,
            }
        }

        Ok(written)
    }

    /// Truncate or extend file size.
    pub fn truncate(&self, size: u64) -> Result<(), VfsError> {
        let perms = *self.permissions.read().unwrap();
        if !perms.is_writable() {
            return Err(VfsError::PermissionDenied);
        }
        self.ops.truncate(size)?;
        self.size.store(size, Ordering::SeqCst);
        Ok(())
    }

    /// Look up a child entry in a directory.
    pub fn lookup(&self, name: &str) -> Result<Arc<Inode>, VfsError> {
        if self.ino_type != InodeType::Directory {
            return Err(VfsError::NotADirectory);
        }
        self.ops.lookup(name)
    }

    /// Create a regular file or special node inside this directory.
    pub fn create(&self, name: &str, ino_type: InodeType, perms: Permissions) -> Result<Arc<Inode>, VfsError> {
        if self.ino_type != InodeType::Directory {
            return Err(VfsError::NotADirectory);
        }
        let dir_perms = *self.permissions.read().unwrap();
        if !dir_perms.is_writable() {
            return Err(VfsError::PermissionDenied);
        }
        self.ops.create(name, ino_type, perms)
    }

    /// Create a subdirectory inside this directory.
    pub fn mkdir(&self, name: &str, perms: Permissions) -> Result<Arc<Inode>, VfsError> {
        if self.ino_type != InodeType::Directory {
            return Err(VfsError::NotADirectory);
        }
        let dir_perms = *self.permissions.read().unwrap();
        if !dir_perms.is_writable() {
            return Err(VfsError::PermissionDenied);
        }
        self.ops.mkdir(name, perms)
    }

    /// Unlink a non-directory entry.
    pub fn unlink(&self, name: &str) -> Result<(), VfsError> {
        if self.ino_type != InodeType::Directory {
            return Err(VfsError::NotADirectory);
        }
        let dir_perms = *self.permissions.read().unwrap();
        if !dir_perms.is_writable() {
            return Err(VfsError::PermissionDenied);
        }
        self.ops.unlink(name)
    }

    /// Remove an empty subdirectory.
    pub fn rmdir(&self, name: &str) -> Result<(), VfsError> {
        if self.ino_type != InodeType::Directory {
            return Err(VfsError::NotADirectory);
        }
        let dir_perms = *self.permissions.read().unwrap();
        if !dir_perms.is_writable() {
            return Err(VfsError::PermissionDenied);
        }
        self.ops.rmdir(name)
    }

    /// Read directory entries.
    pub fn readdir(&self) -> Result<Vec<DirEntry>, VfsError> {
        if self.ino_type != InodeType::Directory {
            return Err(VfsError::NotADirectory);
        }
        self.ops.readdir()
    }

    /// Create a symbolic link in this directory.
    pub fn symlink(&self, name: &str, target: &str) -> Result<Arc<Inode>, VfsError> {
        if self.ino_type != InodeType::Directory {
            return Err(VfsError::NotADirectory);
        }
        let dir_perms = *self.permissions.read().unwrap();
        if !dir_perms.is_writable() {
            return Err(VfsError::PermissionDenied);
        }
        self.ops.symlink(name, target)
    }

    /// Attach an existing Inode (e.g. device node or mount point) to this directory.
    pub fn add_entry(&self, name: &str, node: Arc<Inode>) -> Result<(), VfsError> {
        if self.ino_type != InodeType::Directory {
            return Err(VfsError::NotADirectory);
        }
        let dir_perms = *self.permissions.read().unwrap();
        if !dir_perms.is_writable() {
            return Err(VfsError::PermissionDenied);
        }
        self.ops.add_entry(name, node)
    }

    /// Read the destination target of a symbolic link.
    pub fn readlink(&self) -> Result<String, VfsError> {
        if self.ino_type != InodeType::Symlink {
            return Err(VfsError::InvalidArgument);
        }
        self.ops.readlink()
    }

    /// Device I/O control.
    pub fn ioctl(&self, cmd: u32, arg: usize) -> Result<usize, VfsError> {
        self.ops.ioctl(cmd, arg)
    }
}

/// Open flags governing access permissions and creation semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpenFlags {
    pub read: bool,
    pub write: bool,
    pub create: bool,
    pub truncate: bool,
    pub append: bool,
    pub directory: bool,
    pub nonblocking: bool,
    pub exclusive: bool,
}

impl OpenFlags {
    pub const O_RDONLY: Self = Self {
        read: true,
        write: false,
        create: false,
        truncate: false,
        append: false,
        directory: false,
        nonblocking: false,
        exclusive: false,
    };

    pub const O_WRONLY: Self = Self {
        read: false,
        write: true,
        create: false,
        truncate: false,
        append: false,
        directory: false,
        nonblocking: false,
        exclusive: false,
    };

    pub const O_RDWR: Self = Self {
        read: true,
        write: true,
        create: false,
        truncate: false,
        append: false,
        directory: false,
        nonblocking: false,
        exclusive: false,
    };

    pub fn read_only() -> Self {
        Self::O_RDONLY
    }

    pub fn write_only() -> Self {
        Self::O_WRONLY
    }

    pub fn read_write() -> Self {
        Self::O_RDWR
    }

    pub fn create_or_truncate() -> Self {
        Self {
            read: true,
            write: true,
            create: true,
            truncate: true,
            append: false,
            directory: false,
            nonblocking: false,
            exclusive: false,
        }
    }

    pub fn append_only() -> Self {
        Self {
            read: false,
            write: true,
            create: true,
            truncate: false,
            append: true,
            directory: false,
            nonblocking: false,
            exclusive: false,
        }
    }

    pub fn directory() -> Self {
        Self {
            read: true,
            write: false,
            create: false,
            truncate: false,
            append: false,
            directory: true,
            nonblocking: false,
            exclusive: false,
        }
    }

    /// Translate POSIX open flags (O_RDONLY, O_WRONLY, O_RDWR, O_CREAT, etc.).
    pub fn from_posix(flags: i32) -> Self {
        let access_mode = flags & 0x0003;
        let read = access_mode == 0 || access_mode == 2; // O_RDONLY or O_RDWR
        let write = access_mode == 1 || access_mode == 2; // O_WRONLY or O_RDWR
        let create = (flags & 0o100) != 0; // O_CREAT
        let exclusive = (flags & 0o200) != 0; // O_EXCL
        let truncate = (flags & 0o1000) != 0; // O_TRUNC
        let append = (flags & 0o2000) != 0; // O_APPEND
        let directory = (flags & 0o200000) != 0; // O_DIRECTORY

        Self {
            read,
            write,
            create,
            truncate,
            append,
            directory,
            nonblocking: false,
            exclusive,
        }
    }

    /// Translate Windows NT DesiredAccess & Disposition flags.
    pub fn from_nt(desired_access: u32, disposition: u32) -> Self {
        // GENERIC_READ = 0x80000000, FILE_READ_DATA = 0x0001
        let read = (desired_access & 0x80000001) != 0;
        // GENERIC_WRITE = 0x40000000, FILE_WRITE_DATA = 0x0002
        let write = (desired_access & 0x40000002) != 0;
        // FILE_APPEND_DATA = 0x0004
        let append = (desired_access & 0x0004) != 0;

        // Dispositions:
        // FILE_SUPERSEDE = 0, FILE_OPEN = 1, FILE_CREATE = 2,
        // FILE_OPEN_IF = 3, FILE_OVERWRITE = 4, FILE_OVERWRITE_IF = 5
        let create = matches!(disposition, 0 | 2 | 3 | 5);
        let truncate = matches!(disposition, 0 | 4 | 5);
        let exclusive = disposition == 2;

        Self {
            read: if !read && !write { true } else { read },
            write,
            create,
            truncate,
            append,
            directory: false,
            nonblocking: false,
            exclusive,
        }
    }
}

/// Open File Descriptor representing an active handle to an Inode.
pub struct FileDescriptor {
    pub id: usize,
    pub inode: Arc<Inode>,
    pub offset: Mutex<u64>,
    pub flags: OpenFlags,
    pub path: String,
}

impl FileDescriptor {
    pub fn new(id: usize, inode: Arc<Inode>, flags: OpenFlags, path: String) -> Self {
        Self {
            id,
            inode,
            offset: Mutex::new(0),
            flags,
            path,
        }
    }

    /// Read data from the file descriptor, advancing the internal offset.
    pub fn read(&self, buf: &mut [u8]) -> Result<usize, VfsError> {
        if !self.flags.read {
            return Err(VfsError::PermissionDenied);
        }
        let mut offset_guard = self.offset.lock().unwrap();
        let bytes_read = self.inode.read(*offset_guard, buf)?;
        *offset_guard += bytes_read as u64;
        Ok(bytes_read)
    }

    /// Write data through the file descriptor, advancing the internal offset.
    pub fn write(&self, buf: &[u8]) -> Result<usize, VfsError> {
        if !self.flags.write {
            return Err(VfsError::PermissionDenied);
        }
        let mut offset_guard = self.offset.lock().unwrap();
        let write_offset = if self.flags.append {
            self.inode.get_size()
        } else {
            *offset_guard
        };

        let bytes_written = self.inode.write(write_offset, buf)?;
        *offset_guard = write_offset + bytes_written as u64;
        Ok(bytes_written)
    }

    /// Reposition the file descriptor offset.
    pub fn seek(&self, whence: SeekWhence) -> Result<u64, VfsError> {
        let mut offset_guard = self.offset.lock().unwrap();
        let current = *offset_guard;
        let file_size = self.inode.get_size();

        let new_offset = match whence {
            SeekWhence::Start(target) => target,
            SeekWhence::Current(diff) => {
                if diff < 0 && (-diff as u64) > current {
                    return Err(VfsError::InvalidArgument);
                }
                (current as i64 + diff) as u64
            }
            SeekWhence::End(diff) => {
                if diff < 0 && (-diff as u64) > file_size {
                    return Err(VfsError::InvalidArgument);
                }
                (file_size as i64 + diff) as u64
            }
        };

        *offset_guard = new_offset;
        Ok(new_offset)
    }

    /// Read directory contents if this descriptor references a directory.
    pub fn readdir(&self) -> Result<Vec<DirEntry>, VfsError> {
        if self.inode.ino_type != InodeType::Directory {
            return Err(VfsError::NotADirectory);
        }
        self.inode.readdir()
    }

    pub fn get_offset(&self) -> u64 {
        *self.offset.lock().unwrap()
    }

    pub fn set_offset(&self, offset: u64) {
        *self.offset.lock().unwrap() = offset;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct DummyFileOps {
        buf: RwLock<Vec<u8>>,
    }

    impl FileOperations for DummyFileOps {
        fn read(&self, offset: u64, buf: &mut [u8]) -> Result<usize, VfsError> {
            let data = self.buf.read().unwrap();
            if offset >= data.len() as u64 {
                return Ok(0);
            }
            let avail = &data[offset as usize..];
            let n = std::cmp::min(buf.len(), avail.len());
            buf[..n].copy_from_slice(&avail[..n]);
            Ok(n)
        }

        fn write(&self, offset: u64, buf: &[u8]) -> Result<usize, VfsError> {
            let mut data = self.buf.write().unwrap();
            let end = (offset as usize) + buf.len();
            if end > data.len() {
                data.resize(end, 0);
            }
            data[offset as usize..end].copy_from_slice(buf);
            Ok(buf.len())
        }
    }

    #[test]
    fn test_permissions_posix_and_nt() {
        let p_file = Permissions::default_file();
        assert!(p_file.is_readable());
        assert!(p_file.is_writable());
        assert!(!p_file.is_executable());

        let p_exec = Permissions::default_executable();
        assert!(p_exec.is_executable());

        let nt_attrs = Permissions::FILE_ATTRIBUTE_READONLY | Permissions::FILE_ATTRIBUTE_HIDDEN;
        let p_nt = Permissions::from_nt_attributes(nt_attrs);
        assert!(p_nt.readonly);
        assert!(p_nt.hidden);
        assert!(!p_nt.is_writable());
    }

    #[test]
    fn test_file_descriptor_seek_and_rw() {
        let ops = Arc::new(DummyFileOps {
            buf: RwLock::new(vec![b'A', b'B', b'C', b'D']),
        });
        let inode = Arc::new(Inode::new(100, InodeType::File, Permissions::default_file(), ops));
        inode.set_size(4);

        let fd = FileDescriptor::new(1, inode, OpenFlags::read_write(), "/test".into());

        let mut buf = [0u8; 2];
        let n = fd.read(&mut buf).unwrap();
        assert_eq!(n, 2);
        assert_eq!(&buf, b"AB");
        assert_eq!(fd.get_offset(), 2);

        // Seek forward
        let new_off = fd.seek(SeekWhence::Start(1)).unwrap();
        assert_eq!(new_off, 1);
        let n2 = fd.read(&mut buf).unwrap();
        assert_eq!(n2, 2);
        assert_eq!(&buf, b"BC");

        // Write
        fd.seek(SeekWhence::End(0)).unwrap();
        let written = fd.write(b"EF").unwrap();
        assert_eq!(written, 2);
        assert_eq!(fd.inode.get_size(), 6);
    }
}
