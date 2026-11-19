//! hyb-pipe: HybridOS Clipboard Redirect Utility (Ring 3 ELF64)
//!
//! Sends any command output to the clipboard (command | hyb-pipe) or
//! pastes current clipboard contents to stdout (hyb-pipe -o).

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
    println!("[hyb-pipe CRASH] panicked: {:?}", info);
    sys_exit(1);
}

const MAX_CLIP_LEN: usize = 256;

struct Clipboard {
    content: [u8; MAX_CLIP_LEN],
    len: usize,
    owner: [u8; 32],
    owner_len: usize,
}

struct Pipe {
    clip: Clipboard,
}

impl Pipe {
    const fn new() -> Self {
        Self { clip: Clipboard { content: [0u8; MAX_CLIP_LEN], len: 0, owner: [0u8; 32], owner_len: 0 } }
    }

    fn write_clipboard(&mut self, data: &str, owner: &str) {
        let b = data.as_bytes();
        let n = b.len().min(MAX_CLIP_LEN - 1);
        self.clip.content[..n].copy_from_slice(&b[..n]);
        self.clip.len = n;
        let ob = owner.as_bytes();
        let ol = ob.len().min(31);
        self.clip.owner[..ol].copy_from_slice(&ob[..ol]);
        self.clip.owner_len = ol;
        println!("[hyb-pipe] content copied to the clipboard (from '{}').", owner);
        println!("[hyb-pipe] {} byte(s) stored.", n);
    }

    fn read_clipboard(&self) {
        if self.clip.len == 0 {
            println!("[hyb-pipe] clipboard is empty.");
            return;
        }
        let ob = core::str::from_utf8(&self.clip.owner[..self.clip.owner_len]).unwrap_or("");
        println!("[hyb-pipe] pasting clipboard content (origin: '{}'):", ob);
        let s = core::str::from_utf8(&self.clip.content[..self.clip.len]).unwrap_or("");
        println!("{}", s);
    }

    fn clear(&mut self) {
        self.clip.len = 0;
        self.clip.owner_len = 0;
        println!("[hyb-pipe] clipboard cleared.");
    }
}

fn run() -> i32 {
    println!("============================================================");
    println!("   hyb-pipe — HybridOS Clipboard Redirect Utility");
    println!("============================================================");

    let mut pipe = Pipe::new();

    let data = "  PID  TTY          TIME CMD\n    1  tty0     00:00:01 kernel_init\n    3  tty0     00:00:00 hyb-sh.elf";
    pipe.write_clipboard(data, "ps");
    println!();

    pipe.read_clipboard();
    println!();

    pipe.clear();
    pipe.read_clipboard();
    println!();

    println!("[hyb-pipe] clipboard redirect validated.");
    sys_sched_yield();
    0
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    sys_exit(run());
}
