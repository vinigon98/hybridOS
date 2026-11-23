//! Windows NT Environment, TEB, PEB and KUSER_SHARED_DATA for HybridOS Ring 3.

use core::arch::asm;

pub const KUSER_SHARED_DATA_ADDR: u64 = 0x7FFE_0000;
pub const USER_TEB_ADDR: u64          = 0x7FFE_1000;
pub const USER_PEB_ADDR: u64          = 0x7FFE_2000;

/// KUSER_SHARED_DATA structure at fixed address 0x7FFE0000
#[repr(C)]
pub struct KUserSharedData {
    pub tick_count_low_deprecated: u32,       // 0x0000
    pub tick_count_multiplier: u32,           // 0x0004
    pub interrupt_time: u64,                  // 0x0008
    pub system_time: u64,                     // 0x0014
    pub time_zone_bias: u64,                  // 0x0020
    pub _reserved0: [u8; 0x026C - 0x0028],
    pub nt_major_version: u32,                // 0x026C
    pub nt_minor_version: u32,                // 0x0270
    pub _reserved1: [u8; 0x0320 - 0x0274],
    pub tick_count_quad: u64,                 // 0x0320
}

/// Reads the linear address of the TEB (Thread Environment Block) from GS:[0x30]
#[inline(always)]
pub fn get_teb_address() -> u64 {
    let teb_addr: u64;
    unsafe {
        asm!(
            "mov {}, gs:[0x30]",
            out(reg) teb_addr,
            options(nostack, preserves_flags)
        );
    }
    teb_addr
}

/// Reads the linear address of the PEB (Process Environment Block) from GS:[0x60]
#[inline(always)]
pub fn get_peb_address() -> u64 {
    let peb_addr: u64;
    unsafe {
        asm!(
            "mov {}, gs:[0x60]",
            out(reg) peb_addr,
            options(nostack, preserves_flags)
        );
    }
    peb_addr
}

/// Reads the current Process ID (PID) from GS:[0x40]
#[inline(always)]
pub fn get_current_process_id() -> u32 {
    let pid: u64;
    unsafe {
        asm!(
            "mov {}, gs:[0x40]",
            out(reg) pid,
            options(nostack, preserves_flags)
        );
    }
    pid as u32
}

/// Reads the current Thread ID (TID) from GS:[0x48]
#[inline(always)]
pub fn get_current_thread_id() -> u32 {
    let tid: u64;
    unsafe {
        asm!(
            "mov {}, gs:[0x48]",
            out(reg) tid,
            options(nostack, preserves_flags)
        );
    }
    tid as u32
}

/// Reads LastErrorValue from GS:[0x68]
#[inline(always)]
pub fn get_last_error() -> u32 {
    let err: u32;
    unsafe {
        asm!(
            "mov {:e}, gs:[0x68]",
            out(reg) err,
            options(nostack, preserves_flags)
        );
    }
    err
}

/// Writes LastErrorValue into GS:[0x68]
#[inline(always)]
pub fn set_last_error(err: u32) {
    unsafe {
        asm!(
            "mov gs:[0x68], {:e}",
            in(reg) err,
            options(nostack, preserves_flags)
        );
    }
}

/// Obtains a static reference to KUSER_SHARED_DATA at 0x7FFE0000
#[inline(always)]
pub fn get_kuser_shared_data() -> &'static KUserSharedData {
    unsafe { &*(KUSER_SHARED_DATA_ADDR as *const KUserSharedData) }
}

/// Reads TickCount64 directly from KUSER_SHARED_DATA without needing any system call
#[inline(always)]
pub fn get_tick_count64() -> u64 {
    let kuser = get_kuser_shared_data();
    kuser.tick_count_quad
}

/// Reads NT Major Version from KUSER_SHARED_DATA (0x7FFE026C)
#[inline(always)]
pub fn get_nt_major_version() -> u32 {
    let ptr = (KUSER_SHARED_DATA_ADDR + 0x026C) as *const u32;
    unsafe { *ptr }
}

/// Reads NT Minor Version from KUSER_SHARED_DATA (0x7FFE0270)
#[inline(always)]
pub fn get_nt_minor_version() -> u32 {
    let ptr = (KUSER_SHARED_DATA_ADDR + 0x0270) as *const u32;
    unsafe { *ptr }
}
