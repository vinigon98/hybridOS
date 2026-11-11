//! HybridOS Network Layer - Internet Control Message Protocol (ICMP) & Ping
//!
//! Implements RFC 792 ICMP protocol, Echo Request/Reply generation and parsing,
//! destination unreachable messages, internet checksum computation, and echo reply handling.

#[allow(unused_imports)]
use alloc::string::{String, ToString};
#[allow(unused_imports)]
use alloc::vec::Vec;
#[allow(unused_imports)]
use alloc::boxed::Box;
#[allow(unused_imports)]
use crate::compat::FloatExt;
use std::fmt;

use super::ipv4::{compute_internet_checksum, verify_internet_checksum, Ipv4Address, Ipv4Packet, Ipv4Protocol};

/// Length of the ICMP message header (8 bytes).
pub const ICMP_HEADER_LEN: usize = 8;

/// Known ICMP message types (RFC 792).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IcmpType {
    /// Echo Reply (Type 0)
    EchoReply,
    /// Destination Unreachable (Type 3)
    DestinationUnreachable,
    /// Source Quench (Type 4)
    SourceQuench,
    /// Redirect (Type 5)
    Redirect,
    /// Echo Request (Type 8)
    EchoRequest,
    /// Router Advertisement (Type 9)
    RouterAdvertisement,
    /// Router Solicitation (Type 10)
    RouterSolicitation,
    /// Time Exceeded (Type 11)
    TimeExceeded,
    /// Parameter Problem (Type 12)
    ParameterProblem,
    /// Timestamp (Type 13)
    Timestamp,
    /// Timestamp Reply (Type 14)
    TimestampReply,
    /// Unrecognized ICMP type
    Unknown(u8),
}

impl IcmpType {
    pub const ECHO_REPLY_RAW: u8 = 0;
    pub const DEST_UNREACHABLE_RAW: u8 = 3;
    pub const SOURCE_QUENCH_RAW: u8 = 4;
    pub const REDIRECT_RAW: u8 = 5;
    pub const ECHO_REQUEST_RAW: u8 = 8;
    pub const ROUTER_ADV_RAW: u8 = 9;
    pub const ROUTER_SOL_RAW: u8 = 10;
    pub const TIME_EXCEEDED_RAW: u8 = 11;
    pub const PARAM_PROBLEM_RAW: u8 = 12;
    pub const TIMESTAMP_RAW: u8 = 13;
    pub const TIMESTAMP_REPLY_RAW: u8 = 14;

    pub fn from_u8(val: u8) -> Self {
        match val {
            Self::ECHO_REPLY_RAW => Self::EchoReply,
            Self::DEST_UNREACHABLE_RAW => Self::DestinationUnreachable,
            Self::SOURCE_QUENCH_RAW => Self::SourceQuench,
            Self::REDIRECT_RAW => Self::Redirect,
            Self::ECHO_REQUEST_RAW => Self::EchoRequest,
            Self::ROUTER_ADV_RAW => Self::RouterAdvertisement,
            Self::ROUTER_SOL_RAW => Self::RouterSolicitation,
            Self::TIME_EXCEEDED_RAW => Self::TimeExceeded,
            Self::PARAM_PROBLEM_RAW => Self::ParameterProblem,
            Self::TIMESTAMP_RAW => Self::Timestamp,
            Self::TIMESTAMP_REPLY_RAW => Self::TimestampReply,
            other => Self::Unknown(other),
        }
    }

    pub fn as_u8(&self) -> u8 {
        match self {
            Self::EchoReply => Self::ECHO_REPLY_RAW,
            Self::DestinationUnreachable => Self::DEST_UNREACHABLE_RAW,
            Self::SourceQuench => Self::SOURCE_QUENCH_RAW,
            Self::Redirect => Self::REDIRECT_RAW,
            Self::EchoRequest => Self::ECHO_REQUEST_RAW,
            Self::RouterAdvertisement => Self::ROUTER_ADV_RAW,
            Self::RouterSolicitation => Self::ROUTER_SOL_RAW,
            Self::TimeExceeded => Self::TIME_EXCEEDED_RAW,
            Self::ParameterProblem => Self::PARAM_PROBLEM_RAW,
            Self::Timestamp => Self::TIMESTAMP_RAW,
            Self::TimestampReply => Self::TIMESTAMP_REPLY_RAW,
            Self::Unknown(val) => *val,
        }
    }
}

impl fmt::Display for IcmpType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EchoReply => write!(f, "Echo Reply (0)"),
            Self::DestinationUnreachable => write!(f, "Destination Unreachable (3)"),
            Self::SourceQuench => write!(f, "Source Quench (4)"),
            Self::Redirect => write!(f, "Redirect (5)"),
            Self::EchoRequest => write!(f, "Echo Request (8)"),
            Self::RouterAdvertisement => write!(f, "Router Advertisement (9)"),
            Self::RouterSolicitation => write!(f, "Router Solicitation (10)"),
            Self::TimeExceeded => write!(f, "Time Exceeded (11)"),
            Self::ParameterProblem => write!(f, "Parameter Problem (12)"),
            Self::Timestamp => write!(f, "Timestamp (13)"),
            Self::TimestampReply => write!(f, "Timestamp Reply (14)"),
            Self::Unknown(val) => write!(f, "Unknown ICMP Type ({})", val),
        }
    }
}

/// Destination Unreachable Codes (Type 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DestUnreachableCode {
    NetUnreachable = 0,
    HostUnreachable = 1,
    ProtocolUnreachable = 2,
    PortUnreachable = 3,
    FragmentationNeeded = 4,
    SourceRouteFailed = 5,
    DestinationNetworkUnknown = 6,
    DestinationHostUnknown = 7,
}

/// Echo (Ping) Header parameters extracted from `rest_of_header`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IcmpEchoHeader {
    /// Echo session identifier (e.g., process ID or random nonce).
    pub identifier: u16,
    /// Echo sequence number (incrementing packet counter).
    pub sequence_number: u16,
}

/// ICMP Header according to RFC 792.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IcmpHeader {
    /// ICMP control message type.
    pub icmp_type: IcmpType,
    /// Sub-code specific to message type.
    pub code: u8,
    /// 16-bit internet checksum covering header and data.
    pub checksum: u16,
    /// 4-byte message-specific header data (identifier/sequence for Echo).
    pub rest_of_header: [u8; 4],
}

impl IcmpHeader {
    /// Create a new ICMP header.
    pub fn new(icmp_type: IcmpType, code: u8, rest_of_header: [u8; 4]) -> Self {
        Self {
            icmp_type,
            code,
            checksum: 0,
            rest_of_header,
        }
    }

    /// Extract Echo identification and sequence number if applicable.
    pub fn echo_info(&self) -> Option<IcmpEchoHeader> {
        match self.icmp_type {
            IcmpType::EchoRequest | IcmpType::EchoReply => {
                let identifier = u16::from_be_bytes([self.rest_of_header[0], self.rest_of_header[1]]);
                let sequence_number = u16::from_be_bytes([self.rest_of_header[2], self.rest_of_header[3]]);
                Some(IcmpEchoHeader {
                    identifier,
                    sequence_number,
                })
            }
            _ => None,
        }
    }
}

/// Complete ICMP packet with header and payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IcmpPacket {
    pub header: IcmpHeader,
    pub payload: Vec<u8>,
}

impl IcmpPacket {
    /// Construct a ping ICMP Echo Request packet (Type 8, Code 0).
    pub fn build_echo_request(identifier: u16, sequence_number: u16, payload: &[u8]) -> Self {
        let mut rest = [0u8; 4];
        rest[0..2].copy_from_slice(&identifier.to_be_bytes());
        rest[2..4].copy_from_slice(&sequence_number.to_be_bytes());

        let mut pkt = Self {
            header: IcmpHeader::new(IcmpType::EchoRequest, 0, rest),
            payload: payload.to_vec(),
        };

        pkt.header.checksum = pkt.calculate_checksum();
        pkt
    }

    /// Construct an ICMP Echo Reply (Type 0, Code 0) echoing a received Echo Request.
    pub fn build_echo_reply(request: &IcmpPacket) -> Result<Self, IcmpError> {
        if request.header.icmp_type != IcmpType::EchoRequest {
            return Err(IcmpError::NotAnEchoRequest(request.header.icmp_type));
        }

        let mut pkt = Self {
            header: IcmpHeader::new(IcmpType::EchoReply, 0, request.header.rest_of_header),
            payload: request.payload.clone(),
        };

        pkt.header.checksum = pkt.calculate_checksum();
        Ok(pkt)
    }

    /// Construct a Destination Unreachable ICMP packet (Type 3).
    pub fn build_dest_unreachable(code: DestUnreachableCode, orig_datagram: &[u8]) -> Self {
        let rest = [0u8; 4]; // Next-hop MTU can be placed in rest[2..4] for code 4 if needed
        let mut pkt = Self {
            header: IcmpHeader::new(IcmpType::DestinationUnreachable, code as u8, rest),
            payload: orig_datagram.to_vec(),
        };

        pkt.header.checksum = pkt.calculate_checksum();
        pkt
    }

    /// Calculate RFC 1071 internet checksum covering header and payload.
    pub fn calculate_checksum(&self) -> u16 {
        let mut buf = vec![0u8; ICMP_HEADER_LEN + self.payload.len()];
        buf[0] = self.header.icmp_type.as_u8();
        buf[1] = self.header.code;
        buf[2..4].copy_from_slice(&0u16.to_be_bytes()); // Checksum zeroed during calculation
        buf[4..8].copy_from_slice(&self.header.rest_of_header);
        buf[8..].copy_from_slice(&self.payload);

        compute_internet_checksum(&buf)
    }

    /// Parse an ICMP packet from a byte buffer.
    pub fn parse(buf: &[u8]) -> Result<Self, IcmpError> {
        if buf.len() < ICMP_HEADER_LEN {
            return Err(IcmpError::BufferTooShort {
                expected: ICMP_HEADER_LEN,
                actual: buf.len(),
            });
        }

        // Verify checksum over the entire ICMP message
        if !verify_internet_checksum(buf) {
            let received_csum = u16::from_be_bytes([buf[2], buf[3]]);
            return Err(IcmpError::ChecksumMismatch {
                received: received_csum,
            });
        }

        let icmp_type = IcmpType::from_u8(buf[0]);
        let code = buf[1];
        let checksum = u16::from_be_bytes([buf[2], buf[3]]);
        let mut rest_of_header = [0u8; 4];
        rest_of_header.copy_from_slice(&buf[4..8]);

        let payload = buf[ICMP_HEADER_LEN..].to_vec();

        Ok(Self {
            header: IcmpHeader {
                icmp_type,
                code,
                checksum,
                rest_of_header,
            },
            payload,
        })
    }

    /// Serialize this ICMP packet to a byte buffer.
    pub fn serialize(&self, buf: &mut [u8]) -> Result<usize, IcmpError> {
        let total_len = ICMP_HEADER_LEN + self.payload.len();
        if buf.len() < total_len {
            return Err(IcmpError::BufferTooShort {
                expected: total_len,
                actual: buf.len(),
            });
        }

        buf[0] = self.header.icmp_type.as_u8();
        buf[1] = self.header.code;
        buf[2..4].copy_from_slice(&0u16.to_be_bytes());
        buf[4..8].copy_from_slice(&self.header.rest_of_header);
        buf[8..total_len].copy_from_slice(&self.payload);

        let csum = compute_internet_checksum(&buf[..total_len]);
        buf[2..4].copy_from_slice(&csum.to_be_bytes());

        Ok(total_len)
    }

    /// Serialize to a newly allocated byte vector.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = vec![0u8; ICMP_HEADER_LEN + self.payload.len()];
        let _ = self.serialize(&mut buf);
        buf
    }

    /// Helper to get echo info if available.
    pub fn echo_info(&self) -> Option<IcmpEchoHeader> {
        self.header.echo_info()
    }
}

/// ICMP Echo Request Packet Handler.
///
/// When an incoming IPv4 packet contains an ICMP Echo Request, this function parses it,
/// verifies its checksum, and constructs an IPv4 packet encapsulating the corresponding
/// ICMP Echo Reply addressed back to the sender.
pub fn handle_echo_request(
    src_ip: Ipv4Address,
    dst_ip: Ipv4Address,
    icmp_data: &[u8],
) -> Result<Option<Ipv4Packet>, IcmpError> {
    let req = IcmpPacket::parse(icmp_data)?;

    if req.header.icmp_type == IcmpType::EchoRequest {
        let reply = IcmpPacket::build_echo_reply(&req)?;
        let reply_payload = reply.to_bytes();

        let ipv4_packet = Ipv4Packet::new(dst_ip, src_ip, Ipv4Protocol::Icmp, reply_payload)
            .map_err(|e| IcmpError::Ipv4EncapsulationError(format!("{:?}", e)))?;

        Ok(Some(ipv4_packet))
    } else {
        Ok(None)
    }
}

/// Errors related to ICMP operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IcmpError {
    BufferTooShort { expected: usize, actual: usize },
    ChecksumMismatch { received: u16 },
    NotAnEchoRequest(IcmpType),
    Ipv4EncapsulationError(String),
}

impl fmt::Display for IcmpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BufferTooShort { expected, actual } => {
                write!(f, "ICMP buffer too short: expected {} bytes, got {}", expected, actual)
            }
            Self::ChecksumMismatch { received } => {
                write!(f, "ICMP checksum verification failed (received 0x{:04X})", received)
            }
            Self::NotAnEchoRequest(t) => {
                write!(f, "Expected Echo Request (8), but found {}", t)
            }
            Self::Ipv4EncapsulationError(msg) => {
                write!(f, "Failed to encapsulate ICMP in IPv4: {}", msg)
            }
        }
    }
}

impl std::error::Error for IcmpError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_icmp_echo_request_build_and_parse() {
        let payload = b"HybridOS_Echo_Ping_Payload";
        let req = IcmpPacket::build_echo_request(0xCAFE, 1, payload);

        let bytes = req.to_bytes();
        assert_eq!(bytes.len(), ICMP_HEADER_LEN + payload.len());

        let parsed = IcmpPacket::parse(&bytes).expect("Parsed ICMP request");
        assert_eq!(parsed.header.icmp_type, IcmpType::EchoRequest);
        assert_eq!(parsed.header.code, 0);

        let echo_info = parsed.echo_info().expect("Echo info present");
        assert_eq!(echo_info.identifier, 0xCAFE);
        assert_eq!(echo_info.sequence_number, 1);
        assert_eq!(parsed.payload, payload);
    }

    #[test]
    fn test_icmp_echo_reply_handler() {
        let client_ip = Ipv4Address::new(192, 168, 1, 50);
        let host_ip = Ipv4Address::new(192, 168, 1, 1);

        let ping_payload = b"PING_PONG_VALIDATION";
        let req = IcmpPacket::build_echo_request(1234, 42, ping_payload);
        let req_bytes = req.to_bytes();

        let response = handle_echo_request(client_ip, host_ip, &req_bytes)
            .expect("Handle echo ok")
            .expect("Reply generated");

        assert_eq!(response.header.src, host_ip);
        assert_eq!(response.header.dst, client_ip);
        assert_eq!(response.header.protocol, Ipv4Protocol::Icmp);

        let reply_icmp = IcmpPacket::parse(&response.payload).expect("Parsed reply payload");
        assert_eq!(reply_icmp.header.icmp_type, IcmpType::EchoReply);
        let echo = reply_icmp.echo_info().unwrap();
        assert_eq!(echo.identifier, 1234);
        assert_eq!(echo.sequence_number, 42);
        assert_eq!(reply_icmp.payload, ping_payload);
    }

    #[test]
    fn test_icmp_corrupt_checksum_fails() {
        let req = IcmpPacket::build_echo_request(1, 1, b"DATA");
        let mut bytes = req.to_bytes();
        bytes[2] ^= 0xFF; // Corrupt checksum byte

        assert!(matches!(
            IcmpPacket::parse(&bytes),
            Err(IcmpError::ChecksumMismatch { .. })
        ));
    }
}
