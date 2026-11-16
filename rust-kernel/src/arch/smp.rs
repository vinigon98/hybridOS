//! SMP (Symmetric Multiprocessing) Architecture & Universal Core Boot Engine.
//!
//! Provides bare-metal multicore bringing-up via:
//! - Hardware INIT-SIPI-SIPI sequence with real 16-bit -> 32-bit -> 64-bit trampoline
//! - UEFI Limine SMP MP-protocol support
//! - Boot spinlock synchronization preventing AP bring-up race conditions
//! - Per-CPU memory, stack, TSS, and GDT configuration

use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use crate::arch::apic;
use crate::arch::acpi::MadtInfo;
use crate::arch::percpu::{init_percpu, MAX_CPUS};
use crate::sync::SpinLock;

pub static CPU_ONLINE_COUNT: AtomicU32 = AtomicU32::new(1);
static SMP_BOOT_SPINLOCK: SpinLock<()> = SpinLock::new(());
static AP_INITIALIZED_FLAG: AtomicBool = AtomicBool::new(false);

/// Physical address allocated for the 16-bit real-mode bootstrap trampoline.
pub const SMP_TRAMPOLINE_PHYS: u64 = 0x8000;
pub const SMP_TRAMPOLINE_VECTOR: u8 = (SMP_TRAMPOLINE_PHYS / 4096) as u8; // 0x08

/// Dedicated 64 KiB kernel stack buffers for Application Processors.
#[repr(C, align(4096))]
struct ApKernelStack([u8; 65536]);
static mut AP_STACKS: [ApKernelStack; MAX_CPUS] = [const { ApKernelStack([0u8; 65536]) }; MAX_CPUS];

/// Real machine code payload for AP bring-up:
/// 1. 16-bit Real Mode: Reset segments, LGDT, CR0.PE -> Far Jump 32-bit (0x8020).
/// 2. 32-bit Protected Mode: CR4.PAE, load CR3 PML4, EFER.LME|NXE, CR0.PG|PE -> Far Jump 64-bit (0x8060).
/// 3. 64-bit Long Mode: Load 64-bit RSP, jump to Rust `ap_startup_entry`.
static SMP_TRAMPOLINE_CODE: &[u8] = &[
    // 0x00: Real Mode Entry
    0xFA,                         // cli
    0x31, 0xC0,                   // xor ax, ax
    0x8E, 0xD8,                   // mov ds, ax
    0x8E, 0xC0,                   // mov es, ax
    0x8E, 0xD0,                   // mov ss, ax
    0xBC, 0x00, 0x7E,             // mov sp, 0x7E00
    0x0F, 0x01, 0x16, 0x70, 0x80, // lgdt [0x8070]
    0x0F, 0x20, 0xC0,             // mov eax, cr0
    0x0C, 0x01,                   // or al, 1
    0x0F, 0x22, 0xC0,             // mov cr0, eax
    0xEA, 0x20, 0x80, 0x08, 0x00, // jmp 0x08:0x8020
    0x90, 0x90,                   // nop padding to 0x20

    // 0x20: 32-bit Protected Mode Entry
    0x66, 0xB8, 0x10, 0x00,       // mov ax, 0x10
    0x8E, 0xD8,                   // mov ds, ax
    0x8E, 0xC0,                   // mov es, ax
    0x8E, 0xD0,                   // mov ss, ax
    0x0F, 0x20, 0xE0,             // mov eax, cr4
    0x83, 0xC8, 0x20,             // or eax, 0x20 (PAE bit 5)
    0x0F, 0x22, 0xE0,             // mov cr4, eax
    0xA1, 0x40, 0x81, 0x00, 0x00, // mov eax, [0x8140] (PML4 CR3)
    0x0F, 0x22, 0xD8,             // mov cr3, eax
    0xB9, 0x80, 0x00, 0x00, 0xC0, // mov ecx, 0xC0000080 (EFER)
    0x0F, 0x32,                   // rdmsr
    0x0D, 0x00, 0x09, 0x00, 0x00, // or eax, 0x900 (LME bit 8 | NXE bit 11)
    0x0F, 0x30,                   // wrmsr
    0x0F, 0x20, 0xC0,             // mov eax, cr0
    0x0D, 0x01, 0x00, 0x00, 0x80, // or eax, 0x80000001 (PG | PE)
    0x0F, 0x22, 0xC0,             // mov cr0, eax
    0xEA, 0x60, 0x80, 0x00, 0x00, 0x18, 0x00, // jmp 0x18:0x8060
    0x90, 0x90, 0x90, 0x90, 0x90, // nop padding to 0x60

    // 0x60: 64-bit Long Mode Entry
    0x48, 0x8B, 0x24, 0x25, 0x48, 0x81, 0x00, 0x00, // mov rsp, [0x8148]
    0x8B, 0x3C, 0x25, 0x58, 0x81, 0x00, 0x00,       // mov edi, [0x8158] (cpu_id)
    0x8B, 0x34, 0x25, 0x5C, 0x81, 0x00, 0x00,       // mov esi, [0x815C] (lapic_id)
    0x48, 0x8B, 0x04, 0x25, 0x50, 0x81, 0x00, 0x00, // mov rax, [0x8150] (entry fn)
    0xFF, 0xE0,                                     // jmp rax
];

/// Boots all secondary Application Processors discovered in MADT.
pub unsafe fn init_smp(madt: &MadtInfo, hhdm_offset: u64) {
    crate::serial_println!("================================================================");
    crate::serial_println!(" [SMP] Initializing HybridOS Multicore Architecture Subsystem...");
    crate::serial_println!("================================================================");

    let bsp_lapic_id = apic::get_lapic_id();
    let current_cr3 = crate::arch::msr::read_cr3();

    // Check if UEFI Limine SMP protocol already primed the cores
    if let Some(smp_resp) = crate::boot::limine::get_smp_response() {
        if smp_resp.cpu_count > 1 && !smp_resp.cpus.is_null() {
            crate::serial_println!("[SMP] Limine MP protocol active: detected {} CPUs.", smp_resp.cpu_count);
            let cpus = core::slice::from_raw_parts(smp_resp.cpus, smp_resp.cpu_count as usize);
            let mut cpu_index = 1;

            for &cpu_ptr in cpus {
                if cpu_ptr.is_null() { continue; }
                let cpu = &mut *cpu_ptr;
                if cpu.lapic_id != bsp_lapic_id && (cpu_index as usize) < MAX_CPUS {
                    let _guard = SMP_BOOT_SPINLOCK.lock();
                    AP_INITIALIZED_FLAG.store(false, Ordering::SeqCst);

                    // Assign Limine AP startup vector
                    cpu.goto_address = Some(ap_limine_entry_point);

                    // Wait for AP acknowledgement
                    for _ in 0..100_000 {
                        if AP_INITIALIZED_FLAG.load(Ordering::Acquire) {
                            break;
                        }
                        core::hint::spin_loop();
                    }

                    cpu_index += 1;
                }
            }

            crate::serial_println!("[SMP] Multicore bootstrap complete: {} CPUs online.",
                CPU_ONLINE_COUNT.load(Ordering::SeqCst));
            return;
        }
    }

    // Bare-Metal Universal INIT-SIPI-SIPI Sequence
    crate::serial_println!("[SMP] Executing Native Hardware INIT-SIPI-SIPI Bootstrap...");

    // 1. Copy trampoline machine code to physical address 0x8000
    let trampoline_virt = (hhdm_offset + SMP_TRAMPOLINE_PHYS) as *mut u8;
    core::ptr::copy_nonoverlapping(SMP_TRAMPOLINE_CODE.as_ptr(), trampoline_virt, SMP_TRAMPOLINE_CODE.len());

    // 2. Setup temporary GDT at 0x8070
    setup_trampoline_gdt(hhdm_offset);

    let mut cpu_id: u32 = 1;
    for core in &madt.cores {
        if core.lapic_id == bsp_lapic_id || !core.is_enabled {
            continue;
        }
        if (cpu_id as usize) >= MAX_CPUS {
            break;
        }

        boot_application_processor(cpu_id, core.lapic_id, current_cr3, hhdm_offset);
        cpu_id += 1;
    }

    crate::serial_println!("[SMP] Multicore online: {} CPU core(s) active and operational.\n",
        CPU_ONLINE_COUNT.load(Ordering::SeqCst));
}

unsafe fn setup_trampoline_gdt(hhdm_offset: u64) {
    let gdt_ptr = (hhdm_offset + 0x8070) as *mut u16;
    let gdt_base = (hhdm_offset + 0x8080) as *mut u64;

    // GDT Pointer: Limit (31) and Base (0x8080)
    *gdt_ptr = 31;
    *((hhdm_offset + 0x8072) as *mut u32) = 0x8080;

    // GDT Descriptors:
    *gdt_base.add(0) = 0;                  // Null
    *gdt_base.add(1) = 0x00CF9A000000FFFF; // 32-bit Code (0x08)
    *gdt_base.add(2) = 0x00CF92000000FFFF; // 32-bit Data (0x10)
    *gdt_base.add(3) = 0x00AF9A000000FFFF; // 64-bit Code (0x18)
}

unsafe fn boot_application_processor(cpu_id: u32, lapic_id: u32, cr3: u64, hhdm_offset: u64) {
    let _guard = SMP_BOOT_SPINLOCK.lock();
    AP_INITIALIZED_FLAG.store(false, Ordering::SeqCst);

    // Setup AP Mailbox at 0x8090..0x80D0
    let stack_top = AP_STACKS[cpu_id as usize].0.as_ptr() as u64 + 65536;
    *((hhdm_offset + 0x8090) as *mut u64) = cr3;
    *((hhdm_offset + 0x8098) as *mut u64) = stack_top;
    *((hhdm_offset + 0x80C0) as *mut u64) = ap_startup_entry as *const () as u64;
    *((hhdm_offset + 0x80C8) as *mut u32) = cpu_id;
    *((hhdm_offset + 0x80CC) as *mut u32) = lapic_id;

    // Phase 1: INIT IPI
    apic::send_ipi(lapic_id, apic::ICR_DELIVERY_INIT, 0);
    pit_delay_ms(10);

    // Phase 2: First SIPI (Startup IPI) -> points to page 0x8000
    apic::send_ipi(lapic_id, apic::ICR_DELIVERY_STARTUP, SMP_TRAMPOLINE_VECTOR);
    pit_delay_us(200);

    // Phase 3: Second SIPI if AP hasn't responded yet
    if !AP_INITIALIZED_FLAG.load(Ordering::Acquire) {
        apic::send_ipi(lapic_id, apic::ICR_DELIVERY_STARTUP, SMP_TRAMPOLINE_VECTOR);
    }

    // Await AP online signal (timeout: ~50ms)
    for _ in 0..500_000 {
        if AP_INITIALIZED_FLAG.load(Ordering::Acquire) {
            crate::serial_println!("[SMP] CPU Core #{} (LAPIC ID {}) successfully booted.", cpu_id, lapic_id);
            return;
        }
        core::hint::spin_loop();
    }

    crate::serial_println!("[SMP WARNING] CPU Core #{} (LAPIC ID {}) boot timeout.", cpu_id, lapic_id);
}

/// 64-bit Long Mode entry point for secondary cores booted via native INIT-SIPI-SIPI.
#[no_mangle]
pub unsafe extern "C" fn ap_startup_entry(cpu_id: u32, _lapic_id: u32) -> ! {
    // 1. Initialize per-CPU data and GS_BASE
    init_percpu(cpu_id);

    // 2. Load the system IDT on this core
    crate::arch::load_kernel_idt();

    // 3. Initialize Local APIC on this core
    let hhdm = crate::boot::get_hhdm_offset();
    apic::init_local_apic(0xFEE00000, hhdm);

    // 4. Mark core online and notify BSP
    CPU_ONLINE_COUNT.fetch_add(1, Ordering::SeqCst);
    AP_INITIALIZED_FLAG.store(true, Ordering::Release);

    crate::serial_println!("[AP] Core #{} online: GS_BASE programmed, IDT loaded, LAPIC active, idle ready.", cpu_id);

    // 5. Enter idle/scheduling loop with interrupts enabled
    loop {
        core::arch::asm!("sti; hlt");
    }
}

/// Entry point when booted via Limine MP protocol.
unsafe extern "C" fn ap_limine_entry_point(info: *const crate::boot::limine::LimineSmpInfo) -> ! {
    let cpu_id = (*info).processor_id;
    let lapic_id = (*info).lapic_id;
    ap_startup_entry(cpu_id, lapic_id);
}

// Microsecond and millisecond PIT delay calibration routines
unsafe fn pit_delay_ms(ms: u32) {
    for _ in 0..ms {
        pit_delay_us(1000);
    }
}

unsafe fn pit_delay_us(us: u32) {
    for _ in 0..(us * 10) {
        crate::arch::msr::io_wait();
    }
}
