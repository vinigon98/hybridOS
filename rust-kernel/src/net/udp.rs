//! User Datagram Protocol (UDP) Implementation (RFC 768)
//!
//! Provides the core transport layer abstractions for connectionless,
//! unreliable datagram delivery across HybridOS:
//! - [`UdpHeader`]: UDP fixed-size 8-byte header representation.
//! - [`UdpDatagram`]: Datagram container with IPv4 addressing and payload.
//! - [`compute_udp_checksum`] & [`verify_udp_checksum`]: RFC 768 IPv4 pseudo-header checksum.
//! - [`PortDemuxTable`]: Port binding, multiplexing, and demultiplexing.
//! - [`UdpQueue`]: Thread-safe reception and transmission datagram queues.
//! - [`UdpSubsystem`]: Unified transport coordinator supporting loopback and routing.

use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::sync::{Arc, Mutex, RwLock};

/// Standard UDP header length in bytes.
pub const UDP_HEADER_LEN: usize = 8;

/// IPv4 UDP Protocol Number.
pub const IP_PROTOCOL_UDP: u8 = 17;

/// Range for ephemeral port allocation.
pub const EPHEMERAL_PORT_START: u16 = 49152;
pub const EPHEMERAL_PORT_END: u16 = 65535;

/// UDP Specific Errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UdpError {
    PacketTooShort,
    InvalidLength,
    InvalidChecksum,
    PortAlreadyBound,
    PortNotBound,
    NoEphemeralPortAvailable,
    QueueFull,
    QueueEmpty,
    BufferTooSmall,
    NetworkUnreachable,
}

impl fmt::Display for UdpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UdpError::PacketTooShort => write!(f, "UDP packet is shorter than 8-byte header"),
            UdpError::InvalidLength => write!(f, "UDP header length field does not match payload length"),
            UdpError::InvalidChecksum => write!(f, "UDP pseudo-header checksum verification failed"),
            UdpError::PortAlreadyBound => write!(f, "Port is already bound by another socket/endpoint"),
            UdpError::PortNotBound => write!(f, "Port is not currently bound"),
            UdpError::NoEphemeralPortAvailable => write!(f, "All ephemeral ports (49152-65535) are in use"),
            UdpError::QueueFull => write!(f, "UDP datagram queue capacity exceeded"),
            UdpError::QueueEmpty => write!(f, "UDP datagram queue is empty"),
            UdpError::BufferTooSmall => write!(f, "Destination buffer too small to receive datagram"),
            UdpError::NetworkUnreachable => write!(f, "Destination IP address is unreachable"),
        }
    }
}

impl std::error::Error for UdpError {}

/// Standard 8-byte UDP header.
///
/// ```text
///  0      7 8     15 16    23 24    31
/// +--------+--------+--------+--------+
/// |     Source      |   Destination   |
/// |      Port       |      Port       |
/// +--------+--------+--------+--------+
/// |                 |                 |
/// |     Length      |    Checksum     |
/// +--------+--------+--------+--------+
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UdpHeader {
    pub src_port: u16,
    pub dst_port: u16,
    pub length: u16,
    pub checksum: u16,
}

impl UdpHeader {
    /// Construct a new UDP Header.
    #[inline]
    pub const fn new(src_port: u16, dst_port: u16, payload_len: usize) -> Self {
        Self {
            src_port,
            dst_port,
            length: (UDP_HEADER_LEN + payload_len) as u16,
            checksum: 0,
        }
    }

    /// Serialize header to 8 raw bytes (big-endian/network byte order).
    pub fn to_bytes(&self) -> [u8; UDP_HEADER_LEN] {
        let mut bytes = [0u8; UDP_HEADER_LEN];
        bytes[0..2].copy_from_slice(&self.src_port.to_be_bytes());
        bytes[2..4].copy_from_slice(&self.dst_port.to_be_bytes());
        bytes[4..6].copy_from_slice(&self.length.to_be_bytes());
        bytes[6..8].copy_from_slice(&self.checksum.to_be_bytes());
        bytes
    }

    /// Parse header from byte slice.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, UdpError> {
        if bytes.len() < UDP_HEADER_LEN {
            return Err(UdpError::PacketTooShort);
        }

        let src_port = u16::from_be_bytes([bytes[0], bytes[1]]);
        let dst_port = u16::from_be_bytes([bytes[2], bytes[3]]);
        let length = u16::from_be_bytes([bytes[4], bytes[5]]);
        let checksum = u16::from_be_bytes([bytes[6], bytes[7]]);

        if (length as usize) < UDP_HEADER_LEN {
            return Err(UdpError::InvalidLength);
        }

        Ok(Self {
            src_port,
            dst_port,
            length,
            checksum,
        })
    }
}

/// Compute RFC 768 / RFC 1071 16-bit one's complement sum.
fn fold_checksum(mut sum: u32) -> u16 {
    while (sum >> 16) != 0 {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }
    let folded = !(sum as u16);
    // RFC 768: If computed checksum is 0, it is transmitted as 0xFFFF.
    if folded == 0 {
        0xFFFF
    } else {
        folded
    }
}

/// Compute RFC 768 UDP Checksum over IPv4 Pseudo-Header, UDP Header, and Payload.
///
/// IPv4 Pseudo-Header layout:
/// - Source IPv4: 4 bytes
/// - Destination IPv4: 4 bytes
/// - Zero: 1 byte
/// - Protocol: 1 byte (17)
/// - UDP Length: 2 bytes
pub fn compute_udp_checksum(
    src_ip: [u8; 4],
    dst_ip: [u8; 4],
    header: &UdpHeader,
    payload: &[u8],
) -> u16 {
    let mut sum: u32 = 0;

    // 1. Pseudo-header: Source IP (2 words)
    sum += u16::from_be_bytes([src_ip[0], src_ip[1]]) as u32;
    sum += u16::from_be_bytes([src_ip[2], src_ip[3]]) as u32;

    // 2. Pseudo-header: Destination IP (2 words)
    sum += u16::from_be_bytes([dst_ip[0], dst_ip[1]]) as u32;
    sum += u16::from_be_bytes([dst_ip[2], dst_ip[3]]) as u32;

    // 3. Pseudo-header: Zero (8 bits) + Protocol (8 bits = 17)
    sum += IP_PROTOCOL_UDP as u32;

    // 4. Pseudo-header: UDP Length
    sum += header.length as u32;

    // 5. UDP Header fields (with checksum treated as 0)
    sum += header.src_port as u32;
    sum += header.dst_port as u32;
    sum += header.length as u32;
    // Checksum field itself is treated as 0 during computation

    // 6. Payload words
    let mut i = 0;
    while i + 1 < payload.len() {
        sum += u16::from_be_bytes([payload[i], payload[i + 1]]) as u32;
        i += 2;
    }
    // Odd byte padding
    if i < payload.len() {
        sum += u16::from_be_bytes([payload[i], 0x00]) as u32;
    }

    fold_checksum(sum)
}

/// Verify UDP packet checksum.
///
/// If header checksum is 0x0000, checksum calculation was omitted by sender (valid per RFC 768).
pub fn verify_udp_checksum(
    src_ip: [u8; 4],
    dst_ip: [u8; 4],
    header: &UdpHeader,
    payload: &[u8],
) -> bool {
    if header.checksum == 0 {
        return true; // Checksum disabled/omitted
    }

    let calculated = compute_udp_checksum(src_ip, dst_ip, header, payload);
    calculated == header.checksum
}

/// Complete UDP Datagram combining IP endpoints, ports, header, and payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UdpDatagram {
    pub src_ip: [u8; 4],
    pub dst_ip: [u8; 4],
    pub src_port: u16,
    pub dst_port: u16,
    pub data: Vec<u8>,
}

impl UdpDatagram {
    /// Construct a new datagram.
    pub fn new(
        src_ip: [u8; 4],
        dst_ip: [u8; 4],
        src_port: u16,
        dst_port: u16,
        data: Vec<u8>,
    ) -> Self {
        Self {
            src_ip,
            dst_ip,
            src_port,
            dst_port,
            data,
        }
    }

    /// Total serialized UDP segment size (header + data).
    #[inline]
    pub fn len(&self) -> usize {
        UDP_HEADER_LEN + self.data.len()
    }

    /// Check if datagram has zero payload.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Serialize into a raw UDP packet (header with valid checksum + payload).
    pub fn serialize(&self) -> Vec<u8> {
        let mut header = UdpHeader::new(self.src_port, self.dst_port, self.data.len());
        header.checksum = compute_udp_checksum(self.src_ip, self.dst_ip, &header, &self.data);

        let mut packet = Vec::with_capacity(self.len());
        packet.extend_from_slice(&header.to_bytes());
        packet.extend_from_slice(&self.data);
        packet
    }

    /// Parse a raw UDP packet with checksum verification.
    pub fn parse(
        src_ip: [u8; 4],
        dst_ip: [u8; 4],
        raw_packet: &[u8],
    ) -> Result<Self, UdpError> {
        let header = UdpHeader::from_bytes(raw_packet)?;
        let total_len = header.length as usize;

        if raw_packet.len() < total_len {
            return Err(UdpError::PacketTooShort);
        }

        let payload = &raw_packet[UDP_HEADER_LEN..total_len];

        if !verify_udp_checksum(src_ip, dst_ip, &header, payload) {
            return Err(UdpError::InvalidChecksum);
        }

        Ok(Self {
            src_ip,
            dst_ip,
            src_port: header.src_port,
            dst_port: header.dst_port,
            data: payload.to_vec(),
        })
    }
}

/// Thread-safe bounded datagram queue.
#[derive(Debug)]
pub struct UdpQueue {
    queue: Mutex<VecDeque<UdpDatagram>>,
    capacity: usize,
}

impl UdpQueue {
    pub const DEFAULT_CAPACITY: usize = 256;

    pub fn new(capacity: usize) -> Self {
        Self {
            queue: Mutex::new(VecDeque::with_capacity(capacity.min(64))),
            capacity,
        }
    }

    /// Push a datagram into the queue. Returns `Err(UdpError::QueueFull)` if at capacity.
    pub fn push(&self, datagram: UdpDatagram) -> Result<(), UdpError> {
        let mut q = self.queue.lock().unwrap();
        if q.len() >= self.capacity {
            return Err(UdpError::QueueFull);
        }
        q.push_back(datagram);
        Ok(())
    }

    /// Pop the next available datagram from the queue.
    pub fn pop(&self) -> Option<UdpDatagram> {
        let mut q = self.queue.lock().unwrap();
        q.pop_front()
    }

    /// Peek the next datagram without removing it.
    pub fn peek(&self) -> Option<UdpDatagram> {
        let q = self.queue.lock().unwrap();
        q.front().cloned()
    }

    /// Number of queued datagrams.
    pub fn len(&self) -> usize {
        let q = self.queue.lock().unwrap();
        q.len()
    }

    /// Check if queue is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Clear all pending datagrams.
    pub fn clear(&self) {
        let mut q = self.queue.lock().unwrap();
        q.clear();
    }
}

/// Port Demultiplexing and Binding Table.
///
/// Maps UDP port numbers to reception queues. Handles ephemeral port
/// allocation for client sockets.
#[derive(Debug)]
pub struct PortDemuxTable {
    bindings: RwLock<HashMap<u16, Arc<UdpQueue>>>,
    next_ephemeral: Mutex<u16>,
}

impl PortDemuxTable {
    pub fn new() -> Self {
        Self {
            bindings: RwLock::new(HashMap::new()),
            next_ephemeral: Mutex::new(EPHEMERAL_PORT_START),
        }
    }

    /// Bind a specific port to a reception queue.
    pub fn bind(&self, port: u16, queue: Arc<UdpQueue>) -> Result<(), UdpError> {
        let mut map = self.bindings.write().unwrap();
        if map.contains_key(&port) {
            return Err(UdpError::PortAlreadyBound);
        }
        map.insert(port, queue);
        Ok(())
    }

    /// Unbind a port.
    pub fn unbind(&self, port: u16) -> Result<(), UdpError> {
        let mut map = self.bindings.write().unwrap();
        if map.remove(&port).is_some() {
            Ok(())
        } else {
            Err(UdpError::PortNotBound)
        }
    }

    /// Allocate an available ephemeral port (49152..65535) and bind the queue.
    pub fn bind_ephemeral(&self, queue: Arc<UdpQueue>) -> Result<u16, UdpError> {
        let mut map = self.bindings.write().unwrap();
        let mut next_port = self.next_ephemeral.lock().unwrap();

        let total_ports = (EPHEMERAL_PORT_END - EPHEMERAL_PORT_START + 1) as usize;
        for _ in 0..total_ports {
            let port = *next_port;
            if *next_port >= EPHEMERAL_PORT_END {
                *next_port = EPHEMERAL_PORT_START;
            } else {
                *next_port += 1;
            }

            if !map.contains_key(&port) {
                map.insert(port, queue);
                return Ok(port);
            }
        }

        Err(UdpError::NoEphemeralPortAvailable)
    }

    /// Check if a port is bound.
    pub fn is_bound(&self, port: u16) -> bool {
        let map = self.bindings.read().unwrap();
        map.contains_key(&port)
    }

    /// Retrieve the reception queue bound to a given destination port.
    pub fn get_queue(&self, port: u16) -> Option<Arc<UdpQueue>> {
        let map = self.bindings.read().unwrap();
        map.get(&port).cloned()
    }

    /// Dispatch an incoming datagram to its destination port queue.
    pub fn demux(&self, datagram: UdpDatagram) -> Result<(), UdpError> {
        let queue = self.get_queue(datagram.dst_port).ok_or(UdpError::PortNotBound)?;
        queue.push(datagram)
    }
}

/// Unified UDP Transport Subsystem.
///
/// Orchestrates port multiplexing/demultiplexing, TX queues, RX routing,
/// and local loopback (127.0.0.0/8).
pub struct UdpSubsystem {
    pub demux: Arc<PortDemuxTable>,
    pub tx_queue: Arc<UdpQueue>,
}

impl UdpSubsystem {
    pub fn new() -> Self {
        Self {
            demux: Arc::new(PortDemuxTable::new()),
            tx_queue: Arc::new(UdpQueue::new(UdpQueue::DEFAULT_CAPACITY)),
        }
    }

    /// Determine if an IPv4 address is loopback (127.0.0.0/8).
    #[inline]
    pub fn is_loopback(ip: [u8; 4]) -> bool {
        ip[0] == 127
    }

    /// Transmit a datagram.
    ///
    /// If the destination is loopback, it is directly routed to the destination port's
    /// RX queue. Otherwise, it is enqueued into the subsystem's TX queue for physical
    /// transmission by the network driver.
    pub fn send(&self, datagram: UdpDatagram) -> Result<(), UdpError> {
        if Self::is_loopback(datagram.dst_ip) {
            // Immediate local loopback delivery
            self.demux.demux(datagram)
        } else {
            // Queue for lower layer (IP/VirtIO) transmission
            self.tx_queue.push(datagram)
        }
    }

    /// Process a raw received UDP frame from network hardware.
    pub fn receive_raw(
        &self,
        src_ip: [u8; 4],
        dst_ip: [u8; 4],
        raw_packet: &[u8],
    ) -> Result<(), UdpError> {
        let datagram = UdpDatagram::parse(src_ip, dst_ip, raw_packet)?;
        self.demux.demux(datagram)
    }
}
