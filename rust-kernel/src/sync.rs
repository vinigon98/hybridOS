//! Kernel Synchronization Primitives for Bare-Metal (#![no_std]).
//!
//! Provides interrupt-safe spinlocks and initialization guards without standard library dependencies.

#[allow(unused_imports)]
use alloc::string::{String, ToString};
#[allow(unused_imports)]
use alloc::vec::Vec;
#[allow(unused_imports)]
use alloc::boxed::Box;
#[allow(unused_imports)]
use crate::compat::FloatExt;
use core::cell::UnsafeCell;
use core::fmt;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

/// A spinlock providing mutual exclusion in Ring 0 bare metal.
pub struct SpinLock<T: ?Sized> {
    locked: AtomicBool,
    data: UnsafeCell<T>,
}

impl<T: Default> Default for SpinLock<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

unsafe impl<T: ?Sized + Send> Sync for SpinLock<T> {}
unsafe impl<T: ?Sized + Send> Send for SpinLock<T> {}

impl<T> SpinLock<T> {
    /// Creates a new unlocked spinlock.
    pub const fn new(data: T) -> Self {
        Self {
            locked: AtomicBool::new(false),
            data: UnsafeCell::new(data),
        }
    }

    /// Acquires the lock, spinning until available.
    pub fn lock(&self) -> SpinLockGuard<'_, T> {
        while self.locked.swap(true, Ordering::Acquire) {
            core::hint::spin_loop();
        }
        SpinLockGuard { lock: self }
    }

    /// Attempts to acquire the lock without spinning.
    pub fn try_lock(&self) -> Option<SpinLockGuard<'_, T>> {
        if !self.locked.swap(true, Ordering::Acquire) {
            Some(SpinLockGuard { lock: self })
        } else {
            None
        }
    }

    /// Returns a mutable reference to the underlying data (requires &mut self).
    pub fn get_mut(&mut self) -> &mut T {
        self.data.get_mut()
    }
}

/// RAII guard returned by `SpinLock::lock`. Releases the lock when dropped.
pub struct SpinLockGuard<'a, T: ?Sized> {
    lock: &'a SpinLock<T>,
}

impl<T: ?Sized> Deref for SpinLockGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.lock.data.get() }
    }
}

impl<T: ?Sized> DerefMut for SpinLockGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<T: ?Sized> Drop for SpinLockGuard<'_, T> {
    fn drop(&mut self) {
        self.lock.locked.store(false, Ordering::Release);
    }
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for SpinLockGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

/// A read-write spinlock for read-heavy kernel data structures.
pub struct RwLock<T: ?Sized> {
    state: AtomicU32, // 0 = unlocked, 1..=0x7FFF_FFFF = reader count, 0x8000_0000 = writer locked
    data: UnsafeCell<T>,
}

unsafe impl<T: ?Sized + Send> Sync for RwLock<T> {}
unsafe impl<T: ?Sized + Send> Send for RwLock<T> {}

const WRITER_BIT: u32 = 0x8000_0000;

pub use alloc::sync::{Arc, Weak};
pub use core::sync::atomic;

pub struct PoisonError<T>(pub T);

impl<T> PoisonError<T> {
    pub fn new(guard: T) -> Self {
        Self(guard)
    }
    pub fn into_inner(self) -> T {
        self.0
    }
    pub fn get_ref(&self) -> &T {
        &self.0
    }
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

impl<T> fmt::Debug for PoisonError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PoisonError")
    }
}

impl<T> fmt::Display for PoisonError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PoisonError")
    }
}

pub type LockResult<Guard> = Result<Guard, PoisonError<Guard>>;

/// Mutex compatible with std::sync::Mutex returning Result for unwrap()
#[derive(Default)]
pub struct Mutex<T: ?Sized> {
    inner: SpinLock<T>,
}

impl<T> Mutex<T> {
    pub const fn new(val: T) -> Self {
        Self {
            inner: SpinLock::new(val),
        }
    }

    pub fn lock(&self) -> Result<SpinLockGuard<'_, T>, PoisonError<SpinLockGuard<'_, T>>> {
        Ok(self.inner.lock())
    }

    pub fn try_lock(&self) -> Result<SpinLockGuard<'_, T>, PoisonError<SpinLockGuard<'_, T>>> {
        Ok(self.inner.lock())
    }
}

impl<T: core::fmt::Debug> core::fmt::Debug for Mutex<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Mutex")
    }
}

pub type MutexGuard<'a, T> = SpinLockGuard<'a, T>;

impl<T: ?Sized> fmt::Debug for RwLock<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RwLock")
    }
}

impl<T> RwLock<T> {
    pub const fn new(data: T) -> Self {
        Self {
            state: AtomicU32::new(0),
            data: UnsafeCell::new(data),
        }
    }

    pub fn read(&self) -> Result<RwLockReadGuard<'_, T>, PoisonError<RwLockReadGuard<'_, T>>> {
        loop {
            let current = self.state.load(Ordering::Relaxed);
            if current & WRITER_BIT != 0 {
                core::hint::spin_loop();
                continue;
            }
            if self.state.compare_exchange_weak(
                current,
                current + 1,
                Ordering::Acquire,
                Ordering::Relaxed,
            ).is_ok() {
                return Ok(RwLockReadGuard { lock: self });
            }
        }
    }

    pub fn write(&self) -> Result<RwLockWriteGuard<'_, T>, PoisonError<RwLockWriteGuard<'_, T>>> {
        loop {
            if self.state.compare_exchange_weak(
                0,
                WRITER_BIT,
                Ordering::Acquire,
                Ordering::Relaxed,
            ).is_ok() {
                return Ok(RwLockWriteGuard { lock: self });
            }
            core::hint::spin_loop();
        }
    }
}

pub struct RwLockReadGuard<'a, T: ?Sized> {
    lock: &'a RwLock<T>,
}

impl<T: ?Sized> Deref for RwLockReadGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.lock.data.get() }
    }
}

impl<T: ?Sized> Drop for RwLockReadGuard<'_, T> {
    fn drop(&mut self) {
        self.lock.state.fetch_sub(1, Ordering::Release);
    }
}

pub struct RwLockWriteGuard<'a, T: ?Sized> {
    lock: &'a RwLock<T>,
}

impl<T: ?Sized> Deref for RwLockWriteGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.lock.data.get() }
    }
}

impl<T: ?Sized> DerefMut for RwLockWriteGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<T: ?Sized> Drop for RwLockWriteGuard<'_, T> {
    fn drop(&mut self) {
        self.lock.state.store(0, Ordering::Release);
    }
}

/// A cell which can only be written once (useful for kernel subsystem initialization).
pub struct OnceLock<T> {
    initialized: AtomicBool,
    data: UnsafeCell<Option<T>>,
}

unsafe impl<T: Send + Sync> Sync for OnceLock<T> {}
unsafe impl<T: Send> Send for OnceLock<T> {}

impl<T> OnceLock<T> {
    pub const fn new() -> Self {
        Self {
            initialized: AtomicBool::new(false),
            data: UnsafeCell::new(None),
        }
    }

    pub fn get(&self) -> Option<&T> {
        if self.initialized.load(Ordering::Acquire) {
            unsafe { (*self.data.get()).as_ref() }
        } else {
            None
        }
    }

    pub fn set(&self, value: T) -> Result<(), T> {
        if self.initialized.load(Ordering::Acquire) {
            return Err(value);
        }
        unsafe {
            *self.data.get() = Some(value);
        }
        self.initialized.store(true, Ordering::Release);
        Ok(())
    }

    pub fn get_or_init<F>(&self, f: F) -> &T
    where
        F: FnOnce() -> T,
    {
        if let Some(val) = self.get() {
            return val;
        }
        let val = f();
        let _ = self.set(val);
        self.get().unwrap()
    }
}
