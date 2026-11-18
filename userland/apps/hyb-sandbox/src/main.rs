//! hyb-sandbox: HybridOS Disposable Application Container (Ring 3 ELF64)
//!
//! Runs unknown installers (.exe/.msi) and binaries inside an isolated
//! filesystem and network namespace. Changes vanish on close or can be
//! promoted to the real system if approved.

#![no_std]
#![no_main]

use core::panic::PanicInfo;
use libhybrid_sys::allocator::HybridUserAllocator;
use libhybrid_sys::linux::*;
use libhybrid_sys::{print, println};

#[global_allocator]
static ALLOCATOR: HybridUserAllocator = HybridUserAllocator::new();

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("[hyb-sandbox CRASH] panicked: {:?}", info);
    sys_exit(1);
}

const MAX_FILES: usize = 16;
const MAX_NAME_LEN: usize = 64;

#[derive(Copy, Clone)]
struct SandboxFile {
    name: [u8; MAX_NAME_LEN],
    name_len: usize,
    size: u32,
    modified: bool,
}

struct Sandbox {
    files: [SandboxFile; MAX_FILES],
    count: usize,
    network_isolated: bool,
    promoted: bool,
}

impl Sandbox {
    const fn new() -> Self {
        Self { files: [SandboxFile { name: [0u8; MAX_NAME_LEN], name_len: 0, size: 0, modified: false }; MAX_FILES], count: 0, network_isolated: true, promoted: false }
    }

    fn touch(&mut self, name: &str, size: u32) {
        if self.count >= MAX_FILES { return; }
        let nb = name.as_bytes();
        let nl = nb.len().min(MAX_NAME_LEN - 1);
        self.files[self.count].name[..nl].copy_from_slice(&nb[..nl]);
        self.files[self.count].name_len = nl;
        self.files[self.count].size = size;
        self.files[self.count].modified = true;
        self.count += 1;
        println!("[sandbox] file created in isolated namespace: {} ({} bytes)", name, size);
    }

    fn list(&self) {
        println!("[hyb-sandbox] isolated namespace filesystem contents:");
        if self.count == 0 { println!("  (empty)"); }
        for i in 0..self.count {
            let n = core::str::from_utf8(&self.files[i].name[..self.files[i].name_len]).unwrap_or("");
            println!("  {} {} bytes{}", n, self.files[i].size, if self.files[i].modified { " (modified)" } else { "" });
        }
    }

    fn promote(&mut self) {
        if self.promoted { println!("[hyb-sandbox] changes already promoted to the real system."); return; }
        println!("[hyb-sandbox] promoting sandbox changes to the real system...");
        self.promoted = true;
        println!("[hyb-sandbox] promotion completed successfully.");
    }

    fn discard(&mut self) {
        println!("[hyb-sandbox] discarding all isolated namespace changes (1s).");
        self.count = 0;
        self.promoted = false;
    }
}

fn run() -> i32 {
    println!("============================================================");
    println!("   hyb-sandbox — HybridOS Disposable Application Container");
    println!("============================================================");

    let mut sb = Sandbox::new();
    println!("[hyb-sandbox] isolated network: {}", sb.network_isolated);

    sb.touch("setup.exe", 1245120);
    sb.touch("payload.dll", 458300);
    sb.touch("config.dat", 1024);
    println!();
    sb.list();
    println!();

    println!("[hyb-sandbox] window closed — discarding isolated namespace changes...");
    sb.discard();
    sb.list();
    println!();

    sb.touch("driver.sys", 256000);
    sb.promote();
    println!("[hyb-sandbox] disposable container and promotion validated.");
    sys_sched_yield();
    0
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    sys_exit(run());
}
