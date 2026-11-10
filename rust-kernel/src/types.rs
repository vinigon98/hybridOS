//! Fundamental kernel types, NTSTATUS codes, and POSIX errno mappings.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct NtStatus(pub i32);

impl NtStatus {
    pub const SUCCESS: Self = NtStatus(0x00000000);
    pub const UNSUCCESSFUL: Self = NtStatus(-1073741823); // 0xC0000001
    pub const NOT_IMPLEMENTED: Self = NtStatus(-1073741822); // 0xC0000002
    pub const INVALID_HANDLE: Self = NtStatus(-1073741816); // 0xC0000008
    pub const INVALID_PARAMETER: Self = NtStatus(-1073741811); // 0xC000000D
    pub const NO_MEMORY: Self = NtStatus(-1073741801); // 0xC0000017
    pub const ACCESS_DENIED: Self = NtStatus(-1073741790); // 0xC0000022
    pub const FILE_NOT_FOUND: Self = NtStatus(-1073741772); // 0xC0000034
    pub const TIMEOUT: Self = NtStatus(0x00000102);
    pub const PENDING: Self = NtStatus(0x00000103);
    pub const BUFFER_TOO_SMALL: Self = NtStatus(-1073741789); // 0xC0000023
    pub const END_OF_FILE: Self = NtStatus(-1073741807); // 0xC0000011
    pub const INFO_LENGTH_MISMATCH: Self = NtStatus(-1073741820); // 0xC0000004
    pub const SUSPEND_COUNT_EXCEEDED: Self = NtStatus(-1073741750); // 0xC000004A
    pub const THREAD_IS_TERMINATING: Self = NtStatus(-1073741749); // 0xC000004B
    pub const NOT_SUPPORTED: Self = NtStatus(-1073741637); // 0xC00000BB

    pub fn is_success(&self) -> bool {
        self.0 >= 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum PosixError {
    EPERM = 1,
    ENOENT = 2,
    ESRCH = 3,
    EINTR = 4,
    EIO = 5,
    ENXIO = 6,
    EBADF = 9,
    ECHILD = 10,
    EAGAIN = 11,
    ENOMEM = 12,
    EACCES = 13,
    EFAULT = 14,
    EEXIST = 17,
    ENODEV = 19,
    ENOTDIR = 20,
    EISDIR = 21,
    EINVAL = 22,
    EMFILE = 24,
    ENOTTY = 25,
    EFBIG = 27,
    ENOSPC = 28,
    ESPIPE = 29,
    EROFS = 30,
    EMLINK = 31,
    EPIPE = 32,
    ERANGE = 34,
    ENAMETOOLONG = 36,
    ENOSYS = 38,
    ENOTSOCK = 88,
    EDESTADDRREQ = 89,
    EMSGSIZE = 90,
    EPROTOTYPE = 91,
    ENOPROTOOPT = 92,
    EPROTONOSUPPORT = 93,
    ESOCKTNOSUPPORT = 94,
    EOPNOTSUPP = 95,
    EPFNOSUPPORT = 96,
    EAFNOSUPPORT = 97,
    EADDRINUSE = 98,
    EADDRNOTAVAIL = 99,
    ENETDOWN = 100,
    ENETUNREACH = 101,
    ENETRESET = 102,
    ECONNABORTED = 103,
    ECONNRESET = 104,
    ENOBUFS = 105,
    EISCONN = 106,
    ENOTCONN = 107,
    ESHUTDOWN = 108,
    ETOOMANYREFS = 109,
    ETIMEDOUT = 110,
    ECONNREFUSED = 111,
    EHOSTDOWN = 112,
    EHOSTUNREACH = 113,
    EALREADY = 114,
    EINPROGRESS = 115,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct Handle(pub usize);

impl Handle {
    pub const INVALID: Self = Handle(usize::MAX);
    pub const STDIN: Self = Handle(0);
    pub const STDOUT: Self = Handle(1);
    pub const STDERR: Self = Handle(2);
}
