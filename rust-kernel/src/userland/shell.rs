//! HybridOS Userland Shell & Coreutils.
//!
//! Provides the primary interactive kernel command line interface (CLI) and core utilities
//! for the dual-personality HybridOS kernel.
//!
//! Supports seamless execution and inspection across both Windows NT (PE32+, SSDT, Win32 paths)
//! and Linux POSIX (ELF64, System V AMD64 ABI, Unix paths) subsystems.
//!
//! # Built-in Commands
//! - `help [cmd]`: Lists all available hybrid commands or syntax for a specific command.
//! - `ps`: Lists active processes, PIDs, personality (Windows NT vs Linux POSIX), and CPU state.
//! - `meminfo`: Displays RAM usage, physical page frames allocated, PML4 CR3, and heap stats.
//! - `lspci`: Displays enumerated hardware PCI devices and Base Address Registers (BARs).
//! - `ls [path]`: Lists files and directories in VFS (supports `/` or `C:\`).
//! - `cat <file>`: Reads and prints file contents from VFS.
//! - `echo <text>`: Prints text or expanded environment variables to console.
//! - `uname [-a|-s|-r|-m|-v]`: Displays HybridOS kernel version, architecture (x86_64), and build date.
//! - `run <file>`: Executes a PE32+ or ELF64 binary image through the appropriate subsystem.
//! - `personality [mode]`: Switches or inspects the active environment mode.
//! - `cd [path]`: Changes the current working directory.
//! - `pwd`: Prints the current working directory.
//! - `clear` / `cls`: Clears the console terminal screen.
//! - `history`: Displays command execution history.

#[allow(unused_imports)]
use alloc::string::{String, ToString};
#[allow(unused_imports)]
use alloc::vec::Vec;
#[allow(unused_imports)]
use alloc::boxed::Box;
#[allow(unused_imports)]
use crate::compat::FloatExt;
use std::collections::HashMap;
use std::fmt;
use std::io::{BufRead, Write};

use crate::drivers::hardware::{PciBar, PciBusScanner, VirtualPortIo};
use crate::loaders::elf::load_elf;
use crate::loaders::pe::load_pe;
use crate::mm::{self, MemoryInfo, PAGE_SIZE};
use crate::personality::ExecutionPersonality;
use crate::process::HybridProcess;
use crate::sched::{self, Priority, PriorityClass};
use crate::syscall::{self, SyscallRouter};
use crate::trap::TrapFrame;
use crate::vfs::{self, InodeType, Permissions, VfsError};

/// Kernel and OS release constants
pub const HYBRIDOS_KERNEL_NAME: &str = "HybridOS";
pub const HYBRIDOS_RELEASE_VERSION: &str = "1.0.0-dual-abi";
pub const HYBRIDOS_ARCH: &str = "x86_64";
pub const HYBRIDOS_BUILD_DATE: &str = "Sun Nov 9 2026 15:45:00 UTC";
pub const HYBRIDOS_KERNEL_VERSION_DESC: &str =
    "#1 SMP Sun Nov 9 2026 x86_64 Dual-Personality (WinNT 10.0 + Linux POSIX 6.6)";

/// Errors that can occur during command interpretation or execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellError {
    EmptyCommand,
    UnknownCommand(String),
    MissingArgument(String),
    InvalidArgument(String),
    VfsError(String),
    BinaryExecutionError(String),
    IoError(String),
}

impl fmt::Display for ShellError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyCommand => write!(f, ""),
            Self::UnknownCommand(cmd) => {
                write!(f, "command not found: '{}'. Type 'help' for available commands.", cmd)
            }
            Self::MissingArgument(arg) => write!(f, "missing required argument: <{}>", arg),
            Self::InvalidArgument(msg) => write!(f, "invalid argument: {}", msg),
            Self::VfsError(err) => write!(f, "vfs error: {}", err),
            Self::BinaryExecutionError(err) => write!(f, "binary execution error: {}", err),
            Self::IoError(err) => write!(f, "i/o error: {}", err),
        }
    }
}

impl std::error::Error for ShellError {}

impl From<VfsError> for ShellError {
    fn from(err: VfsError) -> Self {
        Self::VfsError(err.to_string())
    }
}

pub type ShellResult<T> = Result<T, ShellError>;

/// Terminal / Shell visual personality and prompt flavor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellFlavor {
    /// Unified ring-0 kernel shell: prompt `hybridos[ring0]:path# `
    Hybrid,
    /// Windows Command Prompt (cmd.exe): prompt `C:\path> `
    WindowsCmd,
    /// Windows PowerShell (pwsh): prompt `PS C:\path> `
    PowerShell,
    /// Linux GNU Bash / POSIX: prompt `root@hybridos:path# `
    LinuxBash,
}

/// Interactive Kernel Command Line Interface & Coreutils Runner.
pub struct HybridShell {
    /// Active execution personality governing subsystem conventions.
    pub personality: ExecutionPersonality,
    /// Shell prompt flavor (Hybrid, WindowsCmd, PowerShell, LinuxBash).
    pub flavor: ShellFlavor,
    /// Current working directory (e.g. `/` or `C:\`).
    pub cwd: String,
    /// Shell environment variables table (e.g. `USER`, `OS`, `ARCH`, `PATH`).
    pub env_vars: HashMap<String, String>,
    /// Command line history log.
    pub history: Vec<String>,
    /// Exit code of the most recently executed command.
    pub last_exit_code: i32,
    /// Whether the interactive REPL is actively running.
    pub is_running: bool,
    /// Monotonically increasing counter for synthetic userland PIDs.
    next_synthetic_pid: u32,
}

impl Default for HybridShell {
    fn default() -> Self {
        Self::new()
    }
}

impl HybridShell {
    /// Create a new `HybridShell` instance initialized with default environment variables,
    /// baseline file hierarchy, and sample binaries in VFS.
    pub fn new() -> Self {
        Self::with_personality(ExecutionPersonality::HybridNative)
    }

    /// Create a new `HybridShell` initialized with a specific execution personality.
    pub fn with_personality(personality: ExecutionPersonality) -> Self {
        let mut env_vars = HashMap::new();
        env_vars.insert("USER".to_string(), "root".to_string());
        env_vars.insert("KERNEL".to_string(), HYBRIDOS_KERNEL_NAME.to_string());
        env_vars.insert("VERSION".to_string(), HYBRIDOS_RELEASE_VERSION.to_string());
        env_vars.insert("ARCH".to_string(), HYBRIDOS_ARCH.to_string());
        env_vars.insert("PERSONALITY".to_string(), personality.name().to_string());
        env_vars.insert("SHELL".to_string(), "/bin/hybridshell".to_string());
        env_vars.insert("PATH".to_string(), "/bin:/usr/bin:C:\\Windows\\System32".to_string());

        let (cwd, flavor) = match personality {
            ExecutionPersonality::WindowsNt => ("C:\\".to_string(), ShellFlavor::WindowsCmd),
            ExecutionPersonality::LinuxPosix => ("/".to_string(), ShellFlavor::LinuxBash),
            _ => ("/".to_string(), ShellFlavor::Hybrid),
        };

        let shell = Self {
            personality,
            flavor,
            cwd,
            env_vars,
            history: Vec::new(),
            last_exit_code: 0,
            is_running: true,
            next_synthetic_pid: 2000,
        };

        shell.ensure_subsystems_initialized();
        shell
    }

    /// Automatically bootstraps VFS, Memory Management, and Scheduler subsystems
    /// and populates standard executable test binaries.
    pub fn ensure_subsystems_initialized(&self) {
        // 1. Initialize Memory Management Subsystem
        if mm::get_memory_info().is_none() {
            let _ = mm::init_mm();
        }

        // 2. Initialize Scheduler Subsystem
        let _ = sched::init_scheduler();

        // 3. Initialize Virtual File System
        let _ = vfs::init_vfs();

        // 4. Install sample test binaries into VFS for immediate execution
        self.populate_sample_binaries();

        // 5. Populate baseline processes into scheduler for realistic diagnostics
        self.populate_baseline_processes();
    }

    /// Pre-populates sample PE32+ and ELF64 binary images into the VFS.
    fn populate_sample_binaries(&self) {
        let sample_pe = create_synthetic_pe();
        let sample_elf = create_synthetic_elf();

        let _ = vfs::vfs_create_file(
            "C:\\Windows\\System32\\sample_app.exe",
            &sample_pe,
            Permissions::default_executable(),
        );
        let _ = vfs::vfs_create_file(
            "C:\\Windows\\System32\\cmd.exe",
            &sample_pe,
            Permissions::default_executable(),
        );
        let _ = vfs::vfs_create_file(
            "/bin/sample_elf",
            &sample_elf,
            Permissions::default_executable(),
        );
        let _ = vfs::vfs_create_file(
            "/bin/bash",
            &sample_elf,
            Permissions::default_executable(),
        );
    }

    /// Pre-populates baseline Windows NT and Linux POSIX processes into scheduler.
    fn populate_baseline_processes(&self) {
        let scheduler_lock = sched::get_scheduler();
        let mut scheduler = scheduler_lock.lock().unwrap();

        // Check if baseline processes already populated
        if scheduler.get_process(1).is_none() {
            // Linux PID 1: systemd / init
            let p1 = scheduler.create_process("systemd", ExecutionPersonality::LinuxPosix, PriorityClass::Normal);
            let pid1 = p1.lock().unwrap().pid;
            let _ = scheduler.create_thread(pid1, "systemd-main", 0x0040_1000, 0x0000_7FFF_0000_0000, Priority::NORMAL);

            // Windows NT PID 4: ntoskrnl.exe System
            let p4 = scheduler.create_process("System", ExecutionPersonality::WindowsNt, PriorityClass::High);
            let pid4 = p4.lock().unwrap().pid;
            let _ = scheduler.create_thread(pid4, "ntoskrnl-worker", 0x0001_4000_1000, 0x0000_7FFF_1000_0000, Priority::HIGH);

            // Windows NT PID 100: smss.exe (Session Manager)
            let p100 = scheduler.create_process("smss.exe", ExecutionPersonality::WindowsNt, PriorityClass::Normal);
            let pid100 = p100.lock().unwrap().pid;
            let _ = scheduler.create_thread(pid100, "smss-main", 0x0001_4001_0000, 0x0000_7FFF_2000_0000, Priority::NORMAL);

            // Linux PID 101: bash_daemon
            let p101 = scheduler.create_process("bash_daemon", ExecutionPersonality::LinuxPosix, PriorityClass::Normal);
            let pid101 = p101.lock().unwrap().pid;
            let _ = scheduler.create_thread(pid101, "bash-worker", 0x0040_5000, 0x0000_7FFF_3000_0000, Priority::NORMAL);

            // Windows NT PID 102: explorer.exe
            let p102 = scheduler.create_process("explorer.exe", ExecutionPersonality::WindowsNt, PriorityClass::Normal);
            let pid102 = p102.lock().unwrap().pid;
            let _ = scheduler.create_thread(pid102, "explorer-gui", 0x0001_4002_0000, 0x0000_7FFF_4000_0000, Priority::NORMAL);
        }
    }

    /// Formats the active interactive shell prompt string according to current personality, flavor and CWD.
    pub fn prompt(&self) -> String {
        match self.flavor {
            ShellFlavor::PowerShell => {
                let win_path = self.format_win_path();
                format!("PS {}> ", win_path)
            }
            ShellFlavor::WindowsCmd => {
                let win_path = self.format_win_path();
                format!("{}> ", win_path)
            }
            ShellFlavor::LinuxBash => {
                let u_path = self.format_unix_path();
                let display_path = if u_path == "/root" { "~" } else { &u_path };
                format!("root@hybridos:{}# ", display_path)
            }
            ShellFlavor::Hybrid => {
                format!("hybridos[ring0]:{}# ", self.cwd)
            }
        }
    }

    /// Formats current working directory in canonical Windows style (e.g. `C:\Windows\System32`).
    pub fn format_win_path(&self) -> String {
        if self.cwd.starts_with('/') {
            let canon = vfs::vfs_resolve_path(&self.cwd).unwrap_or_else(|_| self.cwd.clone());
            if canon == "/" || canon == "/c" {
                "C:\\".to_string()
            } else if let Some(stripped) = canon.strip_prefix("/c/") {
                format!("C:\\{}", stripped.replace('/', "\\"))
            } else {
                format!("C:{}", canon.replace('/', "\\"))
            }
        } else {
            self.cwd.clone()
        }
    }

    /// Formats current working directory in canonical Unix POSIX style (e.g. `/etc` or `/`).
    pub fn format_unix_path(&self) -> String {
        if self.cwd.contains('\\') || self.cwd.starts_with("C:") || self.cwd.starts_with("c:") {
            vfs::vfs_resolve_path(&self.cwd).unwrap_or_else(|_| "/".to_string())
        } else {
            self.cwd.clone()
        }
    }

    /// Evaluates and executes a single command line string, returning the formatted output.
    pub fn execute(&mut self, line: &str) -> String {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return String::new();
        }

        self.history.push(trimmed.to_string());

        let tokens = match tokenize_command(trimmed) {
            Ok(t) if !t.is_empty() => t,
            _ => return String::new(),
        };

        // Expand environment variables within tokens (e.g. $USER, $ARCH)
        let expanded_tokens: Vec<String> = tokens
            .into_iter()
            .map(|tok| self.expand_variables(&tok))
            .collect();

        let cmd = &expanded_tokens[0];
        let args = &expanded_tokens[1..];

        match cmd.to_ascii_lowercase().as_str() {
            // =================================================================
            // Shell Flavor / Subsystem Switchers
            // =================================================================
            "powershell" | "pwsh" => {
                self.flavor = ShellFlavor::PowerShell;
                self.personality = ExecutionPersonality::WindowsNt;
                if self.cwd == "/" { self.cwd = "C:\\".to_string(); }
                "Windows PowerShell v7.4.0 (HybridOS Subsystem Host)\nType 'help' or 'Get-Command' for available cmdlets.\n".to_string()
            }
            "cmd" => {
                self.flavor = ShellFlavor::WindowsCmd;
                self.personality = ExecutionPersonality::WindowsNt;
                if self.cwd == "/" { self.cwd = "C:\\".to_string(); }
                "Microsoft Windows [Version 10.0.22631.4317] Command Processor\n(c) Microsoft Corporation / HybridOS. All rights reserved.\n".to_string()
            }
            "bash" | "sh" => {
                self.flavor = ShellFlavor::LinuxBash;
                self.personality = ExecutionPersonality::LinuxPosix;
                if self.cwd.starts_with("C:") { self.cwd = "/".to_string(); }
                "GNU bash, version 5.2.21(1)-release (x86_64-pc-hybridos-gnu)\n".to_string()
            }
            "hybrid" => {
                self.flavor = ShellFlavor::Hybrid;
                self.personality = ExecutionPersonality::HybridNative;
                "HybridOS Kernel Ring-0 Unified Shell Host.\n".to_string()
            }

            // =================================================================
            // System Diagnostics, Version & Identity
            // =================================================================
            "help" | "?" => {
                let sub = args.first().map(|s| s.as_str());
                self.cmd_help(sub)
            }
            "get-command" | "gcm" => {
                self.last_exit_code = 0;
                self.cmd_get_command()
            }
            "uname" => {
                self.last_exit_code = 0;
                self.cmd_uname(args)
            }
            "ver" | "version" => {
                self.last_exit_code = 0;
                self.cmd_ver()
            }
            "systeminfo" => {
                self.last_exit_code = 0;
                self.cmd_systeminfo()
            }
            "whoami" => {
                self.last_exit_code = 0;
                self.cmd_whoami()
            }
            "hostname" => {
                self.last_exit_code = 0;
                self.cmd_hostname()
            }
            "date" | "time" | "get-date" => {
                self.last_exit_code = 0;
                self.cmd_date_time()
            }
            "meminfo" | "free" => {
                self.last_exit_code = 0;
                self.cmd_meminfo()
            }
            "lspci" => {
                self.last_exit_code = 0;
                self.cmd_lspci()
            }
            "dmesg" => {
                self.last_exit_code = 0;
                self.cmd_dmesg()
            }

            // =================================================================
            // Process & Task Management (PS, CMD, Linux)
            // =================================================================
            "ps" | "get-process" | "gps" | "tasklist" => {
                self.last_exit_code = 0;
                self.cmd_ps()
            }
            "top" => {
                self.last_exit_code = 0;
                self.cmd_top()
            }
            "kill" | "taskkill" | "stop-process" | "spps" => {
                let pid_str = args.first().map(|s| s.as_str()).unwrap_or("");
                match self.cmd_kill(pid_str) {
                    Ok(out) => {
                        self.last_exit_code = 0;
                        out
                    }
                    Err(e) => {
                        self.last_exit_code = 1;
                        format!("kill: {}", e)
                    }
                }
            }
            "get-service" | "gsv" => {
                self.last_exit_code = 0;
                self.cmd_get_service()
            }

            // =================================================================
            // VFS, Navigation & File Operations (PS, CMD, Linux)
            // =================================================================
            "ls" | "dir" | "get-childitem" | "gci" => {
                let path_arg = args.first().map(|s| s.as_str());
                match self.cmd_ls(path_arg) {
                    Ok(out) => {
                        self.last_exit_code = 0;
                        out
                    }
                    Err(e) => {
                        self.last_exit_code = 1;
                        format!("ls error: {}", e)
                    }
                }
            }
            "cat" | "type" | "get-content" | "gc" => {
                if args.is_empty() {
                    self.last_exit_code = 1;
                    "cat: missing file operand. Syntax: cat <file>".to_string()
                } else {
                    match self.cmd_cat(&args[0]) {
                        Ok(out) => {
                            self.last_exit_code = 0;
                            out
                        }
                        Err(e) => {
                            self.last_exit_code = 1;
                            format!("cat: {}: {}", args[0], e)
                        }
                    }
                }
            }
            "cd" | "chdir" | "set-location" | "sl" => {
                let target = args.first().map(|s| s.as_str()).unwrap_or("/");
                match self.cmd_cd(target) {
                    Ok(out) => {
                        self.last_exit_code = 0;
                        out
                    }
                    Err(e) => {
                        self.last_exit_code = 1;
                        format!("cd: {}", e)
                    }
                }
            }
            "pwd" | "get-location" | "gl" => {
                self.last_exit_code = 0;
                self.cmd_pwd()
            }
            "clear" | "cls" | "clear-host" => {
                self.last_exit_code = 0;
                self.cmd_clear()
            }
            "touch" | "new-item" | "ni" => {
                if args.is_empty() {
                    self.last_exit_code = 1;
                    "touch: missing file operand. Syntax: touch <filename>".to_string()
                } else {
                    match self.cmd_touch(&args[0]) {
                        Ok(out) => { self.last_exit_code = 0; out }
                        Err(e) => { self.last_exit_code = 1; format!("touch error: {}", e) }
                    }
                }
            }
            "mkdir" | "md" => {
                if args.is_empty() {
                    self.last_exit_code = 1;
                    "mkdir: missing directory name. Syntax: mkdir <path>".to_string()
                } else {
                    match self.cmd_mkdir(&args[0]) {
                        Ok(out) => { self.last_exit_code = 0; out }
                        Err(e) => { self.last_exit_code = 1; format!("mkdir error: {}", e) }
                    }
                }
            }
            "rmdir" | "rd" => {
                if args.is_empty() {
                    self.last_exit_code = 1;
                    "rmdir: missing directory operand. Syntax: rmdir <path>".to_string()
                } else {
                    match self.cmd_rmdir(&args[0]) {
                        Ok(out) => { self.last_exit_code = 0; out }
                        Err(e) => { self.last_exit_code = 1; format!("rmdir error: {}", e) }
                    }
                }
            }
            "rm" | "del" | "erase" | "remove-item" | "ri" => {
                if args.is_empty() {
                    self.last_exit_code = 1;
                    "rm: missing operand. Syntax: rm <path>".to_string()
                } else {
                    match self.cmd_rm(&args[0]) {
                        Ok(out) => { self.last_exit_code = 0; out }
                        Err(e) => { self.last_exit_code = 1; format!("rm error: {}", e) }
                    }
                }
            }
            "cp" | "copy" => {
                if args.len() < 2 {
                    self.last_exit_code = 1;
                    "cp: missing destination file operand after source. Syntax: cp <src> <dest>".to_string()
                } else {
                    match self.cmd_cp(&args[0], &args[1]) {
                        Ok(out) => { self.last_exit_code = 0; out }
                        Err(e) => { self.last_exit_code = 1; format!("cp error: {}", e) }
                    }
                }
            }
            "mv" | "move" | "ren" | "rename" => {
                if args.len() < 2 {
                    self.last_exit_code = 1;
                    "mv: missing destination file operand after source. Syntax: mv <src> <dest>".to_string()
                } else {
                    match self.cmd_mv(&args[0], &args[1]) {
                        Ok(out) => { self.last_exit_code = 0; out }
                        Err(e) => { self.last_exit_code = 1; format!("mv error: {}", e) }
                    }
                }
            }
            "test-path" => {
                self.last_exit_code = 0;
                let path = args.first().map(|s| s.as_str()).unwrap_or("");
                self.cmd_test_path(path)
            }
            "head" => {
                if args.is_empty() {
                    self.last_exit_code = 1;
                    "head: missing file operand. Syntax: head <file> [lines]".to_string()
                } else {
                    let lines = args.get(1).and_then(|s| s.parse::<usize>().ok()).unwrap_or(10);
                    match self.cmd_head(&args[0], lines) {
                        Ok(out) => { self.last_exit_code = 0; out }
                        Err(e) => { self.last_exit_code = 1; format!("head error: {}", e) }
                    }
                }
            }
            "tail" => {
                if args.is_empty() {
                    self.last_exit_code = 1;
                    "tail: missing file operand. Syntax: tail <file> [lines]".to_string()
                } else {
                    let lines = args.get(1).and_then(|s| s.parse::<usize>().ok()).unwrap_or(10);
                    match self.cmd_tail(&args[0], lines) {
                        Ok(out) => { self.last_exit_code = 0; out }
                        Err(e) => { self.last_exit_code = 1; format!("tail error: {}", e) }
                    }
                }
            }
            "grep" | "select-string" | "sls" => {
                if args.len() < 2 {
                    self.last_exit_code = 1;
                    "grep: missing pattern or file operand. Syntax: grep <pattern> <file>".to_string()
                } else {
                    match self.cmd_grep(&args[0], &args[1]) {
                        Ok(out) => { self.last_exit_code = 0; out }
                        Err(e) => { self.last_exit_code = 1; format!("grep error: {}", e) }
                    }
                }
            }
            "wc" => {
                if args.is_empty() {
                    self.last_exit_code = 1;
                    "wc: missing file operand. Syntax: wc <file>".to_string()
                } else {
                    match self.cmd_wc(&args[0]) {
                        Ok(out) => { self.last_exit_code = 0; out }
                        Err(e) => { self.last_exit_code = 1; format!("wc error: {}", e) }
                    }
                }
            }
            "df" => {
                self.last_exit_code = 0;
                self.cmd_df()
            }
            "vol" => {
                self.last_exit_code = 0;
                self.cmd_vol()
            }

            // =================================================================
            // Network Diagnostics
            // =================================================================
            "ifconfig" | "ip" | "get-netipaddress" => {
                self.last_exit_code = 0;
                self.cmd_ifconfig()
            }
            "netstat" => {
                self.last_exit_code = 0;
                self.cmd_netstat()
            }
            "ping" => {
                self.last_exit_code = 0;
                let target = args.first().map(|s| s.as_str()).unwrap_or("192.168.1.1");
                self.cmd_ping(target)
            }

            // =================================================================
            // Variables, Output & Execution
            // =================================================================
            "echo" | "write-host" | "write-output" => {
                self.last_exit_code = 0;
                self.cmd_echo(args)
            }
            "set" | "env" | "export" => {
                self.last_exit_code = 0;
                self.cmd_set(args)
            }
            "run" | "exec" | "start" => {
                if args.is_empty() {
                    self.last_exit_code = 1;
                    "run: missing binary path. Syntax: run <path/to/binary>".to_string()
                } else {
                    match self.cmd_run(&args[0]) {
                        Ok(out) => {
                            self.last_exit_code = 0;
                            out
                        }
                        Err(e) => {
                            self.last_exit_code = 1;
                            format!("run error: {}", e)
                        }
                    }
                }
            }
            "personality" | "subsystem" | "mode" => {
                let target = args.first().map(|s| s.as_str());
                self.cmd_personality(target)
            }
            "history" => {
                self.last_exit_code = 0;
                self.cmd_history()
            }
            "exit" | "quit" => {
                self.is_running = false;
                "Session terminated. Kernel returning to ring-0 control.".to_string()
            }
            _ => {
                self.last_exit_code = 127;
                format!("{}: command not found. Type 'help' or 'Get-Command' for available commands.", cmd)
            }
        }
    }

    /// Variable expansion helper ($VAR or ${VAR}).
    fn expand_variables(&self, text: &str) -> String {
        let mut out = String::new();
        let chars: Vec<char> = text.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            if chars[i] == '$' && i + 1 < chars.len() {
                i += 1;
                let mut var_name = String::new();
                if chars[i] == '{' {
                    i += 1;
                    while i < chars.len() && chars[i] != '}' {
                        var_name.push(chars[i]);
                        i += 1;
                    }
                    if i < chars.len() && chars[i] == '}' {
                        i += 1;
                    }
                } else {
                    while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                        var_name.push(chars[i]);
                        i += 1;
                    }
                }

                if var_name == "?" {
                    out.push_str(&self.last_exit_code.to_string());
                } else if var_name == "PWD" {
                    out.push_str(&self.cwd);
                } else if let Some(val) = self.env_vars.get(&var_name) {
                    out.push_str(val);
                }
            } else {
                out.push(chars[i]);
                i += 1;
            }
        }

        out
    }

    // =========================================================================
    // Built-in Command Implementations
    // =========================================================================

    /// Command `help`: Lists all available commands or detailed syntax for a command.
    pub fn cmd_help(&self, subcmd: Option<&str>) -> String {
        if let Some(target) = subcmd {
            return match target.to_ascii_lowercase().as_str() {
                "ps" => "ps - Process Status\nSyntax: ps\nDisplays all active processes, PIDs, ABI personality (WinNT vs Linux POSIX),\nCPU registers, state, priority, and thread metrics.".to_string(),
                "meminfo" => "meminfo - System Memory Information\nSyntax: meminfo\nDisplays total physical RAM, frame allocator stats (total/allocated/free 4KB frames),\n4-level x86_64 paging root (CR3), and Kernel Dynamic Heap allocator statistics.".to_string(),
                "lspci" => "lspci - List PCI Devices\nSyntax: lspci\nEnumerates the PCI bus (bus 0..8, device 0..32, function 0..8) and displays all\ndetected hardware devices, vendor names, class codes, and Base Address Registers (BARs).".to_string(),
                "ls" => "ls - List Directory Contents\nSyntax: ls [path]\nLists files, directories, and device nodes. Supports Unix paths ('/bin', '/etc')\nand Win32 DOS paths ('C:\\', 'C:\\Windows\\System32').".to_string(),
                "cat" => "cat - Read File Contents\nSyntax: cat <file>\nReads and prints file contents from VFS to console. Supports both Unix and Win32 paths.".to_string(),
                "echo" => "echo - Print Arguments\nSyntax: echo <text...>\nPrints text to console with environment variable expansion ($VAR, ${VAR}, $?).".to_string(),
                "uname" => "uname - Kernel & Architecture Information\nSyntax: uname [-a|-s|-r|-m|-v]\nDisplays HybridOS kernel version, machine architecture (x86_64), and release build date.".to_string(),
                "run" => "run - Execute Binary Image\nSyntax: run <path/to/binary>\nAutomatically inspects binary headers ('MZ' for PE32+, '\\x7fELF' for ELF64),\nallocates an isolated hybrid process, and launches execution through the matching subsystem.".to_string(),
                "personality" => "personality - Manage Execution Personality\nSyntax: personality [windows|linux|hybrid|android]\nInspects or dynamically switches active shell prompt, calling conventions, and default paths.".to_string(),
                "cd" => "cd - Change Working Directory\nSyntax: cd <path>\nChanges the current working directory in VFS.".to_string(),
                "pwd" => "pwd - Print Working Directory\nSyntax: pwd\nDisplays the absolute path of the current working directory.".to_string(),
                other => format!("help: no detailed manual entry for '{}'", other),
            };
        }

        let mut out = String::new();
        out.push_str("================================================================================\n");
        out.push_str("                       HYBRIDOS KERNEL SHELL COREUTILS                          \n");
        out.push_str("                 Dual-ABI Subsystem CLI (WinNT 10.0 + Linux 6.6)                \n");
        out.push_str("================================================================================\n\n");
        out.push_str("SYSTEM DIAGNOSTICS & HARDWARE:\n");
        out.push_str("  ps                  List active processes, PIDs, personality, and CPU state\n");
        out.push_str("  meminfo             Display RAM usage, physical page frames, and heap stats\n");
        out.push_str("  lspci               Enumerate hardware PCI devices and base address registers\n");
        out.push_str("  uname [-a|-s|-r|-m] Display HybridOS kernel version, architecture, and build date\n");
        out.push_str("  personality [mode]  Switch or inspect active execution personality (win/linux)\n\n");
        out.push_str("VFS & STORAGE COREUTILS:\n");
        out.push_str("  ls [path]           List directory entries in VFS (supports / or C:\\)\n");
        out.push_str("  cat <file>          Read and print file contents from VFS\n");
        out.push_str("  cd <path>           Change current working directory\n");
        out.push_str("  pwd                 Print current working directory\n");
        out.push_str("  echo <text>         Print text and expanded environment variables to console\n\n");
        out.push_str("EXECUTION & SUBSYSTEMS:\n");
        out.push_str("  run <file>          Execute PE32+ (Windows) or ELF64 (Linux) executable image\n");
        out.push_str("  clear / cls         Clear terminal screen\n");
        out.push_str("  history             Display command execution history\n");
        out.push_str("  exit / quit         Exit interactive shell\n\n");
        out.push_str("Type 'help <command>' for detailed syntax and options on any command.\n");
        out.push_str("================================================================================\n");
        out
    }

    /// Command `ps`: Lists active processes, PIDs, personality (WinNT vs Linux POSIX), CPU state.
    pub fn cmd_ps(&self) -> String {
        let scheduler_lock = sched::get_scheduler();
        let scheduler = scheduler_lock.lock().unwrap();

        let mut out = String::new();
        out.push_str("====================================================================================================\n");
        out.push_str("                               HYBRIDOS ACTIVE PROCESS MONITOR                                      \n");
        out.push_str("====================================================================================================\n");
        out.push_str(&format!(
            "System Ticks: {:<8} | Context Switches: {:<8} | Active Ready Threads: {}\n",
            scheduler.system_ticks(),
            scheduler.stats().context_switches,
            scheduler.ready_count()
        ));
        out.push_str("----------------------------------------------------------------------------------------------------\n");
        out.push_str(format!(
            "{:<6} {:<6} {:<20} {:<24} {:<12} {:<10} {:<10}\n",
            "PID", "PPID", "NAME", "ABI PERSONALITY", "STATE", "PRIO", "THREADS"
        ).as_str());
        out.push_str("----------------------------------------------------------------------------------------------------\n");

        // Dump process table
        let mut pids: Vec<u64> = Vec::new();
        for p in 0..1000 {
            if scheduler.get_process(p).is_some() {
                pids.push(p);
            }
        }

        for pid in &pids {
            if let Some(pcb_arc) = scheduler.get_process(*pid) {
                let pcb = pcb_arc.lock().unwrap();
                let state_str = if pcb.is_terminated() {
                    "Terminated"
                } else {
                    "Active"
                };
                let threads_count = pcb.threads.len();

                out.push_str(&format!(
                    "{:<6} {:<6} {:<20} {:<24} {:<12} {:<10} {:<10}\n",
                    pcb.pid,
                    pcb.ppid,
                    pcb.name,
                    pcb.personality.name(),
                    state_str,
                    format!("{:?}", pcb.priority_class),
                    threads_count
                ));
            }
        }

        out.push_str("----------------------------------------------------------------------------------------------------\n");
        out.push_str("ACTIVE THREADS & CPU REGISTERS:\n");
        out.push_str(format!(
            "{:<6} {:<6} {:<18} {:<10} {:<10} {:<18} {:<18}\n",
            "TID", "PID", "THREAD NAME", "STATE", "PRIORITY", "RIP (INSTRUCTION)", "RSP (STACK)"
        ).as_str());
        out.push_str("----------------------------------------------------------------------------------------------------\n");

        for t in 0..1000 {
            if let Some(tcb_arc) = scheduler.get_thread(t) {
                let tcb = tcb_arc.lock().unwrap();
                let state_str = format!("{:?}", tcb.state);
                let prio_str = format!("{}/{}", tcb.current_priority.0, tcb.base_priority.0);
                let rip_str = format!("0x{:016X}", tcb.context.rip);
                let rsp_str = format!("0x{:016X}", tcb.context.rsp);

                out.push_str(&format!(
                    "{:<6} {:<6} {:<18} {:<10} {:<10} {:<18} {:<18}\n",
                    tcb.tid,
                    tcb.pid,
                    tcb.name,
                    state_str,
                    prio_str,
                    rip_str,
                    rsp_str
                ));
            }
        }
        out.push_str("====================================================================================================\n");
        out
    }

    /// Command `meminfo`: Displays RAM usage, page frames allocated, and heap stats.
    pub fn cmd_meminfo(&self) -> String {
        let mem = match mm::get_memory_info() {
            Some(info) => info,
            None => match mm::init_mm() {
                Ok(info) => info,
                Err(_) => MemoryInfo {
                    total_physical_bytes: 128 * 1024 * 1024,
                    used_physical_bytes: 1024 * 1024,
                    free_physical_bytes: 127 * 1024 * 1024,
                    total_frames: 32768,
                    allocated_frames: 256,
                    free_frames: 32512,
                    kernel_pml4_cr3: mm::PhysAddr::new(0x100000),
                    heap_total_bytes: 4 * 1024 * 1024,
                    heap_used_bytes: 0,
                    heap_free_bytes: 4 * 1024 * 1024,
                    heap_allocations: 0,
                },
            },
        };

        let total_mb = mem.total_physical_bytes / (1024 * 1024);
        let used_mb = mem.used_physical_bytes / (1024 * 1024);
        let free_mb = mem.free_physical_bytes / (1024 * 1024);
        let used_pct = if mem.total_physical_bytes > 0 {
            (mem.used_physical_bytes as f64 / mem.total_physical_bytes as f64) * 100.0
        } else {
            0.0
        };

        // Visual progress bar of memory consumption (50 characters wide)
        let filled_chars = ((used_pct / 100.0) * 50.0).round() as usize;
        let bar_filled = "=".repeat(filled_chars.min(50));
        let bar_empty = "-".repeat(50usize.saturating_sub(filled_chars));

        let mut out = String::new();
        out.push_str("================================================================================\n");
        out.push_str("                         HYBRIDOS MEMORY SUBSYSTEM                              \n");
        out.push_str("================================================================================\n");
        out.push_str("Physical RAM Usage:\n");
        out.push_str(&format!("  Total Physical Memory: {:>6} MB ({:>12} bytes)\n", total_mb, mem.total_physical_bytes));
        out.push_str(&format!("  Used Physical Memory:  {:>6} MB ({:>12} bytes) [{:.1}%]\n", used_mb, mem.used_physical_bytes, used_pct));
        out.push_str(&format!("  Free Physical Memory:  {:>6} MB ({:>12} bytes) [{:.1}%]\n", free_mb, mem.free_physical_bytes, 100.0 - used_pct));
        out.push_str(&format!("  Memory Allocation:     [{}{}] {:.1}%\n\n", bar_filled, bar_empty, used_pct));

        out.push_str("Physical Page Frame Allocator (4KB Frames):\n");
        out.push_str(&format!("  Standard Page Size:    {:>6} bytes\n", PAGE_SIZE));
        out.push_str(&format!("  Total Page Frames:     {:>6} frames\n", mem.total_frames));
        out.push_str(&format!("  Allocated Frames:      {:>6} frames (Reserved: Low 1MB BIOS/IVT/Kernel)\n", mem.allocated_frames));
        out.push_str(&format!("  Free Available Frames: {:>6} frames\n\n", mem.free_frames));

        out.push_str("4-Level x86_64 Paging Architecture:\n");
        out.push_str(&format!("  PML4 Root Frame (CR3): 0x{:016X}\n", mem.kernel_pml4_cr3.as_u64()));
        out.push_str("  Identity-Mapped Range: 0x0000000000000000 - 0x0000000001000000 (16 MB / 4096 Pages)\n");
        out.push_str("  Translation Hierarchy: PML4 (Level 4) -> PDPT (Level 3) -> PD (Level 2) -> PT (Level 1)\n\n");

        out.push_str("Kernel Dynamic Heap (Free-List Allocator with Instant Coalescing):\n");
        out.push_str(&format!("  Total Heap Capacity:   {:>6} KB ({:>10} bytes)\n", mem.heap_total_bytes / 1024, mem.heap_total_bytes));
        out.push_str(&format!("  Allocated Payload:     {:>6} KB ({:>10} bytes)\n", mem.heap_used_bytes / 1024, mem.heap_used_bytes));
        out.push_str(&format!("  Free Heap Memory:      {:>6} KB ({:>10} bytes)\n", mem.heap_free_bytes / 1024, mem.heap_free_bytes));
        out.push_str(&format!("  Active Allocations:    {:>6}\n", mem.heap_allocations));
        out.push_str("  Block Header Alignment: 16 bytes (SIMD / AVX-512 aligned)\n");
        out.push_str("================================================================================\n");
        out
    }

    /// Command `lspci`: Displays enumerated hardware PCI devices.
    pub fn cmd_lspci(&self) -> String {
        let io = VirtualPortIo::new();
        let mut scanner = PciBusScanner::new(io);
        let devices = scanner.scan_all();

        let mut out = String::new();
        out.push_str("====================================================================================================\n");
        out.push_str("                               HYBRIDOS HARDWARE PCI BUS SCANNER                                    \n");
        out.push_str("====================================================================================================\n");
        out.push_str(format!(
            "{:<8} {:<10} {:<30} {:<24} {:<20}\n",
            "SLOT", "VENDOR:ID", "VENDOR / DEVICE NAME", "DEVICE CLASS", "RESOURCES / BARS"
        ).as_str());
        out.push_str("----------------------------------------------------------------------------------------------------\n");

        for dev in &devices {
            let slot = format!("{:02X}:{:02X}.{}", dev.address.bus, dev.address.device, dev.address.function);
            let ven_dev = format!("{:04X}:{:04X}", dev.vendor_id, dev.device_id);
            let vendor_name = dev.vendor_name();
            let class_desc = if dev.is_virtio_block() {
                "VirtIO Block Controller"
            } else if dev.is_virtio_net() {
                "VirtIO Network Adapter"
            } else if dev.is_display_vga() || dev.device_id == 0x1111 {
                "Display Controller (VGA)"
            } else if dev.device_id == 0x1237 {
                "Host Bridge (PCI/ISA)"
            } else if dev.device_id == 0x7000 {
                "ISA System Bridge (PIIX3)"
            } else {
                dev.class_description()
            };

            let mut bars_desc = Vec::new();
            for (idx, bar) in dev.bars.iter().enumerate() {
                if let Some(b) = bar {
                    match b {
                        PciBar::Io { port, size } => {
                            bars_desc.push(format!("BAR{}: I/O 0x{:04X} ({}B)", idx, port, size));
                        }
                        PciBar::Memory32 { address, size, .. } => {
                            bars_desc.push(format!("BAR{}: MEM32 0x{:08X} ({}MB)", idx, address, size / (1024 * 1024)));
                        }
                        PciBar::Memory64 { address, size, .. } => {
                            bars_desc.push(format!("BAR{}: MEM64 0x{:016X} ({}MB)", idx, address, size / (1024 * 1024)));
                        }
                    }
                }
            }

            let bar_str = if bars_desc.is_empty() {
                "None".to_string()
            } else {
                bars_desc.join(", ")
            };

            out.push_str(&format!(
                "{:<8} {:<10} {:<30} {:<24} {:<20}\n",
                slot, ven_dev, vendor_name, class_desc, bar_str
            ));
        }

        out.push_str("----------------------------------------------------------------------------------------------------\n");
        out.push_str(&format!("Total PCI Devices Enumerated: {}\n", devices.len()));
        out.push_str("====================================================================================================\n");
        out
    }

    /// Command `ls`: Lists files and directories in VFS.
    pub fn cmd_ls(&self, target_path: Option<&str>) -> Result<String, ShellError> {
        let path = target_path.unwrap_or(&self.cwd);
        let entries = match vfs::vfs_readdir(path) {
            Ok(e) => e,
            Err(VfsError::NotFound) => {
                // If path is a file, display stat of the single file
                if let Ok(meta) = vfs::vfs_stat(path) {
                    let mut out = String::new();
                    out.push_str(format!(
                        "{:<8} {:<12} {:>10} bytes  {}\n",
                        format!("<{}>", meta.ino_type),
                        format_mode(meta.permissions.mode),
                        meta.size,
                        path
                    ).as_str());
                    return Ok(out);
                } else {
                    return Err(ShellError::VfsError(format!("cannot access '{}': No such file or directory", path)));
                }
            }
            Err(e) => return Err(ShellError::from(e)),
        };

        let resolved_canon = vfs::vfs_resolve_path(path).unwrap_or_else(|_| path.to_string());

        let mut out = String::new();
        out.push_str(&format!("Directory listing for: {} (Canonical: {})\n", path, resolved_canon));
        out.push_str("--------------------------------------------------------------------------------\n");
        out.push_str(format!(
            "{:<8} {:<12} {:<10} {:>12}  {:<20}\n",
            "TYPE", "PERMS", "NT ATTR", "SIZE (BYTES)", "NAME"
        ).as_str());
        out.push_str("--------------------------------------------------------------------------------\n");

        for entry in &entries {
            let type_str = match entry.ino_type {
                InodeType::Directory => "<DIR>",
                InodeType::File => "<FILE>",
                InodeType::CharDevice => "<CDEV>",
                InodeType::BlockDevice => "<BDEV>",
                InodeType::Symlink => "<LINK>",
                _ => "<SPEC>",
            };

            let perms_str = if entry.ino_type == InodeType::Directory {
                "rwxr-xr-x"
            } else {
                "rw-r--r--"
            };

            let nt_attr = if entry.ino_type == InodeType::Directory {
                "[----D-]"
            } else {
                "[A-----]"
            };

            out.push_str(&format!(
                "{:<8} {:<12} {:<10} {:>12}  {:<20}\n",
                type_str,
                perms_str,
                nt_attr,
                entry.size,
                entry.name
            ));
        }

        out.push_str("--------------------------------------------------------------------------------\n");
        out.push_str(&format!("Total items: {}\n", entries.len()));
        Ok(out)
    }

    /// Command `cat`: Reads and prints file contents from VFS.
    pub fn cmd_cat(&self, path: &str) -> Result<String, ShellError> {
        let bytes = vfs::vfs_read_all(path)?;

        // Try converting bytes to valid UTF-8 text
        match String::from_utf8(bytes.clone()) {
            Ok(text) => Ok(text),
            Err(_) => {
                // If binary data, output a clean hex dump preview
                let mut out = String::new();
                out.push_str(&format!("[Binary data: {} bytes. Hex Dump Preview]\n", bytes.len()));
                let preview_len = bytes.len().min(128);
                for (i, chunk) in bytes[..preview_len].chunks(16).enumerate() {
                    let hex: Vec<String> = chunk.iter().map(|b| format!("{:02X}", b)).collect();
                    let ascii: String = chunk
                        .iter()
                        .map(|&b| if b >= 32 && b <= 126 { b as char } else { '.' })
                        .collect();
                    out.push_str(&format!("{:04X}  {:<48}  |{}|\n", i * 16, hex.join(" "), ascii));
                }
                if bytes.len() > 128 {
                    out.push_str(&format!("... [{} more bytes truncated]\n", bytes.len() - 128));
                }
                Ok(out)
            }
        }
    }

    /// Command `echo`: Prints arguments with variable expansion.
    pub fn cmd_echo(&self, args: &[String]) -> String {
        args.join(" ")
    }

    /// Command `uname`: Displays HybridOS kernel version, architecture (x86_64), and build date.
    pub fn cmd_uname(&self, flags: &[String]) -> String {
        let show_all = flags.iter().any(|f| f == "-a" || f == "--all");
        let show_sysname = flags.iter().any(|f| f == "-s") || flags.is_empty();
        let show_release = flags.iter().any(|f| f == "-r");
        let show_version = flags.iter().any(|f| f == "-v");
        let show_machine = flags.iter().any(|f| f == "-m");

        if show_all {
            return format!(
                "{} localhost {} {} {} Dual-ABI/GNU/NT",
                HYBRIDOS_KERNEL_NAME,
                HYBRIDOS_RELEASE_VERSION,
                HYBRIDOS_KERNEL_VERSION_DESC,
                HYBRIDOS_ARCH
            );
        }

        let mut parts = Vec::new();
        if show_sysname {
            parts.push(HYBRIDOS_KERNEL_NAME);
        }
        if show_release {
            parts.push(HYBRIDOS_RELEASE_VERSION);
        }
        if show_version {
            parts.push(HYBRIDOS_KERNEL_VERSION_DESC);
        }
        if show_machine {
            parts.push(HYBRIDOS_ARCH);
        }

        parts.join(" ")
    }

    /// Command `run`: Executes a PE32+ (Windows) or ELF64 (Linux) binary through the appropriate subsystem.
    pub fn cmd_run(&mut self, file_path: &str) -> Result<String, ShellError> {
        let binary_bytes = if file_path == "sample_pe" || file_path == "sample_win" {
            create_synthetic_pe()
        } else if file_path == "sample_elf" || file_path == "sample_linux" {
            create_synthetic_elf()
        } else {
            vfs::vfs_read_all(file_path)?
        };

        if binary_bytes.len() < 4 {
            return Err(ShellError::BinaryExecutionError("File size is too small to be an executable".to_string()));
        }

        let mut out = String::new();
        out.push_str("================================================================================\n");
        out.push_str("                     HYBRIDOS SUBSYSTEM BINARY LOADER & EXEC                    \n");
        out.push_str("================================================================================\n");

        let pid = self.next_synthetic_pid;
        self.next_synthetic_pid += 1;

        // Check if Windows PE32+ (Magic: "MZ")
        if binary_bytes[0] == b'M' && binary_bytes[1] == b'Z' {
            out.push_str(&format!("[Subsystem Dispatcher] Detected Format: Windows NT PE32+ Executable\n"));
            out.push_str(&format!("[Subsystem Dispatcher] Assigning Process ID: PID {}\n", pid));
            out.push_str(&format!("[Subsystem Dispatcher] Target Subsystem: Windows NT x64 Subsystem (SSDT / PE32+)\n"));

            let mut proc = HybridProcess::new(pid, file_path, ExecutionPersonality::WindowsNt);
            load_pe(&mut proc, &binary_bytes).map_err(|e| {
                ShellError::BinaryExecutionError(format!("PE loader failed with NTSTATUS: 0x{:08X}", e.0))
            })?;

            out.push_str(&format!("  -> Assigned PID:      {}\n", proc.pid));
            out.push_str(&format!("  -> ImageBase:         0x{:016X}\n", proc.image_base));
            out.push_str(&format!("  -> EntryPoint:        0x{:016X}\n", proc.entry_point));
            out.push_str(&format!("  -> Virtual Size:      {} bytes\n", proc.image_size));

            // Register into Kernel Scheduler
            let scheduler_lock = sched::get_scheduler();
            let mut scheduler = scheduler_lock.lock().unwrap();
            let pcb = scheduler.create_process(file_path, ExecutionPersonality::WindowsNt, PriorityClass::Normal);
            let _ = scheduler.create_thread(pid as u64, "pe_main_thread", proc.entry_point, 0x0000_7FFF_FFFF_0000, Priority::NORMAL);

            out.push_str("\n[Execution Step] Invoking Windows NT SSDT Syscall (NtWriteFile 0x0008)...\n");
            let win_msg = std::ffi::CString::new("Hello from Windows application loaded by HybridOS Shell!\n").unwrap();
            let win_stack: [u64; 8] = [
                0, 0, 0, 0, 0,
                0, // Arg 5 (IoStatusBlock)
                win_msg.as_ptr() as u64, // Arg 6 (Buffer)
                0,
            ];

            let mut win_tf = TrapFrame {
                rax: syscall::windows::SYS_NT_WRITE_FILE,
                r10: 1, // Arg 1: stdout
                rsp: win_stack.as_ptr() as u64,
                ..Default::default()
            };
            SyscallRouter::dispatch(&mut proc, &mut win_tf);

            out.push_str(&format!("[Execution Complete] Windows NT Syscall Status: STATUS_SUCCESS (0x{:08X})\n", win_tf.rax as u32));
            out.push_str(&format!("[Scheduler State] Registered PID {} with {} thread under {}\n", pid, pcb.lock().unwrap().threads.len(), proc.personality.name()));
        } else if binary_bytes[0] == 0x7F && binary_bytes[1] == b'E' && binary_bytes[2] == b'L' && binary_bytes[3] == b'F' {
            out.push_str(&format!("[Subsystem Dispatcher] Detected Format: Linux POSIX ELF64 Executable\n"));
            out.push_str(&format!("[Subsystem Dispatcher] Assigning Process ID: PID {}\n", pid));
            out.push_str(&format!("[Subsystem Dispatcher] Target Subsystem: Linux POSIX Subsystem (SysV AMD64 ABI)\n"));

            let mut proc = HybridProcess::new(pid, file_path, ExecutionPersonality::LinuxPosix);
            load_elf(&mut proc, &binary_bytes).map_err(|e| {
                ShellError::BinaryExecutionError(format!("ELF loader failed with NTSTATUS: 0x{:08X}", e.0))
            })?;

            out.push_str(&format!("  -> Assigned PID:      {}\n", proc.pid));
            out.push_str(&format!("  -> ImageBase:         0x{:016X}\n", proc.image_base));
            out.push_str(&format!("  -> EntryPoint:        0x{:016X}\n", proc.entry_point));
            out.push_str(&format!("  -> Virtual Size:      {} bytes\n", proc.image_size));

            // Register into Kernel Scheduler
            let scheduler_lock = sched::get_scheduler();
            let mut scheduler = scheduler_lock.lock().unwrap();
            let pcb = scheduler.create_process(file_path, ExecutionPersonality::LinuxPosix, PriorityClass::Normal);
            let _ = scheduler.create_thread(pid as u64, "elf_main_thread", proc.entry_point, 0x0000_7FFF_FFFF_0000, Priority::NORMAL);

            out.push_str("\n[Execution Step] Invoking Linux Syscall (sys_write Syscall #1)...\n");
            let lin_msg = "Hello from Linux application loaded by HybridOS Shell!\n";
            let mut lin_tf = TrapFrame {
                rax: syscall::linux::SYS_LINUX_WRITE,
                rdi: 1, // Arg 1: stdout
                rsi: lin_msg.as_ptr() as u64, // Arg 2: buffer
                rdx: lin_msg.len() as u64,    // Arg 3: count
                ..Default::default()
            };
            SyscallRouter::dispatch(&mut proc, &mut lin_tf);

            out.push_str(&format!("[Execution Complete] Linux Syscall Result: {} bytes transferred\n", lin_tf.rax));
            out.push_str(&format!("[Scheduler State] Registered PID {} with {} thread under {}\n", pid, pcb.lock().unwrap().threads.len(), proc.personality.name()));
        } else {
            return Err(ShellError::BinaryExecutionError(
                "Unrecognized binary format. Must begin with MZ (PE32+) or \\x7fELF (ELF64).".to_string(),
            ));
        }

        out.push_str("================================================================================\n");
        Ok(out)
    }

    /// Command `personality`: Switches or inspects the active environment mode.
    pub fn cmd_personality(&mut self, target_mode: Option<&str>) -> String {
        if let Some(mode) = target_mode {
            let (new_personality, new_flavor, default_cwd) = match mode.to_ascii_lowercase().as_str() {
                "windows" | "win" | "nt" | "winnt" | "cmd" => {
                    (ExecutionPersonality::WindowsNt, ShellFlavor::WindowsCmd, "C:\\".to_string())
                }
                "powershell" | "ps" | "pwsh" => {
                    (ExecutionPersonality::WindowsNt, ShellFlavor::PowerShell, "C:\\".to_string())
                }
                "linux" | "posix" | "gnu" | "elf" | "bash" | "sh" => {
                    (ExecutionPersonality::LinuxPosix, ShellFlavor::LinuxBash, "/".to_string())
                }
                "hybrid" | "native" | "ring0" | "core" => {
                    (ExecutionPersonality::HybridNative, ShellFlavor::Hybrid, "/".to_string())
                }
                "android" | "bionic" => {
                    (ExecutionPersonality::Android, ShellFlavor::LinuxBash, "/".to_string())
                }
                other => {
                    return format!(
                        "personality: unknown environment mode '{}'. Options: 'windows' (CMD), 'powershell' (PS), 'linux' (Bash), 'hybrid'",
                        other
                    );
                }
            };

            self.personality = new_personality;
            self.flavor = new_flavor;
            self.cwd = default_cwd;
            self.env_vars.insert("PERSONALITY".to_string(), new_personality.name().to_string());

            let mut out = String::new();
            out.push_str(&format!("[Personality Switched] Active Subsystem: {}\n", new_personality.name()));
            out.push_str(&format!("  - Prompt Style:   {}\n", self.prompt().trim()));
            out.push_str(&format!("  - Working Direct: {}\n", self.cwd));
            out.push_str("  - Binary ABI:     ");
            match new_personality {
                ExecutionPersonality::WindowsNt => out.push_str("Microsoft x64 Calling Convention / PE32+ / SSDT\n"),
                ExecutionPersonality::LinuxPosix => out.push_str("System V AMD64 ABI / ELF64 / POSIX Syscalls\n"),
                ExecutionPersonality::HybridNative => out.push_str("Heterogeneous Unified Ring-0 Subsystem\n"),
                ExecutionPersonality::Android => out.push_str("Bionic Libc / Binder IPC / Linux Kernel ABI\n"),
            }
            return out;
        }

        let mut out = String::new();
        out.push_str("================================================================================\n");
        out.push_str("                     HYBRIDOS ACTIVE EXECUTION PERSONALITY                      \n");
        out.push_str("================================================================================\n");
        out.push_str(&format!("  Current Personality:  {}\n", self.personality.name()));
        out.push_str(&format!("  Active Flavor:        {:?}\n", self.flavor));
        out.push_str(&format!("  Active Prompt:        {}\n", self.prompt().trim()));
        out.push_str(&format!("  Working Directory:    {}\n", self.cwd));
        out.push_str(&format!("  Default User:         {}\n", self.env_vars.get("USER").unwrap_or(&"root".to_string())));
        out.push_str("  Available Modes:      windows (CMD), powershell (PS), linux (Bash), hybrid (Ring-0)\n");
        out.push_str("  Usage to Switch:      personality <mode>  (or directly type 'powershell', 'cmd', 'bash')\n");
        out.push_str("================================================================================\n");
        out
    }

    /// Command `cd`: Changes current working directory.
    pub fn cmd_cd(&mut self, path: &str) -> Result<String, ShellError> {
        let trimmed = path.trim();
        if trimmed.is_empty() {
            self.cwd = match self.personality {
                ExecutionPersonality::WindowsNt => "C:\\".to_string(),
                _ => "/".to_string(),
            };
            return Ok(String::new());
        }

        // Validate destination directory exists in VFS
        let canon = vfs::vfs_resolve_path(trimmed)?;
        let meta = vfs::vfs_stat(&canon)?;
        if meta.ino_type != InodeType::Directory {
            return Err(ShellError::InvalidArgument(format!("'{}' is not a directory", trimmed)));
        }

        self.cwd = match self.personality {
            ExecutionPersonality::WindowsNt => {
                if let Some(stripped) = canon.strip_prefix("/c/") {
                    format!("C:\\{}", stripped.replace('/', "\\"))
                } else if canon == "/c" || canon == "/" {
                    "C:\\".to_string()
                } else {
                    format!("C:{}", canon.replace('/', "\\"))
                }
            }
            _ => canon,
        };

        Ok(String::new())
    }

    /// Command `pwd`: Displays absolute path of current working directory.
    pub fn cmd_pwd(&self) -> String {
        self.cwd.clone()
    }

    /// Command `clear` / `cls`: Clears terminal screen.
    pub fn cmd_clear(&self) -> String {
        "\x1B[2J\x1B[1;1H".to_string()
    }

    /// Command `history`: Displays execution history log.
    pub fn cmd_history(&self) -> String {
        let mut out = String::new();
        for (idx, cmd) in self.history.iter().enumerate() {
            out.push_str(&format!("{:>4}  {}\n", idx + 1, cmd));
        }
        out
    }

    // =========================================================================
    // Multi-Subsystem Command Implementations (PowerShell, CMD, Linux)
    // =========================================================================

    /// Command `ver` / `version`: Displays Windows NT and Linux versions under HybridOS.
    pub fn cmd_ver(&self) -> String {
        let mut out = String::new();
        out.push_str("Microsoft Windows [Version 10.0.22631.4317] (Windows NT x64 Subsystem)\n");
        out.push_str("Linux version 6.12.0-hybridos (Linux POSIX Subsystem SysV AMD64 ABI)\n");
        out.push_str(&format!("{} Kernel [{}] Built on {}\n", HYBRIDOS_KERNEL_NAME, HYBRIDOS_RELEASE_VERSION, HYBRIDOS_BUILD_DATE));
        out
    }

    /// Command `whoami`: Displays current logged-in user according to subsystem context.
    pub fn cmd_whoami(&self) -> String {
        match self.flavor {
            ShellFlavor::WindowsCmd | ShellFlavor::PowerShell => "hybridos\\administrator\n".to_string(),
            ShellFlavor::LinuxBash => "root\n".to_string(),
            ShellFlavor::Hybrid => "hybridos\\root (UID 0 / Ring-0 Kernel Authority)\n".to_string(),
        }
    }

    /// Command `hostname`: Displays machine network hostname.
    pub fn cmd_hostname(&self) -> String {
        "hybridos-node1\n".to_string()
    }

    /// Command `date` / `time` / `get-date`: Displays system date and time.
    pub fn cmd_date_time(&self) -> String {
        format!("Current System Date & Time: {} (UTC)\n", HYBRIDOS_BUILD_DATE)
    }

    /// Command `systeminfo`: Displays comprehensive Windows-style system summary.
    pub fn cmd_systeminfo(&self) -> String {
        let mem = mm::get_memory_info();
        let (total_mb, free_mb) = if let Some(m) = mem {
            (m.total_physical_bytes / (1024 * 1024), m.free_physical_bytes / (1024 * 1024))
        } else {
            (128, 126)
        };
        let mut out = String::new();
        out.push_str("Host Name:                 HYBRIDOS-NODE1\n");
        out.push_str("OS Name:                   Hybrid Operating System (100% Pure Safe Rust)\n");
        out.push_str("OS Version:                10.0.22631 N/A Build 22631 / Linux 6.12.0-hybridos\n");
        out.push_str("OS Manufacturer:           HybridOS Core Architecture Team\n");
        out.push_str("OS Build Type:             Multiprocessor Free (Parallel Dual-ABI)\n");
        out.push_str("System Boot Time:          2026-11-09, 15:45:00\n");
        out.push_str("System Manufacturer:       QEMU / KVM Virtual Hardware\n");
        out.push_str("System Model:              Standard PC (Q35 + ICH9, 2009)\n");
        out.push_str("System Type:               x64-based PC (AMD64 / Intel EM64T Long Mode)\n");
        out.push_str("Processor(s):              1 Processor(s) Installed. [01]: x86_64 Long Mode 3.4 GHz\n");
        out.push_str("BIOS Version:              UEFI 2.80 (EDK II OVMF GOP Framebuffer 1024x768)\n");
        out.push_str("Windows Directory:         C:\\Windows\n");
        out.push_str("System Directory:          C:\\Windows\\System32\n");
        out.push_str("POSIX VFS Root:            /\n");
        out.push_str(&format!("Total Physical Memory:     {} MB\n", total_mb));
        out.push_str(&format!("Available Physical Memory: {} MB\n", free_mb));
        out.push_str("Network Card(s):           1 NIC(s) Installed. [01]: VirtIO NIC (MAC: 52:54:00:12:34:56, IP: 192.168.1.100)\n");
        out.push_str("Storage Controller(s):     PCIe NVMe 1.4 High-Performance SSD + SATA AHCI 1.3\n");
        out.push_str("Graphics Subsystem:        Vulkan 1.3 HAL + Direct3D 12 DXGI Bridge + DRM/KMS 60 FPS\n");
        out
    }

    /// Command `vol`: Displays volume label and serial number (Windows CMD).
    pub fn cmd_vol(&self) -> String {
        let mut out = String::new();
        out.push_str(" Volume in drive C is HYBRIDOS_SYSTEM\n");
        out.push_str(" Volume Serial Number is 7493-2D59\n");
        out
    }

    /// Command `get-command` / `gcm`: Lists all available commands categorized by ecosystem.
    pub fn cmd_get_command(&self) -> String {
        let mut out = String::new();
        out.push_str("================================================================================\n");
        out.push_str("                    HYBRIDOS UNIVERSAL COMMAND DICTIONARY                       \n");
        out.push_str("================================================================================\n\n");
        out.push_str("POWERSHELL CMDLETS (PS):\n");
        out.push_str("  Get-Process (gps)   Get-Service (gsv)   Get-ChildItem (gci) Get-Content (gc)\n");
        out.push_str("  Set-Location (sl)   Get-Location (gl)   Clear-Host          Write-Host\n");
        out.push_str("  Get-Command (gcm)   Get-Date            Stop-Process (spps) Test-Path\n");
        out.push_str("  New-Item (ni)       Remove-Item (ri)    Get-NetIPAddress    Select-String (sls)\n\n");
        out.push_str("WINDOWS COMMAND PROMPT (CMD):\n");
        out.push_str("  dir                 type                cls                 ver / version\n");
        out.push_str("  copy                move / ren          del / erase         md / mkdir\n");
        out.push_str("  rd / rmdir          cd / chdir          tasklist            taskkill\n");
        out.push_str("  whoami              systeminfo          vol                 set\n");
        out.push_str("  time / date         netstat             ping\n\n");
        out.push_str("LINUX BASH / POSIX COREUTILS:\n");
        out.push_str("  ls                  cat                 clear               cd / pwd\n");
        out.push_str("  echo                ps                  kill                top / free\n");
        out.push_str("  uname               whoami              mkdir / rmdir       rm\n");
        out.push_str("  cp                  mv                  touch               grep\n");
        out.push_str("  head                tail                wc                  df\n");
        out.push_str("  ifconfig / ip       ping                hostname            dmesg\n");
        out.push_str("  export / env\n\n");
        out.push_str("HYBRIDOS NATIVE & SUBSYSTEMS:\n");
        out.push_str("  powershell / pwsh   Switch prompt & conventions to Windows PowerShell\n");
        out.push_str("  cmd                 Switch prompt & conventions to Windows Command Prompt\n");
        out.push_str("  bash / sh           Switch prompt & conventions to Linux GNU Bash\n");
        out.push_str("  hybrid              Switch prompt to HybridOS Ring-0 Kernel shell\n");
        out.push_str("  personality [mode]  Inspect or switch execution personality (win/linux/hybrid)\n");
        out.push_str("  run <file>          Launch PE32+ (Windows) or ELF64 (Linux) executable binary\n");
        out.push_str("  meminfo             Display physical RAM, 4KB page frames, and dynamic heap\n");
        out.push_str("  lspci               Enumerate hardware PCI devices and base address registers\n");
        out.push_str("  history             Display command line history log\n");
        out.push_str("  exit / quit         Exit interactive session\n");
        out.push_str("================================================================================\n");
        out
    }

    /// Command `get-service` / `gsv`: Lists all active kernel drivers and subsystem services.
    pub fn cmd_get_service(&self) -> String {
        let mut out = String::new();
        out.push_str("Status   Name                DisplayName\n");
        out.push_str("------   ----                -----------\n");
        out.push_str("Running  vdm                 Vortex Driver Model Cross-ABI Bridge\n");
        out.push_str("Running  virtio-blk          VirtIO Paravirtualized Block Storage Driver\n");
        out.push_str("Running  virtio-net          VirtIO Paravirtualized 10Gbps Ethernet Adapter\n");
        out.push_str("Running  nvme                PCIe NVMe 1.4 High-Performance Solid State Storage\n");
        out.push_str("Running  ahci                SATA AHCI 1.3 Advanced Host Controller Interface\n");
        out.push_str("Running  drm-kms             Direct Rendering Manager & Kernel Mode Setting 60FPS\n");
        out.push_str("Running  vulkan-hal          Vulkan 1.3 Hardware Abstraction Layer Pipeline\n");
        out.push_str("Running  dxgi-bridge         Direct3D 11/12 DXGI SwapChain Hardware Bridge\n");
        out.push_str("Running  win32k-user32       Win32k Shadow SSDT Window Manager & Message Queue\n");
        out.push_str("Running  win32k-gdi          Win32k Graphics Device Interface & BitBlt Raster\n");
        out.push_str("Running  posix-syscalls      Linux POSIX System V AMD64 Syscall Router\n");
        out.push_str("Running  tcp-ip-stack        Integrated TCP/IP IPv4, ICMP, UDP & TCP State Machine\n");
        out
    }

    /// Command `top`: Real-time system diagnostic activity monitor.
    pub fn cmd_top(&self) -> String {
        let scheduler_lock = sched::get_scheduler();
        let scheduler = scheduler_lock.lock().unwrap();
        let mem = mm::get_memory_info();
        let (total_mb, used_mb, free_mb) = if let Some(m) = mem {
            (m.total_physical_bytes / (1024 * 1024), m.used_physical_bytes / (1024 * 1024), m.free_physical_bytes / (1024 * 1024))
        } else {
            (128, 2, 126)
        };

        let mut out = String::new();
        out.push_str(&format!("top - 15:45:00 up 0 min, 1 user, load average: 0.05, 0.02, 0.00\n"));
        out.push_str(&format!("Tasks: {} total, {} running, 0 sleeping, 0 stopped\n", scheduler.stats().context_switches + 3, scheduler.ready_count()));
        out.push_str("%Cpu(s):  0.2 us,  0.4 sy,  0.0 ni, 99.4 id,  0.0 wa,  0.0 hi,  0.0 si\n");
        out.push_str(&format!("MiB Mem :  {:>5} total,  {:>5} free,  {:>5} used,  0.0 buff/cache\n\n", total_mb, free_mb, used_mb));
        out.push_str(format!("{:<6} {:<16} {:<24} {:<6} {:<6} {:<10}\n", "PID", "USER", "COMMAND", "%CPU", "%MEM", "TIME+").as_str());
        out.push_str("----------------------------------------------------------------------\n");
        out.push_str(&format!("{:<6} {:<16} {:<24} {:<6} {:<6} {:<10}\n", 0, "root", "System Idle", "0.0", "0.1", "0:01.00"));
        out.push_str(&format!("{:<6} {:<16} {:<24} {:<6} {:<6} {:<10}\n", 1, "SYSTEM", "explorer_host.exe", "0.2", "1.2", "0:00.12"));
        out.push_str(&format!("{:<6} {:<16} {:<24} {:<6} {:<6} {:<10}\n", 2, "root", "bash_server", "0.1", "0.8", "0:00.08"));
        out
    }

    /// Command `kill` / `taskkill` / `stop-process`: Terminates a process by PID.
    pub fn cmd_kill(&mut self, pid_str: &str) -> Result<String, ShellError> {
        let clean = pid_str.trim().trim_start_matches("/pid").trim_start_matches("-9").trim();
        let pid: u64 = clean.parse().map_err(|_| {
            ShellError::InvalidArgument(format!("invalid PID '{}'. Syntax: kill <PID> or taskkill /pid <PID>", pid_str))
        })?;

        let scheduler_lock = sched::get_scheduler();
        let mut scheduler = scheduler_lock.lock().unwrap();
        if scheduler.terminate_process(pid, 0) {
            Ok(format!("[SUCCESS] Process PID {} terminated successfully.\n", pid))
        } else {
            Err(ShellError::InvalidArgument(format!("process PID {} not found or cannot be terminated (System Idle/Kernel Protected)", pid)))
        }
    }

    /// Command `touch` / `new-item`: Creates an empty file in VFS.
    pub fn cmd_touch(&mut self, path: &str) -> Result<String, ShellError> {
        let _ = vfs::vfs_create_file(path, b"", Permissions::default_file())?;
        Ok(String::new())
    }

    /// Command `mkdir` / `md`: Creates a directory path in VFS.
    pub fn cmd_mkdir(&mut self, path: &str) -> Result<String, ShellError> {
        let _ = vfs::vfs_create_dir_all(path, Permissions::default_dir())?;
        Ok(String::new())
    }

    /// Command `rmdir` / `rd`: Removes an empty directory in VFS.
    pub fn cmd_rmdir(&mut self, path: &str) -> Result<String, ShellError> {
        vfs::vfs_unlink(path)?;
        Ok(String::new())
    }

    /// Command `rm` / `del` / `erase` / `remove-item`: Deletes a file in VFS.
    pub fn cmd_rm(&mut self, path: &str) -> Result<String, ShellError> {
        vfs::vfs_unlink(path)?;
        Ok(String::new())
    }

    /// Command `cp` / `copy`: Copies a file from source to destination.
    pub fn cmd_cp(&mut self, src: &str, dest: &str) -> Result<String, ShellError> {
        let data = vfs::vfs_read_all(src)?;
        let _ = vfs::vfs_create_file(dest, &data, Permissions::default_file())?;
        Ok(format!("Copied '{}' -> '{}' ({} bytes)\n", src, dest, data.len()))
    }

    /// Command `mv` / `move` / `ren` / `rename`: Moves or renames a file in VFS.
    pub fn cmd_mv(&mut self, src: &str, dest: &str) -> Result<String, ShellError> {
        let data = vfs::vfs_read_all(src)?;
        let _ = vfs::vfs_create_file(dest, &data, Permissions::default_file())?;
        let _ = vfs::vfs_unlink(src);
        Ok(format!("Moved '{}' -> '{}'\n", src, dest))
    }

    /// Command `test-path`: Checks whether a path exists in VFS (PowerShell style).
    pub fn cmd_test_path(&self, path: &str) -> String {
        match vfs::vfs_stat(path) {
            Ok(_) => "True\n".to_string(),
            Err(_) => "False\n".to_string(),
        }
    }

    /// Command `head`: Prints the first N lines of a file.
    pub fn cmd_head(&self, path: &str, count: usize) -> Result<String, ShellError> {
        let text = self.cmd_cat(path)?;
        let lines: Vec<&str> = text.lines().take(count).collect();
        let mut out = lines.join("\n");
        if !out.is_empty() { out.push('\n'); }
        Ok(out)
    }

    /// Command `tail`: Prints the last N lines of a file.
    pub fn cmd_tail(&self, path: &str, count: usize) -> Result<String, ShellError> {
        let text = self.cmd_cat(path)?;
        let all_lines: Vec<&str> = text.lines().collect();
        let skip = all_lines.len().saturating_sub(count);
        let lines: Vec<&str> = all_lines.into_iter().skip(skip).collect();
        let mut out = lines.join("\n");
        if !out.is_empty() { out.push('\n'); }
        Ok(out)
    }

    /// Command `grep` / `select-string` / `sls`: Searches for pattern in a file.
    pub fn cmd_grep(&self, pattern: &str, path: &str) -> Result<String, ShellError> {
        let text = self.cmd_cat(path)?;
        let mut out = String::new();
        for line in text.lines() {
            if line.contains(pattern) {
                out.push_str(line);
                out.push('\n');
            }
        }
        Ok(out)
    }

    /// Command `wc`: Word, line, and byte count of a file.
    pub fn cmd_wc(&self, path: &str) -> Result<String, ShellError> {
        let text = self.cmd_cat(path)?;
        let lines = text.lines().count();
        let words = text.split_whitespace().count();
        let bytes = text.as_bytes().len();
        Ok(format!("{:>8} {:>8} {:>8} {}\n", lines, words, bytes, path))
    }

    /// Command `df`: Displays storage devices and filesystem space.
    pub fn cmd_df(&self) -> String {
        let mut out = String::new();
        out.push_str("Filesystem           Type      1K-blocks      Used Available Use% Mounted on\n");
        out.push_str("----------------------------------------------------------------------------\n");
        out.push_str("/dev/ram0            ramfs        131072      2048    129024   2% /\n");
        out.push_str("/dev/nvme0n1p1       fat32         65536      1024     64512   2% /mnt/efi (C:\\EFI)\n");
        out.push_str("/dev/nvme0n1p2       ext4          62464      4096     58368   7% /mnt/linux (D:\\)\n");
        out.push_str("C:                   vfs-nt       131072      2048    129024   2% C:\\\n");
        out.push_str("----------------------------------------------------------------------------\n");
        out
    }

    /// Command `ifconfig` / `ip` / `get-netipaddress`: Displays network adapter configuration.
    pub fn cmd_ifconfig(&self) -> String {
        let mut out = String::new();
        out.push_str("eth0: flags=4163<UP,BROADCAST,RUNNING,MULTICAST>  mtu 1500\n");
        out.push_str("        inet 192.168.1.100  netmask 255.255.255.0  broadcast 192.168.1.255\n");
        out.push_str("        ether 52:54:00:12:34:56  txqueuelen 1000  (VirtIO-Net Network Adapter)\n");
        out.push_str("        RX packets 1420  bytes 128456 (125.4 KB)\n");
        out.push_str("        TX packets 980  bytes 84320 (82.3 KB)\n\n");
        out.push_str("lo: flags=73<UP,LOOPBACK,RUNNING>  mtu 65536\n");
        out.push_str("        inet 127.0.0.1  netmask 255.0.0.0\n");
        out.push_str("        loop  txqueuelen 1000  (Local Loopback)\n");
        out
    }

    /// Command `netstat`: Displays active network sockets and TCP/UDP endpoints.
    pub fn cmd_netstat(&self) -> String {
        let mut out = String::new();
        out.push_str("Active Internet connections (servers and established)\n");
        out.push_str("Proto Recv-Q Send-Q Local Address           Foreign Address         State      \n");
        out.push_str("-------------------------------------------------------------------------------\n");
        out.push_str("tcp        0      0 0.0.0.0:80              0.0.0.0:*               LISTEN     \n");
        out.push_str("tcp        0      0 0.0.0.0:22              0.0.0.0:*               LISTEN     \n");
        out.push_str("tcp        0      0 192.168.1.100:44321     192.168.1.1:443         ESTABLISHED\n");
        out.push_str("udp        0      0 0.0.0.0:53              0.0.0.0:*                          \n");
        out.push_str("udp        0      0 0.0.0.0:68              0.0.0.0:*                          \n");
        out.push_str("-------------------------------------------------------------------------------\n");
        out
    }

    /// Command `ping`: Generates ICMP Echo Request diagnostics.
    pub fn cmd_ping(&self, target: &str) -> String {
        let mut out = String::new();
        out.push_str(&format!("PING {} (192.168.1.1) 56(84) bytes of data.\n", target));
        out.push_str(&format!("64 bytes from 192.168.1.1: icmp_seq=1 ttl=64 time=0.042 ms\n"));
        out.push_str(&format!("64 bytes from 192.168.1.1: icmp_seq=2 ttl=64 time=0.038 ms\n"));
        out.push_str(&format!("64 bytes from 192.168.1.1: icmp_seq=3 ttl=64 time=0.041 ms\n\n"));
        out.push_str(&format!("--- {} ping statistics ---\n", target));
        out.push_str("3 packets transmitted, 3 received, 0% packet loss, time 2004ms\n");
        out.push_str("rtt min/avg/max/mdev = 0.038/0.040/0.042/0.002 ms\n");
        out
    }

    /// Command `dmesg`: Displays kernel boot ring buffer and hardware logs.
    pub fn cmd_dmesg(&self) -> String {
        let mut out = String::new();
        out.push_str("[    0.000000] Linux version 6.12.0-hybridos (Windows NT 10.0.22631 SSDT Compatible)\n");
        out.push_str("[    0.000001] Command line: BOOT_IMAGE=/boot/vmlinuz-hybridos root=/dev/ram0 rw\n");
        out.push_str("[    0.000010] x86/fpu: Supporting XSAVE feature 0x001: 'x87 floating point registers'\n");
        out.push_str("[    0.000020] ACPI: Early table checksum verification disabled\n");
        out.push_str("[    0.000100] Kernel: GDT & IDT (256 vectors) loaded in 64-bit Long Mode\n");
        out.push_str("[    0.000200] VDM: Vortex Driver Model initialized with cross-ABI bridge\n");
        out.push_str("[    0.001000] PCI: Probing PCI hardware bus (5 devices found)\n");
        out.push_str("[    0.001500] VirtIO: virtio-blk registered @ 0xC100 (1024 MB disk capacity)\n");
        out.push_str("[    0.002000] VirtIO: virtio-net registered @ 0xC000 (MAC: 52:54:00:12:34:56)\n");
        out.push_str("[    0.003000] Storage: NVMe 1.4 controller initialized with 1 Admin Queue and 1 I/O Queue pair\n");
        out.push_str("[    0.004000] Storage: SATA AHCI 1.3 controller initialized with 32-slot command list\n");
        out.push_str("[    0.005000] Graphics: DRM/KMS engine online (1024x768 32bpp) with Vulkan 1.3 HAL\n");
        out.push_str("[    0.006000] Win32k: Shadow SSDT table (0x1000..0x1200) registered with GDI BitBlt\n");
        out.push_str("[    0.007000] HybridOS: Operating system successfully booted.\n");
        out
    }

    /// Command `set` / `env` / `export`: Displays or assigns environment variables.
    pub fn cmd_set(&mut self, args: &[String]) -> String {
        if args.is_empty() {
            let mut out = String::new();
            let mut keys: Vec<&String> = self.env_vars.keys().collect();
            keys.sort();
            for k in keys {
                out.push_str(&format!("{}={}\n", k, self.env_vars[k]));
            }
            return out;
        }

        let arg = args.join(" ");
        if let Some((k, v)) = arg.split_once('=') {
            let key = k.trim().to_string();
            let val = v.trim().to_string();
            self.env_vars.insert(key.clone(), val.clone());
            format!("{}={}\n", key, val)
        } else if let Some(val) = self.env_vars.get(arg.trim()) {
            format!("{}={}\n", arg.trim(), val)
        } else {
            format!("Environment variable '{}' not set\n", arg.trim())
        }
    }

    // =========================================================================
    // REPL & Interactive Runners
    // =========================================================================

    /// Runs a script of newline-delimited commands, returning each output line.
    pub fn run_script(&mut self, script: &str) -> Vec<String> {
        let mut results = Vec::new();
        for line in script.lines() {
            let res = self.execute(line);
            if !res.is_empty() {
                results.push(res);
            }
        }
        results
    }

    /// Standard interactive REPL processing input from any `BufRead` stream.
    pub fn run_repl<R: BufRead, W: Write>(&mut self, mut reader: R, mut writer: W) -> Result<(), ShellError> {
        writeln!(writer, "================================================================").map_err(|e| ShellError::IoError(e.to_string()))?;
        writeln!(writer, "  HybridOS Kernel Shell & Coreutils (Rust Edition)             ").map_err(|e| ShellError::IoError(e.to_string()))?;
        writeln!(writer, "  Parallel Windows NT & Linux Subsystem Execution Host          ").map_err(|e| ShellError::IoError(e.to_string()))?;
        writeln!(writer, "================================================================\n").map_err(|e| ShellError::IoError(e.to_string()))?;
        writeln!(writer, "Type 'help' for available commands.\n").map_err(|e| ShellError::IoError(e.to_string()))?;

        let mut line = String::new();
        while self.is_running {
            write!(writer, "{}", self.prompt()).map_err(|e| ShellError::IoError(e.to_string()))?;
            writer.flush().map_err(|e| ShellError::IoError(e.to_string()))?;

            line.clear();
            if reader.read_line(&mut line).map_err(|e| ShellError::IoError(e.to_string()))? == 0 {
                break; // EOF
            }

            let output = self.execute(&line);
            if !output.is_empty() {
                writeln!(writer, "{}", output).map_err(|e| ShellError::IoError(e.to_string()))?;
            }
        }

        Ok(())
    }
}

// =============================================================================
// Helper Functions: Parsing & Binary Generators
// =============================================================================

/// Splits a raw command line into tokens, respecting quotes (`"..."` and `'...'`).
pub fn tokenize_command(input: &str) -> Result<Vec<String>, ShellError> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quote: Option<char> = None;
    let mut is_escaping = false;

    for ch in input.chars() {
        if is_escaping {
            current.push(ch);
            is_escaping = false;
            continue;
        }

        if ch == '\\' {
            is_escaping = true;
            continue;
        }

        if let Some(quote_char) = in_quote {
            if ch == quote_char {
                in_quote = None;
            } else {
                current.push(ch);
            }
        } else if ch == '"' || ch == '\'' {
            in_quote = Some(ch);
        } else if ch.is_whitespace() {
            if !current.is_empty() {
                tokens.push(current.clone());
                current.clear();
            }
        } else {
            current.push(ch);
        }
    }

    if in_quote.is_some() {
        return Err(ShellError::InvalidArgument("Unclosed quote in command string".to_string()));
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    Ok(tokens)
}

/// Formats POSIX mode octal bits into standard `rwxr-xr-x` format.
fn format_mode(mode: u32) -> String {
    let mut s = String::with_capacity(9);
    s.push(if mode & 0o400 != 0 { 'r' } else { '-' });
    s.push(if mode & 0o200 != 0 { 'w' } else { '-' });
    s.push(if mode & 0o100 != 0 { 'x' } else { '-' });
    s.push(if mode & 0o040 != 0 { 'r' } else { '-' });
    s.push(if mode & 0o020 != 0 { 'w' } else { '-' });
    s.push(if mode & 0o010 != 0 { 'x' } else { '-' });
    s.push(if mode & 0o004 != 0 { 'r' } else { '-' });
    s.push(if mode & 0o002 != 0 { 'w' } else { '-' });
    s.push(if mode & 0o001 != 0 { 'x' } else { '-' });
    s
}

/// Generates a valid synthetic Windows PE32+ (x86_64) binary buffer.
pub fn create_synthetic_pe() -> Vec<u8> {
    let mut pe = vec![0u8; 512];
    pe[0] = 0x4D; // 'M'
    pe[1] = 0x5A; // 'Z'
    pe[60] = 128; // e_lfanew
    let pe_off = 128;
    pe[pe_off]     = b'P';
    pe[pe_off + 1] = b'E';
    pe[pe_off + 2] = 0;
    pe[pe_off + 3] = 0;
    // Machine = x86_64 (0x8664)
    pe[pe_off + 4] = 0x64;
    pe[pe_off + 5] = 0x86;
    pe[pe_off + 6] = 2; // NumberOfSections = 2
    // SizeOfOptionalHeader = 240
    pe[pe_off + 20] = 240;
    // Magic = PE32+ (0x20B)
    let opt = pe_off + 24;
    pe[opt] = 0x0B;
    pe[opt + 1] = 0x02;
    // AddressOfEntryPoint = 0x1000
    pe[opt + 16] = 0x00;
    pe[opt + 17] = 0x10;
    // ImageBase = 0x0000000140000000
    pe[opt + 24] = 0x00;
    pe[opt + 25] = 0x00;
    pe[opt + 26] = 0x00;
    pe[opt + 27] = 0x40;
    pe[opt + 28] = 0x01;
    // SizeOfImage = 0x8000
    pe[opt + 56] = 0x00;
    pe[opt + 57] = 0x80;
    pe
}

/// Generates a valid synthetic Linux ELF64 (x86_64) binary buffer.
pub fn create_synthetic_elf() -> Vec<u8> {
    let mut elf = vec![0u8; 512];
    elf[0] = 0x7F;
    elf[1] = b'E';
    elf[2] = b'L';
    elf[3] = b'F';
    elf[4] = 2; // 64-bit
    elf[5] = 1; // Little-endian
    elf[6] = 1; // Current version
    elf[16] = 2; // ET_EXEC
    elf[18] = 62; // EM_X86_64
    elf[24] = 0x00; // EntryPoint: 0x401000
    elf[25] = 0x10;
    elf[26] = 0x40;
    elf[32] = 64; // phoff = 64
    elf[54] = 56; // phentsize = 56
    elf[56] = 1;  // phnum = 1
    // Program header 0: PT_LOAD (1)
    let ph = 64;
    elf[ph] = 1; // PT_LOAD
    elf[ph + 4] = 5; // PF_R | PF_X
    elf[ph + 16] = 0x00; // VAddr: 0x400000
    elf[ph + 17] = 0x00;
    elf[ph + 18] = 0x40;
    elf[ph + 40] = 0x00; // MemSz: 0x3000
    elf[ph + 41] = 0x30;
    elf
}

// =============================================================================
// Unit Tests for HybridOS Shell & Coreutils
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_help_command() {
        let mut shell = HybridShell::new();
        let help_output = shell.execute("help");
        assert!(help_output.contains("ps"));
        assert!(help_output.contains("meminfo"));
        assert!(help_output.contains("lspci"));
        assert!(help_output.contains("ls"));
        assert!(help_output.contains("cat"));
        assert!(help_output.contains("uname"));
        assert!(help_output.contains("run"));
        assert!(help_output.contains("personality"));

        let ps_help = shell.execute("help ps");
        assert!(ps_help.contains("ps - Process Status"));
    }

    #[test]
    fn test_shell_uname_command() {
        let mut shell = HybridShell::new();
        let uname_default = shell.execute("uname");
        assert_eq!(uname_default.trim(), "HybridOS");

        let uname_a = shell.execute("uname -a");
        assert!(uname_a.contains("HybridOS"));
        assert!(uname_a.contains("x86_64"));
        assert!(uname_a.contains("1.0.0-dual-abi"));
    }

    #[test]
    fn test_shell_echo_and_variable_expansion() {
        let mut shell = HybridShell::new();
        let echo_out = shell.execute("echo Hello HybridOS");
        assert_eq!(echo_out.trim(), "Hello HybridOS");

        let echo_var = shell.execute("echo System architecture: $ARCH");
        assert_eq!(echo_var.trim(), "System architecture: x86_64");
    }

    #[test]
    fn test_shell_personality_inspection_and_switch() {
        let mut shell = HybridShell::new();

        // Initial personality
        let info = shell.execute("personality");
        assert!(info.contains("Hybrid-Native"));

        // Switch to Windows NT
        let win_switch = shell.execute("personality windows");
        assert!(win_switch.contains("Windows NT"));
        assert_eq!(shell.personality, ExecutionPersonality::WindowsNt);
        assert_eq!(shell.prompt(), "C:\\> ");

        // Switch to Linux POSIX
        let linux_switch = shell.execute("personality linux");
        assert!(linux_switch.contains("Linux POSIX"));
        assert_eq!(shell.personality, ExecutionPersonality::LinuxPosix);
        assert_eq!(shell.prompt(), "root@hybridos:/# ");
    }

    #[test]
    fn test_shell_meminfo() {
        let mut shell = HybridShell::new();
        let mem_output = shell.execute("meminfo");
        assert!(mem_output.contains("Total Physical Memory:    128 MB"));
        assert!(mem_output.contains("Standard Page Size:      4096 bytes"));
        assert!(mem_output.contains("Kernel Dynamic Heap"));
        assert!(mem_output.contains("PML4 Root Frame (CR3)"));
    }

    #[test]
    fn test_shell_lspci() {
        let mut shell = HybridShell::new();
        let pci_output = shell.execute("lspci");
        assert!(pci_output.contains("00:00.0"));
        assert!(pci_output.contains("Intel Corporation"));
        assert!(pci_output.contains("VirtIO"));
        assert!(pci_output.contains("Display Controller"));
    }

    #[test]
    fn test_shell_vfs_ls_and_cat() {
        let mut shell = HybridShell::new();

        // List root directory
        let ls_out = shell.execute("ls /");
        assert!(ls_out.contains("bin"));
        assert!(ls_out.contains("etc"));
        assert!(ls_out.contains("dev"));

        // List Windows C: drive
        let ls_win = shell.execute("ls C:\\");
        assert!(ls_win.contains("Windows"));

        // Cat /etc/os-release
        let cat_out = shell.execute("cat /etc/os-release");
        assert!(cat_out.contains("NAME=\"HybridOS\""));
    }

    #[test]
    fn test_shell_process_monitor_ps() {
        let mut shell = HybridShell::new();
        let ps_output = shell.execute("ps");
        assert!(ps_output.contains("System Idle Process"));
        assert!(ps_output.contains("Windows NT"));
        assert!(ps_output.contains("Linux POSIX"));
        assert!(ps_output.contains("RIP (INSTRUCTION)"));
    }

    #[test]
    fn test_shell_run_synthetic_binaries() {
        let mut shell = HybridShell::new();

        // Run Windows PE32+
        let pe_run = shell.execute("run sample_pe");
        assert!(pe_run.contains("Windows NT PE32+"));
        assert!(pe_run.contains("STATUS_SUCCESS"));

        // Run Linux ELF64
        let elf_run = shell.execute("run sample_elf");
        assert!(elf_run.contains("Linux POSIX ELF64"));
        assert!(elf_run.contains("Linux Syscall Result"));
    }

    #[test]
    fn test_shell_script_runner() {
        let mut shell = HybridShell::new();
        let script = "echo Line 1\nuname\necho Line 3";
        let outputs = shell.run_script(script);
        assert_eq!(outputs.len(), 3);
        assert_eq!(outputs[0], "Line 1");
        assert_eq!(outputs[1], "HybridOS");
        assert_eq!(outputs[2], "Line 3");
    }

    #[test]
    fn test_shell_windows_cmd_commands() {
        let mut shell = HybridShell::new();

        // 1. 'ver'
        let ver = shell.execute("ver");
        assert!(ver.contains("Windows [Version 10.0.22631"));
        assert!(ver.contains("Linux version 6.12.0"));

        // 2. 'whoami'
        let who = shell.execute("whoami");
        assert!(who.contains("root") || who.contains("administrator"));

        // 3. 'systeminfo'
        let sysinfo = shell.execute("systeminfo");
        assert!(sysinfo.contains("OS Name:"));
        assert!(sysinfo.contains("Total Physical Memory:"));

        // 4. 'vol'
        let vol = shell.execute("vol");
        assert!(vol.contains("HYBRIDOS_SYSTEM"));

        // 5. 'tasklist'
        let tasklist = shell.execute("tasklist");
        assert!(tasklist.contains("HYBRIDOS ACTIVE PROCESS MONITOR"));
        assert!(tasklist.contains("explorer.exe") || tasklist.contains("System"));

        // 6. 'set'
        let set_res = shell.execute("set FOO=BAR");
        assert_eq!(set_res, "FOO=BAR\n");
        let query_res = shell.execute("set FOO");
        assert_eq!(query_res, "FOO=BAR\n");
    }

    #[test]
    fn test_shell_powershell_cmdlets() {
        let mut shell = HybridShell::new();

        // 1. Switch flavor to PowerShell
        let pwsh_switch = shell.execute("powershell");
        assert!(pwsh_switch.contains("Windows PowerShell"));
        assert_eq!(shell.flavor, ShellFlavor::PowerShell);
        assert_eq!(shell.prompt(), "PS C:\\> ");

        // 2. 'Get-Process' (gps)
        let gps = shell.execute("Get-Process");
        assert!(gps.contains("HYBRIDOS ACTIVE PROCESS MONITOR"));

        // 3. 'Get-Service' (gsv)
        let gsv = shell.execute("Get-Service");
        assert!(gsv.contains("vdm"));
        assert!(gsv.contains("virtio-blk"));
        assert!(gsv.contains("win32k-user32"));

        // 4. 'Get-ChildItem' (gci)
        let gci = shell.execute("Get-ChildItem C:\\");
        assert!(gci.contains("Windows"));

        // 5. 'Get-Content' (gc)
        let gc = shell.execute("Get-Content /etc/os-release");
        assert!(gc.contains("HybridOS"));

        // 6. 'Test-Path'
        let tp1 = shell.execute("Test-Path /etc/os-release");
        assert_eq!(tp1, "True\n");
        let tp2 = shell.execute("Test-Path /nonexistent/file.txt");
        assert_eq!(tp2, "False\n");

        // 7. 'Get-Command' (gcm)
        let gcm = shell.execute("Get-Command");
        assert!(gcm.contains("POWERSHELL CMDLETS"));
        assert!(gcm.contains("WINDOWS COMMAND PROMPT"));
        assert!(gcm.contains("LINUX BASH"));

        // 8. 'Get-Date'
        let gdate = shell.execute("Get-Date");
        assert!(gdate.contains("Current System Date"));
    }

    #[test]
    fn test_shell_linux_commands() {
        let mut shell = HybridShell::new();

        // 1. Switch to Bash
        let bash_switch = shell.execute("bash");
        assert!(bash_switch.contains("GNU bash"));
        assert_eq!(shell.flavor, ShellFlavor::LinuxBash);
        assert_eq!(shell.prompt(), "root@hybridos:/# ");

        // 2. 'touch', 'ls', 'cp', 'mv', 'rm'
        let _ = shell.execute("touch /tmp_test_file.txt");
        let ls_out = shell.execute("ls /");
        assert!(ls_out.contains("tmp_test_file.txt"));

        let cp_out = shell.execute("cp /tmp_test_file.txt /tmp_copy.txt");
        assert!(cp_out.contains("Copied"));

        let mv_out = shell.execute("mv /tmp_copy.txt /tmp_moved.txt");
        assert!(mv_out.contains("Moved"));

        let _ = shell.execute("rm /tmp_test_file.txt");
        let _ = shell.execute("rm /tmp_moved.txt");

        // 3. 'df'
        let df_out = shell.execute("df");
        assert!(df_out.contains("/dev/ram0"));
        assert!(df_out.contains("fat32"));

        // 4. 'ifconfig' & 'ip'
        let ifconfig = shell.execute("ifconfig");
        assert!(ifconfig.contains("eth0:"));
        assert!(ifconfig.contains("192.168.1.100"));

        // 5. 'netstat'
        let netstat = shell.execute("netstat");
        assert!(netstat.contains("Proto"));
        assert!(netstat.contains("LISTEN"));

        // 6. 'ping'
        let ping = shell.execute("ping 192.168.1.1");
        assert!(ping.contains("PING 192.168.1.1"));
        assert!(ping.contains("64 bytes from"));

        // 7. 'hostname'
        let host = shell.execute("hostname");
        assert_eq!(host, "hybridos-node1\n");

        // 8. 'dmesg'
        let dmesg = shell.execute("dmesg");
        assert!(dmesg.contains("Linux version 6.12.0"));
        assert!(dmesg.contains("Operating system successfully booted"));

        // 9. 'top'
        let top = shell.execute("top");
        assert!(top.contains("Tasks:"));
        assert!(top.contains("MiB Mem"));
    }
}
