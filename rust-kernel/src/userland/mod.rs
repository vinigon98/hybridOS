//! Userland Subsystem for HybridOS Kernel.
//!
//! Provides the Userland Shell, Command Line Interface, Coreutils,
//! and Subsystem Execution Environments.

#[allow(unused_imports)]
use alloc::string::{String, ToString};
#[allow(unused_imports)]
use alloc::vec::Vec;
#[allow(unused_imports)]
use alloc::boxed::Box;
#[allow(unused_imports)]
use crate::compat::FloatExt;
pub mod shell;

pub use shell::{HybridShell, ShellError, ShellResult};
