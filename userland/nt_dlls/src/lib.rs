//! nt_dlls: Real Windows Subsystem Core DLL Implementations in Ring 3 for HybridOS.
//!
//! Exposes Win64 ABI functions for:
//! - ntdll.dll
//! - kernel32.dll
//! - user32.dll
//! - gdi32.dll

#![no_std]

pub mod ntdll;
pub mod kernel32;
pub mod user32;
pub mod gdi32;

