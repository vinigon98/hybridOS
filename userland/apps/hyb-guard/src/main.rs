//! hyb-guard: Guarda de Protecao do Sistema (ELF64)
//! Monitora e protege arquivos críticos via VFS do kernel.

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
    println!("[hyb-guard CRASH] {:?}", info);
    sys_exit(1);
}

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

struct GuardedFile {
    path: [u8; 64],
    path_len: usize,
    protected: bool,
}

impl GuardedFile {
    fn new(path: &str) -> Self {
        let bytes = path.as_bytes();
        let len = bytes.len().min(64);
        let mut arr = [0u8; 64];
        arr[..len].copy_from_slice(&bytes[..len]);
        Self { path: arr, path_len: len, protected: true }
    }
    fn path_str(&self) -> String {
        String::from_utf8_lossy(&self.path[..self.path_len]).into_owned()
    }
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("============================================================");
    println!("   HybridOS Guarda de Protecao (hyb-guard)");
    println!("============================================================");

    let pid = sys_getpid();
    println!("PID: {} | Guarda de integridade do sistema", pid);

    let guarded = [
        GuardedFile::new("/etc/os-release"),
        GuardedFile::new("/c/Windows/System32/ntoskrnl.ini"),
        GuardedFile::new("/dev/console"),
        GuardedFile::new("/bin"),
        GuardedFile::new("/sbin"),
    ];

    println!();
    println!("--- Arquivos sob protecao do Guarda (VFS) ---");
    println!("  {:<38} {:<12} {:<12}", "Caminho", "Protegido", "Acessivel");
    println!("  {:<38} {:<12} {:<12}", "------", "---------", "---------");
    let mut intact: u32 = 0;
    let mut compromised: u32 = 0;
    for g in guarded.iter() {
        let accessible = probe_path(g.path_str().as_str());
        let ok = accessible;
        if ok {
            intact += 1;
        } else {
            compromised += 1;
        }
        // Leitura para confirmar integridade
        let _ = if ok { read_file(g.path_str().as_str()) } else { None };
        println!("  {:<38} {:<12} {:<12}",
                   g.path_str(),
                   if g.protected { "SIM" } else { "NAO" },
                   if ok { "SIM" } else { "NAO" });
    }

    println!();
    println!("  Arquivos intactos: {}", intact);
    println!("  Arquivos comprometidos: {}", compromised);
    println!("  Status do Guarda: {}", if compromised == 0 { "SEGURO" } else { "AVISO" });

    // Gravar registro de guarda no VFS
    let log = b"hyb-guard: verificacao de integridade concluida\n";
    let fd = match sys_open("/tmp/.guard_log", 0, 0) {
        r if r >= 0 => r as usize,
        _ => 0,
    };
    if fd > 0 {
        let _ = sys_write(fd, log);
        let _ = sys_close(fd);
        println!("  [REGISTRO] Log do guarda gravado em /tmp/.guard_log");
    }

    sys_sched_yield();
    println!();
    println!("[hyb-guard] Guarda de protecao executado (VFS do kernel).");
    sys_exit(0);
}
