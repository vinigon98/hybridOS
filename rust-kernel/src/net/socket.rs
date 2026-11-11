//! BSD Socket Abstraction Layer & VFS Handle Bridge
//!
//! Provides the standard BSD socket programming interface for both Windows NT
//! (Winsock2 / `WSASocketW`) and Linux POSIX (`sys_socket`):
//! - [`SocketType`]: Stream (TCP), Datagram (UDP), Raw IP.
//! - [`AddressFamily`]: AF_INET (IPv4), AF_INET6, AF_UNIX.
//! - [`SocketAddress`]: IPv4 network endpoint (IP + Port).
//! - [`RingBuffer`]: High-performance circular byte buffer for TX/RX streams.
//! - Non-blocking I/O mode with `EAGAIN` / `EWOULDBLOCK` handling.
//! - [`Socket`]: Full BSD socket object implementing standard operations.
//! - [`SocketFileOps`]: Direct bridge integrating sockets into the HybridOS VFS.

#[allow(unused_imports)]
use alloc::string::{String, ToString};
#[allow(unused_imports)]
use alloc::vec::Vec;
#[allow(unused_imports)]
use alloc::boxed::Box;
#[allow(unused_imports)]
use crate::compat::FloatExt;
use std::collections::VecDeque;
use std::fmt;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, RwLock, Weak};

use crate::types::{NtStatus, PosixError};
use crate::vfs::inode::{FileDescriptor, FileOperations, Inode, InodeType, OpenFlags, Permissions};
use crate::vfs::VfsError;
use super::udp::{UdpDatagram, UdpSubsystem};
use super::tcp::TcpConnection;
use super::filter::{global_network_filter, FilterVerdict, NetworkFilterContext, NetworkFilterOp};

/// Standard BSD Socket Types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum SocketType {
    /// Sequenced, reliable, two-way, connection-based byte streams (TCP).
    Stream = 1,
    /// Connectionless, unreliable messages of a fixed maximum length (UDP).
    Dgram = 2,
    /// Raw network protocol interface.
    Raw = 3,
}

impl SocketType {
    pub fn from_i32(val: i32) -> Result<Self, SocketError> {
        match val & 0xFF {
            1 => Ok(SocketType::Stream),
            2 => Ok(SocketType::Dgram),
            3 => Ok(SocketType::Raw),
            _ => Err(SocketError::InvalidArgument),
        }
    }
}

/// Standard BSD Address Families.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum AddressFamily {
    AfUnspec = 0,
    AfUnix = 1,
    AfInet = 2,
    AfInet6 = 10,
}

impl AddressFamily {
    pub fn from_i32(val: i32) -> Result<Self, SocketError> {
        match val {
            0 => Ok(AddressFamily::AfUnspec),
            1 => Ok(AddressFamily::AfUnix),
            2 => Ok(AddressFamily::AfInet),
            10 => Ok(AddressFamily::AfInet6),
            _ => Err(SocketError::InvalidArgument),
        }
    }
}

/// Transport / Network Protocols.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum SocketProtocol {
    Ip = 0,
    Icmp = 1,
    Tcp = 6,
    Udp = 17,
    Raw = 255,
}

impl SocketProtocol {
    pub fn from_i32(val: i32) -> Result<Self, SocketError> {
        match val {
            0 => Ok(SocketProtocol::Ip),
            1 => Ok(SocketProtocol::Icmp),
            6 => Ok(SocketProtocol::Tcp),
            17 => Ok(SocketProtocol::Udp),
            255 => Ok(SocketProtocol::Raw),
            _ => Err(SocketError::InvalidArgument),
        }
    }
}

/// Socket Operational States.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketState {
    Unbound,
    Bound,
    Listening,
    Connecting,
    Connected,
    Closing,
    Closed,
}

/// Socket Shutdown modes (`shutdown(2)`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShutdownHow {
    Read = 0,
    Write = 1,
    Both = 2,
}

/// BSD Socket Error Codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketError {
    BadFileDescriptor,
    InvalidAddress,
    AlreadyBound,
    NotBound,
    NotListening,
    NotConnected,
    AlreadyConnected,
    ConnectionRefused,
    ConnectionReset,
    AddressInUse,
    WouldBlock,
    TimedOut,
    BufferOverflow,
    InvalidArgument,
    UnsupportedOperation,
    Closed,
}

impl fmt::Display for SocketError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SocketError::BadFileDescriptor => write!(f, "Bad socket descriptor"),
            SocketError::InvalidAddress => write!(f, "Invalid socket address format"),
            SocketError::AlreadyBound => write!(f, "Socket is already bound to an address"),
            SocketError::NotBound => write!(f, "Socket is not bound"),
            SocketError::NotListening => write!(f, "Socket is not in listening state"),
            SocketError::NotConnected => write!(f, "Socket is not connected"),
            SocketError::AlreadyConnected => write!(f, "Socket is already connected"),
            SocketError::ConnectionRefused => write!(f, "Connection refused by peer"),
            SocketError::ConnectionReset => write!(f, "Connection reset by peer"),
            SocketError::AddressInUse => write!(f, "Address or port is already in use"),
            SocketError::WouldBlock => write!(f, "Resource temporarily unavailable (Operation would block)"),
            SocketError::TimedOut => write!(f, "Socket operation timed out"),
            SocketError::BufferOverflow => write!(f, "Socket buffer overflow"),
            SocketError::InvalidArgument => write!(f, "Invalid argument passed to socket operation"),
            SocketError::UnsupportedOperation => write!(f, "Operation not supported on this socket type"),
            SocketError::Closed => write!(f, "Socket has been closed"),
        }
    }
}

impl std::error::Error for SocketError {}

impl From<SocketError> for PosixError {
    fn from(err: SocketError) -> Self {
        match err {
            SocketError::BadFileDescriptor => PosixError::EBADF,
            SocketError::InvalidAddress => PosixError::EINVAL,
            SocketError::AlreadyBound => PosixError::EINVAL,
            SocketError::NotBound => PosixError::EINVAL,
            SocketError::NotListening => PosixError::EINVAL,
            SocketError::NotConnected => PosixError::ENOTTY,
            SocketError::AlreadyConnected => PosixError::EEXIST,
            SocketError::ConnectionRefused => PosixError::ECONNREFUSED,
            SocketError::ConnectionReset => PosixError::EIO,
            SocketError::AddressInUse => PosixError::EEXIST,
            SocketError::WouldBlock => PosixError::EAGAIN,
            SocketError::TimedOut => PosixError::EIO,
            SocketError::BufferOverflow => PosixError::EFBIG,
            SocketError::InvalidArgument => PosixError::EINVAL,
            SocketError::UnsupportedOperation => PosixError::ENOSYS,
            SocketError::Closed => PosixError::EPIPE,
        }
    }
}

impl From<SocketError> for NtStatus {
    fn from(err: SocketError) -> Self {
        match err {
            SocketError::BadFileDescriptor => NtStatus::INVALID_HANDLE,
            SocketError::InvalidAddress => NtStatus::INVALID_PARAMETER,
            SocketError::AlreadyBound => NtStatus::INVALID_PARAMETER,
            SocketError::NotBound => NtStatus::INVALID_PARAMETER,
            SocketError::NotListening => NtStatus::INVALID_PARAMETER,
            SocketError::NotConnected => NtStatus::INVALID_PARAMETER,
            SocketError::AlreadyConnected => NtStatus::INVALID_PARAMETER,
            SocketError::ConnectionRefused => NtStatus::ACCESS_DENIED,
            SocketError::ConnectionReset => NtStatus::UNSUCCESSFUL,
            SocketError::AddressInUse => NtStatus(-1073741771), // STATUS_OBJECT_NAME_COLLISION
            SocketError::WouldBlock => NtStatus::PENDING,
            SocketError::TimedOut => NtStatus::TIMEOUT,
            SocketError::BufferOverflow => NtStatus::BUFFER_TOO_SMALL,
            SocketError::InvalidArgument => NtStatus::INVALID_PARAMETER,
            SocketError::UnsupportedOperation => NtStatus::NOT_SUPPORTED,
            SocketError::Closed => NtStatus::END_OF_FILE,
        }
    }
}

impl From<SocketError> for VfsError {
    fn from(err: SocketError) -> Self {
        match err {
            SocketError::BadFileDescriptor => VfsError::BadFileDescriptor,
            SocketError::InvalidArgument => VfsError::InvalidArgument,
            SocketError::UnsupportedOperation => VfsError::NotSupported,
            SocketError::Closed => VfsError::IoError,
            _ => VfsError::IoError,
        }
    }
}

/// Standard IPv4 Endpoint Address (IP + Port).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SocketAddress {
    pub ip: [u8; 4],
    pub port: u16,
}

impl SocketAddress {
    pub const fn new(a: u8, b: u8, c: u8, d: u8, port: u16) -> Self {
        Self {
            ip: [a, b, c, d],
            port,
        }
    }

    pub const fn loopback(port: u16) -> Self {
        Self::new(127, 0, 0, 1, port)
    }

    pub const fn any(port: u16) -> Self {
        Self::new(0, 0, 0, 0, port)
    }

    #[inline]
    pub fn is_loopback(&self) -> bool {
        self.ip[0] == 127
    }

    #[inline]
    pub fn is_unspecified(&self) -> bool {
        self.ip == [0, 0, 0, 0]
    }
}

impl fmt::Display for SocketAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}.{}.{}.{}:{}",
            self.ip[0], self.ip[1], self.ip[2], self.ip[3], self.port
        )
    }
}

/// Circular Ring Buffer for Socket Send / Receive byte streams.
#[derive(Debug)]
pub struct RingBuffer {
    data: Vec<u8>,
    head: usize, // Read position
    tail: usize, // Write position
    count: usize,
    capacity: usize,
}

impl RingBuffer {
    pub const DEFAULT_CAPACITY: usize = 65536; // 64 KB

    pub fn new(capacity: usize) -> Self {
        Self {
            data: vec![0u8; capacity],
            head: 0,
            tail: 0,
            count: 0,
            capacity,
        }
    }

    /// Write bytes into the circular buffer. Returns the number of bytes written.
    pub fn write(&mut self, buf: &[u8]) -> usize {
        let available = self.capacity - self.count;
        let to_write = buf.len().min(available);

        for &b in &buf[..to_write] {
            self.data[self.tail] = b;
            self.tail = (self.tail + 1) % self.capacity;
        }

        self.count += to_write;
        to_write
    }

    /// Read bytes out of the circular buffer. Returns the number of bytes read.
    pub fn read(&mut self, buf: &mut [u8]) -> usize {
        let to_read = buf.len().min(self.count);

        for item in buf.iter_mut().take(to_read) {
            *item = self.data[self.head];
            self.head = (self.head + 1) % self.capacity;
        }

        self.count -= to_read;
        to_read
    }

    /// Peek bytes from the buffer without advancing the read position.
    pub fn peek(&self, buf: &mut [u8]) -> usize {
        let to_peek = buf.len().min(self.count);
        let mut curr = self.head;

        for item in buf.iter_mut().take(to_peek) {
            *item = self.data[curr];
            curr = (curr + 1) % self.capacity;
        }

        to_peek
    }

    #[inline]
    pub fn available_read(&self) -> usize {
        self.count
    }

    #[inline]
    pub fn available_write(&self) -> usize {
        self.capacity - self.count
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    #[inline]
    pub fn is_full(&self) -> bool {
        self.count >= self.capacity
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn clear(&mut self) {
        self.head = 0;
        self.tail = 0;
        self.count = 0;
    }
}

/// Global atomic socket handle allocator.
static NEXT_SOCKET_ID: AtomicUsize = AtomicUsize::new(100);

/// Core BSD Socket Representation.
pub struct Socket {
    pub id: usize,
    pub family: AddressFamily,
    pub sock_type: SocketType,
    pub protocol: SocketProtocol,

    pub state: Mutex<SocketState>,
    pub local_addr: RwLock<Option<SocketAddress>>,
    pub peer_addr: RwLock<Option<SocketAddress>>,
    pub nonblocking: AtomicBool,
    pub owner_pid: AtomicU32,

    /// Stream transmission and reception circular buffers
    pub rx_buffer: Mutex<RingBuffer>,
    pub tx_buffer: Mutex<RingBuffer>,

    /// Queue of incoming UDP datagrams: (payload, source_addr)
    pub dgram_rx_queue: Mutex<VecDeque<(Vec<u8>, SocketAddress)>>,

    /// Queue of incoming stream connections awaiting `accept(2)`
    pub backlog_queue: Mutex<VecDeque<Arc<Socket>>>,

    /// Direct peer socket link (used for stream loopback connection pairs)
    pub peer_socket: Mutex<Option<Weak<Socket>>>,

    /// Optional underlying TCP connection state machine
    pub tcp_conn: Mutex<Option<TcpConnection>>,
}

impl Socket {
    /// Create a new BSD Socket endpoint.
    pub fn new(
        family: AddressFamily,
        sock_type: SocketType,
        protocol: SocketProtocol,
    ) -> Arc<Self> {
        let id = NEXT_SOCKET_ID.fetch_add(1, Ordering::SeqCst);
        let sock = Arc::new(Self {
            id,
            family,
            sock_type,
            protocol,
            state: Mutex::new(SocketState::Unbound),
            local_addr: RwLock::new(None),
            peer_addr: RwLock::new(None),
            nonblocking: AtomicBool::new(false),
            owner_pid: AtomicU32::new(0),
            rx_buffer: Mutex::new(RingBuffer::new(RingBuffer::DEFAULT_CAPACITY)),
            tx_buffer: Mutex::new(RingBuffer::new(RingBuffer::DEFAULT_CAPACITY)),
            dgram_rx_queue: Mutex::new(VecDeque::new()),
            backlog_queue: Mutex::new(VecDeque::new()),
            peer_socket: Mutex::new(None),
            tcp_conn: Mutex::new(None),
        });
        SocketRegistry::global().register_socket(Arc::clone(&sock));
        sock
    }

    /// Create a new BSD Socket endpoint associated with a specific process ID.
    pub fn new_with_pid(
        family: AddressFamily,
        sock_type: SocketType,
        protocol: SocketProtocol,
        owner_pid: u32,
    ) -> Arc<Self> {
        let sock = Self::new(family, sock_type, protocol);
        sock.set_owner_pid(owner_pid);
        sock
    }

    // ========================================================================
    // Configuration & Flags
    // ========================================================================

    #[inline]
    pub fn owner_pid(&self) -> u32 {
        self.owner_pid.load(Ordering::Relaxed)
    }

    #[inline]
    pub fn set_owner_pid(&self, pid: u32) {
        self.owner_pid.store(pid, Ordering::SeqCst);
    }

    #[inline]
    pub fn is_nonblocking(&self) -> bool {
        self.nonblocking.load(Ordering::Relaxed)
    }

    #[inline]
    pub fn set_nonblocking(&self, nonblocking: bool) {
        self.nonblocking.store(nonblocking, Ordering::SeqCst);
    }

    pub fn local_address(&self) -> Option<SocketAddress> {
        *self.local_addr.read().unwrap()
    }

    pub fn peer_address(&self) -> Option<SocketAddress> {
        *self.peer_addr.read().unwrap()
    }

    pub fn current_state(&self) -> SocketState {
        *self.state.lock().unwrap()
    }

    // ========================================================================
    // Section 1: Address Binding (`bind`)
    // ========================================================================

    /// Bind socket to local address.
    pub fn bind(&self, addr: SocketAddress) -> Result<(), SocketError> {
        // Network Filter Hook interception (HyAV M6-1)
        let ctx = NetworkFilterContext::new(
            self.owner_pid(),
            self.protocol,
            NetworkFilterOp::Bind,
            Some(addr),
            None,
        );
        if global_network_filter().evaluate(&ctx) == FilterVerdict::Block {
            return Err(SocketError::ConnectionRefused);
        }

        let mut state = self.state.lock().unwrap();
        if *state != SocketState::Unbound {
            return Err(SocketError::AlreadyBound);
        }

        // Check if port is in use in the global socket registry
        if SocketRegistry::global().is_port_bound(addr.port) {
            return Err(SocketError::AddressInUse);
        }

        *self.local_addr.write().unwrap() = Some(addr);
        *state = SocketState::Bound;
        SocketRegistry::global().register_port(addr.port, self.id);

        Ok(())
    }

    // ========================================================================
    // Section 2: Passive Listening & Connection Acceptance (`listen` / `accept`)
    // ========================================================================

    /// Enter listening state for incoming connections.
    pub fn listen(&self, _backlog: usize) -> Result<(), SocketError> {
        if self.sock_type != SocketType::Stream {
            return Err(SocketError::UnsupportedOperation);
        }

        let mut state = self.state.lock().unwrap();
        if *state != SocketState::Bound {
            return Err(SocketError::NotBound);
        }

        *state = SocketState::Listening;

        // Initialize TCP state machine in LISTEN mode
        if let Some(local_addr) = *self.local_addr.read().unwrap() {
            let tcp = TcpConnection::new_listen(local_addr.ip, local_addr.port, 1000);
            *self.tcp_conn.lock().unwrap() = Some(tcp);
        }

        Ok(())
    }

    /// Accept an incoming connection from the backlog queue.
    pub fn accept(&self) -> Result<(Arc<Socket>, SocketAddress), SocketError> {
        if self.sock_type != SocketType::Stream {
            return Err(SocketError::UnsupportedOperation);
        }

        let state = *self.state.lock().unwrap();
        if state != SocketState::Listening {
            return Err(SocketError::NotListening);
        }

        let mut backlog = self.backlog_queue.lock().unwrap();
        if let Some(accepted) = backlog.pop_front() {
            let peer = accepted.peer_address().ok_or(SocketError::NotConnected)?;

            // Network Filter Hook interception (HyAV M6-1)
            let local = self.local_address();
            let ctx = NetworkFilterContext::new(
                self.owner_pid(),
                self.protocol,
                NetworkFilterOp::Accept,
                local,
                Some(peer),
            );
            if global_network_filter().evaluate(&ctx) == FilterVerdict::Block {
                return Err(SocketError::ConnectionRefused);
            }

            Ok((accepted, peer))
        } else if self.is_nonblocking() {
            Err(SocketError::WouldBlock)
        } else {
            // In a blocking real OS this sleeps on a wait queue; in our nonblocking engine we return WouldBlock
            Err(SocketError::WouldBlock)
        }
    }

    // ========================================================================
    // Section 3: Connection Initiation (`connect`)
    // ========================================================================

    /// Connect to remote peer address.
    pub fn connect(self: &Arc<Self>, remote: SocketAddress) -> Result<(), SocketError> {
        let local = self.local_address();
        // Network Filter Hook interception (HyAV M6-1)
        let ctx = NetworkFilterContext::new(
            self.owner_pid(),
            self.protocol,
            NetworkFilterOp::Connect,
            local,
            Some(remote),
        );
        if global_network_filter().evaluate(&ctx) == FilterVerdict::Block {
            return Err(SocketError::ConnectionRefused);
        }

        let mut state = self.state.lock().unwrap();
        if *state == SocketState::Connected {
            return Err(SocketError::AlreadyConnected);
        }

        // Auto-bind to ephemeral port if currently unbound
        if *state == SocketState::Unbound {
            let ephemeral = SocketRegistry::global().allocate_ephemeral_port()?;
            *self.local_addr.write().unwrap() = Some(SocketAddress::loopback(ephemeral));
            SocketRegistry::global().register_port(ephemeral, self.id);
            *state = SocketState::Bound;
        }

        *self.peer_addr.write().unwrap() = Some(remote);

        if self.sock_type == SocketType::Stream {
            // Local loopback connection resolution
            if remote.is_loopback() {
                if let Some(server_socket) = SocketRegistry::global().get_socket_by_port(remote.port) {
                    if server_socket.current_state() != SocketState::Listening {
                        return Err(SocketError::ConnectionRefused);
                    }

                    // Create server-side endpoint for accepted connection
                    let local_addr = self.local_addr.read().unwrap().unwrap();
                    let server_peer = Socket::new(self.family, self.sock_type, self.protocol);
                    *server_peer.local_addr.write().unwrap() = Some(remote);
                    *server_peer.peer_addr.write().unwrap() = Some(local_addr);
                    *server_peer.state.lock().unwrap() = SocketState::Connected;

                    // Cross-link peer sockets for direct bidirectional stream transfer
                    *self.peer_socket.lock().unwrap() = Some(Arc::downgrade(&server_peer));
                    *server_peer.peer_socket.lock().unwrap() = Some(Arc::downgrade(self));

                    // Enqueue accepted socket into server's backlog queue
                    server_socket.backlog_queue.lock().unwrap().push_back(server_peer);
                    *state = SocketState::Connected;

                    return Ok(());
                } else {
                    return Err(SocketError::ConnectionRefused);
                }
            }

            *state = SocketState::Connected;
        } else {
            // Datagram / Raw sockets simply associate peer address
            *state = SocketState::Connected;
        }

        Ok(())
    }

    // ========================================================================
    // Section 4: Stream Data Transmission (`send` / `recv`)
    // ========================================================================

    /// Send data through connected stream socket.
    pub fn send(&self, buf: &[u8]) -> Result<usize, SocketError> {
        if self.sock_type != SocketType::Stream {
            return Err(SocketError::UnsupportedOperation);
        }

        let state = *self.state.lock().unwrap();
        if state != SocketState::Connected {
            return Err(SocketError::NotConnected);
        }

        // Direct transfer to connected peer's RX buffer if loopback pair
        if let Some(ref weak_peer) = *self.peer_socket.lock().unwrap() {
            if let Some(peer) = weak_peer.upgrade() {
                let mut peer_rx = peer.rx_buffer.lock().unwrap();
                if peer_rx.available_write() == 0 {
                    if self.is_nonblocking() {
                        return Err(SocketError::WouldBlock);
                    }
                    return Err(SocketError::BufferOverflow);
                }
                let written = peer_rx.write(buf);
                return Ok(written);
            } else {
                return Err(SocketError::ConnectionReset);
            }
        }

        // Otherwise write to local TX buffer
        let mut tx = self.tx_buffer.lock().unwrap();
        if tx.available_write() == 0 {
            if self.is_nonblocking() {
                return Err(SocketError::WouldBlock);
            }
            return Err(SocketError::BufferOverflow);
        }

        let written = tx.write(buf);
        Ok(written)
    }

    /// Receive data from connected stream socket.
    pub fn recv(&self, buf: &mut [u8]) -> Result<usize, SocketError> {
        if self.sock_type != SocketType::Stream {
            return Err(SocketError::UnsupportedOperation);
        }

        let state = *self.state.lock().unwrap();
        if state != SocketState::Connected && state != SocketState::Closing {
            return Err(SocketError::NotConnected);
        }

        let mut rx = self.rx_buffer.lock().unwrap();
        if rx.is_empty() {
            if state == SocketState::Closed || state == SocketState::Closing {
                return Ok(0); // EOF
            }
            if self.is_nonblocking() {
                return Err(SocketError::WouldBlock);
            }
            return Err(SocketError::WouldBlock);
        }

        let read = rx.read(buf);
        Ok(read)
    }

    // ========================================================================
    // Section 5: Datagram Data Transmission (`sendto` / `recvfrom`)
    // ========================================================================

    /// Transmit datagram to specified destination address.
    pub fn sendto(&self, data: &[u8], target: SocketAddress) -> Result<usize, SocketError> {
        if self.sock_type != SocketType::Dgram {
            return Err(SocketError::UnsupportedOperation);
        }

        let mut state = self.state.lock().unwrap();
        if *state == SocketState::Unbound {
            let ephemeral = SocketRegistry::global().allocate_ephemeral_port()?;
            *self.local_addr.write().unwrap() = Some(SocketAddress::loopback(ephemeral));
            SocketRegistry::global().register_port(ephemeral, self.id);
            *state = SocketState::Bound;
        }

        let local_addr = self.local_addr.read().unwrap().unwrap();

        // Local loopback delivery: route directly to target socket's dgram queue
        if target.is_loopback() {
            if let Some(target_sock) = SocketRegistry::global().get_socket_by_port(target.port) {
                let mut q = target_sock.dgram_rx_queue.lock().unwrap();
                if q.len() >= 256 {
                    if self.is_nonblocking() {
                        return Err(SocketError::WouldBlock);
                    }
                    return Err(SocketError::BufferOverflow);
                }
                q.push_back((data.to_vec(), local_addr));
                return Ok(data.len());
            } else {
                return Err(SocketError::ConnectionRefused);
            }
        }

        // Non-loopback: forward to UDP Subsystem
        let datagram = UdpDatagram::new(
            local_addr.ip,
            target.ip,
            local_addr.port,
            target.port,
            data.to_vec(),
        );
        let sub = UdpSubsystem::new();
        sub.send(datagram).map_err(|_| SocketError::ConnectionRefused)?;

        Ok(data.len())
    }

    /// Receive next datagram and return source endpoint address.
    pub fn recvfrom(&self, buf: &mut [u8]) -> Result<(usize, SocketAddress), SocketError> {
        if self.sock_type != SocketType::Dgram {
            return Err(SocketError::UnsupportedOperation);
        }

        let mut q = self.dgram_rx_queue.lock().unwrap();
        if let Some((payload, src_addr)) = q.pop_front() {
            let copy_len = buf.len().min(payload.len());
            buf[..copy_len].copy_from_slice(&payload[..copy_len]);
            Ok((copy_len, src_addr))
        } else if self.is_nonblocking() {
            Err(SocketError::WouldBlock)
        } else {
            Err(SocketError::WouldBlock)
        }
    }

    // ========================================================================
    // Section 6: Socket Shutdown & Teardown
    // ========================================================================

    /// Shutdown read and/or write channels.
    pub fn shutdown(&self, _how: ShutdownHow) -> Result<(), SocketError> {
        let mut state = self.state.lock().unwrap();
        if *state != SocketState::Connected {
            return Err(SocketError::NotConnected);
        }

        *state = SocketState::Closing;
        Ok(())
    }

    /// Close the socket and unregister bound ports.
    pub fn close(&self) -> Result<(), SocketError> {
        let mut state = self.state.lock().unwrap();
        if *state == SocketState::Closed {
            return Ok(());
        }

        *state = SocketState::Closed;

        if let Some(addr) = *self.local_addr.read().unwrap() {
            SocketRegistry::global().unregister_port(addr.port);
        }
        SocketRegistry::global().unregister_socket(self.id);

        // Notify peer if linked
        if let Some(ref weak_peer) = *self.peer_socket.lock().unwrap() {
            if let Some(peer) = weak_peer.upgrade() {
                let mut peer_state = peer.state.lock().unwrap();
                if *peer_state == SocketState::Connected {
                    *peer_state = SocketState::Closing;
                }
            }
        }

        Ok(())
    }

    // ========================================================================
    // Section 7: HybridOS VFS Inode & File Descriptor Bridge
    // ========================================================================

    /// Convert this socket into a VFS [`Inode`].
    pub fn into_inode(self: &Arc<Self>) -> Arc<Inode> {
        let ops = Arc::new(SocketFileOps {
            socket: Arc::clone(self),
        });

        Arc::new(Inode::new(
            self.id as u64,
            InodeType::Socket,
            Permissions::from_mode(0o666),
            ops,
        ))
    }

    /// Bridge socket directly into an open VFS [`FileDescriptor`].
    pub fn into_file_descriptor(self: &Arc<Self>, fd_id: usize) -> Arc<FileDescriptor> {
        let inode = self.into_inode();
        let path = format!("socket:[{}]", self.id);
        Arc::new(FileDescriptor::new(fd_id, inode, OpenFlags::read_write(), path))
    }
}

/// Standard Socket IOCTL Commands (POSIX / Winsock).
pub const FIONBIO: u32 = 0x5421;
pub const FIONREAD: u32 = 0x541B;

/// VFS File Operations Implementation bridging BSD Socket into File Descriptors.
pub struct SocketFileOps {
    pub socket: Arc<Socket>,
}

impl FileOperations for SocketFileOps {
    /// Read from socket stream via standard VFS `read(2)`.
    fn read(&self, _offset: u64, buf: &mut [u8]) -> Result<usize, VfsError> {
        match self.socket.sock_type {
            SocketType::Stream => self.socket.recv(buf).map_err(VfsError::from),
            SocketType::Dgram => {
                let (bytes, _) = self.socket.recvfrom(buf).map_err(VfsError::from)?;
                Ok(bytes)
            }
            SocketType::Raw => Err(VfsError::NotSupported),
        }
    }

    /// Write to socket stream via standard VFS `write(2)`.
    fn write(&self, _offset: u64, buf: &[u8]) -> Result<usize, VfsError> {
        match self.socket.sock_type {
            SocketType::Stream => self.socket.send(buf).map_err(VfsError::from),
            SocketType::Dgram => {
                // If connected, send to peer address
                if let Some(peer) = self.socket.peer_address() {
                    self.socket.sendto(buf, peer).map_err(VfsError::from)
                } else {
                    Err(VfsError::InvalidArgument)
                }
            }
            SocketType::Raw => Err(VfsError::NotSupported),
        }
    }

    /// Socket IOCTL driver supporting `FIONBIO` (non-blocking) and `FIONREAD` (query bytes).
    fn ioctl(&self, cmd: u32, arg: usize) -> Result<usize, VfsError> {
        match cmd {
            FIONBIO => {
                let nonblock = arg != 0;
                self.socket.set_nonblocking(nonblock);
                Ok(0)
            }
            FIONREAD => {
                let available = match self.socket.sock_type {
                    SocketType::Stream => self.socket.rx_buffer.lock().unwrap().available_read(),
                    SocketType::Dgram => {
                        let q = self.socket.dgram_rx_queue.lock().unwrap();
                        q.front().map(|(p, _)| p.len()).unwrap_or(0)
                    }
                    _ => 0,
                };
                Ok(available)
            }
            _ => Err(VfsError::NotSupported),
        }
    }

    fn sync(&self) -> Result<(), VfsError> {
        Ok(())
    }
}

/// Global Registry tracking allocated sockets and bound ports across the OS.
pub struct SocketRegistry {
    sockets: RwLock<std::collections::HashMap<usize, Arc<Socket>>>,
    port_map: RwLock<std::collections::HashMap<u16, usize>>,
    next_ephemeral: Mutex<u16>,
}

impl SocketRegistry {
    pub fn global() -> &'static SocketRegistry {
        static INSTANCE: std::sync::OnceLock<SocketRegistry> = std::sync::OnceLock::new();
        INSTANCE.get_or_init(|| SocketRegistry {
            sockets: RwLock::new(std::collections::HashMap::new()),
            port_map: RwLock::new(std::collections::HashMap::new()),
            next_ephemeral: Mutex::new(super::udp::EPHEMERAL_PORT_START),
        })
    }

    pub fn register_socket(&self, sock: Arc<Socket>) {
        self.sockets.write().unwrap().insert(sock.id, sock);
    }

    pub fn unregister_socket(&self, id: usize) {
        self.sockets.write().unwrap().remove(&id);
    }

    pub fn get_socket(&self, id: usize) -> Option<Arc<Socket>> {
        self.sockets.read().unwrap().get(&id).cloned()
    }

    pub fn register_port(&self, port: u16, sock_id: usize) {
        self.port_map.write().unwrap().insert(port, sock_id);
    }

    pub fn unregister_port(&self, port: u16) {
        self.port_map.write().unwrap().remove(&port);
    }

    pub fn is_port_bound(&self, port: u16) -> bool {
        self.port_map.read().unwrap().contains_key(&port)
    }

    pub fn get_socket_by_port(&self, port: u16) -> Option<Arc<Socket>> {
        let sock_id = *self.port_map.read().unwrap().get(&port)?;
        self.get_socket(sock_id)
    }

    pub fn allocate_ephemeral_port(&self) -> Result<u16, SocketError> {
        let mut next = self.next_ephemeral.lock().unwrap();
        let start = super::udp::EPHEMERAL_PORT_START;
        let end = super::udp::EPHEMERAL_PORT_END;

        for _ in 0..(end - start + 1) {
            let port = *next;
            if *next >= end {
                *next = start;
            } else {
                *next += 1;
            }

            if !self.is_port_bound(port) {
                return Ok(port);
            }
        }

        Err(SocketError::AddressInUse)
    }
}
