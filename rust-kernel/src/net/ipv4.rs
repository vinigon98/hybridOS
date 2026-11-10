//! HybridOS Network Layer - IPv4 Header Parsing, Serialization, Checksum & CIDR Routing
//!
//! Implements RFC 791 IPv4 protocol, RFC 1071 internet checksum calculation,
//! and CIDR longest-prefix-match (LPM) routing table.

use std::fmt;

/// Minimum length of an IPv4 header (IHL = 5, 20 bytes).
pub const IPV4_MIN_HEADER_LEN: usize = 20;

/// Maximum length of an IPv4 header including options (IHL = 15, 60 bytes).
pub const IPV4_MAX_HEADER_LEN: usize = 60;

/// Default Time-To-Live (TTL) for outbound packets.
pub const DEFAULT_TTL: u8 = 64;

/// IPv4 flag: Don't Fragment (DF).
pub const IPV4_FLAG_DF: u16 = 0x4000;

/// IPv4 flag: More Fragments (MF).
pub const IPV4_FLAG_MF: u16 = 0x2000;

/// Mask for fragment offset (13 bits).
pub const IPV4_FRAG_OFFSET_MASK: u16 = 0x1FFF;

/// Represents a 32-bit IPv4 address.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Ipv4Address(pub [u8; 4]);

impl Ipv4Address {
    /// 0.0.0.0 (Unspecified / Any)
    pub const ANY: Self = Self([0, 0, 0, 0]);

    /// 255.255.255.255 (Limited Broadcast)
    pub const BROADCAST: Self = Self([255, 255, 255, 255]);

    /// 127.0.0.1 (Loopback)
    pub const LOOPBACK: Self = Self([127, 0, 0, 1]);

    /// Construct an IPv4 address from 4 octets.
    pub const fn new(a: u8, b: u8, c: u8, d: u8) -> Self {
        Self([a, b, c, d])
    }

    /// Access octets as an array.
    pub const fn octets(&self) -> [u8; 4] {
        self.0
    }

    /// Convert to big-endian 32-bit unsigned integer.
    pub const fn to_u32(&self) -> u32 {
        u32::from_be_bytes(self.0)
    }

    /// Construct from big-endian 32-bit integer.
    pub const fn from_u32(val: u32) -> Self {
        Self(val.to_be_bytes())
    }

    /// Construct from slice of at least 4 bytes.
    pub fn from_slice(slice: &[u8]) -> Option<Self> {
        if slice.len() >= 4 {
            Some(Self([slice[0], slice[1], slice[2], slice[3]]))
        } else {
            None
        }
    }

    /// Parse an IPv4 address string like "192.168.1.1".
    pub fn parse(s: &str) -> Result<Self, Ipv4ParseError> {
        let parts: Vec<&str> = s.split('.').collect();
        if parts.len() != 4 {
            return Err(Ipv4ParseError::InvalidFormat);
        }

        let mut octets = [0u8; 4];
        for (i, part) in parts.iter().enumerate() {
            let val = part.parse::<u8>().map_err(|_| Ipv4ParseError::InvalidOctet)?;
            octets[i] = val;
        }

        Ok(Self(octets))
    }

    /// Generate a netmask from a CIDR prefix length (0..=32).
    pub fn netmask_from_prefix(prefix_len: u8) -> Result<Self, Ipv4Error> {
        if prefix_len > 32 {
            return Err(Ipv4Error::InvalidPrefixLength(prefix_len));
        }

        let mask_u32 = if prefix_len == 0 {
            0u32
        } else {
            !0u32 << (32 - prefix_len)
        };

        Ok(Self::from_u32(mask_u32))
    }

    /// Convert netmask to prefix length if it is a valid contiguous CIDR mask.
    pub fn to_prefix_length(&self) -> Result<u8, Ipv4Error> {
        let val = self.to_u32();
        let leading_ones = val.leading_ones() as u8;
        let trailing_zeros = val.trailing_zeros() as u8;

        if leading_ones + trailing_zeros == 32 {
            Ok(leading_ones)
        } else {
            Err(Ipv4Error::NonContiguousNetmask(*self))
        }
    }

    /// Check if this address matches a subnet defined by a network address and netmask.
    pub fn matches_subnet(&self, network: Ipv4Address, netmask: Ipv4Address) -> bool {
        let self_u32 = self.to_u32();
        let net_u32 = network.to_u32();
        let mask_u32 = netmask.to_u32();

        (self_u32 & mask_u32) == (net_u32 & mask_u32)
    }

    /// Returns true if this is in the 127.0.0.0/8 loopback range.
    pub fn is_loopback(&self) -> bool {
        self.0[0] == 127
    }

    /// Returns true if this is the limited broadcast address (255.255.255.255).
    pub fn is_broadcast(&self) -> bool {
        self.0 == [255, 255, 255, 255]
    }

    /// Returns true if this is in the 224.0.0.0/4 multicast range.
    pub fn is_multicast(&self) -> bool {
        (self.0[0] >= 224) && (self.0[0] <= 239)
    }

    /// Returns true if this is in private ranges (RFC 1918):
    /// - 10.0.0.0/8
    /// - 172.16.0.0/12
    /// - 192.168.0.0/16
    pub fn is_private(&self) -> bool {
        match self.0[0] {
            10 => true,
            172 => (self.0[1] >= 16) && (self.0[1] <= 31),
            192 => self.0[1] == 168,
            _ => false,
        }
    }

    /// Returns true if this is the unspecified address (0.0.0.0).
    pub fn is_unspecified(&self) -> bool {
        self.0 == [0, 0, 0, 0]
    }
}

impl fmt::Display for Ipv4Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}.{}", self.0[0], self.0[1], self.0[2], self.0[3])
    }
}

impl fmt::Debug for Ipv4Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Ipv4Address({})", self)
    }
}

/// IPv4 address parse errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ipv4ParseError {
    InvalidFormat,
    InvalidOctet,
}

impl fmt::Display for Ipv4ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFormat => write!(f, "Invalid IPv4 address format (expected 4 octets separated by '.')"),
            Self::InvalidOctet => write!(f, "Invalid IPv4 octet value (must be 0-255)"),
        }
    }
}

impl std::error::Error for Ipv4ParseError {}

/// Known IPv4 protocol numbers (RFC 790 / IANA).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Ipv4Protocol {
    /// Internet Control Message Protocol (1)
    Icmp,
    /// Internet Group Management Protocol (2)
    Igmp,
    /// Transmission Control Protocol (6)
    Tcp,
    /// User Datagram Protocol (17)
    Udp,
    /// IPv6 Encapsulation (41)
    Ipv6Encap,
    /// Open Shortest Path First (89)
    Ospf,
    /// Raw IP packet (255)
    Raw,
    /// Unrecognized protocol number
    Unknown(u8),
}

impl Ipv4Protocol {
    pub const ICMP_RAW: u8 = 1;
    pub const IGMP_RAW: u8 = 2;
    pub const TCP_RAW: u8 = 6;
    pub const UDP_RAW: u8 = 17;
    pub const IPV6_ENCAP_RAW: u8 = 41;
    pub const OSPF_RAW: u8 = 89;
    pub const RAW_RAW: u8 = 255;

    pub fn from_u8(val: u8) -> Self {
        match val {
            Self::ICMP_RAW => Self::Icmp,
            Self::IGMP_RAW => Self::Igmp,
            Self::TCP_RAW => Self::Tcp,
            Self::UDP_RAW => Self::Udp,
            Self::IPV6_ENCAP_RAW => Self::Ipv6Encap,
            Self::OSPF_RAW => Self::Ospf,
            Self::RAW_RAW => Self::Raw,
            other => Self::Unknown(other),
        }
    }

    pub fn as_u8(&self) -> u8 {
        match self {
            Self::Icmp => Self::ICMP_RAW,
            Self::Igmp => Self::IGMP_RAW,
            Self::Tcp => Self::TCP_RAW,
            Self::Udp => Self::UDP_RAW,
            Self::Ipv6Encap => Self::IPV6_ENCAP_RAW,
            Self::Ospf => Self::OSPF_RAW,
            Self::Raw => Self::RAW_RAW,
            Self::Unknown(val) => *val,
        }
    }
}

impl fmt::Display for Ipv4Protocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Icmp => write!(f, "ICMP (1)"),
            Self::Igmp => write!(f, "IGMP (2)"),
            Self::Tcp => write!(f, "TCP (6)"),
            Self::Udp => write!(f, "UDP (17)"),
            Self::Ipv6Encap => write!(f, "IPv6Encap (41)"),
            Self::Ospf => write!(f, "OSPF (89)"),
            Self::Raw => write!(f, "Raw (255)"),
            Self::Unknown(val) => write!(f, "Unknown ({})", val),
        }
    }
}

/// IPv4 Header structure according to RFC 791.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ipv4Header {
    /// IP version (must be 4).
    pub version: u8,
    /// Internet Header Length (IHL) in 32-bit words (minimum 5 = 20 bytes).
    pub ihl: u8,
    /// Type of Service / Differentiated Services Code Point & ECN.
    pub tos: u8,
    /// Total length of the IPv4 packet (header + payload) in bytes.
    pub total_length: u16,
    /// Packet identification for fragmentation.
    pub identification: u16,
    /// Flags: DF (Don't Fragment), MF (More Fragments).
    pub flags: u8,
    /// Fragment offset in 8-byte blocks.
    pub fragment_offset: u16,
    /// Time To Live in network hops.
    pub ttl: u8,
    /// Upper-layer protocol.
    pub protocol: Ipv4Protocol,
    /// 16-bit one's complement header checksum.
    pub checksum: u16,
    /// Source IP address.
    pub src: Ipv4Address,
    /// Destination IP address.
    pub dst: Ipv4Address,
    /// Optional IP header options.
    pub options: Vec<u8>,
}

impl Ipv4Header {
    /// Create a new standard IPv4 header (IHL = 5, no options).
    pub fn new(
        src: Ipv4Address,
        dst: Ipv4Address,
        protocol: Ipv4Protocol,
        payload_len: usize,
    ) -> Result<Self, Ipv4Error> {
        let total_length = (IPV4_MIN_HEADER_LEN + payload_len) as u16;

        let mut header = Self {
            version: 4,
            ihl: 5,
            tos: 0,
            total_length,
            identification: 0x1234,
            flags: 0x02, // Don't Fragment flag (bit 1 of 3-bit flags)
            fragment_offset: 0,
            ttl: DEFAULT_TTL,
            protocol,
            checksum: 0,
            src,
            dst,
            options: Vec::new(),
        };

        header.checksum = header.calculate_checksum();
        Ok(header)
    }

    /// Header size in bytes based on IHL.
    pub fn header_len(&self) -> usize {
        (self.ihl as usize) * 4
    }

    /// Returns true if the Don't Fragment (DF) flag is set.
    pub fn is_dont_fragment(&self) -> bool {
        (self.flags & 0x02) != 0
    }

    /// Returns true if the More Fragments (MF) flag is set.
    pub fn is_more_fragments(&self) -> bool {
        (self.flags & 0x01) != 0
    }

    /// Parse an IPv4 header from a raw byte buffer.
    pub fn parse(buf: &[u8]) -> Result<(Self, usize), Ipv4Error> {
        if buf.len() < IPV4_MIN_HEADER_LEN {
            return Err(Ipv4Error::BufferTooShort {
                expected: IPV4_MIN_HEADER_LEN,
                actual: buf.len(),
            });
        }

        let ver_ihl = buf[0];
        let version = (ver_ihl >> 4) & 0x0F;
        let ihl = ver_ihl & 0x0F;

        if version != 4 {
            return Err(Ipv4Error::InvalidVersion(version));
        }

        if ihl < 5 {
            return Err(Ipv4Error::InvalidIhl(ihl));
        }

        let header_len = (ihl as usize) * 4;
        if buf.len() < header_len {
            return Err(Ipv4Error::BufferTooShort {
                expected: header_len,
                actual: buf.len(),
            });
        }

        let tos = buf[1];
        let total_length = u16::from_be_bytes([buf[2], buf[3]]);
        let identification = u16::from_be_bytes([buf[4], buf[5]]);

        let flags_and_frag = u16::from_be_bytes([buf[6], buf[7]]);
        let flags = ((flags_and_frag >> 13) & 0x07) as u8;
        let fragment_offset = flags_and_frag & IPV4_FRAG_OFFSET_MASK;

        let ttl = buf[8];
        let protocol = Ipv4Protocol::from_u8(buf[9]);
        let checksum = u16::from_be_bytes([buf[10], buf[11]]);

        let src = Ipv4Address::from_slice(&buf[12..16]).unwrap();
        let dst = Ipv4Address::from_slice(&buf[16..20]).unwrap();

        let options = if header_len > IPV4_MIN_HEADER_LEN {
            buf[IPV4_MIN_HEADER_LEN..header_len].to_vec()
        } else {
            Vec::new()
        };

        // Verify checksum over header slice
        let computed = compute_internet_checksum(&buf[..header_len]);
        if computed != 0 {
            return Err(Ipv4Error::ChecksumMismatch {
                expected: 0,
                calculated: computed,
            });
        }

        let header = Self {
            version,
            ihl,
            tos,
            total_length,
            identification,
            flags,
            fragment_offset,
            ttl,
            protocol,
            checksum,
            src,
            dst,
            options,
        };

        Ok((header, header_len))
    }

    /// Serialize header to a mutable byte buffer.
    pub fn serialize(&self, buf: &mut [u8]) -> Result<usize, Ipv4Error> {
        let hlen = self.header_len();
        if buf.len() < hlen {
            return Err(Ipv4Error::BufferTooShort {
                expected: hlen,
                actual: buf.len(),
            });
        }

        buf[0] = ((self.version & 0x0F) << 4) | (self.ihl & 0x0F);
        buf[1] = self.tos;
        buf[2..4].copy_from_slice(&self.total_length.to_be_bytes());
        buf[4..6].copy_from_slice(&self.identification.to_be_bytes());

        let flags_and_frag = ((self.flags as u16 & 0x07) << 13) | (self.fragment_offset & IPV4_FRAG_OFFSET_MASK);
        buf[6..8].copy_from_slice(&flags_and_frag.to_be_bytes());

        buf[8] = self.ttl;
        buf[9] = self.protocol.as_u8();
        buf[10..12].copy_from_slice(&0u16.to_be_bytes()); // Checksum set to 0 for computation

        buf[12..16].copy_from_slice(&self.src.octets());
        buf[16..20].copy_from_slice(&self.dst.octets());

        if !self.options.is_empty() {
            buf[20..hlen].copy_from_slice(&self.options);
        }

        // Calculate checksum
        let csum = compute_internet_checksum(&buf[..hlen]);
        buf[10..12].copy_from_slice(&csum.to_be_bytes());

        Ok(hlen)
    }

    /// Calculate the checksum for this header.
    pub fn calculate_checksum(&self) -> u16 {
        let mut tmp = [0u8; IPV4_MAX_HEADER_LEN];
        let hlen = self.header_len();
        let _ = self.serialize(&mut tmp[..hlen]);
        u16::from_be_bytes([tmp[10], tmp[11]])
    }
}

/// Complete IPv4 packet with parsed header and payload slice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ipv4Packet {
    pub header: Ipv4Header,
    pub payload: Vec<u8>,
}

impl Ipv4Packet {
    /// Create a new IPv4 packet.
    pub fn new(
        src: Ipv4Address,
        dst: Ipv4Address,
        protocol: Ipv4Protocol,
        payload: Vec<u8>,
    ) -> Result<Self, Ipv4Error> {
        let header = Ipv4Header::new(src, dst, protocol, payload.len())?;
        Ok(Self { header, payload })
    }

    /// Parse an IPv4 packet from a byte slice.
    pub fn parse(buf: &[u8]) -> Result<Self, Ipv4Error> {
        let (header, header_len) = Ipv4Header::parse(buf)?;
        let total_len = header.total_length as usize;

        if buf.len() < total_len {
            return Err(Ipv4Error::BufferTooShort {
                expected: total_len,
                actual: buf.len(),
            });
        }

        if total_len < header_len {
            return Err(Ipv4Error::InvalidTotalLength {
                total_len,
                header_len,
            });
        }

        let payload = buf[header_len..total_len].to_vec();
        Ok(Self { header, payload })
    }

    /// Serialize this IPv4 packet into a buffer.
    pub fn serialize(&self, buf: &mut [u8]) -> Result<usize, Ipv4Error> {
        let total_len = self.header.total_length as usize;
        if buf.len() < total_len {
            return Err(Ipv4Error::BufferTooShort {
                expected: total_len,
                actual: buf.len(),
            });
        }

        let header_len = self.header.serialize(buf)?;
        buf[header_len..total_len].copy_from_slice(&self.payload);
        Ok(total_len)
    }

    /// Serialize to a newly allocated byte vector.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = vec![0u8; self.header.total_length as usize];
        let _ = self.serialize(&mut buf);
        buf
    }
}

/// Calculate the standard RFC 1071 Internet Checksum.
///
/// Sums all 16-bit words using ones' complement addition, then takes
/// the bitwise negation of the 16-bit result.
pub fn compute_internet_checksum(data: &[u8]) -> u16 {
    let mut sum: u32 = 0;
    let mut chunks = data.chunks_exact(2);

    for chunk in &mut chunks {
        let word = u16::from_be_bytes([chunk[0], chunk[1]]) as u32;
        sum = sum.wrapping_add(word);
    }

    // Handle trailing odd byte if present (padded with zero as low byte in network order)
    let remainder = chunks.remainder();
    if !remainder.is_empty() {
        let word = (remainder[0] as u32) << 8;
        sum = sum.wrapping_add(word);
    }

    // Fold 32-bit sum into 16-bit sum
    while (sum >> 16) != 0 {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }

    !(sum as u16)
}

/// Verify internet checksum for a block of data that includes the checksum itself.
/// Returns true if valid (checksum verifies to 0).
pub fn verify_internet_checksum(data: &[u8]) -> bool {
    compute_internet_checksum(data) == 0
}

/// CIDR Route entry representing an entry in the kernel routing table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Route {
    /// Destination network address (e.g., 192.168.1.0).
    pub destination: Ipv4Address,
    /// CIDR prefix length in bits (0 to 32).
    pub prefix_len: u8,
    /// Precomputed subnet mask.
    pub netmask: Ipv4Address,
    /// Next-hop gateway IP (None if directly connected to local link).
    pub gateway: Option<Ipv4Address>,
    /// Associated network interface index.
    pub interface_id: usize,
    /// Route metric (lower indicates higher priority).
    pub metric: u32,
}

impl Route {
    /// Create a new CIDR route.
    pub fn new(
        destination: Ipv4Address,
        prefix_len: u8,
        gateway: Option<Ipv4Address>,
        interface_id: usize,
        metric: u32,
    ) -> Result<Self, Ipv4Error> {
        let netmask = Ipv4Address::netmask_from_prefix(prefix_len)?;
        // Ensure destination address is masked to match network
        let masked_dest = Ipv4Address::from_u32(destination.to_u32() & netmask.to_u32());

        Ok(Self {
            destination: masked_dest,
            prefix_len,
            netmask,
            gateway,
            interface_id,
            metric,
        })
    }

    /// Check if a target IP address matches this route.
    pub fn matches(&self, ip: Ipv4Address) -> bool {
        ip.matches_subnet(self.destination, self.netmask)
    }

    /// Returns true if this is the default route (0.0.0.0/0).
    pub fn is_default(&self) -> bool {
        self.prefix_len == 0
    }
}

/// Kernel CIDR IPv4 Routing Table with Longest Prefix Match (LPM).
#[derive(Debug, Clone, Default)]
pub struct RoutingTable {
    routes: Vec<Route>,
}

impl RoutingTable {
    /// Create an empty routing table.
    pub fn new() -> Self {
        Self { routes: Vec::new() }
    }

    /// Add a route to the table.
    pub fn add_route(&mut self, route: Route) {
        // If a route with the exact destination and prefix length exists, replace it
        if let Some(pos) = self
            .routes
            .iter()
            .position(|r| r.destination == route.destination && r.prefix_len == route.prefix_len)
        {
            self.routes[pos] = route;
        } else {
            self.routes.push(route);
        }

        // Keep routes sorted primarily by prefix_len descending (for LPM), secondarily by metric ascending
        self.routes.sort_by(|a, b| {
            b.prefix_len
                .cmp(&a.prefix_len)
                .then_with(|| a.metric.cmp(&b.metric))
        });
    }

    /// Helper to add a directly attached local network route.
    pub fn add_direct(
        &mut self,
        destination: Ipv4Address,
        prefix_len: u8,
        interface_id: usize,
        metric: u32,
    ) -> Result<(), Ipv4Error> {
        let route = Route::new(destination, prefix_len, None, interface_id, metric)?;
        self.add_route(route);
        Ok(())
    }

    /// Helper to add a route via next-hop gateway.
    pub fn add_gateway(
        &mut self,
        destination: Ipv4Address,
        prefix_len: u8,
        gateway: Ipv4Address,
        interface_id: usize,
        metric: u32,
    ) -> Result<(), Ipv4Error> {
        let route = Route::new(destination, prefix_len, Some(gateway), interface_id, metric)?;
        self.add_route(route);
        Ok(())
    }

    /// Configure the default gateway (0.0.0.0/0).
    pub fn set_default_gateway(
        &mut self,
        gateway: Ipv4Address,
        interface_id: usize,
    ) -> Result<(), Ipv4Error> {
        self.add_gateway(Ipv4Address::ANY, 0, gateway, interface_id, 100)
    }

    /// Look up the best matching route for a destination address using Longest Prefix Match (LPM).
    pub fn lookup(&self, dest: Ipv4Address) -> Option<&Route> {
        // Since routes are pre-sorted by prefix_len DESC, metric ASC,
        // the first matching route is guaranteed to be the longest prefix match with lowest metric.
        self.routes.iter().find(|r| r.matches(dest))
    }

    /// Remove a route matching destination network and prefix length.
    pub fn remove_route(&mut self, destination: Ipv4Address, prefix_len: u8) -> bool {
        let before = self.routes.len();
        self.routes
            .retain(|r| !(r.destination == destination && r.prefix_len == prefix_len));
        self.routes.len() < before
    }

    /// Access all active routes.
    pub fn all_routes(&self) -> &[Route] {
        &self.routes
    }

    /// Clear all routes.
    pub fn clear(&mut self) {
        self.routes.clear();
    }
}

/// IPv4 protocol and routing error states.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ipv4Error {
    BufferTooShort { expected: usize, actual: usize },
    InvalidVersion(u8),
    InvalidIhl(u8),
    InvalidTotalLength { total_len: usize, header_len: usize },
    ChecksumMismatch { expected: u16, calculated: u16 },
    InvalidPrefixLength(u8),
    NonContiguousNetmask(Ipv4Address),
}

impl fmt::Display for Ipv4Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BufferTooShort { expected, actual } => {
                write!(f, "IPv4 buffer too short: expected {} bytes, got {}", expected, actual)
            }
            Self::InvalidVersion(ver) => write!(f, "Invalid IPv4 version (expected 4, got {})", ver),
            Self::InvalidIhl(ihl) => write!(f, "Invalid IPv4 IHL (must be >= 5, got {})", ihl),
            Self::InvalidTotalLength { total_len, header_len } => {
                write!(f, "Total length ({}) less than header length ({})", total_len, header_len)
            }
            Self::ChecksumMismatch { expected, calculated } => {
                write!(f, "IPv4 checksum mismatch: expected 0x{:04X}, got 0x{:04X}", expected, calculated)
            }
            Self::InvalidPrefixLength(len) => write!(f, "Invalid CIDR prefix length ({} > 32)", len),
            Self::NonContiguousNetmask(mask) => write!(f, "Netmask {} is non-contiguous", mask),
        }
    }
}

impl std::error::Error for Ipv4Error {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ipv4_address_basics() {
        let ip = Ipv4Address::new(192, 168, 1, 100);
        assert_eq!(ip.to_string(), "192.168.1.100");
        assert!(ip.is_private());
        assert!(!ip.is_loopback());
        assert!(!ip.is_multicast());

        let parsed = Ipv4Address::parse("10.0.0.1").unwrap();
        assert_eq!(parsed.octets(), [10, 0, 0, 1]);
        assert!(parsed.is_private());

        let lo = Ipv4Address::LOOPBACK;
        assert!(lo.is_loopback());

        let mcast = Ipv4Address::new(224, 0, 0, 251);
        assert!(mcast.is_multicast());
    }

    #[test]
    fn test_cidr_netmask_generation() {
        let mask24 = Ipv4Address::netmask_from_prefix(24).unwrap();
        assert_eq!(mask24, Ipv4Address::new(255, 255, 255, 0));
        assert_eq!(mask24.to_prefix_length().unwrap(), 24);

        let mask16 = Ipv4Address::netmask_from_prefix(16).unwrap();
        assert_eq!(mask16, Ipv4Address::new(255, 255, 0, 0));
        assert_eq!(mask16.to_prefix_length().unwrap(), 16);

        let mask0 = Ipv4Address::netmask_from_prefix(0).unwrap();
        assert_eq!(mask0, Ipv4Address::new(0, 0, 0, 0));
        assert_eq!(mask0.to_prefix_length().unwrap(), 0);

        assert!(Ipv4Address::netmask_from_prefix(33).is_err());
    }

    #[test]
    fn test_rfc1071_checksum_calculation() {
        // Standard RFC 1071 verification vector
        let data = [0x45, 0x00, 0x00, 0x3c, 0x1c, 0x46, 0x40, 0x00, 0x40, 0x06, 0x00, 0x00, 0xac, 0x10, 0x0a, 0x63, 0xac, 0x10, 0x0a, 0x0c];
        let csum = compute_internet_checksum(&data);
        assert_ne!(csum, 0);

        let mut with_csum = data;
        with_csum[10..12].copy_from_slice(&csum.to_be_bytes());
        assert!(verify_internet_checksum(&with_csum));
    }

    #[test]
    fn test_ipv4_packet_roundtrip() {
        let src = Ipv4Address::new(192, 168, 1, 50);
        let dst = Ipv4Address::new(192, 168, 1, 1);
        let payload = b"HELLO_HYBRIDOS_NETWORKING".to_vec();

        let packet = Ipv4Packet::new(src, dst, Ipv4Protocol::Udp, payload.clone()).unwrap();
        let bytes = packet.to_bytes();

        let parsed = Ipv4Packet::parse(&bytes).expect("Packet parsed successfully");
        assert_eq!(parsed.header.src, src);
        assert_eq!(parsed.header.dst, dst);
        assert_eq!(parsed.header.protocol, Ipv4Protocol::Udp);
        assert_eq!(parsed.payload, payload);
    }

    #[test]
    fn test_routing_table_lpm() {
        let mut table = RoutingTable::new();

        // 1. Default gateway (0.0.0.0/0) -> interface 0, gateway 192.168.1.1
        table.set_default_gateway(Ipv4Address::new(192, 168, 1, 1), 0).unwrap();

        // 2. Direct local subnet (192.168.1.0/24) -> interface 0
        table.add_direct(Ipv4Address::new(192, 168, 1, 0), 24, 0, 10).unwrap();

        // 3. More specific host route (192.168.1.50/32) -> interface 1
        table.add_direct(Ipv4Address::new(192, 168, 1, 50), 32, 1, 5).unwrap();

        // 4. Another subnet (10.0.0.0/8) -> interface 2
        table.add_direct(Ipv4Address::new(10, 0, 0, 0), 8, 2, 20).unwrap();

        // Test LPM: 192.168.1.50 should match the /32 route (interface 1)
        let r1 = table.lookup(Ipv4Address::new(192, 168, 1, 50)).unwrap();
        assert_eq!(r1.prefix_len, 32);
        assert_eq!(r1.interface_id, 1);

        // Test LPM: 192.168.1.20 should match the /24 route (interface 0)
        let r2 = table.lookup(Ipv4Address::new(192, 168, 1, 20)).unwrap();
        assert_eq!(r2.prefix_len, 24);
        assert_eq!(r2.interface_id, 0);

        // Test LPM: 10.5.4.3 should match the /8 route (interface 2)
        let r3 = table.lookup(Ipv4Address::new(10, 5, 4, 3)).unwrap();
        assert_eq!(r3.prefix_len, 8);
        assert_eq!(r3.interface_id, 2);

        // Test LPM: 8.8.8.8 should match default route (interface 0, gateway 192.168.1.1)
        let r4 = table.lookup(Ipv4Address::new(8, 8, 8, 8)).unwrap();
        assert_eq!(r4.prefix_len, 0);
        assert_eq!(r4.gateway, Some(Ipv4Address::new(192, 168, 1, 1)));
    }
}
