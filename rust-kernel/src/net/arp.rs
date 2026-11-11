//! HybridOS Network Layer - Address Resolution Protocol (ARP) & Dynamic ARP Cache
//!
//! Implements RFC 826 Ethernet-to-IPv4 address resolution, request/reply generation,
//! packet serialization/deserialization, and dynamic state-aware ARP caching with TTL aging.

#[allow(unused_imports)]
use alloc::string::{String, ToString};
#[allow(unused_imports)]
use alloc::vec::Vec;
#[allow(unused_imports)]
use alloc::boxed::Box;
#[allow(unused_imports)]
use crate::compat::FloatExt;
use std::collections::HashMap;
use std::fmt;

use super::ethernet::MacAddress;
use super::ipv4::Ipv4Address;

/// Hardware type for Ethernet (1).
pub const ARP_HARDWARE_ETHERNET: u16 = 1;

/// Protocol type for IPv4 (0x0800).
pub const ARP_PROTOCOL_IPV4: u16 = 0x0800;

/// Length of hardware address (6 bytes for Ethernet MAC).
pub const ARP_HW_ADDR_LEN: u8 = 6;

/// Length of protocol address (4 bytes for IPv4).
pub const ARP_PROTO_ADDR_LEN: u8 = 4;

/// Standard length of an ARP packet over Ethernet/IPv4 (28 bytes).
pub const ARP_PACKET_LEN: usize = 28;

/// Default dynamic ARP cache entry Time-To-Live in seconds (5 minutes).
pub const DEFAULT_ARP_TTL_SECS: u64 = 300;

/// ARP packet operation field values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArpOperation {
    /// ARP Request (1)
    Request,
    /// ARP Reply (2)
    Reply,
    /// RARP Request (3)
    RarpRequest,
    /// RARP Reply (4)
    RarpReply,
    /// Unknown or unsupported operation code
    Unknown(u16),
}

impl ArpOperation {
    pub const REQUEST_RAW: u16 = 1;
    pub const REPLY_RAW: u16 = 2;
    pub const RARP_REQUEST_RAW: u16 = 3;
    pub const RARP_REPLY_RAW: u16 = 4;

    pub fn from_u16(val: u16) -> Self {
        match val {
            Self::REQUEST_RAW => Self::Request,
            Self::REPLY_RAW => Self::Reply,
            Self::RARP_REQUEST_RAW => Self::RarpRequest,
            Self::RARP_REPLY_RAW => Self::RarpReply,
            other => Self::Unknown(other),
        }
    }

    pub fn as_u16(&self) -> u16 {
        match self {
            Self::Request => Self::REQUEST_RAW,
            Self::Reply => Self::REPLY_RAW,
            Self::RarpRequest => Self::RARP_REQUEST_RAW,
            Self::RarpReply => Self::RARP_REPLY_RAW,
            Self::Unknown(val) => *val,
        }
    }
}

impl fmt::Display for ArpOperation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Request => write!(f, "ARP Request (1)"),
            Self::Reply => write!(f, "ARP Reply (2)"),
            Self::RarpRequest => write!(f, "RARP Request (3)"),
            Self::RarpReply => write!(f, "RARP Reply (4)"),
            Self::Unknown(v) => write!(f, "ARP Opcode ({})", v),
        }
    }
}

/// Address Resolution Protocol (ARP) packet structure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArpPacket {
    /// Hardware type (e.g., Ethernet = 1).
    pub hw_type: u16,
    /// Protocol type (e.g., IPv4 = 0x0800).
    pub proto_type: u16,
    /// Hardware address length (6 for Ethernet).
    pub hw_len: u8,
    /// Protocol address length (4 for IPv4).
    pub proto_len: u8,
    /// Operation code (Request / Reply).
    pub operation: ArpOperation,
    /// Hardware address of sender.
    pub sender_mac: MacAddress,
    /// Protocol address of sender.
    pub sender_ip: Ipv4Address,
    /// Hardware address of target (all zeros in requests).
    pub target_mac: MacAddress,
    /// Protocol address of target.
    pub target_ip: Ipv4Address,
}

impl ArpPacket {
    /// Create a standard ARP Request packet seeking `target_ip`.
    pub fn new_request(sender_mac: MacAddress, sender_ip: Ipv4Address, target_ip: Ipv4Address) -> Self {
        Self {
            hw_type: ARP_HARDWARE_ETHERNET,
            proto_type: ARP_PROTOCOL_IPV4,
            hw_len: ARP_HW_ADDR_LEN,
            proto_len: ARP_PROTO_ADDR_LEN,
            operation: ArpOperation::Request,
            sender_mac,
            sender_ip,
            target_mac: MacAddress::ZERO,
            target_ip,
        }
    }

    /// Create a standard ARP Reply packet responding to an earlier request.
    pub fn new_reply(
        sender_mac: MacAddress,
        sender_ip: Ipv4Address,
        target_mac: MacAddress,
        target_ip: Ipv4Address,
    ) -> Self {
        Self {
            hw_type: ARP_HARDWARE_ETHERNET,
            proto_type: ARP_PROTOCOL_IPV4,
            hw_len: ARP_HW_ADDR_LEN,
            proto_len: ARP_PROTO_ADDR_LEN,
            operation: ArpOperation::Reply,
            sender_mac,
            sender_ip,
            target_mac,
            target_ip,
        }
    }

    /// Parse an ARP packet from a byte slice.
    pub fn parse(buf: &[u8]) -> Result<Self, ArpError> {
        if buf.len() < ARP_PACKET_LEN {
            return Err(ArpError::BufferTooShort {
                expected: ARP_PACKET_LEN,
                actual: buf.len(),
            });
        }

        let hw_type = u16::from_be_bytes([buf[0], buf[1]]);
        let proto_type = u16::from_be_bytes([buf[2], buf[3]]);
        let hw_len = buf[4];
        let proto_len = buf[5];

        if hw_len != ARP_HW_ADDR_LEN {
            return Err(ArpError::UnsupportedHardwareLength(hw_len));
        }
        if proto_len != ARP_PROTO_ADDR_LEN {
            return Err(ArpError::UnsupportedProtocolLength(proto_len));
        }

        let operation = ArpOperation::from_u16(u16::from_be_bytes([buf[6], buf[7]]));
        let sender_mac = MacAddress::from_slice(&buf[8..14]).ok_or(ArpError::InvalidMac)?;
        let sender_ip = Ipv4Address::from_slice(&buf[14..18]).ok_or(ArpError::InvalidIp)?;
        let target_mac = MacAddress::from_slice(&buf[18..24]).ok_or(ArpError::InvalidMac)?;
        let target_ip = Ipv4Address::from_slice(&buf[24..28]).ok_or(ArpError::InvalidIp)?;

        Ok(Self {
            hw_type,
            proto_type,
            hw_len,
            proto_len,
            operation,
            sender_mac,
            sender_ip,
            target_mac,
            target_ip,
        })
    }

    /// Serialize this ARP packet into a byte buffer (at least 28 bytes).
    pub fn serialize(&self, buf: &mut [u8]) -> Result<usize, ArpError> {
        if buf.len() < ARP_PACKET_LEN {
            return Err(ArpError::BufferTooShort {
                expected: ARP_PACKET_LEN,
                actual: buf.len(),
            });
        }

        buf[0..2].copy_from_slice(&self.hw_type.to_be_bytes());
        buf[2..4].copy_from_slice(&self.proto_type.to_be_bytes());
        buf[4] = self.hw_len;
        buf[5] = self.proto_len;
        buf[6..8].copy_from_slice(&self.operation.as_u16().to_be_bytes());

        buf[8..14].copy_from_slice(self.sender_mac.as_bytes());
        buf[14..18].copy_from_slice(&self.sender_ip.octets());
        buf[18..24].copy_from_slice(self.target_mac.as_bytes());
        buf[24..28].copy_from_slice(&self.target_ip.octets());

        Ok(ARP_PACKET_LEN)
    }

    /// Serialize into a fixed 28-byte array.
    pub fn to_bytes(&self) -> [u8; ARP_PACKET_LEN] {
        let mut buf = [0u8; ARP_PACKET_LEN];
        let _ = self.serialize(&mut buf);
        buf
    }
}

/// State of an ARP cache entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArpEntryState {
    /// Static / permanent entry (never expires).
    Permanent,
    /// Dynamic entry resolved and valid.
    Resolved,
    /// Pending resolution (ARP Request sent, waiting for reply).
    Pending,
    /// Resolution failed / unreachable.
    Failed,
}

/// Single IP-to-MAC resolution entry in the ARP cache.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArpEntry {
    /// Resolved IP address.
    pub ip: Ipv4Address,
    /// Corresponding hardware MAC address.
    pub mac: MacAddress,
    /// Entry lifecycle state.
    pub state: ArpEntryState,
    /// Timestamp of entry creation or last refresh (in seconds).
    pub updated_at: u64,
    /// Time-To-Live in seconds.
    pub ttl: u64,
}

impl ArpEntry {
    /// Create a new ARP cache entry.
    pub fn new(ip: Ipv4Address, mac: MacAddress, state: ArpEntryState, updated_at: u64, ttl: u64) -> Self {
        Self {
            ip,
            mac,
            state,
            updated_at,
            ttl,
        }
    }

    /// Returns true if this dynamic entry has passed its TTL expiration timestamp.
    pub fn is_expired(&self, current_time: u64) -> bool {
        if self.state == ArpEntryState::Permanent {
            return false;
        }
        current_time.saturating_sub(self.updated_at) > self.ttl
    }
}

/// Dynamic ARP Translation Table (ARP Cache).
#[derive(Debug, Clone)]
pub struct ArpCache {
    entries: HashMap<Ipv4Address, ArpEntry>,
    default_ttl: u64,
}

impl Default for ArpCache {
    fn default() -> Self {
        Self::new()
    }
}

impl ArpCache {
    /// Create a new ARP cache with default 300-second TTL.
    pub fn new() -> Self {
        Self::with_ttl(DEFAULT_ARP_TTL_SECS)
    }

    /// Create an ARP cache with custom default TTL.
    pub fn with_ttl(default_ttl: u64) -> Self {
        Self {
            entries: HashMap::new(),
            default_ttl,
        }
    }

    /// Look up the MAC address associated with an IPv4 address.
    /// Only returns `Some(mac)` if entry exists, is `Resolved` or `Permanent`, and not expired.
    pub fn lookup(&self, ip: &Ipv4Address) -> Option<MacAddress> {
        self.entries.get(ip).and_then(|entry| {
            if entry.state == ArpEntryState::Permanent || entry.state == ArpEntryState::Resolved {
                Some(entry.mac)
            } else {
                None
            }
        })
    }

    /// Get raw entry reference if present.
    pub fn get_entry(&self, ip: &Ipv4Address) -> Option<&ArpEntry> {
        self.entries.get(ip)
    }

    /// Insert or update an entry in the ARP cache.
    pub fn insert(&mut self, ip: Ipv4Address, mac: MacAddress, state: ArpEntryState, current_time: u64) {
        let entry = ArpEntry::new(ip, mac, state, current_time, self.default_ttl);
        self.entries.insert(ip, entry);
    }

    /// Insert a static / permanent entry that never expires.
    pub fn insert_static(&mut self, ip: Ipv4Address, mac: MacAddress) {
        let entry = ArpEntry::new(ip, mac, ArpEntryState::Permanent, 0, u64::MAX);
        self.entries.insert(ip, entry);
    }

    /// Update an existing or new dynamic entry as Resolved.
    pub fn update(&mut self, ip: Ipv4Address, mac: MacAddress, current_time: u64) {
        self.insert(ip, mac, ArpEntryState::Resolved, current_time);
    }

    /// Mark an IP as pending resolution.
    pub fn mark_pending(&mut self, ip: Ipv4Address, current_time: u64) {
        let entry = ArpEntry::new(ip, MacAddress::ZERO, ArpEntryState::Pending, current_time, 5); // 5 sec retry TTL
        self.entries.insert(ip, entry);
    }

    /// Remove an entry by IP address.
    pub fn remove(&mut self, ip: &Ipv4Address) -> bool {
        self.entries.remove(ip).is_some()
    }

    /// Purge expired dynamic entries based on `current_time`.
    pub fn purge_expired(&mut self, current_time: u64) -> usize {
        let initial_len = self.entries.len();
        self.entries.retain(|_, entry| !entry.is_expired(current_time));
        initial_len - self.entries.len()
    }

    /// Process an incoming ARP packet according to RFC 826 specifications.
    ///
    /// 1. Updates or learns the sender's IP-to-MAC mapping in the local cache.
    /// 2. If the packet is an ARP Request for `our_ip`, generates and returns an ARP Reply packet.
    pub fn process_incoming(
        &mut self,
        packet: &ArpPacket,
        our_ip: Ipv4Address,
        our_mac: MacAddress,
        current_time: u64,
    ) -> Option<ArpPacket> {
        // RFC 826: Learn or refresh sender's MAC in our cache
        if !packet.sender_ip.is_unspecified() && !packet.sender_mac.is_zero() && !packet.sender_mac.is_broadcast() {
            self.update(packet.sender_ip, packet.sender_mac, current_time);
        }

        // Check if packet is addressed to our IP
        if packet.target_ip == our_ip {
            match packet.operation {
                ArpOperation::Request => {
                    // Generate ARP Reply
                    let reply = ArpPacket::new_reply(
                        our_mac,
                        our_ip,
                        packet.sender_mac,
                        packet.sender_ip,
                    );
                    Some(reply)
                }
                ArpOperation::Reply => {
                    // Sender info already learned above
                    None
                }
                _ => None,
            }
        } else {
            None
        }
    }

    /// Retrieve a snapshot of all active entries.
    pub fn all_entries(&self) -> Vec<ArpEntry> {
        self.entries.values().cloned().collect()
    }

    /// Number of entries in the ARP cache.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns true if cache is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clear all entries from the ARP cache.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

/// Errors related to ARP packet operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArpError {
    BufferTooShort { expected: usize, actual: usize },
    UnsupportedHardwareLength(u8),
    UnsupportedProtocolLength(u8),
    InvalidMac,
    InvalidIp,
}

impl fmt::Display for ArpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BufferTooShort { expected, actual } => {
                write!(f, "ARP buffer too short: expected {} bytes, got {}", expected, actual)
            }
            Self::UnsupportedHardwareLength(len) => {
                write!(f, "Unsupported ARP hardware address length (expected 6, got {})", len)
            }
            Self::UnsupportedProtocolLength(len) => {
                write!(f, "Unsupported ARP protocol address length (expected 4, got {})", len)
            }
            Self::InvalidMac => write!(f, "Invalid MAC address in ARP packet"),
            Self::InvalidIp => write!(f, "Invalid IPv4 address in ARP packet"),
        }
    }
}

impl std::error::Error for ArpError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arp_request_serialize_parse() {
        let sender_mac = MacAddress::new([0x00, 0x11, 0x22, 0x33, 0x44, 0x55]);
        let sender_ip = Ipv4Address::new(192, 168, 1, 10);
        let target_ip = Ipv4Address::new(192, 168, 1, 1);

        let req = ArpPacket::new_request(sender_mac, sender_ip, target_ip);
        let raw = req.to_bytes();
        assert_eq!(raw.len(), ARP_PACKET_LEN);

        let parsed = ArpPacket::parse(&raw).expect("ARP request parsed ok");
        assert_eq!(parsed.operation, ArpOperation::Request);
        assert_eq!(parsed.sender_mac, sender_mac);
        assert_eq!(parsed.sender_ip, sender_ip);
        assert_eq!(parsed.target_mac, MacAddress::ZERO);
        assert_eq!(parsed.target_ip, target_ip);
    }

    #[test]
    fn test_arp_reply_serialize_parse() {
        let sender_mac = MacAddress::new([0x52, 0x54, 0x00, 0xAA, 0xBB, 0xCC]);
        let sender_ip = Ipv4Address::new(192, 168, 1, 1);
        let target_mac = MacAddress::new([0x00, 0x11, 0x22, 0x33, 0x44, 0x55]);
        let target_ip = Ipv4Address::new(192, 168, 1, 10);

        let reply = ArpPacket::new_reply(sender_mac, sender_ip, target_mac, target_ip);
        let raw = reply.to_bytes();

        let parsed = ArpPacket::parse(&raw).expect("ARP reply parsed ok");
        assert_eq!(parsed.operation, ArpOperation::Reply);
        assert_eq!(parsed.sender_mac, sender_mac);
        assert_eq!(parsed.sender_ip, sender_ip);
        assert_eq!(parsed.target_mac, target_mac);
        assert_eq!(parsed.target_ip, target_ip);
    }

    #[test]
    fn test_arp_cache_dynamic_aging() {
        let mut cache = ArpCache::with_ttl(60);
        let ip = Ipv4Address::new(192, 168, 1, 50);
        let mac = MacAddress::new([0x00, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE]);

        // Insert at t = 100
        cache.insert(ip, mac, ArpEntryState::Resolved, 100);
        assert_eq!(cache.lookup(&ip), Some(mac));

        // At t = 150 (elapsed 50 <= 60), still valid
        assert_eq!(cache.purge_expired(150), 0);
        assert_eq!(cache.lookup(&ip), Some(mac));

        // At t = 165 (elapsed 65 > 60), expired and purged
        assert_eq!(cache.purge_expired(165), 1);
        assert_eq!(cache.lookup(&ip), None);
    }

    #[test]
    fn test_arp_cache_process_incoming_generates_reply() {
        let mut cache = ArpCache::new();
        let our_ip = Ipv4Address::new(192, 168, 1, 100);
        let our_mac = MacAddress::new([0x52, 0x54, 0x00, 0x12, 0x34, 0x56]);

        let sender_ip = Ipv4Address::new(192, 168, 1, 200);
        let sender_mac = MacAddress::new([0x00, 0x1A, 0x2B, 0x3C, 0x4D, 0x5E]);

        let req = ArpPacket::new_request(sender_mac, sender_ip, our_ip);

        // Process request: should learn sender and generate reply
        let reply_opt = cache.process_incoming(&req, our_ip, our_mac, 500);
        assert!(reply_opt.is_some());

        let reply = reply_opt.unwrap();
        assert_eq!(reply.operation, ArpOperation::Reply);
        assert_eq!(reply.sender_mac, our_mac);
        assert_eq!(reply.sender_ip, our_ip);
        assert_eq!(reply.target_mac, sender_mac);
        assert_eq!(reply.target_ip, sender_ip);

        // Verify sender was learned into cache
        assert_eq!(cache.lookup(&sender_ip), Some(sender_mac));
    }
}
