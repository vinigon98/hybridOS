//! HybridOS Network Layer - Ethernet II Framing & MAC Abstraction
//!
//! Implements IEEE 802.3 / Ethernet II frame serialization, parsing,
//! payload packing/unpacking, and 48-bit MAC address abstractions.

#[allow(unused_imports)]
use alloc::string::{String, ToString};
#[allow(unused_imports)]
use alloc::vec::Vec;
#[allow(unused_imports)]
use alloc::boxed::Box;
#[allow(unused_imports)]
use crate::compat::FloatExt;
use std::fmt;

/// Length of an Ethernet MAC address in bytes.
pub const MAC_ADDR_LEN: usize = 6;

/// Minimum Ethernet II payload length (padded to 46 bytes if shorter).
pub const ETHERNET_MIN_PAYLOAD: usize = 46;

/// Maximum Ethernet II payload length (MTU = 1500 bytes for standard frames).
pub const ETHERNET_MAX_PAYLOAD: usize = 1500;

/// Length of the Ethernet II frame header (6 bytes dst + 6 bytes src + 2 bytes type).
pub const ETHERNET_HEADER_LEN: usize = 14;

/// Minimum Ethernet frame length without FCS (14 header + 46 payload).
pub const ETHERNET_MIN_FRAME_LEN: usize = ETHERNET_HEADER_LEN + ETHERNET_MIN_PAYLOAD;

/// Maximum standard Ethernet frame length without FCS (14 header + 1500 payload).
pub const ETHERNET_MAX_FRAME_LEN: usize = ETHERNET_HEADER_LEN + ETHERNET_MAX_PAYLOAD;

/// Length of the Frame Check Sequence (CRC-32).
pub const ETHERNET_FCS_LEN: usize = 4;

/// Represents a 48-bit IEEE 802 MAC address.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct MacAddress(pub [u8; MAC_ADDR_LEN]);

impl MacAddress {
    /// Broadcast MAC address: `FF:FF:FF:FF:FF:FF`
    pub const BROADCAST: Self = Self([0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]);

    /// Zero / unspecified MAC address: `00:00:00:00:00:00`
    pub const ZERO: Self = Self([0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);

    /// Create a new MAC address from a 6-byte array.
    pub const fn new(octets: [u8; MAC_ADDR_LEN]) -> Self {
        Self(octets)
    }

    /// Access the underlying 6-byte array.
    pub const fn octets(&self) -> [u8; MAC_ADDR_LEN] {
        self.0
    }

    /// Access the underlying bytes as a slice.
    pub fn as_bytes(&self) -> &[u8; MAC_ADDR_LEN] {
        &self.0
    }

    /// Create a MAC address from a byte slice (must have at least 6 bytes).
    pub fn from_slice(slice: &[u8]) -> Option<Self> {
        if slice.len() >= MAC_ADDR_LEN {
            let mut arr = [0u8; MAC_ADDR_LEN];
            arr.copy_from_slice(&slice[..MAC_ADDR_LEN]);
            Some(Self(arr))
        } else {
            None
        }
    }

    /// Returns true if this is the broadcast MAC address.
    pub fn is_broadcast(&self) -> bool {
        self.0 == [0xFF; MAC_ADDR_LEN]
    }

    /// Returns true if this is the zero MAC address.
    pub fn is_zero(&self) -> bool {
        self.0 == [0x00; MAC_ADDR_LEN]
    }

    /// Returns true if this is a multicast MAC address (least significant bit of first octet is 1, non-broadcast).
    pub fn is_multicast(&self) -> bool {
        (self.0[0] & 0x01) == 0x01 && !self.is_broadcast()
    }

    /// Returns true if this is a unicast MAC address (least significant bit of first octet is 0, non-zero).
    pub fn is_unicast(&self) -> bool {
        (self.0[0] & 0x01) == 0x00 && !self.is_zero()
    }

    /// Returns true if this is a locally administered address (second least significant bit of first octet is 1).
    pub fn is_local(&self) -> bool {
        (self.0[0] & 0x02) == 0x02
    }

    /// Parse a MAC address from standard string formats:
    /// - "00:11:22:33:44:55" (colon-separated)
    /// - "00-11-22-33-44-55" (hyphen-separated)
    pub fn parse(s: &str) -> Result<Self, MacParseError> {
        let delimiter = if s.contains(':') {
            ':'
        } else if s.contains('-') {
            '-'
        } else {
            return Err(MacParseError::InvalidFormat);
        };

        let parts: Vec<&str> = s.split(delimiter).collect();
        if parts.len() != MAC_ADDR_LEN {
            return Err(MacParseError::InvalidLength);
        }

        let mut octets = [0u8; MAC_ADDR_LEN];
        for (i, part) in parts.iter().enumerate() {
            if part.len() != 2 {
                return Err(MacParseError::InvalidOctet);
            }
            octets[i] = u8::from_str_radix(part, 16).map_err(|_| MacParseError::InvalidHex)?;
        }

        Ok(Self(octets))
    }
}

impl fmt::Display for MacAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
            self.0[0], self.0[1], self.0[2], self.0[3], self.0[4], self.0[5]
        )
    }
}

impl fmt::Debug for MacAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MacAddress({})", self)
    }
}

/// Errors occurring during MAC address parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MacParseError {
    InvalidFormat,
    InvalidLength,
    InvalidOctet,
    InvalidHex,
}

impl fmt::Display for MacParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFormat => write!(f, "Invalid MAC format (expected ':' or '-')"),
            Self::InvalidLength => write!(f, "Invalid MAC address length (expected 6 octets)"),
            Self::InvalidOctet => write!(f, "Invalid MAC octet length (expected 2 hex digits)"),
            Self::InvalidHex => write!(f, "Non-hexadecimal character in MAC address"),
        }
    }
}

impl std::error::Error for MacParseError {}

/// EtherType field values for Ethernet II frames.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EtherType {
    /// Internet Protocol version 4 (0x0800)
    Ipv4,
    /// Address Resolution Protocol (0x0806)
    Arp,
    /// Wake-on-LAN (0x0842)
    WakeOnLan,
    /// Reverse Address Resolution Protocol (0x8035)
    Rarp,
    /// VLAN-tagged frame (IEEE 802.1Q) (0x8100)
    Vlan,
    /// Internet Protocol version 6 (0x86DD)
    Ipv6,
    /// Link Layer Discovery Protocol (0x88CC)
    Lldp,
    /// Custom or unrecognized EtherType
    Unknown(u16),
}

impl EtherType {
    pub const IPV4_RAW: u16 = 0x0800;
    pub const ARP_RAW: u16 = 0x0806;
    pub const WOL_RAW: u16 = 0x0842;
    pub const RARP_RAW: u16 = 0x8035;
    pub const VLAN_RAW: u16 = 0x8100;
    pub const IPV6_RAW: u16 = 0x86DD;
    pub const LLDP_RAW: u16 = 0x88CC;

    /// Convert from raw 16-bit big-endian value.
    pub fn from_u16(val: u16) -> Self {
        match val {
            Self::IPV4_RAW => Self::Ipv4,
            Self::ARP_RAW => Self::Arp,
            Self::WOL_RAW => Self::WakeOnLan,
            Self::RARP_RAW => Self::Rarp,
            Self::VLAN_RAW => Self::Vlan,
            Self::IPV6_RAW => Self::Ipv6,
            Self::LLDP_RAW => Self::Lldp,
            other => Self::Unknown(other),
        }
    }

    /// Convert to 16-bit integer.
    pub fn as_u16(&self) -> u16 {
        match self {
            Self::Ipv4 => Self::IPV4_RAW,
            Self::Arp => Self::ARP_RAW,
            Self::WakeOnLan => Self::WOL_RAW,
            Self::Rarp => Self::RARP_RAW,
            Self::Vlan => Self::VLAN_RAW,
            Self::Ipv6 => Self::IPV6_RAW,
            Self::Lldp => Self::LLDP_RAW,
            Self::Unknown(val) => *val,
        }
    }
}

impl fmt::Display for EtherType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ipv4 => write!(f, "IPv4 (0x0800)"),
            Self::Arp => write!(f, "ARP (0x0806)"),
            Self::WakeOnLan => write!(f, "WoL (0x0842)"),
            Self::Rarp => write!(f, "RARP (0x8035)"),
            Self::Vlan => write!(f, "VLAN (0x8100)"),
            Self::Ipv6 => write!(f, "IPv6 (0x86DD)"),
            Self::Lldp => write!(f, "LLDP (0x88CC)"),
            Self::Unknown(val) => write!(f, "Unknown (0x{:04X})", val),
        }
    }
}

/// Ethernet II frame header (14 bytes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EthernetHeader {
    /// Destination MAC address (6 bytes).
    pub dst: MacAddress,
    /// Source MAC address (6 bytes).
    pub src: MacAddress,
    /// Protocol type of payload (2 bytes).
    pub ethertype: EtherType,
}

impl EthernetHeader {
    /// Construct a new Ethernet II header.
    pub fn new(dst: MacAddress, src: MacAddress, ethertype: EtherType) -> Self {
        Self { dst, src, ethertype }
    }

    /// Parse header from a byte buffer (at least 14 bytes required).
    pub fn parse(buf: &[u8]) -> Result<Self, EthernetError> {
        if buf.len() < ETHERNET_HEADER_LEN {
            return Err(EthernetError::BufferTooShort {
                expected: ETHERNET_HEADER_LEN,
                actual: buf.len(),
            });
        }

        let dst = MacAddress::from_slice(&buf[0..6]).ok_or(EthernetError::InvalidMac)?;
        let src = MacAddress::from_slice(&buf[6..12]).ok_or(EthernetError::InvalidMac)?;
        let raw_type = u16::from_be_bytes([buf[12], buf[13]]);
        let ethertype = EtherType::from_u16(raw_type);

        Ok(Self { dst, src, ethertype })
    }

    /// Serialize header into buffer (must have at least 14 bytes).
    pub fn serialize(&self, buf: &mut [u8]) -> Result<usize, EthernetError> {
        if buf.len() < ETHERNET_HEADER_LEN {
            return Err(EthernetError::BufferTooShort {
                expected: ETHERNET_HEADER_LEN,
                actual: buf.len(),
            });
        }

        buf[0..6].copy_from_slice(self.dst.as_bytes());
        buf[6..12].copy_from_slice(self.src.as_bytes());
        buf[12..14].copy_from_slice(&self.ethertype.as_u16().to_be_bytes());
        Ok(ETHERNET_HEADER_LEN)
    }

    /// Serialize header to a fixed 14-byte array.
    pub fn to_bytes(&self) -> [u8; ETHERNET_HEADER_LEN] {
        let mut bytes = [0u8; ETHERNET_HEADER_LEN];
        let _ = self.serialize(&mut bytes);
        bytes
    }
}

/// Complete Ethernet II frame with header and payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EthernetFrame {
    pub header: EthernetHeader,
    pub payload: Vec<u8>,
}

impl EthernetFrame {
    /// Create a new Ethernet frame from header and payload.
    pub fn new(header: EthernetHeader, payload: Vec<u8>) -> Self {
        Self { header, payload }
    }

    /// Pack payload into a complete Ethernet II frame buffer.
    ///
    /// If `pad_min_frame` is true and payload is smaller than 46 bytes,
    /// zeros will be appended to reach the minimum 64-byte frame size (60 bytes without FCS).
    pub fn pack(
        dst: MacAddress,
        src: MacAddress,
        ethertype: EtherType,
        payload: &[u8],
        pad_min_frame: bool,
    ) -> Vec<u8> {
        let mut frame = Vec::with_capacity(ETHERNET_HEADER_LEN + payload.len().max(ETHERNET_MIN_PAYLOAD));
        frame.extend_from_slice(dst.as_bytes());
        frame.extend_from_slice(src.as_bytes());
        frame.extend_from_slice(&ethertype.as_u16().to_be_bytes());
        frame.extend_from_slice(payload);

        if pad_min_frame && frame.len() < ETHERNET_MIN_FRAME_LEN {
            frame.resize(ETHERNET_MIN_FRAME_LEN, 0u8);
        }

        frame
    }

    /// Unpack an Ethernet frame from a raw byte buffer.
    pub fn unpack(buffer: &[u8]) -> Result<Self, EthernetError> {
        let header = EthernetHeader::parse(buffer)?;
        let payload = buffer[ETHERNET_HEADER_LEN..].to_vec();

        Ok(Self { header, payload })
    }

    /// Serialize this frame to a byte vector.
    pub fn to_bytes(&self, pad_min_frame: bool) -> Vec<u8> {
        Self::pack(
            self.header.dst,
            self.header.src,
            self.header.ethertype,
            &self.payload,
            pad_min_frame,
        )
    }

    /// Pack with IEEE 802.3 32-bit Frame Check Sequence (FCS) appended.
    pub fn to_bytes_with_fcs(&self, pad_min_frame: bool) -> Vec<u8> {
        let mut bytes = self.to_bytes(pad_min_frame);
        let crc = calculate_crc32(&bytes);
        bytes.extend_from_slice(&crc.to_le_bytes()); // Ethernet FCS is transmitted LSB first
        bytes
    }
}

/// Errors related to Ethernet frame operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EthernetError {
    BufferTooShort { expected: usize, actual: usize },
    BufferTooLong { max: usize, actual: usize },
    InvalidMac,
    InvalidFrameSize,
    FcsMismatch { expected: u32, calculated: u32 },
}

impl fmt::Display for EthernetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BufferTooShort { expected, actual } => {
                write!(f, "Ethernet buffer too short: expected {} bytes, got {}", expected, actual)
            }
            Self::BufferTooLong { max, actual } => {
                write!(f, "Ethernet frame exceeds maximum allowed {} bytes, got {}", max, actual)
            }
            Self::InvalidMac => write!(f, "Invalid MAC address in Ethernet header"),
            Self::InvalidFrameSize => write!(f, "Invalid Ethernet frame size"),
            Self::FcsMismatch { expected, calculated } => {
                write!(f, "FCS / CRC32 mismatch: expected 0x{:08X}, calculated 0x{:08X}", expected, calculated)
            }
        }
    }
}

impl std::error::Error for EthernetError {}

/// Compute standard IEEE 802.3 32-bit cyclic redundancy check (CRC-32 / FCS).
/// Uses polynomial 0xEDB88320 (reversed representation of 0x04C11DB7).
pub fn calculate_crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            let mask = -( (crc & 1) as i32 ) as u32;
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

/// Verify and strip the 4-byte FCS from the end of an Ethernet frame.
pub fn verify_and_strip_fcs(frame: &mut Vec<u8>) -> Result<(), EthernetError> {
    if frame.len() < ETHERNET_HEADER_LEN + ETHERNET_FCS_LEN {
        return Err(EthernetError::BufferTooShort {
            expected: ETHERNET_HEADER_LEN + ETHERNET_FCS_LEN,
            actual: frame.len(),
        });
    }

    let payload_len = frame.len() - ETHERNET_FCS_LEN;
    let expected_crc = u32::from_le_bytes([
        frame[payload_len],
        frame[payload_len + 1],
        frame[payload_len + 2],
        frame[payload_len + 3],
    ]);

    let calculated_crc = calculate_crc32(&frame[..payload_len]);
    if expected_crc != calculated_crc {
        return Err(EthernetError::FcsMismatch {
            expected: expected_crc,
            calculated: calculated_crc,
        });
    }

    frame.truncate(payload_len);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mac_address_basic() {
        let mac = MacAddress::new([0x52, 0x54, 0x00, 0x12, 0x34, 0x56]);
        assert_eq!(mac.to_string(), "52:54:00:12:34:56");
        assert!(mac.is_unicast());
        assert!(!mac.is_broadcast());
        assert!(!mac.is_multicast());

        let bcast = MacAddress::BROADCAST;
        assert!(bcast.is_broadcast());
        assert!(!bcast.is_unicast());

        let mcast = MacAddress::new([0x01, 0x00, 0x5E, 0x00, 0x00, 0x01]);
        assert!(mcast.is_multicast());
        assert!(!mcast.is_unicast());
    }

    #[test]
    fn test_mac_address_parse() {
        let mac_colon = MacAddress::parse("AA:BB:CC:DD:EE:FF").expect("Valid colon MAC");
        assert_eq!(mac_colon.octets(), [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF]);

        let mac_hyphen = MacAddress::parse("12-34-56-78-9a-bc").expect("Valid hyphen MAC");
        assert_eq!(mac_hyphen.octets(), [0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC]);

        assert!(MacAddress::parse("invalid-mac").is_err());
        assert!(MacAddress::parse("00:11:22:33:44").is_err());
        assert!(MacAddress::parse("00:11:22:33:44:GG").is_err());
    }

    #[test]
    fn test_ethertype_conversion() {
        assert_eq!(EtherType::from_u16(0x0800), EtherType::Ipv4);
        assert_eq!(EtherType::from_u16(0x0806), EtherType::Arp);
        assert_eq!(EtherType::from_u16(0x86DD), EtherType::Ipv6);
        assert_eq!(EtherType::from_u16(0x9999), EtherType::Unknown(0x9999));

        assert_eq!(EtherType::Ipv4.as_u16(), 0x0800);
        assert_eq!(EtherType::Arp.as_u16(), 0x0806);
        assert_eq!(EtherType::Ipv6.as_u16(), 0x86DD);
    }

    #[test]
    fn test_ethernet_header_pack_unpack() {
        let dst = MacAddress::new([0x52, 0x54, 0x00, 0xAA, 0xBB, 0xCC]);
        let src = MacAddress::new([0x00, 0x1A, 0x2B, 0x3C, 0x4D, 0x5E]);
        let header = EthernetHeader::new(dst, src, EtherType::Ipv4);

        let mut buf = [0u8; 14];
        let bytes_written = header.serialize(&mut buf).expect("Serialization ok");
        assert_eq!(bytes_written, 14);

        let parsed = EthernetHeader::parse(&buf).expect("Parsing ok");
        assert_eq!(parsed, header);
    }

    #[test]
    fn test_ethernet_frame_packing_with_padding() {
        let dst = MacAddress::BROADCAST;
        let src = MacAddress::new([0x00, 0x11, 0x22, 0x33, 0x44, 0x55]);
        let short_payload = b"PING"; // 4 bytes, needs 42 bytes padding to 46

        let packed = EthernetFrame::pack(dst, src, EtherType::Arp, short_payload, true);
        assert_eq!(packed.len(), ETHERNET_MIN_FRAME_LEN); // 60 bytes
        assert_eq!(&packed[14..18], b"PING");
        assert_eq!(&packed[18..60], &[0u8; 42]);

        let unpacked = EthernetFrame::unpack(&packed).expect("Unpack ok");
        assert_eq!(unpacked.header.dst, dst);
        assert_eq!(unpacked.header.src, src);
        assert_eq!(unpacked.header.ethertype, EtherType::Arp);
        assert_eq!(&unpacked.payload[0..4], b"PING");
    }

    #[test]
    fn test_crc32_and_fcs() {
        let data = b"123456789";
        let crc = calculate_crc32(data);
        assert_eq!(crc, 0xCBF43926); // Known standard check value for CRC-32 "123456789"

        let frame = EthernetFrame::new(
            EthernetHeader::new(MacAddress::BROADCAST, MacAddress::ZERO, EtherType::Ipv4),
            vec![1, 2, 3, 4, 5],
        );
        let mut wire_bytes = frame.to_bytes_with_fcs(true);
        assert_eq!(wire_bytes.len(), ETHERNET_MIN_FRAME_LEN + 4);

        assert!(verify_and_strip_fcs(&mut wire_bytes).is_ok());
        assert_eq!(wire_bytes.len(), ETHERNET_MIN_FRAME_LEN);
    }
}
