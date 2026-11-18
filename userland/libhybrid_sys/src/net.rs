//! libhybrid_sys::net — BSD Socket syscall wrappers for HybridOS Ring 3.
//!
//! These wrappers issue real Linux System V AMD64 syscalls that are handled by the
//! HybridOS kernel's POSIX BSD socket subsystem (`rust-kernel/src/syscall/posix_advanced.rs`),
//! which is the Ring 0 interface to the network stack (smoltcp / E1000 / ARP / IPv4 / TCP / UDP / ICMP).
//!
//! Syscall numbers (x86_64, System V AMD64 ABI, args in rdi/rsi/rdx/r10/r8/r9):
//!   41 sys_socket(domain, type, protocol)
//!   42 sys_connect(sockfd, addr, addrlen)
//!   44 sys_sendto(sockfd, buf, len, flags, dest_addr, addrlen)
//!   45 sys_recvfrom(sockfd, buf, len, flags, src_addr, addrlen)
//!   49 sys_bind(sockfd, addr, addrlen)
//!   50 sys_listen(sockfd, backlog)

// ----------------------------------------------------------------------------
// Address families
// ----------------------------------------------------------------------------
pub const AF_UNSPEC: i32 = 0;
pub const AF_INET: i32 = 2;

// ----------------------------------------------------------------------------
// Socket types
// ----------------------------------------------------------------------------
pub const SOCK_STREAM: i32 = 1;
pub const SOCK_DGRAM: i32 = 2;
pub const SOCK_RAW: i32 = 3;

// ----------------------------------------------------------------------------
// IP protocols
// ----------------------------------------------------------------------------
pub const IPPROTO_ICMP: i32 = 1;
pub const IPPROTO_TCP: i32 = 6;
pub const IPPROTO_UDP: i32 = 17;

// ----------------------------------------------------------------------------
// Socket option levels / names (SOL_SOCKET)
// ----------------------------------------------------------------------------
pub const SOL_SOCKET: i32 = 1;
pub const SO_REUSEADDR: i32 = 2;
pub const SO_RCVBUF: i32 = 8;
pub const SO_SNDBUF: i32 = 7;

// ----------------------------------------------------------------------------
// Shutdown directives
// ----------------------------------------------------------------------------
pub const SHUT_RD: i32 = 0;
pub const SHUT_WR: i32 = 1;
pub const SHUT_RDWR: i32 = 2;

// ----------------------------------------------------------------------------
// Syscall numbers
// ----------------------------------------------------------------------------
pub const SYS_SOCKET: u64 = 41;
pub const SYS_CONNECT: u64 = 42;
pub const SYS_SENDTO: u64 = 44;
pub const SYS_RECVFROM: u64 = 45;
pub const SYS_BIND: u64 = 49;
pub const SYS_LISTEN: u64 = 50;

// ----------------------------------------------------------------------------
// sockaddr_in (network byte order for port and address)
// ----------------------------------------------------------------------------
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SockAddrIn {
    pub sin_family: u16,
    pub sin_port: u16,   // network byte order
    pub sin_addr: u32,   // network byte order
    pub sin_zero: [u8; 8],
}

impl SockAddrIn {
    pub fn new(ip: [u8; 4], port: u16) -> Self {
        Self {
            sin_family: AF_INET as u16,
            sin_port: port.to_be(),
            sin_addr: u32::from_be_bytes(ip),
            sin_zero: [0; 8],
        }
    }

    pub fn any(port: u16) -> Self {
        Self::new([0, 0, 0, 0], port)
    }

    pub fn loopback(port: u16) -> Self {
        Self::new([127, 0, 0, 1], port)
    }

    pub fn ip_bytes(&self) -> [u8; 4] {
        self.sin_addr.to_be_bytes()
    }

    pub fn port(&self) -> u16 {
        u16::from_be(self.sin_port)
    }
}

/// Size of a `sockaddr_in` in bytes, as expected by the kernel syscalls.
pub const SOCKADDR_IN_LEN: u64 = core::mem::size_of::<SockAddrIn>() as u64;

// ----------------------------------------------------------------------------
// Syscall wrappers
// ----------------------------------------------------------------------------

/// `sys_socket(41)`: create a socket, returns a file descriptor (>=0) or a negative error.
pub fn sys_socket(domain: i32, ty: i32, protocol: i32) -> isize {
    unsafe { crate::linux::syscall3(SYS_SOCKET, domain as u64, ty as u64, protocol as u64) }
}

/// `sys_connect(42)`: connect a socket to a remote address.
pub fn sys_connect(sockfd: usize, addr: *const SockAddrIn) -> isize {
    unsafe { crate::linux::syscall3(SYS_CONNECT, sockfd as u64, addr as u64, SOCKADDR_IN_LEN) }
}

/// `sys_bind(49)`: bind a socket to a local address.
pub fn sys_bind(sockfd: usize, addr: *const SockAddrIn) -> isize {
    unsafe { crate::linux::syscall3(SYS_BIND, sockfd as u64, addr as u64, SOCKADDR_IN_LEN) }
}

/// `sys_listen(50)`: listen for incoming connections on a socket.
pub fn sys_listen(sockfd: usize, backlog: i32) -> isize {
    unsafe { crate::linux::syscall2(SYS_LISTEN, sockfd as u64, backlog as u64) }
}

/// `sys_sendto(44)`: send data to a (possibly unconnected) socket.
pub fn sys_sendto(
    sockfd: usize,
    buf: &[u8],
    flags: i32,
    addr: *const SockAddrIn,
) -> isize {
    unsafe {
        crate::linux::syscall6(
            SYS_SENDTO,
            sockfd as u64,
            buf.as_ptr() as u64,
            buf.len() as u64,
            flags as u64,
            addr as u64,
            SOCKADDR_IN_LEN,
        )
    }
}

/// `sys_recvfrom(45)`: receive data from a socket, optionally capturing the source address.
pub fn sys_recvfrom(
    sockfd: usize,
    buf: &mut [u8],
    flags: i32,
    addr: *mut SockAddrIn,
    addrlen: *mut u64,
) -> isize {
    unsafe {
        crate::linux::syscall6(
            SYS_RECVFROM,
            sockfd as u64,
            buf.as_mut_ptr() as u64,
            buf.len() as u64,
            flags as u64,
            addr as u64,
            addrlen as u64,
        )
    }
}

// ----------------------------------------------------------------------------
// Helpers
// ----------------------------------------------------------------------------

/// Format an IPv4 address `[u8; 4]` into a fixed buffer, returning the byte length written.
pub fn format_ipv4(ip: [u8; 4], buf: &mut [u8]) -> usize {
    let mut pos = 0;
    for (i, octet) in ip.iter().enumerate() {
        if i > 0 {
            if pos < buf.len() {
                buf[pos] = b'.';
                pos += 1;
            }
        }
        // Manual decimal conversion (no alloc).
        let mut tmp = [0u8; 3];
        let mut n = *octet;
        let mut len = 0;
        if n == 0 {
            tmp[0] = b'0';
            len = 1;
        } else {
            let mut rev = [0u8; 3];
            let mut r = 0;
            while n > 0 {
                rev[r] = b'0' + (n % 10) as u8;
                n /= 10;
                r += 1;
            }
            while r > 0 {
                r -= 1;
                tmp[len] = rev[r];
                len += 1;
            }
        }
        if pos + len <= buf.len() {
            buf[pos..pos + len].copy_from_slice(&tmp[..len]);
            pos += len;
        } else {
            break;
        }
    }
    pos
}

/// Parse a dotted-decimal IPv4 string (e.g. "192.168.1.100") into `[u8; 4]`.
/// Returns `None` on malformed input.
pub fn parse_ipv4(s: &str) -> Option<[u8; 4]> {
    let mut octets = [0u8; 4];
    let mut idx = 0;
    let mut acc: u32 = 0;
    let mut have_digit = false;
    for ch in s.bytes().chain(core::iter::once(b'.')) {
        match ch {
            b'0'..=b'9' => {
                acc = acc * 10 + (ch - b'0') as u32;
                if acc > 255 {
                    return None;
                }
                have_digit = true;
            }
            b'.' => {
                if !have_digit || idx >= 4 {
                    return None;
                }
                octets[idx] = acc as u8;
                idx += 1;
                acc = 0;
                have_digit = false;
            }
            _ => return None,
        }
    }
    if idx == 4 {
        Some(octets)
    } else {
        None
    }
}
