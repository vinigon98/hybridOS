//! Transmission Control Protocol (TCP) Implementation (RFC 793 / RFC 1122)
//!
//! Provides the transport layer abstractions for connection-oriented, reliable,
//! ordered, and flow-controlled byte-stream delivery:
//! - Full TCP State Machine (RFC 793)
//! - TCP Header Flags, Serialization, Deserialization & Pseudo-Header Checksum
//! - Sequence & Acknowledgment modular 32-bit arithmetic
//! - Sliding Window Buffers & Window Size Advertisement
//! - Active / Passive Connection Establishment (3-Way Handshake)
//! - Graceful Teardown & Reset (RST) Handling

#[allow(unused_imports)]
use alloc::string::{String, ToString};
#[allow(unused_imports)]
use alloc::vec::Vec;
#[allow(unused_imports)]
use alloc::boxed::Box;
#[allow(unused_imports)]
use crate::compat::FloatExt;
use std::collections::{BTreeMap, VecDeque};
use std::fmt;

/// Standard IPv4 TCP Protocol Number.
pub const IP_PROTOCOL_TCP: u8 = 6;

/// Minimum TCP Header Length (without options) in bytes.
pub const TCP_HEADER_MIN_LEN: usize = 20;

/// Default Maximum Segment Size (MSS) for IPv4 over Ethernet.
pub const TCP_DEFAULT_MSS: usize = 1460;

/// Default Sliding Window Buffer Capacity in bytes.
pub const TCP_DEFAULT_WINDOW_CAPACITY: usize = 65535;

/// TCP Protocol Error Codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TcpError {
    InvalidStateTransition,
    PacketTooShort,
    InvalidDataOffset,
    InvalidChecksum,
    ConnectionReset,
    ConnectionAborted,
    ConnectionRefused,
    BrokenPipe,
    BufferFull,
    InvalidSequence,
    WindowOverflow,
}

impl fmt::Display for TcpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TcpError::InvalidStateTransition => write!(f, "Invalid TCP state transition"),
            TcpError::PacketTooShort => write!(f, "TCP segment is shorter than 20-byte minimum header"),
            TcpError::InvalidDataOffset => write!(f, "Invalid TCP header data offset"),
            TcpError::InvalidChecksum => write!(f, "TCP pseudo-header checksum verification failed"),
            TcpError::ConnectionReset => write!(f, "Connection reset by peer (RST)"),
            TcpError::ConnectionAborted => write!(f, "Connection aborted by network software"),
            TcpError::ConnectionRefused => write!(f, "Connection refused by destination port"),
            TcpError::BrokenPipe => write!(f, "Broken pipe: attempt to write to closed connection"),
            TcpError::BufferFull => write!(f, "TCP transmission or receive buffer is full"),
            TcpError::InvalidSequence => write!(f, "Received segment outside acceptable sequence window"),
            TcpError::WindowOverflow => write!(f, "Exceeded advertised TCP receive window"),
        }
    }
}

impl std::error::Error for TcpError {}

/// Full RFC 793 TCP Connection States.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TcpState {
    /// Initial pseudo-state when no connection exists.
    Closed,
    /// Waiting for a connection request from any remote TCP and port.
    Listen,
    /// Waiting for a matching connection request after having sent a connection request.
    SynSent,
    /// Waiting for a confirming connection request acknowledgment after having both received and sent a connection request.
    SynRcvd,
    /// An open connection, data received can be delivered to the application.
    Established,
    /// Waiting for a connection termination request from remote TCP, or acknowledgment of the connection termination request sent.
    FinWait1,
    /// Waiting for a connection termination request from the remote TCP.
    FinWait2,
    /// Waiting for a connection termination request from the local application.
    CloseWait,
    /// Waiting for a connection termination request acknowledgment from the remote TCP.
    Closing,
    /// Waiting for an acknowledgment of the connection termination request previously sent to the remote TCP.
    LastAck,
    /// Waiting for enough time to pass (2MSL) to ensure remote TCP received the acknowledgment of its connection termination request.
    TimeWait,
}

impl TcpState {
    /// Whether this state can send payload application data.
    #[inline]
    pub fn can_send(&self) -> bool {
        matches!(self, TcpState::Established | TcpState::CloseWait)
    }

    /// Whether this state can receive incoming payload data.
    #[inline]
    pub fn can_receive(&self) -> bool {
        matches!(self, TcpState::Established | TcpState::FinWait1 | TcpState::FinWait2)
    }

    /// Whether the connection is active and fully established.
    #[inline]
    pub fn is_established(&self) -> bool {
        matches!(self, TcpState::Established)
    }

    /// Whether the state is considered completely closed or finalized.
    #[inline]
    pub fn is_closed(&self) -> bool {
        matches!(self, TcpState::Closed | TcpState::TimeWait)
    }
}

impl fmt::Display for TcpState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TcpState::Closed => write!(f, "CLOSED"),
            TcpState::Listen => write!(f, "LISTEN"),
            TcpState::SynSent => write!(f, "SYN_SENT"),
            TcpState::SynRcvd => write!(f, "SYN_RCVD"),
            TcpState::Established => write!(f, "ESTABLISHED"),
            TcpState::FinWait1 => write!(f, "FIN_WAIT_1"),
            TcpState::FinWait2 => write!(f, "FIN_WAIT_2"),
            TcpState::CloseWait => write!(f, "CLOSE_WAIT"),
            TcpState::Closing => write!(f, "CLOSING"),
            TcpState::LastAck => write!(f, "LAST_ACK"),
            TcpState::TimeWait => write!(f, "TIME_WAIT"),
        }
    }
}

/// TCP 6-bit control flags (RFC 793).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TcpFlags(pub u8);

impl TcpFlags {
    pub const FIN: u8 = 0x01; // No more data from sender
    pub const SYN: u8 = 0x02; // Synchronize sequence numbers
    pub const RST: u8 = 0x04; // Reset the connection
    pub const PSH: u8 = 0x08; // Push function
    pub const ACK: u8 = 0x10; // Acknowledgment field is significant
    pub const URG: u8 = 0x20; // Urgent pointer field is significant

    #[inline]
    pub const fn empty() -> Self {
        Self(0)
    }

    #[inline]
    pub const fn from_bits(bits: u8) -> Self {
        Self(bits & 0x3F)
    }

    #[inline]
    pub const fn bits(&self) -> u8 {
        self.0
    }

    #[inline]
    pub const fn contains(&self, flag: u8) -> bool {
        (self.0 & flag) != 0
    }

    #[inline]
    pub fn set(&mut self, flag: u8, val: bool) {
        if val {
            self.0 |= flag;
        } else {
            self.0 &= !flag;
        }
    }

    // Flag Queries
    #[inline]
    pub const fn is_fin(&self) -> bool {
        (self.0 & Self::FIN) != 0
    }

    #[inline]
    pub const fn is_syn(&self) -> bool {
        (self.0 & Self::SYN) != 0
    }

    #[inline]
    pub const fn is_rst(&self) -> bool {
        (self.0 & Self::RST) != 0
    }

    #[inline]
    pub const fn is_psh(&self) -> bool {
        (self.0 & Self::PSH) != 0
    }

    #[inline]
    pub const fn is_ack(&self) -> bool {
        (self.0 & Self::ACK) != 0
    }

    #[inline]
    pub const fn is_urg(&self) -> bool {
        (self.0 & Self::URG) != 0
    }

    // Common Flag Constructors
    #[inline]
    pub const fn syn() -> Self {
        Self(Self::SYN)
    }

    #[inline]
    pub const fn ack() -> Self {
        Self(Self::ACK)
    }

    #[inline]
    pub const fn syn_ack() -> Self {
        Self(Self::SYN | Self::ACK)
    }

    #[inline]
    pub const fn fin() -> Self {
        Self(Self::FIN)
    }

    #[inline]
    pub const fn fin_ack() -> Self {
        Self(Self::FIN | Self::ACK)
    }

    #[inline]
    pub const fn rst() -> Self {
        Self(Self::RST)
    }

    #[inline]
    pub const fn rst_ack() -> Self {
        Self(Self::RST | Self::ACK)
    }

    #[inline]
    pub const fn psh_ack() -> Self {
        Self(Self::PSH | Self::ACK)
    }
}

impl fmt::Display for TcpFlags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut flags = Vec::new();
        if self.is_syn() { flags.push("SYN"); }
        if self.is_ack() { flags.push("ACK"); }
        if self.is_fin() { flags.push("FIN"); }
        if self.is_rst() { flags.push("RST"); }
        if self.is_psh() { flags.push("PSH"); }
        if self.is_urg() { flags.push("URG"); }
        if flags.is_empty() {
            write!(f, "NONE")
        } else {
            write!(f, "{}", flags.join("|"))
        }
    }
}

/// Sequence number modular arithmetic comparisons (RFC 793).
///
/// Because sequence numbers wrap around 32 bits, comparisons must be performed
/// via signed difference: `(a - b) as i32`.
#[inline]
pub fn seq_lt(a: u32, b: u32) -> bool {
    ((a.wrapping_sub(b)) as i32) < 0
}

#[inline]
pub fn seq_lte(a: u32, b: u32) -> bool {
    ((a.wrapping_sub(b)) as i32) <= 0
}

#[inline]
pub fn seq_gt(a: u32, b: u32) -> bool {
    ((a.wrapping_sub(b)) as i32) > 0
}

#[inline]
pub fn seq_gte(a: u32, b: u32) -> bool {
    ((a.wrapping_sub(b)) as i32) >= 0
}

/// TCP Segment Header.
///
/// ```text
///  0                   1                   2                   3
///  0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
/// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
/// |          Source Port          |       Destination Port        |
/// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
/// |                        Sequence Number                        |
/// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
/// |                    Acknowledgment Number                      |
/// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
/// |  Data |           |U|A|P|R|S|F|                               |
/// | Offset| Reserved  |R|C|S|S|Y|I|            Window             |
/// |       |           |G|K|H|T|N|N|                               |
/// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
/// |           Checksum            |         Urgent Pointer        |
/// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
/// |                    Options                    |    Padding    |
/// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TcpHeader {
    pub src_port: u16,
    pub dst_port: u16,
    pub seq_num: u32,
    pub ack_num: u32,
    pub data_offset: u8, // In 32-bit words (5 = 20 bytes min)
    pub flags: TcpFlags,
    pub window_size: u16,
    pub checksum: u16,
    pub urgent_pointer: u16,
    pub options: Vec<u8>,
}

impl TcpHeader {
    /// Construct a basic TCP header without options.
    pub fn new(src_port: u16, dst_port: u16, seq_num: u32, ack_num: u32, flags: TcpFlags, window_size: u16) -> Self {
        Self {
            src_port,
            dst_port,
            seq_num,
            ack_num,
            data_offset: 5,
            flags,
            window_size,
            checksum: 0,
            urgent_pointer: 0,
            options: Vec::new(),
        }
    }

    /// Header length in bytes.
    #[inline]
    pub fn header_len(&self) -> usize {
        (self.data_offset as usize) * 4
    }

    /// Serialize header to raw bytes (big-endian).
    pub fn to_bytes(&self) -> Vec<u8> {
        let total_header_len = (self.data_offset as usize) * 4;
        let mut buf = vec![0u8; total_header_len];

        buf[0..2].copy_from_slice(&self.src_port.to_be_bytes());
        buf[2..4].copy_from_slice(&self.dst_port.to_be_bytes());
        buf[4..8].copy_from_slice(&self.seq_num.to_be_bytes());
        buf[8..12].copy_from_slice(&self.ack_num.to_be_bytes());

        // Offset (top 4 bits) + Reserved (bottom 4 bits = 0)
        buf[12] = (self.data_offset << 4) & 0xF0;
        buf[13] = self.flags.bits();

        buf[14..16].copy_from_slice(&self.window_size.to_be_bytes());
        buf[16..18].copy_from_slice(&self.checksum.to_be_bytes());
        buf[18..20].copy_from_slice(&self.urgent_pointer.to_be_bytes());

        if !self.options.is_empty() {
            let opt_len = self.options.len().min(total_header_len - TCP_HEADER_MIN_LEN);
            buf[TCP_HEADER_MIN_LEN..TCP_HEADER_MIN_LEN + opt_len]
                .copy_from_slice(&self.options[..opt_len]);
        }

        buf
    }

    /// Parse TCP header from byte slice. Returns parsed header and total header byte length.
    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), TcpError> {
        if bytes.len() < TCP_HEADER_MIN_LEN {
            return Err(TcpError::PacketTooShort);
        }

        let src_port = u16::from_be_bytes([bytes[0], bytes[1]]);
        let dst_port = u16::from_be_bytes([bytes[2], bytes[3]]);
        let seq_num = u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        let ack_num = u32::from_be_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);

        let data_offset = (bytes[12] >> 4) & 0x0F;
        if data_offset < 5 {
            return Err(TcpError::InvalidDataOffset);
        }

        let total_header_len = (data_offset as usize) * 4;
        if bytes.len() < total_header_len {
            return Err(TcpError::PacketTooShort);
        }

        let flags = TcpFlags::from_bits(bytes[13]);
        let window_size = u16::from_be_bytes([bytes[14], bytes[15]]);
        let checksum = u16::from_be_bytes([bytes[16], bytes[17]]);
        let urgent_pointer = u16::from_be_bytes([bytes[18], bytes[19]]);

        let mut options = Vec::new();
        if total_header_len > TCP_HEADER_MIN_LEN {
            options.extend_from_slice(&bytes[TCP_HEADER_MIN_LEN..total_header_len]);
        }

        Ok((
            Self {
                src_port,
                dst_port,
                seq_num,
                ack_num,
                data_offset,
                flags,
                window_size,
                checksum,
                urgent_pointer,
                options,
            },
            total_header_len,
        ))
    }
}

/// Compute RFC 793 TCP Checksum over IPv4 Pseudo-Header, TCP Header, and Payload.
pub fn compute_tcp_checksum(
    src_ip: [u8; 4],
    dst_ip: [u8; 4],
    header: &TcpHeader,
    payload: &[u8],
) -> u16 {
    let mut sum: u32 = 0;

    // 1. Pseudo-header: Source IP (2 words)
    sum += u16::from_be_bytes([src_ip[0], src_ip[1]]) as u32;
    sum += u16::from_be_bytes([src_ip[2], src_ip[3]]) as u32;

    // 2. Pseudo-header: Destination IP (2 words)
    sum += u16::from_be_bytes([dst_ip[0], dst_ip[1]]) as u32;
    sum += u16::from_be_bytes([dst_ip[2], dst_ip[3]]) as u32;

    // 3. Pseudo-header: Zero (8 bits) + Protocol (8 bits = 6)
    sum += IP_PROTOCOL_TCP as u32;

    // 4. Pseudo-header: TCP Length (Header bytes + Payload bytes)
    let tcp_len = header.header_len() + payload.len();
    sum += tcp_len as u32;

    // 5. Serialize header with checksum = 0
    let mut hdr_bytes = header.to_bytes();
    hdr_bytes[16] = 0;
    hdr_bytes[17] = 0;

    let mut i = 0;
    while i < hdr_bytes.len() {
        sum += u16::from_be_bytes([hdr_bytes[i], hdr_bytes[i + 1]]) as u32;
        i += 2;
    }

    // 6. Payload words
    i = 0;
    while i + 1 < payload.len() {
        sum += u16::from_be_bytes([payload[i], payload[i + 1]]) as u32;
        i += 2;
    }
    if i < payload.len() {
        sum += u16::from_be_bytes([payload[i], 0x00]) as u32;
    }

    // Fold sum
    while (sum >> 16) != 0 {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }
    let folded = !(sum as u16);
    if folded == 0 {
        0xFFFF
    } else {
        folded
    }
}

/// Complete TCP Segment containing IP addresses, header, and payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TcpSegment {
    pub src_ip: [u8; 4],
    pub dst_ip: [u8; 4],
    pub header: TcpHeader,
    pub payload: Vec<u8>,
}

impl TcpSegment {
    pub fn new(
        src_ip: [u8; 4],
        dst_ip: [u8; 4],
        mut header: TcpHeader,
        payload: Vec<u8>,
    ) -> Self {
        header.checksum = compute_tcp_checksum(src_ip, dst_ip, &header, &payload);
        Self {
            src_ip,
            dst_ip,
            header,
            payload,
        }
    }

    /// Serialize segment to wire format (Header + Payload).
    pub fn serialize(&self) -> Vec<u8> {
        let mut wire = self.header.to_bytes();
        wire.extend_from_slice(&self.payload);
        wire
    }

    /// Parse raw TCP segment with checksum verification.
    pub fn parse(src_ip: [u8; 4], dst_ip: [u8; 4], raw: &[u8]) -> Result<Self, TcpError> {
        let (header, hdr_len) = TcpHeader::from_bytes(raw)?;
        let payload = raw[hdr_len..].to_vec();

        let expected_csum = compute_tcp_checksum(src_ip, dst_ip, &header, &payload);
        if header.checksum != expected_csum {
            return Err(TcpError::InvalidChecksum);
        }

        Ok(Self {
            src_ip,
            dst_ip,
            header,
            payload,
        })
    }
}

/// Transmission and Reception Sliding Window Buffers.
#[derive(Debug)]
pub struct SlidingWindow {
    /// In-order received bytes available for reading by application.
    rx_stream: VecDeque<u8>,
    /// Out-of-order received segments indexed by sequence number.
    rx_out_of_order: BTreeMap<u32, Vec<u8>>,
    /// Unacknowledged outgoing bytes indexed by sequence number.
    tx_unacked: VecDeque<(u32, Vec<u8>)>,
    /// Configured maximum window / buffer capacity.
    capacity: usize,
}

impl SlidingWindow {
    pub fn new(capacity: usize) -> Self {
        Self {
            rx_stream: VecDeque::with_capacity(capacity.min(4096)),
            rx_out_of_order: BTreeMap::new(),
            tx_unacked: VecDeque::new(),
            capacity,
        }
    }

    /// Calculate advertised receive window size (`rcv_wnd`).
    #[inline]
    pub fn advertised_rx_window(&self) -> u16 {
        let available = self.capacity.saturating_sub(self.rx_stream.len());
        available.min(0xFFFF) as u16
    }

    /// Push in-order payload into RX buffer. Returns number of bytes accepted.
    pub fn push_rx_in_order(&mut self, data: &[u8]) -> usize {
        let available = self.capacity.saturating_sub(self.rx_stream.len());
        let to_copy = data.len().min(available);
        self.rx_stream.extend(&data[..to_copy]);
        to_copy
    }

    /// Read available stream bytes from RX buffer into destination slice.
    pub fn read_rx(&mut self, buf: &mut [u8]) -> usize {
        let to_read = buf.len().min(self.rx_stream.len());
        for i in 0..to_read {
            buf[i] = self.rx_stream.pop_front().unwrap();
        }
        to_read
    }

    /// Buffer out-of-order segment within receive window.
    pub fn buffer_out_of_order(&mut self, seq: u32, data: Vec<u8>) {
        if self.rx_out_of_order.len() < 32 {
            self.rx_out_of_order.insert(seq, data);
        }
    }

    /// Try to merge contiguous out-of-order segments starting at `rcv_nxt`.
    pub fn reassemble_out_of_order(&mut self, rcv_nxt: &mut u32) {
        while let Some(entry) = self.rx_out_of_order.remove(rcv_nxt) {
            let len = entry.len() as u32;
            self.push_rx_in_order(&entry);
            *rcv_nxt = rcv_nxt.wrapping_add(len);
        }
    }

    /// Record transmitted segment awaiting ACK.
    pub fn push_tx_unacked(&mut self, seq: u32, data: Vec<u8>) {
        self.tx_unacked.push_back((seq, data));
    }

    /// Advance acknowledgment: prune unacknowledged segments up to `ack_num`.
    pub fn advance_tx_ack(&mut self, ack_num: u32) {
        while let Some((seq, data)) = self.tx_unacked.front() {
            let end_seq = seq.wrapping_add(data.len() as u32);
            if seq_lte(end_seq, ack_num) {
                self.tx_unacked.pop_front();
            } else {
                break;
            }
        }
    }

    #[inline]
    pub fn available_rx_bytes(&self) -> usize {
        self.rx_stream.len()
    }
}

/// Sequence and Acknowledgment State Tracking (RFC 793 Transmission Control Block).
#[derive(Debug, Clone)]
pub struct TcpSeqTracker {
    // Send Sequence Variables
    pub snd_una: u32, // Send unacknowledged
    pub snd_nxt: u32, // Send next
    pub snd_wnd: u16, // Send window
    pub iss: u32,     // Initial send sequence number

    // Receive Sequence Variables
    pub rcv_nxt: u32, // Receive next expected
    pub rcv_wnd: u16, // Receive window
    pub irs: u32,     // Initial receive sequence number
}

impl TcpSeqTracker {
    pub fn new(initial_seq: u32, window_size: u16) -> Self {
        Self {
            snd_una: initial_seq,
            snd_nxt: initial_seq,
            snd_wnd: window_size,
            iss: initial_seq,

            rcv_nxt: 0,
            rcv_wnd: window_size,
            irs: 0,
        }
    }
}

/// Full TCP Connection Endpoint.
///
/// Implements RFC 793 3-way handshake, sliding window byte transfers,
/// 4-way teardown, and reset processing.
#[derive(Debug)]
pub struct TcpConnection {
    pub local_ip: [u8; 4],
    pub local_port: u16,
    pub remote_ip: [u8; 4],
    pub remote_port: u16,

    pub state: TcpState,
    pub seq: TcpSeqTracker,
    pub window: SlidingWindow,
    pub mss: usize,
}

impl TcpConnection {
    /// Construct a new connection in CLOSED state.
    pub fn new(
        local_ip: [u8; 4],
        local_port: u16,
        remote_ip: [u8; 4],
        remote_port: u16,
        initial_seq: u32,
    ) -> Self {
        Self {
            local_ip,
            local_port,
            remote_ip,
            remote_port,
            state: TcpState::Closed,
            seq: TcpSeqTracker::new(initial_seq, TCP_DEFAULT_WINDOW_CAPACITY as u16),
            window: SlidingWindow::new(TCP_DEFAULT_WINDOW_CAPACITY),
            mss: TCP_DEFAULT_MSS,
        }
    }

    /// Construct a listening socket connection.
    pub fn new_listen(local_ip: [u8; 4], local_port: u16, initial_seq: u32) -> Self {
        let mut conn = Self::new(local_ip, local_port, [0, 0, 0, 0], 0, initial_seq);
        conn.state = TcpState::Listen;
        conn
    }

    // ========================================================================
    // Section 1: Active Connection Establishment (Client 3-Way Handshake)
    // ========================================================================

    /// Initiate Active Open: sends SYN, enters `SYN_SENT`.
    pub fn connect(&mut self) -> Result<TcpSegment, TcpError> {
        if self.state != TcpState::Closed {
            return Err(TcpError::InvalidStateTransition);
        }

        let flags = TcpFlags::syn();
        let header = TcpHeader::new(
            self.local_port,
            self.remote_port,
            self.seq.snd_nxt,
            0,
            flags,
            self.window.advertised_rx_window(),
        );

        // SYN consumes 1 sequence number
        self.seq.snd_nxt = self.seq.snd_nxt.wrapping_add(1);
        self.state = TcpState::SynSent;

        Ok(TcpSegment::new(self.local_ip, self.remote_ip, header, Vec::new()))
    }

    // ========================================================================
    // Section 2: Segment Processing & State Machine Driver
    // ========================================================================

    /// Process an incoming TCP segment. Returns an optional response segment (e.g. ACK, SYN-ACK, RST).
    pub fn process_segment(&mut self, seg: &TcpSegment) -> Result<Option<TcpSegment>, TcpError> {
        // If RST is received, immediately terminate to CLOSED
        if seg.header.flags.is_rst() {
            self.state = TcpState::Closed;
            return Err(TcpError::ConnectionReset);
        }

        match self.state {
            // Server Passive Open: Listen -> receives SYN -> sends SYN-ACK -> SynRcvd
            TcpState::Listen => {
                if seg.header.flags.is_syn() {
                    self.remote_ip = seg.src_ip;
                    self.remote_port = seg.header.src_port;

                    self.seq.irs = seg.header.seq_num;
                    self.seq.rcv_nxt = seg.header.seq_num.wrapping_add(1);
                    self.seq.snd_wnd = seg.header.window_size;

                    // Formulate SYN-ACK
                    let flags = TcpFlags::syn_ack();
                    let header = TcpHeader::new(
                        self.local_port,
                        self.remote_port,
                        self.seq.snd_nxt,
                        self.seq.rcv_nxt,
                        flags,
                        self.window.advertised_rx_window(),
                    );

                    // SYN consumes 1 sequence number
                    self.seq.snd_nxt = self.seq.snd_nxt.wrapping_add(1);
                    self.state = TcpState::SynRcvd;

                    return Ok(Some(TcpSegment::new(self.local_ip, self.remote_ip, header, Vec::new())));
                }
                Ok(None)
            }

            // Client: SynSent -> receives SYN-ACK -> sends ACK -> Established
            TcpState::SynSent => {
                if seg.header.flags.is_syn() && seg.header.flags.is_ack() {
                    // Verify ACK acknowledges our SYN
                    if seg.header.ack_num == self.seq.snd_nxt {
                        self.seq.snd_una = seg.header.ack_num;
                        self.seq.irs = seg.header.seq_num;
                        self.seq.rcv_nxt = seg.header.seq_num.wrapping_add(1);
                        self.seq.snd_wnd = seg.header.window_size;

                        // Send final ACK of 3-way handshake
                        let flags = TcpFlags::ack();
                        let header = TcpHeader::new(
                            self.local_port,
                            self.remote_port,
                            self.seq.snd_nxt,
                            self.seq.rcv_nxt,
                            flags,
                            self.window.advertised_rx_window(),
                        );

                        self.state = TcpState::Established;
                        return Ok(Some(TcpSegment::new(self.local_ip, self.remote_ip, header, Vec::new())));
                    }
                }
                Ok(None)
            }

            // Server: SynRcvd -> receives ACK -> Established
            TcpState::SynRcvd => {
                if seg.header.flags.is_ack() && seg.header.ack_num == self.seq.snd_nxt {
                    self.seq.snd_una = seg.header.ack_num;
                    self.seq.snd_wnd = seg.header.window_size;
                    self.state = TcpState::Established;
                }
                Ok(None)
            }

            // Established State: Bi-directional Data Exchange & Connection Teardown
            TcpState::Established => {
                let mut response_required = false;

                // Process ACK field
                if seg.header.flags.is_ack() {
                    self.seq.snd_una = seg.header.ack_num;
                    self.seq.snd_wnd = seg.header.window_size;
                    self.window.advance_tx_ack(seg.header.ack_num);
                }

                // Process Payload Data
                if !seg.payload.is_empty() {
                    if seg.header.seq_num == self.seq.rcv_nxt {
                        // In-order data
                        self.window.push_rx_in_order(&seg.payload);
                        self.seq.rcv_nxt = self.seq.rcv_nxt.wrapping_add(seg.payload.len() as u32);
                        // Reassemble any pending out-of-order data
                        self.window.reassemble_out_of_order(&mut self.seq.rcv_nxt);
                        response_required = true;
                    } else if seq_gt(seg.header.seq_num, self.seq.rcv_nxt) {
                        // Out-of-order segment within receive window
                        self.window.buffer_out_of_order(seg.header.seq_num, seg.payload.clone());
                        response_required = true; // Send duplicate ACK with current rcv_nxt
                    }
                }

                // Process FIN (Passive Close)
                if seg.header.flags.is_fin() {
                    self.seq.rcv_nxt = self.seq.rcv_nxt.wrapping_add(1);
                    self.state = TcpState::CloseWait;
                    response_required = true;
                }

                if response_required {
                    let header = TcpHeader::new(
                        self.local_port,
                        self.remote_port,
                        self.seq.snd_nxt,
                        self.seq.rcv_nxt,
                        TcpFlags::ack(),
                        self.window.advertised_rx_window(),
                    );
                    return Ok(Some(TcpSegment::new(self.local_ip, self.remote_ip, header, Vec::new())));
                }

                Ok(None)
            }

            // Client Active Close: FinWait1 -> receives ACK -> FinWait2
            TcpState::FinWait1 => {
                if seg.header.flags.is_ack() && seg.header.ack_num == self.seq.snd_nxt {
                    self.seq.snd_una = seg.header.ack_num;
                    if seg.header.flags.is_fin() {
                        // Simultaneous close or FIN+ACK together
                        self.seq.rcv_nxt = self.seq.rcv_nxt.wrapping_add(1);
                        self.state = TcpState::TimeWait;
                        let header = TcpHeader::new(
                            self.local_port,
                            self.remote_port,
                            self.seq.snd_nxt,
                            self.seq.rcv_nxt,
                            TcpFlags::ack(),
                            self.window.advertised_rx_window(),
                        );
                        return Ok(Some(TcpSegment::new(self.local_ip, self.remote_ip, header, Vec::new())));
                    }
                    self.state = TcpState::FinWait2;
                } else if seg.header.flags.is_fin() {
                    // Simultaneous close: received FIN before our FIN was acknowledged
                    self.seq.rcv_nxt = self.seq.rcv_nxt.wrapping_add(1);
                    self.state = TcpState::Closing;
                    let header = TcpHeader::new(
                        self.local_port,
                        self.remote_port,
                        self.seq.snd_nxt,
                        self.seq.rcv_nxt,
                        TcpFlags::ack(),
                        self.window.advertised_rx_window(),
                    );
                    return Ok(Some(TcpSegment::new(self.local_ip, self.remote_ip, header, Vec::new())));
                }
                Ok(None)
            }

            // Client Active Close: FinWait2 -> receives FIN -> sends ACK -> TimeWait
            TcpState::FinWait2 => {
                if seg.header.flags.is_fin() {
                    self.seq.rcv_nxt = self.seq.rcv_nxt.wrapping_add(1);
                    self.state = TcpState::TimeWait;
                    let header = TcpHeader::new(
                        self.local_port,
                        self.remote_port,
                        self.seq.snd_nxt,
                        self.seq.rcv_nxt,
                        TcpFlags::ack(),
                        self.window.advertised_rx_window(),
                    );
                    return Ok(Some(TcpSegment::new(self.local_ip, self.remote_ip, header, Vec::new())));
                }
                Ok(None)
            }

            // Simultaneous Close: Closing -> receives ACK -> TimeWait
            TcpState::Closing => {
                if seg.header.flags.is_ack() && seg.header.ack_num == self.seq.snd_nxt {
                    self.state = TcpState::TimeWait;
                }
                Ok(None)
            }

            // Server Passive Close: LastAck -> receives ACK -> Closed
            TcpState::LastAck => {
                if seg.header.flags.is_ack() && seg.header.ack_num == self.seq.snd_nxt {
                    self.state = TcpState::Closed;
                }
                Ok(None)
            }

            TcpState::TimeWait => {
                // Ignore duplicate FINs / send ACK if needed
                if seg.header.flags.is_fin() {
                    let header = TcpHeader::new(
                        self.local_port,
                        self.remote_port,
                        self.seq.snd_nxt,
                        self.seq.rcv_nxt,
                        TcpFlags::ack(),
                        self.window.advertised_rx_window(),
                    );
                    return Ok(Some(TcpSegment::new(self.local_ip, self.remote_ip, header, Vec::new())));
                }
                Ok(None)
            }

            TcpState::CloseWait | TcpState::Closed => Ok(None),
        }
    }

    // ========================================================================
    // Section 3: Data Transmission & Reception
    // ========================================================================

    /// Send application byte slice. Splits data into segments respecting MSS and window.
    pub fn send_data(&mut self, data: &[u8]) -> Result<Vec<TcpSegment>, TcpError> {
        if !self.state.can_send() {
            return Err(TcpError::BrokenPipe);
        }

        let mut segments = Vec::new();
        let mut offset = 0;

        while offset < data.len() {
            let chunk_len = (data.len() - offset).min(self.mss);
            let chunk = &data[offset..offset + chunk_len];

            let flags = if offset + chunk_len >= data.len() {
                TcpFlags::psh_ack()
            } else {
                TcpFlags::ack()
            };

            let header = TcpHeader::new(
                self.local_port,
                self.remote_port,
                self.seq.snd_nxt,
                self.seq.rcv_nxt,
                flags,
                self.window.advertised_rx_window(),
            );

            let segment = TcpSegment::new(self.local_ip, self.remote_ip, header, chunk.to_vec());

            // Track unacknowledged data and advance sequence number
            self.window.push_tx_unacked(self.seq.snd_nxt, chunk.to_vec());
            self.seq.snd_nxt = self.seq.snd_nxt.wrapping_add(chunk_len as u32);

            segments.push(segment);
            offset += chunk_len;
        }

        Ok(segments)
    }

    /// Read available stream bytes from receive sliding window.
    pub fn read_data(&mut self, buf: &mut [u8]) -> usize {
        self.window.read_rx(buf)
    }

    // ========================================================================
    // Section 4: Connection Teardown & Reset
    // ========================================================================

    /// Close connection initiated by local endpoint (Active Close or Last ACK).
    pub fn close(&mut self) -> Result<Option<TcpSegment>, TcpError> {
        match self.state {
            TcpState::Established => {
                // Send FIN+ACK -> enter FinWait1
                let flags = TcpFlags::fin_ack();
                let header = TcpHeader::new(
                    self.local_port,
                    self.remote_port,
                    self.seq.snd_nxt,
                    self.seq.rcv_nxt,
                    flags,
                    self.window.advertised_rx_window(),
                );

                self.seq.snd_nxt = self.seq.snd_nxt.wrapping_add(1); // FIN consumes 1 sequence number
                self.state = TcpState::FinWait1;

                Ok(Some(TcpSegment::new(self.local_ip, self.remote_ip, header, Vec::new())))
            }
            TcpState::CloseWait => {
                // Local close after receiving remote FIN -> enter LastAck
                let flags = TcpFlags::fin_ack();
                let header = TcpHeader::new(
                    self.local_port,
                    self.remote_port,
                    self.seq.snd_nxt,
                    self.seq.rcv_nxt,
                    flags,
                    self.window.advertised_rx_window(),
                );

                self.seq.snd_nxt = self.seq.snd_nxt.wrapping_add(1);
                self.state = TcpState::LastAck;

                Ok(Some(TcpSegment::new(self.local_ip, self.remote_ip, header, Vec::new())))
            }
            TcpState::Listen | TcpState::SynSent => {
                self.state = TcpState::Closed;
                Ok(None)
            }
            TcpState::Closed => Ok(None),
            _ => Err(TcpError::InvalidStateTransition),
        }
    }

    /// Send RST segment to abort connection.
    pub fn abort(&mut self) -> TcpSegment {
        let header = TcpHeader::new(
            self.local_port,
            self.remote_port,
            self.seq.snd_nxt,
            self.seq.rcv_nxt,
            TcpFlags::rst_ack(),
            0,
        );
        self.state = TcpState::Closed;
        TcpSegment::new(self.local_ip, self.remote_ip, header, Vec::new())
    }

    /// Transition from TIME_WAIT to CLOSED (e.g. 2MSL timer expiration).
    pub fn expire_time_wait(&mut self) {
        if self.state == TcpState::TimeWait {
            self.state = TcpState::Closed;
        }
    }
}
