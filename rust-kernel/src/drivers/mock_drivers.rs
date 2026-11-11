#[allow(unused_imports)]
use alloc::string::{String, ToString};
#[allow(unused_imports)]
use alloc::vec::Vec;
#[allow(unused_imports)]
use alloc::boxed::Box;
#[allow(unused_imports)]
use crate::compat::FloatExt;
use crate::drivers::vdm::{WdmDriver, LinuxDriver, Irp};
use crate::types::{NtStatus, PosixError};

pub struct MockRealtekWdmDriver;

impl WdmDriver for MockRealtekWdmDriver {
    fn name(&self) -> &str {
        "net_realtek_wdm"
    }

    fn dispatch(&self, irp: &mut Irp) -> NtStatus {
        if irp.major_function == Irp::IRP_MJ_READ {
            let data = b"[Hardware Frame from Windows Realtek WDM Driver]";
            let len = data.len().min(irp.user_buffer.len());
            irp.user_buffer[..len].copy_from_slice(&data[..len]);
            irp.bytes_transferred = len;
            println!("[Windows WDM Driver] Handled IRP_MJ_READ (transferred {} bytes)", len);
            NtStatus::SUCCESS
        } else {
            NtStatus::NOT_IMPLEMENTED
        }
    }
}

pub struct MockAmdGpuLinuxDriver;

impl LinuxDriver for MockAmdGpuLinuxDriver {
    fn name(&self) -> &str {
        "drm_amdgpu_linux"
    }

    fn read(&self, buf: &mut [u8], _offset: u64) -> Result<usize, PosixError> {
        let data = b"[Vulkan Hardware Buffer from Linux AMDGPU Driver]";
        let len = data.len().min(buf.len());
        buf[..len].copy_from_slice(&data[..len]);
        println!("[Linux GPU Driver] Handled file_operations::read() ({} bytes)", len);
        Ok(len)
    }

    fn write(&self, buf: &[u8], _offset: u64) -> Result<usize, PosixError> {
        println!("[Linux GPU Driver] Wrote {} bytes to VRAM", buf.len());
        Ok(buf.len())
    }

    fn ioctl(&self, _cmd: u32, _arg: usize) -> Result<i64, PosixError> {
        Ok(0)
    }
}
