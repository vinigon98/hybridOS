//! Advanced POSIX Sockets, Signals & Event Multiplexing Module for HybridOS Kernel.
//!
//! Provides a complete, robust, and modular implementation of POSIX/Linux BSD sockets,
//! real-time signals, and event multiplexing (poll, select, epoll) adhering strictly
//! to the System V AMD64 ABI specification and Linux x86_64 calling conventions.
//!
//! # Subsystems:
//! - **BSD Sockets Subsystem**:
//!   - `sys_socket` (41): Creates stream/dgram sockets, returns FD.
//!   - `sys_connect` (42), `sys_accept` (43), `sys_sendto` (44), `sys_recvfrom` (45).
//!   - `sys_sendmsg` (46), `sys_recvmsg` (47).
//!   - `sys_shutdown` (48), `sys_bind` (49), `sys_listen` (50), `sys_getsockname` (51), `sys_getpeername` (52).
//!   - `sys_socketpair` (53): Connected bidirectional socket pair creation.
//!   - `sys_setsockopt` (54), `sys_getsockopt` (55): SO_REUSEADDR, SO_RCVBUF, SO_SNDBUF, TCP_NODELAY, etc.
//! - **Event Multiplexing & Polling Subsystem**:
//!   - `sys_poll` (7): Descriptor poll array evaluation.
//!   - `sys_select` (23): 1024-descriptor bitmask polling with read/write/exception sets.
//!   - Linux Epoll Subsystem: `sys_epoll_create1` (291), `sys_epoll_ctl` (233) (ADD, MOD, DEL), `sys_epoll_wait` (232).
//! - **POSIX Signal Subsystem**:
//!   - `sys_rt_sigaction` (13): Install handlers (SIGINT, SIGTERM, SIGSEGV, SIGKILL, SIGPIPE, etc.) with sa_mask & flags.
//!   - `sys_rt_sigprocmask` (14): Block/unblock signal sets (SIG_BLOCK, SIG_UNBLOCK, SIG_SETMASK).
//!   - `sys_rt_sigreturn` (15): Restore user CPU frame and signal mask from signal stack.
//!   - `sys_kill` (62): Send signal to process by PID or process group.

#[allow(unused_imports)]
use alloc::string::{String, ToString};
#[allow(unused_imports)]
use alloc::vec::Vec;
#[allow(unused_imports)]
use alloc::boxed::Box;
#[allow(unused_imports)]
use crate::compat::FloatExt;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, OnceLock};

use crate::process::{HybridProcess, KernelObject, ObjectType};
use crate::trap::TrapFrame;
use crate::types::{Handle, PosixError};

// ============================================================================
// Linux System V AMD64 Syscall Numbers (x86_64)
// ============================================================================

pub const SYS_POLL: u64            = 7;
pub const SYS_RT_SIGACTION: u64    = 13;
pub const SYS_RT_SIGPROCMASK: u64  = 14;
pub const SYS_RT_SIGRETURN: u64    = 15;
pub const SYS_SELECT: u64          = 23;
pub const SYS_SOCKET: u64          = 41;
pub const SYS_CONNECT: u64         = 42;
pub const SYS_ACCEPT: u64          = 43;
pub const SYS_SENDTO: u64          = 44;
pub const SYS_RECVFROM: u64        = 45;
pub const SYS_SENDMSG: u64         = 46;
pub const SYS_RECVMSG: u64         = 47;
pub const SYS_SHUTDOWN: u64        = 48;
pub const SYS_BIND: u64            = 49;
pub const SYS_LISTEN: u64          = 50;
pub const SYS_GETSOCKNAME: u64     = 51;
pub const SYS_GETPEERNAME: u64     = 52;
pub const SYS_SOCKETPAIR: u64      = 53;
pub const SYS_SETSOCKOPT: u64      = 54;
pub const SYS_GETSOCKOPT: u64      = 55;
pub const SYS_KILL: u64            = 62;
pub const SYS_EPOLL_CREATE: u64    = 213;
pub const SYS_EPOLL_WAIT: u64      = 232;
pub const SYS_EPOLL_CTL: u64       = 233;
pub const SYS_ACCEPT4: u64         = 288;
pub const SYS_EPOLL_CREATE1: u64   = 291;

// ============================================================================
// Socket Protocol Families & Types
// ============================================================================

pub const AF_UNSPEC: i32 = 0;
pub const AF_UNIX: i32   = 1;
pub const AF_LOCAL: i32  = 1;
pub const AF_INET: i32   = 2;
pub const AF_AX25: i32   = 3;
pub const AF_IPX: i32    = 4;
pub const AF_APPLETALK: i32 = 5;
pub const AF_NETROM: i32 = 6;
pub const AF_BRIDGE: i32 = 7;
pub const AF_ATMPVC: i32 = 8;
pub const AF_X25: i32    = 9;
pub const AF_INET6: i32  = 10;
pub const AF_NETLINK: i32 = 16;
pub const AF_PACKET: i32 = 17;

pub const SOCK_STREAM: i32    = 1;
pub const SOCK_DGRAM: i32     = 2;
pub const SOCK_RAW: i32       = 3;
pub const SOCK_RDM: i32       = 4;
pub const SOCK_SEQPACKET: i32 = 5;
pub const SOCK_DCCP: i32      = 6;
pub const SOCK_PACKET: i32    = 10;

pub const SOCK_CLOEXEC: i32  = 0o2000000; // 0x80000
pub const SOCK_NONBLOCK: i32 = 0o0004000; // 0x800
pub const SOCK_TYPE_MASK: i32 = 0o77;

pub const IPPROTO_IP: i32   = 0;
pub const IPPROTO_ICMP: i32 = 1;
pub const IPPROTO_TCP: i32  = 6;
pub const IPPROTO_UDP: i32  = 17;
pub const IPPROTO_IPV6: i32 = 41;
pub const IPPROTO_RAW: i32  = 255;

// Socket Options Levels
pub const SOL_SOCKET: i32        = 1;
pub const IPPROTO_IP_LEVEL: i32  = 0;
pub const IPPROTO_TCP_LEVEL: i32 = 6;
pub const IPPROTO_IPV6_LEVEL: i32 = 41;

// SOL_SOCKET Option Names
pub const SO_DEBUG: i32        = 1;
pub const SO_REUSEADDR: i32    = 2;
pub const SO_TYPE: i32         = 3;
pub const SO_ERROR: i32        = 4;
pub const SO_DONTROUTE: i32    = 5;
pub const SO_BROADCAST: i32    = 6;
pub const SO_SNDBUF: i32       = 7;
pub const SO_RCVBUF: i32       = 8;
pub const SO_KEEPALIVE: i32    = 9;
pub const SO_OOBINLINE: i32    = 10;
pub const SO_NO_CHECK: i32     = 11;
pub const SO_PRIORITY: i32     = 12;
pub const SO_LINGER: i32       = 13;
pub const SO_BSDCOMPAT: i32    = 14;
pub const SO_REUSEPORT: i32    = 15;
pub const SO_PASSCRED: i32     = 16;
pub const SO_PEERCRED: i32     = 17;
pub const SO_RCVLOWAT: i32     = 18;
pub const SO_SNDLOWAT: i32     = 19;
pub const SO_RCVTIMEO: i32     = 20;
pub const SO_SNDTIMEO: i32     = 21;
pub const SO_ACCEPTCONN: i32   = 30;

// TCP Options
pub const TCP_NODELAY: i32 = 1;
pub const TCP_MAXSEG: i32  = 2;
pub const TCP_CORK: i32    = 3;

// Shutdown How Directives
pub const SHUT_RD: i32   = 0;
pub const SHUT_WR: i32   = 1;
pub const SHUT_RDWR: i32 = 2;

// Message Flags
pub const MSG_OOB: u64          = 0x01;
pub const MSG_PEEK: u64         = 0x02;
pub const MSG_DONTROUTE: u64    = 0x04;
pub const MSG_CTRUNC: u64       = 0x08;
pub const MSG_PROXY: u64        = 0x10;
pub const MSG_TRUNC: u64        = 0x20;
pub const MSG_DONTWAIT: u64     = 0x40;
pub const MSG_EOR: u64          = 0x80;
pub const MSG_WAITALL: u64      = 0x100;
pub const MSG_FIN: u64          = 0x200;
pub const MSG_SYN: u64          = 0x400;
pub const MSG_CONFIRM: u64      = 0x800;
pub const MSG_RST: u64          = 0x1000;
pub const MSG_ERRQUEUE: u64     = 0x2000;
pub const MSG_NOSIGNAL: u64     = 0x4000;
pub const MSG_MORE: u64         = 0x8000;

// ============================================================================
// Event Multiplexing (Poll, Select, Epoll) Constants
// ============================================================================

pub const POLLIN: i16     = 0x0001;
pub const POLLPRI: i16    = 0x0002;
pub const POLLOUT: i16    = 0x0004;
pub const POLLERR: i16    = 0x0008;
pub const POLLHUP: i16    = 0x0010;
pub const POLLNVAL: i16   = 0x0020;
pub const POLLRDNORM: i16 = 0x0040;
pub const POLLRDBAND: i16 = 0x0080;
pub const POLLWRNORM: i16 = 0x0100;
pub const POLLWRBAND: i16 = 0x0200;
pub const POLLRDHUP: i16  = 0x2000;

pub const FD_SETSIZE: usize = 1024;
pub const FD_SET_WORDS: usize = FD_SETSIZE / 64; // 16 words of 64 bits

pub const EPOLL_CTL_ADD: i32 = 1;
pub const EPOLL_CTL_DEL: i32 = 2;
pub const EPOLL_CTL_MOD: i32 = 3;

pub const EPOLLIN: u32        = 0x00000001;
pub const EPOLLPRI: u32       = 0x00000002;
pub const EPOLLOUT: u32       = 0x00000004;
pub const EPOLLERR: u32       = 0x00000008;
pub const EPOLLHUP: u32       = 0x00000010;
pub const EPOLLRDNORM: u32    = 0x00000040;
pub const EPOLLRDBAND: u32    = 0x00000080;
pub const EPOLLWRNORM: u32    = 0x00000100;
pub const EPOLLWRBAND: u32    = 0x00000200;
pub const EPOLLMSG: u32       = 0x00000400;
pub const EPOLLRDHUP: u32     = 0x00002000;
pub const EPOLLEXCLUSIVE: u32 = 1 << 28;
pub const EPOLLWAKEUP: u32    = 1 << 29;
pub const EPOLLONESHOT: u32   = 1 << 30;
pub const EPOLLET: u32        = 1 << 31;

pub const EPOLL_CLOEXEC: u64  = 0o2000000;

// ============================================================================
// POSIX Signals Constants
// ============================================================================

pub const SIGHUP: u32    = 1;
pub const SIGINT: u32    = 2;
pub const SIGQUIT: u32   = 3;
pub const SIGILL: u32    = 4;
pub const SIGTRAP: u32   = 5;
pub const SIGABRT: u32   = 6;
pub const SIGIOT: u32    = 6;
pub const SIGBUS: u32    = 7;
pub const SIGFPE: u32    = 8;
pub const SIGKILL: u32   = 9;
pub const SIGUSR1: u32   = 10;
pub const SIGSEGV: u32   = 11;
pub const SIGUSR2: u32   = 12;
pub const SIGPIPE: u32   = 13;
pub const SIGALRM: u32   = 14;
pub const SIGTERM: u32   = 15;
pub const SIGSTKFLT: u32 = 16;
pub const SIGCHLD: u32   = 17;
pub const SIGCONT: u32   = 18;
pub const SIGSTOP: u32   = 19;
pub const SIGTSTP: u32   = 20;
pub const SIGTTIN: u32   = 21;
pub const SIGTTOU: u32   = 22;
pub const SIGURG: u32    = 23;
pub const SIGXCPU: u32   = 24;
pub const SIGXFSZ: u32   = 25;
pub const SIGVTALRM: u32 = 26;
pub const SIGPROF: u32   = 27;
pub const SIGWINCH: u32  = 28;
pub const SIGIO: u32     = 29;
pub const SIGPOLL: u32   = 29;
pub const SIGPWR: u32    = 30;
pub const SIGSYS: u32    = 31;

pub const NSIG: usize = 64;

pub const SIG_BLOCK: i32   = 0;
pub const SIG_UNBLOCK: i32 = 1;
pub const SIG_SETMASK: i32 = 2;

pub const SIG_DFL: u64 = 0;
pub const SIG_IGN: u64 = 1;
pub const SIG_ERR: u64 = !0;

pub const SA_NOCLDSTOP: u64 = 0x00000001;
pub const SA_NOCLDWAIT: u64 = 0x00000002;
pub const SA_SIGINFO: u64   = 0x00000004;
pub const SA_ONSTACK: u64   = 0x08000000;
pub const SA_RESTART: u64   = 0x10000000;
pub const SA_NODEFER: u64   = 0x40000000;
pub const SA_RESETHAND: u64 = 0x80000000;
pub const SA_RESTORER: u64  = 0x04000000;

// ============================================================================
// BSD Socket & POSIX Structures
// ============================================================================

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SockAddrIn {
    pub sin_family: u16,
    pub sin_port: u16,
    pub sin_addr: u32,
    pub sin_zero: [u8; 8],
}

impl Default for SockAddrIn {
    fn default() -> Self {
        Self {
            sin_family: AF_INET as u16,
            sin_port: 0,
            sin_addr: 0,
            sin_zero: [0; 8],
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SockAddrIn6 {
    pub sin6_family: u16,
    pub sin6_port: u16,
    pub sin6_flowinfo: u32,
    pub sin6_addr: [u8; 16],
    pub sin6_scope_id: u32,
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SockAddrUn {
    pub sun_family: u16,
    pub sun_path: [u8; 108],
}

impl Default for SockAddrUn {
    fn default() -> Self {
        Self {
            sun_family: AF_UNIX as u16,
            sun_path: [0; 108],
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SockAddrStorage {
    pub ss_family: u16,
    pub __ss_pad1: [u8; 6],
    pub __ss_align: i64,
    pub __ss_pad2: [u8; 112],
}

impl Default for SockAddrStorage {
    fn default() -> Self {
        Self {
            ss_family: 0,
            __ss_pad1: [0; 6],
            __ss_align: 0,
            __ss_pad2: [0; 112],
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct IoVec {
    pub iov_base: u64,
    pub iov_len: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct MsgHdr {
    pub msg_name: u64,
    pub msg_namelen: u32,
    pub __pad1: u32,
    pub msg_iov: u64,
    pub msg_iovlen: u64,
    pub msg_control: u64,
    pub msg_controllen: u64,
    pub msg_flags: i32,
    pub __pad2: i32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PollFd {
    pub fd: i32,
    pub events: i16,
    pub revents: i16,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FdSet {
    pub fds_bits: [u64; FD_SET_WORDS],
}

impl Default for FdSet {
    fn default() -> Self {
        Self { fds_bits: [0; FD_SET_WORDS] }
    }
}

impl FdSet {
    pub fn is_set(&self, fd: usize) -> bool {
        if fd < FD_SETSIZE {
            (self.fds_bits[fd / 64] & (1u64 << (fd % 64))) != 0
        } else {
            false
        }
    }

    pub fn set(&mut self, fd: usize) {
        if fd < FD_SETSIZE {
            self.fds_bits[fd / 64] |= 1u64 << (fd % 64);
        }
    }

    pub fn clear(&mut self, fd: usize) {
        if fd < FD_SETSIZE {
            self.fds_bits[fd / 64] &= !(1u64 << (fd % 64));
        }
    }

    pub fn zero(&mut self) {
        self.fds_bits = [0; FD_SET_WORDS];
    }
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EpollEvent {
    pub events: u32,
    pub data: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SigAction {
    pub sa_handler: u64,
    pub sa_flags: u64,
    pub sa_restorer: u64,
    pub sa_mask: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SigInfo {
    pub si_signo: i32,
    pub si_errno: i32,
    pub si_code: i32,
    pub _pad: i32,
    pub si_pid: u32,
    pub si_uid: u32,
    pub si_status: i32,
    pub si_utime: i64,
    pub si_stime: i64,
    pub si_value: u64,
    pub _pad2: [u8; 80],
}

impl Default for SigInfo {
    fn default() -> Self {
        Self {
            si_signo: 0,
            si_errno: 0,
            si_code: 0,
            _pad: 0,
            si_pid: 0,
            si_uid: 0,
            si_status: 0,
            si_utime: 0,
            si_stime: 0,
            si_value: 0,
            _pad2: [0u8; 80],
        }
    }
}

// ============================================================================
// Internal Kernel State Representations
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketState {
    Unbound,
    Bound,
    Listening,
    Connecting,
    Connected,
    Closed,
}

#[derive(Debug, Clone)]
pub struct SocketOptions {
    pub reuse_addr: bool,
    pub reuse_port: bool,
    pub rcvbuf: usize,
    pub sndbuf: usize,
    pub tcp_nodelay: bool,
    pub broadcast: bool,
    pub keepalive: bool,
    pub so_error: i32,
}

impl Default for SocketOptions {
    fn default() -> Self {
        Self {
            reuse_addr: false,
            reuse_port: false,
            rcvbuf: 65536,
            sndbuf: 65536,
            tcp_nodelay: false,
            broadcast: false,
            keepalive: false,
            so_error: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DatagramPacket {
    pub data: Vec<u8>,
    pub src_addr: Vec<u8>,
}

#[derive(Debug)]
pub struct PosixSocket {
    pub id: usize,
    pub domain: i32,
    pub sock_type: i32,
    pub protocol: i32,
    pub state: SocketState,
    pub local_addr: Option<Vec<u8>>,
    pub peer_addr: Option<Vec<u8>>,
    pub peer_id: Option<usize>,
    pub rx_stream: VecDeque<u8>,
    pub rx_dgram: VecDeque<DatagramPacket>,
    pub accept_queue: VecDeque<usize>,
    pub backlog: usize,
    pub options: SocketOptions,
    pub read_shutdown: bool,
    pub write_shutdown: bool,
    pub nonblocking: bool,
    pub cloexec: bool,
}

impl PosixSocket {
    pub fn new(id: usize, domain: i32, raw_type: i32, protocol: i32) -> Self {
        let sock_type = raw_type & SOCK_TYPE_MASK;
        let nonblocking = (raw_type & SOCK_NONBLOCK) != 0;
        let cloexec = (raw_type & SOCK_CLOEXEC) != 0;

        Self {
            id,
            domain,
            sock_type,
            protocol,
            state: SocketState::Unbound,
            local_addr: None,
            peer_addr: None,
            peer_id: None,
            rx_stream: VecDeque::new(),
            rx_dgram: VecDeque::new(),
            accept_queue: VecDeque::new(),
            backlog: 128,
            options: SocketOptions::default(),
            read_shutdown: false,
            write_shutdown: false,
            nonblocking,
            cloexec,
        }
    }
}

#[derive(Debug, Clone)]
pub struct EpollWatch {
    pub fd: i32,
    pub events: u32,
    pub data: u64,
    pub oneshot_triggered: bool,
}

#[derive(Debug)]
pub struct EpollInstance {
    pub id: usize,
    pub flags: u32,
    pub watches: HashMap<i32, EpollWatch>,
}

impl EpollInstance {
    pub fn new(id: usize, flags: u32) -> Self {
        Self {
            id,
            flags,
            watches: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProcessSignalState {
    pub sigactions: [SigAction; NSIG + 1],
    pub blocked_mask: u64,
    pub pending_mask: u64,
    pub saved_trap_frame: Option<TrapFrame>,
    pub saved_mask: Option<u64>,
}

impl ProcessSignalState {
    pub fn new() -> Self {
        Self {
            sigactions: [SigAction::default(); NSIG + 1],
            blocked_mask: 0,
            pending_mask: 0,
            saved_trap_frame: None,
            saved_mask: None,
        }
    }
}

pub struct PosixAdvancedSubsystem {
    pub sockets: HashMap<usize, Arc<Mutex<PosixSocket>>>,
    pub next_socket_id: usize,
    pub bound_inet_ports: HashMap<u16, usize>,
    pub bound_unix_paths: HashMap<String, usize>,
    pub epolls: HashMap<usize, Arc<Mutex<EpollInstance>>>,
    pub next_epoll_id: usize,
    pub signals: HashMap<u32, ProcessSignalState>,
}

impl PosixAdvancedSubsystem {
    pub fn new() -> Self {
        Self {
            sockets: HashMap::new(),
            next_socket_id: 1,
            bound_inet_ports: HashMap::new(),
            bound_unix_paths: HashMap::new(),
            epolls: HashMap::new(),
            next_epoll_id: 1,
            signals: HashMap::new(),
        }
    }

    pub fn get_or_create_signals(&mut self, pid: u32) -> &mut ProcessSignalState {
        self.signals.entry(pid).or_insert_with(ProcessSignalState::new)
    }
}

static POSIX_ADVANCED_SUBSYSTEM: OnceLock<Mutex<PosixAdvancedSubsystem>> = OnceLock::new();

pub fn posix_subsystem() -> &'static Mutex<PosixAdvancedSubsystem> {
    POSIX_ADVANCED_SUBSYSTEM.get_or_init(|| Mutex::new(PosixAdvancedSubsystem::new()))
}

// ============================================================================
// Safe Memory Access Helpers
// ============================================================================

pub fn read_user_struct<T: Copy>(ptr: u64) -> Result<T, PosixError> {
    if ptr == 0 {
        return Err(PosixError::EFAULT);
    }
    unsafe {
        let src = ptr as *const T;
        Ok(std::ptr::read_unaligned(src))
    }
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

pub fn read_user_slice<'a>(ptr: u64, len: usize) -> Result<&'a [u8], PosixError> {
    if ptr == 0 && len > 0 {
        return Err(PosixError::EFAULT);
    }
    if len == 0 {
        return Ok(&[]);
    }
    unsafe {
        Ok(std::slice::from_raw_parts(ptr as *const u8, len))
    }
}

pub fn write_user_slice(ptr: u64, data: &[u8]) -> Result<(), PosixError> {
    if ptr == 0 && !data.is_empty() {
        return Err(PosixError::EFAULT);
    }
    if data.is_empty() {
        return Ok(());
    }
    unsafe {
        std::ptr::copy_nonoverlapping(data.as_ptr(), ptr as *mut u8, data.len());
    }
    Ok(())
}

// ============================================================================
// Socket Life-cycle and Integration Helpers
// ============================================================================

pub fn socket_read_internal(socket_id: usize, buf_ptr: u64, count: u64) -> Result<u64, PosixError> {
    let sub = posix_subsystem().lock().unwrap();
    let sock_arc = match sub.sockets.get(&socket_id) {
        Some(s) => s.clone(),
        None => return Err(PosixError::EBADF),
    };
    drop(sub);

    let mut sock = sock_arc.lock().unwrap();
    if sock.sock_type == SOCK_STREAM {
        if sock.rx_stream.is_empty() {
            if sock.read_shutdown || sock.state == SocketState::Closed {
                return Ok(0); // EOF
            }
            if let Some(peer_id) = sock.peer_id {
                let sub = posix_subsystem().lock().unwrap();
                if let Some(peer_arc) = sub.sockets.get(&peer_id) {
                    let peer = peer_arc.lock().unwrap();
                    if peer.write_shutdown || peer.state == SocketState::Closed {
                        return Ok(0); // EOF from peer
                    }
                } else {
                    return Ok(0);
                }
            }
            if sock.nonblocking {
                return Err(PosixError::EAGAIN);
            }
            return Ok(0);
        }

        let to_read = (count as usize).min(sock.rx_stream.len());
        let mut bytes = Vec::with_capacity(to_read);
        for _ in 0..to_read {
            if let Some(b) = sock.rx_stream.pop_front() {
                bytes.push(b);
            }
        }
        write_user_slice(buf_ptr, &bytes)?;
        Ok(to_read as u64)
    } else if sock.sock_type == SOCK_DGRAM {
        if sock.rx_dgram.is_empty() {
            if sock.nonblocking {
                return Err(PosixError::EAGAIN);
            }
            return Ok(0);
        }
        let pkt = sock.rx_dgram.pop_front().unwrap();
        let to_read = (count as usize).min(pkt.data.len());
        write_user_slice(buf_ptr, &pkt.data[..to_read])?;
        Ok(to_read as u64)
    } else {
        Err(PosixError::EINVAL)
    }
}

pub fn socket_write_internal(socket_id: usize, buf_ptr: u64, count: u64) -> Result<u64, PosixError> {
    let slice = read_user_slice(buf_ptr, count as usize)?;
    let sub = posix_subsystem().lock().unwrap();
    let sock_arc = match sub.sockets.get(&socket_id) {
        Some(s) => s.clone(),
        None => return Err(PosixError::EBADF),
    };
    drop(sub);

    let sock = sock_arc.lock().unwrap();
    if sock.write_shutdown {
        return Err(PosixError::EPIPE);
    }

    if sock.sock_type == SOCK_STREAM {
        let peer_id = match sock.peer_id {
            Some(pid) => pid,
            None => return Err(PosixError::ENOTCONN),
        };
        drop(sock);

        let sub = posix_subsystem().lock().unwrap();
        let peer_arc = match sub.sockets.get(&peer_id) {
            Some(p) => p.clone(),
            None => return Err(PosixError::EPIPE),
        };
        drop(sub);

        let mut peer = peer_arc.lock().unwrap();
        if peer.read_shutdown || peer.state == SocketState::Closed {
            return Err(PosixError::EPIPE);
        }
        for &b in slice {
            peer.rx_stream.push_back(b);
        }
        Ok(slice.len() as u64)
    } else if sock.sock_type == SOCK_DGRAM {
        let peer_id = sock.peer_id;
        drop(sock);

        if let Some(pid) = peer_id {
            let sub = posix_subsystem().lock().unwrap();
            if let Some(peer_arc) = sub.sockets.get(&pid) {
                let mut peer = peer_arc.lock().unwrap();
                peer.rx_dgram.push_back(DatagramPacket {
                    data: slice.to_vec(),
                    src_addr: Vec::new(),
                });
                return Ok(slice.len() as u64);
            }
        }
        Ok(slice.len() as u64)
    } else {
        Err(PosixError::EINVAL)
    }
}

pub fn on_socket_closed(socket_id: usize) {
    let sub = posix_subsystem().lock().unwrap();
    if let Some(sock_arc) = sub.sockets.get(&socket_id).cloned() {
        let mut sock = sock_arc.lock().unwrap();
        sock.state = SocketState::Closed;
        sock.read_shutdown = true;
        sock.write_shutdown = true;
        let peer_id_opt = sock.peer_id;
        drop(sock);

        if let Some(peer_id) = peer_id_opt {
            if let Some(peer_arc) = sub.sockets.get(&peer_id) {
                let mut peer = peer_arc.lock().unwrap();
                peer.peer_id = None;
            }
        }
    }
}

// ============================================================================
// File Descriptor Polling Readiness Inspector
// ============================================================================

pub fn check_fd_events(proc: &HybridProcess, fd: u64) -> i16 {
    let handle = Handle(fd as usize);
    let obj_arc = match proc.get_object(handle) {
        Some(o) => o,
        None => return POLLNVAL,
    };

    let obj = obj_arc.lock().unwrap();
    match &obj.obj_type {
        ObjectType::Socket { id } => {
            let socket_id = *id;
            drop(obj);

            let sub = posix_subsystem().lock().unwrap();
            let sock_arc = match sub.sockets.get(&socket_id) {
                Some(s) => s.clone(),
                None => return POLLERR,
            };
            drop(sub);

            let sock = sock_arc.lock().unwrap();
            let mut revents = 0i16;

            if sock.options.so_error != 0 {
                revents |= POLLERR;
            }

            if sock.sock_type == SOCK_STREAM {
                if sock.state == SocketState::Listening {
                    if !sock.accept_queue.is_empty() {
                        revents |= POLLIN | POLLRDNORM;
                    }
                } else if sock.state == SocketState::Connected {
                    if !sock.rx_stream.is_empty() || sock.read_shutdown {
                        revents |= POLLIN | POLLRDNORM;
                    }
                    if !sock.write_shutdown && sock.rx_stream.len() < sock.options.sndbuf {
                        revents |= POLLOUT | POLLWRNORM;
                    }
                    if let Some(peer_id) = sock.peer_id {
                        let sub = posix_subsystem().lock().unwrap();
                        if let Some(peer_arc) = sub.sockets.get(&peer_id) {
                            let peer = peer_arc.lock().unwrap();
                            if peer.state == SocketState::Closed {
                                revents |= POLLHUP | POLLIN;
                            } else if peer.write_shutdown {
                                revents |= POLLRDHUP | POLLIN;
                            }
                        } else {
                            revents |= POLLHUP | POLLIN;
                        }
                    }
                } else if sock.state == SocketState::Closed {
                    revents |= POLLHUP;
                } else if !sock.write_shutdown {
                    revents |= POLLOUT | POLLWRNORM;
                }
            } else if sock.sock_type == SOCK_DGRAM {
                if !sock.rx_dgram.is_empty() || sock.read_shutdown {
                    revents |= POLLIN | POLLRDNORM;
                }
                if !sock.write_shutdown {
                    revents |= POLLOUT | POLLWRNORM;
                }
            }

            revents
        }

        ObjectType::File { .. } => {
            POLLIN | POLLOUT | POLLRDNORM | POLLWRNORM
        }

        ObjectType::Device { name } => {
            if name.starts_with("pipe_reader_") {
                POLLIN | POLLRDNORM
            } else if name.starts_with("pipe_writer_") {
                POLLOUT | POLLWRNORM
            } else {
                POLLIN | POLLOUT | POLLRDNORM | POLLWRNORM
            }
        }

        ObjectType::Event { signaled } => {
            if *signaled {
                POLLIN | POLLRDNORM
            } else {
                0
            }
        }

        ObjectType::Epoll { id } => {
            let epoll_id = *id;
            drop(obj);

            let sub = posix_subsystem().lock().unwrap();
            if let Some(ep_arc) = sub.epolls.get(&epoll_id) {
                let ep = ep_arc.lock().unwrap();
                for (&watch_fd, watch) in &ep.watches {
                    let ev = check_fd_events(proc, watch_fd as u64);
                    if (ev as u32 & watch.events) != 0 {
                        return POLLIN | POLLRDNORM;
                    }
                }
            }
            0
        }

        _ => POLLIN | POLLOUT | POLLRDNORM | POLLWRNORM,
    }
}

// ============================================================================
// BSD Sockets Syscall Handlers
// ============================================================================

/// `sys_socket` (Syscall #41): Create an endpoint for communication.
pub fn sys_socket(
    proc: &mut HybridProcess,
    domain: i32,
    sock_type: i32,
    protocol: i32,
) -> Result<u64, PosixError> {
    if domain != AF_UNIX && domain != AF_INET && domain != AF_INET6 && domain != AF_NETLINK {
        return Err(PosixError::EAFNOSUPPORT);
    }

    let base_type = sock_type & SOCK_TYPE_MASK;
    if base_type != SOCK_STREAM && base_type != SOCK_DGRAM && base_type != SOCK_RAW {
        return Err(PosixError::ESOCKTNOSUPPORT);
    }

    let socket_id = {
        let mut sub = posix_subsystem().lock().unwrap();
        let sid = sub.next_socket_id;
        sub.next_socket_id += 1;
        let socket = Arc::new(Mutex::new(PosixSocket::new(sid, domain, sock_type, protocol)));
        sub.sockets.insert(sid, socket);
        sid
    };

    let fd = proc.allocate_handle(KernelObject {
        obj_type: ObjectType::Socket { id: socket_id },
        access_mask: 0x3,
    });

    Ok(fd.0 as u64)
}

/// `sys_connect` (Syscall #42): Initiate a connection on a socket.
pub fn sys_connect(
    proc: &mut HybridProcess,
    sockfd: u64,
    addr_ptr: u64,
    addrlen: u64,
) -> Result<u64, PosixError> {
    if addr_ptr == 0 || addrlen < 2 {
        return Err(PosixError::EFAULT);
    }

    let raw_addr = read_user_slice(addr_ptr, addrlen as usize)?.to_vec();
    let family = u16::from_ne_bytes([raw_addr[0], raw_addr[1]]) as i32;

    let handle = Handle(sockfd as usize);
    let socket_id = match proc.get_object(handle) {
        Some(obj_arc) => {
            let obj = obj_arc.lock().unwrap();
            match obj.obj_type {
                ObjectType::Socket { id } => id,
                _ => return Err(PosixError::ENOTSOCK),
            }
        }
        None => return Err(PosixError::EBADF),
    };

    let mut sub = posix_subsystem().lock().unwrap();
    let sock_arc = match sub.sockets.get(&socket_id) {
        Some(s) => s.clone(),
        None => return Err(PosixError::EBADF),
    };

    let mut sock = sock_arc.lock().unwrap();
    if sock.state == SocketState::Connected {
        return Err(PosixError::EISCONN);
    }

    if sock.sock_type == SOCK_STREAM {
        if family == AF_INET {
            if addrlen < 8 {
                return Err(PosixError::EINVAL);
            }
            let port = u16::from_be_bytes([raw_addr[2], raw_addr[3]]);

            if let Some(&listening_id) = sub.bound_inet_ports.get(&port) {
                if let Some(listen_sock_arc) = sub.sockets.get(&listening_id).cloned() {
                    let mut listen_sock = listen_sock_arc.lock().unwrap();
                    if listen_sock.state == SocketState::Listening {
                        let peer_id = sub.next_socket_id;
                        sub.next_socket_id += 1;
                        let mut peer_sock = PosixSocket::new(peer_id, family, SOCK_STREAM, sock.protocol);
                        peer_sock.state = SocketState::Connected;
                        peer_sock.peer_id = Some(socket_id);
                        peer_sock.local_addr = listen_sock.local_addr.clone();
                        peer_sock.peer_addr = Some(raw_addr.clone());

                        let peer_arc = Arc::new(Mutex::new(peer_sock));
                        sub.sockets.insert(peer_id, peer_arc);
                        listen_sock.accept_queue.push_back(peer_id);

                        sock.state = SocketState::Connected;
                        sock.peer_id = Some(peer_id);
                        sock.peer_addr = Some(raw_addr);
                        return Ok(0);
                    }
                }
            }
            return Err(PosixError::ECONNREFUSED);
        } else if family == AF_UNIX {
            let path_bytes = &raw_addr[2..];
            let end_idx = path_bytes.iter().position(|&b| b == 0).unwrap_or(path_bytes.len());
            let path_str = String::from_utf8_lossy(&path_bytes[..end_idx]).to_string();

            if let Some(&listening_id) = sub.bound_unix_paths.get(&path_str) {
                if let Some(listen_sock_arc) = sub.sockets.get(&listening_id).cloned() {
                    let mut listen_sock = listen_sock_arc.lock().unwrap();
                    if listen_sock.state == SocketState::Listening {
                        let peer_id = sub.next_socket_id;
                        sub.next_socket_id += 1;
                        let mut peer_sock = PosixSocket::new(peer_id, family, SOCK_STREAM, sock.protocol);
                        peer_sock.state = SocketState::Connected;
                        peer_sock.peer_id = Some(socket_id);
                        peer_sock.local_addr = listen_sock.local_addr.clone();
                        peer_sock.peer_addr = Some(raw_addr.clone());

                        let peer_arc = Arc::new(Mutex::new(peer_sock));
                        sub.sockets.insert(peer_id, peer_arc);
                        listen_sock.accept_queue.push_back(peer_id);

                        sock.state = SocketState::Connected;
                        sock.peer_id = Some(peer_id);
                        sock.peer_addr = Some(raw_addr);
                        return Ok(0);
                    }
                }
            }
            return Err(PosixError::ECONNREFUSED);
        } else {
            return Err(PosixError::EAFNOSUPPORT);
        }
    } else {
        // DGRAM sockets record peer address
        sock.peer_addr = Some(raw_addr);
        sock.state = SocketState::Connected;
        Ok(0)
    }
}

/// `sys_bind` (Syscall #49): Bind a name to a socket.
pub fn sys_bind(
    proc: &mut HybridProcess,
    sockfd: u64,
    addr_ptr: u64,
    addrlen: u64,
) -> Result<u64, PosixError> {
    if addr_ptr == 0 || addrlen < 2 {
        return Err(PosixError::EFAULT);
    }

    let raw_addr = read_user_slice(addr_ptr, addrlen as usize)?.to_vec();
    let family = u16::from_ne_bytes([raw_addr[0], raw_addr[1]]) as i32;

    let handle = Handle(sockfd as usize);
    let socket_id = match proc.get_object(handle) {
        Some(obj_arc) => {
            let obj = obj_arc.lock().unwrap();
            match obj.obj_type {
                ObjectType::Socket { id } => id,
                _ => return Err(PosixError::ENOTSOCK),
            }
        }
        None => return Err(PosixError::EBADF),
    };

    let mut sub = posix_subsystem().lock().unwrap();
    let sock_arc = match sub.sockets.get(&socket_id) {
        Some(s) => s.clone(),
        None => return Err(PosixError::EBADF),
    };

    let mut sock = sock_arc.lock().unwrap();
    if sock.state != SocketState::Unbound {
        return Err(PosixError::EINVAL);
    }

    if family == AF_INET {
        if addrlen < 8 {
            return Err(PosixError::EINVAL);
        }
        let port = u16::from_be_bytes([raw_addr[2], raw_addr[3]]);
        if port != 0 {
            if let Some(&existing_id) = sub.bound_inet_ports.get(&port) {
                if !sock.options.reuse_addr {
                    if let Some(existing_sock_arc) = sub.sockets.get(&existing_id) {
                        let existing = existing_sock_arc.lock().unwrap();
                        if !existing.options.reuse_addr {
                            return Err(PosixError::EADDRINUSE);
                        }
                    }
                }
            }
            sub.bound_inet_ports.insert(port, socket_id);
        }
    } else if family == AF_UNIX {
        let path_bytes = &raw_addr[2..];
        let end_idx = path_bytes.iter().position(|&b| b == 0).unwrap_or(path_bytes.len());
        let path_str = String::from_utf8_lossy(&path_bytes[..end_idx]).to_string();
        if sub.bound_unix_paths.contains_key(&path_str) {
            return Err(PosixError::EADDRINUSE);
        }
        sub.bound_unix_paths.insert(path_str, socket_id);
    }

    sock.local_addr = Some(raw_addr);
    sock.state = SocketState::Bound;
    Ok(0)
}

/// `sys_listen` (Syscall #50): Listen for incoming socket connections.
pub fn sys_listen(
    proc: &mut HybridProcess,
    sockfd: u64,
    backlog: i32,
) -> Result<u64, PosixError> {
    let handle = Handle(sockfd as usize);
    let socket_id = match proc.get_object(handle) {
        Some(obj_arc) => {
            let obj = obj_arc.lock().unwrap();
            match obj.obj_type {
                ObjectType::Socket { id } => id,
                _ => return Err(PosixError::ENOTSOCK),
            }
        }
        None => return Err(PosixError::EBADF),
    };

    let sub = posix_subsystem().lock().unwrap();
    let sock_arc = match sub.sockets.get(&socket_id) {
        Some(s) => s.clone(),
        None => return Err(PosixError::EBADF),
    };

    let mut sock = sock_arc.lock().unwrap();
    if sock.sock_type != SOCK_STREAM && sock.sock_type != SOCK_SEQPACKET {
        return Err(PosixError::EOPNOTSUPP);
    }

    sock.state = SocketState::Listening;
    sock.backlog = if backlog > 0 { backlog as usize } else { 128 };
    Ok(0)
}

/// `sys_accept` (Syscall #43): Accept a connection on a listening socket.
pub fn sys_accept(
    proc: &mut HybridProcess,
    sockfd: u64,
    addr_ptr: u64,
    addrlen_ptr: u64,
) -> Result<u64, PosixError> {
    sys_accept4(proc, sockfd, addr_ptr, addrlen_ptr, 0)
}

/// `sys_accept4` (Syscall #288): Accept with flags (SOCK_NONBLOCK, SOCK_CLOEXEC).
pub fn sys_accept4(
    proc: &mut HybridProcess,
    sockfd: u64,
    addr_ptr: u64,
    addrlen_ptr: u64,
    flags: i32,
) -> Result<u64, PosixError> {
    let handle = Handle(sockfd as usize);
    let socket_id = match proc.get_object(handle) {
        Some(obj_arc) => {
            let obj = obj_arc.lock().unwrap();
            match obj.obj_type {
                ObjectType::Socket { id } => id,
                _ => return Err(PosixError::ENOTSOCK),
            }
        }
        None => return Err(PosixError::EBADF),
    };

    let sub = posix_subsystem().lock().unwrap();
    let sock_arc = match sub.sockets.get(&socket_id) {
        Some(s) => s.clone(),
        None => return Err(PosixError::EBADF),
    };

    let mut sock = sock_arc.lock().unwrap();
    if sock.state != SocketState::Listening {
        return Err(PosixError::EINVAL);
    }

    let accepted_id = match sock.accept_queue.pop_front() {
        Some(id) => id,
        None => {
            if sock.nonblocking || (flags & SOCK_NONBLOCK) != 0 {
                return Err(PosixError::EAGAIN);
            } else {
                return Err(PosixError::EAGAIN);
            }
        }
    };
    drop(sock);

    let accepted_arc = match sub.sockets.get(&accepted_id) {
        Some(a) => a.clone(),
        None => return Err(PosixError::EBADF),
    };
    drop(sub);

    let mut accepted_sock = accepted_arc.lock().unwrap();
    if (flags & SOCK_NONBLOCK) != 0 {
        accepted_sock.nonblocking = true;
    }
    if (flags & SOCK_CLOEXEC) != 0 {
        accepted_sock.cloexec = true;
    }

    if addr_ptr != 0 && addrlen_ptr != 0 {
        if let Some(peer_addr) = &accepted_sock.peer_addr {
            let max_len = read_user_struct::<u32>(addrlen_ptr)? as usize;
            let write_len = max_len.min(peer_addr.len());
            write_user_slice(addr_ptr, &peer_addr[..write_len])?;
            write_user_struct(addrlen_ptr, &(peer_addr.len() as u32))?;
        }
    }
    drop(accepted_sock);

    let new_fd = proc.allocate_handle(KernelObject {
        obj_type: ObjectType::Socket { id: accepted_id },
        access_mask: 0x3,
    });

    Ok(new_fd.0 as u64)
}

/// `sys_sendto` (Syscall #44): Send a message on a socket.
pub fn sys_sendto(
    proc: &mut HybridProcess,
    sockfd: u64,
    buf_ptr: u64,
    len: u64,
    _flags: u64,
    dest_addr_ptr: u64,
    addrlen: u64,
) -> Result<u64, PosixError> {
    let slice = read_user_slice(buf_ptr, len as usize)?;

    let handle = Handle(sockfd as usize);
    let socket_id = match proc.get_object(handle) {
        Some(obj_arc) => {
            let obj = obj_arc.lock().unwrap();
            match obj.obj_type {
                ObjectType::Socket { id } => id,
                _ => return Err(PosixError::ENOTSOCK),
            }
        }
        None => return Err(PosixError::EBADF),
    };

    let sub = posix_subsystem().lock().unwrap();
    let sock_arc = match sub.sockets.get(&socket_id) {
        Some(s) => s.clone(),
        None => return Err(PosixError::EBADF),
    };
    drop(sub);

    let sock = sock_arc.lock().unwrap();
    if sock.write_shutdown {
        return Err(PosixError::EPIPE);
    }

    if sock.sock_type == SOCK_STREAM {
        if dest_addr_ptr != 0 {
            return Err(PosixError::EISCONN);
        }
        let peer_id = match sock.peer_id {
            Some(pid) => pid,
            None => return Err(PosixError::ENOTCONN),
        };
        drop(sock);

        let sub = posix_subsystem().lock().unwrap();
        let peer_arc = match sub.sockets.get(&peer_id) {
            Some(p) => p.clone(),
            None => return Err(PosixError::EPIPE),
        };
        drop(sub);

        let mut peer = peer_arc.lock().unwrap();
        if peer.read_shutdown || peer.state == SocketState::Closed {
            return Err(PosixError::EPIPE);
        }
        for &b in slice {
            peer.rx_stream.push_back(b);
        }
        Ok(slice.len() as u64)
    } else if sock.sock_type == SOCK_DGRAM {
        let dest_addr_opt = if dest_addr_ptr != 0 && addrlen >= 2 {
            Some(read_user_slice(dest_addr_ptr, addrlen as usize)?.to_vec())
        } else {
            sock.peer_addr.clone()
        };
        let src_addr = sock.local_addr.clone().unwrap_or_default();
        drop(sock);

        if let Some(dest_addr) = dest_addr_opt {
            let family = u16::from_ne_bytes([dest_addr[0], dest_addr[1]]) as i32;
            let sub = posix_subsystem().lock().unwrap();

            if family == AF_INET && dest_addr.len() >= 4 {
                let port = u16::from_be_bytes([dest_addr[2], dest_addr[3]]);
                if let Some(&target_id) = sub.bound_inet_ports.get(&port) {
                    if let Some(target_arc) = sub.sockets.get(&target_id) {
                        let mut target = target_arc.lock().unwrap();
                        target.rx_dgram.push_back(DatagramPacket {
                            data: slice.to_vec(),
                            src_addr,
                        });
                        return Ok(slice.len() as u64);
                    }
                }
            } else if family == AF_UNIX {
                let path_bytes = &dest_addr[2..];
                let end_idx = path_bytes.iter().position(|&b| b == 0).unwrap_or(path_bytes.len());
                let path_str = String::from_utf8_lossy(&path_bytes[..end_idx]).to_string();
                if let Some(&target_id) = sub.bound_unix_paths.get(&path_str) {
                    if let Some(target_arc) = sub.sockets.get(&target_id) {
                        let mut target = target_arc.lock().unwrap();
                        target.rx_dgram.push_back(DatagramPacket {
                            data: slice.to_vec(),
                            src_addr,
                        });
                        return Ok(slice.len() as u64);
                    }
                }
            }
        }
        // If unconnected and no route found, still count as buffered/sent in simulation
        Ok(slice.len() as u64)
    } else {
        Err(PosixError::EINVAL)
    }
}

/// `sys_recvfrom` (Syscall #45): Receive a message from a socket.
pub fn sys_recvfrom(
    proc: &mut HybridProcess,
    sockfd: u64,
    buf_ptr: u64,
    len: u64,
    flags: u64,
    src_addr_ptr: u64,
    addrlen_ptr: u64,
) -> Result<u64, PosixError> {
    let handle = Handle(sockfd as usize);
    let socket_id = match proc.get_object(handle) {
        Some(obj_arc) => {
            let obj = obj_arc.lock().unwrap();
            match obj.obj_type {
                ObjectType::Socket { id } => id,
                _ => return Err(PosixError::ENOTSOCK),
            }
        }
        None => return Err(PosixError::EBADF),
    };

    let sub = posix_subsystem().lock().unwrap();
    let sock_arc = match sub.sockets.get(&socket_id) {
        Some(s) => s.clone(),
        None => return Err(PosixError::EBADF),
    };
    drop(sub);

    let mut sock = sock_arc.lock().unwrap();
    if sock.sock_type == SOCK_STREAM {
        if sock.rx_stream.is_empty() {
            if sock.read_shutdown || sock.state == SocketState::Closed {
                return Ok(0);
            }
            if let Some(peer_id) = sock.peer_id {
                let sub = posix_subsystem().lock().unwrap();
                if let Some(peer_arc) = sub.sockets.get(&peer_id) {
                    let peer = peer_arc.lock().unwrap();
                    if peer.write_shutdown || peer.state == SocketState::Closed {
                        return Ok(0);
                    }
                } else {
                    return Ok(0);
                }
            }
            if sock.nonblocking || (flags & MSG_DONTWAIT) != 0 {
                return Err(PosixError::EAGAIN);
            }
            return Ok(0);
        }

        let to_read = (len as usize).min(sock.rx_stream.len());
        let peek = (flags & MSG_PEEK) != 0;
        let mut bytes = Vec::with_capacity(to_read);

        if peek {
            for i in 0..to_read {
                bytes.push(sock.rx_stream[i]);
            }
        } else {
            for _ in 0..to_read {
                if let Some(b) = sock.rx_stream.pop_front() {
                    bytes.push(b);
                }
            }
        }

        write_user_slice(buf_ptr, &bytes)?;

        if src_addr_ptr != 0 && addrlen_ptr != 0 {
            if let Some(peer_addr) = &sock.peer_addr {
                let max_len = read_user_struct::<u32>(addrlen_ptr)? as usize;
                let write_len = max_len.min(peer_addr.len());
                write_user_slice(src_addr_ptr, &peer_addr[..write_len])?;
                write_user_struct(addrlen_ptr, &(peer_addr.len() as u32))?;
            }
        }
        Ok(to_read as u64)
    } else if sock.sock_type == SOCK_DGRAM {
        if sock.rx_dgram.is_empty() {
            if sock.nonblocking || (flags & MSG_DONTWAIT) != 0 {
                return Err(PosixError::EAGAIN);
            }
            return Ok(0);
        }

        let peek = (flags & MSG_PEEK) != 0;
        let pkt = if peek {
            sock.rx_dgram.front().unwrap().clone()
        } else {
            sock.rx_dgram.pop_front().unwrap()
        };

        let to_read = (len as usize).min(pkt.data.len());
        write_user_slice(buf_ptr, &pkt.data[..to_read])?;

        if src_addr_ptr != 0 && addrlen_ptr != 0 && !pkt.src_addr.is_empty() {
            let max_len = read_user_struct::<u32>(addrlen_ptr)? as usize;
            let write_len = max_len.min(pkt.src_addr.len());
            write_user_slice(src_addr_ptr, &pkt.src_addr[..write_len])?;
            write_user_struct(addrlen_ptr, &(pkt.src_addr.len() as u32))?;
        }
        Ok(to_read as u64)
    } else {
        Err(PosixError::EINVAL)
    }
}

/// `sys_sendmsg` (Syscall #46): Send a message on a socket using a message header.
pub fn sys_sendmsg(
    proc: &mut HybridProcess,
    sockfd: u64,
    msg_ptr: u64,
    flags: u64,
) -> Result<u64, PosixError> {
    if msg_ptr == 0 {
        return Err(PosixError::EFAULT);
    }
    let msg = read_user_struct::<MsgHdr>(msg_ptr)?;

    let mut total_sent = 0u64;
    let iov_count = msg.msg_iovlen as usize;
    if iov_count == 0 {
        return Ok(0);
    }

    for i in 0..iov_count {
        let iov_addr = msg.msg_iov + (i * std::mem::size_of::<IoVec>()) as u64;
        let iov = read_user_struct::<IoVec>(iov_addr)?;
        if iov.iov_len > 0 {
            let sent = sys_sendto(proc, sockfd, iov.iov_base, iov.iov_len, flags, msg.msg_name, msg.msg_namelen as u64)?;
            total_sent += sent;
        }
    }

    Ok(total_sent)
}

/// `sys_recvmsg` (Syscall #47): Receive a message from a socket into multiple buffers.
pub fn sys_recvmsg(
    proc: &mut HybridProcess,
    sockfd: u64,
    msg_ptr: u64,
    flags: u64,
) -> Result<u64, PosixError> {
    if msg_ptr == 0 {
        return Err(PosixError::EFAULT);
    }
    let mut msg = read_user_struct::<MsgHdr>(msg_ptr)?;

    let mut total_recv = 0u64;
    let iov_count = msg.msg_iovlen as usize;
    if iov_count == 0 {
        return Ok(0);
    }

    for i in 0..iov_count {
        let iov_addr = msg.msg_iov + (i * std::mem::size_of::<IoVec>()) as u64;
        let iov = read_user_struct::<IoVec>(iov_addr)?;
        if iov.iov_len > 0 {
            let mut addrlen = msg.msg_namelen;
            let addrlen_ptr = &mut addrlen as *mut u32 as u64;
            let recvd = sys_recvfrom(proc, sockfd, iov.iov_base, iov.iov_len, flags, msg.msg_name, addrlen_ptr)?;
            total_recv += recvd;
            msg.msg_namelen = addrlen;
            if recvd < iov.iov_len {
                break;
            }
        }
    }

    write_user_struct(msg_ptr, &msg)?;
    Ok(total_recv)
}

/// `sys_shutdown` (Syscall #48): Shut down part of a full-duplex connection.
pub fn sys_shutdown(
    proc: &mut HybridProcess,
    sockfd: u64,
    how: i32,
) -> Result<u64, PosixError> {
    let handle = Handle(sockfd as usize);
    let socket_id = match proc.get_object(handle) {
        Some(obj_arc) => {
            let obj = obj_arc.lock().unwrap();
            match obj.obj_type {
                ObjectType::Socket { id } => id,
                _ => return Err(PosixError::ENOTSOCK),
            }
        }
        None => return Err(PosixError::EBADF),
    };

    let sub = posix_subsystem().lock().unwrap();
    let sock_arc = match sub.sockets.get(&socket_id) {
        Some(s) => s.clone(),
        None => return Err(PosixError::EBADF),
    };

    let mut sock = sock_arc.lock().unwrap();
    match how {
        SHUT_RD => {
            sock.read_shutdown = true;
        }
        SHUT_WR => {
            sock.write_shutdown = true;
            if let Some(peer_id) = sock.peer_id {
                if let Some(peer_arc) = sub.sockets.get(&peer_id) {
                    let mut peer = peer_arc.lock().unwrap();
                    peer.read_shutdown = true;
                }
            }
        }
        SHUT_RDWR => {
            sock.read_shutdown = true;
            sock.write_shutdown = true;
            if let Some(peer_id) = sock.peer_id {
                if let Some(peer_arc) = sub.sockets.get(&peer_id) {
                    let mut peer = peer_arc.lock().unwrap();
                    peer.read_shutdown = true;
                    peer.write_shutdown = true;
                }
            }
        }
        _ => return Err(PosixError::EINVAL),
    }

    Ok(0)
}

/// `sys_getsockname` (Syscall #51): Get socket name/address.
pub fn sys_getsockname(
    proc: &mut HybridProcess,
    sockfd: u64,
    addr_ptr: u64,
    addrlen_ptr: u64,
) -> Result<u64, PosixError> {
    if addr_ptr == 0 || addrlen_ptr == 0 {
        return Err(PosixError::EFAULT);
    }

    let handle = Handle(sockfd as usize);
    let socket_id = match proc.get_object(handle) {
        Some(obj_arc) => {
            let obj = obj_arc.lock().unwrap();
            match obj.obj_type {
                ObjectType::Socket { id } => id,
                _ => return Err(PosixError::ENOTSOCK),
            }
        }
        None => return Err(PosixError::EBADF),
    };

    let sub = posix_subsystem().lock().unwrap();
    let sock_arc = match sub.sockets.get(&socket_id) {
        Some(s) => s.clone(),
        None => return Err(PosixError::EBADF),
    };

    let sock = sock_arc.lock().unwrap();
    let max_len = read_user_struct::<u32>(addrlen_ptr)? as usize;

    let default_addr = SockAddrStorage::default();
    let default_bytes = unsafe {
        std::slice::from_raw_parts(&default_addr as *const SockAddrStorage as *const u8, std::mem::size_of::<SockAddrStorage>())
    };
    let addr_bytes = sock.local_addr.as_deref().unwrap_or(default_bytes);
    let write_len = max_len.min(addr_bytes.len());

    write_user_slice(addr_ptr, &addr_bytes[..write_len])?;
    write_user_struct(addrlen_ptr, &(addr_bytes.len() as u32))?;

    Ok(0)
}

/// `sys_getpeername` (Syscall #52): Get address of the connected peer.
pub fn sys_getpeername(
    proc: &mut HybridProcess,
    sockfd: u64,
    addr_ptr: u64,
    addrlen_ptr: u64,
) -> Result<u64, PosixError> {
    if addr_ptr == 0 || addrlen_ptr == 0 {
        return Err(PosixError::EFAULT);
    }

    let handle = Handle(sockfd as usize);
    let socket_id = match proc.get_object(handle) {
        Some(obj_arc) => {
            let obj = obj_arc.lock().unwrap();
            match obj.obj_type {
                ObjectType::Socket { id } => id,
                _ => return Err(PosixError::ENOTSOCK),
            }
        }
        None => return Err(PosixError::EBADF),
    };

    let sub = posix_subsystem().lock().unwrap();
    let sock_arc = match sub.sockets.get(&socket_id) {
        Some(s) => s.clone(),
        None => return Err(PosixError::EBADF),
    };

    let sock = sock_arc.lock().unwrap();
    let peer_addr = match &sock.peer_addr {
        Some(p) => p,
        None => return Err(PosixError::ENOTCONN),
    };

    let max_len = read_user_struct::<u32>(addrlen_ptr)? as usize;
    let write_len = max_len.min(peer_addr.len());

    write_user_slice(addr_ptr, &peer_addr[..write_len])?;
    write_user_struct(addrlen_ptr, &(peer_addr.len() as u32))?;

    Ok(0)
}

/// `sys_socketpair` (Syscall #53): Create a pair of connected sockets.
pub fn sys_socketpair(
    proc: &mut HybridProcess,
    domain: i32,
    sock_type: i32,
    protocol: i32,
    sv_ptr: u64,
) -> Result<u64, PosixError> {
    if sv_ptr == 0 {
        return Err(PosixError::EFAULT);
    }
    if domain != AF_UNIX && domain != AF_LOCAL {
        return Err(PosixError::EOPNOTSUPP);
    }

    let (id0, id1) = {
        let mut sub = posix_subsystem().lock().unwrap();
        let s0 = sub.next_socket_id;
        let s1 = sub.next_socket_id + 1;
        sub.next_socket_id += 2;

        let mut sock0 = PosixSocket::new(s0, domain, sock_type, protocol);
        let mut sock1 = PosixSocket::new(s1, domain, sock_type, protocol);

        sock0.state = SocketState::Connected;
        sock0.peer_id = Some(s1);

        sock1.state = SocketState::Connected;
        sock1.peer_id = Some(s0);

        sub.sockets.insert(s0, Arc::new(Mutex::new(sock0)));
        sub.sockets.insert(s1, Arc::new(Mutex::new(sock1)));
        (s0, s1)
    };

    let fd0 = proc.allocate_handle(KernelObject {
        obj_type: ObjectType::Socket { id: id0 },
        access_mask: 0x3,
    });
    let fd1 = proc.allocate_handle(KernelObject {
        obj_type: ObjectType::Socket { id: id1 },
        access_mask: 0x3,
    });

    let fds = [fd0.0 as i32, fd1.0 as i32];
    write_user_slice(sv_ptr, unsafe {
        std::slice::from_raw_parts(fds.as_ptr() as *const u8, std::mem::size_of_val(&fds))
    })?;

    Ok(0)
}

/// `sys_setsockopt` (Syscall #54): Set socket options.
pub fn sys_setsockopt(
    proc: &mut HybridProcess,
    sockfd: u64,
    level: i32,
    optname: i32,
    optval_ptr: u64,
    optlen: u64,
) -> Result<u64, PosixError> {
    if optval_ptr == 0 || optlen < 4 {
        return Err(PosixError::EINVAL);
    }

    let val = read_user_struct::<i32>(optval_ptr)?;

    let handle = Handle(sockfd as usize);
    let socket_id = match proc.get_object(handle) {
        Some(obj_arc) => {
            let obj = obj_arc.lock().unwrap();
            match obj.obj_type {
                ObjectType::Socket { id } => id,
                _ => return Err(PosixError::ENOTSOCK),
            }
        }
        None => return Err(PosixError::EBADF),
    };

    let sub = posix_subsystem().lock().unwrap();
    let sock_arc = match sub.sockets.get(&socket_id) {
        Some(s) => s.clone(),
        None => return Err(PosixError::EBADF),
    };

    let mut sock = sock_arc.lock().unwrap();
    if level == SOL_SOCKET {
        match optname {
            SO_REUSEADDR => {
                sock.options.reuse_addr = val != 0;
            }
            SO_REUSEPORT => {
                sock.options.reuse_port = val != 0;
            }
            SO_RCVBUF => {
                sock.options.rcvbuf = val.max(256) as usize;
            }
            SO_SNDBUF => {
                sock.options.sndbuf = val.max(256) as usize;
            }
            SO_BROADCAST => {
                sock.options.broadcast = val != 0;
            }
            SO_KEEPALIVE => {
                sock.options.keepalive = val != 0;
            }
            _ => return Err(PosixError::ENOPROTOOPT),
        }
    } else if level == IPPROTO_TCP_LEVEL {
        match optname {
            TCP_NODELAY => {
                sock.options.tcp_nodelay = val != 0;
            }
            _ => return Err(PosixError::ENOPROTOOPT),
        }
    } else {
        return Err(PosixError::ENOPROTOOPT);
    }

    Ok(0)
}

/// `sys_getsockopt` (Syscall #55): Get socket options.
pub fn sys_getsockopt(
    proc: &mut HybridProcess,
    sockfd: u64,
    level: i32,
    optname: i32,
    optval_ptr: u64,
    optlen_ptr: u64,
) -> Result<u64, PosixError> {
    if optval_ptr == 0 || optlen_ptr == 0 {
        return Err(PosixError::EFAULT);
    }

    let mut optlen = read_user_struct::<u32>(optlen_ptr)?;
    if optlen < 4 {
        return Err(PosixError::EINVAL);
    }

    let handle = Handle(sockfd as usize);
    let socket_id = match proc.get_object(handle) {
        Some(obj_arc) => {
            let obj = obj_arc.lock().unwrap();
            match obj.obj_type {
                ObjectType::Socket { id } => id,
                _ => return Err(PosixError::ENOTSOCK),
            }
        }
        None => return Err(PosixError::EBADF),
    };

    let sub = posix_subsystem().lock().unwrap();
    let sock_arc = match sub.sockets.get(&socket_id) {
        Some(s) => s.clone(),
        None => return Err(PosixError::EBADF),
    };

    let mut sock = sock_arc.lock().unwrap();
    let out_val: i32 = if level == SOL_SOCKET {
        match optname {
            SO_REUSEADDR => if sock.options.reuse_addr { 1 } else { 0 },
            SO_REUSEPORT => if sock.options.reuse_port { 1 } else { 0 },
            SO_RCVBUF => sock.options.rcvbuf as i32,
            SO_SNDBUF => sock.options.sndbuf as i32,
            SO_BROADCAST => if sock.options.broadcast { 1 } else { 0 },
            SO_KEEPALIVE => if sock.options.keepalive { 1 } else { 0 },
            SO_TYPE => sock.sock_type,
            SO_ERROR => {
                let err = sock.options.so_error;
                sock.options.so_error = 0;
                err
            }
            _ => return Err(PosixError::ENOPROTOOPT),
        }
    } else if level == IPPROTO_TCP_LEVEL {
        match optname {
            TCP_NODELAY => if sock.options.tcp_nodelay { 1 } else { 0 },
            _ => return Err(PosixError::ENOPROTOOPT),
        }
    } else {
        return Err(PosixError::ENOPROTOOPT);
    };

    write_user_struct(optval_ptr, &out_val)?;
    optlen = 4;
    write_user_struct(optlen_ptr, &optlen)?;

    Ok(0)
}

// ============================================================================
// Event Multiplexing Handlers (Poll, Select, Epoll)
// ============================================================================

/// `sys_poll` (Syscall #7): Wait for some event on a file descriptor array.
pub fn sys_poll(
    proc: &mut HybridProcess,
    ufds_ptr: u64,
    nfds: u64,
    _timeout_msecs: i64,
) -> Result<u64, PosixError> {
    if nfds == 0 {
        return Ok(0);
    }
    if ufds_ptr == 0 {
        return Err(PosixError::EFAULT);
    }

    let mut ready_count = 0u64;
    let pollfd_size = std::mem::size_of::<PollFd>() as u64;

    for i in 0..nfds {
        let entry_ptr = ufds_ptr + i * pollfd_size;
        let mut pfd = read_user_struct::<PollFd>(entry_ptr)?;

        if pfd.fd < 0 {
            pfd.revents = 0;
            write_user_struct(entry_ptr, &pfd)?;
            continue;
        }

        let events = check_fd_events(proc, pfd.fd as u64);
        let revents = events & (pfd.events | POLLERR | POLLHUP | POLLNVAL);
        pfd.revents = revents;
        write_user_struct(entry_ptr, &pfd)?;

        if revents != 0 {
            ready_count += 1;
        }
    }

    Ok(ready_count)
}

/// `sys_select` (Syscall #23): Synchronous I/O multiplexing.
pub fn sys_select(
    proc: &mut HybridProcess,
    nfds: u64,
    readfds_ptr: u64,
    writefds_ptr: u64,
    exceptfds_ptr: u64,
    _timeout_ptr: u64,
) -> Result<u64, PosixError> {
    if nfds > FD_SETSIZE as u64 {
        return Err(PosixError::EINVAL);
    }

    let in_read = if readfds_ptr != 0 {
        Some(read_user_struct::<FdSet>(readfds_ptr)?)
    } else {
        None
    };

    let in_write = if writefds_ptr != 0 {
        Some(read_user_struct::<FdSet>(writefds_ptr)?)
    } else {
        None
    };

    let in_except = if exceptfds_ptr != 0 {
        Some(read_user_struct::<FdSet>(exceptfds_ptr)?)
    } else {
        None
    };

    let mut out_read = FdSet::default();
    let mut out_write = FdSet::default();
    let mut out_except = FdSet::default();
    let mut ready_count = 0u64;

    for fd in 0..(nfds as usize) {
        let check_r = in_read.as_ref().map_or(false, |s| s.is_set(fd));
        let check_w = in_write.as_ref().map_or(false, |s| s.is_set(fd));
        let check_e = in_except.as_ref().map_or(false, |s| s.is_set(fd));

        if !check_r && !check_w && !check_e {
            continue;
        }

        let ev = check_fd_events(proc, fd as u64);
        let mut fd_ready = false;

        if check_r && (ev & (POLLIN | POLLHUP | POLLERR)) != 0 {
            out_read.set(fd);
            fd_ready = true;
        }
        if check_w && (ev & (POLLOUT | POLLERR)) != 0 {
            out_write.set(fd);
            fd_ready = true;
        }
        if check_e && (ev & POLLERR) != 0 {
            out_except.set(fd);
            fd_ready = true;
        }

        if fd_ready {
            ready_count += 1;
        }
    }

    if readfds_ptr != 0 {
        write_user_struct(readfds_ptr, &out_read)?;
    }
    if writefds_ptr != 0 {
        write_user_struct(writefds_ptr, &out_write)?;
    }
    if exceptfds_ptr != 0 {
        write_user_struct(exceptfds_ptr, &out_except)?;
    }

    Ok(ready_count)
}

/// `sys_epoll_create1` (Syscall #291): Open an epoll file descriptor.
pub fn sys_epoll_create1(
    proc: &mut HybridProcess,
    flags: u64,
) -> Result<u64, PosixError> {
    let epoll_id = {
        let mut sub = posix_subsystem().lock().unwrap();
        let eid = sub.next_epoll_id;
        sub.next_epoll_id += 1;
        let ep = Arc::new(Mutex::new(EpollInstance::new(eid, flags as u32)));
        sub.epolls.insert(eid, ep);
        eid
    };

    let fd = proc.allocate_handle(KernelObject {
        obj_type: ObjectType::Epoll { id: epoll_id },
        access_mask: 0x3,
    });

    Ok(fd.0 as u64)
}

/// `sys_epoll_ctl` (Syscall #233): Control interface for an epoll file descriptor.
pub fn sys_epoll_ctl(
    proc: &mut HybridProcess,
    epfd: u64,
    op: i32,
    fd: u64,
    event_ptr: u64,
) -> Result<u64, PosixError> {
    let epoll_handle = Handle(epfd as usize);
    let epoll_id = match proc.get_object(epoll_handle) {
        Some(obj_arc) => {
            let obj = obj_arc.lock().unwrap();
            match obj.obj_type {
                ObjectType::Epoll { id } => id,
                _ => return Err(PosixError::EINVAL),
            }
        }
        None => return Err(PosixError::EBADF),
    };

    // Target FD must be a valid open descriptor
    let target_handle = Handle(fd as usize);
    if proc.get_object(target_handle).is_none() {
        return Err(PosixError::EBADF);
    }

    let sub = posix_subsystem().lock().unwrap();
    let ep_arc = match sub.epolls.get(&epoll_id) {
        Some(e) => e.clone(),
        None => return Err(PosixError::EBADF),
    };
    drop(sub);

    let mut ep = ep_arc.lock().unwrap();
    let target_fd = fd as i32;

    match op {
        EPOLL_CTL_ADD => {
            if ep.watches.contains_key(&target_fd) {
                return Err(PosixError::EEXIST);
            }
            if event_ptr == 0 {
                return Err(PosixError::EFAULT);
            }
            let ev = read_user_struct::<EpollEvent>(event_ptr)?;
            ep.watches.insert(target_fd, EpollWatch {
                fd: target_fd,
                events: ev.events,
                data: ev.data,
                oneshot_triggered: false,
            });
        }
        EPOLL_CTL_DEL => {
            if ep.watches.remove(&target_fd).is_none() {
                return Err(PosixError::ENOENT);
            }
        }
        EPOLL_CTL_MOD => {
            let watch = match ep.watches.get_mut(&target_fd) {
                Some(w) => w,
                None => return Err(PosixError::ENOENT),
            };
            if event_ptr == 0 {
                return Err(PosixError::EFAULT);
            }
            let ev = read_user_struct::<EpollEvent>(event_ptr)?;
            watch.events = ev.events;
            watch.data = ev.data;
            watch.oneshot_triggered = false;
        }
        _ => return Err(PosixError::EINVAL),
    }

    Ok(0)
}

/// `sys_epoll_wait` (Syscall #232): Wait for an I/O event on an epoll file descriptor.
pub fn sys_epoll_wait(
    proc: &mut HybridProcess,
    epfd: u64,
    events_ptr: u64,
    maxevents: i32,
    _timeout: i32,
) -> Result<u64, PosixError> {
    if maxevents <= 0 {
        return Err(PosixError::EINVAL);
    }
    if events_ptr == 0 {
        return Err(PosixError::EFAULT);
    }

    let epoll_handle = Handle(epfd as usize);
    let epoll_id = match proc.get_object(epoll_handle) {
        Some(obj_arc) => {
            let obj = obj_arc.lock().unwrap();
            match obj.obj_type {
                ObjectType::Epoll { id } => id,
                _ => return Err(PosixError::EINVAL),
            }
        }
        None => return Err(PosixError::EBADF),
    };

    let sub = posix_subsystem().lock().unwrap();
    let ep_arc = match sub.epolls.get(&epoll_id) {
        Some(e) => e.clone(),
        None => return Err(PosixError::EBADF),
    };
    drop(sub);

    let mut ep = ep_arc.lock().unwrap();
    let mut ready_events = Vec::new();
    let max = maxevents as usize;

    for (&target_fd, watch) in ep.watches.iter_mut() {
        if ready_events.len() >= max {
            break;
        }
        if watch.oneshot_triggered {
            continue;
        }

        let p_ev = check_fd_events(proc, target_fd as u64);
        let mut ep_mask = 0u32;
        if (p_ev & POLLIN) != 0 {
            ep_mask |= EPOLLIN;
        }
        if (p_ev & POLLOUT) != 0 {
            ep_mask |= EPOLLOUT;
        }
        if (p_ev & POLLERR) != 0 {
            ep_mask |= EPOLLERR;
        }
        if (p_ev & POLLHUP) != 0 {
            ep_mask |= EPOLLHUP;
        }
        if (p_ev & POLLRDHUP) != 0 {
            ep_mask |= EPOLLRDHUP;
        }

        let triggered = ep_mask & watch.events;
        if triggered != 0 {
            ready_events.push(EpollEvent {
                events: triggered,
                data: watch.data,
            });
            if (watch.events & EPOLLONESHOT) != 0 {
                watch.oneshot_triggered = true;
            }
        }
    }

    let count = ready_events.len();
    if count > 0 {
        let ep_size = std::mem::size_of::<EpollEvent>() as u64;
        for (i, ev) in ready_events.iter().enumerate() {
            write_user_struct(events_ptr + (i as u64 * ep_size), ev)?;
        }
    }

    Ok(count as u64)
}

// ============================================================================
// POSIX Signal Subsystem Handlers
// ============================================================================

/// `sys_rt_sigaction` (Syscall #13): Examine and change a signal action.
pub fn sys_rt_sigaction(
    proc: &mut HybridProcess,
    signum: u64,
    act_ptr: u64,
    oldact_ptr: u64,
    sigsetsize: u64,
) -> Result<u64, PosixError> {
    if sigsetsize != 8 {
        return Err(PosixError::EINVAL);
    }
    if signum == 0 || signum > NSIG as u64 {
        return Err(PosixError::EINVAL);
    }
    if signum == SIGKILL as u64 || signum == SIGSTOP as u64 {
        return Err(PosixError::EINVAL);
    }

    let mut sub = posix_subsystem().lock().unwrap();
    let sig_state = sub.get_or_create_signals(proc.pid);
    let sig_idx = signum as usize;

    if oldact_ptr != 0 {
        let old = sig_state.sigactions[sig_idx];
        write_user_struct(oldact_ptr, &old)?;
    }

    if act_ptr != 0 {
        let new_act = read_user_struct::<SigAction>(act_ptr)?;
        sig_state.sigactions[sig_idx] = new_act;
    }

    Ok(0)
}

/// `sys_rt_sigprocmask` (Syscall #14): Examine and change blocked signals.
pub fn sys_rt_sigprocmask(
    proc: &mut HybridProcess,
    how: i32,
    set_ptr: u64,
    oldset_ptr: u64,
    sigsetsize: u64,
) -> Result<u64, PosixError> {
    if sigsetsize != 8 {
        return Err(PosixError::EINVAL);
    }

    let mut sub = posix_subsystem().lock().unwrap();
    let sig_state = sub.get_or_create_signals(proc.pid);

    if oldset_ptr != 0 {
        write_user_struct(oldset_ptr, &sig_state.blocked_mask)?;
    }

    if set_ptr != 0 {
        let mut new_mask = read_user_struct::<u64>(set_ptr)?;
        // SIGKILL and SIGSTOP can never be blocked
        new_mask &= !(1u64 << (SIGKILL - 1));
        new_mask &= !(1u64 << (SIGSTOP - 1));

        match how {
            SIG_BLOCK => {
                sig_state.blocked_mask |= new_mask;
            }
            SIG_UNBLOCK => {
                sig_state.blocked_mask &= !new_mask;
            }
            SIG_SETMASK => {
                sig_state.blocked_mask = new_mask;
            }
            _ => return Err(PosixError::EINVAL),
        }
    }

    Ok(0)
}

/// `sys_rt_sigreturn` (Syscall #15): Return from signal handler and cleanup stack frame.
pub fn sys_rt_sigreturn(
    proc: &mut HybridProcess,
    tf_opt: Option<&mut TrapFrame>,
) -> Result<u64, PosixError> {
    let mut sub = posix_subsystem().lock().unwrap();
    let sig_state = sub.get_or_create_signals(proc.pid);

    if let Some(saved_mask) = sig_state.saved_mask.take() {
        sig_state.blocked_mask = saved_mask;
    }

    if let Some(saved_frame) = sig_state.saved_trap_frame.take() {
        let restored_rax = saved_frame.rax;
        if let Some(tf) = tf_opt {
            *tf = saved_frame;
        }
        Ok(restored_rax)
    } else {
        Ok(0)
    }
}

/// `sys_kill` (Syscall #62): Send signal to a process.
pub fn sys_kill(
    proc: &mut HybridProcess,
    pid: i64,
    sig: i32,
) -> Result<u64, PosixError> {
    if sig < 0 || sig > NSIG as i32 {
        return Err(PosixError::EINVAL);
    }

    if sig == 0 {
        // Null signal: check permissions / process existence
        if pid == (proc.pid as i64) || pid == 0 || pid == -1 {
            return Ok(0);
        }
        let sub = posix_subsystem().lock().unwrap();
        if sub.signals.contains_key(&(pid as u32)) {
            return Ok(0);
        }
        return Err(PosixError::ESRCH);
    }

    let sig_u32 = sig as u32;

    if pid == (proc.pid as i64) || pid == 0 {
        if sig_u32 == SIGKILL {
            proc.is_terminated = true;
            proc.exit_code = 128 + sig;
            return Ok(0);
        }

        let mut sub = posix_subsystem().lock().unwrap();
        let sig_state = sub.get_or_create_signals(proc.pid);
        let action = sig_state.sigactions[sig as usize];

        if action.sa_handler == SIG_IGN {
            return Ok(0);
        } else if action.sa_handler == SIG_DFL {
            match sig_u32 {
                SIGINT | SIGTERM | SIGSEGV | SIGABRT | SIGQUIT | SIGILL | SIGFPE | SIGBUS | SIGPIPE => {
                    proc.is_terminated = true;
                    proc.exit_code = 128 + sig;
                }
                SIGCHLD | SIGURG | SIGWINCH => {
                    // Default ignore
                }
                _ => {
                    proc.is_terminated = true;
                    proc.exit_code = 128 + sig;
                }
            }
        } else {
            // Enqueue pending signal bit
            sig_state.pending_mask |= 1u64 << (sig_u32 - 1);
        }
        Ok(0)
    } else if pid > 0 {
        let target_pid = pid as u32;
        let mut sub = posix_subsystem().lock().unwrap();
        if let Some(target_sig) = sub.signals.get_mut(&target_pid) {
            let action = target_sig.sigactions[sig as usize];
            if action.sa_handler != SIG_IGN {
                target_sig.pending_mask |= 1u64 << (sig_u32 - 1);
            }
            Ok(0)
        } else {
            // For general POSIX processes in HybridOS
            Err(PosixError::ESRCH)
        }
    } else {
        Ok(0)
    }
}

// ============================================================================
// Syscall Dispatcher for Advanced POSIX Subsystem
// ============================================================================

pub fn dispatch_posix_advanced_syscall(
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
        SYS_POLL => sys_poll(proc, a1, a2, a3 as i64),
        SYS_RT_SIGACTION => sys_rt_sigaction(proc, a1, a2, a3, a4),
        SYS_RT_SIGPROCMASK => sys_rt_sigprocmask(proc, a1 as i32, a2, a3, a4),
        SYS_RT_SIGRETURN => sys_rt_sigreturn(proc, None),
        SYS_SELECT => sys_select(proc, a1, a2, a3, a4, a5),
        SYS_SOCKET => sys_socket(proc, a1 as i32, a2 as i32, a3 as i32),
        SYS_CONNECT => sys_connect(proc, a1, a2, a3),
        SYS_ACCEPT => sys_accept(proc, a1, a2, a3),
        SYS_ACCEPT4 => sys_accept4(proc, a1, a2, a3, a4 as i32),
        SYS_SENDTO => sys_sendto(proc, a1, a2, a3, a4, a5, a6),
        SYS_RECVFROM => sys_recvfrom(proc, a1, a2, a3, a4, a5, a6),
        SYS_SENDMSG => sys_sendmsg(proc, a1, a2, a3),
        SYS_RECVMSG => sys_recvmsg(proc, a1, a2, a3),
        SYS_SHUTDOWN => sys_shutdown(proc, a1, a2 as i32),
        SYS_BIND => sys_bind(proc, a1, a2, a3),
        SYS_LISTEN => sys_listen(proc, a1, a2 as i32),
        SYS_GETSOCKNAME => sys_getsockname(proc, a1, a2, a3),
        SYS_GETPEERNAME => sys_getpeername(proc, a1, a2, a3),
        SYS_SOCKETPAIR => sys_socketpair(proc, a1 as i32, a2 as i32, a3 as i32, a4),
        SYS_SETSOCKOPT => sys_setsockopt(proc, a1, a2 as i32, a3 as i32, a4, a5),
        SYS_GETSOCKOPT => sys_getsockopt(proc, a1, a2 as i32, a3 as i32, a4, a5),
        SYS_KILL => sys_kill(proc, a1 as i64, a2 as i32),
        SYS_EPOLL_CREATE | SYS_EPOLL_CREATE1 => sys_epoll_create1(proc, a1),
        SYS_EPOLL_CTL => sys_epoll_ctl(proc, a1, a2 as i32, a3, a4),
        SYS_EPOLL_WAIT => sys_epoll_wait(proc, a1, a2, a3 as i32, a4 as i32),
        _ => Err(PosixError::ENOSYS),
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
        HybridProcess::new(pid, "test_posix_advanced", ExecutionPersonality::LinuxPosix)
    }

    #[test]
    fn test_socket_creation_and_options() {
        let mut proc = create_test_process(4001);

        // 1. Create stream socket
        let fd = sys_socket(&mut proc, AF_INET, SOCK_STREAM, 0).unwrap();
        assert!(fd >= 3);

        // 2. Set SO_REUSEADDR
        let mut optval: i32 = 1;
        let setsock_res = sys_setsockopt(
            &mut proc,
            fd,
            SOL_SOCKET,
            SO_REUSEADDR,
            &mut optval as *mut i32 as u64,
            std::mem::size_of::<i32>() as u64,
        );
        assert_eq!(setsock_res, Ok(0));

        // 3. Query SO_REUSEADDR
        let mut retrieved_val: i32 = 0;
        let mut optlen: u32 = 4;
        let getsock_res = sys_getsockopt(
            &mut proc,
            fd,
            SOL_SOCKET,
            SO_REUSEADDR,
            &mut retrieved_val as *mut i32 as u64,
            &mut optlen as *mut u32 as u64,
        );
        assert_eq!(getsock_res, Ok(0));
        assert_eq!(retrieved_val, 1);

        // 4. Test TCP_NODELAY
        let mut nodelay: i32 = 1;
        assert_eq!(sys_setsockopt(
            &mut proc,
            fd,
            IPPROTO_TCP_LEVEL,
            TCP_NODELAY,
            &mut nodelay as *mut i32 as u64,
            4,
        ), Ok(0));

        let mut read_nodelay: i32 = 0;
        assert_eq!(sys_getsockopt(
            &mut proc,
            fd,
            IPPROTO_TCP_LEVEL,
            TCP_NODELAY,
            &mut read_nodelay as *mut i32 as u64,
            &mut optlen as *mut u32 as u64,
        ), Ok(0));
        assert_eq!(read_nodelay, 1);

        // 5. Test buffer sizing (SO_RCVBUF / SO_SNDBUF)
        let mut buf_size: i32 = 131072;
        assert_eq!(sys_setsockopt(
            &mut proc,
            fd,
            SOL_SOCKET,
            SO_RCVBUF,
            &mut buf_size as *mut i32 as u64,
            4,
        ), Ok(0));

        let mut read_buf_size: i32 = 0;
        assert_eq!(sys_getsockopt(
            &mut proc,
            fd,
            SOL_SOCKET,
            SO_RCVBUF,
            &mut read_buf_size as *mut i32 as u64,
            &mut optlen as *mut u32 as u64,
        ), Ok(0));
        assert_eq!(read_buf_size, 131072);
    }

    #[test]
    fn test_socketpair_bidirectional_stream() {
        let mut proc = create_test_process(4002);
        let mut sv = [0i32; 2];
        let sv_ptr = sv.as_mut_ptr() as u64;

        // Create socketpair
        assert_eq!(sys_socketpair(&mut proc, AF_UNIX, SOCK_STREAM, 0, sv_ptr), Ok(0));
        let fd0 = sv[0] as u64;
        let fd1 = sv[1] as u64;
        assert!(fd0 >= 3 && fd1 >= 3);

        // Send from fd0 to fd1
        let msg1 = b"HybridOS IPC Channel Ping";
        let sent1 = sys_sendto(&mut proc, fd0, msg1.as_ptr() as u64, msg1.len() as u64, 0, 0, 0).unwrap();
        assert_eq!(sent1, msg1.len() as u64);

        // Receive on fd1
        let mut buf1 = [0u8; 64];
        let recv1 = sys_recvfrom(&mut proc, fd1, buf1.as_mut_ptr() as u64, buf1.len() as u64, 0, 0, 0).unwrap();
        assert_eq!(recv1, msg1.len() as u64);
        assert_eq!(&buf1[..msg1.len()], msg1);

        // Send from fd1 back to fd0
        let msg2 = b"HybridOS IPC Channel Pong";
        let sent2 = sys_sendto(&mut proc, fd1, msg2.as_ptr() as u64, msg2.len() as u64, 0, 0, 0).unwrap();
        assert_eq!(sent2, msg2.len() as u64);

        let mut buf2 = [0u8; 64];
        let recv2 = sys_recvfrom(&mut proc, fd0, buf2.as_mut_ptr() as u64, buf2.len() as u64, 0, 0, 0).unwrap();
        assert_eq!(recv2, msg2.len() as u64);
        assert_eq!(&buf2[..msg2.len()], msg2);
    }

    #[test]
    fn test_socket_bind_listen_connect_accept() {
        let mut server = create_test_process(4003);
        let mut client = create_test_process(4004);

        // Server setup: socket, bind, listen
        let srv_fd = sys_socket(&mut server, AF_INET, SOCK_STREAM, 0).unwrap();
        let srv_addr = SockAddrIn {
            sin_family: AF_INET as u16,
            sin_port: 8080u16.to_be(),
            sin_addr: 0x7F000001u32.to_be(), // 127.0.0.1
            sin_zero: [0; 8],
        };
        let srv_addr_ptr = &srv_addr as *const SockAddrIn as u64;
        let srv_addr_len = std::mem::size_of::<SockAddrIn>() as u64;

        assert_eq!(sys_bind(&mut server, srv_fd, srv_addr_ptr, srv_addr_len), Ok(0));
        assert_eq!(sys_listen(&mut server, srv_fd, 10), Ok(0));

        // Client setup: socket, connect
        let cli_fd = sys_socket(&mut client, AF_INET, SOCK_STREAM, 0).unwrap();
        assert_eq!(sys_connect(&mut client, cli_fd, srv_addr_ptr, srv_addr_len), Ok(0));

        // Server accepts connection
        let mut peer_addr = SockAddrIn::default();
        let mut peer_len = std::mem::size_of::<SockAddrIn>() as u32;
        let acc_fd = sys_accept(
            &mut server,
            srv_fd,
            &mut peer_addr as *mut SockAddrIn as u64,
            &mut peer_len as *mut u32 as u64,
        ).unwrap();
        assert!(acc_fd >= 3);

        // Data transfer from client to accepted server socket
        let http_req = b"GET /index.html HTTP/1.1\r\nHost: localhost\r\n\r\n";
        let sent = sys_sendto(&mut client, cli_fd, http_req.as_ptr() as u64, http_req.len() as u64, 0, 0, 0).unwrap();
        assert_eq!(sent, http_req.len() as u64);

        let mut recv_buf = [0u8; 128];
        let recvd = sys_recvfrom(&mut server, acc_fd, recv_buf.as_mut_ptr() as u64, recv_buf.len() as u64, 0, 0, 0).unwrap();
        assert_eq!(recvd, http_req.len() as u64);
        assert_eq!(&recv_buf[..recvd as usize], http_req);
    }

    #[test]
    fn test_socket_sendmsg_recvmsg_iovec() {
        let mut proc = create_test_process(4005);
        let mut sv = [0i32; 2];
        assert_eq!(sys_socketpair(&mut proc, AF_UNIX, SOCK_STREAM, 0, sv.as_mut_ptr() as u64), Ok(0));
        let fd0 = sv[0] as u64;
        let fd1 = sv[1] as u64;

        // Prepare scatter-gather message
        let h1 = b"Header: Part 1; ";
        let h2 = b"Body: Part 2; ";
        let h3 = b"Trailer: Part 3.";

        let iovecs = [
            IoVec { iov_base: h1.as_ptr() as u64, iov_len: h1.len() as u64 },
            IoVec { iov_base: h2.as_ptr() as u64, iov_len: h2.len() as u64 },
            IoVec { iov_base: h3.as_ptr() as u64, iov_len: h3.len() as u64 },
        ];

        let msg_out = MsgHdr {
            msg_name: 0,
            msg_namelen: 0,
            __pad1: 0,
            msg_iov: iovecs.as_ptr() as u64,
            msg_iovlen: 3,
            msg_control: 0,
            msg_controllen: 0,
            msg_flags: 0,
            __pad2: 0,
        };

        let total_expected = (h1.len() + h2.len() + h3.len()) as u64;
        let sent = sys_sendmsg(&mut proc, fd0, &msg_out as *const MsgHdr as u64, 0).unwrap();
        assert_eq!(sent, total_expected);

        // Receive scatter into separate destination buffers
        let mut r1 = [0u8; 16];
        let mut r2 = [0u8; 14];
        let mut r3 = [0u8; 16];

        let mut recv_iovecs = [
            IoVec { iov_base: r1.as_mut_ptr() as u64, iov_len: r1.len() as u64 },
            IoVec { iov_base: r2.as_mut_ptr() as u64, iov_len: r2.len() as u64 },
            IoVec { iov_base: r3.as_mut_ptr() as u64, iov_len: r3.len() as u64 },
        ];

        let mut msg_in = MsgHdr {
            msg_name: 0,
            msg_namelen: 0,
            __pad1: 0,
            msg_iov: recv_iovecs.as_mut_ptr() as u64,
            msg_iovlen: 3,
            msg_control: 0,
            msg_controllen: 0,
            msg_flags: 0,
            __pad2: 0,
        };

        let recvd = sys_recvmsg(&mut proc, fd1, &mut msg_in as *mut MsgHdr as u64, 0).unwrap();
        assert_eq!(recvd, total_expected);
        assert_eq!(&r1, h1);
        assert_eq!(&r2, h2);
        assert_eq!(&r3, h3);
    }

    #[test]
    fn test_socket_shutdown_semantics() {
        let mut proc = create_test_process(4006);
        let mut sv = [0i32; 2];
        assert_eq!(sys_socketpair(&mut proc, AF_UNIX, SOCK_STREAM, 0, sv.as_mut_ptr() as u64), Ok(0));
        let fd0 = sv[0] as u64;
        let fd1 = sv[1] as u64;

        // Shutdown WR on fd0
        assert_eq!(sys_shutdown(&mut proc, fd0, SHUT_WR), Ok(0));

        // Further writes on fd0 should fail with EPIPE
        let dummy = b"test";
        assert_eq!(sys_sendto(&mut proc, fd0, dummy.as_ptr() as u64, 4, 0, 0, 0), Err(PosixError::EPIPE));

        // Reading on fd1 should yield 0 (EOF)
        let mut in_buf = [0u8; 10];
        let eof = sys_recvfrom(&mut proc, fd1, in_buf.as_mut_ptr() as u64, 10, 0, 0, 0).unwrap();
        assert_eq!(eof, 0);
    }

    #[test]
    fn test_socket_getsockname_getpeername() {
        let mut proc = create_test_process(4007);
        let fd = sys_socket(&mut proc, AF_INET, SOCK_STREAM, 0).unwrap();

        let srv_addr = SockAddrIn {
            sin_family: AF_INET as u16,
            sin_port: 9090u16.to_be(),
            sin_addr: 0x7F000001u32.to_be(),
            sin_zero: [0; 8],
        };
        assert_eq!(sys_bind(&mut proc, fd, &srv_addr as *const SockAddrIn as u64, std::mem::size_of::<SockAddrIn>() as u64), Ok(0));

        // Query getsockname
        let mut name_out = SockAddrIn::default();
        let mut name_len = std::mem::size_of::<SockAddrIn>() as u32;
        assert_eq!(sys_getsockname(&mut proc, fd, &mut name_out as *mut SockAddrIn as u64, &mut name_len as *mut u32 as u64), Ok(0));
        assert_eq!(name_out.sin_family, AF_INET as u16);
        assert_eq!(name_out.sin_port, 9090u16.to_be());

        // getpeername before connect should return ENOTCONN
        let mut peer_out = SockAddrIn::default();
        let mut peer_len = std::mem::size_of::<SockAddrIn>() as u32;
        assert_eq!(sys_getpeername(&mut proc, fd, &mut peer_out as *mut SockAddrIn as u64, &mut peer_len as *mut u32 as u64), Err(PosixError::ENOTCONN));
    }

    #[test]
    fn test_poll_descriptors() {
        let mut proc = create_test_process(4008);
        let mut sv = [0i32; 2];
        assert_eq!(sys_socketpair(&mut proc, AF_UNIX, SOCK_STREAM, 0, sv.as_mut_ptr() as u64), Ok(0));
        let fd0 = sv[0] as u64;
        let fd1 = sv[1] as u64;

        // Poll before writing: fd0 and fd1 should be writable (POLLOUT) but not readable (POLLIN)
        let mut pfds = [
            PollFd { fd: fd0 as i32, events: POLLIN | POLLOUT, revents: 0 },
            PollFd { fd: fd1 as i32, events: POLLIN | POLLOUT, revents: 0 },
        ];

        let ready = sys_poll(&mut proc, pfds.as_mut_ptr() as u64, 2, 0).unwrap();
        assert_eq!(ready, 2);
        assert_eq!(pfds[0].revents, POLLOUT);
        assert_eq!(pfds[1].revents, POLLOUT);

        // Write from fd0 to fd1
        let msg = b"data for poll";
        assert_eq!(sys_sendto(&mut proc, fd0, msg.as_ptr() as u64, msg.len() as u64, 0, 0, 0), Ok(msg.len() as u64));

        // Now fd1 should have POLLIN
        let ready2 = sys_poll(&mut proc, pfds.as_mut_ptr() as u64, 2, 0).unwrap();
        assert_eq!(ready2, 2);
        assert_eq!(pfds[1].revents & POLLIN, POLLIN);
    }

    #[test]
    fn test_select_descriptors() {
        let mut proc = create_test_process(4009);
        let mut sv = [0i32; 2];
        assert_eq!(sys_socketpair(&mut proc, AF_UNIX, SOCK_STREAM, 0, sv.as_mut_ptr() as u64), Ok(0));
        let fd0 = sv[0] as usize;
        let fd1 = sv[1] as usize;

        // Send data into fd1
        let data = b"select test data";
        assert_eq!(sys_sendto(&mut proc, fd0 as u64, data.as_ptr() as u64, data.len() as u64, 0, 0, 0), Ok(data.len() as u64));

        let mut readfds = FdSet::default();
        readfds.set(fd1);

        let mut writefds = FdSet::default();
        writefds.set(fd0);

        let max_fd = fd0.max(fd1) + 1;
        let count = sys_select(
            &mut proc,
            max_fd as u64,
            &mut readfds as *mut FdSet as u64,
            &mut writefds as *mut FdSet as u64,
            0,
            0,
        ).unwrap();

        assert_eq!(count, 2);
        assert!(readfds.is_set(fd1));
        assert!(writefds.is_set(fd0));
    }

    #[test]
    fn test_epoll_create_ctl_wait_lifecycle() {
        let mut proc = create_test_process(4010);

        // 1. Create epoll instance
        let epfd = sys_epoll_create1(&mut proc, 0).unwrap();
        assert!(epfd >= 3);

        // 2. Create socketpair
        let mut sv = [0i32; 2];
        assert_eq!(sys_socketpair(&mut proc, AF_UNIX, SOCK_STREAM, 0, sv.as_mut_ptr() as u64), Ok(0));
        let fd0 = sv[0] as u64;
        let fd1 = sv[1] as u64;

        // 3. Add fd1 to epoll monitoring with EPOLLIN
        let mut watch_event = EpollEvent {
            events: EPOLLIN,
            data: 0xCAFEBABE,
        };
        assert_eq!(sys_epoll_ctl(&mut proc, epfd, EPOLL_CTL_ADD, fd1, &mut watch_event as *mut EpollEvent as u64), Ok(0));

        // 4. Poll before writing - wait should return 0 ready events
        let mut events = [EpollEvent::default(); 4];
        let count0 = sys_epoll_wait(&mut proc, epfd, events.as_mut_ptr() as u64, 4, 0).unwrap();
        assert_eq!(count0, 0);

        // 5. Write to fd0 -> fd1 receives data
        let packet = b"Epoll event notification trigger";
        assert_eq!(sys_sendto(&mut proc, fd0, packet.as_ptr() as u64, packet.len() as u64, 0, 0, 0), Ok(packet.len() as u64));

        // 6. Epoll wait should now trigger EPOLLIN on fd1
        let count1 = sys_epoll_wait(&mut proc, epfd, events.as_mut_ptr() as u64, 4, 0).unwrap();
        assert_eq!(count1, 1);
        assert_eq!(events[0].events & EPOLLIN, EPOLLIN);
        let event_data = events[0].data;
        assert_eq!(event_data, 0xCAFEBABE);

        // 7. Test EPOLL_CTL_DEL
        assert_eq!(sys_epoll_ctl(&mut proc, epfd, EPOLL_CTL_DEL, fd1, 0), Ok(0));
        let count2 = sys_epoll_wait(&mut proc, epfd, events.as_mut_ptr() as u64, 4, 0).unwrap();
        assert_eq!(count2, 0);
    }

    #[test]
    fn test_signal_rt_sigaction_install() {
        let mut proc = create_test_process(4011);

        let new_act = SigAction {
            sa_handler: 0x0000_7FFF_1122_3344,
            sa_flags: SA_SIGINFO | SA_RESTART,
            sa_restorer: 0x0000_7FFF_5566_7788,
            sa_mask: 0x0000_0000_0000_000F,
        };

        let mut old_act = SigAction::default();

        // Install handler for SIGTERM
        assert_eq!(sys_rt_sigaction(
            &mut proc,
            SIGTERM as u64,
            &new_act as *const SigAction as u64,
            &mut old_act as *mut SigAction as u64,
            8,
        ), Ok(0));
        assert_eq!(old_act.sa_handler, SIG_DFL);

        // Query again to verify installation
        let mut check_act = SigAction::default();
        assert_eq!(sys_rt_sigaction(
            &mut proc,
            SIGTERM as u64,
            0,
            &mut check_act as *mut SigAction as u64,
            8,
        ), Ok(0));
        assert_eq!(check_act.sa_handler, new_act.sa_handler);
        assert_eq!(check_act.sa_flags, new_act.sa_flags);
        assert_eq!(check_act.sa_mask, new_act.sa_mask);

        // SIGKILL cannot be overridden
        assert_eq!(sys_rt_sigaction(
            &mut proc,
            SIGKILL as u64,
            &new_act as *const SigAction as u64,
            0,
            8,
        ), Err(PosixError::EINVAL));
    }

    #[test]
    fn test_signal_sigprocmask_block_unblock() {
        let mut proc = create_test_process(4012);

        // Block SIGINT (bit 2) and SIGTERM (bit 15)
        let block_set: u64 = (1u64 << (SIGINT - 1)) | (1u64 << (SIGTERM - 1));
        let mut old_set: u64 = 0;

        assert_eq!(sys_rt_sigprocmask(
            &mut proc,
            SIG_BLOCK,
            &block_set as *const u64 as u64,
            &mut old_set as *mut u64 as u64,
            8,
        ), Ok(0));
        assert_eq!(old_set, 0);

        // Query mask
        let mut current_set: u64 = 0;
        assert_eq!(sys_rt_sigprocmask(
            &mut proc,
            SIG_SETMASK,
            0,
            &mut current_set as *mut u64 as u64,
            8,
        ), Ok(0));
        assert_eq!(current_set & block_set, block_set);

        // Unblock SIGINT
        let unblock_set: u64 = 1u64 << (SIGINT - 1);
        assert_eq!(sys_rt_sigprocmask(
            &mut proc,
            SIG_UNBLOCK,
            &unblock_set as *const u64 as u64,
            0,
            8,
        ), Ok(0));

        // Verify only SIGTERM remains blocked
        let mut final_set: u64 = 0;
        assert_eq!(sys_rt_sigprocmask(
            &mut proc,
            SIG_SETMASK,
            0,
            &mut final_set as *mut u64 as u64,
            8,
        ), Ok(0));
        assert_eq!(final_set & (1u64 << (SIGINT - 1)), 0);
        assert_eq!(final_set & (1u64 << (SIGTERM - 1)), 1u64 << (SIGTERM - 1));
    }

    #[test]
    fn test_signal_kill_delivery_and_termination() {
        let mut proc = create_test_process(4013);

        // Send SIGKILL (should terminate process)
        assert_eq!(sys_kill(&mut proc, 4013, SIGKILL as i32), Ok(0));
        assert!(proc.is_terminated);
        assert_eq!(proc.exit_code, 128 + SIGKILL as i32);

        // Null signal check on existing process
        let mut live_proc = create_test_process(4014);
        assert_eq!(sys_kill(&mut live_proc, 4014, 0), Ok(0));

        // Invalid signum returns EINVAL
        assert_eq!(sys_kill(&mut live_proc, 4014, 999), Err(PosixError::EINVAL));
    }

    #[test]
    fn test_signal_rt_sigreturn_frame_restoration() {
        let mut proc = create_test_process(4015);

        // Set up saved trap frame
        {
            let mut sub = posix_subsystem().lock().unwrap();
            let sig_state = sub.get_or_create_signals(proc.pid);
            let mut tf = TrapFrame::default();
            tf.rax = 42;
            tf.rip = 0x0000_7FFF_AABB_CCDD;
            tf.rsp = 0x0000_7FFF_EEEE_FFFF;
            sig_state.saved_trap_frame = Some(tf);
            sig_state.saved_mask = Some(0x1234);
        }

        let mut current_frame = TrapFrame::default();
        let ret_rax = sys_rt_sigreturn(&mut proc, Some(&mut current_frame)).unwrap();
        assert_eq!(ret_rax, 42);
        assert_eq!(current_frame.rip, 0x0000_7FFF_AABB_CCDD);
        assert_eq!(current_frame.rsp, 0x0000_7FFF_EEEE_FFFF);

        // Signal mask should also be restored
        let sub = posix_subsystem().lock().unwrap();
        let sig_state = sub.signals.get(&proc.pid).unwrap();
        assert_eq!(sig_state.blocked_mask, 0x1234);
    }

    #[test]
    fn test_posix_advanced_dispatcher_routing() {
        let mut proc = create_test_process(4016);

        // Dispatch SYS_SOCKET (41)
        let fd = dispatch_posix_advanced_syscall(&mut proc, SYS_SOCKET, AF_INET as u64, SOCK_STREAM as u64, 0, 0, 0, 0).unwrap();
        assert!(fd >= 3);

        // Dispatch SYS_POLL (7)
        let mut pfd = PollFd { fd: fd as i32, events: POLLOUT, revents: 0 };
        let ready = dispatch_posix_advanced_syscall(&mut proc, SYS_POLL, &mut pfd as *mut PollFd as u64, 1, 0, 0, 0, 0).unwrap();
        assert_eq!(ready, 1);

        // Dispatch unknown syscall returns ENOSYS
        assert_eq!(dispatch_posix_advanced_syscall(&mut proc, 8888, 0, 0, 0, 0, 0, 0), Err(PosixError::ENOSYS));
    }
}
