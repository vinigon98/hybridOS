//! HyAV Network Policy Engine (`net_policy.rs`)
//!
//! Provides firewall policy evaluation across Domain, Private, and Public network profiles,
//! active threat response actions (TCP RST synthesis, temporary IP ban with TTL expiration),
//! and audit logging via JSON-Lines telemetry (Windows Event IDs 1117 and 1118).

#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::fmt;
use std::net::{IpAddr, Ipv4Addr};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

// ============================================================================
// 1. Network Profiles & Traffic Primitives
// ============================================================================

/// Windows-compatible firewall network profiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NetworkProfile {
    Domain,
    Private,
    Public,
}

impl NetworkProfile {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Domain => "Domain",
            Self::Private => "Private",
            Self::Public => "Public",
        }
    }

    pub fn from_str_name(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "domain" => Some(Self::Domain),
            "private" => Some(Self::Private),
            "public" => Some(Self::Public),
            _ => None,
        }
    }
}

impl fmt::Display for NetworkProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Network traffic direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TrafficDirection {
    Inbound,
    Outbound,
}

impl TrafficDirection {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Inbound => "Inbound",
            Self::Outbound => "Outbound",
        }
    }
}

impl fmt::Display for TrafficDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Supported IP transport protocols.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IpProtocol {
    Tcp,
    Udp,
    Icmp,
    Other(u8),
}

impl IpProtocol {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Tcp => "TCP",
            Self::Udp => "UDP",
            Self::Icmp => "ICMP",
            Self::Other(_) => "OTHER",
        }
    }

    pub const fn iana_number(&self) -> u8 {
        match self {
            Self::Tcp => 6,
            Self::Udp => 17,
            Self::Icmp => 1,
            Self::Other(n) => *n,
        }
    }
}

impl fmt::Display for IpProtocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Captured connection or packet metadata evaluated by the policy engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkConnection {
    pub direction: TrafficDirection,
    pub protocol: IpProtocol,
    pub src_ip: IpAddr,
    pub src_port: u16,
    pub dst_ip: IpAddr,
    pub dst_port: u16,
    pub pid: Option<u32>,
    pub process_path: Option<String>,
    pub tcp_seq: u32,
    pub tcp_ack: u32,
    pub tcp_flags: u8,
}

impl NetworkConnection {
    pub fn new_tcp(
        direction: TrafficDirection,
        src_ip: IpAddr,
        src_port: u16,
        dst_ip: IpAddr,
        dst_port: u16,
        tcp_seq: u32,
        tcp_ack: u32,
        tcp_flags: u8,
    ) -> Self {
        Self {
            direction,
            protocol: IpProtocol::Tcp,
            src_ip,
            src_port,
            dst_ip,
            dst_port,
            pid: None,
            process_path: None,
            tcp_seq,
            tcp_ack,
            tcp_flags,
        }
    }

    pub fn new_udp(
        direction: TrafficDirection,
        src_ip: IpAddr,
        src_port: u16,
        dst_ip: IpAddr,
        dst_port: u16,
    ) -> Self {
        Self {
            direction,
            protocol: IpProtocol::Udp,
            src_ip,
            src_port,
            dst_ip,
            dst_port,
            pid: None,
            process_path: None,
            tcp_seq: 0,
            tcp_ack: 0,
            tcp_flags: 0,
        }
    }

    pub fn with_process(mut self, pid: u32, path: impl Into<String>) -> Self {
        self.pid = Some(pid);
        self.process_path = Some(path.into());
        self
    }
}

// ============================================================================
// 2. Rules, Matchers, and Actions
// ============================================================================

/// IP address matching rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IpMatcher {
    Any,
    Exact(IpAddr),
    CidrV4(Ipv4Addr, u8),
}

impl IpMatcher {
    pub fn matches(&self, ip: &IpAddr) -> bool {
        match self {
            Self::Any => true,
            Self::Exact(target) => target == ip,
            Self::CidrV4(net, prefix) => {
                if let IpAddr::V4(v4) = ip {
                    if *prefix == 0 {
                        return true;
                    }
                    if *prefix > 32 {
                        return false;
                    }
                    let mask: u32 = !((1u64 << (32 - prefix)) - 1) as u32;
                    let net_u32 = u32::from_be_bytes(net.octets());
                    let ip_u32 = u32::from_be_bytes(v4.octets());
                    (ip_u32 & mask) == (net_u32 & mask)
                } else {
                    false
                }
            }
        }
    }
}

/// Port matching criteria.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PortMatcher {
    Any,
    Exact(u16),
    Range(u16, u16),
    List(Vec<u16>),
}

impl PortMatcher {
    pub fn matches(&self, port: u16) -> bool {
        match self {
            Self::Any => true,
            Self::Exact(p) => *p == port,
            Self::Range(start, end) => port >= *start && port <= *end,
            Self::List(ports) => ports.contains(&port),
        }
    }
}

/// Action specified by a rule definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleAction {
    Allow,
    Drop,
    RejectTcpRst,
    BanIp { duration_secs: u64, reason: String },
    AuditOnly,
}

/// Final verdict resolved after evaluating a connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResponseVerdict {
    Allow,
    Drop {
        rule_id: String,
        reason: String,
    },
    TcpRst {
        rule_id: String,
        reason: String,
        rst_packet: Vec<u8>,
    },
    Banned {
        rule_id: String,
        banned_ip: IpAddr,
        duration_secs: u64,
        expires_at: u64,
        reason: String,
    },
}

/// A configured firewall rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FirewallRule {
    pub id: String,
    pub name: String,
    pub priority: u32,
    pub enabled: bool,
    pub profiles: Vec<NetworkProfile>,
    pub direction: Option<TrafficDirection>,
    pub protocol: Option<IpProtocol>,
    pub remote_ip: IpMatcher,
    pub remote_port: PortMatcher,
    pub local_port: PortMatcher,
    pub action: RuleAction,
    pub reason: String,
}

impl FirewallRule {
    pub fn matches(&self, active_profile: NetworkProfile, conn: &NetworkConnection) -> bool {
        if !self.enabled {
            return false;
        }

        if !self.profiles.contains(&active_profile) {
            return false;
        }

        if let Some(dir) = self.direction {
            if dir != conn.direction {
                return false;
            }
        }

        if let Some(proto) = self.protocol {
            if proto != conn.protocol {
                return false;
            }
        }

        let (remote_ip, remote_port, local_port) = match conn.direction {
            TrafficDirection::Inbound => (conn.src_ip, conn.src_port, conn.dst_port),
            TrafficDirection::Outbound => (conn.dst_ip, conn.dst_port, conn.src_port),
        };

        if !self.remote_ip.matches(&remote_ip) {
            return false;
        }

        if !self.remote_port.matches(remote_port) {
            return false;
        }

        if !self.local_port.matches(local_port) {
            return false;
        }

        true
    }
}

// ============================================================================
// 3. Time Abstraction & Temporary IP Ban Engine
// ============================================================================

/// Trait to supply seconds epoch time, allowing deterministic unit testing.
pub trait Clock: Send + Sync {
    fn now_secs(&self) -> u64;
}

/// System wall clock.
#[derive(Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_secs(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }
}

/// Mockable clock for time progression tests.
#[derive(Debug)]
pub struct MockClock {
    now: AtomicU64,
}

impl MockClock {
    pub fn new(initial_secs: u64) -> Self {
        Self {
            now: AtomicU64::new(initial_secs),
        }
    }

    pub fn advance(&self, secs: u64) {
        self.now.fetch_add(secs, Ordering::SeqCst);
    }

    pub fn set(&self, secs: u64) {
        self.now.store(secs, Ordering::SeqCst);
    }
}

impl Clock for MockClock {
    fn now_secs(&self) -> u64 {
        self.now.load(Ordering::SeqCst)
    }
}

/// An active or historical IP ban record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BanEntry {
    pub ip: IpAddr,
    pub banned_at: u64,
    pub expires_at: u64,
    pub duration_secs: u64,
    pub reason: String,
    pub hit_count: u32,
}

/// Status of an IP ban lifecycle event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BanLifecycleStatus {
    Active,
    Expired,
    Unbanned,
}

impl BanLifecycleStatus {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Active => "Active",
            Self::Expired => "Expired",
            Self::Unbanned => "Unbanned",
        }
    }
}

/// Manager tracking active temporary IP bans and automatic expiration.
pub struct IpBanManager {
    bans: HashMap<IpAddr, BanEntry>,
    clock: Arc<dyn Clock>,
}

impl IpBanManager {
    pub fn new(clock: Arc<dyn Clock>) -> Self {
        Self {
            bans: HashMap::new(),
            clock,
        }
    }

    pub fn with_system_clock() -> Self {
        Self::new(Arc::new(SystemClock))
    }

    /// Enact a temporary IP ban for `duration_secs`.
    pub fn ban_ip(&mut self, ip: IpAddr, duration_secs: u64, reason: String) -> BanEntry {
        let now = self.clock.now_secs();
        let expires_at = now.saturating_add(duration_secs);

        let entry = BanEntry {
            ip,
            banned_at: now,
            expires_at,
            duration_secs,
            reason,
            hit_count: 1,
        };

        self.bans.insert(ip, entry.clone());
        entry
    }

    /// Check if an IP is currently banned. If banned, increments hit count and returns entry.
    pub fn check_and_record_hit(&mut self, ip: &IpAddr) -> Option<BanEntry> {
        let now = self.clock.now_secs();

        if let Some(entry) = self.bans.get_mut(ip) {
            if now < entry.expires_at {
                entry.hit_count = entry.hit_count.saturating_add(1);
                Some(entry.clone())
            } else {
                // Expired on check
                None
            }
        } else {
            None
        }
    }

    /// Remove all expired bans and return the list of expired entries.
    pub fn cleanup_expired_bans(&mut self) -> Vec<BanEntry> {
        let now = self.clock.now_secs();
        let mut expired = Vec::new();

        self.bans.retain(|_, entry| {
            if now >= entry.expires_at {
                expired.push(entry.clone());
                false
            } else {
                true
            }
        });

        expired
    }

    /// Explicitly unban an IP address.
    pub fn unban_ip(&mut self, ip: &IpAddr) -> Option<BanEntry> {
        self.bans.remove(ip)
    }

    /// Number of active bans currently held.
    pub fn active_ban_count(&self) -> usize {
        let now = self.clock.now_secs();
        self.bans.values().filter(|e| now < e.expires_at).count()
    }

    pub fn get_ban(&self, ip: &IpAddr) -> Option<&BanEntry> {
        self.bans.get(ip)
    }
}

// ============================================================================
// 4. TCP RST Packet Synthesis (RFC 793 / RFC 1071)
// ============================================================================

/// Calculates the standard RFC 1071 16-bit one's complement Internet Checksum.
pub fn calculate_checksum(data: &[u8]) -> u16 {
    let mut sum: u32 = 0;
    let mut chunks = data.chunks_exact(2);

    for chunk in chunks.by_ref() {
        let word = u16::from_be_bytes([chunk[0], chunk[1]]);
        sum = sum.wrapping_add(word as u32);
    }

    let remainder = chunks.remainder();
    if !remainder.is_empty() {
        let word = u16::from_be_bytes([remainder[0], 0]);
        sum = sum.wrapping_add(word as u32);
    }

    while (sum >> 16) != 0 {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }

    !(sum as u16)
}

/// Synthesize a raw IPv4 TCP RST packet in pure safe Rust.
///
/// Inverts source and destination IPs/ports, sets proper sequence/acknowledgment numbers,
/// asserts the RST (0x04) or RST+ACK (0x14) flag, and computes both IPv4 and TCP checksums.
pub fn synthesize_ipv4_tcp_rst(conn: &NetworkConnection) -> Vec<u8> {
    let (src_v4, dst_v4) = match (conn.dst_ip, conn.src_ip) {
        (IpAddr::V4(s), IpAddr::V4(d)) => (s, d),
        _ => (Ipv4Addr::new(127, 0, 0, 1), Ipv4Addr::new(127, 0, 0, 1)),
    };

    let rst_src_port = conn.dst_port;
    let rst_dst_port = conn.src_port;

    // TCP Sequence and Acknowledgment logic (RFC 793):
    // If incoming packet had ACK set: seq = incoming ack, flags = RST (0x04)
    // If incoming packet did NOT have ACK set: seq = 0, ack = incoming seq + 1, flags = RST | ACK (0x14)
    let has_ack = (conn.tcp_flags & 0x10) != 0;
    let (seq_num, ack_num, flags) = if has_ack {
        (conn.tcp_ack, 0, 0x04u8) // RST
    } else {
        (0, conn.tcp_seq.wrapping_add(1), 0x14u8) // RST | ACK
    };

    let mut packet = vec![0u8; 40]; // 20 bytes IPv4 + 20 bytes TCP

    // --- IPv4 Header (20 bytes) ---
    packet[0] = 0x45; // Version 4, IHL 5
    packet[1] = 0x00; // DSCP / ECN
    packet[2..4].copy_from_slice(&40u16.to_be_bytes()); // Total length = 40
    packet[4..6].copy_from_slice(&0xBEEFu16.to_be_bytes()); // Identification
    packet[6..8].copy_from_slice(&0x4000u16.to_be_bytes()); // Flags = Don't Fragment
    packet[8] = 64; // TTL = 64
    packet[9] = 6; // Protocol = TCP (6)
    packet[10..12].copy_from_slice(&[0, 0]); // Zero checksum for calculation
    packet[12..16].copy_from_slice(&src_v4.octets());
    packet[16..20].copy_from_slice(&dst_v4.octets());

    let ip_csum = calculate_checksum(&packet[0..20]);
    packet[10..12].copy_from_slice(&ip_csum.to_be_bytes());

    // --- TCP Header (20 bytes) ---
    packet[20..22].copy_from_slice(&rst_src_port.to_be_bytes());
    packet[22..24].copy_from_slice(&rst_dst_port.to_be_bytes());
    packet[24..28].copy_from_slice(&seq_num.to_be_bytes());
    packet[28..32].copy_from_slice(&ack_num.to_be_bytes());
    packet[32] = 0x50; // Data offset: 5 32-bit words (20 bytes), reserved: 0
    packet[33] = flags; // RST or RST|ACK
    packet[34..36].copy_from_slice(&0u16.to_be_bytes()); // Window size = 0
    packet[36..38].copy_from_slice(&[0, 0]); // TCP Checksum placeholder
    packet[38..40].copy_from_slice(&0u16.to_be_bytes()); // Urgent pointer = 0

    // TCP Checksum calculation with 12-byte Pseudo-Header:
    // Source IP (4), Dest IP (4), Zero (1), Protocol (1), TCP Segment Length (2)
    let mut pseudo_header = Vec::with_capacity(12 + 20);
    pseudo_header.extend_from_slice(&src_v4.octets());
    pseudo_header.extend_from_slice(&dst_v4.octets());
    pseudo_header.push(0);
    pseudo_header.push(6); // Protocol TCP
    pseudo_header.extend_from_slice(&20u16.to_be_bytes()); // TCP length
    pseudo_header.extend_from_slice(&packet[20..40]); // TCP header

    let tcp_csum = calculate_checksum(&pseudo_header);
    packet[36..38].copy_from_slice(&tcp_csum.to_be_bytes());

    packet
}

// ============================================================================
// 5. JSON-Lines Telemetry & Audit (Event IDs 1117 & 1118)
// ============================================================================

/// Helper to escape JSON string values safely.
fn escape_json_str(input: &str) -> String {
    let mut out = String::with_capacity(input.len() + 8);
    for ch in input.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out
}

/// Windows Security Audit Event 1117:
/// "Network Policy Enforcement Action Taken / Network Threat Blocked".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event1117NetworkAction {
    pub timestamp: String,
    pub profile: NetworkProfile,
    pub action: String,
    pub direction: TrafficDirection,
    pub protocol: IpProtocol,
    pub src_ip: IpAddr,
    pub src_port: u16,
    pub dst_ip: IpAddr,
    pub dst_port: u16,
    pub rule_id: String,
    pub reason: String,
    pub pid: Option<u32>,
    pub process_path: Option<String>,
}

impl Event1117NetworkAction {
    /// Formats this event as a single line JSON string (JSON-Lines compatible).
    pub fn to_json_line(&self) -> String {
        let pid_str = match self.pid {
            Some(p) => p.to_string(),
            None => "null".to_string(),
        };

        let path_str = match &self.process_path {
            Some(p) => format!("\"{}\"", escape_json_str(p)),
            None => "null".to_string(),
        };

        format!(
            "{{\"event_id\":1117,\"event_code\":\"0x045D\",\"event_name\":\"NetworkPolicyBlock\",\
            \"timestamp\":\"{}\",\"profile\":\"{}\",\"action\":\"{}\",\"direction\":\"{}\",\
            \"protocol\":\"{}\",\"src_ip\":\"{}\",\"src_port\":{},\
            \"dst_ip\":\"{}\",\"dst_port\":{},\
            \"rule_id\":\"{}\",\"reason\":\"{}\",\"pid\":{},\"process_path\":{}}}\n",
            escape_json_str(&self.timestamp),
            self.profile.as_str(),
            escape_json_str(&self.action),
            self.direction.as_str(),
            self.protocol.as_str(),
            self.src_ip,
            self.src_port,
            self.dst_ip,
            self.dst_port,
            escape_json_str(&self.rule_id),
            escape_json_str(&self.reason),
            pid_str,
            path_str
        )
    }
}

/// Windows Security Audit Event 1118:
/// "Network IP Ban Lifecycle Event (Active, Expired, Unbanned)".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event1118IpBanLifecycle {
    pub timestamp: String,
    pub profile: NetworkProfile,
    pub banned_ip: IpAddr,
    pub status: BanLifecycleStatus,
    pub duration_seconds: u64,
    pub reason: String,
    pub total_active_bans: usize,
}

impl Event1118IpBanLifecycle {
    /// Formats this event as a single line JSON string (JSON-Lines compatible).
    pub fn to_json_line(&self) -> String {
        format!(
            "{{\"event_id\":1118,\"event_code\":\"0x045E\",\"event_name\":\"IpBanLifecycle\",\
            \"timestamp\":\"{}\",\"profile\":\"{}\",\"banned_ip\":\"{}\",\"status\":\"{}\",\
            \"duration_seconds\":{},\"reason\":\"{}\",\"total_active_bans\":{}}}\n",
            escape_json_str(&self.timestamp),
            self.profile.as_str(),
            self.banned_ip,
            self.status.as_str(),
            self.duration_seconds,
            escape_json_str(&self.reason),
            self.total_active_bans
        )
    }
}

/// In-memory audit sink collecting JSON-Lines telemetry strings.
#[derive(Debug, Default)]
pub struct AuditLogger {
    lines: Vec<String>,
}

impl AuditLogger {
    pub fn new() -> Self {
        Self { lines: Vec::new() }
    }

    pub fn emit_1117(&mut self, event: Event1117NetworkAction) {
        self.lines.push(event.to_json_line());
    }

    pub fn emit_1118(&mut self, event: Event1118IpBanLifecycle) {
        self.lines.push(event.to_json_line());
    }

    pub fn records(&self) -> &[String] {
        &self.lines
    }

    pub fn to_json_lines_string(&self) -> String {
        self.lines.concat()
    }

    pub fn clear(&mut self) {
        self.lines.clear();
    }
}

// ============================================================================
// 6. Firewall Policy Engine
// ============================================================================

/// Complete firewall policy engine coordinating profiles, rules, ban lists, and telemetry.
pub struct FirewallPolicyEngine {
    active_profile: NetworkProfile,
    rules: Vec<FirewallRule>,
    ban_manager: IpBanManager,
    logger: AuditLogger,
    clock: Arc<dyn Clock>,
}

impl FirewallPolicyEngine {
    pub fn new(active_profile: NetworkProfile, clock: Arc<dyn Clock>) -> Self {
        Self {
            active_profile,
            rules: Vec::new(),
            ban_manager: IpBanManager::new(clock.clone()),
            logger: AuditLogger::new(),
            clock,
        }
    }

    pub fn with_system_clock(active_profile: NetworkProfile) -> Self {
        Self::new(active_profile, Arc::new(SystemClock))
    }

    pub fn active_profile(&self) -> NetworkProfile {
        self.active_profile
    }

    pub fn set_profile(&mut self, profile: NetworkProfile) {
        self.active_profile = profile;
    }

    pub fn add_rule(&mut self, rule: FirewallRule) {
        self.rules.push(rule);
        // Sort ascending by priority number (lower value = higher precedence)
        self.rules.sort_by_key(|r| r.priority);
    }

    pub fn ban_manager(&self) -> &IpBanManager {
        &self.ban_manager
    }

    pub fn ban_manager_mut(&mut self) -> &mut IpBanManager {
        &mut self.ban_manager
    }

    pub fn audit_logger(&self) -> &AuditLogger {
        &self.logger
    }

    pub fn audit_logger_mut(&mut self) -> &mut AuditLogger {
        &mut self.logger
    }

    /// Progress engine time and process expired IP bans, generating Event ID 1118 notices.
    pub fn tick(&mut self) {
        let expired = self.ban_manager.cleanup_expired_bans();
        let total_active = self.ban_manager.active_ban_count();
        let now = self.clock.now_secs();
        let timestamp = format!("1970-01-01T00:00:{:02}Z", now);

        for ban in expired {
            let event = Event1118IpBanLifecycle {
                timestamp: timestamp.clone(),
                profile: self.active_profile,
                banned_ip: ban.ip,
                status: BanLifecycleStatus::Expired,
                duration_seconds: ban.duration_secs,
                reason: "Temporary IP ban TTL expired".to_string(),
                total_active_bans: total_active,
            };
            self.logger.emit_1118(event);
        }
    }

    /// Evaluates an incoming or outgoing connection against active bans, rule sets, and profiles.
    pub fn evaluate_connection(&mut self, conn: &NetworkConnection) -> ResponseVerdict {
        let remote_ip = match conn.direction {
            TrafficDirection::Inbound => conn.src_ip,
            TrafficDirection::Outbound => conn.dst_ip,
        };

        let now = self.clock.now_secs();
        let timestamp = format!("1970-01-01T00:00:{:02}Z", now);

        // 1. Check if the remote IP is currently subject to a temporary ban
        if let Some(ban) = self.ban_manager.check_and_record_hit(&remote_ip) {
            let reason = format!("IP temporarily banned: {}", ban.reason);

            // Emit Event 1117 audit log
            self.logger.emit_1117(Event1117NetworkAction {
                timestamp,
                profile: self.active_profile,
                action: "BannedDrop".to_string(),
                direction: conn.direction,
                protocol: conn.protocol,
                src_ip: conn.src_ip,
                src_port: conn.src_port,
                dst_ip: conn.dst_ip,
                dst_port: conn.dst_port,
                rule_id: "SYS-IP-BAN".to_string(),
                reason: reason.clone(),
                pid: conn.pid,
                process_path: conn.process_path.clone(),
            });

            return ResponseVerdict::Banned {
                rule_id: "SYS-IP-BAN".to_string(),
                banned_ip: remote_ip,
                duration_secs: ban.duration_secs,
                expires_at: ban.expires_at,
                reason,
            };
        }

        // 2. Iterate matching rules in priority order
        for rule in &self.rules {
            if rule.matches(self.active_profile, conn) {
                match &rule.action {
                    RuleAction::Allow => {
                        return ResponseVerdict::Allow;
                    }
                    RuleAction::Drop => {
                        self.logger.emit_1117(Event1117NetworkAction {
                            timestamp,
                            profile: self.active_profile,
                            action: "Drop".to_string(),
                            direction: conn.direction,
                            protocol: conn.protocol,
                            src_ip: conn.src_ip,
                            src_port: conn.src_port,
                            dst_ip: conn.dst_ip,
                            dst_port: conn.dst_port,
                            rule_id: rule.id.clone(),
                            reason: rule.reason.clone(),
                            pid: conn.pid,
                            process_path: conn.process_path.clone(),
                        });

                        return ResponseVerdict::Drop {
                            rule_id: rule.id.clone(),
                            reason: rule.reason.clone(),
                        };
                    }
                    RuleAction::RejectTcpRst => {
                        let rst_packet = if conn.protocol == IpProtocol::Tcp {
                            synthesize_ipv4_tcp_rst(conn)
                        } else {
                            Vec::new()
                        };

                        self.logger.emit_1117(Event1117NetworkAction {
                            timestamp,
                            profile: self.active_profile,
                            action: "TcpRst".to_string(),
                            direction: conn.direction,
                            protocol: conn.protocol,
                            src_ip: conn.src_ip,
                            src_port: conn.src_port,
                            dst_ip: conn.dst_ip,
                            dst_port: conn.dst_port,
                            rule_id: rule.id.clone(),
                            reason: rule.reason.clone(),
                            pid: conn.pid,
                            process_path: conn.process_path.clone(),
                        });

                        return ResponseVerdict::TcpRst {
                            rule_id: rule.id.clone(),
                            reason: rule.reason.clone(),
                            rst_packet,
                        };
                    }
                    RuleAction::BanIp {
                        duration_secs,
                        reason,
                    } => {
                        let ban_entry = self.ban_manager.ban_ip(
                            remote_ip,
                            *duration_secs,
                            reason.clone(),
                        );
                        let total_active = self.ban_manager.active_ban_count();

                        // Emit Event 1117 for the blocked connection attempt
                        self.logger.emit_1117(Event1117NetworkAction {
                            timestamp: timestamp.clone(),
                            profile: self.active_profile,
                            action: "BanTriggered".to_string(),
                            direction: conn.direction,
                            protocol: conn.protocol,
                            src_ip: conn.src_ip,
                            src_port: conn.src_port,
                            dst_ip: conn.dst_ip,
                            dst_port: conn.dst_port,
                            rule_id: rule.id.clone(),
                            reason: format!("{}: {}", rule.reason, reason),
                            pid: conn.pid,
                            process_path: conn.process_path.clone(),
                        });

                        // Emit Event 1118 for the lifecycle enactment of the new IP ban
                        self.logger.emit_1118(Event1118IpBanLifecycle {
                            timestamp,
                            profile: self.active_profile,
                            banned_ip: remote_ip,
                            status: BanLifecycleStatus::Active,
                            duration_seconds: *duration_secs,
                            reason: reason.clone(),
                            total_active_bans: total_active,
                        });

                        return ResponseVerdict::Banned {
                            rule_id: rule.id.clone(),
                            banned_ip: remote_ip,
                            duration_secs: *duration_secs,
                            expires_at: ban_entry.expires_at,
                            reason: reason.clone(),
                        };
                    }
                    RuleAction::AuditOnly => {
                        self.logger.emit_1117(Event1117NetworkAction {
                            timestamp: timestamp.clone(),
                            profile: self.active_profile,
                            action: "AuditAllow".to_string(),
                            direction: conn.direction,
                            protocol: conn.protocol,
                            src_ip: conn.src_ip,
                            src_port: conn.src_port,
                            dst_ip: conn.dst_ip,
                            dst_port: conn.dst_port,
                            rule_id: rule.id.clone(),
                            reason: rule.reason.clone(),
                            pid: conn.pid,
                            process_path: conn.process_path.clone(),
                        });
                        // Fall-through to next rule or default
                    }
                }
            }
        }

        // 3. Default fallback per profile:
        // - Domain: Inbound allowed on internal networks, outbound allowed
        // - Private: Inbound dropped by default, outbound allowed
        // - Public: Inbound rejected with TCP RST, outbound allowed
        match conn.direction {
            TrafficDirection::Outbound => ResponseVerdict::Allow,
            TrafficDirection::Inbound => match self.active_profile {
                NetworkProfile::Domain => ResponseVerdict::Allow,
                NetworkProfile::Private => {
                    let reason = "Default drop inbound on Private profile".to_string();
                    self.logger.emit_1117(Event1117NetworkAction {
                        timestamp,
                        profile: self.active_profile,
                        action: "DefaultDrop".to_string(),
                        direction: conn.direction,
                        protocol: conn.protocol,
                        src_ip: conn.src_ip,
                        src_port: conn.src_port,
                        dst_ip: conn.dst_ip,
                        dst_port: conn.dst_port,
                        rule_id: "DEFAULT-PRIV-INBOUND".to_string(),
                        reason: reason.clone(),
                        pid: conn.pid,
                        process_path: conn.process_path.clone(),
                    });
                    ResponseVerdict::Drop {
                        rule_id: "DEFAULT-PRIV-INBOUND".to_string(),
                        reason,
                    }
                }
                NetworkProfile::Public => {
                    let reason = "Default TCP RST inbound on Public profile".to_string();
                    let rst_packet = if conn.protocol == IpProtocol::Tcp {
                        synthesize_ipv4_tcp_rst(conn)
                    } else {
                        Vec::new()
                    };

                    self.logger.emit_1117(Event1117NetworkAction {
                        timestamp,
                        profile: self.active_profile,
                        action: "DefaultTcpRst".to_string(),
                        direction: conn.direction,
                        protocol: conn.protocol,
                        src_ip: conn.src_ip,
                        src_port: conn.src_port,
                        dst_ip: conn.dst_ip,
                        dst_port: conn.dst_port,
                        rule_id: "DEFAULT-PUB-INBOUND".to_string(),
                        reason: reason.clone(),
                        pid: conn.pid,
                        process_path: conn.process_path.clone(),
                    });

                    ResponseVerdict::TcpRst {
                        rule_id: "DEFAULT-PUB-INBOUND".to_string(),
                        reason,
                        rst_packet,
                    }
                }
            },
        }
    }
}

// ============================================================================
// 7. Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_ipv4_conn(direction: TrafficDirection, dst_port: u16) -> NetworkConnection {
        NetworkConnection::new_tcp(
            direction,
            IpAddr::V4(Ipv4Addr::new(198, 51, 100, 42)),
            49152,
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 5)),
            dst_port,
            1000,
            500,
            0x18, // PSH | ACK
        )
        .with_process(4128, "C:\\Windows\\System32\\svchost.exe")
    }

    #[test]
    fn test_network_profiles_and_string_representation() {
        assert_eq!(NetworkProfile::Domain.as_str(), "Domain");
        assert_eq!(NetworkProfile::Private.as_str(), "Private");
        assert_eq!(NetworkProfile::Public.as_str(), "Public");

        assert_eq!(
            NetworkProfile::from_str_name("domain"),
            Some(NetworkProfile::Domain)
        );
        assert_eq!(
            NetworkProfile::from_str_name("PRIVATE"),
            Some(NetworkProfile::Private)
        );
        assert_eq!(
            NetworkProfile::from_str_name("public"),
            Some(NetworkProfile::Public)
        );
        assert_eq!(NetworkProfile::from_str_name("invalid"), None);
    }

    #[test]
    fn test_ip_and_port_matchers() {
        let ip_any = IpMatcher::Any;
        assert!(ip_any.matches(&IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1))));

        let ip_exact = IpMatcher::Exact(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100)));
        assert!(ip_exact.matches(&IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100))));
        assert!(!ip_exact.matches(&IpAddr::V4(Ipv4Addr::new(192, 168, 1, 101))));

        let cidr = IpMatcher::CidrV4(Ipv4Addr::new(192, 168, 1, 0), 24);
        assert!(cidr.matches(&IpAddr::V4(Ipv4Addr::new(192, 168, 1, 55))));
        assert!(!cidr.matches(&IpAddr::V4(Ipv4Addr::new(192, 168, 2, 55))));

        let port_exact = PortMatcher::Exact(445);
        assert!(port_exact.matches(445));
        assert!(!port_exact.matches(80));

        let port_range = PortMatcher::Range(1000, 2000);
        assert!(port_range.matches(1500));
        assert!(!port_range.matches(2001));

        let port_list = PortMatcher::List(vec![80, 443, 8080]);
        assert!(port_list.matches(443));
        assert!(!port_list.matches(22));
    }

    #[test]
    fn test_profile_specific_firewall_verdicts() {
        let clock = Arc::new(MockClock::new(1000));
        let mut engine = FirewallPolicyEngine::new(NetworkProfile::Domain, clock.clone());

        // Inbound rule allowing SMB (445) only on Domain
        engine.add_rule(FirewallRule {
            id: "R-SMB-DOMAIN".to_string(),
            name: "Allow SMB in Domain".to_string(),
            priority: 10,
            enabled: true,
            profiles: vec![NetworkProfile::Domain],
            direction: Some(TrafficDirection::Inbound),
            protocol: Some(IpProtocol::Tcp),
            remote_ip: IpMatcher::Any,
            remote_port: PortMatcher::Any,
            local_port: PortMatcher::Exact(445),
            action: RuleAction::Allow,
            reason: "Domain trusted file sharing".to_string(),
        });

        // Inbound rule resetting SMB on Public profile
        engine.add_rule(FirewallRule {
            id: "R-SMB-PUBLIC-RST".to_string(),
            name: "Block SMB on Public with RST".to_string(),
            priority: 10,
            enabled: true,
            profiles: vec![NetworkProfile::Public],
            direction: Some(TrafficDirection::Inbound),
            protocol: Some(IpProtocol::Tcp),
            remote_ip: IpMatcher::Any,
            remote_port: PortMatcher::Any,
            local_port: PortMatcher::Exact(445),
            action: RuleAction::RejectTcpRst,
            reason: "Block SMB on Public Hotspot".to_string(),
        });

        let conn = sample_ipv4_conn(TrafficDirection::Inbound, 445);

        // Under Domain profile -> Allowed
        engine.set_profile(NetworkProfile::Domain);
        let verdict = engine.evaluate_connection(&conn);
        assert_eq!(verdict, ResponseVerdict::Allow);

        // Under Public profile -> Rejected with TCP RST
        engine.set_profile(NetworkProfile::Public);
        let verdict = engine.evaluate_connection(&conn);
        match verdict {
            ResponseVerdict::TcpRst {
                rule_id,
                rst_packet,
                ..
            } => {
                assert_eq!(rule_id, "R-SMB-PUBLIC-RST");
                assert_eq!(rst_packet.len(), 40);
                assert_eq!(rst_packet[33], 0x04); // RST flag set
            }
            other => panic!("Expected TcpRst, got {:?}", other),
        }

        // Under Private profile -> Default Private Drop
        engine.set_profile(NetworkProfile::Private);
        let verdict = engine.evaluate_connection(&conn);
        match verdict {
            ResponseVerdict::Drop { rule_id, .. } => {
                assert_eq!(rule_id, "DEFAULT-PRIV-INBOUND");
            }
            other => panic!("Expected Drop, got {:?}", other),
        }
    }

    #[test]
    fn test_tcp_rst_synthesis_rfc_compliance() {
        let conn = NetworkConnection::new_tcp(
            TrafficDirection::Inbound,
            IpAddr::V4(Ipv4Addr::new(198, 51, 100, 10)),
            54321,
            IpAddr::V4(Ipv4Addr::new(192, 168, 1, 50)),
            80,
            2000,
            1000,
            0x10, // ACK flag set
        );

        let rst = synthesize_ipv4_tcp_rst(&conn);
        assert_eq!(rst.len(), 40);

        // IPv4 Header verification
        assert_eq!(rst[0], 0x45); // IPv4, IHL 5
        assert_eq!(&rst[2..4], &40u16.to_be_bytes()); // Total length 40
        assert_eq!(rst[9], 6); // Protocol TCP
        assert_eq!(&rst[12..16], &[192, 168, 1, 50]); // Source IP inverted
        assert_eq!(&rst[16..20], &[198, 51, 100, 10]); // Destination IP inverted

        // Validate IPv4 checksum is 0 when computed over entire header
        let ip_csum = calculate_checksum(&rst[0..20]);
        assert_eq!(ip_csum, 0);

        // TCP Header verification
        let src_port = u16::from_be_bytes([rst[20], rst[21]]);
        let dst_port = u16::from_be_bytes([rst[22], rst[23]]);
        assert_eq!(src_port, 80);
        assert_eq!(dst_port, 54321);

        // Since incoming had ACK set, RST seq = incoming ack (1000)
        let seq = u32::from_be_bytes([rst[24], rst[25], rst[26], rst[27]]);
        assert_eq!(seq, 1000);

        // Flags should be RST (0x04)
        assert_eq!(rst[33], 0x04);
    }

    #[test]
    fn test_temporary_ip_ban_enactment_and_ttl_expiration() {
        let mock_clock = Arc::new(MockClock::new(100_000));
        let mut engine = FirewallPolicyEngine::new(NetworkProfile::Public, mock_clock.clone());

        let attacker_ip = IpAddr::V4(Ipv4Addr::new(203, 0, 113, 99));

        // Rule to ban attacker on scanning port 1337 for 300 seconds (5 min)
        engine.add_rule(FirewallRule {
            id: "R-BAN-SCANNER".to_string(),
            name: "Ban Port Scanner".to_string(),
            priority: 1,
            enabled: true,
            profiles: vec![NetworkProfile::Public],
            direction: Some(TrafficDirection::Inbound),
            protocol: Some(IpProtocol::Tcp),
            remote_ip: IpMatcher::Exact(attacker_ip),
            remote_port: PortMatcher::Any,
            local_port: PortMatcher::Exact(1337),
            action: RuleAction::BanIp {
                duration_secs: 300,
                reason: "Suspicious port scanning attempt".to_string(),
            },
            reason: "Detected scanning behavior".to_string(),
        });

        let conn = NetworkConnection::new_tcp(
            TrafficDirection::Inbound,
            attacker_ip,
            40000,
            IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)),
            1337,
            1,
            0,
            0x02, // SYN
        );

        // First attempt triggers BanIp
        let verdict = engine.evaluate_connection(&conn);
        match verdict {
            ResponseVerdict::Banned {
                banned_ip,
                duration_secs,
                expires_at,
                ..
            } => {
                assert_eq!(banned_ip, attacker_ip);
                assert_eq!(duration_secs, 300);
                assert_eq!(expires_at, 100_300);
            }
            other => panic!("Expected Banned verdict, got {:?}", other),
        }

        assert_eq!(engine.ban_manager().active_ban_count(), 1);

        // Subsequent attempt to any other port while banned (e.g. port 80) is immediately dropped
        let conn_web = NetworkConnection::new_tcp(
            TrafficDirection::Inbound,
            attacker_ip,
            40001,
            IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)),
            80,
            1,
            0,
            0x02,
        );

        let verdict2 = engine.evaluate_connection(&conn_web);
        match verdict2 {
            ResponseVerdict::Banned { rule_id, .. } => {
                assert_eq!(rule_id, "SYS-IP-BAN");
            }
            other => panic!("Expected SYS-IP-BAN, got {:?}", other),
        }

        // Advance time by 299 seconds -> still active
        mock_clock.advance(299);
        assert_eq!(engine.ban_manager().active_ban_count(), 1);

        // Advance 2 more seconds (total 301 seconds -> expired)
        mock_clock.advance(2);
        assert_eq!(engine.ban_manager().active_ban_count(), 0);

        // Call engine tick to process expired ban telemetry (Event 1118 Expired)
        engine.tick();
        assert_eq!(engine.ban_manager().active_ban_count(), 0);

        // Inspect audit telemetry
        let json_lines = engine.audit_logger().to_json_lines_string();
        assert!(json_lines.contains("\"event_id\":1117"));
        assert!(json_lines.contains("\"event_id\":1118"));
        assert!(json_lines.contains("\"status\":\"Active\""));
        assert!(json_lines.contains("\"status\":\"Expired\""));
    }

    #[test]
    fn test_audit_json_lines_formatting_event_1117_and_1118() {
        let mut logger = AuditLogger::new();

        let event1117 = Event1117NetworkAction {
            timestamp: "2026-11-10T15:30:00Z".to_string(),
            profile: NetworkProfile::Public,
            action: "TcpRst".to_string(),
            direction: TrafficDirection::Inbound,
            protocol: IpProtocol::Tcp,
            src_ip: IpAddr::V4(Ipv4Addr::new(198, 51, 100, 77)),
            src_port: 55555,
            dst_ip: IpAddr::V4(Ipv4Addr::new(192, 168, 1, 20)),
            dst_port: 445,
            rule_id: "R-BLOCK-SMB".to_string(),
            reason: "SMB blocked on public network".to_string(),
            pid: Some(999),
            process_path: Some("C:\\Windows\\System32\\ntoskrnl.exe".to_string()),
        };

        logger.emit_1117(event1117);

        let event1118 = Event1118IpBanLifecycle {
            timestamp: "2026-11-10T15:30:01Z".to_string(),
            profile: NetworkProfile::Public,
            banned_ip: IpAddr::V4(Ipv4Addr::new(198, 51, 100, 77)),
            status: BanLifecycleStatus::Active,
            duration_seconds: 600,
            reason: "Repeated port probe".to_string(),
            total_active_bans: 1,
        };

        logger.emit_1118(event1118);

        let output = logger.to_json_lines_string();
        let lines: Vec<&str> = output.lines().collect();

        assert_eq!(lines.len(), 2);

        // Validate line 1 is Event 1117
        assert!(lines[0].starts_with("{\"event_id\":1117"));
        assert!(lines[0].contains("\"event_code\":\"0x045D\""));
        assert!(lines[0].contains("\"event_name\":\"NetworkPolicyBlock\""));
        assert!(lines[0].contains("\"src_ip\":\"198.51.100.77\""));
        assert!(lines[0].contains("\"src_port\":55555"));
        assert!(lines[0].contains("\"dst_port\":445"));
        assert!(lines[0].contains("\"pid\":999"));
        assert!(lines[0].contains("\"process_path\":\"C:\\\\Windows\\\\System32\\\\ntoskrnl.exe\""));

        // Validate line 2 is Event 1118
        assert!(lines[1].starts_with("{\"event_id\":1118"));
        assert!(lines[1].contains("\"event_code\":\"0x045E\""));
        assert!(lines[1].contains("\"event_name\":\"IpBanLifecycle\""));
        assert!(lines[1].contains("\"banned_ip\":\"198.51.100.77\""));
        assert!(lines[1].contains("\"status\":\"Active\""));
        assert!(lines[1].contains("\"duration_seconds\":600"));
        assert!(lines[1].contains("\"total_active_bans\":1"));
    }
}
