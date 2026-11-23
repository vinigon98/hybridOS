pub const AF_INET: i32 = 2; pub const SOCK_STREAM: i32 = 1;
pub const SYS_SOCKET: u64 = 41; pub const SYS_CONNECT: u64 = 42; pub const SYS_SENDTO: u64 = 44;
pub const SYS_RECVFROM: u64 = 45; pub const SYS_BIND: u64 = 49; pub const SYS_LISTEN: u64 = 50;
#[repr(C)] #[derive(Debug, Clone, Copy, PartialEq, Eq)] pub struct SockAddrIn { pub sin_family: u16, pub sin_port: u16, pub sin_addr: u32, pub sin_zero: [u8; 8], }
impl SockAddrIn { pub fn new(ip: [u8; 4], port: u16) -> Self { Self { sin_family: AF_INET as u16, sin_port: port.to_be(), sin_addr: u32::from_be_bytes(ip), sin_zero: [0; 8] } } pub fn ip_bytes(&self) -> [u8; 4] { self.sin_addr.to_be_bytes() } pub fn port(&self) -> u16 { u16::from_be(self.sin_port) } }
pub const SOCKADDR_IN_LEN: u64 = core::mem::size_of::<SockAddrIn>() as u64;
pub fn sys_socket(domain: i32, ty: i32, protocol: i32) -> isize { unsafe { crate::linux::syscall3(SYS_SOCKET, domain as u64, ty as u64, protocol as u64) } }
pub fn sys_connect(sockfd: usize, addr: *const SockAddrIn) -> isize { unsafe { crate::linux::syscall3(SYS_CONNECT, sockfd as u64, addr as u64, SOCKADDR_IN_LEN) } }
pub fn sys_bind(sockfd: usize, addr: *const SockAddrIn) -> isize { unsafe { crate::linux::syscall3(SYS_BIND, sockfd as u64, addr as u64, SOCKADDR_IN_LEN) } }
pub fn sys_listen(sockfd: usize, backlog: i32) -> isize { unsafe { crate::linux::syscall2(SYS_LISTEN, sockfd as u64, backlog as u64) } }
pub fn sys_sendto(sockfd: usize, buf: &[u8], flags: i32, addr: *const SockAddrIn) -> isize { unsafe { crate::linux::syscall6(SYS_SENDTO, sockfd as u64, buf.as_ptr() as u64, buf.len() as u64, flags as u64, addr as u64, SOCKADDR_IN_LEN) } }
pub fn sys_recvfrom(sockfd: usize, buf: &mut [u8], flags: i32, addr: *mut SockAddrIn, addrlen: *mut u64) -> isize { unsafe { crate::linux::syscall6(SYS_RECVFROM, sockfd as u64, buf.as_mut_ptr() as u64, buf.len() as u64, flags as u64, addr as u64, addrlen as u64) } }
