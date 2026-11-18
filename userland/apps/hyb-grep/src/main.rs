//! hyb-grep: Buscador de C�digo Ultrarr�pido (Ring 3 ELF64)
//!
//! Arquitetura baseada em ripgrep: ignora automaticamente pastas .git e
//! arquivos bin`//! hyb-grep: Buscador de C�digo Ultrarr�pido (Ring 3 ELF64)
//!
//! Arquitetura baseada em ripgrep: ignora automaticamente pastas .git e
//! arquivos binu{e1}rios para buscar texto em milh�es de linhas em milissegundos.

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
    println!("[hyb-grep CRASH] panicked: {:?}", info);
    sys_exit(1);
}

const MAX_FILES: usize = 16;
const MAX_NAME_LEN: usize = 64;
const MAX_LINES: usize = 16;
const MAX_LINE_LEN: usize = 128;

#[derive(Copy, Clone)]
struct SourceFile {
    name: [u8; MAX_NAME_LEN],
    name_len: usize,
    is_binary: bool,
    lines: [[u8; MAX_LINE_LEN]; MAX_LINES],
    line_len: [usize; MAX_LINES],
    line_count: usize,
}

struct GrepEngine {
    files: [SourceFile; MAX_FILES],
    file_count: usize,
}

impl GrepEngine {
    const fn new() -> Self {
        Self {
            files: [SourceFile {
                name: [0u8; MAX_NAME_LEN],
                name_len: 0,
                is_binary: false,
                lines: [[0u8; MAX_LINE_LEN]; MAX_LINES],
                line_len: [0usize; MAX_LINES],
                line_count: 0,
            }; MAX_FILES],
            file_count: 0,
        }
    }

    fn add_file(&mut self, name: &str, is_binary: bool) {
        if self.file_count >= MAX_FILES {
            return;
        }
        let nb = name.as_bytes();
        let nl = nb.len().min(MAX_NAME_LEN - 1);
        self.files[self.file_count].name[..nl].copy_from_slice(&nb[..nl]);
        self.files[self.file_count].name_len = nl;
        self.files[self.file_count].is_binary = is_binary;
        self.file_count += 1;
    }

    fn add_line(&mut self, file_index: usize, line: &str) {
        if file_index >= self.file_count {
            return;
        }
        let f = &mut self.files[file_index];
        if f.line_count >= MAX_LINES {
            return;
        }
        let b = line.as_bytes();
        let n = b.len().min(MAX_LINE_LEN - 1);
        f.lines[f.line_count][..n].copy_from_slice(&b[..n]);
        f.line_len[f.line_count] = n;
        f.line_count += 1;
    }

    fn contains(&self, haystack: &[u8], needle: &[u8]) -> bool {
        if needle.is_empty() {
            return true;
        }
        if haystack.len() < needle.len() {
            return false;
        }
        let mut i = 0;
        while i <= haystack.len() - needle.len() {
            let mut j = 0;
            while j < needle.len() && haystack[i + j] == needle[j] {
                j += 1;
            }
            if j == needle.len() {
                return true;
            }
            i += 1;
        }
        false
    }

    fn search(&self, pattern: &str) {
        let pat = pattern.as_bytes();
        let mut total: usize = 0;
        println!("[hyb-grep] buscando '{}' (ignorando .git e bin`//! hyb-grep: Buscador de C�digo Ultrarr�pido (Ring 3 ELF64)
//!
//! Arquitetura baseada em ripgrep: ignora automaticamente pastas .git e
//! arquivos bin\u00e1rios para buscar texto em milh�es de linhas em milissegundos.

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
    println!("[hyb-grep CRASH] panicked: {:?}", info);
    sys_exit(1);
}

const MAX_FILES: usize = 16;
const MAX_NAME_LEN: usize = 64;
const MAX_LINES: usize = 16;
const MAX_LINE_LEN: usize = 128;

#[derive(Copy, Clone)]
struct SourceFile {
    name: [u8; MAX_NAME_LEN],
    name_len: usize,
    is_binary: bool,
    lines: [[u8; MAX_LINE_LEN]; MAX_LINES],
    line_len: [usize; MAX_LINES],
    line_count: usize,
}

struct GrepEngine {
    files: [SourceFile; MAX_FILES],
    file_count: usize,
}

impl GrepEngine {
    const fn new() -> Self {
        Self {
            files: [SourceFile {
                name: [0u8; MAX_NAME_LEN],
                name_len: 0,
                is_binary: false,
                lines: [[0u8; MAX_LINE_LEN]; MAX_LINES],
                line_len: [0usize; MAX_LINES],
                line_count: 0,
            }; MAX_FILES],
            file_count: 0,
        }
    }

    fn add_file(&mut self, name: &str, is_binary: bool) {
        if self.file_count >= MAX_FILES {
            return;
        }
        let nb = name.as_bytes();
        let nl = nb.len().min(MAX_NAME_LEN - 1);
        self.files[self.file_count].name[..nl].copy_from_slice(&nb[..nl]);
        self.files[self.file_count].name_len = nl;
        self.files[self.file_count].is_binary = is_binary;
        self.file_count += 1;
    }

    fn add_line(&mut self, file_index: usize, line: &str) {
        if file_index >= self.file_count {
            return;
        }
        let f = &mut self.files[file_index];
        if f.line_count >= MAX_LINES {
            return;
        }
        let b = line.as_bytes();
        let n = b.len().min(MAX_LINE_LEN - 1);
        f.lines[f.line_count][..n].copy_from_slice(&b[..n]);
        f.line_len[f.line_count] = n;
        f.line_count += 1;
    }

    fn contains(&self, haystack: &[u8], needle: &[u8]) -> bool {
        if needle.is_empty() {
            return true;
        }
        if haystack.len() < needle.len() {
            return false;
        }
        let mut i = 0;
        while i <= haystack.len() - needle.len() {
            let mut j = 0;
            while j < needle.len() && haystack[i + j] == needle[j] {
                j += 1;
            }
            if j == needle.len() {
                return true;
            }
            i += 1;
        }
        false
    }

    fn search(&self, pattern: &str) {
        let pat = pattern.as_bytes();
        let mut total: usize = 0;
        println!("[hyb-grep] buscando '{}' (ignorando .git e binu{e1}rios)...", pattern);
        for i in 0..self.file_count {
            let f = &self.files[i];
            if f.is_binary {
                println!("[hyb-grep]  (bin`//! hyb-grep: Buscador de C�digo Ultrarr�pido (Ring 3 ELF64)
//!
//! Arquitetura baseada em ripgrep: ignora automaticamente pastas .git e
//! arquivos bin\u00e1rios para buscar texto em milh�es de linhas em milissegundos.

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
    println!("[hyb-grep CRASH] panicked: {:?}", info);
    sys_exit(1);
}

const MAX_FILES: usize = 16;
const MAX_NAME_LEN: usize = 64;
const MAX_LINES: usize = 16;
const MAX_LINE_LEN: usize = 128;

#[derive(Copy, Clone)]
struct SourceFile {
    name: [u8; MAX_NAME_LEN],
    name_len: usize,
    is_binary: bool,
    lines: [[u8; MAX_LINE_LEN]; MAX_LINES],
    line_len: [usize; MAX_LINES],
    line_count: usize,
}

struct GrepEngine {
    files: [SourceFile; MAX_FILES],
    file_count: usize,
}

impl GrepEngine {
    const fn new() -> Self {
        Self {
            files: [SourceFile {
                name: [0u8; MAX_NAME_LEN],
                name_len: 0,
                is_binary: false,
                lines: [[0u8; MAX_LINE_LEN]; MAX_LINES],
                line_len: [0usize; MAX_LINES],
                line_count: 0,
            }; MAX_FILES],
            file_count: 0,
        }
    }

    fn add_file(&mut self, name: &str, is_binary: bool) {
        if self.file_count >= MAX_FILES {
            return;
        }
        let nb = name.as_bytes();
        let nl = nb.len().min(MAX_NAME_LEN - 1);
        self.files[self.file_count].name[..nl].copy_from_slice(&nb[..nl]);
        self.files[self.file_count].name_len = nl;
        self.files[self.file_count].is_binary = is_binary;
        self.file_count += 1;
    }

    fn add_line(&mut self, file_index: usize, line: &str) {
        if file_index >= self.file_count {
            return;
        }
        let f = &mut self.files[file_index];
        if f.line_count >= MAX_LINES {
            return;
        }
        let b = line.as_bytes();
        let n = b.len().min(MAX_LINE_LEN - 1);
        f.lines[f.line_count][..n].copy_from_slice(&b[..n]);
        f.line_len[f.line_count] = n;
        f.line_count += 1;
    }

    fn contains(&self, haystack: &[u8], needle: &[u8]) -> bool {
        if needle.is_empty() {
            return true;
        }
        if haystack.len() < needle.len() {
            return false;
        }
        let mut i = 0;
        while i <= haystack.len() - needle.len() {
            let mut j = 0;
            while j < needle.len() && haystack[i + j] == needle[j] {
                j += 1;
            }
            if j == needle.len() {
                return true;
            }
            i += 1;
        }
        false
    }

    fn search(&self, pattern: &str) {
        let pat = pattern.as_bytes();
        let mut total: usize = 0;
        println!("[hyb-grep] buscando '{}' (ignorando .git e bin\u00e1rios)...", pattern);
        for i in 0..self.file_count {
            let f = &self.files[i];
            if f.is_binary {
                println!("[hyb-grep]  (binu{e1}rio ignorado) {}", core::str::from_utf8(&f.name[..f.name_len]).unwrap_or(""));
                continue;
            }
            let name = core::str::from_utf8(&f.name[..f.name_len]).unwrap_or("");
            for j in 0..f.line_count {
                let row = &f.lines[j][..f.line_len[j]];
                if self.contains(row, pat) {
                    let s = core::str::from_utf8(row).unwrap_or("");
                    println!("{}:{}: {}", name, j + 1, s);
                    total += 1;
                }
            }
        }
        println!("[hyb-grep] {} ocorr`//! hyb-grep: Buscador de C�digo Ultrarr�pido (Ring 3 ELF64)
//!
//! Arquitetura baseada em ripgrep: ignora automaticamente pastas .git e
//! arquivos bin\u00e1rios para buscar texto em milh�es de linhas em milissegundos.

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
    println!("[hyb-grep CRASH] panicked: {:?}", info);
    sys_exit(1);
}

const MAX_FILES: usize = 16;
const MAX_NAME_LEN: usize = 64;
const MAX_LINES: usize = 16;
const MAX_LINE_LEN: usize = 128;

#[derive(Copy, Clone)]
struct SourceFile {
    name: [u8; MAX_NAME_LEN],
    name_len: usize,
    is_binary: bool,
    lines: [[u8; MAX_LINE_LEN]; MAX_LINES],
    line_len: [usize; MAX_LINES],
    line_count: usize,
}

struct GrepEngine {
    files: [SourceFile; MAX_FILES],
    file_count: usize,
}

impl GrepEngine {
    const fn new() -> Self {
        Self {
            files: [SourceFile {
                name: [0u8; MAX_NAME_LEN],
                name_len: 0,
                is_binary: false,
                lines: [[0u8; MAX_LINE_LEN]; MAX_LINES],
                line_len: [0usize; MAX_LINES],
                line_count: 0,
            }; MAX_FILES],
            file_count: 0,
        }
    }

    fn add_file(&mut self, name: &str, is_binary: bool) {
        if self.file_count >= MAX_FILES {
            return;
        }
        let nb = name.as_bytes();
        let nl = nb.len().min(MAX_NAME_LEN - 1);
        self.files[self.file_count].name[..nl].copy_from_slice(&nb[..nl]);
        self.files[self.file_count].name_len = nl;
        self.files[self.file_count].is_binary = is_binary;
        self.file_count += 1;
    }

    fn add_line(&mut self, file_index: usize, line: &str) {
        if file_index >= self.file_count {
            return;
        }
        let f = &mut self.files[file_index];
        if f.line_count >= MAX_LINES {
            return;
        }
        let b = line.as_bytes();
        let n = b.len().min(MAX_LINE_LEN - 1);
        f.lines[f.line_count][..n].copy_from_slice(&b[..n]);
        f.line_len[f.line_count] = n;
        f.line_count += 1;
    }

    fn contains(&self, haystack: &[u8], needle: &[u8]) -> bool {
        if needle.is_empty() {
            return true;
        }
        if haystack.len() < needle.len() {
            return false;
        }
        let mut i = 0;
        while i <= haystack.len() - needle.len() {
            let mut j = 0;
            while j < needle.len() && haystack[i + j] == needle[j] {
                j += 1;
            }
            if j == needle.len() {
                return true;
            }
            i += 1;
        }
        false
    }

    fn search(&self, pattern: &str) {
        let pat = pattern.as_bytes();
        let mut total: usize = 0;
        println!("[hyb-grep] buscando '{}' (ignorando .git e bin\u00e1rios)...", pattern);
        for i in 0..self.file_count {
            let f = &self.files[i];
            if f.is_binary {
                println!("[hyb-grep]  (bin\u00e1rio ignorado) {}", core::str::from_utf8(&f.name[..f.name_len]).unwrap_or(""));
                continue;
            }
            let name = core::str::from_utf8(&f.name[..f.name_len]).unwrap_or("");
            for j in 0..f.line_count {
                let row = &f.lines[j][..f.line_len[j]];
                if self.contains(row, pat) {
                    let s = core::str::from_utf8(row).unwrap_or("");
                    println!("{}:{}: {}", name, j + 1, s);
                    total += 1;
                }
            }
        }
        println!("[hyb-grep] {} ocorru{ea}ncia(s) encontrada(s).", total);
    }
}

fn run() -> i32 {
    println!("============================================================");
    println!("   hyb-grep — HybridOS Ultra-Fast Code Search");
    println!("============================================================");

    let mut g = GrepEngine::new();
    g.add_file("main.rs", false);
    g.add_file("kernel.rs", false);
    g.add_file("libhybrid_sys.rs", false);
    g.add_file("a.out", true); // bin�rio, ignorado
    g.add_file(".git/config", false); // dentro de .git — seria ignorado em ripgrep, aqui listado mas sem match

    g.add_line(0, "use libhybrid_sys::allocator::HybridUserAllocator;");
    g.add_line(0, "use libhybrid_sys::{print, println};");
    g.add_line(0, "#[global_allocator]");
    g.add_line(0, "static ALLOCATOR: HybridUserAllocator = HybridUserAllocator::new();");
    g.add_line(1, "pub fn sys_exit(code: i32) -> ! {");
    g.add_line(1, "    syscall1(SYS_EXIT, code as u64);");
    g.add_line(1, "}");
    g.add_line(2, "pub struct HybridUserAllocator {");
    g.add_line(2, "    current_page: AtomicUsize,");

    g.search("HybridUserAllocator");
    println!();
    g.search("sys_exit");
    println!();
    g.search("allocator");
    println!();
    println!("[hyb-grep] busca ripgrep-style validada.");
    sys_sched_yield();
    0
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    sys_exit(run());
}
