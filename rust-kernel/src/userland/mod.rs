//! Userland Subsystem for HybridOS Kernel.
//!
//! Provides the Userland Shell, Command Line Interface, Coreutils,
//! and Subsystem Execution Environments.

pub mod shell;

pub use shell::{HybridShell, ShellError, ShellResult};
