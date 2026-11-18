//! hyb-cronctl: CLI do Agendador (Ring 3 ELF64)
//!
//! Interface de linha de comando para listar, pausar, disparar e ver logs de
//! sa�da (stdout/stderr) de qualquer tarefa agendada.

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
    println!("[hyb-cronctl CRASH] panicked: {:?}", info);
    sys_exit(1);
}

const MAX_JOBS: usize = 8;
const MAX_NAME_LEN: usize = 48;
const MAX_CMD_LEN: usize = 96;
const MAX_LOG_LEN: usize = 96;
const MAX_LOGS: usize = 8;

#[derive(Copy, Clone)]
struct CronJob {
    name: [u8; MAX_NAME_LEN],
    name_len: usize,
    cmd: [u8; MAX_CMD_LEN],
    cmd_len: usize,
    enabled: bool,
    run_count: u32,
}

#[derive(Copy, Clone)]
struct JobLog {
    job: [u8; MAX_NAME_LEN],
    job_len: usize,
    line: [u8; MAX_LOG_LEN],
    line_len: usize,
}

struct CronCtl {
    jobs: [CronJob; MAX_JOBS],
    job_count: usize,
    logs: [JobLog; MAX_LOGS],
    log_count: usize,
}

impl CronCtl {
    const fn new() -> Self {
        Self {
            jobs: [CronJob { name: [0u8; MAX_NAME_LEN], name_len: 0, cmd: [0u8; MAX_CMD_LEN], cmd_len: 0, enabled: false, run_count: 0 }; MAX_JOBS],
            job_count: 0,
            logs: [JobLog { job: [0u8; MAX_NAME_LEN], job_len: 0, line: [0u8; MAX_LOG_LEN], line_len: 0 }; MAX_LOGS],
            log_count: 0,
        }
    }

    fn add_job(&mut self, name: &str, cmd: &str) {
        if self.job_count >= MAX_JOBS {
            return;
        }
        let nb = name.as_bytes();
        let nl = nb.len().min(MAX_NAME_LEN - 1);
        self.jobs[self.job_count].name[..nl].copy_from_slice(&nb[..nl]);
        self.jobs[self.job_count].name_len = nl;
        let cb = cmd.as_bytes();
        let cl = cb.len().min(MAX_CMD_LEN - 1);
        self.jobs[self.job_count].cmd[..cl].copy_from_slice(&cb[..cl]);
        self.jobs[self.job_count].cmd_len = cl;
        self.jobs[self.job_count].enabled = true;
        self.jobs[self.job_count].run_count = 0;
        self.job_count += 1;
        println!("[hyb-cronctl] job registrado: '{}'", name);
    }

    fn find(&self, name: &str) -> Option<usize> {
        for i in 0..self.job_count {
            let n = core::str::from_utf8(&self.jobs[i].name[..self.jobs[i].name_len]).unwrap_or("");
            if n == name {
                return Some(i);
            }
        }
        None
    }

    fn list(&self) {
        println!("[hyb-cronctl] tarefas agendadas:");
        if self.job_count == 0 {
            println!("  (nenhuma)");
        }
        for i in 0..self.job_count {
            let nm = core::str::from_utf8(&self.jobs[i].name[..self.jobs[i].name_len]).unwrap_or("");
            let cd = core::str::from_utf8(&self.jobs[i].cmd[..self.jobs[i].cmd_len]).unwrap_or("");
            println!("  {} | cmd={} | runs={} | enabled={}", nm, cd, self.jobs[i].run_count, self.jobs[i].enabled);
        }
    }

    fn pause(&mut self, name: &str) {
        match self.find(name) {
            Some(i) => {
                self.jobs[i].enabled = false;
                println!("[hyb-cronctl] job '{}' pausado.", name);
            }
            None => println!("[hyb-cronctl] job n`//! hyb-cronctl: CLI do Agendador (Ring 3 ELF64)
//!
//! Interface de linha de comando para listar, pausar, disparar e ver logs de
//! sa�da (stdout/stderr) de qualquer tarefa agendada.

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
    println!("[hyb-cronctl CRASH] panicked: {:?}", info);
    sys_exit(1);
}

const MAX_JOBS: usize = 8;
const MAX_NAME_LEN: usize = 48;
const MAX_CMD_LEN: usize = 96;
const MAX_LOG_LEN: usize = 96;
const MAX_LOGS: usize = 8;

#[derive(Copy, Clone)]
struct CronJob {
    name: [u8; MAX_NAME_LEN],
    name_len: usize,
    cmd: [u8; MAX_CMD_LEN],
    cmd_len: usize,
    enabled: bool,
    run_count: u32,
}

#[derive(Copy, Clone)]
struct JobLog {
    job: [u8; MAX_NAME_LEN],
    job_len: usize,
    line: [u8; MAX_LOG_LEN],
    line_len: usize,
}

struct CronCtl {
    jobs: [CronJob; MAX_JOBS],
    job_count: usize,
    logs: [JobLog; MAX_LOGS],
    log_count: usize,
}

impl CronCtl {
    const fn new() -> Self {
        Self {
            jobs: [CronJob { name: [0u8; MAX_NAME_LEN], name_len: 0, cmd: [0u8; MAX_CMD_LEN], cmd_len: 0, enabled: false, run_count: 0 }; MAX_JOBS],
            job_count: 0,
            logs: [JobLog { job: [0u8; MAX_NAME_LEN], job_len: 0, line: [0u8; MAX_LOG_LEN], line_len: 0 }; MAX_LOGS],
            log_count: 0,
        }
    }

    fn add_job(&mut self, name: &str, cmd: &str) {
        if self.job_count >= MAX_JOBS {
            return;
        }
        let nb = name.as_bytes();
        let nl = nb.len().min(MAX_NAME_LEN - 1);
        self.jobs[self.job_count].name[..nl].copy_from_slice(&nb[..nl]);
        self.jobs[self.job_count].name_len = nl;
        let cb = cmd.as_bytes();
        let cl = cb.len().min(MAX_CMD_LEN - 1);
        self.jobs[self.job_count].cmd[..cl].copy_from_slice(&cb[..cl]);
        self.jobs[self.job_count].cmd_len = cl;
        self.jobs[self.job_count].enabled = true;
        self.jobs[self.job_count].run_count = 0;
        self.job_count += 1;
        println!("[hyb-cronctl] job registrado: '{}'", name);
    }

    fn find(&self, name: &str) -> Option<usize> {
        for i in 0..self.job_count {
            let n = core::str::from_utf8(&self.jobs[i].name[..self.jobs[i].name_len]).unwrap_or("");
            if n == name {
                return Some(i);
            }
        }
        None
    }

    fn list(&self) {
        println!("[hyb-cronctl] tarefas agendadas:");
        if self.job_count == 0 {
            println!("  (nenhuma)");
        }
        for i in 0..self.job_count {
            let nm = core::str::from_utf8(&self.jobs[i].name[..self.jobs[i].name_len]).unwrap_or("");
            let cd = core::str::from_utf8(&self.jobs[i].cmd[..self.jobs[i].cmd_len]).unwrap_or("");
            println!("  {} | cmd={} | runs={} | enabled={}", nm, cd, self.jobs[i].run_count, self.jobs[i].enabled);
        }
    }

    fn pause(&mut self, name: &str) {
        match self.find(name) {
            Some(i) => {
                self.jobs[i].enabled = false;
                println!("[hyb-cronctl] job '{}' pausado.", name);
            }
            None => println!("[hyb-cronctl] job nu{e3}o encontrado: {}", name),
        }
    }

    fn resume(&mut self, name: &str) {
        match self.find(name) {
            Some(i) => {
                self.jobs[i].enabled = true;
                println!("[hyb-cronctl] job '{}' retomado.", name);
            }
            None => println!("[hyb-cronctl] job n`//! hyb-cronctl: CLI do Agendador (Ring 3 ELF64)
//!
//! Interface de linha de comando para listar, pausar, disparar e ver logs de
//! sa�da (stdout/stderr) de qualquer tarefa agendada.

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
    println!("[hyb-cronctl CRASH] panicked: {:?}", info);
    sys_exit(1);
}

const MAX_JOBS: usize = 8;
const MAX_NAME_LEN: usize = 48;
const MAX_CMD_LEN: usize = 96;
const MAX_LOG_LEN: usize = 96;
const MAX_LOGS: usize = 8;

#[derive(Copy, Clone)]
struct CronJob {
    name: [u8; MAX_NAME_LEN],
    name_len: usize,
    cmd: [u8; MAX_CMD_LEN],
    cmd_len: usize,
    enabled: bool,
    run_count: u32,
}

#[derive(Copy, Clone)]
struct JobLog {
    job: [u8; MAX_NAME_LEN],
    job_len: usize,
    line: [u8; MAX_LOG_LEN],
    line_len: usize,
}

struct CronCtl {
    jobs: [CronJob; MAX_JOBS],
    job_count: usize,
    logs: [JobLog; MAX_LOGS],
    log_count: usize,
}

impl CronCtl {
    const fn new() -> Self {
        Self {
            jobs: [CronJob { name: [0u8; MAX_NAME_LEN], name_len: 0, cmd: [0u8; MAX_CMD_LEN], cmd_len: 0, enabled: false, run_count: 0 }; MAX_JOBS],
            job_count: 0,
            logs: [JobLog { job: [0u8; MAX_NAME_LEN], job_len: 0, line: [0u8; MAX_LOG_LEN], line_len: 0 }; MAX_LOGS],
            log_count: 0,
        }
    }

    fn add_job(&mut self, name: &str, cmd: &str) {
        if self.job_count >= MAX_JOBS {
            return;
        }
        let nb = name.as_bytes();
        let nl = nb.len().min(MAX_NAME_LEN - 1);
        self.jobs[self.job_count].name[..nl].copy_from_slice(&nb[..nl]);
        self.jobs[self.job_count].name_len = nl;
        let cb = cmd.as_bytes();
        let cl = cb.len().min(MAX_CMD_LEN - 1);
        self.jobs[self.job_count].cmd[..cl].copy_from_slice(&cb[..cl]);
        self.jobs[self.job_count].cmd_len = cl;
        self.jobs[self.job_count].enabled = true;
        self.jobs[self.job_count].run_count = 0;
        self.job_count += 1;
        println!("[hyb-cronctl] job registrado: '{}'", name);
    }

    fn find(&self, name: &str) -> Option<usize> {
        for i in 0..self.job_count {
            let n = core::str::from_utf8(&self.jobs[i].name[..self.jobs[i].name_len]).unwrap_or("");
            if n == name {
                return Some(i);
            }
        }
        None
    }

    fn list(&self) {
        println!("[hyb-cronctl] tarefas agendadas:");
        if self.job_count == 0 {
            println!("  (nenhuma)");
        }
        for i in 0..self.job_count {
            let nm = core::str::from_utf8(&self.jobs[i].name[..self.jobs[i].name_len]).unwrap_or("");
            let cd = core::str::from_utf8(&self.jobs[i].cmd[..self.jobs[i].cmd_len]).unwrap_or("");
            println!("  {} | cmd={} | runs={} | enabled={}", nm, cd, self.jobs[i].run_count, self.jobs[i].enabled);
        }
    }

    fn pause(&mut self, name: &str) {
        match self.find(name) {
            Some(i) => {
                self.jobs[i].enabled = false;
                println!("[hyb-cronctl] job '{}' pausado.", name);
            }
            None => println!("[hyb-cronctl] job n\u00e3o encontrado: {}", name),
        }
    }

    fn resume(&mut self, name: &str) {
        match self.find(name) {
            Some(i) => {
                self.jobs[i].enabled = true;
                println!("[hyb-cronctl] job '{}' retomado.", name);
            }
            None => println!("[hyb-cronctl] job nu{e3}o encontrado: {}", name),
        }
    }

    fn trigger(&mut self, name: &str) {
        match self.find(name) {
            Some(i) => {
                self.jobs[i].run_count += 1;
                let cd = core::str::from_utf8(&self.jobs[i].cmd[..self.jobs[i].cmd_len]).unwrap_or("");
                println!("[hyb-cronctl] disparando '{}' -> {}", name, cd);
                self.add_log(name, "stdout: job executado (simulado)");
            }
            None => println!("[hyb-cronctl] job n`//! hyb-cronctl: CLI do Agendador (Ring 3 ELF64)
//!
//! Interface de linha de comando para listar, pausar, disparar e ver logs de
//! sa�da (stdout/stderr) de qualquer tarefa agendada.

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
    println!("[hyb-cronctl CRASH] panicked: {:?}", info);
    sys_exit(1);
}

const MAX_JOBS: usize = 8;
const MAX_NAME_LEN: usize = 48;
const MAX_CMD_LEN: usize = 96;
const MAX_LOG_LEN: usize = 96;
const MAX_LOGS: usize = 8;

#[derive(Copy, Clone)]
struct CronJob {
    name: [u8; MAX_NAME_LEN],
    name_len: usize,
    cmd: [u8; MAX_CMD_LEN],
    cmd_len: usize,
    enabled: bool,
    run_count: u32,
}

#[derive(Copy, Clone)]
struct JobLog {
    job: [u8; MAX_NAME_LEN],
    job_len: usize,
    line: [u8; MAX_LOG_LEN],
    line_len: usize,
}

struct CronCtl {
    jobs: [CronJob; MAX_JOBS],
    job_count: usize,
    logs: [JobLog; MAX_LOGS],
    log_count: usize,
}

impl CronCtl {
    const fn new() -> Self {
        Self {
            jobs: [CronJob { name: [0u8; MAX_NAME_LEN], name_len: 0, cmd: [0u8; MAX_CMD_LEN], cmd_len: 0, enabled: false, run_count: 0 }; MAX_JOBS],
            job_count: 0,
            logs: [JobLog { job: [0u8; MAX_NAME_LEN], job_len: 0, line: [0u8; MAX_LOG_LEN], line_len: 0 }; MAX_LOGS],
            log_count: 0,
        }
    }

    fn add_job(&mut self, name: &str, cmd: &str) {
        if self.job_count >= MAX_JOBS {
            return;
        }
        let nb = name.as_bytes();
        let nl = nb.len().min(MAX_NAME_LEN - 1);
        self.jobs[self.job_count].name[..nl].copy_from_slice(&nb[..nl]);
        self.jobs[self.job_count].name_len = nl;
        let cb = cmd.as_bytes();
        let cl = cb.len().min(MAX_CMD_LEN - 1);
        self.jobs[self.job_count].cmd[..cl].copy_from_slice(&cb[..cl]);
        self.jobs[self.job_count].cmd_len = cl;
        self.jobs[self.job_count].enabled = true;
        self.jobs[self.job_count].run_count = 0;
        self.job_count += 1;
        println!("[hyb-cronctl] job registrado: '{}'", name);
    }

    fn find(&self, name: &str) -> Option<usize> {
        for i in 0..self.job_count {
            let n = core::str::from_utf8(&self.jobs[i].name[..self.jobs[i].name_len]).unwrap_or("");
            if n == name {
                return Some(i);
            }
        }
        None
    }

    fn list(&self) {
        println!("[hyb-cronctl] tarefas agendadas:");
        if self.job_count == 0 {
            println!("  (nenhuma)");
        }
        for i in 0..self.job_count {
            let nm = core::str::from_utf8(&self.jobs[i].name[..self.jobs[i].name_len]).unwrap_or("");
            let cd = core::str::from_utf8(&self.jobs[i].cmd[..self.jobs[i].cmd_len]).unwrap_or("");
            println!("  {} | cmd={} | runs={} | enabled={}", nm, cd, self.jobs[i].run_count, self.jobs[i].enabled);
        }
    }

    fn pause(&mut self, name: &str) {
        match self.find(name) {
            Some(i) => {
                self.jobs[i].enabled = false;
                println!("[hyb-cronctl] job '{}' pausado.", name);
            }
            None => println!("[hyb-cronctl] job n\u00e3o encontrado: {}", name),
        }
    }

    fn resume(&mut self, name: &str) {
        match self.find(name) {
            Some(i) => {
                self.jobs[i].enabled = true;
                println!("[hyb-cronctl] job '{}' retomado.", name);
            }
            None => println!("[hyb-cronctl] job n\u00e3o encontrado: {}", name),
        }
    }

    fn trigger(&mut self, name: &str) {
        match self.find(name) {
            Some(i) => {
                self.jobs[i].run_count += 1;
                let cd = core::str::from_utf8(&self.jobs[i].cmd[..self.jobs[i].cmd_len]).unwrap_or("");
                println!("[hyb-cronctl] disparando '{}' -> {}", name, cd);
                self.add_log(name, "stdout: job executado (simulado)");
            }
            None => println!("[hyb-cronctl] job nu{e3}o encontrado: {}", name),
        }
    }

    fn logs(&self, name: &str) {
        println!("[hyb-cronctl] logs de sa�da (stdout/stderr) para '{}':", name);
        let mut shown: usize = 0;
        for i in 0..self.log_count {
            let j = core::str::from_utf8(&self.logs[i].job[..self.logs[i].job_len]).unwrap_or("");
            if j == name {
                let l = core::str::from_utf8(&self.logs[i].line[..self.logs[i].line_len]).unwrap_or("");
                println!("  {}", l);
                shown += 1;
            }
        }
        if shown == 0 {
            println!("  (sem logs)");
        }
    }

    fn add_log(&mut self, job: &str, line: &str) {
        if self.log_count >= MAX_LOGS {
            return;
        }
        let jb = job.as_bytes();
        let jl = jb.len().min(MAX_NAME_LEN - 1);
        self.logs[self.log_count].job[..jl].copy_from_slice(&jb[..jl]);
        self.logs[self.log_count].job_len = jl;
        let lb = line.as_bytes();
        let ll = lb.len().min(MAX_LOG_LEN - 1);
        self.logs[self.log_count].line[..ll].copy_from_slice(&lb[..ll]);
        self.logs[self.log_count].line_len = ll;
        self.log_count += 1;
    }
}

// Formata uma string de log est�tica (sem aloca��o).
fn format(msg: &str) -> &str {
    msg
}

fn run() -> i32 {
    println!("============================================================");
    println!("   hyb-cronctl — HybridOS Scheduler CLI");
    println!("============================================================");

    let mut ctl = CronCtl::new();
    ctl.add_job("backup-daily", "/usr/bin/hyb-sync /mnt/data /mnt/backup");
    ctl.add_job("log-rotate", "/usr/bin/hyb-logs --rotate");
    ctl.add_job("net-reconnect", "/usr/bin/hyb-netctl --reconnect");

    ctl.list();
    println!();
    ctl.pause("log-rotate");
    ctl.trigger("backup-daily");
    ctl.trigger("net-reconnect");
    println!();
    ctl.logs("backup-daily");
    ctl.logs("log-rotate");
    println!();
    ctl.resume("log-rotate");
    ctl.list();
    println!();
    println!("[hyb-cronctl] listar/pausar/disparar/logs de tarefas validados.");
    sys_sched_yield();
    0
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    sys_exit(run());
}
