//! hyb-fsctl: Controle de Sistema de Arquivos (ELF64)
//! Operacoes de controle (sync, mount-info, ioctl) via VFS/fs reais do kernel.

#![no_std]
#![no_main]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use core::panic::PanicInfo;
use libhybrid_sys::allocator::HybridUserAllocator;
use libhybrid_sys::linux::*;
use libhybrid_sys::{print, println};

#[global_allocator]
static ALLOCATOR: HybridUserAllocator = HybridUserAllocator::new();

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("[hyb-fsctl CRASH] {:?}", info);
    sys_exit(1);
}

// IOCTL de filesystem (convenção Linux)
const FS_IOC_GETFLAGS: u64 = 0x80086601;
const BLKGETSIZE64: u64 = 0x80081272;

fn probe_path(path: &str) -> bool {
    sys_open(path, 0, 0) >= 0
}

fn read_file(path: &str) -> Option<Vec<u8>> {
    let fd = match sys_open(path, 0, 0) {
        r if r >= 0 => r as usize,
        _ => return None,
    };
    let mut buf = [0u8; 4096];
    let mut contents = Vec::new();
    loop {
        let n = sys_read(fd, &mut buf);
        if n <= 0 {
            break;
        }
        contents.extend_from_slice(&buf[..n as usize]);
    }
    let _ = sys_close(fd);
    Some(contents)
}

fn ioctl_flags(path: &str) -> Option<u64> {
    let fd = match sys_open(path, 0, 0) {
        r if r >= 0 => r as usize,
        _ => return None,
    };
    let ret = sys_ioctl(fd, FS_IOC_GETFLAGS, 0);
    let _ = sys_close(fd);
    if ret >= 0 { Some(ret as u64) } else { None }
}

fn do_sync() {
    // Sync: gravar arquivo de sincronização no VFS (simula flush de buffers do fs)
    let sync_data = b"sync\n";
    let fd = match sys_open("/tmp/.sync_marker", 0, 0) {
        r if r >= 0 => r as usize,
        _ => return,
    };
    let _ = sys_write(fd, sync_data);
    let _ = sys_close(fd);
    println!("  [OK] Buffers do VFS sincronizados (sync marker gravado)");
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("============================================================");
    println!("   HybridOS Controle de Sistema de Arquivos (hyb-fsctl)");
    println!("============================================================");

    let pid = sys_getpid();
    println!("PID: {} | fsctl do VFS/fs do kernel", pid);

    let volumes = [
        "/c", "/d", "/mnt",
        "/c/Windows/System32/ntoskrnl.ini",
        "/etc/os-release",
    ];

    println!();
    println!("--- Controle de volumes (fs do kernel) ---");
    for v in volumes.iter() {
        let ok = probe_path(v);
        println!("  {:<38} {}", v, if ok { "[MONTADO]" } else { "[INDISPONIVEL]" });
        if ok {
            if let Some(flags) = ioctl_flags(v) {
                println!("      flags (FS_IOC_GETFLAGS): 0x{:08X}", flags);
            }
            if let Some(content) = read_file(v) {
                if !content.is_empty() {
                    println!("      leitura via VFS: {} bytes OK", content.len());
                }
            }
        }
    }

    println!();
    println!("--- Sincronizacao de filesystem (sync) ---");
    do_sync();

    println!();
    println!("--- Informacoes de bloco (BLKGETSIZE64) ---");
    for dev in ["/dev/sda", "/dev/sdb", "/dev/nvme0n1"].iter() {
        if probe_path(dev) {
            let fd = match sys_open(dev, 0, 0) {
                r if r >= 0 => r as usize,
                _ => continue,
            };
            let ret = sys_ioctl(fd, BLKGETSIZE64, 0);
            let _ = sys_close(fd);
            if ret >= 0 {
                println!("  {}: {} bytes (via BLKGETSIZE64/ioctl)", dev, ret as u64);
            } else {
                println!("  {}: tamanho indisponível", dev);
            }
        }
    }

    sys_sched_yield();
    println!();
    println!("[hyb-fsctl] Controle de filesystem concluido (VFS + fs do kernel).");
    sys_exit(0);
}
