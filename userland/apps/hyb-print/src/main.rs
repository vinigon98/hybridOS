//! hyb-print: Spooler de Impressao (ELF64)
//! Fila de impressao via VFS (/var/spool/print/).

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
    println!("[hyb-print CRASH] {:?}", info);
    sys_exit(1);
}

struct PrintJob {
    name: [u8; 32],
    name_len: usize,
    size: u32,
}

impl PrintJob {
    fn new(name: &str, size: u32) -> Self {
        let bytes = name.as_bytes();
        let len = bytes.len().min(32);
        let mut arr = [0u8; 32];
        arr[..len].copy_from_slice(&bytes[..len]);
        Self { name: arr, name_len: len, size }
    }
    fn name_str(&self) -> String {
        String::from_utf8_lossy(&self.name[..self.name_len]).into_owned()
    }
}

fn u32_to_dec(mut n: u32) -> String {
    if n == 0 {
        return String::from("0");
    }
    let mut digits: [u8; 10] = [0; 10];
    let mut len = 0usize;
    while n > 0 {
        digits[len] = b'0' + (n % 10) as u8;
        n /= 10;
        len += 1;
    }
    let mut s = String::new();
    while len > 0 {
        len -= 1;
        s.push(digits[len] as char);
    }
    s
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
    println!("   HybridOS Spooler de Impressao (hyb-print)");
    println!("============================================================");

    let pid = sys_getpid();
    println!("PID: {} | Spooler de impressao do sistema", pid);

    let mut queue: Vec<PrintJob> = Vec::new();
    queue.push(PrintJob::new("relatorio_financeiro.pdf", 245760));
    queue.push(PrintJob::new("manual_usuario.docx", 131072));
    queue.push(PrintJob::new("fatura_cliente.pdf", 98304));
    queue.push(PrintJob::new("diagrama_rede.vsdx", 196608));

    println!();
    println!("--- Fila de impressao (/var/spool/print/) ---");
    println!("  {:<32} {:<10} {:<10}", "Job", "Tamanho", "Status");
    println!("  {:<32} {:<10} {:<10}", "----", "-------", "------");
    for job in queue.iter() {
        println!("  {:<32} {:<10} {:<10}", job.name_str(), u32_to_dec(job.size), "AGUARDANDO");
    }

    println!();
    println!("--- Processando spool (gravacao no VFS) ---");
    let mut processed: u32 = 0;
    for job in queue.iter() {
        let mut spool_path = String::new();
        spool_path.push_str("/var/spool/print/");
        spool_path.push_str(&job.name_str());
        spool_path.push_str(".spool");

        let mut job_data = String::new();
        job_data.push_str("PrintJob:");
        job_data.push_str(&job.name_str());
        job_data.push_str("\nSize:");
        job_data.push_str(&u32_to_dec(job.size));
        job_data.push_str("\nStatus:completed\n");

        if write_file(spool_path.as_str(), job_data.as_bytes()) {
            println!("  [IMPRESSO] {} -> {} ({} bytes)", job.name_str(), spool_path, u32_to_dec(job.size));
            processed += 1;
        } else {
            println!("  [FALHA] Nao foi possivel spool de {}", job.name_str());
        }
    }

    let mut report = String::new();
    report.push_str("hyb-print: ");
    report.push_str(&u32_to_dec(processed));
    report.push_str(" jobs processados\n");
    let _ = write_file("/tmp/.print_log", report.as_bytes());

    println!();
    println!("  Total de jobs: {}", queue.len());
    println!("  Jobs processados: {}", u32_to_dec(processed));
    println!("  Spooler concluido.");

    sys_sched_yield();
    println!();
    println!("[hyb-print] Spooler de impressao concluido (VFS do kernel).");
    sys_exit(0);
}
