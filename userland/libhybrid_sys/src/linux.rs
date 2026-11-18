//! Linux System V AMD64 ABI Syscall Primitives for HybridOS Ring 3.

use core::arch::asm;

pub const SYS_READ: u64        = 0;
pub const SYS_WRITE: u64       = 1;
pub const SYS_OPEN: u64        = 2;
pub const SYS_CLOSE: u64       = 3;
pub const SYS_MMAP: u64        = 9;
pub const SYS_MUNMAP: u64      = 11;
pub const SYS_IOCTL: u64       = 16;
pub const SYS_SCHED_YIELD: u64 = 24;
pub const SYS_GETPID: u64      = 39;
pub const SYS_KILL: u64        = 62;
pub const SYS_EXIT: u64        = 60;
pub const SYS_SYSINFO: u64 = 200;
pub const SYS_PCI_ENUM: u64 = 201;
pub const SYS_PROC_LIST: u64 = 202;
pub const PROT_NONE: u32       = 0x0;
pub const PROT_READ: u32       = 0x1;
pub const PROT_WRITE: u32      = 0x2;
pub const PROT_EXEC: u32       = 0x4;
pub const MAP_SHARED: u32      = 0x01;
pub const MAP_PRIVATE: u32     = 0x02;
pub const MAP_ANONYMOUS: u32   = 0x20;
pub const STDIN_FILENO: usize  = 0;
pub const STDOUT_FILENO: usize = 1;
pub const STDERR_FILENO: usize = 2;

#[inline(always)]
pub unsafe fn syscall0(nr: u64) -> isize {
    let ret: isize;
    asm!("syscall", inlateout("rax") nr => ret, out("rcx") _, out("r11") _, options(nostack, preserves_flags));
    ret
}
#[inline(always)]
pub unsafe fn syscall1(nr: u64, a1: u64) -> isize {
    let ret: isize;
    asm!("syscall", inlateout("rax") nr => ret, in("rdi") a1, out("rcx") _, out("r11") _, options(nostack, preserves_flags));
    ret
}
#[inline(always)]
pub unsafe fn syscall2(nr: u64, a1: u64, a2: u64) -> isize {
    let ret: isize;
    asm!("syscall", inlateout("rax") nr => ret, in("rdi") a1, in("rsi") a2, out("rcx") _, out("r11") _, options(nostack, preserves_flags));
    ret
}
#[inline(always)]
pub unsafe fn syscall3(nr: u64, a1: u64, a2: u64, a3: u64) -> isize {
    let ret: isize;
    asm!("syscall", inlateout("rax") nr => ret, in("rdi") a1, in("rsi") a2, in("rdx") a3, out("rcx") _, out("r11") _, options(nostack, preserves_flags));
    ret
}
#[inline(always)]
pub unsafe fn syscall6(nr: u64, a1: u64, a2: u64, a3: u64, a4: u64, a5: u64, a6: u64) -> isize {
    let ret: isize;
    asm!("syscall", inlateout("rax") nr => ret, in("rdi") a1, in("rsi") a2, in("rdx") a3, in("r10") a4, in("r8") a5, in("r9") a6, out("rcx") _, out("r11") _, options(nostack, preserves_flags));
    ret
}
pub fn sys_read(fd: usize, buf: &mut [u8]) -> isize { unsafe { syscall3(SYS_READ, fd as u64, buf.as_mut_ptr() as u64, buf.len() as u64) } }
pub fn sys_write(fd: usize, buf: &[u8]) -> isize { unsafe { syscall3(SYS_WRITE, fd as u64, buf.as_ptr() as u64, buf.len() as u64) } }
pub fn sys_open(path: &str, flags: u32, mode: u32) -> isize { unsafe { syscall3(SYS_OPEN, path.as_ptr() as u64, flags as u64, mode as u64) } }
pub fn sys_close(fd: usize) -> isize { unsafe { syscall1(SYS_CLOSE, fd as u64) } }
pub fn sys_mmap(addr: u64, len: usize, prot: u32, flags: u32, fd: usize, offset: u64) -> *mut u8 { let ret = unsafe { syscall6(SYS_MMAP, addr, len as u64, prot as u64, flags as u64, fd as u64, offset) }; ret as *mut u8 }
pub fn sys_munmap(addr: *mut u8, len: usize) -> isize { unsafe { syscall2(SYS_MUNMAP, addr as u64, len as u64) } }
pub fn sys_sched_yield() -> isize { unsafe { syscall0(SYS_SCHED_YIELD) } }
pub fn sys_getpid() -> usize {
    let r: isize = unsafe { syscall0(SYS_GETPID) };
    r as usize
}
pub fn sys_exit(code: i32) -> ! {
    let a: u64 = code as u64;
    unsafe { syscall1(SYS_EXIT, a); loop { asm!("ud2", options(noreturn)); } }
}
pub fn sys_ioctl(fd: usize, cmd: u64, arg: u64) -> isize { unsafe { syscall3(SYS_IOCTL, fd as u64, cmd, arg) } }
pub fn sys_kill(pid: i64, sig: i32) -> isize { unsafe { syscall2(SYS_KILL, pid as u64, sig as u64) } }

#[repr(C)] #[derive(Debug, Clone, Copy, Default)] pub struct SysInfo { pub total_mem_bytes: u64, pub free_mem_bytes: u64, pub used_mem_bytes: u64, pub total_frames: u64, pub free_frames: u64, pub heap_used_bytes: u64, pub heap_free_bytes: u64, pub context_switches: u64, pub total_ticks: u64, pub uptime_ticks: u64, pub cpu_count: u32, _reserved: [u32; 3] }
#[repr(C)] #[derive(Debug, Clone, Copy, Default)] pub struct PciEntry { pub vendor_id: u32, pub device_id: u32, pub class_code: u32, pub subclass: u32, pub bus: u32, pub dev: u32, pub func: u32, pub irq: u32, }
#[repr(C)] #[derive(Debug, Clone, Copy, Default)] pub struct ProcEntry { pub pid: u64, pub personality: u32, pub status: u32, pub name: [u8; 32], }

pub fn sys_sysinfo(info: &mut SysInfo) -> isize {
    let ptr: *mut SysInfo = info;
    unsafe { syscall6(SYS_SYSINFO, ptr as u64, 0, 0, 0, 0, 0) }
}
pub fn sys_pci_enum(entries: *mut PciEntry, max_entries: u64) -> isize { unsafe { syscall2(SYS_PCI_ENUM, entries as u64, max_entries) } }
pub fn sys_proc_list(entries: *mut ProcEntry, max_entries: u64) -> isize { unsafe { syscall2(SYS_PROC_LIST, entries as u64, max_entries) } }
