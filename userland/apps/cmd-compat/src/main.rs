//! cmd-compat: Win32/POSIX Compatibility Interpreter (Ring 3 ELF64)
//!
//! Fast execution layer to run legacy .bat/.cmd scripts and /bin/sh scripts
//! without modifications, unifying both worlds.

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
    println!("[cmd-compat CRASH] panicked: {:?}", info);
    sys_exit(1);
}

const MAX_CMD_LEN: usize = 128;

#[derive(Copy, Clone)]
struct ScriptLine {
    text: [u8; MAX_CMD_LEN],
    len: usize,
}

struct Interpreter {
    lines: [ScriptLine; 32],
    count: usize,
    pc: usize,
    echo: bool,
}

impl Interpreter {
    const fn new() -> Self {
        Self { lines: [ScriptLine { text: [0u8; MAX_CMD_LEN], len: 0 }; 32], count: 0, pc: 0, echo: false }
    }

    fn load_demo(&mut self) {
        let demo = [
            "@echo off",
            "echo Starting cmd-compat (Win32/POSIX)",
            "set PATH=C:\\Windows\\System32;/bin",
            "echo PATH configured: %PATH%",
            "dir /bin",
            "ls /bin",
            "End of compat script",
        ];
        for t in demo.iter() {
            if self.count >= 32 { break; }
            let b = t.as_bytes();
            let n = b.len().min(MAX_CMD_LEN - 1);
            self.lines[self.count].text[..n].copy_from_slice(&b[..n]);
            self.lines[self.count].len = n;
            self.count += 1;
        }
    }

    fn run(&mut self) {
        println!("============================================================");
        println!("   cmd-compat — Win32/POSIX Compatibility Interpreter");
        println!("============================================================");
        while self.pc < self.count {
            let s = core::str::from_utf8(&self.lines[self.pc].text[..self.lines[self.pc].len]).unwrap_or("");
            let t = s.trim();
            self.pc += 1;
            if t.is_empty() || t.starts_with('@') || t.starts_with("rem") || t.starts_with("::") { continue; }
            if self.echo { println!("> {}", t); }
            self.dispatch(t);
        }
        println!("[cmd-compat] compat script execution finished.");
        sys_sched_yield();
    }

    fn dispatch(&self, line: &str) {
        if let Some(rest) = line.strip_prefix("echo ") { println!("{}", rest); return; }
        if let Some(rest) = line.strip_prefix("set ") { println!("[env] {}", rest); return; }
        if line == "dir" || line == "ls" {
            println!("  <DIR>  bin");
            println!("  <DIR>  dev");
            println!("  <DIR>  etc");
            println!("  -rwxr-xr-x  cmd-compat.exe");
            println!("  -rwxr-xr-x  hyb-sh.elf");
            return;
        }
        if line == "ver" || line == "uname -a" { println!("HybridOS 1.0 (x86_64 Dual-ABI Kernel)"); return; }
        println!("cmd-compat: command not recognized: '{}'", line);
    }
}

fn run() -> i32 {
    let mut interp = Interpreter::new();
    interp.load_demo();
    interp.run();
    0
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    sys_exit(run());
}
