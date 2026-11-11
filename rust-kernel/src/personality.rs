//! Execution personalities representing distinct OS runtime semantics.

#[allow(unused_imports)]
use alloc::string::{String, ToString};
#[allow(unused_imports)]
use alloc::vec::Vec;
#[allow(unused_imports)]
use alloc::boxed::Box;
#[allow(unused_imports)]
use crate::compat::FloatExt;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionPersonality {
    HybridNative,
    WindowsNt,
    LinuxPosix,
    Android,
}

impl ExecutionPersonality {
    pub fn name(&self) -> &'static str {
        match self {
            Self::HybridNative => "Hybrid-Native (Ring-0)",
            Self::WindowsNt => "Windows NT x64 (PE32+ / SSDT)",
            Self::LinuxPosix => "Linux POSIX (ELF64 / SysV)",
            Self::Android => "Android (Bionic / Binder / HAL)",
        }
    }
}
