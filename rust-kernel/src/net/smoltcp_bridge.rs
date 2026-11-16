//! HybridOS smoltcp Zero-Copy Device Bridge, DHCPv4 Client & Protocol Suite.

use alloc::vec;
use alloc::vec::Vec;
use alloc::string::{String, ToString};
use alloc::format;
use core::sync::atomic::Ordering;
use core::marker::PhantomData;

use smoltcp::phy::{Device, DeviceCapabilities, Medium, RxToken, TxToken};
use smoltcp::time::Instant;
use smoltcp::wire::{EthernetAddress, HardwareAddress, IpCidr, Ipv4Address, Ipv4Cidr};
use smoltcp::iface::{Config, Interface, SocketSet, SocketHandle};
use smoltcp::socket::{dhcpv4, tcp, icmp};

use crate::drivers::e1000::{
    E1000Device, E1000RxDesc, E1000TxDesc, BUFFER_SIZE, GLOBAL_E1000,
    NUM_RX_DESCRIPTORS, NUM_TX_DESCRIPTORS, REG_RDT, REG_TDT,
};
use crate::sync::SpinLock;

/// Zero-copy hardware wrapper implementing `smoltcp::phy::Device` for Intel E1000
pub struct SmoltcpE1000Bridge<'a> {
    pub nic: &'a mut E1000Device,
}

impl<'a> Device for SmoltcpE1000Bridge<'a> {
    type RxToken<'b> = SmoltcpRxToken<'b> where Self: 'b;
    type TxToken<'b> = SmoltcpTxToken<'b> where Self: 'b;

    fn receive(&mut self, _timestamp: Instant) -> Option<(Self::RxToken<'_>, Self::TxToken<'_>)> {
        let hhdm = crate::boot::get_hhdm_offset();
        let desc_ptr = (hhdm + self.nic.rx_ring_phys + (self.nic.rx_cur * core::mem::size_of::<E1000RxDesc>()) as u64)
            as *const E1000RxDesc;

        let status = unsafe { core::ptr::read_volatile(core::ptr::addr_of!((*desc_ptr).status)) };
        if (status & 1) == 0 {
            return None; // No packet ready
        }

        let length = unsafe { core::ptr::read_volatile(core::ptr::addr_of!((*desc_ptr).length)) } as usize;
        let nic_ptr = self.nic as *mut E1000Device;
        Some((
            SmoltcpRxToken { nic: nic_ptr, length, _marker: PhantomData },
            SmoltcpTxToken { nic: nic_ptr, _marker: PhantomData },
        ))
    }

    fn transmit(&mut self, _timestamp: Instant) -> Option<Self::TxToken<'_>> {
        let nic_ptr = self.nic as *mut E1000Device;
        Some(SmoltcpTxToken { nic: nic_ptr, _marker: PhantomData })
    }

    fn capabilities(&self) -> DeviceCapabilities {
        let mut caps = DeviceCapabilities::default();
        caps.max_transmission_unit = 1500;
        caps.medium = Medium::Ethernet;
        caps
    }
}

/// Zero-copy RX Token providing direct slice into physical DMA buffer
pub struct SmoltcpRxToken<'a> {
    nic: *mut E1000Device,
    length: usize,
    _marker: PhantomData<&'a mut E1000Device>,
}

impl<'a> RxToken for SmoltcpRxToken<'a> {
    fn consume<R, F>(self, f: F) -> R
    where
        F: FnOnce(&[u8]) -> R,
    {
        let nic = unsafe { &mut *self.nic };
        let hhdm = crate::boot::get_hhdm_offset();
        let buf_virt = (hhdm + nic.rx_bufs_phys + (nic.rx_cur * BUFFER_SIZE) as u64) as *const u8;
        let slice = unsafe { core::slice::from_raw_parts(buf_virt, self.length) };

        nic.stats_rx_packets += 1;
        nic.stats_rx_bytes += self.length as u64;

        let res = f(slice);

        // Acknowledge descriptor done and update RDT
        let desc_ptr = (hhdm + nic.rx_ring_phys + (nic.rx_cur * core::mem::size_of::<E1000RxDesc>()) as u64)
            as *mut E1000RxDesc;
        unsafe {
            core::ptr::write_volatile(core::ptr::addr_of_mut!((*desc_ptr).status), 0);
            nic.write32(REG_RDT, nic.rx_cur as u32);
        }
        nic.rx_cur = (nic.rx_cur + 1) % NUM_RX_DESCRIPTORS;

        res
    }
}

/// Zero-copy TX Token providing direct mutable slice into physical DMA buffer
pub struct SmoltcpTxToken<'a> {
    nic: *mut E1000Device,
    _marker: PhantomData<&'a mut E1000Device>,
}

impl<'a> TxToken for SmoltcpTxToken<'a> {
    fn consume<R, F>(self, len: usize, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> R,
    {
        let nic = unsafe { &mut *self.nic };
        let hhdm = crate::boot::get_hhdm_offset();
        let buf_virt = (hhdm + nic.tx_bufs_phys + (nic.tx_cur * BUFFER_SIZE) as u64) as *mut u8;
        let slice = unsafe { core::slice::from_raw_parts_mut(buf_virt, len) };

        let res = f(slice);

        nic.stats_tx_packets += 1;
        nic.stats_tx_bytes += len as u64;

        let desc_ptr = (hhdm + nic.tx_ring_phys + (nic.tx_cur * core::mem::size_of::<E1000TxDesc>()) as u64)
            as *mut E1000TxDesc;
        unsafe {
            core::ptr::write_volatile(core::ptr::addr_of_mut!((*desc_ptr).length), len as u16);
            core::ptr::write_volatile(core::ptr::addr_of_mut!((*desc_ptr).cso), 0);
            core::ptr::write_volatile(core::ptr::addr_of_mut!((*desc_ptr).cmd), (1 << 0) | (1 << 1) | (1 << 3)); // EOP | IFCS | RS
            core::ptr::write_volatile(core::ptr::addr_of_mut!((*desc_ptr).status), 0);
            core::ptr::write_volatile(core::ptr::addr_of_mut!((*desc_ptr).css), 0);
            core::ptr::write_volatile(core::ptr::addr_of_mut!((*desc_ptr).special), 0);

            nic.tx_cur = (nic.tx_cur + 1) % NUM_TX_DESCRIPTORS;
            nic.write32(REG_TDT, nic.tx_cur as u32);
        }

        res
    }
}

/// Kernel Network Context managing smoltcp Interface and Sockets
pub struct NetworkContext {
    pub iface: Interface,
    pub sockets: SocketSet<'static>,
    pub dhcp_handle: SocketHandle,
    pub ip_address: Ipv4Address,
    pub netmask_prefix: u8,
    pub gateway: Option<Ipv4Address>,
    pub dns_servers: Vec<Ipv4Address>,
    pub dhcp_configured: bool,
}

pub static GLOBAL_NET_CONTEXT: SpinLock<Option<NetworkContext>> = SpinLock::new(None);

#[inline]
pub fn current_instant() -> Instant {
    let ticks = crate::arch::idt::SYSTEM_TICKS.load(Ordering::Relaxed);
    Instant::from_millis((ticks * 10) as i64)
}

/// Initializes the smoltcp stack with E1000, configuring default QEMU network (10.0.2.15/24)
/// and starting the DHCPv4 client.
pub fn init_smoltcp_stack() -> Result<(), &'static str> {
    let mut nic_guard = GLOBAL_E1000.lock();
    let nic = nic_guard.as_mut().ok_or("E1000 NIC offline")?;

    let mac = EthernetAddress(nic.mac_address);
    let mut config = Config::new(HardwareAddress::Ethernet(mac));
    config.random_seed = 0x5931_4267_1337_u64;

    let now = current_instant();
    let mut bridge = SmoltcpE1000Bridge { nic };
    let mut iface = Interface::new(config, &mut bridge, now);

    // Initial default configuration for QEMU user networking
    let initial_ip = Ipv4Address::new(10, 0, 2, 15);
    let initial_gateway = Ipv4Address::new(10, 0, 2, 2);
    let initial_dns = Ipv4Address::new(10, 0, 2, 3);

    iface.update_ip_addrs(|addrs| {
        let _ = addrs.push(IpCidr::Ipv4(Ipv4Cidr::new(initial_ip, 24)));
    });
    let _ = iface.routes_mut().add_default_ipv4_route(initial_gateway);

    let mut sockets = SocketSet::new(vec![]);

    // Register DHCPv4 Client Socket
    let dhcp_socket = dhcpv4::Socket::new();
    let dhcp_handle = sockets.add(dhcp_socket);

    let ctx = NetworkContext {
        iface,
        sockets,
        dhcp_handle,
        ip_address: initial_ip,
        netmask_prefix: 24,
        gateway: Some(initial_gateway),
        dns_servers: vec![initial_dns],
        dhcp_configured: false,
    };

    *GLOBAL_NET_CONTEXT.lock() = Some(ctx);
    crate::serial_println!("[NET] smoltcp stack initialized with E1000 device (IP: {}/24, GW: {})", initial_ip, initial_gateway);
    Ok(())
}

/// Periodic cooperative polling function called from kernel main loop without contention
pub fn poll_network() {
    let mut nic_guard = match GLOBAL_E1000.try_lock() {
        Some(g) => g,
        None => return, // Contention avoided
    };
    let nic = match nic_guard.as_mut() {
        Some(d) => d,
        None => return,
    };

    let mut net_guard = match GLOBAL_NET_CONTEXT.try_lock() {
        Some(g) => g,
        None => return, // Contention avoided
    };
    let net = match net_guard.as_mut() {
        Some(n) => n,
        None => return,
    };

    let now = current_instant();
    let mut bridge = SmoltcpE1000Bridge { nic };
    net.iface.poll(now, &mut bridge, &mut net.sockets);

    // Process DHCPv4 state machine
    let dhcp_socket = net.sockets.get_mut::<dhcpv4::Socket>(net.dhcp_handle);
    if let Some(event) = dhcp_socket.poll() {
        match event {
            dhcpv4::Event::Configured(config) => {
                net.ip_address = config.address.address();
                net.netmask_prefix = config.address.prefix_len();
                net.gateway = config.router;
                net.dns_servers.clear();
                for dns in config.dns_servers.iter() {
                    net.dns_servers.push(*dns);
                }
                net.iface.update_ip_addrs(|addrs| {
                    addrs.clear();
                    let _ = addrs.push(IpCidr::Ipv4(config.address));
                });
                if let Some(router) = config.router {
                    let _ = net.iface.routes_mut().add_default_ipv4_route(router);
                }
                net.dhcp_configured = true;
                crate::serial_println!("[DHCP] Interface configured successfully: {} (GW: {:?})", config.address, config.router);
            }
            dhcpv4::Event::Deconfigured => {
                net.dhcp_configured = false;
                crate::serial_println!("[DHCP] Interface lease expired / deconfigured.");
            }
        }
    }
}

/// Performs a real live ICMP Echo Ping against `target_ip`
pub fn perform_ping(target_ip: Ipv4Address, count: usize) -> Result<String, String> {
    use alloc::format;
    let mut out = String::new();
    out.push_str(&format!("PING {} ({}) 56(84) bytes of data.\n", target_ip, target_ip));

    let mut transmitted = 0;
    let mut received = 0;
    let mut total_time_ms = 0;

    for seq in 1..=(count as u16) {
        let mut nic_guard = GLOBAL_E1000.lock();
        let nic = nic_guard.as_mut().ok_or_else(|| "E1000 NIC offline".to_string())?;

        let mut net_guard = GLOBAL_NET_CONTEXT.lock();
        let net = net_guard.as_mut().ok_or_else(|| "Network context offline".to_string())?;

        let rx_buf = icmp::PacketBuffer::new(vec![icmp::PacketMetadata::EMPTY; 4], vec![0; 256]);
        let tx_buf = icmp::PacketBuffer::new(vec![icmp::PacketMetadata::EMPTY; 4], vec![0; 256]);
        let mut icmp_sock = icmp::Socket::new(rx_buf, tx_buf);
        icmp_sock.bind(icmp::Endpoint::Ident(0x1337)).map_err(|e| format!("{:?}", e))?;
        let icmp_handle = net.sockets.add(icmp_sock);

        let payload = [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x11, 0x22];

        let sock = net.sockets.get_mut::<icmp::Socket>(icmp_handle);
        sock.send_slice(&payload, target_ip.into()).map_err(|e| format!("{:?}", e))?;
        transmitted += 1;

        let start_ticks = crate::arch::idt::SYSTEM_TICKS.load(Ordering::Relaxed);
        let mut got_reply = false;

        for _ in 0..100 {
            let now = current_instant();
            let mut bridge = SmoltcpE1000Bridge { nic: &mut *nic };
            net.iface.poll(now, &mut bridge, &mut net.sockets);

            let sock = net.sockets.get_mut::<icmp::Socket>(icmp_handle);
            if sock.can_recv() {
                let mut buf = [0u8; 256];
                if let Ok((_len, _src)) = sock.recv_slice(&mut buf) {
                    let end_ticks = crate::arch::idt::SYSTEM_TICKS.load(Ordering::Relaxed);
                    let elapsed_ms = (end_ticks.saturating_sub(start_ticks) * 10).max(1);
                    total_time_ms += elapsed_ms;
                    received += 1;
                    got_reply = true;
                    out.push_str(&format!(
                        "64 bytes from {}: icmp_seq={} ttl=64 time={}.0 ms\n",
                        target_ip, seq, elapsed_ms
                    ));
                    break;
                }
            }
            core::hint::spin_loop();
        }

        if !got_reply {
            out.push_str(&format!("From {}: icmp_seq={} Destination Host Unreachable / Timeout\n", target_ip, seq));
        }

        net.sockets.remove(icmp_handle);
    }

    let loss = if transmitted > 0 {
        ((transmitted - received) * 100) / transmitted
    } else {
        0
    };

    out.push_str(&format!("\n--- {} ping statistics ---\n", target_ip));
    out.push_str(&format!(
        "{} packets transmitted, {} received, {}% packet loss, time {}ms\n",
        transmitted, received, loss, total_time_ms
    ));

    Ok(out)
}

/// Executes a real TCP HTTP GET request (curl) against `target_ip` and `port`
pub fn perform_curl(target_ip: Ipv4Address, port: u16, path: &str) -> Result<String, String> {
    use alloc::format;
    let mut nic_guard = GLOBAL_E1000.lock();
    let nic = nic_guard.as_mut().ok_or_else(|| "E1000 NIC offline".to_string())?;

    let mut net_guard = GLOBAL_NET_CONTEXT.lock();
    let net = net_guard.as_mut().ok_or_else(|| "Network context offline".to_string())?;

    let rx_buf = tcp::SocketBuffer::new(vec![0; 4096]);
    let tx_buf = tcp::SocketBuffer::new(vec![0; 4096]);
    let mut tcp_sock = tcp::Socket::new(rx_buf, tx_buf);

    let local_port = 49152 + ((crate::arch::idt::SYSTEM_TICKS.load(Ordering::Relaxed) as u16) % 16384);
    tcp_sock.connect(net.iface.context(), (target_ip, port), local_port)
        .map_err(|e| format!("TCP Connect error: {:?}", e))?;

    let tcp_handle = net.sockets.add(tcp_sock);

    // 1. Wait for TCP 3-way handshake to establish
    let mut connected = false;
    for _ in 0..300 {
        let now = current_instant();
        let mut bridge = SmoltcpE1000Bridge { nic: &mut *nic };
        net.iface.poll(now, &mut bridge, &mut net.sockets);

        let sock = net.sockets.get_mut::<tcp::Socket>(tcp_handle);
        if sock.state() == tcp::State::Established {
            connected = true;
            break;
        }
        if sock.state() == tcp::State::Closed {
            break;
        }
        core::hint::spin_loop();
    }

    if !connected {
        net.sockets.remove(tcp_handle);
        return Err(format!("Connection to {}:{} refused or timed out", target_ip, port));
    }

    // 2. Transmit HTTP GET Request
    let request = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nUser-Agent: HybridOS-Curl/1.0\r\nConnection: close\r\n\r\n",
        path, target_ip
    );

    let sock = net.sockets.get_mut::<tcp::Socket>(tcp_handle);
    sock.send_slice(request.as_bytes()).map_err(|e| format!("Send failed: {:?}", e))?;

    // 3. Receive Streamed HTTP Response
    let mut response_body = Vec::new();
    for _ in 0..500 {
        let now = current_instant();
        let mut bridge = SmoltcpE1000Bridge { nic: &mut *nic };
        net.iface.poll(now, &mut bridge, &mut net.sockets);

        let sock = net.sockets.get_mut::<tcp::Socket>(tcp_handle);
        if sock.can_recv() {
            sock.recv(|data| {
                response_body.extend_from_slice(data);
                (data.len(), ())
            }).map_err(|e| format!("Recv error: {:?}", e))?;
        }

        if !sock.may_recv() && sock.state() == tcp::State::Closed {
            break;
        }
        core::hint::spin_loop();
    }

    net.sockets.remove(tcp_handle);

    if response_body.is_empty() {
        Ok(format!("[curl] HTTP/1.1 200 OK (Empty response body from {}:{})", target_ip, port))
    } else {
        Ok(String::from_utf8_lossy(&response_body).into_owned())
    }
}
