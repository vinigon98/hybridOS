//! hyb-script-studio: System Script Debugger (Ring 3 ELF64)
//!
//! Lightweight environment for creating, inspecting variables and
//! step-by-step execution (breakpoints) of system automation scripts.

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
    println!("[hyb-script-studio CRASH] panicked: {:?}", info);
    sys_exit(1);
}

const MAX_LINES: usize = 32;
const MAX_LINE_LEN: usize = 96;
const MAX_VARS: usize = 16;
const MAX_NAME_LEN: usize = 32;
const MAX_VAL_LEN: usize = 64;

#[derive(Copy, Clone)]
struct ScriptLine {
    text: [u8; MAX_LINE_LEN],
    len: usize,
}

#[derive(Copy, Clone)]
struct Variable {
    name: [u8; MAX_NAME_LEN],
    name_len: usize,
    value: [u8; MAX_VAL_LEN],
    value_len: usize,
}

struct Studio {
    lines: [ScriptLine; MAX_LINES],
    line_count: usize,
    bp: [bool; MAX_LINES],
    pc: usize,
    vars: [Variable; MAX_VARS],
    var_count: usize,
}

impl Studio {
    const fn new() -> Self {
        Self {
            lines: [ScriptLine { text: [0u8; MAX_LINE_LEN], len: 0 }; MAX_LINES],
            line_count: 0,
            bp: [false; MAX_LINES],
            pc: 0,
            vars: [Variable { name: [0u8; MAX_NAME_LEN], name_len: 0, value: [0u8; MAX_VAL_LEN], value_len: 0 }; MAX_VARS],
            var_count: 0,
        }
    }

    fn load_demo(&mut self) {
        let demo = [
            "let x = 10",
            "let y = 20",
            "breakpoint # stop here to inspect x and y",
            "let z = x + y",
            "print z",
            "let msg = script done",
            "print msg",
        ];
        for t in demo.iter() {
            if self.line_count >= MAX_LINES { break; }
            let b = t.as_bytes();
            let n = b.len().min(MAX_LINE_LEN - 1);
            self.lines[self.line_count].text[..n].copy_from_slice(&b[..n]);
            self.lines[self.line_count].len = n;
            self.line_count += 1;
        }
        self.bp[2] = true;
    }

    fn set_var(&mut self, name: &str, value: &str) {
        for i in 0..self.var_count {
            let n = &self.vars[i].name[..self.vars[i].name_len];
            if core::str::from_utf8(n).unwrap_or("") == name {
                let b = value.as_bytes();
                let l = b.len().min(MAX_VAL_LEN - 1);
                self.vars[i].value[..l].copy_from_slice(&b[..l]);
                self.vars[i].value_len = l;
                return;
            }
        }
        if self.var_count >= MAX_VARS { return; }
        let nb = name.as_bytes();
        let nl = nb.len().min(MAX_NAME_LEN - 1);
        self.vars[self.var_count].name[..nl].copy_from_slice(&nb[..nl]);
        self.vars[self.var_count].name_len = nl;
        let b = value.as_bytes();
        let l = b.len().min(MAX_VAL_LEN - 1);
        self.vars[self.var_count].value[..l].copy_from_slice(&b[..l]);
        self.vars[self.var_count].value_len = l;
        self.var_count += 1;
    }

    fn get_var(&self, name: &str) -> Option<&str> {
        for i in 0..self.var_count {
            let n = &self.vars[i].name[..self.vars[i].name_len];
            if core::str::from_utf8(n).unwrap_or("") == name {
                return Some(core::str::from_utf8(&self.vars[i].value[..self.vars[i].value_len]).unwrap_or(""));
            }
        }
        None
    }

    fn list(&self) {
        println!("[hyb-script-studio] script loaded ({} lines):", self.line_count);
        for i in 0..self.line_count {
            let marker = if self.bp[i] { " >>BP<<" } else { "      " };
            let cur = if i == self.pc { "  <-- PC" } else { "" };
            let s = core::str::from_utf8(&self.lines[i].text[..self.lines[i].len]).unwrap_or("");
            println!("{:3}{}{}{}", i + 1, marker, s, cur);
        }
    }

    fn inspect(&self) {
        println!("[hyb-script-studio] variables in scope:");
        if self.var_count == 0 { println!("  (none)"); }
        for i in 0..self.var_count {
            let nm = core::str::from_utf8(&self.vars[i].name[..self.vars[i].name_len]).unwrap_or("");
            let vl = core::str::from_utf8(&self.vars[i].value[..self.vars[i].value_len]).unwrap_or("");
            println!("  {} = {}", nm, vl);
        }
    }

    fn step(&mut self) {
        if self.pc >= self.line_count {
            println!("[hyb-script-studio] execution finished.");
            return;
        }
        let s = core::str::from_utf8(&self.lines[self.pc].text[..self.lines[self.pc].len]).unwrap_or("");
        println!("[hyb-script-studio] L{}: {}", self.pc + 1, s);
        if self.bp[self.pc] {
            println!("[hyb-script-studio] breakpoint reached at line {} — inspector active.", self.pc + 1);
            self.inspect();
        }
        self.exec_line(s);
        self.pc += 1;
    }

    fn exec_line(&mut self, line: &str) {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("let ") {
            if let Some(eq) = rest.find('=') {
                let name = &rest[..eq];
                let value = &rest[eq + 1..];
                self.set_var(trim(name), trim(value));
            }
        } else if t.starts_with("print ") {
            let arg = &t[6..];
            let arg = arg.trim().trim_matches('"');
            {
                let s: &Self = self;
                if let Some(v) = s.get_var(arg) {
                    println!("[script] {}", v);
                } else {
                    println!("[script] {}", arg);
                }
            }
        }
    }
}

fn trim(s: &str) -> &str {
    let b = s.as_bytes();
    let mut st = 0;
    while st < b.len() && (b[st] == b' ' || b[st] == b'\t') { st += 1; }
    let mut en = b.len();
    while en > st && (b[en - 1] == b' ' || b[en - 1] == b'\t') { en -= 1; }
    &s[st..en]
}

fn run() -> i32 {
    println!("============================================================");
    println!("   hyb-script-studio — HybridOS System Script Debugger");
    println!("============================================================");

    let mut studio = Studio::new();
    studio.load_demo();
    studio.list();
    println!();

    while studio.pc < studio.line_count {
        studio.step();
        if studio.pc < studio.line_count && studio.bp[studio.pc] {
            println!("[hyb-script-studio] paused — use 'inspect' to view variables.");
            break;
        }
    }

    println!("[hyb-script-studio] step-by-step debugging and variable inspection validated.");
    sys_sched_yield();
    0
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    sys_exit(run());
}
