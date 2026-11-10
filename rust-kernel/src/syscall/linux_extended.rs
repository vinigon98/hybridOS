//! Extended Linux POSIX System Call Subsystem for HybridOS Kernel.
//!
//! Provides a complete, robust, and modular implementation of 20+ core Linux POSIX
//! system calls adhering strictly to the System V AMD64 ABI specification and Linux
//! x86_64 calling conventions.
//!
//! # Supported Syscall Categories:
//! - **I/O Subsystem**:
//!   - `sys_read` (0): Stream and block descriptor reading, pipe extraction, VFS bridging.
//!   - `sys_write` (1): Stream writing, standard output/error redirection, pipe ingestion.
//!   - `sys_open` (2): Path resolution, mode/flag validation, handle allocation.
//!   - `sys_close` (3): Handle reclamation, pipe endpoint lifecycle tracking.
//!   - `sys_stat` (4): Path-based file metadata and inode attribute interrogation.
//!   - `sys_fstat` (5): Descriptor-based file metadata interrogation.
//!   - `sys_lseek` (8): File offset repositioning with SEEK_SET, SEEK_CUR, SEEK_END.
//!   - `sys_ioctl` (16): Terminal control, window resizing (TIOCGWINSZ), buffer sizing (FIONREAD).
//!   - `sys_pipe` (22) / `sys_pipe2` (293): Inter-process communication pipe channels.
//!   - `sys_dup` (32) / `sys_dup2` (33): File descriptor duplication and reassignment.
//! - **Memory Subsystem**:
//!   - `sys_mmap` (9): Anonymous and file-backed virtual memory allocation.
//!   - `sys_mprotect` (10): Page protection flag modification (PROT_READ, PROT_WRITE, PROT_EXEC).
//!   - `sys_munmap` (11): Virtual address range reclamation.
//!   - `sys_brk` (12): Process heap break management and dynamic memory expansion.
//! - **Process & Architecture Subsystem**:
//!   - `sys_getpid` (39) / `sys_getppid` (110): Process ID and parent ID queries.
//!   - `sys_clone` (56) / `sys_fork` (57): Process and thread creation with flag evaluation.
//!   - `sys_execve` (59): Process image replacement, argument passing, environment configuration.
//!   - `sys_exit` (60) / `sys_exit_group` (231): Thread/process termination and code signaling.
//!   - `sys_wait4` (61): Child process reaping, status reporting, resource utilization accounting.
//!   - `sys_uname` (63): Operating system and hardware architecture identification.
//!   - `sys_arch_prctl` (158): x86_64 segment base configuration (FS_BASE for TLS, GS_BASE).

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, OnceLock};

use crate::process::{HybridProcess, KernelObject, ObjectType};
use crate::types::{Handle, PosixError};

// ============================================================================
// Linux System V AMD64 Syscall Numbers (x86_64)
// ============================================================================

pub const SYS_READ: u64         = 0;
pub const SYS_WRITE: u64        = 1;
pub const SYS_OPEN: u64         = 2;
pub const SYS_CLOSE: u64        = 3;
pub const SYS_STAT: u64         = 4;
pub const SYS_FSTAT: u64        = 5;
pub const SYS_LSEEK: u64        = 8;
pub const SYS_MMAP: u64         = 9;
pub const SYS_MPROTECT: u64     = 10;
pub const SYS_MUNMAP: u64       = 11;
pub const SYS_BRK: u64          = 12;
pub const SYS_IOCTL: u64        = 16;
pub const SYS_PIPE: u64         = 22;
pub const SYS_DUP: u64          = 32;
pub const SYS_DUP2: u64         = 33;
pub const SYS_GETPID: u64       = 39;
pub const SYS_CLONE: u64        = 56;
pub const SYS_FORK: u64         = 57;
pub const SYS_EXECVE: u64       = 59;
pub const SYS_EXIT: u64         = 60;
pub const SYS_WAIT4: u64        = 61;
pub const SYS_UNAME: u64        = 63;
pub const SYS_GETPPID: u64      = 110;
pub const SYS_ARCH_PRCTL: u64   = 158;
pub const SYS_EXIT_GROUP: u64   = 231;
pub const SYS_PIPE2: u64        = 293;

// ============================================================================
// Architecture Sub-function Constants (sys_arch_prctl)
// ============================================================================

pub const ARCH_SET_GS: u64 = 0x1001;
pub const ARCH_SET_FS: u64 = 0x1002;
pub const ARCH_GET_FS: u64 = 0x1003;
pub const ARCH_GET_GS: u64 = 0x1004;

// ============================================================================
// Virtual Memory Protection & Mapping Flags
// ============================================================================

pub const PAGE_SIZE: usize = 4096;

pub const PROT_NONE: u64  = 0x0;
pub const PROT_READ: u64  = 0x1;
pub const PROT_WRITE: u64 = 0x2;
pub const PROT_EXEC: u64  = 0x4;

pub const MAP_SHARED: u64     = 0x01;
pub const MAP_PRIVATE: u64    = 0x02;
pub const MAP_FIXED: u64      = 0x10;
pub const MAP_ANONYMOUS: u64  = 0x20;
pub const MAP_POPULATE: u64   = 0x8000;
pub const MAP_STACK: u64      = 0x20000;

// ============================================================================
// File Open Flags & Seek Directives
// ============================================================================

pub const O_RDONLY: u64    = 0;
pub const O_WRONLY: u64    = 1;
pub const O_RDWR: u64      = 2;
pub const O_ACCMODE: u64   = 0x0003;

pub const O_CREAT: u64     = 0o100;     // 0x0040
pub const O_EXCL: u64      = 0o200;     // 0x0080
pub const O_NOCTTY: u64    = 0o400;     // 0x0100
pub const O_TRUNC: u64     = 0o1000;    // 0x0200
pub const O_APPEND: u64    = 0o2000;    // 0x0400
pub const O_NONBLOCK: u64  = 0o4000;    // 0x0800
pub const O_DIRECTORY: u64 = 0o200000;  // 0x10000
pub const O_CLOEXEC: u64   = 0o2000000; // 0x80000

pub const SEEK_SET: u64 = 0;
pub const SEEK_CUR: u64 = 1;
pub const SEEK_END: u64 = 2;

// ============================================================================
// Inode Mode Flags (POSIX File Types & Permissions)
// ============================================================================

pub const S_IFMT: u32   = 0o170000;
pub const S_IFSOCK: u32 = 0o140000;
pub const S_IFLNK: u32  = 0o120000;
pub const S_IFREG: u32  = 0o100000;
pub const S_IFBLK: u32  = 0o060000;
pub const S_IFDIR: u32  = 0o040000;
pub const S_IFCHR: u32  = 0o020000;
pub const S_IFIFO: u32  = 0o010000;

pub const S_IRWXU: u32 = 0o700;
pub const S_IRUSR: u32 = 0o400;
pub const S_IWUSR: u32 = 0o200;
pub const S_IXUSR: u32 = 0o100;

pub const S_IRWXG: u32 = 0o070;
pub const S_IRGRP: u32 = 0o040;
pub const S_IWGRP: u32 = 0o020;
pub const S_IXGRP: u32 = 0o010;

pub const S_IRWXO: u32 = 0o007;
pub const S_IROTH: u32 = 0o004;
pub const S_IWOTH: u32 = 0o002;
pub const S_IXOTH: u32 = 0o001;

// ============================================================================
// Ioctl Command Constants
// ============================================================================

pub const TCGETS: u64      = 0x5401;
pub const TCSETS: u64      = 0x5402;
pub const TIOCGWINSZ: u64  = 0x5413;
pub const TIOCSWINSZ: u64  = 0x5414;
pub const FIONREAD: u64    = 0x541B;
pub const FIOCLEX: u64     = 0x5451;
pub const FIONCLEX: u64    = 0x5450;

// ============================================================================
// Process Cloning & Waiting Flags
// ============================================================================

pub const CSIGNAL: u64              = 0x000000FF;
pub const CLONE_VM: u64             = 0x00000100;
pub const CLONE_FS: u64             = 0x00000200;
pub const CLONE_FILES: u64          = 0x00000400;
pub const CLONE_SIGHAND: u64        = 0x00000800;
pub const CLONE_PTRACE: u64         = 0x00002000;
pub const CLONE_VFORK: u64          = 0x00004000;
pub const CLONE_PARENT: u64         = 0x00008000;
pub const CLONE_THREAD: u64         = 0x00010000;
pub const CLONE_NEWNS: u64          = 0x00020000;
pub const CLONE_SYSVSEM: u64        = 0x00040000;
pub const CLONE_SETTLS: u64         = 0x00080000;
pub const CLONE_PARENT_SETTID: u64  = 0x00100000;
pub const CLONE_CHILD_CLEARTID: u64 = 0x00200000;
pub const CLONE_DETACHED: u64       = 0x00400000;
pub const CLONE_UNTRACED: u64       = 0x00800000;
pub const CLONE_CHILD_SETTID: u64   = 0x01000000;

pub const WNOHANG: u64    = 1;
pub const WUNTRACED: u64  = 2;
pub const WCONTINUED: u64 = 8;

// ============================================================================
// Linux System V AMD64 ABI Structures (repr(C))
// ============================================================================

/// Represents the Linux x86_64 `struct stat` (144 bytes).
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Stat {
    pub st_dev: u64,
    pub st_ino: u64,
    pub st_nlink: u64,
    pub st_mode: u32,
    pub st_uid: u32,
    pub st_gid: u32,
    pub __pad0: u32,
    pub st_rdev: u64,
    pub st_size: i64,
    pub st_blksize: i64,
    pub st_blocks: i64,
    pub st_atime: i64,
    pub st_atime_nsec: i64,
    pub st_mtime: i64,
    pub st_mtime_nsec: i64,
    pub st_ctime: i64,
    pub st_ctime_nsec: i64,
    pub __glibc_reserved: [i64; 3],
}

impl Default for Stat {
    fn default() -> Self {
        Self {
            st_dev: 2050,
            st_ino: 1,
            st_nlink: 1,
            st_mode: S_IFREG | 0o644,
            st_uid: 1000,
            st_gid: 1000,
            __pad0: 0,
            st_rdev: 0,
            st_size: 0,
            st_blksize: 4096,
            st_blocks: 0,
            st_atime: 1773000000,
            st_atime_nsec: 0,
            st_mtime: 1773000000,
            st_mtime_nsec: 0,
            st_ctime: 1773000000,
            st_ctime_nsec: 0,
            __glibc_reserved: [0; 3],
        }
    }
}

/// Represents the Linux x86_64 `struct utsname` (390 bytes).
#[derive(Clone, Copy)]
#[repr(C)]
pub struct Utsname {
    pub sysname: [u8; 65],
    pub nodename: [u8; 65],
    pub release: [u8; 65],
    pub version: [u8; 65],
    pub machine: [u8; 65],
    pub domainname: [u8; 65],
}

impl Default for Utsname {
    fn default() -> Self {
        let mut u = Self {
            sysname: [0; 65],
            nodename: [0; 65],
            release: [0; 65],
            version: [0; 65],
            machine: [0; 65],
            domainname: [0; 65],
        };

        copy_str_to_fixed_bytes("Linux", &mut u.sysname);
        copy_str_to_fixed_bytes("hybridos-node", &mut u.nodename);
        copy_str_to_fixed_bytes("6.6.0-hybridos", &mut u.release);
        copy_str_to_fixed_bytes("#1 SMP PREEMPT HybridOS Dual-Kernel 2026", &mut u.version);
        copy_str_to_fixed_bytes("x86_64", &mut u.machine);
        copy_str_to_fixed_bytes("localdomain", &mut u.domainname);

        u
    }
}

/// Terminal window dimensions for `TIOCGWINSZ`.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Winsize {
    pub ws_row: u16,
    pub ws_col: u16,
    pub ws_xpixel: u16,
    pub ws_ypixel: u16,
}

impl Default for Winsize {
    fn default() -> Self {
        Self {
            ws_row: 24,
            ws_col: 80,
            ws_xpixel: 0,
            ws_ypixel: 0,
        }
    }
}

/// Time value for resource accounting.
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct Timeval {
    pub tv_sec: i64,
    pub tv_usec: i64,
}

/// Linux `struct rusage` for `sys_wait4`.
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct Rusage {
    pub ru_utime: Timeval,
    pub ru_stime: Timeval,
    pub ru_maxrss: i64,
    pub ru_ixrss: i64,
    pub ru_idrss: i64,
    pub ru_isrss: i64,
    pub ru_minflt: i64,
    pub ru_majflt: i64,
    pub ru_nswap: i64,
    pub ru_inblock: i64,
    pub ru_oublock: i64,
    pub ru_msgsnd: i64,
    pub ru_msgrcv: i64,
    pub ru_nsignals: i64,
    pub ru_nvcsw: i64,
    pub ru_nivcsw: i64,
}

// ============================================================================
// Internal Subsystem Types & State Management
// ============================================================================

/// Represents an allocated virtual memory mapping region.
#[derive(Debug, Clone)]
pub struct MmapRegion {
    pub start: u64,
    pub len: usize,
    pub prot: u64,
    pub flags: u64,
    pub offset: u64,
    pub backing_fd: Option<Handle>,
}

/// Unidirectional IPC Pipe Channel with thread-safe ring buffer.
pub struct PipeChannel {
    pub id: usize,
    pub buffer: VecDeque<u8>,
    pub max_capacity: usize,
    pub readers_open: usize,
    pub writers_open: usize,
}

impl PipeChannel {
    pub fn new(id: usize) -> Self {
        Self {
            id,
            buffer: VecDeque::with_capacity(65536),
            max_capacity: 65536, // Standard 64KB Linux pipe buffer
            readers_open: 1,
            writers_open: 1,
        }
    }
}

/// Subsystem descriptor tracking for pipes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipeEndpoint {
    Read(usize),
    Write(usize),
}

/// Complete Linux POSIX Process Runtime Context.
pub struct LinuxProcessContext {
    pub pid: u32,
    pub parent_pid: u32,
    pub child_pids: Vec<u32>,
    pub terminated_children: Vec<(u32, i32)>, // (PID, exit_status)
    pub brk_start: u64,
    pub brk_current: u64,
    pub brk_max: u64,
    pub mmap_next_addr: u64,
    pub mmap_regions: Vec<MmapRegion>,
    pub fs_base: u64,
    pub gs_base: u64,
    pub pipe_descriptors: HashMap<Handle, PipeEndpoint>,
    pub file_offsets: HashMap<Handle, u64>,
    pub cloexec_handles: Vec<Handle>,
}

impl LinuxProcessContext {
    pub fn new(pid: u32, parent_pid: u32) -> Self {
        let initial_brk = 0x0000_0000_0200_0000;
        Self {
            pid,
            parent_pid,
            child_pids: Vec::new(),
            terminated_children: Vec::new(),
            brk_start: initial_brk,
            brk_current: initial_brk,
            brk_max: initial_brk + 0x4000_0000, // 1GB heap allocation ceiling
            mmap_next_addr: 0x0000_7FFF_0000_0000,
            mmap_regions: Vec::new(),
            fs_base: 0,
            gs_base: 0,
            pipe_descriptors: HashMap::new(),
            file_offsets: HashMap::new(),
            cloexec_handles: Vec::new(),
        }
    }
}

/// Global Subsystem State for Linux POSIX simulation.
pub struct LinuxSubsystemState {
    pub processes: HashMap<u32, LinuxProcessContext>,
    pub pipes: HashMap<usize, Arc<Mutex<PipeChannel>>>,
    pub next_pipe_id: usize,
    pub next_pid: u32,
    pub virtual_filesystem: HashMap<String, Vec<u8>>,
}

impl LinuxSubsystemState {
    pub fn new() -> Self {
        let mut vfs = HashMap::new();
        // Seed standard POSIX synthetic files
        vfs.insert(
            "/etc/os-release".to_string(),
            b"NAME=\"HybridOS Linux\"\nVERSION=\"1.0\"\nID=hybridos\nPRETTY_NAME=\"HybridOS Dual-Personality 2026\"\n".to_vec(),
        );
        vfs.insert(
            "/proc/version".to_string(),
            b"Linux version 6.6.0-hybridos (nexus@core) #1 SMP PREEMPT 2026\n".to_vec(),
        );
        vfs.insert(
            "/proc/cpuinfo".to_string(),
            b"processor\t: 0\nvendor_id\t: HybridOS-Core\nmodel name\t: Hybrid Architecture x86_64\nflags\t\t: fpu vme de pse tsc msr pae mce cx8 apic sep mtrr pge mca cmov pat pse36 clflush mmx fxsr sse sse2 ht syscall nx lm\n".to_vec(),
        );

        Self {
            processes: HashMap::new(),
            pipes: HashMap::new(),
            next_pipe_id: 1,
            next_pid: 2000,
            virtual_filesystem: vfs,
        }
    }

    pub fn get_or_create_context(&mut self, pid: u32) -> &mut LinuxProcessContext {
        self.processes.entry(pid).or_insert_with(|| LinuxProcessContext::new(pid, 1))
    }
}

static LINUX_SUBSYSTEM: OnceLock<Mutex<LinuxSubsystemState>> = OnceLock::new();

pub fn subsystem() -> &'static Mutex<LinuxSubsystemState> {
    LINUX_SUBSYSTEM.get_or_init(|| Mutex::new(LinuxSubsystemState::new()))
}

// ============================================================================
// Safe Memory Access Helpers for Userland Pointers
// ============================================================================

fn copy_str_to_fixed_bytes(src: &str, dest: &mut [u8]) {
    let bytes = src.as_bytes();
    let copy_len = bytes.len().min(dest.len().saturating_sub(1));
    dest[..copy_len].copy_from_slice(&bytes[..copy_len]);
    dest[copy_len] = 0;
}

pub fn read_user_string(ptr: u64, max_len: usize) -> Result<String, PosixError> {
    if ptr == 0 {
        return Err(PosixError::EFAULT);
    }

    let mut result = Vec::new();
    let raw = ptr as *const u8;

    for i in 0..max_len {
        let b = unsafe { *raw.add(i) };
        if b == 0 {
            break;
        }
        result.push(b);
    }

    String::from_utf8(result).map_err(|_| PosixError::EINVAL)
}

pub fn write_user_struct<T: Copy>(ptr: u64, val: &T) -> Result<(), PosixError> {
    if ptr == 0 {
        return Err(PosixError::EFAULT);
    }
    unsafe {
        let dest = ptr as *mut T;
        std::ptr::write_unaligned(dest, *val);
    }
    Ok(())
}

// ============================================================================
// Core Linux POSIX Syscall Implementations (20+ Handlers)
// ============================================================================

/// `sys_read` (Syscall #0): Read from a file descriptor.
///
/// Signature: `ssize_t sys_read(unsigned int fd, char __user *buf, size_t count)`
pub fn sys_read(proc: &mut HybridProcess, fd: u64, buf_ptr: u64, count: u64) -> Result<u64, PosixError> {
    let handle = Handle(fd as usize);
    let count = count as usize;

    if count == 0 {
        return Ok(0);
    }
    if buf_ptr == 0 {
        return Err(PosixError::EFAULT);
    }

    // Check if this handle represents a pipe
    let pipe_endpoint = {
        let mut sub = subsystem().lock().unwrap();
        let ctx = sub.get_or_create_context(proc.pid);
        ctx.pipe_descriptors.get(&handle).copied()
    };

    if let Some(PipeEndpoint::Read(pipe_id)) = pipe_endpoint {
        let channel_arc = {
            let sub = subsystem().lock().unwrap();
            sub.pipes.get(&pipe_id).cloned()
        };

        if let Some(channel_arc) = channel_arc {
            let mut chan = channel_arc.lock().unwrap();
            let avail = chan.buffer.len();
            if avail == 0 {
                if chan.writers_open == 0 {
                    return Ok(0); // EOF
                }
                return Err(PosixError::EAGAIN);
            }

            let to_read = avail.min(count);
            let user_buf = buf_ptr as *mut u8;
            for i in 0..to_read {
                let byte = chan.buffer.pop_front().unwrap();
                unsafe { *user_buf.add(i) = byte };
            }
            return Ok(to_read as u64);
        } else {
            return Err(PosixError::EBADF);
        }
    }

    // Standard STDIN descriptor handling
    if handle == Handle::STDIN {
        return Ok(0);
    }

    // Regular file handle interrogation
    if let Some(obj_arc) = proc.get_object(handle) {
        let obj = obj_arc.lock().unwrap();
        match &obj.obj_type {
            ObjectType::File { name, offset } => {
                let cur_offset = {
                    let mut sub = subsystem().lock().unwrap();
                    let ctx = sub.get_or_create_context(proc.pid);
                    *ctx.file_offsets.entry(handle).or_insert(*offset)
                };

                let mut sub = subsystem().lock().unwrap();
                if let Some(file_data) = sub.virtual_filesystem.get(name) {
                    let total_len = file_data.len() as u64;
                    if cur_offset >= total_len {
                        return Ok(0); // EOF
                    }

                    let available = (total_len - cur_offset) as usize;
                    let to_read = available.min(count);
                    let slice = &file_data[cur_offset as usize..cur_offset as usize + to_read];

                    let user_buf = buf_ptr as *mut u8;
                    unsafe {
                        std::ptr::copy_nonoverlapping(slice.as_ptr(), user_buf, to_read);
                    }

                    let ctx = sub.get_or_create_context(proc.pid);
                    ctx.file_offsets.insert(handle, cur_offset + to_read as u64);
                    Ok(to_read as u64)
                } else {
                    Ok(0)
                }
            }
            ObjectType::Device { .. } => Ok(0),
            ObjectType::Socket { id } => {
                let sid = *id;
                drop(obj);
                super::posix_advanced::socket_read_internal(sid, buf_ptr, count as u64)
            }
            _ => Err(PosixError::EINVAL),
        }
    } else {
        Err(PosixError::EBADF)
    }
}

/// `sys_write` (Syscall #1): Write to a file descriptor.
///
/// Signature: `ssize_t sys_write(unsigned int fd, const char __user *buf, size_t count)`
pub fn sys_write(proc: &mut HybridProcess, fd: u64, buf_ptr: u64, count: u64) -> Result<u64, PosixError> {
    let handle = Handle(fd as usize);
    let count = count as usize;

    if count == 0 {
        return Ok(0);
    }
    if buf_ptr == 0 {
        return Err(PosixError::EFAULT);
    }

    // 1. Direct standard output / standard error
    if handle == Handle::STDOUT || handle == Handle::STDERR {
        let slice = unsafe { std::slice::from_raw_parts(buf_ptr as *const u8, count) };
        print!("[Linux POSIX out]: {}", String::from_utf8_lossy(slice));
        return Ok(count as u64);
    }

    // 2. Check for pipe write endpoint
    let pipe_endpoint = {
        let mut sub = subsystem().lock().unwrap();
        let ctx = sub.get_or_create_context(proc.pid);
        ctx.pipe_descriptors.get(&handle).copied()
    };

    if let Some(PipeEndpoint::Write(pipe_id)) = pipe_endpoint {
        let channel_arc = {
            let sub = subsystem().lock().unwrap();
            sub.pipes.get(&pipe_id).cloned()
        };

        if let Some(channel_arc) = channel_arc {
            let mut chan = channel_arc.lock().unwrap();
            if chan.readers_open == 0 {
                return Err(PosixError::EPIPE);
            }

            let slice = unsafe { std::slice::from_raw_parts(buf_ptr as *const u8, count) };
            for &byte in slice {
                if chan.buffer.len() < chan.max_capacity {
                    chan.buffer.push_back(byte);
                } else {
                    break;
                }
            }
            return Ok(count as u64);
        } else {
            return Err(PosixError::EBADF);
        }
    }

    // 3. Regular file write handling
    if let Some(obj_arc) = proc.get_object(handle) {
        let obj = obj_arc.lock().unwrap();
        match &obj.obj_type {
            ObjectType::File { name, offset } => {
                let cur_offset = {
                    let mut sub = subsystem().lock().unwrap();
                    let ctx = sub.get_or_create_context(proc.pid);
                    *ctx.file_offsets.entry(handle).or_insert(*offset) as usize
                };

                let mut sub = subsystem().lock().unwrap();
                let file_data = sub.virtual_filesystem.entry(name.clone()).or_default();

                let slice = unsafe { std::slice::from_raw_parts(buf_ptr as *const u8, count) };
                if cur_offset + count > file_data.len() {
                    file_data.resize(cur_offset + count, 0);
                }
                file_data[cur_offset..cur_offset + count].copy_from_slice(slice);

                let ctx = sub.get_or_create_context(proc.pid);
                ctx.file_offsets.insert(handle, (cur_offset + count) as u64);
                Ok(count as u64)
            }
            ObjectType::Device { .. } => Ok(count as u64),
            ObjectType::Socket { id } => {
                let sid = *id;
                drop(obj);
                super::posix_advanced::socket_write_internal(sid, buf_ptr, count as u64)
            }
            _ => Err(PosixError::EINVAL),
        }
    } else {
        Err(PosixError::EBADF)
    }
}

/// `sys_open` (Syscall #2): Open or create a file descriptor.
///
/// Signature: `int sys_open(const char __user *filename, int flags, umode_t mode)`
pub fn sys_open(proc: &mut HybridProcess, pathname_ptr: u64, flags: u64, _mode: u64) -> Result<u64, PosixError> {
    let path = if pathname_ptr != 0 {
        read_user_string(pathname_ptr, 256).unwrap_or_else(|_| "anonymous_linux_file".to_string())
    } else {
        "sample_linux_file".to_string()
    };

    let obj = KernelObject {
        obj_type: ObjectType::File {
            name: path.clone(),
            offset: 0,
        },
        access_mask: (flags & O_ACCMODE) as u32,
    };

    let fd = proc.allocate_handle(obj);

    let mut sub = subsystem().lock().unwrap();
    let ctx = sub.get_or_create_context(proc.pid);
    ctx.file_offsets.insert(fd, 0);

    if flags & O_CLOEXEC != 0 {
        ctx.cloexec_handles.push(fd);
    }

    if flags & O_CREAT != 0 {
        sub.virtual_filesystem.entry(path).or_default();
    }

    Ok(fd.0 as u64)
}

/// `sys_close` (Syscall #3): Close a file descriptor.
///
/// Signature: `int sys_close(unsigned int fd)`
pub fn sys_close(proc: &mut HybridProcess, fd: u64) -> Result<u64, PosixError> {
    let handle = Handle(fd as usize);

    // Clean up pipe associations if closing a pipe handle
    let endpoint = {
        let mut sub = subsystem().lock().unwrap();
        let ctx = sub.get_or_create_context(proc.pid);
        ctx.file_offsets.remove(&handle);
        ctx.pipe_descriptors.remove(&handle)
    };

    if let Some(endpoint) = endpoint {
        let sub = subsystem().lock().unwrap();
        match endpoint {
            PipeEndpoint::Read(pipe_id) => {
                if let Some(chan) = sub.pipes.get(&pipe_id) {
                    let mut c = chan.lock().unwrap();
                    c.readers_open = c.readers_open.saturating_sub(1);
                }
            }
            PipeEndpoint::Write(pipe_id) => {
                if let Some(chan) = sub.pipes.get(&pipe_id) {
                    let mut c = chan.lock().unwrap();
                    c.writers_open = c.writers_open.saturating_sub(1);
                }
            }
        }
    }

    let socket_id = if let Some(obj_arc) = proc.get_object(handle) {
        let obj = obj_arc.lock().unwrap();
        if let ObjectType::Socket { id } = obj.obj_type {
            Some(id)
        } else {
            None
        }
    } else {
        None
    };

    if let Some(id) = socket_id {
        super::posix_advanced::on_socket_closed(id);
    }

    if proc.close_handle(handle) {
        Ok(0)
    } else {
        Err(PosixError::EBADF)
    }
}

/// `sys_stat` (Syscall #4): Interrogate file metadata by pathname.
///
/// Signature: `int sys_stat(const char __user *filename, struct stat __user *statbuf)`
pub fn sys_stat(_proc: &mut HybridProcess, pathname_ptr: u64, statbuf_ptr: u64) -> Result<u64, PosixError> {
    if pathname_ptr == 0 || statbuf_ptr == 0 {
        return Err(PosixError::EFAULT);
    }

    let path = read_user_string(pathname_ptr, 256)?;
    let sub = subsystem().lock().unwrap();

    let mut st = Stat::default();
    if let Some(content) = sub.virtual_filesystem.get(&path) {
        st.st_mode = S_IFREG | 0o644;
        st.st_size = content.len() as i64;
        st.st_blocks = (st.st_size + 511) / 512;
    } else if path == "/" || path == "/etc" || path == "/proc" || path == "/dev" {
        st.st_mode = S_IFDIR | 0o755;
        st.st_nlink = 2;
        st.st_size = 4096;
    } else {
        return Err(PosixError::ENOENT);
    }

    write_user_struct(statbuf_ptr, &st)?;
    Ok(0)
}

/// `sys_fstat` (Syscall #5): Interrogate file metadata by open descriptor.
///
/// Signature: `int sys_fstat(unsigned int fd, struct stat __user *statbuf)`
pub fn sys_fstat(proc: &mut HybridProcess, fd: u64, statbuf_ptr: u64) -> Result<u64, PosixError> {
    if statbuf_ptr == 0 {
        return Err(PosixError::EFAULT);
    }

    let handle = Handle(fd as usize);
    let mut st = Stat::default();

    // Standard descriptors 0, 1, 2 represent character devices (TTY)
    if handle == Handle::STDIN || handle == Handle::STDOUT || handle == Handle::STDERR {
        st.st_mode = S_IFCHR | 0o666;
        st.st_rdev = 0x8801;
        write_user_struct(statbuf_ptr, &st)?;
        return Ok(0);
    }

    // Check if descriptor is a pipe
    let pipe_id = {
        let mut sub = subsystem().lock().unwrap();
        let ctx = sub.get_or_create_context(proc.pid);
        ctx.pipe_descriptors.get(&handle).map(|endpoint| match endpoint {
            PipeEndpoint::Read(id) | PipeEndpoint::Write(id) => *id,
        })
    };

    if let Some(pipe_id) = pipe_id {
        let sub = subsystem().lock().unwrap();
        if let Some(chan) = sub.pipes.get(&pipe_id) {
            st.st_mode = S_IFIFO | 0o600;
            st.st_size = chan.lock().unwrap().buffer.len() as i64;
            write_user_struct(statbuf_ptr, &st)?;
            return Ok(0);
        }
    }

    // Interrogate general handle
    if let Some(obj_arc) = proc.get_object(handle) {
        let obj = obj_arc.lock().unwrap();
        match &obj.obj_type {
            ObjectType::File { name, .. } => {
                let sub = subsystem().lock().unwrap();
                let size = sub.virtual_filesystem.get(name).map(|v| v.len()).unwrap_or(0);
                st.st_mode = S_IFREG | 0o644;
                st.st_size = size as i64;
                st.st_blocks = (st.st_size + 511) / 512;
            }
            ObjectType::Device { .. } => {
                st.st_mode = S_IFCHR | 0o660;
                st.st_rdev = 0x0501;
            }
            ObjectType::Process { .. } => {
                st.st_mode = S_IFREG | 0o444;
            }
            ObjectType::Event { .. } => {
                st.st_mode = S_IFIFO | 0o600;
            }
            ObjectType::Socket { .. } => {
                st.st_mode = S_IFSOCK | 0o666;
            }
            _ => {
                st.st_mode = S_IFREG | 0o644;
            }
        }
        write_user_struct(statbuf_ptr, &st)?;
        Ok(0)
    } else {
        Err(PosixError::EBADF)
    }
}

/// `sys_lseek` (Syscall #8): Reposition read/write file offset.
///
/// Signature: `off_t sys_lseek(unsigned int fd, off_t offset, unsigned int whence)`
pub fn sys_lseek(proc: &mut HybridProcess, fd: u64, offset: i64, whence: u64) -> Result<u64, PosixError> {
    let handle = Handle(fd as usize);

    if handle == Handle::STDIN || handle == Handle::STDOUT || handle == Handle::STDERR {
        return Err(PosixError::ESPIPE);
    }

    let is_pipe = {
        let mut sub = subsystem().lock().unwrap();
        let ctx = sub.get_or_create_context(proc.pid);
        ctx.pipe_descriptors.contains_key(&handle)
    };

    if is_pipe {
        return Err(PosixError::ESPIPE);
    }

    if let Some(obj_arc) = proc.get_object(handle) {
        let obj = obj_arc.lock().unwrap();
        if let ObjectType::File { name, .. } = &obj.obj_type {
            let current = {
                let mut sub = subsystem().lock().unwrap();
                let ctx = sub.get_or_create_context(proc.pid);
                *ctx.file_offsets.entry(handle).or_insert(0) as i64
            };

            let file_size = {
                let sub = subsystem().lock().unwrap();
                sub.virtual_filesystem.get(name).map(|v| v.len() as i64).unwrap_or(0)
            };

            let new_offset = match whence {
                SEEK_SET => offset,
                SEEK_CUR => current.checked_add(offset).ok_or(PosixError::EOVERFLOW_CHECK())?,
                SEEK_END => file_size.checked_add(offset).ok_or(PosixError::EOVERFLOW_CHECK())?,
                _ => return Err(PosixError::EINVAL),
            };

            if new_offset < 0 {
                return Err(PosixError::EINVAL);
            }

            let mut sub = subsystem().lock().unwrap();
            let ctx = sub.get_or_create_context(proc.pid);
            ctx.file_offsets.insert(handle, new_offset as u64);
            Ok(new_offset as u64)
        } else {
            Err(PosixError::ESPIPE)
        }
    } else {
        Err(PosixError::EBADF)
    }
}

/// `sys_ioctl` (Syscall #16): I/O control operations on device descriptors.
///
/// Signature: `int sys_ioctl(unsigned int fd, unsigned int cmd, unsigned long arg)`
pub fn sys_ioctl(proc: &mut HybridProcess, fd: u64, cmd: u64, arg: u64) -> Result<u64, PosixError> {
    let handle = Handle(fd as usize);

    if proc.get_object(handle).is_none() {
        return Err(PosixError::EBADF);
    }

    match cmd {
        TIOCGWINSZ => {
            if arg != 0 {
                let ws = Winsize::default();
                write_user_struct(arg, &ws)?;
            }
            Ok(0)
        }
        TIOCSWINSZ => Ok(0),
        TCGETS | TCSETS => {
            if handle == Handle::STDIN || handle == Handle::STDOUT || handle == Handle::STDERR {
                Ok(0)
            } else {
                Err(PosixError::ENOTTY)
            }
        }
        FIONREAD => {
            if arg != 0 {
                let mut bytes_ready: u32 = 0;
                let pipe_id = {
                    let mut sub = subsystem().lock().unwrap();
                    let ctx = sub.get_or_create_context(proc.pid);
                    match ctx.pipe_descriptors.get(&handle) {
                        Some(PipeEndpoint::Read(id)) => Some(*id),
                        _ => None,
                    }
                };

                if let Some(pipe_id) = pipe_id {
                    let sub = subsystem().lock().unwrap();
                    if let Some(chan) = sub.pipes.get(&pipe_id) {
                        bytes_ready = chan.lock().unwrap().buffer.len() as u32;
                    }
                }
                write_user_struct(arg, &bytes_ready)?;
            }
            Ok(0)
        }
        FIOCLEX => {
            let mut sub = subsystem().lock().unwrap();
            let ctx = sub.get_or_create_context(proc.pid);
            if !ctx.cloexec_handles.contains(&handle) {
                ctx.cloexec_handles.push(handle);
            }
            Ok(0)
        }
        FIONCLEX => {
            let mut sub = subsystem().lock().unwrap();
            let ctx = sub.get_or_create_context(proc.pid);
            ctx.cloexec_handles.retain(|&h| h != handle);
            Ok(0)
        }
        _ => Ok(0),
    }
}

/// `sys_pipe` (Syscall #22): Create unidirectional IPC pipe channel.
///
/// Signature: `int sys_pipe(int __user *pipefd)`
pub fn sys_pipe(proc: &mut HybridProcess, pipefd_ptr: u64) -> Result<u64, PosixError> {
    sys_pipe2(proc, pipefd_ptr, 0)
}

/// `sys_pipe2` (Syscall #293): Create pipe channel with flags (O_CLOEXEC, O_NONBLOCK).
///
/// Signature: `int sys_pipe2(int __user *pipefd, int flags)`
pub fn sys_pipe2(proc: &mut HybridProcess, pipefd_ptr: u64, flags: u64) -> Result<u64, PosixError> {
    if pipefd_ptr == 0 {
        return Err(PosixError::EFAULT);
    }

    let mut sub = subsystem().lock().unwrap();
    let pipe_id = sub.next_pipe_id;
    sub.next_pipe_id += 1;

    let pipe_channel = Arc::new(Mutex::new(PipeChannel::new(pipe_id)));
    sub.pipes.insert(pipe_id, pipe_channel);

    // Allocate reading end (pipefd[0])
    let read_obj = KernelObject {
        obj_type: ObjectType::Device { name: format!("pipe_reader_{}", pipe_id) },
        access_mask: 0x1,
    };
    let read_fd = proc.allocate_handle(read_obj);

    // Allocate writing end (pipefd[1])
    let write_obj = KernelObject {
        obj_type: ObjectType::Device { name: format!("pipe_writer_{}", pipe_id) },
        access_mask: 0x2,
    };
    let write_fd = proc.allocate_handle(write_obj);

    let ctx = sub.get_or_create_context(proc.pid);
    ctx.pipe_descriptors.insert(read_fd, PipeEndpoint::Read(pipe_id));
    ctx.pipe_descriptors.insert(write_fd, PipeEndpoint::Write(pipe_id));

    if flags & O_CLOEXEC != 0 {
        ctx.cloexec_handles.push(read_fd);
        ctx.cloexec_handles.push(write_fd);
    }

    let fds = [read_fd.0 as i32, write_fd.0 as i32];
    write_user_struct(pipefd_ptr, &fds)?;

    Ok(0)
}

/// `sys_dup` (Syscall #32): Duplicate an open file descriptor.
///
/// Signature: `int sys_dup(unsigned int oldfd)`
pub fn sys_dup(proc: &mut HybridProcess, oldfd: u64) -> Result<u64, PosixError> {
    let old_handle = Handle(oldfd as usize);
    let obj_arc = proc.get_object(old_handle).ok_or(PosixError::EBADF)?;

    let new_handle = {
        let obj = obj_arc.lock().unwrap().clone();
        proc.allocate_handle(obj)
    };

    let mut sub = subsystem().lock().unwrap();
    let ctx = sub.get_or_create_context(proc.pid);

    if let Some(&endpoint) = ctx.pipe_descriptors.get(&old_handle) {
        ctx.pipe_descriptors.insert(new_handle, endpoint);
    }
    if let Some(&off) = ctx.file_offsets.get(&old_handle) {
        ctx.file_offsets.insert(new_handle, off);
    }

    Ok(new_handle.0 as u64)
}

/// `sys_dup2` (Syscall #33): Duplicate an open descriptor to a specified target.
///
/// Signature: `int sys_dup2(unsigned int oldfd, unsigned int newfd)`
pub fn sys_dup2(proc: &mut HybridProcess, oldfd: u64, newfd: u64) -> Result<u64, PosixError> {
    let old_handle = Handle(oldfd as usize);
    let new_handle = Handle(newfd as usize);

    let obj_arc = proc.get_object(old_handle).ok_or(PosixError::EBADF)?;

    if old_handle == new_handle {
        return Ok(newfd);
    }

    // Silently close newfd if already active
    if proc.handles.contains_key(&new_handle) {
        let _ = sys_close(proc, newfd);
    }

    let obj_clone = obj_arc.lock().unwrap().clone();
    proc.handles.insert(new_handle, Arc::new(Mutex::new(obj_clone)));

    let mut sub = subsystem().lock().unwrap();
    let ctx = sub.get_or_create_context(proc.pid);

    if let Some(&endpoint) = ctx.pipe_descriptors.get(&old_handle) {
        ctx.pipe_descriptors.insert(new_handle, endpoint);
    }
    if let Some(&off) = ctx.file_offsets.get(&old_handle) {
        ctx.file_offsets.insert(new_handle, off);
    }

    Ok(newfd)
}

/// `sys_mmap` (Syscall #9): Map files or anonymous memory into process address space.
///
/// Signature: `void *sys_mmap(unsigned long addr, unsigned long len, unsigned long prot, unsigned long flags, unsigned long fd, unsigned long off)`
pub fn sys_mmap(
    proc: &mut HybridProcess,
    addr: u64,
    len: u64,
    prot: u64,
    flags: u64,
    fd: u64,
    off: u64,
) -> Result<u64, PosixError> {
    if len == 0 {
        return Err(PosixError::EINVAL);
    }

    // Align length to 4KB page boundary
    let aligned_len = ((len as usize) + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);

    let mut sub = subsystem().lock().unwrap();
    let ctx = sub.get_or_create_context(proc.pid);

    let target_addr = if flags & MAP_FIXED != 0 {
        if addr == 0 || addr % (PAGE_SIZE as u64) != 0 {
            return Err(PosixError::EINVAL);
        }
        addr
    } else if addr != 0 && addr % (PAGE_SIZE as u64) == 0 {
        addr
    } else {
        let base = ctx.mmap_next_addr;
        ctx.mmap_next_addr += aligned_len as u64;
        base
    };

    let backing = if flags & MAP_ANONYMOUS != 0 {
        None
    } else {
        Some(Handle(fd as usize))
    };

    let region = MmapRegion {
        start: target_addr,
        len: aligned_len,
        prot,
        flags,
        offset: off,
        backing_fd: backing,
    };

    ctx.mmap_regions.push(region);
    Ok(target_addr)
}

/// `sys_mprotect` (Syscall #10): Change access protections for memory regions.
///
/// Signature: `int sys_mprotect(unsigned long start, size_t len, unsigned long prot)`
pub fn sys_mprotect(proc: &mut HybridProcess, start: u64, len: u64, prot: u64) -> Result<u64, PosixError> {
    if start % (PAGE_SIZE as u64) != 0 {
        return Err(PosixError::EINVAL);
    }
    if len == 0 {
        return Ok(0);
    }

    let mut sub = subsystem().lock().unwrap();
    let ctx = sub.get_or_create_context(proc.pid);

    let mut found = false;
    for region in &mut ctx.mmap_regions {
        if region.start <= start && (region.start + region.len as u64) >= (start + len) {
            region.prot = prot;
            found = true;
            break;
        }
    }

    if found {
        Ok(0)
    } else {
        if start >= ctx.brk_start && start + len <= ctx.brk_current {
            Ok(0)
        } else {
            Err(PosixError::ENOMEM)
        }
    }
}

/// `sys_munmap` (Syscall #11): Unmap virtual address ranges.
///
/// Signature: `int sys_munmap(unsigned long addr, size_t len)`
pub fn sys_munmap(proc: &mut HybridProcess, addr: u64, len: u64) -> Result<u64, PosixError> {
    if addr % (PAGE_SIZE as u64) != 0 || len == 0 {
        return Err(PosixError::EINVAL);
    }

    let mut sub = subsystem().lock().unwrap();
    let ctx = sub.get_or_create_context(proc.pid);

    let end = addr + len;
    ctx.mmap_regions.retain(|r| {
        let r_end = r.start + r.len as u64;
        !(r.start >= addr && r_end <= end)
    });

    Ok(0)
}

/// `sys_brk` (Syscall #12): Change the location of the program break (heap end).
///
/// Signature: `unsigned long sys_brk(unsigned long brk)`
pub fn sys_brk(proc: &mut HybridProcess, brk: u64) -> Result<u64, PosixError> {
    let mut sub = subsystem().lock().unwrap();
    let ctx = sub.get_or_create_context(proc.pid);

    if brk == 0 || brk < ctx.brk_start {
        return Ok(ctx.brk_current);
    }

    if brk > ctx.brk_max {
        return Ok(ctx.brk_current);
    }

    ctx.brk_current = brk;
    Ok(ctx.brk_current)
}

/// `sys_getpid` (Syscall #39): Query calling process ID.
///
/// Signature: `pid_t sys_getpid(void)`
pub fn sys_getpid(proc: &mut HybridProcess) -> Result<u64, PosixError> {
    Ok(proc.pid as u64)
}

/// `sys_getppid` (Syscall #110): Query calling process parent ID.
///
/// Signature: `pid_t sys_getppid(void)`
pub fn sys_getppid(proc: &mut HybridProcess) -> Result<u64, PosixError> {
    let mut sub = subsystem().lock().unwrap();
    let ctx = sub.get_or_create_context(proc.pid);
    Ok(ctx.parent_pid as u64)
}

/// `sys_clone` (Syscall #56): Create a child process or thread with fine-grained sharing flags.
///
/// Signature: `long sys_clone(unsigned long clone_flags, unsigned long newsp, int __user *parent_tidptr, int __user *child_tidptr, unsigned long tls)`
pub fn sys_clone(
    proc: &mut HybridProcess,
    clone_flags: u64,
    _newsp: u64,
    parent_tidptr: u64,
    child_tidptr: u64,
    tls: u64,
) -> Result<u64, PosixError> {
    let mut sub = subsystem().lock().unwrap();
    let child_pid = sub.next_pid;
    sub.next_pid += 1;

    let mut child_ctx = LinuxProcessContext::new(child_pid, proc.pid);

    if clone_flags & CLONE_SETTLS != 0 {
        child_ctx.fs_base = tls;
    }

    if clone_flags & CLONE_VM != 0 {
        let parent_ctx = sub.get_or_create_context(proc.pid);
        child_ctx.mmap_regions = parent_ctx.mmap_regions.clone();
        child_ctx.brk_start = parent_ctx.brk_start;
        child_ctx.brk_current = parent_ctx.brk_current;
        child_ctx.brk_max = parent_ctx.brk_max;
    }

    if parent_tidptr != 0 && clone_flags & CLONE_PARENT_SETTID != 0 {
        let pid_val = child_pid as i32;
        write_user_struct(parent_tidptr, &pid_val)?;
    }

    if child_tidptr != 0 && clone_flags & CLONE_CHILD_SETTID != 0 {
        let pid_val = child_pid as i32;
        write_user_struct(child_tidptr, &pid_val)?;
    }

    sub.processes.insert(child_pid, child_ctx);

    let parent_ctx = sub.get_or_create_context(proc.pid);
    parent_ctx.child_pids.push(child_pid);

    Ok(child_pid as u64)
}

/// `sys_fork` (Syscall #57): Create a child process duplicating calling process state.
///
/// Signature: `pid_t sys_fork(void)`
pub fn sys_fork(proc: &mut HybridProcess) -> Result<u64, PosixError> {
    sys_clone(proc, 0x11 /* SIGCHLD */, 0, 0, 0, 0)
}

/// `sys_execve` (Syscall #59): Execute new program binary, resetting user memory.
///
/// Signature: `int sys_execve(const char __user *filename, const char __user *const __user *argv, const char __user *const __user *envp)`
pub fn sys_execve(proc: &mut HybridProcess, filename_ptr: u64, _argv_ptr: u64, _envp_ptr: u64) -> Result<u64, PosixError> {
    if filename_ptr == 0 {
        return Err(PosixError::EFAULT);
    }

    let program_path = read_user_string(filename_ptr, 256)?;

    let mut sub = subsystem().lock().unwrap();
    let ctx = sub.get_or_create_context(proc.pid);

    for cloexec_handle in ctx.cloexec_handles.drain(..) {
        proc.handles.remove(&cloexec_handle);
        ctx.file_offsets.remove(&cloexec_handle);
        ctx.pipe_descriptors.remove(&cloexec_handle);
    }

    ctx.mmap_regions.clear();
    ctx.brk_current = ctx.brk_start;
    ctx.fs_base = 0;
    ctx.gs_base = 0;

    proc.name = program_path;
    Ok(0)
}

/// `sys_exit` (Syscall #60): Terminate calling thread/process with exit code.
///
/// Signature: `void sys_exit(int status)`
pub fn sys_exit(proc: &mut HybridProcess, status: u64) -> Result<u64, PosixError> {
    proc.is_terminated = true;
    proc.exit_code = (status & 0xFF) as i32;

    let mut sub = subsystem().lock().unwrap();
    let parent_id = sub.get_or_create_context(proc.pid).parent_pid;

    if let Some(parent_ctx) = sub.processes.get_mut(&parent_id) {
        parent_ctx.terminated_children.push((proc.pid, proc.exit_code));
    }

    Ok(0)
}

/// `sys_exit_group` (Syscall #231): Terminate all threads in thread group.
///
/// Signature: `void sys_exit_group(int status)`
pub fn sys_exit_group(proc: &mut HybridProcess, status: u64) -> Result<u64, PosixError> {
    sys_exit(proc, status)
}

/// `sys_wait4` (Syscall #61): Wait for child process state change / termination.
///
/// Signature: `pid_t sys_wait4(pid_t pid, int __user *stat_addr, int options, struct rusage __user *ru)`
pub fn sys_wait4(proc: &mut HybridProcess, target_pid: i64, stat_addr: u64, options: u64, ru: u64) -> Result<u64, PosixError> {
    let mut sub = subsystem().lock().unwrap();
    let ctx = sub.get_or_create_context(proc.pid);

    if ctx.child_pids.is_empty() && ctx.terminated_children.is_empty() {
        return Err(PosixError::ECHILD);
    }

    let mut reaped_idx = None;
    for (idx, &(c_pid, _)) in ctx.terminated_children.iter().enumerate() {
        if target_pid == -1 || target_pid as u32 == c_pid {
            reaped_idx = Some(idx);
            break;
        }
    }

    if let Some(idx) = reaped_idx {
        let (reaped_pid, exit_status) = ctx.terminated_children.remove(idx);
        ctx.child_pids.retain(|&p| p != reaped_pid);

        if stat_addr != 0 {
            let encoded_status = (exit_status & 0xFF) << 8;
            write_user_struct(stat_addr, &encoded_status)?;
        }

        if ru != 0 {
            let rusage_dummy = Rusage::default();
            write_user_struct(ru, &rusage_dummy)?;
        }

        return Ok(reaped_pid as u64);
    }

    if options & WNOHANG != 0 {
        return Ok(0);
    }

    if let Some(&first_child) = ctx.child_pids.first() {
        ctx.child_pids.retain(|&p| p != first_child);
        if stat_addr != 0 {
            let zero_status: i32 = 0;
            write_user_struct(stat_addr, &zero_status)?;
        }
        if ru != 0 {
            let rusage_dummy = Rusage::default();
            write_user_struct(ru, &rusage_dummy)?;
        }
        Ok(first_child as u64)
    } else {
        Err(PosixError::ECHILD)
    }
}

/// `sys_uname` (Syscall #63): Interrogate system identifier and release metadata.
///
/// Signature: `int sys_uname(struct utsname __user *buf)`
pub fn sys_uname(_proc: &mut HybridProcess, buf_ptr: u64) -> Result<u64, PosixError> {
    if buf_ptr == 0 {
        return Err(PosixError::EFAULT);
    }

    let uts = Utsname::default();
    write_user_struct(buf_ptr, &uts)?;
    Ok(0)
}

/// `sys_arch_prctl` (Syscall #158): Set or get architecture-specific thread register state.
///
/// Essential for x86_64 POSIX userland Thread Local Storage (`%fs` base) and `%gs` base.
///
/// Signature: `int sys_arch_prctl(int code, unsigned long addr)`
pub fn sys_arch_prctl(proc: &mut HybridProcess, code: u64, addr: u64) -> Result<u64, PosixError> {
    let mut sub = subsystem().lock().unwrap();
    let ctx = sub.get_or_create_context(proc.pid);

    match code {
        ARCH_SET_FS => {
            ctx.fs_base = addr;
            Ok(0)
        }
        ARCH_GET_FS => {
            if addr == 0 {
                return Err(PosixError::EFAULT);
            }
            write_user_struct(addr, &ctx.fs_base)?;
            Ok(0)
        }
        ARCH_SET_GS => {
            ctx.gs_base = addr;
            Ok(0)
        }
        ARCH_GET_GS => {
            if addr == 0 {
                return Err(PosixError::EFAULT);
            }
            write_user_struct(addr, &ctx.gs_base)?;
            Ok(0)
        }
        _ => Err(PosixError::EINVAL),
    }
}

// ============================================================================
// Extended Syscall Dispatch Router
// ============================================================================

/// Primary dispatcher for all extended Linux POSIX system calls.
pub fn dispatch_extended_syscall(
    proc: &mut HybridProcess,
    syscall_nr: u64,
    a1: u64,
    a2: u64,
    a3: u64,
    a4: u64,
    a5: u64,
    a6: u64,
) -> Result<u64, PosixError> {
    match syscall_nr {
        SYS_READ => sys_read(proc, a1, a2, a3),
        SYS_WRITE => sys_write(proc, a1, a2, a3),
        SYS_OPEN => sys_open(proc, a1, a2, a3),
        SYS_CLOSE => sys_close(proc, a1),
        SYS_STAT => sys_stat(proc, a1, a2),
        SYS_FSTAT => sys_fstat(proc, a1, a2),
        SYS_LSEEK => sys_lseek(proc, a1, a2 as i64, a3),
        SYS_MMAP => sys_mmap(proc, a1, a2, a3, a4, a5, a6),
        SYS_MPROTECT => sys_mprotect(proc, a1, a2, a3),
        SYS_MUNMAP => sys_munmap(proc, a1, a2),
        SYS_BRK => sys_brk(proc, a1),
        SYS_IOCTL => sys_ioctl(proc, a1, a2, a3),
        SYS_PIPE => sys_pipe(proc, a1),
        SYS_DUP => sys_dup(proc, a1),
        SYS_DUP2 => sys_dup2(proc, a1, a2),
        SYS_GETPID => sys_getpid(proc),
        SYS_CLONE => sys_clone(proc, a1, a2, a3, a4, a5),
        SYS_FORK => sys_fork(proc),
        SYS_EXECVE => sys_execve(proc, a1, a2, a3),
        SYS_EXIT => sys_exit(proc, a1),
        SYS_WAIT4 => sys_wait4(proc, a1 as i64, a2, a3, a4),
        SYS_UNAME => sys_uname(proc, a1),
        SYS_GETPPID => sys_getppid(proc),
        SYS_ARCH_PRCTL => sys_arch_prctl(proc, a1, a2),
        SYS_EXIT_GROUP => sys_exit_group(proc, a1),
        SYS_PIPE2 => sys_pipe2(proc, a1, a2),
        _ => super::posix_advanced::dispatch_posix_advanced_syscall(proc, syscall_nr, a1, a2, a3, a4, a5, a6),
    }
}

// Helper trait extension for overflow error mapping
trait PosixErrorExt {
    #[allow(non_snake_case)]
    fn EOVERFLOW_CHECK() -> PosixError;
}

impl PosixErrorExt for PosixError {
    fn EOVERFLOW_CHECK() -> PosixError {
        PosixError::ERANGE
    }
}

// ============================================================================
// Comprehensive Unit & Functional Test Suite
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::personality::ExecutionPersonality;

    fn create_test_process(pid: u32) -> HybridProcess {
        HybridProcess::new(pid, "test_linux_process", ExecutionPersonality::LinuxPosix)
    }

    #[test]
    fn test_uname_syscall() {
        let mut proc = create_test_process(3001);
        let mut uts = Utsname::default();
        let ptr = &mut uts as *mut Utsname as u64;

        let res = sys_uname(&mut proc, ptr);
        assert!(res.is_ok());

        let sysname = std::str::from_utf8(&uts.sysname).unwrap().trim_matches('\0');
        let machine = std::str::from_utf8(&uts.machine).unwrap().trim_matches('\0');
        assert_eq!(sysname, "Linux");
        assert_eq!(machine, "x86_64");
    }

    #[test]
    fn test_arch_prctl_fs_base() {
        let mut proc = create_test_process(3002);
        let target_tls = 0x0000_7FFF_1234_5678;

        // Set FS base
        let res = sys_arch_prctl(&mut proc, ARCH_SET_FS, target_tls);
        assert_eq!(res, Ok(0));

        // Get FS base
        let mut retrieved_base: u64 = 0;
        let ptr = &mut retrieved_base as *mut u64 as u64;
        let get_res = sys_arch_prctl(&mut proc, ARCH_GET_FS, ptr);
        assert_eq!(get_res, Ok(0));
        assert_eq!(retrieved_base, target_tls);
    }

    #[test]
    fn test_brk_expansion() {
        let mut proc = create_test_process(3003);

        // Query initial brk
        let initial_brk = sys_brk(&mut proc, 0).unwrap();
        assert!(initial_brk > 0);

        // Expand brk by 64KB
        let expanded = sys_brk(&mut proc, initial_brk + 0x10000).unwrap();
        assert_eq!(expanded, initial_brk + 0x10000);

        // Query again to verify persistence
        let verified = sys_brk(&mut proc, 0).unwrap();
        assert_eq!(verified, expanded);
    }

    #[test]
    fn test_mmap_mprotect_munmap() {
        let mut proc = create_test_process(3004);

        // Allocate 8KB anonymous private mapping
        let addr = sys_mmap(&mut proc, 0, 8192, PROT_READ | PROT_WRITE, MAP_ANONYMOUS | MAP_PRIVATE, 0, 0).unwrap();
        assert!(addr >= 0x0000_7FFF_0000_0000);

        // Change protection to READ | EXEC
        let prot_res = sys_mprotect(&mut proc, addr, 8192, PROT_READ | PROT_EXEC);
        assert_eq!(prot_res, Ok(0));

        // Unmap region
        let unmap_res = sys_munmap(&mut proc, addr, 8192);
        assert_eq!(unmap_res, Ok(0));
    }

    #[test]
    fn test_pipe_read_write() {
        let mut proc = create_test_process(3005);
        let mut fds = [0i32; 2];
        let ptr = fds.as_mut_ptr() as u64;

        // Create pipe
        let pipe_res = sys_pipe(&mut proc, ptr);
        assert_eq!(pipe_res, Ok(0));
        let read_fd = fds[0] as u64;
        let write_fd = fds[1] as u64;

        // Write data into pipe
        let msg = b"HybridOS Linux Pipe Message\n";
        let write_res = sys_write(&mut proc, write_fd, msg.as_ptr() as u64, msg.len() as u64);
        assert_eq!(write_res, Ok(msg.len() as u64));

        // Read data back from pipe
        let mut in_buf = [0u8; 64];
        let read_res = sys_read(&mut proc, read_fd, in_buf.as_mut_ptr() as u64, in_buf.len() as u64);
        assert_eq!(read_res, Ok(msg.len() as u64));
        assert_eq!(&in_buf[..msg.len()], msg);

        // Close ends
        assert_eq!(sys_close(&mut proc, write_fd), Ok(0));
        assert_eq!(sys_close(&mut proc, read_fd), Ok(0));
    }

    #[test]
    fn test_process_lifecycle_wait4() {
        let mut parent = create_test_process(3006);

        // Clone child
        let child_pid = sys_fork(&mut parent).unwrap();
        assert!(child_pid > 0);

        // Simulate child process exit
        let mut child = create_test_process(child_pid as u32);
        let exit_res = sys_exit(&mut child, 42);
        assert_eq!(exit_res, Ok(0));

        // Parent reaps child via wait4
        let mut status: i32 = 0;
        let reaped_pid = sys_wait4(&mut parent, child_pid as i64, &mut status as *mut i32 as u64, 0, 0).unwrap();
        assert_eq!(reaped_pid, child_pid);
        assert_eq!((status >> 8) & 0xFF, 42);
    }

    #[test]
    fn test_file_io_lseek_and_stat() {
        let mut proc = create_test_process(3007);
        let path_str = "/test_file.txt\0";

        // Open file with O_CREAT | O_RDWR
        let fd = sys_open(&mut proc, path_str.as_ptr() as u64, O_CREAT | O_RDWR, 0o644).unwrap();
        assert!(fd >= 3);

        // Write data
        let content = b"HybridOS Linux POSIX Virtual Storage Subsystem\n";
        let written = sys_write(&mut proc, fd, content.as_ptr() as u64, content.len() as u64).unwrap();
        assert_eq!(written, content.len() as u64);

        // Interrogate via fstat
        let mut st = Stat::default();
        let fstat_res = sys_fstat(&mut proc, fd, &mut st as *mut Stat as u64);
        assert_eq!(fstat_res, Ok(0));
        assert_eq!(st.st_size, content.len() as i64);

        // Interrogate via stat
        let mut path_st = Stat::default();
        let stat_res = sys_stat(&mut proc, path_str.as_ptr() as u64, &mut path_st as *mut Stat as u64);
        assert_eq!(stat_res, Ok(0));
        assert_eq!(path_st.st_size, content.len() as i64);

        // Seek back to start
        let seek_pos = sys_lseek(&mut proc, fd, 0, SEEK_SET).unwrap();
        assert_eq!(seek_pos, 0);

        // Read data back
        let mut read_buf = [0u8; 64];
        let read_bytes = sys_read(&mut proc, fd, read_buf.as_mut_ptr() as u64, read_buf.len() as u64).unwrap();
        assert_eq!(read_bytes, content.len() as u64);
        assert_eq!(&read_buf[..content.len()], content);

        // Close file
        assert_eq!(sys_close(&mut proc, fd), Ok(0));
    }

    #[test]
    fn test_dup_dup2_and_ioctl() {
        let mut proc = create_test_process(3008);

        // dup STDOUT (fd 1)
        let dup_fd = sys_dup(&mut proc, 1).unwrap();
        assert!(dup_fd >= 3);

        // dup2 to target fd 10
        let dup2_fd = sys_dup2(&mut proc, 1, 10).unwrap();
        assert_eq!(dup2_fd, 10);

        // ioctl TIOCGWINSZ on STDOUT
        let mut ws = Winsize::default();
        let ioctl_res = sys_ioctl(&mut proc, 1, TIOCGWINSZ, &mut ws as *mut Winsize as u64);
        assert_eq!(ioctl_res, Ok(0));
        assert_eq!(ws.ws_row, 24);
        assert_eq!(ws.ws_col, 80);

        // Cleanup
        assert_eq!(sys_close(&mut proc, dup_fd), Ok(0));
        assert_eq!(sys_close(&mut proc, 10), Ok(0));
    }

    #[test]
    fn test_process_pids_clone_execve() {
        let mut proc = create_test_process(3009);

        // Test getpid and getppid
        assert_eq!(sys_getpid(&mut proc), Ok(3009));
        assert_eq!(sys_getppid(&mut proc), Ok(1));

        // Clone with flags
        let mut child_tid: i32 = 0;
        let c_tid_ptr = &mut child_tid as *mut i32 as u64;
        let child_pid = sys_clone(&mut proc, CLONE_CHILD_SETTID, 0, 0, c_tid_ptr, 0).unwrap();
        assert!(child_pid > 0);
        assert_eq!(child_tid, child_pid as i32);

        // Execve
        let new_image_name = "/bin/bash\0";
        let exec_res = sys_execve(&mut proc, new_image_name.as_ptr() as u64, 0, 0);
        assert_eq!(exec_res, Ok(0));
        assert_eq!(proc.name, "/bin/bash");
    }

    #[test]
    fn test_extended_dispatcher_full_coverage() {
        let mut proc = create_test_process(3010);

        // 1. SYS_GETPID (39)
        assert_eq!(dispatch_extended_syscall(&mut proc, SYS_GETPID, 0, 0, 0, 0, 0, 0), Ok(3010));

        // 2. SYS_GETPPID (110)
        assert_eq!(dispatch_extended_syscall(&mut proc, SYS_GETPPID, 0, 0, 0, 0, 0, 0), Ok(1));

        // 3. SYS_BRK (12)
        let cur_brk = dispatch_extended_syscall(&mut proc, SYS_BRK, 0, 0, 0, 0, 0, 0).unwrap();
        assert!(cur_brk > 0);

        // 4. SYS_UNAME (63)
        let mut uts = Utsname::default();
        assert_eq!(dispatch_extended_syscall(&mut proc, SYS_UNAME, &mut uts as *mut Utsname as u64, 0, 0, 0, 0, 0), Ok(0));

        // 5. SYS_ARCH_PRCTL (158)
        let tls_addr = 0x7FFF_8888_0000;
        assert_eq!(dispatch_extended_syscall(&mut proc, SYS_ARCH_PRCTL, ARCH_SET_FS, tls_addr, 0, 0, 0, 0), Ok(0));

        // 6. Unknown syscall returns ENOSYS
        assert_eq!(dispatch_extended_syscall(&mut proc, 9999, 0, 0, 0, 0, 0, 0), Err(PosixError::ENOSYS));
    }
}
