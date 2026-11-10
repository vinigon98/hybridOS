//! HybridOS Network Layer Core Subsystem
//!
//! Provides the primary network architecture for HybridOS:
//! - Ethernet II framing and MAC address abstraction (`ethernet`)
//! - Address Resolution Protocol (ARP) & dynamic ARP cache (`arp`)
//! - IPv4 packet framing, RFC 1071 checksums, and CIDR routing (`ipv4`)
//! - Internet Control Message Protocol (ICMP) and ping handling (`icmp`)
//! - Network interface abstraction, packet dispatcher, and `init_net()`

pub mod ethernet;
pub mod arp;
pub mod ipv4;
pub mod icmp;
pub mod udp;
pub mod tcp;
pub mod socket;

use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};

pub use ethernet::{
    calculate_crc32, verify_and_strip_fcs, EtherType, EthernetError, EthernetFrame,
    EthernetHeader, MacAddress, ETHERNET_HEADER_LEN, ETHERNET_MAX_FRAME_LEN,
    ETHERNET_MIN_FRAME_LEN,
};
pub use arp::{
    ArpCache, ArpEntry, ArpEntryState, ArpError, ArpOperation, ArpPacket, ARP_PACKET_LEN,
};
pub use ipv4::{
    compute_internet_checksum, verify_internet_checksum, Ipv4Address, Ipv4Error, Ipv4Header,
    Ipv4Packet, Ipv4Protocol, Route, RoutingTable, DEFAULT_TTL, IPV4_MIN_HEADER_LEN,
};
pub use icmp::{
    handle_echo_request, DestUnreachableCode, IcmpEchoHeader, IcmpError, IcmpHeader,
    IcmpPacket, IcmpType, ICMP_HEADER_LEN,
};
pub use udp::{
    compute_udp_checksum, verify_udp_checksum, PortDemuxTable, UdpDatagram, UdpError, UdpHeader,
    UdpQueue, UdpSubsystem, UDP_HEADER_LEN,
};
pub use tcp::{
    compute_tcp_checksum, seq_gt, seq_gte, seq_lt, seq_lte, SlidingWindow, TcpConnection,
    TcpError, TcpFlags, TcpHeader, TcpSegment, TcpSeqTracker, TcpState, TCP_DEFAULT_MSS,
    TCP_DEFAULT_WINDOW_CAPACITY, TCP_HEADER_MIN_LEN,
};
pub use socket::{
    AddressFamily, RingBuffer, ShutdownHow, Socket, SocketAddress, SocketError, SocketFileOps,
    SocketProtocol, SocketRegistry, SocketState, SocketType, FIONBIO, FIONREAD,
};

/// Network interface operational statistics.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct InterfaceStats {
    pub rx_packets: u64,
    pub tx_packets: u64,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub rx_dropped: u64,
    pub tx_dropped: u64,
    pub rx_errors: u64,
    pub tx_errors: u64,
}

/// Network Interface Abstraction representing a physical or virtual network card.
#[derive(Debug, Clone)]
pub struct NetworkInterface {
    /// Unique identifier for interface.
    pub id: usize,
    /// Human-readable name (e.g., "eth0", "lo").
    pub name: String,
    /// Physical 48-bit MAC address.
    pub mac: MacAddress,
    /// Assigned IPv4 address.
    pub ip: Ipv4Address,
    /// Subnet mask.
    pub netmask: Ipv4Address,
    /// Default gateway IP if applicable.
    pub gateway: Option<Ipv4Address>,
    /// Maximum Transmission Unit in bytes (typically 1500 for Ethernet).
    pub mtu: usize,
    /// Link administrative state (true = UP, false = DOWN).
    pub is_up: bool,
    /// Receive frame queue (frames delivered from driver / wire).
    pub rx_queue: VecDeque<Vec<u8>>,
    /// Transmit frame queue (frames queued for hardware transmission).
    pub tx_queue: VecDeque<Vec<u8>>,
    /// Interface traffic statistics.
    pub stats: InterfaceStats,
}

impl NetworkInterface {
    /// Create a new network interface.
    pub fn new(
        id: usize,
        name: &str,
        mac: MacAddress,
        ip: Ipv4Address,
        netmask: Ipv4Address,
        gateway: Option<Ipv4Address>,
        mtu: usize,
    ) -> Self {
        Self {
            id,
            name: name.to_string(),
            mac,
            ip,
            netmask,
            gateway,
            mtu,
            is_up: true,
            rx_queue: VecDeque::new(),
            tx_queue: VecDeque::new(),
            stats: InterfaceStats::default(),
        }
    }

    /// Enqueue a frame for hardware transmission.
    pub fn queue_tx(&mut self, frame: Vec<u8>) {
        self.stats.tx_packets += 1;
        self.stats.tx_bytes += frame.len() as u64;
        self.tx_queue.push_back(frame);
    }

    /// Dequeue the next outbound frame ready for driver transmission.
    pub fn pop_tx(&mut self) -> Option<Vec<u8>> {
        self.tx_queue.pop_front()
    }

    /// Enqueue a frame received by hardware driver into interface RX buffer.
    pub fn push_rx(&mut self, frame: Vec<u8>) {
        self.stats.rx_packets += 1;
        self.stats.rx_bytes += frame.len() as u64;
        self.rx_queue.push_back(frame);
    }

    /// Dequeue the next incoming frame for kernel packet dispatching.
    pub fn pop_rx(&mut self) -> Option<Vec<u8>> {
        self.rx_queue.pop_front()
    }

    /// Check if there are frames pending transmission.
    pub fn has_tx(&self) -> bool {
        !self.tx_queue.is_empty()
    }

    /// Check if there are frames waiting to be processed.
    pub fn has_rx(&self) -> bool {
        !self.rx_queue.is_empty()
    }
}

/// Events resulting from network packet dispatching.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetworkEvent {
    /// Handled an ARP Request addressed to us and queued an ARP Reply.
    ArpRequestAnswered {
        sender_ip: Ipv4Address,
        sender_mac: MacAddress,
        reply_frame_len: usize,
    },
    /// Learned or updated an ARP entry from an incoming ARP Reply.
    ArpReplyLearned {
        sender_ip: Ipv4Address,
        sender_mac: MacAddress,
    },
    /// Handled an incoming ICMP Echo Request and queued an ICMP Echo Reply.
    IcmpEchoReplySent {
        target_ip: Ipv4Address,
        seq: u16,
        frame_len: usize,
    },
    /// Received an ICMP Echo Reply (ping response).
    IcmpEchoReplyReceived {
        from_ip: Ipv4Address,
        ident: u16,
        seq: u16,
        payload_len: usize,
    },
    /// Received an IPv4 packet destined for another protocol (TCP, UDP, etc.).
    Ipv4PacketReceived {
        src: Ipv4Address,
        dst: Ipv4Address,
        protocol: Ipv4Protocol,
        payload_len: usize,
    },
    /// Packet dropped due to validation error or filter.
    PacketDropped {
        reason: String,
    },
}

/// Central Network Stack Manager and Packet Dispatcher.
#[derive(Debug)]
pub struct NetworkStack {
    /// Registered interfaces (index corresponds to interface id).
    pub interfaces: Vec<NetworkInterface>,
    /// Kernel ARP translation cache.
    pub arp_cache: ArpCache,
    /// Kernel CIDR routing table.
    pub routing_table: RoutingTable,
    /// Current monotonic system clock in seconds.
    pub system_time_secs: u64,
    /// Next ICMP Echo sequence number.
    pub next_ping_seq: u16,
}

impl Default for NetworkStack {
    fn default() -> Self {
        Self::new()
    }
}

impl NetworkStack {
    /// Create a new, empty network stack.
    pub fn new() -> Self {
        Self {
            interfaces: Vec::new(),
            arp_cache: ArpCache::new(),
            routing_table: RoutingTable::new(),
            system_time_secs: 100, // Initialize with reasonable non-zero timestamp
            next_ping_seq: 1,
        }
    }

    /// Register a network interface and return its assigned interface ID.
    pub fn add_interface(&mut self, mut iface: NetworkInterface) -> usize {
        let id = self.interfaces.len();
        iface.id = id;
        self.interfaces.push(iface);
        id
    }

    /// Access interface by ID.
    pub fn get_interface(&self, id: usize) -> Option<&NetworkInterface> {
        self.interfaces.get(id)
    }

    /// Access mutable interface by ID.
    pub fn get_interface_mut(&mut self, id: usize) -> Option<&mut NetworkInterface> {
        self.interfaces.get_mut(id)
    }

    /// Find interface by device name.
    pub fn get_interface_by_name(&self, name: &str) -> Option<&NetworkInterface> {
        self.interfaces.iter().find(|i| i.name == name)
    }

    /// Find mutable interface by device name.
    pub fn get_interface_by_name_mut(&mut self, name: &str) -> Option<&mut NetworkInterface> {
        self.interfaces.iter_mut().find(|i| i.name == name)
    }

    /// Advance system time and purge expired ARP entries.
    pub fn tick(&mut self, delta_secs: u64) {
        self.system_time_secs = self.system_time_secs.saturating_add(delta_secs);
        self.arp_cache.purge_expired(self.system_time_secs);
    }

    /// Dispatch and process a raw Ethernet packet received on an interface.
    pub fn process_incoming_packet(
        &mut self,
        interface_id: usize,
        raw_frame: &[u8],
    ) -> NetworkEvent {
        if interface_id >= self.interfaces.len() {
            return NetworkEvent::PacketDropped {
                reason: format!("Invalid interface id {}", interface_id),
            };
        }

        let (iface_mac, _iface_ip, is_up) = {
            let iface = &self.interfaces[interface_id];
            (iface.mac, iface.ip, iface.is_up)
        };

        if !is_up {
            return NetworkEvent::PacketDropped {
                reason: "Interface is DOWN".to_string(),
            };
        }

        // 1. Unpack Ethernet frame
        let eth_frame = match EthernetFrame::unpack(raw_frame) {
            Ok(f) => f,
            Err(e) => {
                let iface = &mut self.interfaces[interface_id];
                iface.stats.rx_errors += 1;
                return NetworkEvent::PacketDropped {
                    reason: format!("Ethernet unpack error: {:?}", e),
                };
            }
        };

        // Filter destination MAC (must be unicast to us, broadcast, or multicast)
        if eth_frame.header.dst != iface_mac
            && !eth_frame.header.dst.is_broadcast()
            && !eth_frame.header.dst.is_multicast()
        {
            let iface = &mut self.interfaces[interface_id];
            iface.stats.rx_dropped += 1;
            return NetworkEvent::PacketDropped {
                reason: format!("MAC address filter: frame addressed to {}", eth_frame.header.dst),
            };
        }

        // 2. Dispatch based on EtherType
        match eth_frame.header.ethertype {
            EtherType::Arp => self.handle_arp(interface_id, &eth_frame),
            EtherType::Ipv4 => self.handle_ipv4(interface_id, &eth_frame),
            other => {
                let iface = &mut self.interfaces[interface_id];
                iface.stats.rx_dropped += 1;
                NetworkEvent::PacketDropped {
                    reason: format!("Unhandled EtherType: {:?}", other),
                }
            }
        }
    }

    /// Process ARP packet on interface.
    fn handle_arp(&mut self, interface_id: usize, frame: &EthernetFrame) -> NetworkEvent {
        let (iface_mac, iface_ip) = {
            let iface = &self.interfaces[interface_id];
            (iface.mac, iface.ip)
        };

        let arp_pkt = match ArpPacket::parse(&frame.payload) {
            Ok(p) => p,
            Err(e) => {
                let iface = &mut self.interfaces[interface_id];
                iface.stats.rx_errors += 1;
                return NetworkEvent::PacketDropped {
                    reason: format!("ARP parse error: {:?}", e),
                };
            }
        };

        let reply_opt = self.arp_cache.process_incoming(
            &arp_pkt,
            iface_ip,
            iface_mac,
            self.system_time_secs,
        );

        if let Some(reply) = reply_opt {
            let reply_bytes = reply.to_bytes();
            let out_frame = EthernetFrame::pack(
                arp_pkt.sender_mac,
                iface_mac,
                EtherType::Arp,
                &reply_bytes,
                true,
            );
            let frame_len = out_frame.len();

            let iface = &mut self.interfaces[interface_id];
            iface.queue_tx(out_frame);

            NetworkEvent::ArpRequestAnswered {
                sender_ip: arp_pkt.sender_ip,
                sender_mac: arp_pkt.sender_mac,
                reply_frame_len: frame_len,
            }
        } else {
            NetworkEvent::ArpReplyLearned {
                sender_ip: arp_pkt.sender_ip,
                sender_mac: arp_pkt.sender_mac,
            }
        }
    }

    /// Process IPv4 packet on interface.
    fn handle_ipv4(&mut self, interface_id: usize, frame: &EthernetFrame) -> NetworkEvent {
        let (iface_mac, iface_ip) = {
            let iface = &self.interfaces[interface_id];
            (iface.mac, iface.ip)
        };

        let ip_pkt = match Ipv4Packet::parse(&frame.payload) {
            Ok(p) => p,
            Err(e) => {
                let iface = &mut self.interfaces[interface_id];
                iface.stats.rx_errors += 1;
                return NetworkEvent::PacketDropped {
                    reason: format!("IPv4 parse error: {:?}", e),
                };
            }
        };

        // Check if destination is this interface, broadcast, or loopback
        let is_for_us = ip_pkt.header.dst == iface_ip
            || ip_pkt.header.dst == Ipv4Address::BROADCAST
            || ip_pkt.header.dst == Ipv4Address::LOOPBACK;

        if !is_for_us {
            let iface = &mut self.interfaces[interface_id];
            iface.stats.rx_dropped += 1;
            return NetworkEvent::PacketDropped {
                reason: format!("Destination IP {} not destined for this host", ip_pkt.header.dst),
            };
        }

        match ip_pkt.header.protocol {
            Ipv4Protocol::Icmp => {
                // Parse ICMP
                let icmp_pkt = match IcmpPacket::parse(&ip_pkt.payload) {
                    Ok(p) => p,
                    Err(e) => {
                        let iface = &mut self.interfaces[interface_id];
                        iface.stats.rx_errors += 1;
                        return NetworkEvent::PacketDropped {
                            reason: format!("ICMP parse error: {:?}", e),
                        };
                    }
                };

                match icmp_pkt.header.icmp_type {
                    IcmpType::EchoRequest => {
                        // Generate Echo Reply
                        let echo_info = icmp_pkt.echo_info().unwrap_or(IcmpEchoHeader {
                            identifier: 0,
                            sequence_number: 0,
                        });

                        let reply_icmp = match IcmpPacket::build_echo_reply(&icmp_pkt) {
                            Ok(r) => r,
                            Err(e) => {
                                return NetworkEvent::PacketDropped {
                                    reason: format!("Failed building ICMP reply: {:?}", e),
                                };
                            }
                        };

                        let reply_ip = match Ipv4Packet::new(
                            iface_ip,
                            ip_pkt.header.src,
                            Ipv4Protocol::Icmp,
                            reply_icmp.to_bytes(),
                        ) {
                            Ok(p) => p,
                            Err(e) => {
                                return NetworkEvent::PacketDropped {
                                    reason: format!("Failed encapsulating IPv4 reply: {:?}", e),
                                };
                            }
                        };

                        // Outbound destination MAC
                        let target_mac = self
                            .arp_cache
                            .lookup(&ip_pkt.header.src)
                            .unwrap_or(frame.header.src);

                        let out_frame = EthernetFrame::pack(
                            target_mac,
                            iface_mac,
                            EtherType::Ipv4,
                            &reply_ip.to_bytes(),
                            true,
                        );
                        let frame_len = out_frame.len();

                        let iface = &mut self.interfaces[interface_id];
                        iface.queue_tx(out_frame);

                        NetworkEvent::IcmpEchoReplySent {
                            target_ip: ip_pkt.header.src,
                            seq: echo_info.sequence_number,
                            frame_len,
                        }
                    }
                    IcmpType::EchoReply => {
                        let echo_info = icmp_pkt.echo_info().unwrap_or(IcmpEchoHeader {
                            identifier: 0,
                            sequence_number: 0,
                        });

                        NetworkEvent::IcmpEchoReplyReceived {
                            from_ip: ip_pkt.header.src,
                            ident: echo_info.identifier,
                            seq: echo_info.sequence_number,
                            payload_len: icmp_pkt.payload.len(),
                        }
                    }
                    _ => NetworkEvent::PacketDropped {
                        reason: format!("Unhandled ICMP type: {:?}", icmp_pkt.header.icmp_type),
                    },
                }
            }
            other_proto => NetworkEvent::Ipv4PacketReceived {
                src: ip_pkt.header.src,
                dst: ip_pkt.header.dst,
                protocol: other_proto,
                payload_len: ip_pkt.payload.len(),
            },
        }
    }

    /// Transmit an IPv4 packet through the network stack.
    /// Performs CIDR routing lookup to determine egress interface and next hop,
    /// queries the ARP cache for next-hop MAC, and enqueues the Ethernet frame.
    pub fn send_ipv4_packet(
        &mut self,
        dst_ip: Ipv4Address,
        protocol: Ipv4Protocol,
        payload: &[u8],
    ) -> Result<usize, &'static str> {
        // 1. Route lookup
        let (iface_id, next_hop) = {
            let route = self.routing_table.lookup(dst_ip).ok_or("No route to host")?;
            let next_hop = route.gateway.unwrap_or(dst_ip);
            (route.interface_id, next_hop)
        };

        if iface_id >= self.interfaces.len() {
            return Err("Invalid egress interface from route");
        }

        let (src_ip, src_mac) = {
            let iface = &self.interfaces[iface_id];
            (iface.ip, iface.mac)
        };

        // 2. Resolve next-hop MAC via ARP cache
        let dst_mac = match self.arp_cache.lookup(&next_hop) {
            Some(mac) => mac,
            None => {
                // If MAC not in cache, queue an ARP Request on the egress interface
                self.send_arp_request(iface_id, next_hop)?;
                return Err("ARP cache miss; ARP Request transmitted");
            }
        };

        // 3. Build IPv4 packet
        let ip_pkt = Ipv4Packet::new(src_ip, dst_ip, protocol, payload.to_vec())
            .map_err(|_| "Failed to build IPv4 packet")?;
        let ip_bytes = ip_pkt.to_bytes();

        // 4. Encapsulate in Ethernet frame
        let frame = EthernetFrame::pack(dst_mac, src_mac, EtherType::Ipv4, &ip_bytes, true);
        let frame_len = frame.len();

        let iface = &mut self.interfaces[iface_id];
        iface.queue_tx(frame);

        Ok(frame_len)
    }

    /// Construct and queue an ARP Request packet for `target_ip` on `interface_id`.
    pub fn send_arp_request(
        &mut self,
        interface_id: usize,
        target_ip: Ipv4Address,
    ) -> Result<(), &'static str> {
        if interface_id >= self.interfaces.len() {
            return Err("Invalid interface ID for ARP request");
        }

        let (src_mac, src_ip) = {
            let iface = &self.interfaces[interface_id];
            (iface.mac, iface.ip)
        };

        let arp_req = ArpPacket::new_request(src_mac, src_ip, target_ip);
        let arp_bytes = arp_req.to_bytes();

        let eth_frame = EthernetFrame::pack(
            MacAddress::BROADCAST,
            src_mac,
            EtherType::Arp,
            &arp_bytes,
            true,
        );

        // Mark target as pending in ARP cache
        self.arp_cache.mark_pending(target_ip, self.system_time_secs);

        let iface = &mut self.interfaces[interface_id];
        iface.queue_tx(eth_frame);

        Ok(())
    }

    /// Send an ICMP Echo Request (Ping) to `dst_ip`.
    pub fn ping(
        &mut self,
        dst_ip: Ipv4Address,
        payload_data: &[u8],
    ) -> Result<u16, &'static str> {
        let seq = self.next_ping_seq;
        self.next_ping_seq = self.next_ping_seq.wrapping_add(1);

        let icmp_req = IcmpPacket::build_echo_request(0x1337, seq, payload_data);
        let icmp_bytes = icmp_req.to_bytes();

        self.send_ipv4_packet(dst_ip, Ipv4Protocol::Icmp, &icmp_bytes)?;
        Ok(seq)
    }
}

/// Global Network Subsystem Status descriptor returned by `init_net()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NetworkStatus {
    pub interface_count: usize,
    pub route_count: usize,
    pub arp_entry_count: usize,
    pub loopback_ready: bool,
    pub eth0_ready: bool,
    pub eth0_ip: Ipv4Address,
    pub eth0_mac: MacAddress,
}

static GLOBAL_NETWORK_STACK: OnceLock<Mutex<NetworkStack>> = OnceLock::new();

/// Access the global network stack mutex.
pub fn get_network_stack() -> &'static Mutex<NetworkStack> {
    GLOBAL_NETWORK_STACK.get_or_init(|| {
        let mut stack = NetworkStack::new();
        setup_default_stack(&mut stack);
        Mutex::new(stack)
    })
}

/// Helper to configure default loopback and eth0 interfaces and routes.
fn setup_default_stack(stack: &mut NetworkStack) {
    // 1. Loopback interface (lo: 127.0.0.1 / 255.0.0.0)
    let lo_netmask = Ipv4Address::new(255, 0, 0, 0);
    let lo_interface = NetworkInterface::new(
        0,
        "lo",
        MacAddress::ZERO,
        Ipv4Address::LOOPBACK,
        lo_netmask,
        None,
        65535,
    );
    let lo_id = stack.add_interface(lo_interface);

    // 2. VirtIO Ethernet interface (eth0: 192.168.1.100 / 255.255.255.0)
    let eth0_mac = MacAddress::new([0x52, 0x54, 0x00, 0x12, 0x34, 0x56]); // Standard QEMU VirtIO MAC
    let eth0_ip = Ipv4Address::new(192, 168, 1, 100);
    let eth0_netmask = Ipv4Address::new(255, 255, 255, 0);
    let eth0_gw = Ipv4Address::new(192, 168, 1, 1);
    let eth0_interface = NetworkInterface::new(
        1,
        "eth0",
        eth0_mac,
        eth0_ip,
        eth0_netmask,
        Some(eth0_gw),
        1500,
    );
    let eth0_id = stack.add_interface(eth0_interface);

    // 3. Routing Table Entries
    // Loopback route (127.0.0.0/8)
    let _ = stack.routing_table.add_direct(Ipv4Address::new(127, 0, 0, 0), 8, lo_id, 0);
    // Local subnet route (192.168.1.0/24)
    let _ = stack.routing_table.add_direct(Ipv4Address::new(192, 168, 1, 0), 24, eth0_id, 10);
    // Default Gateway route (0.0.0.0/0 -> 192.168.1.1)
    let _ = stack.routing_table.set_default_gateway(eth0_gw, eth0_id);

    // 4. Prepopulate static ARP entry for default gateway
    let gw_mac = MacAddress::new([0x52, 0x54, 0x00, 0xAA, 0xBB, 0x01]);
    stack.arp_cache.insert_static(eth0_gw, gw_mac);
}

/// Initialize the HybridOS Network Layer Core Subsystem.
pub fn init_net() -> NetworkStatus {
    let stack_mutex = get_network_stack();
    let stack = stack_mutex.lock().unwrap();

    let eth0 = stack.get_interface_by_name("eth0").unwrap();
    let lo = stack.get_interface_by_name("lo").unwrap();

    NetworkStatus {
        interface_count: stack.interfaces.len(),
        route_count: stack.routing_table.all_routes().len(),
        arp_entry_count: stack.arp_cache.len(),
        loopback_ready: lo.is_up,
        eth0_ready: eth0.is_up,
        eth0_ip: eth0.ip,
        eth0_mac: eth0.mac,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use crate::vfs::inode::InodeType;

    #[test]
    fn test_network_interface_queuing() {
        let mac = MacAddress::new([0x00, 0x11, 0x22, 0x33, 0x44, 0x55]);
        let ip = Ipv4Address::new(10, 0, 0, 2);
        let mask = Ipv4Address::new(255, 255, 255, 0);

        let mut iface = NetworkInterface::new(0, "test0", mac, ip, mask, None, 1500);

        assert!(!iface.has_tx());
        iface.queue_tx(vec![1, 2, 3]);
        assert!(iface.has_tx());
        assert_eq!(iface.pop_tx(), Some(vec![1, 2, 3]));
        assert!(!iface.has_tx());

        assert!(!iface.has_rx());
        iface.push_rx(vec![4, 5, 6]);
        assert!(iface.has_rx());
        assert_eq!(iface.pop_rx(), Some(vec![4, 5, 6]));
    }

    #[test]
    fn test_packet_dispatcher_arp_request_reply_loop() {
        let mut stack = NetworkStack::new();
        setup_default_stack(&mut stack);

        let eth0_id = stack.get_interface_by_name("eth0").unwrap().id;
        let eth0_ip = stack.get_interface(eth0_id).unwrap().ip;
        let eth0_mac = stack.get_interface(eth0_id).unwrap().mac;

        let host_ip = Ipv4Address::new(192, 168, 1, 222);
        let host_mac = MacAddress::new([0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x01]);

        // External host transmits ARP Request seeking eth0_ip
        let arp_req = ArpPacket::new_request(host_mac, host_ip, eth0_ip);
        let raw_frame = EthernetFrame::pack(
            MacAddress::BROADCAST,
            host_mac,
            EtherType::Arp,
            &arp_req.to_bytes(),
            true,
        );

        // Stack processes incoming frame
        let event = stack.process_incoming_packet(eth0_id, &raw_frame);
        assert!(matches!(event, NetworkEvent::ArpRequestAnswered { .. }));

        // Egress TX queue on eth0 should contain the generated ARP Reply
        let iface = stack.get_interface_mut(eth0_id).unwrap();
        assert!(iface.has_tx());

        let out_frame_bytes = iface.pop_tx().unwrap();
        let out_frame = EthernetFrame::unpack(&out_frame_bytes).unwrap();

        assert_eq!(out_frame.header.dst, host_mac);
        assert_eq!(out_frame.header.src, eth0_mac);
        assert_eq!(out_frame.header.ethertype, EtherType::Arp);

        let parsed_reply = ArpPacket::parse(&out_frame.payload).unwrap();
        assert_eq!(parsed_reply.operation, ArpOperation::Reply);
        assert_eq!(parsed_reply.sender_ip, eth0_ip);
        assert_eq!(parsed_reply.sender_mac, eth0_mac);
        assert_eq!(parsed_reply.target_ip, host_ip);
        assert_eq!(parsed_reply.target_mac, host_mac);

        // Also verify host was learned in stack's ARP cache
        assert_eq!(stack.arp_cache.lookup(&host_ip), Some(host_mac));
    }

    #[test]
    fn test_packet_dispatcher_icmp_ping_echo_loop() {
        let mut stack = NetworkStack::new();
        setup_default_stack(&mut stack);

        let eth0_id = stack.get_interface_by_name("eth0").unwrap().id;
        let eth0_ip = stack.get_interface(eth0_id).unwrap().ip;
        let eth0_mac = stack.get_interface(eth0_id).unwrap().mac;

        let client_ip = Ipv4Address::new(192, 168, 1, 77);
        let client_mac = MacAddress::new([0x00, 0x50, 0x56, 0xC0, 0x00, 0x08]);

        // Prepopulate ARP cache for client so return frame knows MAC
        stack.arp_cache.insert_static(client_ip, client_mac);

        // Construct incoming ping (ICMP Echo Request)
        let ping_payload = b"PING_FROM_TEST_CLIENT";
        let icmp_req = IcmpPacket::build_echo_request(0x4321, 99, ping_payload);
        let ip_pkt = Ipv4Packet::new(client_ip, eth0_ip, Ipv4Protocol::Icmp, icmp_req.to_bytes()).unwrap();

        let raw_frame = EthernetFrame::pack(
            eth0_mac,
            client_mac,
            EtherType::Ipv4,
            &ip_pkt.to_bytes(),
            true,
        );

        // Process incoming ping
        let event = stack.process_incoming_packet(eth0_id, &raw_frame);
        match event {
            NetworkEvent::IcmpEchoReplySent { target_ip, seq, .. } => {
                assert_eq!(target_ip, client_ip);
                assert_eq!(seq, 99);
            }
            other => panic!("Expected IcmpEchoReplySent, got {:?}", other),
        }

        // Verify outgoing frame on eth0
        let iface = stack.get_interface_mut(eth0_id).unwrap();
        assert!(iface.has_tx());

        let out_bytes = iface.pop_tx().unwrap();
        let out_eth = EthernetFrame::unpack(&out_bytes).unwrap();

        assert_eq!(out_eth.header.dst, client_mac);
        assert_eq!(out_eth.header.src, eth0_mac);
        assert_eq!(out_eth.header.ethertype, EtherType::Ipv4);

        let out_ip = Ipv4Packet::parse(&out_eth.payload).unwrap();
        assert_eq!(out_ip.header.src, eth0_ip);
        assert_eq!(out_ip.header.dst, client_ip);
        assert_eq!(out_ip.header.protocol, Ipv4Protocol::Icmp);

        let out_icmp = IcmpPacket::parse(&out_ip.payload).unwrap();
        assert_eq!(out_icmp.header.icmp_type, IcmpType::EchoReply);

        let echo_info = out_icmp.echo_info().unwrap();
        assert_eq!(echo_info.identifier, 0x4321);
        assert_eq!(echo_info.sequence_number, 99);
        assert_eq!(out_icmp.payload, ping_payload);
    }

    #[test]
    fn test_send_ipv4_and_ping() {
        let mut stack = NetworkStack::new();
        setup_default_stack(&mut stack);

        // Target on local subnet with pre-populated ARP cache
        let target_ip = Ipv4Address::new(192, 168, 1, 55);
        let target_mac = MacAddress::new([0x00, 0x11, 0x22, 0x33, 0x44, 0x55]);
        stack.arp_cache.insert_static(target_ip, target_mac);

        // Send ping
        let seq = stack.ping(target_ip, b"TEST_PING_PAYLOAD").expect("Ping sent");
        assert_eq!(seq, 1);

        let eth0 = stack.get_interface_by_name_mut("eth0").unwrap();
        assert!(eth0.has_tx());
        let frame_bytes = eth0.pop_tx().unwrap();

        let eth_frame = EthernetFrame::unpack(&frame_bytes).unwrap();
        assert_eq!(eth_frame.header.dst, target_mac);
        assert_eq!(eth_frame.header.ethertype, EtherType::Ipv4);
    }

    #[test]
    fn test_init_net_global() {
        let status = init_net();
        assert!(status.interface_count >= 2);
        assert!(status.route_count >= 3);
        assert!(status.loopback_ready);
        assert!(status.eth0_ready);
        assert_eq!(status.eth0_ip, Ipv4Address::new(192, 168, 1, 100));
        assert_eq!(status.eth0_mac, MacAddress::new([0x52, 0x54, 0x00, 0x12, 0x34, 0x56]));
    }

    // ========================================================================
    // Section: UDP Unit Tests
    // ========================================================================

    #[test]
    fn test_udp_header_serialization() {
        let header = UdpHeader::new(8080, 9090, 12);
        assert_eq!(header.src_port, 8080);
        assert_eq!(header.dst_port, 9090);
        assert_eq!(header.length, 20); // 8 header + 12 payload
        assert_eq!(header.checksum, 0);

        let bytes = header.to_bytes();
        assert_eq!(bytes.len(), 8);

        let parsed = UdpHeader::from_bytes(&bytes).expect("Failed to parse UDP header");
        assert_eq!(parsed.src_port, 8080);
        assert_eq!(parsed.dst_port, 9090);
        assert_eq!(parsed.length, 20);
    }

    #[test]
    fn test_udp_checksum_calculation_and_verification() {
        let src_ip = [192, 168, 1, 100];
        let dst_ip = [192, 168, 1, 1];
        let payload = b"Hello, HybridOS UDP!";

        let mut header = UdpHeader::new(5000, 53, payload.len());
        let csum = compute_udp_checksum(src_ip, dst_ip, &header, payload);
        assert_ne!(csum, 0);

        header.checksum = csum;
        assert!(verify_udp_checksum(src_ip, dst_ip, &header, payload));

        // Test corruption detection
        header.checksum = csum.wrapping_add(1);
        assert!(!verify_udp_checksum(src_ip, dst_ip, &header, payload));
    }

    #[test]
    fn test_udp_demux_binding_and_ephemeral_alloc() {
        let demux = PortDemuxTable::new();
        let queue1 = Arc::new(UdpQueue::new(16));
        let queue2 = Arc::new(UdpQueue::new(16));

        assert!(demux.bind(5050, queue1).is_ok());
        assert!(demux.is_bound(5050));

        // Duplicate bind should fail
        assert_eq!(demux.bind(5050, queue2.clone()), Err(UdpError::PortAlreadyBound));

        // Ephemeral allocation
        let eph_port = demux.bind_ephemeral(queue2).expect("Ephemeral bind failed");
        assert!(eph_port >= udp::EPHEMERAL_PORT_START);
        assert!(demux.is_bound(eph_port));

        // Unbind
        assert!(demux.unbind(5050).is_ok());
        assert!(!demux.is_bound(5050));
    }

    #[test]
    fn test_udp_loopback_transmission() {
        let sub = UdpSubsystem::new();
        let rx_queue = Arc::new(UdpQueue::new(32));

        sub.demux.bind(7777, rx_queue.clone()).expect("Failed to bind port");

        let payload = b"Loopback Datagram Test";
        let datagram = UdpDatagram::new(
            [127, 0, 0, 1],
            [127, 0, 0, 1],
            45000,
            7777,
            payload.to_vec(),
        );

        sub.send(datagram).expect("Loopback send failed");
        assert_eq!(rx_queue.len(), 1);

        let received = rx_queue.pop().expect("No datagram received");
        assert_eq!(received.src_port, 45000);
        assert_eq!(received.dst_port, 7777);
        assert_eq!(received.data, payload);
    }

    // ========================================================================
    // Section: TCP Unit Tests
    // ========================================================================

    #[test]
    fn test_tcp_flags_and_header() {
        let mut flags = TcpFlags::empty();
        assert!(!flags.is_syn());
        flags.set(TcpFlags::SYN, true);
        flags.set(TcpFlags::ACK, true);
        assert!(flags.is_syn());
        assert!(flags.is_ack());
        assert_eq!(flags.bits(), TcpFlags::SYN | TcpFlags::ACK);

        let header = TcpHeader::new(80, 12345, 100, 200, flags, 65535);
        let bytes = header.to_bytes();
        assert_eq!(bytes.len(), 20);

        let (parsed, len) = TcpHeader::from_bytes(&bytes).expect("Failed to parse TCP header");
        assert_eq!(len, 20);
        assert_eq!(parsed.src_port, 80);
        assert_eq!(parsed.dst_port, 12345);
        assert_eq!(parsed.seq_num, 100);
        assert_eq!(parsed.ack_num, 200);
        assert!(parsed.flags.is_syn());
        assert!(parsed.flags.is_ack());
        assert_eq!(parsed.window_size, 65535);
    }

    #[test]
    fn test_tcp_sequence_number_comparisons() {
        assert!(seq_lt(10, 20));
        assert!(seq_lte(20, 20));
        assert!(seq_gt(30, 20));
        assert!(seq_gte(20, 20));

        // Test wrapping around 2^32
        let a = 0xFFFF_FFFEu32;
        let b = 2u32;
        assert!(seq_lt(a, b));
        assert!(seq_gt(b, a));
    }

    #[test]
    fn test_tcp_3way_handshake_and_established_state() {
        let client_ip = [127, 0, 0, 1];
        let server_ip = [127, 0, 0, 1];
        let client_port = 50000;
        let server_port = 80;

        let mut server = TcpConnection::new_listen(server_ip, server_port, 1000);
        assert_eq!(server.state, TcpState::Listen);

        let mut client = TcpConnection::new(client_ip, client_port, server_ip, server_port, 500);
        assert_eq!(client.state, TcpState::Closed);

        // 1. Client initiates connect (SYN)
        let syn_seg = client.connect().expect("Client connect failed");
        assert_eq!(client.state, TcpState::SynSent);
        assert!(syn_seg.header.flags.is_syn());
        assert_eq!(syn_seg.header.seq_num, 500);

        // 2. Server receives SYN, responds with SYN-ACK
        let syn_ack_seg = server
            .process_segment(&syn_seg)
            .expect("Server failed to process SYN")
            .expect("Server did not send SYN-ACK");
        assert_eq!(server.state, TcpState::SynRcvd);
        assert!(syn_ack_seg.header.flags.is_syn());
        assert!(syn_ack_seg.header.flags.is_ack());
        assert_eq!(syn_ack_seg.header.ack_num, 501); // 500 + 1

        // 3. Client receives SYN-ACK, responds with ACK
        let ack_seg = client
            .process_segment(&syn_ack_seg)
            .expect("Client failed to process SYN-ACK")
            .expect("Client did not send final ACK");
        assert_eq!(client.state, TcpState::Established);
        assert!(ack_seg.header.flags.is_ack());
        assert_eq!(ack_seg.header.ack_num, 1001); // 1000 + 1

        // 4. Server receives final ACK -> ESTABLISHED
        let resp = server.process_segment(&ack_seg).expect("Server failed to process final ACK");
        assert!(resp.is_none());
        assert_eq!(server.state, TcpState::Established);

        // Both endpoints are now ESTABLISHED
        assert!(client.state.is_established());
        assert!(server.state.is_established());
    }

    #[test]
    fn test_tcp_data_transfer_and_sliding_window() {
        let client_ip = [127, 0, 0, 1];
        let server_ip = [127, 0, 0, 1];
        let mut client = TcpConnection::new(client_ip, 50001, server_ip, 8080, 100);
        let mut server = TcpConnection::new(server_ip, 8080, client_ip, 50001, 200);

        // Move directly to Established for data test
        client.state = TcpState::Established;
        server.state = TcpState::Established;
        client.seq.rcv_nxt = 200;
        server.seq.rcv_nxt = 100;

        let payload = b"Streaming TCP data into sliding window";
        let segments = client.send_data(payload).expect("send_data failed");
        assert_eq!(segments.len(), 1);

        // Server processes segment
        let ack_resp = server
            .process_segment(&segments[0])
            .expect("Server failed to process data")
            .expect("Server did not send ACK for data");
        assert!(ack_resp.header.flags.is_ack());
        assert_eq!(ack_resp.header.ack_num, 100 + payload.len() as u32);

        // Client processes ACK
        let _ = client.process_segment(&ack_resp);
        assert_eq!(client.seq.snd_una, 100 + payload.len() as u32);

        // Server reads received data
        let mut read_buf = [0u8; 64];
        let n = server.read_data(&mut read_buf);
        assert_eq!(n, payload.len());
        assert_eq!(&read_buf[..n], payload);
    }

    #[test]
    fn test_tcp_connection_teardown() {
        let mut client = TcpConnection::new([127, 0, 0, 1], 50002, [127, 0, 0, 1], 80, 100);
        let mut server = TcpConnection::new([127, 0, 0, 1], 80, [127, 0, 0, 1], 50002, 200);

        client.state = TcpState::Established;
        server.state = TcpState::Established;
        client.seq.rcv_nxt = 200;
        server.seq.rcv_nxt = 100;

        // 1. Client initiates active close -> FIN_WAIT_1
        let fin_seg = client.close().expect("close failed").expect("No FIN segment");
        assert_eq!(client.state, TcpState::FinWait1);
        assert!(fin_seg.header.flags.is_fin());

        // 2. Server receives FIN -> sends ACK -> CLOSE_WAIT
        let ack_seg = server
            .process_segment(&fin_seg)
            .expect("Server failed to process FIN")
            .expect("Server did not send ACK for FIN");
        assert_eq!(server.state, TcpState::CloseWait);

        // 3. Client receives ACK -> FIN_WAIT_2
        let _ = client.process_segment(&ack_seg);
        assert_eq!(client.state, TcpState::FinWait2);

        // 4. Server calls close -> sends FIN -> LAST_ACK
        let srv_fin = server.close().expect("Server close failed").expect("No server FIN");
        assert_eq!(server.state, TcpState::LastAck);

        // 5. Client receives server FIN -> sends ACK -> TIME_WAIT
        let cl_ack = client
            .process_segment(&srv_fin)
            .expect("Client failed to process server FIN")
            .expect("No client final ACK");
        assert_eq!(client.state, TcpState::TimeWait);

        // 6. Server receives ACK -> CLOSED
        let _ = server.process_segment(&cl_ack);
        assert_eq!(server.state, TcpState::Closed);

        // 7. Time wait expiration
        client.expire_time_wait();
        assert_eq!(client.state, TcpState::Closed);
    }

    // ========================================================================
    // Section: BSD Socket Unit Tests
    // ========================================================================

    #[test]
    fn test_ring_buffer_operations() {
        let mut rb = RingBuffer::new(16);
        assert!(rb.is_empty());
        assert_eq!(rb.available_write(), 16);

        let data1 = b"12345678";
        assert_eq!(rb.write(data1), 8);
        assert_eq!(rb.available_read(), 8);
        assert_eq!(rb.available_write(), 8);

        let mut peek_buf = [0u8; 4];
        assert_eq!(rb.peek(&mut peek_buf), 4);
        assert_eq!(&peek_buf, b"1234");
        assert_eq!(rb.available_read(), 8); // Peek does not consume

        let mut read_buf = [0u8; 8];
        assert_eq!(rb.read(&mut read_buf), 8);
        assert_eq!(&read_buf, data1);
        assert!(rb.is_empty());

        // Test circular wraparound
        let data2 = b"ABCDEFGHIJKL";
        assert_eq!(rb.write(data2), 12);
        let mut small_read = [0u8; 6];
        assert_eq!(rb.read(&mut small_read), 6);
        assert_eq!(&small_read, b"ABCDEF");

        let data3 = b"MNOPQR";
        assert_eq!(rb.write(data3), 6);
        let mut final_buf = [0u8; 12];
        let n = rb.read(&mut final_buf);
        assert_eq!(n, 12);
        assert_eq!(&final_buf[..n], b"GHIJKLMNOPQR");
    }

    #[test]
    fn test_bsd_socket_udp_loopback() {
        let sock1 = Socket::new(AddressFamily::AfInet, SocketType::Dgram, SocketProtocol::Udp);
        let sock2 = Socket::new(AddressFamily::AfInet, SocketType::Dgram, SocketProtocol::Udp);

        let addr1 = SocketAddress::loopback(9001);
        let addr2 = SocketAddress::loopback(9002);

        sock1.bind(addr1).expect("Failed to bind sock1");
        sock2.bind(addr2).expect("Failed to bind sock2");

        let msg = b"HybridOS UDP BSD Socket Packet";
        let sent = sock1.sendto(msg, addr2).expect("sendto failed");
        assert_eq!(sent, msg.len());

        let mut rx_buf = [0u8; 64];
        let (rcvd, src) = sock2.recvfrom(&mut rx_buf).expect("recvfrom failed");
        assert_eq!(rcvd, msg.len());
        assert_eq!(&rx_buf[..rcvd], msg);
        assert_eq!(src, addr1);

        sock1.close().unwrap();
        sock2.close().unwrap();
    }

    #[test]
    fn test_bsd_socket_stream_connection() {
        let server = Socket::new(AddressFamily::AfInet, SocketType::Stream, SocketProtocol::Tcp);
        let client = Socket::new(AddressFamily::AfInet, SocketType::Stream, SocketProtocol::Tcp);

        let srv_addr = SocketAddress::loopback(9099);
        server.bind(srv_addr).expect("Server bind failed");
        server.listen(5).expect("Server listen failed");

        client.connect(srv_addr).expect("Client connect failed");

        let (accepted, peer_addr) = server.accept().expect("Server accept failed");
        assert_eq!(peer_addr, client.local_address().unwrap());

        // Bidirectional transfer: Client -> Server
        let client_msg = b"Hello from BSD Client!";
        let written = client.send(client_msg).expect("Client send failed");
        assert_eq!(written, client_msg.len());

        let mut srv_buf = [0u8; 64];
        let read = accepted.recv(&mut srv_buf).expect("Server recv failed");
        assert_eq!(read, client_msg.len());
        assert_eq!(&srv_buf[..read], client_msg);

        // Server -> Client
        let srv_msg = b"Response from BSD Server!";
        let written2 = accepted.send(srv_msg).expect("Server send failed");
        assert_eq!(written2, srv_msg.len());

        let mut cl_buf = [0u8; 64];
        let read2 = client.recv(&mut cl_buf).expect("Client recv failed");
        assert_eq!(read2, srv_msg.len());
        assert_eq!(&cl_buf[..read2], srv_msg);

        client.close().unwrap();
        accepted.close().unwrap();
        server.close().unwrap();
    }

    #[test]
    fn test_bsd_socket_nonblocking_mode() {
        let sock = Socket::new(AddressFamily::AfInet, SocketType::Stream, SocketProtocol::Tcp);
        sock.set_nonblocking(true);
        assert!(sock.is_nonblocking());

        // In non-blocking unbound state or empty stream, should return NotConnected / WouldBlock
        let mut buf = [0u8; 16];
        let err = sock.recv(&mut buf).unwrap_err();
        assert_eq!(err, SocketError::NotConnected);
    }

    #[test]
    fn test_socket_vfs_file_descriptor_bridge() {
        let server = Socket::new(AddressFamily::AfInet, SocketType::Stream, SocketProtocol::Tcp);
        let client = Socket::new(AddressFamily::AfInet, SocketType::Stream, SocketProtocol::Tcp);

        let addr = SocketAddress::loopback(9191);
        server.bind(addr).expect("Bind failed");
        server.listen(1).expect("Listen failed");
        client.connect(addr).expect("Connect failed");

        let (accepted, _) = server.accept().expect("Accept failed");

        // Convert client socket to VFS FileDescriptor
        let client_fd = client.into_file_descriptor(3);
        assert_eq!(client_fd.id, 3);
        assert_eq!(client_fd.inode.ino_type, InodeType::Socket);

        // Convert accepted server socket to VFS Inode and test FileOperations
        let accepted_inode = accepted.into_inode();

        // Write via client VFS descriptor
        let write_msg = b"VFS Inode Stream Test";
        let written = client_fd.inode.ops.write(0, write_msg).expect("VFS write failed");
        assert_eq!(written, write_msg.len());

        // Read via server VFS Inode
        let mut read_buf = [0u8; 64];
        let n = accepted_inode.ops.read(0, &mut read_buf).expect("VFS read failed");
        assert_eq!(n, write_msg.len());
        assert_eq!(&read_buf[..n], write_msg);

        // Test IOCTL FIONREAD on accepted socket ops
        let available = accepted_inode.ops.ioctl(FIONREAD, 0).expect("IOCTL failed");
        assert_eq!(available, 0); // All bytes consumed

        client.close().unwrap();
        accepted.close().unwrap();
        server.close().unwrap();
    }
}
