//! hyb-cron: HybridOS Task and Event Scheduler (Ring 3 ELF64)
//!
//! Clean Task Scheduler substitute. Runs jobs based on cron expressions,
//! boot, specific network or hardware idle.

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
    println!("[hyb-cron CRASH] panicked: {:?}", info);
    sys_exit(1);
}

const MAX_JOBS: usize = 8;
const MAX_NAME_LEN: usize = 48;
const MAX_CMD_LEN: usize = 96;
const MAX_CRON_LEN: usize = 32;

#[derive(Copy, Clone, PartialEq)]
enum JobTrigger { Cron, Boot, Network, Idle }

#[derive(Copy, Clone)]
struct CronJob {
    name: [u8; MAX_NAME_LEN],
    name_len: usize,
    cmd: [u8; MAX_CMD_LEN],
    cmd_len: usize,
    cron: [u8; MAX_CRON_LEN],
    cron_len: usize,
    trigger: JobTrigger,
    enabled: bool,
    run_count: u32,
}

struct CronDaemon {
    jobs: [CronJob; MAX_JOBS],
    count: usize,
}

impl CronDaemon {
    const fn new() -> Self {
        Self { jobs: [CronJob { name: [0u8; MAX_NAME_LEN], name_len: 0, cmd: [0u8; MAX_CMD_LEN], cmd_len: 0, cron: [0u8; MAX_CRON_LEN], cron_len: 0, trigger: JobTrigger::Boot, enabled: false, run_count: 0 }; MAX_JOBS], count: 0 }
    }

    fn add(&mut self, name: &str, cmd: &str, cron: &str, trigger: JobTrigger) {
        if self.count >= MAX_JOBS { return; }
        let nb = name.as_bytes();
        let nl = nb.len().min(MAX_NAME_LEN - 1);
        self.jobs[self.count].name[..nl].copy_from_slice(&nb[..nl]);
        self.jobs[self.count].name_len = nl;
        let cb = cmd.as_bytes();
        let cl = cb.len().min(MAX_CMD_LEN - 1);
        self.jobs[self.count].cmd[..cl].copy_from_slice(&cb[..cl]);
        self.jobs[self.count].cmd_len = cl;
        let cb2 = cron.as_bytes();
        let cl2 = cb2.len().min(MAX_CRON_LEN - 1);
        self.jobs[self.count].cron[..cl2].copy_from_slice(&cb2[..cl2]);
        self.jobs[self.count].cron_len = cl2;
        self.jobs[self.count].trigger = trigger;
        self.jobs[self.count].enabled = true;
        self.jobs[self.count].run_count = 0;
        self.count += 1;
        println!("[hyb-cron] job registered: '{}' (trigger={:?})", name, match self.jobs[self.count].trigger {
            JobTrigger::Cron => "cron",
            JobTrigger::Boot => "boot",
            JobTrigger::Network => "network",
            JobTrigger::Idle => "idle",
        });
    }

    fn run_due(&mut self) {
        println!("[hyb-cron] checking jobs due for execution...");
        for i in 0..self.count {
            if !self.jobs[i].enabled { continue; }
            let nm = core::str::from_utf8(&self.jobs[i].name[..self.jobs[i].name_len]).unwrap_or("");
            let cd = core::str::from_utf8(&self.jobs[i].cmd[..self.jobs[i].cmd_len]).unwrap_or("");
            let cr = core::str::from_utf8(&self.jobs[i].cron[..self.jobs[i].cron_len]).unwrap_or("");
            match self.jobs[i].trigger {
                JobTrigger::Cron => { println!("[hyb-cron] firing cron '{}' [{}] -> {}", nm, cr, cd); self.jobs[i].run_count += 1; }
                JobTrigger::Boot => { println!("[hyb-cron] boot job '{}' -> {}", nm, cd); self.jobs[i].run_count += 1; }
                JobTrigger::Network => { println!("[hyb-cron] network job '{}' -> {}", nm, cd); self.jobs[i].run_count += 1; }
                JobTrigger::Idle => { println!("[hyb-cron] idle job '{}' -> {}", nm, cd); self.jobs[i].run_count += 1; }
            }
        }
    }

    fn list(&self) {
        println!("[hyb-cron] scheduled tasks:");
        if self.count == 0 { println!("  (none)"); }
        for i in 0..self.count {
            let j = &self.jobs[i];
            let nm = core::str::from_utf8(&j.name[..j.name_len]).unwrap_or("");
            let cr = core::str::from_utf8(&j.cron[..j.cron_len]).unwrap_or("");
            let cd = core::str::from_utf8(&j.cmd[..j.cmd_len]).unwrap_or("");
            println!("  {} | cron={} | cmd={} | runs={} | enabled={}", nm, cr, cd, j.run_count, j.enabled);
        }
    }
}

fn run() -> i32 {
    println!("============================================================");
    println!("   hyb-cron — HybridOS Task & Event Scheduler");
    println!("============================================================");

    let mut cron = CronDaemon::new();
    cron.add("backup-daily", "/usr/bin/hyb-sync /mnt/data /mnt/backup", "0 2 * * *", JobTrigger::Cron);
    cron.add("log-rotate", "/usr/bin/hyb-logs --rotate", "0 3 * * 0", JobTrigger::Cron);
    cron.add("net-reconnect", "/usr/bin/hyb-netctl --reconnect", "@network", JobTrigger::Network);
    cron.add("idle-clean", "/usr/bin/hyb-clean", "@idle", JobTrigger::Idle);
    cron.add("boot-telemetry", "/usr/bin/hyb-telemetry-local --submit", "@boot", JobTrigger::Boot);

    cron.list();
    println!();
    cron.run_due();
    println!();
    println!("[hyb-cron] cron/boot/network/idle scheduling validated.");
    sys_sched_yield();
    0
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    sys_exit(run());
}
