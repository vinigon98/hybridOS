//! Multi-core Per-CPU Architecture Data Structure and GS_BASE Handling.

use crate::arch::msr::{self, IA32_GS_BASE, IA32_KERNEL_GS_BASE};

pub const MAX_CPUS: usize = 64;

#[repr(C, align(64))]
pub struct PerCpuData {
    pub kernel_rsp: u64,
    pub user_rsp_scratch: u64,
    pub current_task_id: u64,
    pub current_task_ptr: usize,
    pub cpu_id: u32,
    pub reserved: u32,
    pub self_ptr: u64,
    pub syscall_stack: [u8; 16384],
}

impl PerCpuData {
    pub const fn new(cpu_id: u32) -> Self {
        Self {
            kernel_rsp: 0,
            user_rsp_scratch: 0,
            current_task_id: 0,
            current_task_ptr: 0,
            cpu_id,
            reserved: 0,
            self_ptr: 0,
            syscall_stack: [0u8; 16384],
        }
    }
}

static mut PERCPU_TABLE: [PerCpuData; MAX_CPUS] = [const { PerCpuData::new(0) }; MAX_CPUS];

pub unsafe fn init_percpu(cpu_id: u32) {
    let idx = (cpu_id as usize).min(MAX_CPUS - 1);
    let cpu_ptr = &raw mut PERCPU_TABLE[idx];

    (*cpu_ptr).cpu_id = cpu_id;
    (*cpu_ptr).self_ptr = cpu_ptr as u64;
    let stack_top = (*cpu_ptr).syscall_stack.as_ptr() as u64 + (*cpu_ptr).syscall_stack.len() as u64;
    (*cpu_ptr).kernel_rsp = stack_top;

    msr::wrmsr(IA32_GS_BASE, cpu_ptr as u64);
    msr::wrmsr(IA32_KERNEL_GS_BASE, cpu_ptr as u64);

    crate::serial_println!(
        "[PERCPU] Core #{} online: GS_BASE=0x{:016X}, syscall_rsp0=0x{:016X}",
        cpu_id, cpu_ptr as u64, stack_top
    );
}

#[inline]
pub fn get_cpu_data(cpu_id: u32) -> &'static mut PerCpuData {
    let idx = (cpu_id as usize).min(MAX_CPUS - 1);
    unsafe { &mut *(&raw mut PERCPU_TABLE[idx]) }
}

#[inline]
pub fn get_bsp_cpu_data() -> &'static mut PerCpuData {
    get_cpu_data(0)
}

#[inline]
pub fn get_current_cpu_data() -> &'static mut PerCpuData {
    let gs_base = unsafe { msr::rdmsr(IA32_GS_BASE) };
    if gs_base != 0 {
        unsafe { &mut *(gs_base as *mut PerCpuData) }
    } else {
        get_bsp_cpu_data()
    }
}

#[inline]
pub fn update_current_cpu_task(tid: u64, kernel_rsp: u64, task_ptr: usize) {
    let data = get_current_cpu_data();
    data.current_task_id = tid;
    data.kernel_rsp = kernel_rsp;
    data.current_task_ptr = task_ptr;
}
