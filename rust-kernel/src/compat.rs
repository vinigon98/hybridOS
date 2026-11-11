//! Standard library compatibility shim for `#![no_std]` execution.
#![allow(unused_imports, dead_code)]

pub use core::*;

pub mod sync {
    pub use alloc::sync::*;
    pub use core::sync::atomic;

    use crate::sync::SpinLock;

    pub struct Mutex<T: ?Sized> {
        inner: SpinLock<T>,
    }

    impl<T> Mutex<T> {
        pub const fn new(val: T) -> Self {
            Self {
                inner: SpinLock::new(val),
            }
        }

        pub fn lock(&self) -> Result<crate::sync::SpinLockGuard<'_, T>, ()> {
            Ok(self.inner.lock())
        }

        pub fn try_lock(&self) -> Result<crate::sync::SpinLockGuard<'_, T>, ()> {
            Ok(self.inner.lock())
        }
    }

    impl<T: Default> Default for Mutex<T> {
        fn default() -> Self {
            Self::new(T::default())
        }
    }

    impl<T: core::fmt::Debug> core::fmt::Debug for Mutex<T> {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            write!(f, "Mutex")
        }
    }

    pub struct RwLock<T: ?Sized> {
        inner: SpinLock<T>,
    }

    impl<T> RwLock<T> {
        pub const fn new(val: T) -> Self {
            Self {
                inner: SpinLock::new(val),
            }
        }

        pub fn read(&self) -> Result<crate::sync::SpinLockGuard<'_, T>, ()> {
            Ok(self.inner.lock())
        }

        pub fn write(&self) -> Result<crate::sync::SpinLockGuard<'_, T>, ()> {
            Ok(self.inner.lock())
        }
    }

    impl<T: Default> Default for RwLock<T> {
        fn default() -> Self {
            Self::new(T::default())
        }
    }

    impl<T: core::fmt::Debug> core::fmt::Debug for RwLock<T> {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            write!(f, "RwLock")
        }
    }

    pub use crate::sync::OnceLock;
}

pub mod collections {
    pub use alloc::collections::{BTreeMap, BTreeSet, VecDeque};
    use alloc::collections::BTreeMap as Map;
    use alloc::collections::BTreeSet as Set;

    #[derive(Clone, PartialEq, Eq)]
    pub struct HashMap<K: Ord, V>(Map<K, V>);

    impl<K: Ord, V> HashMap<K, V> {
        pub const fn new() -> Self {
            Self(Map::new())
        }

        pub fn with_capacity(_cap: usize) -> Self {
            Self(Map::new())
        }

        pub fn insert(&mut self, k: K, v: V) -> Option<V> {
            self.0.insert(k, v)
        }

        pub fn get<Q>(&self, k: &Q) -> Option<&V>
        where
            K: core::borrow::Borrow<Q>,
            Q: Ord + ?Sized,
        {
            self.0.get(k)
        }

        pub fn get_mut<Q>(&mut self, k: &Q) -> Option<&mut V>
        where
            K: core::borrow::Borrow<Q>,
            Q: Ord + ?Sized,
        {
            self.0.get_mut(k)
        }

        pub fn contains_key<Q>(&self, k: &Q) -> bool
        where
            K: core::borrow::Borrow<Q>,
            Q: Ord + ?Sized,
        {
            self.0.contains_key(k)
        }

        pub fn remove<Q>(&mut self, k: &Q) -> Option<V>
        where
            K: core::borrow::Borrow<Q>,
            Q: Ord + ?Sized,
        {
            self.0.remove(k)
        }

        pub fn len(&self) -> usize {
            self.0.len()
        }

        pub fn is_empty(&self) -> bool {
            self.0.is_empty()
        }

        pub fn clear(&mut self) {
            self.0.clear()
        }

        pub fn iter(&self) -> alloc::collections::btree_map::Iter<'_, K, V> {
            self.0.iter()
        }

        pub fn iter_mut(&mut self) -> alloc::collections::btree_map::IterMut<'_, K, V> {
            self.0.iter_mut()
        }

        pub fn keys(&self) -> alloc::collections::btree_map::Keys<'_, K, V> {
            self.0.keys()
        }

        pub fn values(&self) -> alloc::collections::btree_map::Values<'_, K, V> {
            self.0.values()
        }

        pub fn values_mut(&mut self) -> alloc::collections::btree_map::ValuesMut<'_, K, V> {
            self.0.values_mut()
        }

        pub fn entry(&mut self, key: K) -> alloc::collections::btree_map::Entry<'_, K, V> {
            self.0.entry(key)
        }

        pub fn retain<F>(&mut self, f: F)
        where
            F: FnMut(&K, &mut V) -> bool,
        {
            self.0.retain(f)
        }
    }

    impl<K: Ord, V> Default for HashMap<K, V> {
        fn default() -> Self {
            Self::new()
        }
    }

    impl<K: Ord + core::fmt::Debug, V: core::fmt::Debug> core::fmt::Debug for HashMap<K, V> {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            self.0.fmt(f)
        }
    }

    impl<K: Ord, V> core::ops::Index<&K> for HashMap<K, V> {
        type Output = V;
        fn index(&self, key: &K) -> &V {
            &self.0[key]
        }
    }

    impl<K: Ord, V> core::ops::IndexMut<&K> for HashMap<K, V> {
        fn index_mut(&mut self, key: &K) -> &mut V {
            self.0.get_mut(key).expect("no entry found for key")
        }
    }

    impl<K: Ord, V> IntoIterator for HashMap<K, V> {
        type Item = (K, V);
        type IntoIter = alloc::collections::btree_map::IntoIter<K, V>;
        fn into_iter(self) -> Self::IntoIter {
            self.0.into_iter()
        }
    }

    impl<'a, K: Ord, V> IntoIterator for &'a HashMap<K, V> {
        type Item = (&'a K, &'a V);
        type IntoIter = alloc::collections::btree_map::Iter<'a, K, V>;
        fn into_iter(self) -> Self::IntoIter {
            self.0.iter()
        }
    }

    impl<'a, K: Ord, V> IntoIterator for &'a mut HashMap<K, V> {
        type Item = (&'a K, &'a mut V);
        type IntoIter = alloc::collections::btree_map::IterMut<'a, K, V>;
        fn into_iter(self) -> Self::IntoIter {
            self.0.iter_mut()
        }
    }

    impl<K: Ord, V> FromIterator<(K, V)> for HashMap<K, V> {
        fn from_iter<T: IntoIterator<Item = (K, V)>>(iter: T) -> Self {
            let mut map = Self::new();
            for (k, v) in iter {
                map.insert(k, v);
            }
            map
        }
    }

    #[derive(Clone, PartialEq, Eq)]
    pub struct HashSet<T: Ord>(Set<T>);

    impl<T: Ord> HashSet<T> {
        pub const fn new() -> Self {
            Self(Set::new())
        }

        pub fn with_capacity(_cap: usize) -> Self {
            Self(Set::new())
        }

        pub fn insert(&mut self, value: T) -> bool {
            self.0.insert(value)
        }

        pub fn contains<Q>(&self, value: &Q) -> bool
        where
            T: core::borrow::Borrow<Q>,
            Q: Ord + ?Sized,
        {
            self.0.contains(value)
        }

        pub fn remove<Q>(&mut self, value: &Q) -> bool
        where
            T: core::borrow::Borrow<Q>,
            Q: Ord + ?Sized,
        {
            self.0.remove(value)
        }

        pub fn len(&self) -> usize {
            self.0.len()
        }

        pub fn is_empty(&self) -> bool {
            self.0.is_empty()
        }

        pub fn clear(&mut self) {
            self.0.clear()
        }

        pub fn retain<F>(&mut self, f: F)
        where
            F: FnMut(&T) -> bool,
        {
            self.0.retain(f);
        }

        pub fn iter(&self) -> alloc::collections::btree_set::Iter<'_, T> {
            self.0.iter()
        }
    }

    impl<T: Ord> Default for HashSet<T> {
        fn default() -> Self {
            Self::new()
        }
    }

    impl<T: Ord + core::fmt::Debug> core::fmt::Debug for HashSet<T> {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            self.0.fmt(f)
        }
    }

    impl<T: Ord> IntoIterator for HashSet<T> {
        type Item = T;
        type IntoIter = alloc::collections::btree_set::IntoIter<T>;
        fn into_iter(self) -> Self::IntoIter {
            self.0.into_iter()
        }
    }

    impl<'a, T: Ord> IntoIterator for &'a HashSet<T> {
        type Item = &'a T;
        type IntoIter = alloc::collections::btree_set::Iter<'a, T>;
        fn into_iter(self) -> Self::IntoIter {
            self.0.iter()
        }
    }

    impl<T: Ord> FromIterator<T> for HashSet<T> {
        fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
            let mut set = Self::new();
            for item in iter {
                set.insert(item);
            }
            set
        }
    }
}

pub mod io {
    pub use core::fmt::Result as FmtResult;
    pub type Result<T> = core::result::Result<T, Error>;

    #[derive(Debug)]
    pub struct Error {
        kind: ErrorKind,
        msg: Option<alloc::string::String>,
    }

    impl Error {
        pub fn new(kind: ErrorKind, msg: impl Into<alloc::string::String>) -> Self {
            Self {
                kind,
                msg: Some(msg.into()),
            }
        }

        pub fn from(kind: ErrorKind) -> Self {
            Self { kind, msg: None }
        }

        pub fn kind(&self) -> ErrorKind {
            self.kind
        }
    }

    impl core::fmt::Display for Error {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            if let Some(ref msg) = self.msg {
                write!(f, "{:?}: {}", self.kind, msg)
            } else {
                write!(f, "{:?}", self.kind)
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ErrorKind {
        NotFound,
        PermissionDenied,
        ConnectionRefused,
        ConnectionReset,
        ConnectionAborted,
        NotConnected,
        AddrInUse,
        AddrNotAvailable,
        BrokenPipe,
        AlreadyExists,
        WouldBlock,
        InvalidInput,
        InvalidData,
        TimedOut,
        WriteZero,
        Interrupted,
        UnexpectedEof,
        Other,
    }

    pub trait Read {
        fn read(&mut self, buf: &mut [u8]) -> Result<usize>;
        fn read_to_string(&mut self, _buf: &mut alloc::string::String) -> Result<usize> {
            Ok(0)
        }
    }

    pub trait Write {
        fn write(&mut self, buf: &[u8]) -> Result<usize>;
        fn flush(&mut self) -> Result<()>;
        fn write_all(&mut self, mut buf: &[u8]) -> Result<()> {
            while !buf.is_empty() {
                match self.write(buf) {
                    Ok(0) => return Err(Error::from(ErrorKind::WriteZero)),
                    Ok(n) => buf = &buf[n..],
                    Err(e) => return Err(e),
                }
            }
            Ok(())
        }
        fn write_fmt(&mut self, fmt: core::fmt::Arguments<'_>) -> Result<()> {
            struct Adapter<'a, T: ?Sized>(&'a mut T);
            impl<'a, T: ?Sized + Write> core::fmt::Write for Adapter<'a, T> {
                fn write_str(&mut self, s: &str) -> core::fmt::Result {
                    self.0.write_all(s.as_bytes()).map_err(|_| core::fmt::Error)
                }
            }
            core::fmt::write(&mut Adapter(self), fmt).map_err(|_| Error::from(ErrorKind::Other))
        }
    }

    pub trait BufRead: Read {
        fn fill_buf(&mut self) -> Result<&[u8]>;
        fn consume(&mut self, amt: usize);
        fn read_line(&mut self, buf: &mut alloc::string::String) -> Result<usize> {
            let mut count = 0;
            let mut byte = [0u8; 1];
            loop {
                match self.read(&mut byte) {
                    Ok(0) => break,
                    Ok(1) => {
                        count += 1;
                        buf.push(byte[0] as char);
                        if byte[0] == b'\n' {
                            break;
                        }
                    }
                    Ok(_) => unreachable!(),
                    Err(e) => return Err(e),
                }
            }
            Ok(count)
        }
    }
}

pub mod path {
    use alloc::string::String;

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct PathBuf(String);

    impl PathBuf {
        pub fn new() -> Self {
            Self(String::new())
        }
        pub fn from(s: impl Into<String>) -> Self {
            Self(s.into())
        }
        pub fn as_path(&self) -> &Path {
            Path::new(&self.0)
        }
        pub fn to_str(&self) -> Option<&str> {
            Some(&self.0)
        }
    }

    impl core::ops::Deref for PathBuf {
        type Target = Path;
        fn deref(&self) -> &Path {
            Path::new(&self.0)
        }
    }

    #[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct Path(str);

    impl Path {
        pub fn new(s: &str) -> &Self {
            unsafe { &*(s as *const str as *const Path) }
        }
        pub fn to_str(&self) -> Option<&str> {
            Some(&self.0)
        }
        pub fn to_string_lossy(&self) -> alloc::string::String {
            alloc::string::String::from(&self.0)
        }
        pub fn file_name(&self) -> Option<&str> {
            let s = &self.0;
            s.rsplit(|c| c == '/' || c == '\\').next()
        }
    }

    impl From<&str> for PathBuf {
        fn from(s: &str) -> Self {
            Self(alloc::string::String::from(s))
        }
    }

    impl From<alloc::string::String> for PathBuf {
        fn from(s: alloc::string::String) -> Self {
            Self(s)
        }
    }

    impl core::convert::AsRef<Path> for Path {
        fn as_ref(&self) -> &Path {
            self
        }
    }

    impl core::convert::AsRef<Path> for PathBuf {
        fn as_ref(&self) -> &Path {
            self.as_path()
        }
    }

    impl core::convert::AsRef<Path> for str {
        fn as_ref(&self) -> &Path {
            Path::new(self)
        }
    }

    impl core::convert::AsRef<Path> for alloc::string::String {
        fn as_ref(&self) -> &Path {
            Path::new(self)
        }
    }
}

pub mod ffi {
    use alloc::string::String;
    use alloc::vec::Vec;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct CString(Vec<u8>);

    impl CString {
        pub fn new(s: impl AsRef<str>) -> Result<Self, ()> {
            let bytes = s.as_ref().as_bytes();
            let mut v = Vec::with_capacity(bytes.len() + 1);
            v.extend_from_slice(bytes);
            v.push(0);
            Ok(Self(v))
        }

        pub fn as_ptr(&self) -> *const u8 {
            self.0.as_ptr()
        }

        pub fn as_c_str(&self) -> &CStr {
            CStr::from_bytes_with_nul(&self.0).unwrap()
        }
    }

    pub struct CStr([u8]);

    impl CStr {
        pub fn from_bytes_with_nul(b: &[u8]) -> Result<&Self, ()> {
            Ok(unsafe { &*(b as *const [u8] as *const CStr) })
        }

        pub unsafe fn from_ptr<'a>(ptr: *const i8) -> &'a Self {
            let uptr = ptr as *const u8;
            let mut len = 0;
            while *uptr.add(len) != 0 {
                len += 1;
            }
            let slice = core::slice::from_raw_parts(uptr, len + 1);
            &*(slice as *const [u8] as *const CStr)
        }

        pub fn to_str(&self) -> Result<&str, core::str::Utf8Error> {
            let len = self.0.len().saturating_sub(1);
            core::str::from_utf8(&self.0[..len])
        }

        pub fn to_string_lossy(&self) -> alloc::string::String {
            alloc::string::String::from(self.to_str().unwrap_or(""))
        }

        pub fn to_bytes(&self) -> &[u8] {
            let len = self.0.len().saturating_sub(1);
            &self.0[..len]
        }
    }
}

pub mod thread {
    pub fn yield_now() {
        core::hint::spin_loop();
    }
    pub fn sleep(dur: core::time::Duration) {
        let iters = dur.as_micros() * 100;
        for _ in 0..iters {
            core::hint::spin_loop();
        }
    }
}

pub mod time {
    pub use core::time::*;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    pub struct SystemTime(pub core::time::Duration);
    pub const UNIX_EPOCH: SystemTime = SystemTime(core::time::Duration::from_secs(0));

    impl SystemTime {
        pub fn now() -> Self {
            UNIX_EPOCH
        }
        pub fn duration_since(&self, _: SystemTime) -> Result<core::time::Duration, ()> {
            Ok(core::time::Duration::from_secs(0))
        }
    }
}

pub mod fs {
    pub struct File;
    pub struct OpenOptions;

    impl OpenOptions {
        pub fn new() -> Self {
            Self
        }
        pub fn read(&mut self, _: bool) -> &mut Self {
            self
        }
        pub fn write(&mut self, _: bool) -> &mut Self {
            self
        }
        pub fn create(&mut self, _: bool) -> &mut Self {
            self
        }
        pub fn truncate(&mut self, _: bool) -> &mut Self {
            self
        }
        pub fn open(&self, _: impl AsRef<super::path::Path>) -> Result<File, super::io::Error> {
            Ok(File)
        }
    }

    impl File {
        pub fn open(_: impl AsRef<super::path::Path>) -> Result<File, super::io::Error> {
            Ok(File)
        }
        pub fn create(_: impl AsRef<super::path::Path>) -> Result<File, super::io::Error> {
            Ok(File)
        }
        pub fn read_to_string(&mut self, _buf: &mut alloc::string::String) -> super::io::Result<usize> {
            Ok(0)
        }
    }

    impl super::io::Read for File {
        fn read(&mut self, _buf: &mut [u8]) -> super::io::Result<usize> {
            Ok(0)
        }
    }

    impl super::io::Write for File {
        fn write(&mut self, buf: &[u8]) -> super::io::Result<usize> {
            Ok(buf.len())
        }
        fn flush(&mut self) -> super::io::Result<()> {
            Ok(())
        }
    }
}

pub mod net {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct Ipv4Addr(pub [u8; 4]);

    impl Ipv4Addr {
        pub const fn new(a: u8, b: u8, c: u8, d: u8) -> Self {
            Self([a, b, c, d])
        }
        pub const fn octets(&self) -> [u8; 4] {
            self.0
        }
        pub const fn is_loopback(&self) -> bool {
            self.0[0] == 127
        }
        pub const fn is_unspecified(&self) -> bool {
            self.0[0] == 0 && self.0[1] == 0 && self.0[2] == 0 && self.0[3] == 0
        }
    }

    impl core::fmt::Display for Ipv4Addr {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            write!(f, "{}.{}.{}.{}", self.0[0], self.0[1], self.0[2], self.0[3])
        }
    }

    impl From<u32> for Ipv4Addr {
        fn from(val: u32) -> Self {
            Self(val.to_be_bytes())
        }
    }

    impl From<Ipv4Addr> for u32 {
        fn from(ip: Ipv4Addr) -> Self {
            u32::from_be_bytes(ip.0)
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct Ipv6Addr(pub [u8; 16]);

    impl Ipv6Addr {
        pub const fn new(_a: u16, _b: u16, _c: u16, _d: u16, _e: u16, _f: u16, _g: u16, _h: u16) -> Self {
            Self([0; 16])
        }
    }
}

pub mod vec {
    pub use alloc::vec::*;
}

pub mod string {
    pub use alloc::string::*;
}

pub mod boxed {
    pub use alloc::boxed::*;
}

pub mod error {
    pub use core::error::*;
}

pub trait FloatExt: Copy + Sized {
    fn round(self) -> Self;
    fn floor(self) -> Self;
    fn ceil(self) -> Self;
    fn trunc(self) -> Self;
    fn fract(self) -> Self;
    fn abs(self) -> Self;
    fn signum(self) -> Self;
    fn powf(self, n: Self) -> Self;
    fn powi(self, n: i32) -> Self;
    fn sqrt(self) -> Self;
    fn hypot(self, other: Self) -> Self;
    fn sin(self) -> Self;
    fn cos(self) -> Self;
    fn tan(self) -> Self;
    fn atan2(self, other: Self) -> Self;
    fn exp(self) -> Self;
    fn ln(self) -> Self;
    fn log10(self) -> Self;
    fn log2(self) -> Self;
    fn sinh(self) -> Self;
    fn cosh(self) -> Self;
}

impl FloatExt for f32 {
    fn round(self) -> Self {
        if self >= 0.0 {
            self.floor() + if self.fract() >= 0.5 { 1.0 } else { 0.0 }
        } else {
            self.ceil() - if (-self).fract() >= 0.5 { 1.0 } else { 0.0 }
        }
    }

    fn floor(self) -> Self {
        let i = self as i64;
        if self < (i as f32) {
            (i - 1) as f32
        } else {
            i as f32
        }
    }

    fn ceil(self) -> Self {
        let i = self as i64;
        if self > (i as f32) {
            (i + 1) as f32
        } else {
            i as f32
        }
    }

    fn trunc(self) -> Self {
        (self as i64) as f32
    }

    fn fract(self) -> Self {
        self - self.trunc()
    }

    fn abs(self) -> Self {
        if self < 0.0 { -self } else { self }
    }

    fn signum(self) -> Self {
        if self > 0.0 { 1.0 } else if self < 0.0 { -1.0 } else { 0.0 }
    }

    fn sqrt(self) -> Self {
        (self as f64).sqrt() as f32
    }

    fn sinh(self) -> Self {
        (self as f64).sinh() as f32
    }

    fn cosh(self) -> Self {
        (self as f64).cosh() as f32
    }

    fn hypot(self, other: Self) -> Self {
        (self * self + other * other).sqrt()
    }

    fn powi(self, mut n: i32) -> Self {
        let mut base = self;
        if n < 0 {
            base = 1.0 / base;
            n = -n;
        }
        let mut acc = 1.0;
        let mut exp = n as u32;
        while exp > 0 {
            if exp % 2 == 1 {
                acc *= base;
            }
            base *= base;
            exp /= 2;
        }
        acc
    }

    fn sin(self) -> Self {
        (self as f64).sin() as f32
    }

    fn cos(self) -> Self {
        (self as f64).cos() as f32
    }

    fn tan(self) -> Self {
        (self as f64).tan() as f32
    }

    fn atan2(self, other: Self) -> Self {
        (self as f64).atan2(other as f64) as f32
    }

    fn exp(self) -> Self {
        (self as f64).exp() as f32
    }

    fn ln(self) -> Self {
        (self as f64).ln() as f32
    }

    fn log10(self) -> Self {
        (self as f64).log10() as f32
    }

    fn log2(self) -> Self {
        (self as f64).log2() as f32
    }

    fn powf(self, n: Self) -> Self {
        (self as f64).powf(n as f64) as f32
    }
}

impl FloatExt for f64 {
    fn round(self) -> Self {
        if self >= 0.0 {
            self.floor() + if self.fract() >= 0.5 { 1.0 } else { 0.0 }
        } else {
            self.ceil() - if (-self).fract() >= 0.5 { 1.0 } else { 0.0 }
        }
    }

    fn floor(self) -> Self {
        let i = self as i64;
        if self < (i as f64) {
            (i - 1) as f64
        } else {
            i as f64
        }
    }

    fn ceil(self) -> Self {
        let i = self as i64;
        if self > (i as f64) {
            (i + 1) as f64
        } else {
            i as f64
        }
    }

    fn trunc(self) -> Self {
        (self as i64) as f64
    }

    fn fract(self) -> Self {
        self - self.trunc()
    }

    fn abs(self) -> Self {
        if self < 0.0 { -self } else { self }
    }

    fn signum(self) -> Self {
        if self > 0.0 { 1.0 } else if self < 0.0 { -1.0 } else { 0.0 }
    }

    fn sqrt(self) -> Self {
        if self <= 0.0 {
            return 0.0;
        }
        let mut guess = self;
        if guess < 1.0 {
            guess = 1.0;
        }
        for _ in 0..12 {
            guess = 0.5 * (guess + self / guess);
        }
        guess
    }

    fn hypot(self, other: Self) -> Self {
        (self * self + other * other).sqrt()
    }

    fn powi(self, mut n: i32) -> Self {
        let mut base = self;
        if n < 0 {
            base = 1.0 / base;
            n = -n;
        }
        let mut acc = 1.0;
        let mut exp = n as u32;
        while exp > 0 {
            if exp % 2 == 1 {
                acc *= base;
            }
            base *= base;
            exp /= 2;
        }
        acc
    }

    fn sin(self) -> Self {
        const PI: f64 = 3.14159265358979323846;
        const TWO_PI: f64 = 6.28318530717958647692;
        let mut x = self % TWO_PI;
        if x > PI {
            x -= TWO_PI;
        } else if x < -PI {
            x += TWO_PI;
        }
        let x2 = x * x;
        let x3 = x2 * x;
        let x5 = x3 * x2;
        let x7 = x5 * x2;
        let x9 = x7 * x2;
        let x11 = x9 * x2;
        x - (x3 / 6.0) + (x5 / 120.0) - (x7 / 5040.0) + (x9 / 362880.0) - (x11 / 39916800.0)
    }

    fn cos(self) -> Self {
        const PI_2: f64 = 1.57079632679489661923;
        (self + PI_2).sin()
    }

    fn tan(self) -> Self {
        let c = self.cos();
        if c.abs() < 1e-12 {
            0.0
        } else {
            self.sin() / c
        }
    }

    fn atan2(self, other: Self) -> Self {
        const PI: f64 = 3.14159265358979323846;
        let y = self;
        let x = other;
        if x == 0.0 {
            if y > 0.0 {
                PI / 2.0
            } else if y < 0.0 {
                -PI / 2.0
            } else {
                0.0
            }
        } else {
            let r = y / x;
            let atan_approx = r / (1.0 + 0.28086 * r * r);
            if x > 0.0 {
                atan_approx
            } else if y >= 0.0 {
                atan_approx + PI
            } else {
                atan_approx - PI
            }
        }
    }

    fn exp(self) -> Self {
        if self < -50.0 {
            return 0.0;
        }
        if self > 50.0 {
            return 1e21;
        }
        let x = self / 16.0;
        let mut term = 1.0;
        let mut sum = 1.0;
        for i in 1..20 {
            term *= x / (i as f64);
            sum += term;
        }
        sum = sum * sum;
        sum = sum * sum;
        sum = sum * sum;
        sum = sum * sum;
        sum
    }

    fn ln(self) -> Self {
        if self <= 0.0 {
            return 0.0;
        }
        let mut val = self;
        let mut exp = 0;
        const LN_2: f64 = 0.693147180559945309417;
        while val >= 2.0 {
            val /= 2.0;
            exp += 1;
        }
        while val < 0.5 {
            val *= 2.0;
            exp -= 1;
        }
        let y = (val - 1.0) / (val + 1.0);
        let y2 = y * y;
        let mut term = y;
        let mut sum = y;
        for i in 1..10 {
            term *= y2;
            sum += term / ((2 * i + 1) as f64);
        }
        2.0 * sum + (exp as f64) * LN_2
    }

    fn log10(self) -> Self {
        self.ln() * 0.43429448190325182765
    }

    fn log2(self) -> Self {
        self.ln() * 1.44269504088896340736
    }

    fn powf(self, n: Self) -> Self {
        if n == 0.0 {
            return 1.0;
        }
        if self == 0.0 {
            return 0.0;
        }
        if self < 0.0 {
            let int_n = n as i32;
            if (int_n as f64) == n {
                return self.powi(int_n);
            }
            return 0.0;
        }
        (n * self.ln()).exp()
    }

    fn sinh(self) -> Self {
        (self.exp() - (-self).exp()) / 2.0
    }

    fn cosh(self) -> Self {
        (self.exp() + (-self).exp()) / 2.0
    }
}

pub trait StrLossyExt {
    fn to_string_lossy(&self) -> &str;
}

impl StrLossyExt for str {
    fn to_string_lossy(&self) -> &str {
        self
    }
}
