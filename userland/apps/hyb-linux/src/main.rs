#![no_std] #![no_main]
use libhybrid_sys::linux::{sys_read, sys_write, sys_exit, sys_getpid, sys_proc_list, STDIN_FILENO, STDOUT_FILENO};
use libhybrid_sys::linux::ProcEntry;
fn put(s: &str) { let _ = sys_write(STDOUT_FILENO, s.as_bytes()); }
fn putln(s: &str) { put(s); put("\r\n"); }
fn put_u64(v: u64) { if v == 0 { put("0"); return; } let mut tmp = [0u8; 20]; let mut n = v; let mut pos = 0; while n > 0 { tmp[pos] = b'0' + (n % 10) as u8; n /= 10; pos += 1; } while pos > 0 { pos -= 1; put(core::str::from_utf8(&[tmp[pos]]).unwrap_or("")); } }
fn put_i32(v: i32) { if v < 0 { put("-"); put_u64((-v) as u64); return; } put_u64(v as u64); }
static BANNER: &str = "\r\nHybridOS hyb-linux v0.1 - Subsistema POSIX/Linux de Kernel Compartilhado\r\n";
fn run() {
    put(BANNER);
    put("PID: "); put_u64(sys_getpid() as u64); put("\r\n");
    put("Interface de syscalls Linux (System V AMD64 ABI):\r\n");
    put("  read(0) write(1) open(2) close(3) mmap(9) munmap(11)\r\n");
    put("  sched_yield(24) getpid(39) kill(62) exit(60)\r\n");
    put("  HybridOS: sysinfo(200) pci_enum(201) proc_list(202)\r\n\r\n");
    put("Tabela de processos do kernel (sys_proc_list):\r\n");
    put("PID    Personalidade  Status  Nome\r\n");
    let mut entries = [ProcEntry::default(); 16];
    let n = sys_proc_list(entries.as_mut_ptr(), entries.len() as u64);
    let count = if n < 0 { 0 } else { n as usize };
    if count == 0 { putln("(nenhum processo listado ou syscall indisponível)"); }
    for i in 0..count.min(entries.len()) {
        let e = entries[i];
        let name = core::str::from_utf8(&e.name[..e.name.iter().position(|&b| b == 0).unwrap_or(32)]).unwrap_or("");
        put_u64(e.pid); put("     "); put_u64(e.personality as u64); put("           ");
        put_i32(e.status as i32); put("         "); putln(name);
    }
    put("\nFim da lista de processos.\r\n");
}
#[no_mangle] pub extern "C" fn _start() -> ! { run(); sys_exit(0); }
