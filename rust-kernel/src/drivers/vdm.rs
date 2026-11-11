//! Vortex Driver Model (VDM) Subsystem in Rust.
//! Unifies Windows WDM IRP-based drivers and Linux file_operations drivers.

#[allow(unused_imports)]
use alloc::string::{String, ToString};
#[allow(unused_imports)]
use alloc::vec::Vec;
#[allow(unused_imports)]
use alloc::boxed::Box;
#[allow(unused_imports)]
use crate::compat::FloatExt;
use std::collections::HashMap;
use std::sync::Arc;
use crate::types::{NtStatus, PosixError};

#[derive(Debug, Clone)]
pub struct Irp {
    pub major_function: u8,
    pub user_buffer: Vec<u8>,
    pub buffer_length: usize,
    pub file_offset: u64,
    pub ioctl_code: u32,
    pub status: NtStatus,
    pub bytes_transferred: usize,
}

impl Irp {
    pub const IRP_MJ_CREATE: u8 = 0x00;
    pub const IRP_MJ_CLOSE: u8  = 0x02;
    pub const IRP_MJ_READ: u8   = 0x03;
    pub const IRP_MJ_WRITE: u8  = 0x04;
    pub const IRP_MJ_DEVICE_CONTROL: u8 = 0x0E;

    pub fn new_read(len: usize, offset: u64) -> Self {
        Self {
            major_function: Self::IRP_MJ_READ,
            user_buffer: vec![0u8; len],
            buffer_length: len,
            file_offset: offset,
            ioctl_code: 0,
            status: NtStatus::SUCCESS,
            bytes_transferred: 0,
        }
    }
}

pub trait WdmDriver: Send + Sync {
    fn name(&self) -> &str;
    fn dispatch(&self, irp: &mut Irp) -> NtStatus;
}

pub trait LinuxDriver: Send + Sync {
    fn name(&self) -> &str;
    fn read(&self, buf: &mut [u8], offset: u64) -> Result<usize, PosixError>;
    fn write(&self, buf: &[u8], offset: u64) -> Result<usize, PosixError>;
    fn ioctl(&self, cmd: u32, arg: usize) -> Result<i64, PosixError>;
}

pub enum UnifiedDriver {
    Windows(Arc<dyn WdmDriver>),
    Linux(Arc<dyn LinuxDriver>),
}

pub struct VortexDriverManager {
    drivers: HashMap<String, UnifiedDriver>,
}

impl VortexDriverManager {
    pub fn new() -> Self {
        Self {
            drivers: HashMap::new(),
        }
    }

    pub fn register_windows_driver(&mut self, drv: Arc<dyn WdmDriver>) {
        let name = drv.name().to_string();
        println!("[VDM - Rust] Registered Windows WDM Driver: '{}' (Bridge: Accessible via /dev/{})", name, name);
        self.drivers.insert(name, UnifiedDriver::Windows(drv));
    }

    pub fn register_linux_driver(&mut self, drv: Arc<dyn LinuxDriver>) {
        let name = drv.name().to_string();
        println!("[VDM - Rust] Registered Linux Driver: '{}' (Bridge: Accessible via \\Device\\{})", name, name);
        self.drivers.insert(name, UnifiedDriver::Linux(drv));
    }

    /// Read call originated from a Linux process/syscall targeting a device.
    /// Can read from BOTH Linux drivers and Windows WDM drivers seamlessly!
    pub fn linux_read_device(&self, dev_name: &str, buf: &mut [u8], offset: u64) -> Result<usize, PosixError> {
        match self.drivers.get(dev_name) {
            Some(UnifiedDriver::Linux(ldrv)) => ldrv.read(buf, offset),
            Some(UnifiedDriver::Windows(wdrv)) => {
                println!("[VDM Bridge] Intercepted Linux read() -> Translating to Windows IRP_MJ_READ on '{}'...", dev_name);
                let mut irp = Irp::new_read(buf.len(), offset);
                let status = wdrv.dispatch(&mut irp);
                if status.is_success() {
                    let transferred = irp.bytes_transferred.min(buf.len());
                    buf[..transferred].copy_from_slice(&irp.user_buffer[..transferred]);
                    Ok(transferred)
                } else {
                    Err(PosixError::EIO)
                }
            }
            None => Err(PosixError::ENODEV),
        }
    }

    /// IRP dispatch call originated from a Windows process/syscall.
    /// Can dispatch to BOTH Windows WDM drivers and Linux drivers seamlessly!
    pub fn windows_dispatch_irp(&self, dev_name: &str, irp: &mut Irp) -> NtStatus {
        match self.drivers.get(dev_name) {
            Some(UnifiedDriver::Windows(wdrv)) => wdrv.dispatch(irp),
            Some(UnifiedDriver::Linux(ldrv)) => {
                println!("[VDM Bridge] Intercepted Windows IRP -> Translating to Linux file_operations on '{}'...", dev_name);
                if irp.major_function == Irp::IRP_MJ_READ {
                    match ldrv.read(&mut irp.user_buffer, irp.file_offset) {
                        Ok(n) => {
                            irp.bytes_transferred = n;
                            NtStatus::SUCCESS
                        }
                        Err(_) => NtStatus::UNSUCCESSFUL,
                    }
                } else {
                    NtStatus::NOT_IMPLEMENTED
                }
            }
            None => NtStatus::FILE_NOT_FOUND,
        }
    }
}
