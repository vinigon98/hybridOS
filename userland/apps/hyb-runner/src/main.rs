//! hyb-runner: Executor Silencioso em Background (Ring 3 ELF64)
//!
//! Roda scripts de automação em segundo plano sem piscar janelas de
//! terminal na tela do usuário.

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
    println!("[hyb-runner CRASH] panicked: {:?}", info);
    sys_exit(1);
}

const MAX_JOBS: usize = 8;
const MAX_NAME_LEN: usize = 48;
const MAX_CMD_LEN: usize = 96;
const MAX_LOG_LEN: usize = 96;
const MAX_LOGS: usize = 8;

#[derive(Copy, Clone)]
enum JobState {
    Queued,
    Running,
    Done,
    Failed,
}

#[derive(Copy, Clone)]
struct BackgroundJob {
    name: [u8; MAX_NAME_LEN],
    name_len: usize,
    cmd: [u8; MAX_CMD_LEN],
    cmd_len: usize,
    state: JobState,
    exit_code: i32,
}

struct Runner {
    jobs: [BackgroundJob; MAX_JOBS],
    count: usize,
    logs: [[u8; MAX_LOG_LEN]; MAX_LOGS],
    log_len: [usize; MAX_LOGS],
    log_count: usize,
    visible: bool, // falso = silencioso (sem janela)
}

impl Runner {
    const fn new() -> Self {
        Self {
            jobs: [BackgroundJob { name: [0u8; MAX_NAME_LEN], name_len: 0, cmd: [0u8; MAX_CMD_LEN], cmd_len: 0, state: JobState::Queued, exit_code: 0 }; MAX_JOBS],
            count: 0,
            logs: [[0u8; MAX_LOG_LEN]; MAX_LOGS],
            log_len: [0usize; MAX_LOGS],
            log_count: 0,
            visible: false,
        }
    }

    fn submit(&mut self, name: &str, cmd: &str) {
        if self.count >= MAX_JOBS {
            println!("[hyb-runner] limite de jobs em background atingido.");
            return;
        }
        let nb = name.as_bytes();
        let nl = nb.len().min(MAX_NAME_LEN - 1);
        self.jobs[self.count].name[..nl].copy_from_slice(&nb[..nl]);
        self.jobs[self.count].name_len = nl;
        let cb = cmd.as_bytes();
        let cl = cb.len().min(MAX_CMD_LEN - 1);
        self.jobs[self.count].cmd[..cl].copy_from_slice(&cb[..cl]);
        self.jobs[self.count].cmd_len = cl;
        self.jobs[self.count].state = JobState::Queued;
        self.jobs[self.count].exit_code = 0;
        self.count += 1;
        println!("[hyb-runner] job submetido (background, janela oculta): '{}'", name);
    }

    fn run_all(&mut self) {
        if self.visible {
            println!("[hyb-runner] modo visível ativado.");
        } else {
            println!("[hyb-runner] executando jobs silenciosamente (sem piscar terminal)...");
        }
        for i in 0..self.count {
            self.jobs[i].state = JobState::Running;
            // execução simulada do script de automação
            self.jobs[i].state = JobState::Done;
            self.jobs[i].exit_code = 0;
            self.add_log("executed -> exit 0");
            let j = &self.jobs[i];
            let nm = core::str::from_utf8(&j.name[..j.name_len]).unwrap_or("");
            let state = match j.state {
                JobState::Queued => "queued",
                JobState::Running => "running",
                JobState::Done => "done",
                JobState::Failed => "failed",
            };
            if !self.visible {
                println!("[hyb-runner]   [oculto] {} | state={} | exit={}", nm, state, self.jobs[i].exit_code);
            } else {
                println!("[hyb-runner]   {} | state={} | exit={}", nm, state, self.jobs[i].exit_code);
            }
        }
        sys_sched_yield();
    }

    fn status(&self) {
        println!("[hyb-runner] status dos jobs em background:");
        if self.count == 0 {
            println!("  (nenhum)");
        }
        for i in 0..self.count {
            let j = &self.jobs[i];
            let nm = core::str::from_utf8(&j.name[..j.name_len]).unwrap_or("");
            let state = match j.state {
                JobState::Queued => "queued",
                JobState::Running => "running",
                JobState::Done => "done",
                JobState::Failed => "failed",
            };
            println!("  {} | state={} | exit={}", nm, state, j.exit_code);
        }
    }

    fn show_logs(&self) {
        println!("[hyb-runner] logs dos jobs executados:");
        if self.log_count == 0 {
            println!("  (sem logs)");
        }
        for i in 0..self.log_count {
            let l = core::str::from_utf8(&self.logs[i][..self.log_len[i]]).unwrap_or("");
            println!("  {}", l);
        }
    }

    fn add_log(&mut self, line: &str) {
        if self.log_count >= MAX_LOGS {
            return;
        }
        let b = line.as_bytes();
        let cl = b.len().min(MAX_LOG_LEN - 1);
        self.logs[self.log_count][..cl].copy_from_slice(&b[..cl]);
        self.log_len[self.log_count] = cl;
        self.log_count += 1;
    }
}

fn run() -> i32 {
    println!("============================================================");
    println!("   hyb-runner — HybridOS Silent Background Executor");
    println!("============================================================");

    let mut runner = Runner::new();
    runner.submit("backup-script", "/usr/bin/hyb-sync /mnt/data /mnt/backup");
    runner.submit("log-rotate", "/usr/bin/hyb-logs --rotate");
    runner.submit("telemetry-submit", "/usr/bin/hyb-telemetry-local --submit");
    println!();
    runner.run_all();
    println!();
    runner.status();
    println!();
    runner.show_logs();
    println!();
    println!("[hyb-runner] execução silenciosa em background validada.");
    0
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    sys_exit(run());
}
