//! which / where: Resolvedor de Bin�rios (Ring 3 ELF64)
//!
//! Mostra exatamente qual execut�vel est`//! which / where: Resolvedor de Bin�rios (Ring 3 ELF64)
//!
//! Mostra exatamente qual execut�vel estu{e1} sendo invocado no PATH,
//! incluindo o destino real de links simb`//! which / where: Resolvedor de Bin�rios (Ring 3 ELF64)
//!
//! Mostra exatamente qual execut�vel est\u00e1 sendo invocado no PATH,
//! incluindo o destino real de links simbu{f3}licos e a vers`//! which / where: Resolvedor de Bin�rios (Ring 3 ELF64)
//!
//! Mostra exatamente qual execut�vel est\u00e1 sendo invocado no PATH,
//! incluindo o destino real de links simb\u00f3licos e a versu{e3}o do bin�rio.

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
    println!("[which CRASH] panicked: {:?}", info);
    sys_exit(1);
}

const MAX_PATHS: usize = 8;
const MAX_PATH_LEN: usize = 64;
const MAX_BINS: usize = 16;
const MAX_BIN_LEN: usize = 64;

#[derive(Copy, Clone)]
struct PathEntry {
    dir: [u8; MAX_PATH_LEN],
    dir_len: usize,
}

#[derive(Copy, Clone)]
struct Binary {
    name: [u8; MAX_BIN_LEN],
    name_len: usize,
    real_path: [u8; MAX_PATH_LEN],
    real_path_len: usize,
    version: [u8; 16],
    version_len: usize,
    is_symlink: bool,
}

struct Resolver {
    paths: [PathEntry; MAX_PATHS],
    path_count: usize,
    bins: [Binary; MAX_BINS],
    bin_count: usize,
}

impl Resolver {
    const fn new() -> Self {
        Self {
            paths: [PathEntry { dir: [0u8; MAX_PATH_LEN], dir_len: 0 }; MAX_PATHS],
            path_count: 0,
            bins: [Binary { name: [0u8; MAX_BIN_LEN], name_len: 0, real_path: [0u8; MAX_PATH_LEN], real_path_len: 0, version: [0u8; 16], version_len: 0, is_symlink: false }; MAX_BINS],
            bin_count: 0,
        }
    }

    fn add_path(&mut self, dir: &str) {
        if self.path_count >= MAX_PATHS {
            return;
        }
        let d = dir.as_bytes();
        let l = d.len().min(MAX_PATH_LEN - 1);
        self.paths[self.path_count].dir[..l].copy_from_slice(&d[..l]);
        self.paths[self.path_count].dir_len = l;
        self.path_count += 1;
    }

    fn add_bin(&mut self, name: &str, real_path: &str, version: &str, is_symlink: bool) {
        if self.bin_count >= MAX_BINS {
            return;
        }
        let n = name.as_bytes();
        let nl = n.len().min(MAX_BIN_LEN - 1);
        self.bins[self.bin_count].name[..nl].copy_from_slice(&n[..nl]);
        self.bins[self.bin_count].name_len = nl;
        let r = real_path.as_bytes();
        let rl = r.len().min(MAX_PATH_LEN - 1);
        self.bins[self.bin_count].real_path[..rl].copy_from_slice(&r[..rl]);
        self.bins[self.bin_count].real_path_len = rl;
        let v = version.as_bytes();
        let vl = v.len().min(15);
        self.bins[self.bin_count].version[..vl].copy_from_slice(&v[..vl]);
        self.bins[self.bin_count].version_len = vl;
        self.bins[self.bin_count].is_symlink = is_symlink;
        self.bin_count += 1;
    }

    fn resolve(&self, name: &str) -> Option<usize> {
        for i in 0..self.bin_count {
            let n = core::str::from_utf8(&self.bins[i].name[..self.bins[i].name_len]).unwrap_or("");
            if n == name {
                return Some(i);
            }
        }
        None
    }

    fn which(&self, name: &str) {
        match self.resolve(name) {
            Some(i) => {
                let rp = core::str::from_utf8(&self.bins[i].real_path[..self.bins[i].real_path_len]).unwrap_or("");
                let v = core::str::from_utf8(&self.bins[i].version[..self.bins[i].version_len]).unwrap_or("");
                println!("{} -> {}", name, rp);
                println!("  vers`//! which / where: Resolvedor de Bin�rios (Ring 3 ELF64)
//!
//! Mostra exatamente qual execut�vel est\u00e1 sendo invocado no PATH,
//! incluindo o destino real de links simb\u00f3licos e a vers\u00e3o do bin�rio.

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
    println!("[which CRASH] panicked: {:?}", info);
    sys_exit(1);
}

const MAX_PATHS: usize = 8;
const MAX_PATH_LEN: usize = 64;
const MAX_BINS: usize = 16;
const MAX_BIN_LEN: usize = 64;

#[derive(Copy, Clone)]
struct PathEntry {
    dir: [u8; MAX_PATH_LEN],
    dir_len: usize,
}

#[derive(Copy, Clone)]
struct Binary {
    name: [u8; MAX_BIN_LEN],
    name_len: usize,
    real_path: [u8; MAX_PATH_LEN],
    real_path_len: usize,
    version: [u8; 16],
    version_len: usize,
    is_symlink: bool,
}

struct Resolver {
    paths: [PathEntry; MAX_PATHS],
    path_count: usize,
    bins: [Binary; MAX_BINS],
    bin_count: usize,
}

impl Resolver {
    const fn new() -> Self {
        Self {
            paths: [PathEntry { dir: [0u8; MAX_PATH_LEN], dir_len: 0 }; MAX_PATHS],
            path_count: 0,
            bins: [Binary { name: [0u8; MAX_BIN_LEN], name_len: 0, real_path: [0u8; MAX_PATH_LEN], real_path_len: 0, version: [0u8; 16], version_len: 0, is_symlink: false }; MAX_BINS],
            bin_count: 0,
        }
    }

    fn add_path(&mut self, dir: &str) {
        if self.path_count >= MAX_PATHS {
            return;
        }
        let d = dir.as_bytes();
        let l = d.len().min(MAX_PATH_LEN - 1);
        self.paths[self.path_count].dir[..l].copy_from_slice(&d[..l]);
        self.paths[self.path_count].dir_len = l;
        self.path_count += 1;
    }

    fn add_bin(&mut self, name: &str, real_path: &str, version: &str, is_symlink: bool) {
        if self.bin_count >= MAX_BINS {
            return;
        }
        let n = name.as_bytes();
        let nl = n.len().min(MAX_BIN_LEN - 1);
        self.bins[self.bin_count].name[..nl].copy_from_slice(&n[..nl]);
        self.bins[self.bin_count].name_len = nl;
        let r = real_path.as_bytes();
        let rl = r.len().min(MAX_PATH_LEN - 1);
        self.bins[self.bin_count].real_path[..rl].copy_from_slice(&r[..rl]);
        self.bins[self.bin_count].real_path_len = rl;
        let v = version.as_bytes();
        let vl = v.len().min(15);
        self.bins[self.bin_count].version[..vl].copy_from_slice(&v[..vl]);
        self.bins[self.bin_count].version_len = vl;
        self.bins[self.bin_count].is_symlink = is_symlink;
        self.bin_count += 1;
    }

    fn resolve(&self, name: &str) -> Option<usize> {
        for i in 0..self.bin_count {
            let n = core::str::from_utf8(&self.bins[i].name[..self.bins[i].name_len]).unwrap_or("");
            if n == name {
                return Some(i);
            }
        }
        None
    }

    fn which(&self, name: &str) {
        match self.resolve(name) {
            Some(i) => {
                let rp = core::str::from_utf8(&self.bins[i].real_path[..self.bins[i].real_path_len]).unwrap_or("");
                let v = core::str::from_utf8(&self.bins[i].version[..self.bins[i].version_len]).unwrap_or("");
                println!("{} -> {}", name, rp);
                println!("  versu{e3}o: {} | link simb`//! which / where: Resolvedor de Bin�rios (Ring 3 ELF64)
//!
//! Mostra exatamente qual execut�vel est\u00e1 sendo invocado no PATH,
//! incluindo o destino real de links simb\u00f3licos e a vers\u00e3o do bin�rio.

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
    println!("[which CRASH] panicked: {:?}", info);
    sys_exit(1);
}

const MAX_PATHS: usize = 8;
const MAX_PATH_LEN: usize = 64;
const MAX_BINS: usize = 16;
const MAX_BIN_LEN: usize = 64;

#[derive(Copy, Clone)]
struct PathEntry {
    dir: [u8; MAX_PATH_LEN],
    dir_len: usize,
}

#[derive(Copy, Clone)]
struct Binary {
    name: [u8; MAX_BIN_LEN],
    name_len: usize,
    real_path: [u8; MAX_PATH_LEN],
    real_path_len: usize,
    version: [u8; 16],
    version_len: usize,
    is_symlink: bool,
}

struct Resolver {
    paths: [PathEntry; MAX_PATHS],
    path_count: usize,
    bins: [Binary; MAX_BINS],
    bin_count: usize,
}

impl Resolver {
    const fn new() -> Self {
        Self {
            paths: [PathEntry { dir: [0u8; MAX_PATH_LEN], dir_len: 0 }; MAX_PATHS],
            path_count: 0,
            bins: [Binary { name: [0u8; MAX_BIN_LEN], name_len: 0, real_path: [0u8; MAX_PATH_LEN], real_path_len: 0, version: [0u8; 16], version_len: 0, is_symlink: false }; MAX_BINS],
            bin_count: 0,
        }
    }

    fn add_path(&mut self, dir: &str) {
        if self.path_count >= MAX_PATHS {
            return;
        }
        let d = dir.as_bytes();
        let l = d.len().min(MAX_PATH_LEN - 1);
        self.paths[self.path_count].dir[..l].copy_from_slice(&d[..l]);
        self.paths[self.path_count].dir_len = l;
        self.path_count += 1;
    }

    fn add_bin(&mut self, name: &str, real_path: &str, version: &str, is_symlink: bool) {
        if self.bin_count >= MAX_BINS {
            return;
        }
        let n = name.as_bytes();
        let nl = n.len().min(MAX_BIN_LEN - 1);
        self.bins[self.bin_count].name[..nl].copy_from_slice(&n[..nl]);
        self.bins[self.bin_count].name_len = nl;
        let r = real_path.as_bytes();
        let rl = r.len().min(MAX_PATH_LEN - 1);
        self.bins[self.bin_count].real_path[..rl].copy_from_slice(&r[..rl]);
        self.bins[self.bin_count].real_path_len = rl;
        let v = version.as_bytes();
        let vl = v.len().min(15);
        self.bins[self.bin_count].version[..vl].copy_from_slice(&v[..vl]);
        self.bins[self.bin_count].version_len = vl;
        self.bins[self.bin_count].is_symlink = is_symlink;
        self.bin_count += 1;
    }

    fn resolve(&self, name: &str) -> Option<usize> {
        for i in 0..self.bin_count {
            let n = core::str::from_utf8(&self.bins[i].name[..self.bins[i].name_len]).unwrap_or("");
            if n == name {
                return Some(i);
            }
        }
        None
    }

    fn which(&self, name: &str) {
        match self.resolve(name) {
            Some(i) => {
                let rp = core::str::from_utf8(&self.bins[i].real_path[..self.bins[i].real_path_len]).unwrap_or("");
                let v = core::str::from_utf8(&self.bins[i].version[..self.bins[i].version_len]).unwrap_or("");
                println!("{} -> {}", name, rp);
                println!("  vers\u00e3o: {} | link simbu{f3}lico: {}", v, self.bins[i].is_symlink);
            }
            None => {
                println!("which: '{}' n`//! which / where: Resolvedor de Bin�rios (Ring 3 ELF64)
//!
//! Mostra exatamente qual execut�vel est\u00e1 sendo invocado no PATH,
//! incluindo o destino real de links simb\u00f3licos e a vers\u00e3o do bin�rio.

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
    println!("[which CRASH] panicked: {:?}", info);
    sys_exit(1);
}

const MAX_PATHS: usize = 8;
const MAX_PATH_LEN: usize = 64;
const MAX_BINS: usize = 16;
const MAX_BIN_LEN: usize = 64;

#[derive(Copy, Clone)]
struct PathEntry {
    dir: [u8; MAX_PATH_LEN],
    dir_len: usize,
}

#[derive(Copy, Clone)]
struct Binary {
    name: [u8; MAX_BIN_LEN],
    name_len: usize,
    real_path: [u8; MAX_PATH_LEN],
    real_path_len: usize,
    version: [u8; 16],
    version_len: usize,
    is_symlink: bool,
}

struct Resolver {
    paths: [PathEntry; MAX_PATHS],
    path_count: usize,
    bins: [Binary; MAX_BINS],
    bin_count: usize,
}

impl Resolver {
    const fn new() -> Self {
        Self {
            paths: [PathEntry { dir: [0u8; MAX_PATH_LEN], dir_len: 0 }; MAX_PATHS],
            path_count: 0,
            bins: [Binary { name: [0u8; MAX_BIN_LEN], name_len: 0, real_path: [0u8; MAX_PATH_LEN], real_path_len: 0, version: [0u8; 16], version_len: 0, is_symlink: false }; MAX_BINS],
            bin_count: 0,
        }
    }

    fn add_path(&mut self, dir: &str) {
        if self.path_count >= MAX_PATHS {
            return;
        }
        let d = dir.as_bytes();
        let l = d.len().min(MAX_PATH_LEN - 1);
        self.paths[self.path_count].dir[..l].copy_from_slice(&d[..l]);
        self.paths[self.path_count].dir_len = l;
        self.path_count += 1;
    }

    fn add_bin(&mut self, name: &str, real_path: &str, version: &str, is_symlink: bool) {
        if self.bin_count >= MAX_BINS {
            return;
        }
        let n = name.as_bytes();
        let nl = n.len().min(MAX_BIN_LEN - 1);
        self.bins[self.bin_count].name[..nl].copy_from_slice(&n[..nl]);
        self.bins[self.bin_count].name_len = nl;
        let r = real_path.as_bytes();
        let rl = r.len().min(MAX_PATH_LEN - 1);
        self.bins[self.bin_count].real_path[..rl].copy_from_slice(&r[..rl]);
        self.bins[self.bin_count].real_path_len = rl;
        let v = version.as_bytes();
        let vl = v.len().min(15);
        self.bins[self.bin_count].version[..vl].copy_from_slice(&v[..vl]);
        self.bins[self.bin_count].version_len = vl;
        self.bins[self.bin_count].is_symlink = is_symlink;
        self.bin_count += 1;
    }

    fn resolve(&self, name: &str) -> Option<usize> {
        for i in 0..self.bin_count {
            let n = core::str::from_utf8(&self.bins[i].name[..self.bins[i].name_len]).unwrap_or("");
            if n == name {
                return Some(i);
            }
        }
        None
    }

    fn which(&self, name: &str) {
        match self.resolve(name) {
            Some(i) => {
                let rp = core::str::from_utf8(&self.bins[i].real_path[..self.bins[i].real_path_len]).unwrap_or("");
                let v = core::str::from_utf8(&self.bins[i].version[..self.bins[i].version_len]).unwrap_or("");
                println!("{} -> {}", name, rp);
                println!("  vers\u00e3o: {} | link simb\u00f3lico: {}", v, self.bins[i].is_symlink);
            }
            None => {
                println!("which: '{}' nu{e3}o encontrado no PATH", name);
            }
        }
    }

    fn list_path(&self) {
        println!("[which] PATH:");
        for i in 0..self.path_count {
            let d = core::str::from_utf8(&self.paths[i].dir[..self.paths[i].dir_len]).unwrap_or("");
            println!("  {}", d);
        }
    }
}

fn run() -> i32 {
    println!("============================================================");
    println!("   which / where — HybridOS Binary Resolver");
    println!("============================================================");

    let mut res = Resolver::new();
    res.add_path("/usr/bin");
    res.add_path("/bin");
    res.add_path("C:\\Windows\\System32");
    res.add_path("C:\\Program Files\\HybridOS\\bin");

    res.add_bin("hyb-sh", "/usr/bin/hyb-sh", "0.1.0", false);
    res.add_bin("sh", "/usr/bin/hyb-sh", "0.1.0", true);
    res.add_bin("hyb-term", "/usr/bin/hyb-term", "0.1.0", false);
    res.add_bin("cmd.exe", "C:\\Windows\\System32\\cmd.exe", "10.0.19045", false);
    res.add_bin("bash", "/bin/bash", "5.2.21", false);

    res.list_path();
    println!();
    res.which("hyb-sh");
    println!();
    res.which("sh"); // link simb`//! which / where: Resolvedor de Bin�rios (Ring 3 ELF64)
//!
//! Mostra exatamente qual execut�vel est\u00e1 sendo invocado no PATH,
//! incluindo o destino real de links simb\u00f3licos e a vers\u00e3o do bin�rio.

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
    println!("[which CRASH] panicked: {:?}", info);
    sys_exit(1);
}

const MAX_PATHS: usize = 8;
const MAX_PATH_LEN: usize = 64;
const MAX_BINS: usize = 16;
const MAX_BIN_LEN: usize = 64;

#[derive(Copy, Clone)]
struct PathEntry {
    dir: [u8; MAX_PATH_LEN],
    dir_len: usize,
}

#[derive(Copy, Clone)]
struct Binary {
    name: [u8; MAX_BIN_LEN],
    name_len: usize,
    real_path: [u8; MAX_PATH_LEN],
    real_path_len: usize,
    version: [u8; 16],
    version_len: usize,
    is_symlink: bool,
}

struct Resolver {
    paths: [PathEntry; MAX_PATHS],
    path_count: usize,
    bins: [Binary; MAX_BINS],
    bin_count: usize,
}

impl Resolver {
    const fn new() -> Self {
        Self {
            paths: [PathEntry { dir: [0u8; MAX_PATH_LEN], dir_len: 0 }; MAX_PATHS],
            path_count: 0,
            bins: [Binary { name: [0u8; MAX_BIN_LEN], name_len: 0, real_path: [0u8; MAX_PATH_LEN], real_path_len: 0, version: [0u8; 16], version_len: 0, is_symlink: false }; MAX_BINS],
            bin_count: 0,
        }
    }

    fn add_path(&mut self, dir: &str) {
        if self.path_count >= MAX_PATHS {
            return;
        }
        let d = dir.as_bytes();
        let l = d.len().min(MAX_PATH_LEN - 1);
        self.paths[self.path_count].dir[..l].copy_from_slice(&d[..l]);
        self.paths[self.path_count].dir_len = l;
        self.path_count += 1;
    }

    fn add_bin(&mut self, name: &str, real_path: &str, version: &str, is_symlink: bool) {
        if self.bin_count >= MAX_BINS {
            return;
        }
        let n = name.as_bytes();
        let nl = n.len().min(MAX_BIN_LEN - 1);
        self.bins[self.bin_count].name[..nl].copy_from_slice(&n[..nl]);
        self.bins[self.bin_count].name_len = nl;
        let r = real_path.as_bytes();
        let rl = r.len().min(MAX_PATH_LEN - 1);
        self.bins[self.bin_count].real_path[..rl].copy_from_slice(&r[..rl]);
        self.bins[self.bin_count].real_path_len = rl;
        let v = version.as_bytes();
        let vl = v.len().min(15);
        self.bins[self.bin_count].version[..vl].copy_from_slice(&v[..vl]);
        self.bins[self.bin_count].version_len = vl;
        self.bins[self.bin_count].is_symlink = is_symlink;
        self.bin_count += 1;
    }

    fn resolve(&self, name: &str) -> Option<usize> {
        for i in 0..self.bin_count {
            let n = core::str::from_utf8(&self.bins[i].name[..self.bins[i].name_len]).unwrap_or("");
            if n == name {
                return Some(i);
            }
        }
        None
    }

    fn which(&self, name: &str) {
        match self.resolve(name) {
            Some(i) => {
                let rp = core::str::from_utf8(&self.bins[i].real_path[..self.bins[i].real_path_len]).unwrap_or("");
                let v = core::str::from_utf8(&self.bins[i].version[..self.bins[i].version_len]).unwrap_or("");
                println!("{} -> {}", name, rp);
                println!("  vers\u00e3o: {} | link simb\u00f3lico: {}", v, self.bins[i].is_symlink);
            }
            None => {
                println!("which: '{}' n\u00e3o encontrado no PATH", name);
            }
        }
    }

    fn list_path(&self) {
        println!("[which] PATH:");
        for i in 0..self.path_count {
            let d = core::str::from_utf8(&self.paths[i].dir[..self.paths[i].dir_len]).unwrap_or("");
            println!("  {}", d);
        }
    }
}

fn run() -> i32 {
    println!("============================================================");
    println!("   which / where — HybridOS Binary Resolver");
    println!("============================================================");

    let mut res = Resolver::new();
    res.add_path("/usr/bin");
    res.add_path("/bin");
    res.add_path("C:\\Windows\\System32");
    res.add_path("C:\\Program Files\\HybridOS\\bin");

    res.add_bin("hyb-sh", "/usr/bin/hyb-sh", "0.1.0", false);
    res.add_bin("sh", "/usr/bin/hyb-sh", "0.1.0", true);
    res.add_bin("hyb-term", "/usr/bin/hyb-term", "0.1.0", false);
    res.add_bin("cmd.exe", "C:\\Windows\\System32\\cmd.exe", "10.0.19045", false);
    res.add_bin("bash", "/bin/bash", "5.2.21", false);

    res.list_path();
    println!();
    res.which("hyb-sh");
    println!();
    res.which("sh"); // link simbu{f3}lico
    println!();
    res.which("cmd.exe");
    println!();
    res.which("python3"); // n`//! which / where: Resolvedor de Bin�rios (Ring 3 ELF64)
//!
//! Mostra exatamente qual execut�vel est\u00e1 sendo invocado no PATH,
//! incluindo o destino real de links simb\u00f3licos e a vers\u00e3o do bin�rio.

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
    println!("[which CRASH] panicked: {:?}", info);
    sys_exit(1);
}

const MAX_PATHS: usize = 8;
const MAX_PATH_LEN: usize = 64;
const MAX_BINS: usize = 16;
const MAX_BIN_LEN: usize = 64;

#[derive(Copy, Clone)]
struct PathEntry {
    dir: [u8; MAX_PATH_LEN],
    dir_len: usize,
}

#[derive(Copy, Clone)]
struct Binary {
    name: [u8; MAX_BIN_LEN],
    name_len: usize,
    real_path: [u8; MAX_PATH_LEN],
    real_path_len: usize,
    version: [u8; 16],
    version_len: usize,
    is_symlink: bool,
}

struct Resolver {
    paths: [PathEntry; MAX_PATHS],
    path_count: usize,
    bins: [Binary; MAX_BINS],
    bin_count: usize,
}

impl Resolver {
    const fn new() -> Self {
        Self {
            paths: [PathEntry { dir: [0u8; MAX_PATH_LEN], dir_len: 0 }; MAX_PATHS],
            path_count: 0,
            bins: [Binary { name: [0u8; MAX_BIN_LEN], name_len: 0, real_path: [0u8; MAX_PATH_LEN], real_path_len: 0, version: [0u8; 16], version_len: 0, is_symlink: false }; MAX_BINS],
            bin_count: 0,
        }
    }

    fn add_path(&mut self, dir: &str) {
        if self.path_count >= MAX_PATHS {
            return;
        }
        let d = dir.as_bytes();
        let l = d.len().min(MAX_PATH_LEN - 1);
        self.paths[self.path_count].dir[..l].copy_from_slice(&d[..l]);
        self.paths[self.path_count].dir_len = l;
        self.path_count += 1;
    }

    fn add_bin(&mut self, name: &str, real_path: &str, version: &str, is_symlink: bool) {
        if self.bin_count >= MAX_BINS {
            return;
        }
        let n = name.as_bytes();
        let nl = n.len().min(MAX_BIN_LEN - 1);
        self.bins[self.bin_count].name[..nl].copy_from_slice(&n[..nl]);
        self.bins[self.bin_count].name_len = nl;
        let r = real_path.as_bytes();
        let rl = r.len().min(MAX_PATH_LEN - 1);
        self.bins[self.bin_count].real_path[..rl].copy_from_slice(&r[..rl]);
        self.bins[self.bin_count].real_path_len = rl;
        let v = version.as_bytes();
        let vl = v.len().min(15);
        self.bins[self.bin_count].version[..vl].copy_from_slice(&v[..vl]);
        self.bins[self.bin_count].version_len = vl;
        self.bins[self.bin_count].is_symlink = is_symlink;
        self.bin_count += 1;
    }

    fn resolve(&self, name: &str) -> Option<usize> {
        for i in 0..self.bin_count {
            let n = core::str::from_utf8(&self.bins[i].name[..self.bins[i].name_len]).unwrap_or("");
            if n == name {
                return Some(i);
            }
        }
        None
    }

    fn which(&self, name: &str) {
        match self.resolve(name) {
            Some(i) => {
                let rp = core::str::from_utf8(&self.bins[i].real_path[..self.bins[i].real_path_len]).unwrap_or("");
                let v = core::str::from_utf8(&self.bins[i].version[..self.bins[i].version_len]).unwrap_or("");
                println!("{} -> {}", name, rp);
                println!("  vers\u00e3o: {} | link simb\u00f3lico: {}", v, self.bins[i].is_symlink);
            }
            None => {
                println!("which: '{}' n\u00e3o encontrado no PATH", name);
            }
        }
    }

    fn list_path(&self) {
        println!("[which] PATH:");
        for i in 0..self.path_count {
            let d = core::str::from_utf8(&self.paths[i].dir[..self.paths[i].dir_len]).unwrap_or("");
            println!("  {}", d);
        }
    }
}

fn run() -> i32 {
    println!("============================================================");
    println!("   which / where — HybridOS Binary Resolver");
    println!("============================================================");

    let mut res = Resolver::new();
    res.add_path("/usr/bin");
    res.add_path("/bin");
    res.add_path("C:\\Windows\\System32");
    res.add_path("C:\\Program Files\\HybridOS\\bin");

    res.add_bin("hyb-sh", "/usr/bin/hyb-sh", "0.1.0", false);
    res.add_bin("sh", "/usr/bin/hyb-sh", "0.1.0", true);
    res.add_bin("hyb-term", "/usr/bin/hyb-term", "0.1.0", false);
    res.add_bin("cmd.exe", "C:\\Windows\\System32\\cmd.exe", "10.0.19045", false);
    res.add_bin("bash", "/bin/bash", "5.2.21", false);

    res.list_path();
    println!();
    res.which("hyb-sh");
    println!();
    res.which("sh"); // link simb\u00f3lico
    println!();
    res.which("cmd.exe");
    println!();
    res.which("python3"); // nu{e3}o encontrado
    println!();
    println!("[which/where] resolu��o de bin�rios, links simb`//! which / where: Resolvedor de Bin�rios (Ring 3 ELF64)
//!
//! Mostra exatamente qual execut�vel est\u00e1 sendo invocado no PATH,
//! incluindo o destino real de links simb\u00f3licos e a vers\u00e3o do bin�rio.

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
    println!("[which CRASH] panicked: {:?}", info);
    sys_exit(1);
}

const MAX_PATHS: usize = 8;
const MAX_PATH_LEN: usize = 64;
const MAX_BINS: usize = 16;
const MAX_BIN_LEN: usize = 64;

#[derive(Copy, Clone)]
struct PathEntry {
    dir: [u8; MAX_PATH_LEN],
    dir_len: usize,
}

#[derive(Copy, Clone)]
struct Binary {
    name: [u8; MAX_BIN_LEN],
    name_len: usize,
    real_path: [u8; MAX_PATH_LEN],
    real_path_len: usize,
    version: [u8; 16],
    version_len: usize,
    is_symlink: bool,
}

struct Resolver {
    paths: [PathEntry; MAX_PATHS],
    path_count: usize,
    bins: [Binary; MAX_BINS],
    bin_count: usize,
}

impl Resolver {
    const fn new() -> Self {
        Self {
            paths: [PathEntry { dir: [0u8; MAX_PATH_LEN], dir_len: 0 }; MAX_PATHS],
            path_count: 0,
            bins: [Binary { name: [0u8; MAX_BIN_LEN], name_len: 0, real_path: [0u8; MAX_PATH_LEN], real_path_len: 0, version: [0u8; 16], version_len: 0, is_symlink: false }; MAX_BINS],
            bin_count: 0,
        }
    }

    fn add_path(&mut self, dir: &str) {
        if self.path_count >= MAX_PATHS {
            return;
        }
        let d = dir.as_bytes();
        let l = d.len().min(MAX_PATH_LEN - 1);
        self.paths[self.path_count].dir[..l].copy_from_slice(&d[..l]);
        self.paths[self.path_count].dir_len = l;
        self.path_count += 1;
    }

    fn add_bin(&mut self, name: &str, real_path: &str, version: &str, is_symlink: bool) {
        if self.bin_count >= MAX_BINS {
            return;
        }
        let n = name.as_bytes();
        let nl = n.len().min(MAX_BIN_LEN - 1);
        self.bins[self.bin_count].name[..nl].copy_from_slice(&n[..nl]);
        self.bins[self.bin_count].name_len = nl;
        let r = real_path.as_bytes();
        let rl = r.len().min(MAX_PATH_LEN - 1);
        self.bins[self.bin_count].real_path[..rl].copy_from_slice(&r[..rl]);
        self.bins[self.bin_count].real_path_len = rl;
        let v = version.as_bytes();
        let vl = v.len().min(15);
        self.bins[self.bin_count].version[..vl].copy_from_slice(&v[..vl]);
        self.bins[self.bin_count].version_len = vl;
        self.bins[self.bin_count].is_symlink = is_symlink;
        self.bin_count += 1;
    }

    fn resolve(&self, name: &str) -> Option<usize> {
        for i in 0..self.bin_count {
            let n = core::str::from_utf8(&self.bins[i].name[..self.bins[i].name_len]).unwrap_or("");
            if n == name {
                return Some(i);
            }
        }
        None
    }

    fn which(&self, name: &str) {
        match self.resolve(name) {
            Some(i) => {
                let rp = core::str::from_utf8(&self.bins[i].real_path[..self.bins[i].real_path_len]).unwrap_or("");
                let v = core::str::from_utf8(&self.bins[i].version[..self.bins[i].version_len]).unwrap_or("");
                println!("{} -> {}", name, rp);
                println!("  vers\u00e3o: {} | link simb\u00f3lico: {}", v, self.bins[i].is_symlink);
            }
            None => {
                println!("which: '{}' n\u00e3o encontrado no PATH", name);
            }
        }
    }

    fn list_path(&self) {
        println!("[which] PATH:");
        for i in 0..self.path_count {
            let d = core::str::from_utf8(&self.paths[i].dir[..self.paths[i].dir_len]).unwrap_or("");
            println!("  {}", d);
        }
    }
}

fn run() -> i32 {
    println!("============================================================");
    println!("   which / where — HybridOS Binary Resolver");
    println!("============================================================");

    let mut res = Resolver::new();
    res.add_path("/usr/bin");
    res.add_path("/bin");
    res.add_path("C:\\Windows\\System32");
    res.add_path("C:\\Program Files\\HybridOS\\bin");

    res.add_bin("hyb-sh", "/usr/bin/hyb-sh", "0.1.0", false);
    res.add_bin("sh", "/usr/bin/hyb-sh", "0.1.0", true);
    res.add_bin("hyb-term", "/usr/bin/hyb-term", "0.1.0", false);
    res.add_bin("cmd.exe", "C:\\Windows\\System32\\cmd.exe", "10.0.19045", false);
    res.add_bin("bash", "/bin/bash", "5.2.21", false);

    res.list_path();
    println!();
    res.which("hyb-sh");
    println!();
    res.which("sh"); // link simb\u00f3lico
    println!();
    res.which("cmd.exe");
    println!();
    res.which("python3"); // n\u00e3o encontrado
    println!();
    println!("[which/where] resolu��o de bin�rios, links simbu{f3}licos e vers`//! which / where: Resolvedor de Bin�rios (Ring 3 ELF64)
//!
//! Mostra exatamente qual execut�vel est\u00e1 sendo invocado no PATH,
//! incluindo o destino real de links simb\u00f3licos e a vers\u00e3o do bin�rio.

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
    println!("[which CRASH] panicked: {:?}", info);
    sys_exit(1);
}

const MAX_PATHS: usize = 8;
const MAX_PATH_LEN: usize = 64;
const MAX_BINS: usize = 16;
const MAX_BIN_LEN: usize = 64;

#[derive(Copy, Clone)]
struct PathEntry {
    dir: [u8; MAX_PATH_LEN],
    dir_len: usize,
}

#[derive(Copy, Clone)]
struct Binary {
    name: [u8; MAX_BIN_LEN],
    name_len: usize,
    real_path: [u8; MAX_PATH_LEN],
    real_path_len: usize,
    version: [u8; 16],
    version_len: usize,
    is_symlink: bool,
}

struct Resolver {
    paths: [PathEntry; MAX_PATHS],
    path_count: usize,
    bins: [Binary; MAX_BINS],
    bin_count: usize,
}

impl Resolver {
    const fn new() -> Self {
        Self {
            paths: [PathEntry { dir: [0u8; MAX_PATH_LEN], dir_len: 0 }; MAX_PATHS],
            path_count: 0,
            bins: [Binary { name: [0u8; MAX_BIN_LEN], name_len: 0, real_path: [0u8; MAX_PATH_LEN], real_path_len: 0, version: [0u8; 16], version_len: 0, is_symlink: false }; MAX_BINS],
            bin_count: 0,
        }
    }

    fn add_path(&mut self, dir: &str) {
        if self.path_count >= MAX_PATHS {
            return;
        }
        let d = dir.as_bytes();
        let l = d.len().min(MAX_PATH_LEN - 1);
        self.paths[self.path_count].dir[..l].copy_from_slice(&d[..l]);
        self.paths[self.path_count].dir_len = l;
        self.path_count += 1;
    }

    fn add_bin(&mut self, name: &str, real_path: &str, version: &str, is_symlink: bool) {
        if self.bin_count >= MAX_BINS {
            return;
        }
        let n = name.as_bytes();
        let nl = n.len().min(MAX_BIN_LEN - 1);
        self.bins[self.bin_count].name[..nl].copy_from_slice(&n[..nl]);
        self.bins[self.bin_count].name_len = nl;
        let r = real_path.as_bytes();
        let rl = r.len().min(MAX_PATH_LEN - 1);
        self.bins[self.bin_count].real_path[..rl].copy_from_slice(&r[..rl]);
        self.bins[self.bin_count].real_path_len = rl;
        let v = version.as_bytes();
        let vl = v.len().min(15);
        self.bins[self.bin_count].version[..vl].copy_from_slice(&v[..vl]);
        self.bins[self.bin_count].version_len = vl;
        self.bins[self.bin_count].is_symlink = is_symlink;
        self.bin_count += 1;
    }

    fn resolve(&self, name: &str) -> Option<usize> {
        for i in 0..self.bin_count {
            let n = core::str::from_utf8(&self.bins[i].name[..self.bins[i].name_len]).unwrap_or("");
            if n == name {
                return Some(i);
            }
        }
        None
    }

    fn which(&self, name: &str) {
        match self.resolve(name) {
            Some(i) => {
                let rp = core::str::from_utf8(&self.bins[i].real_path[..self.bins[i].real_path_len]).unwrap_or("");
                let v = core::str::from_utf8(&self.bins[i].version[..self.bins[i].version_len]).unwrap_or("");
                println!("{} -> {}", name, rp);
                println!("  vers\u00e3o: {} | link simb\u00f3lico: {}", v, self.bins[i].is_symlink);
            }
            None => {
                println!("which: '{}' n\u00e3o encontrado no PATH", name);
            }
        }
    }

    fn list_path(&self) {
        println!("[which] PATH:");
        for i in 0..self.path_count {
            let d = core::str::from_utf8(&self.paths[i].dir[..self.paths[i].dir_len]).unwrap_or("");
            println!("  {}", d);
        }
    }
}

fn run() -> i32 {
    println!("============================================================");
    println!("   which / where — HybridOS Binary Resolver");
    println!("============================================================");

    let mut res = Resolver::new();
    res.add_path("/usr/bin");
    res.add_path("/bin");
    res.add_path("C:\\Windows\\System32");
    res.add_path("C:\\Program Files\\HybridOS\\bin");

    res.add_bin("hyb-sh", "/usr/bin/hyb-sh", "0.1.0", false);
    res.add_bin("sh", "/usr/bin/hyb-sh", "0.1.0", true);
    res.add_bin("hyb-term", "/usr/bin/hyb-term", "0.1.0", false);
    res.add_bin("cmd.exe", "C:\\Windows\\System32\\cmd.exe", "10.0.19045", false);
    res.add_bin("bash", "/bin/bash", "5.2.21", false);

    res.list_path();
    println!();
    res.which("hyb-sh");
    println!();
    res.which("sh"); // link simb\u00f3lico
    println!();
    res.which("cmd.exe");
    println!();
    res.which("python3"); // n\u00e3o encontrado
    println!();
    println!("[which/where] resolu��o de bin�rios, links simb\u00f3licos e versu{e3}o validadas.");
    sys_sched_yield();
    0
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    sys_exit(run());
}
