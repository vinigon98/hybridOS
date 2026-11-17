//! hyb-trim: Operacao TRIM / SSD (ELF64)
//! Dispara TRIM via sys_ioctl (mapeado para controle de bloco do VFS/fs do kernel).

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
    println!("[hyb-trim CRASH] {:?}", info);
    sys_exit(1);
}

// IOCTL commands para controle de dispositivos de bloco (convenção Linux)
const BLKDISCARD: u64 = 0x1269; // TRIM/discard de blocos
const BLKGETSIZE: u64 = 0x1260; // Tamanho do dispositivo

fn probe_path(path: &str) -> bool {
    sys_open(path, 0, 0) >= 0
}

fn ioctl_trim(path: &str) -> bool {
    let fd = match sys_open(path, 0, 0) {
        r if r >= 0 => r as usize,
        _ => return false,
    };
    // Solicita TRIM/discard do dispositivo via sys_ioctl -> camada de bloco do kernel
    let ret = sys_ioctl(fd, BLKDISCARD, 0);
    let _ = sys_close(fd);
    ret >= 0
}

fn ioctl_get_size(path: &str) -> Option<u64> {
    let fd = match sys_open(path, 0, 0) {
        r if r >= 0 => r as usize,
        _ => return None,
    };
    let ret = sys_ioctl(fd, BLKGETSIZE, 0);
    let _ = sys_close(fd);
    if ret >= 0 { Some(ret as u64) } else { None }
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("============================================================");
    println!("   HybridOS TRIM / Utilitario de SSD (hyb-trim)");
    println!("============================================================");

    let pid = sys_getpid();
    println!("PID: {} | Operacao TRIM em dispositivos de bloco", pid);

    let devices = [
        ("/dev/sda", "SSD / Disco SATA 0"),
        ("/dev/sdb", "SSD / Disco SATA 1"),
        ("/dev/nvme0n1", "NVMe SSD 0"),
        ("/dev/vda", "VirtIO-Bloco 0"),
    ];

    println!();
    println!("--- Dispositivos de bloco detectados (VFS) ---");
    for (dev, label) in devices.iter() {
        let ok = probe_path(dev);
        println!("  {:<18} {:<24} {}", dev, label, if ok { "[PRESENTE]" } else { "[AUSENTE]" });
    }

    println!();
    println!("--- Executando TRIM (discard de blocos ociosos) ---");
    for (dev, _) in devices.iter() {
        if probe_path(dev) {
            if let Some(size) = ioctl_get_size(dev) {
                println!("  {}: tamanho={} bytes (via BLKGETSIZE/ioctl)", dev, size);
            }
            if ioctl_trim(dev) {
                println!("  {}: TRIM executado com sucesso (BLKDISCARD via sys_ioctl)", dev);
            } else {
                println!("  {}: TRIM nao suportado ou falhou", dev);
            }
        } else {
            println!("  {}: ignorado (dispositivo nao acessível via VFS)", dev);
        }
    }

    sys_sched_yield();
    println!();
    println!("[hyb-trim] Operacao TRIM concluida (integracao com bloco/VFS do kernel).");
    sys_exit(0);
}
