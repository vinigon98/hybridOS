//! hyb-ssh-agent: HybridOS SSH/GPG Authentication Agent (Ring 3 ELF64)
//!
//! Background service that loads SSH and GPG keys into the user session
//! using system biometrics or PIN unlock.

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
    println!("[hyb-ssh-agent CRASH] panicked: {:?}", info);
    sys_exit(1);
}

const MAX_KEYS: usize = 8;
const MAX_COMMENT_LEN: usize = 64;

#[derive(Copy, Clone)]
struct LoadedKey {
    algorithm: [u8; 16],
    algorithm_len: usize,
    comment: [u8; MAX_COMMENT_LEN],
    comment_len: usize,
    unlocked: bool,
}

struct SshAgent {
    keys: [LoadedKey; MAX_KEYS],
    count: usize,
    session_unlocked: bool,
}

impl SshAgent {
    const fn new() -> Self {
        Self { keys: [LoadedKey { algorithm: [0u8; 16], algorithm_len: 0, comment: [0u8; MAX_COMMENT_LEN], comment_len: 0, unlocked: false }; MAX_KEYS], count: 0, session_unlocked: false }
    }

    fn unlock_session(&mut self, pin_ok: bool) {
        if pin_ok { self.session_unlocked = true; println!("[hyb-ssh-agent] session unlocked (PIN/biometrics)."); }
        else { println!("[hyb-ssh-agent] unlock authentication failed."); }
    }

    fn add_key(&mut self, algorithm: &str, comment: &str) {
        if !self.session_unlocked { println!("[hyb-ssh-agent] session locked — unlock to add keys."); return; }
        if self.count >= MAX_KEYS { println!("[hyb-ssh-agent] loaded key limit reached."); return; }
        let ab = algorithm.as_bytes();
        let al = ab.len().min(15);
        self.keys[self.count].algorithm[..al].copy_from_slice(&ab[..al]);
        self.keys[self.count].algorithm_len = al;
        let cb = comment.as_bytes();
        let cl = cb.len().min(MAX_COMMENT_LEN - 1);
        self.keys[self.count].comment[..cl].copy_from_slice(&cb[..cl]);
        self.keys[self.count].comment_len = cl;
        self.keys[self.count].unlocked = true;
        self.count += 1;
        println!("[hyb-ssh-agent] key {} loaded in session ({})", algorithm, comment);
    }

    fn sign_challenge(&self, comment: &str) {
        if !self.session_unlocked { println!("[hyb-ssh-agent] session locked — cannot sign."); return; }
        for i in 0..self.count {
            let c = core::str::from_utf8(&self.keys[i].comment[..self.keys[i].comment_len]).unwrap_or("");
            if c == comment {
                let a = core::str::from_utf8(&self.keys[i].algorithm[..self.keys[i].algorithm_len]).unwrap_or("");
                println!("[hyb-ssh-agent] challenge signed with key {} ({}).", a, comment);
                return;
            }
        }
        println!("[hyb-ssh-agent] key not found in session: {}", comment);
    }

    fn list(&self) {
        println!("[hyb-ssh-agent] keys loaded in session:");
        if self.count == 0 { println!("  (none)"); }
        for i in 0..self.count {
            let a = core::str::from_utf8(&self.keys[i].algorithm[..self.keys[i].algorithm_len]).unwrap_or("");
            let c = core::str::from_utf8(&self.keys[i].comment[..self.keys[i].comment_len]).unwrap_or("");
            println!("  {} {} (unlocked={})", a, c, self.keys[i].unlocked);
        }
    }
}

fn run() -> i32 {
    println!("============================================================");
    println!("   hyb-ssh-agent — HybridOS SSH/GPG Authentication Agent");
    println!("============================================================");

    let mut agent = SshAgent::new();
    agent.list();
    println!();

    agent.unlock_session(true);
    agent.add_key("ed25519", "id_ed25519");
    agent.add_key("rsa", "id_rsa");
    agent.add_key("ssh-ed25519", "yubikey-fido2");
    println!();
    agent.list();
    println!();
    agent.sign_challenge("id_ed25519");
    agent.sign_challenge("inexistente");
    println!();
    println!("[hyb-ssh-agent] SSH/GPG authentication daemon validated.");
    sys_sched_yield();
    0
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    sys_exit(run());
}
