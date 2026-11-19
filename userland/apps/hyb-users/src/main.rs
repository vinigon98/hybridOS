//! hyb-users: Gerenciamento de Usuarios (ELF64)
//! Lista e gerencia contas de usuario com persistência em VFS (/etc/passwd-style).

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
    println!("[hyb-users CRASH] {:?}", info);
    sys_exit(1);
}

struct User {
    username: [u8; 32],
    username_len: usize,
    uid: u32,
    shell: [u8; 16],
    shell_len: usize,
}

impl User {
    fn new(username: &str, uid: u32, shell: &str) -> Self {
        let ub = username.as_bytes();
        let ul = ub.len().min(32);
        let mut ua = [0u8; 32];
        ua[..ul].copy_from_slice(&ub[..ul]);
        let sb = shell.as_bytes();
        let sl = sb.len().min(16);
        let mut sa = [0u8; 16];
        sa[..sl].copy_from_slice(&sb[..sl]);
        Self { username: ua, username_len: ul, uid, shell: sa, shell_len: sl }
    }

    fn username_str(&self) -> String {
        String::from_utf8_lossy(&self.username[..self.username_len]).into_owned()
    }

    fn shell_str(&self) -> String {
        String::from_utf8_lossy(&self.shell[..self.shell_len]).into_owned()
    }
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

fn write_file(path: &str, data: &[u8]) -> bool {
    let fd = match sys_open(path, 0, 0) {
        r if r >= 0 => r as usize,
        _ => return false,
    };
    let mut written = 0usize;
    while written < data.len() {
        let n = sys_write(fd, &data[written..]);
        if n <= 0 {
            break;
        }
        written += n as usize;
    }
    let _ = sys_close(fd);
    written == data.len()
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("============================================================");
    println!("   HybridOS Gerenciamento de Usuarios (hyb-users)");
    println!("============================================================");

    let pid = sys_getpid();
    println!("PID do gerenciador: {}", pid);

    // Contas de usuario do sistema (modelo real: /etc/passwd)
    let mut users: Vec<User> = Vec::new();
    users.push(User::new("Administrator", 0, "/bin/sh"));
    users.push(User::new("hybridos", 1000, "/bin/sh"));
    users.push(User::new("guest", 1001, "/bin/false"));

    println!();
    println!("--- Contas de usuario no sistema ---");
    println!("  {:<16} {:<6} {:<12}", "Usuario", "UID", "Shell");
    println!("  {:<16} {:<6} {:<12}", "--------", "---", "-----");
    for u in users.iter() {
        println!("  {:<16} {:<6} {:<12}", u.username_str(), u.uid, u.shell_str());
    }

    // Ler arquivo de senhas via VFS se existir
    println!();
    println!("--- Arquivo de identidades no VFS (/etc/passwd) ---");
    if let Some(content) = read_file("/etc/passwd") {
        if content.is_empty() {
            println!("  (aberto via VFS, vazio)");
        } else {
            let text = String::from_utf8_lossy(&content);
            print!("{}", text);
        }
    } else {
        // Criar arquivo de identidades padrao via VFS
        let passwd = b"Administrator:x:0:0:Administrator:/root:/bin/sh\nhybridos:x:1000:1000:HybridOS User:/home/hybridos:/bin/sh\nguest:x:1001:1001:Guest:/home/guest:/bin/false\n";
        if write_file("/etc/passwd", passwd) {
            println!("  Criado /etc/passwd no VFS do kernel com contas padrao.");
        } else {
            println!("  Nao foi possivel criar /etc/passwd via VFS");
        }
    }

    sys_sched_yield();
    println!();
    println!("[hyb-users] Gerenciamento de usuarios concluido (VFS do kernel).");
    sys_exit(0);
}
