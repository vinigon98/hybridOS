//! Local APIC (Advanced Programmable Interrupt Controller) Driver.
//!
//! Controls hardware interrupt delivery, Inter-Processor Interrupts (IPI),
//! Spurious Vector configuration, and calibrated timer for x86_64 Long Mode.

use core::sync::atomic::{AtomicU64, Ordering};
use crate::arch::msr;
use crate::sync::SpinLock;

pub const IA32_APIC_BASE_MSR: u32 = 0x1B;
pub const IA32_APIC_BASE_ENABLE: u64 = 1 << 11;
pub const IA32_APIC_BASE_BSP: u64 = 1 << 8;

// Local APIC Register Offsets (MMIO 4-byte aligned on 16-byte boundaries)
pub const LAPIC_ID: u32             = 0x020;
pub const LAPIC_VERSION: u32        = 0x030;
pub const LAPIC_TPR: u32            = 0x080; // Task Priority Register
pub const LAPIC_APR: u32            = 0x090;
pub const LAPIC_PPR: u32            = 0x0A0;
pub const LAPIC_EOI: u32            = 0x0B0; // End of Interrupt
pub const LAPIC_RRD: u32            = 0x0C0;
pub const LAPIC_LDR: u32            = 0x0D0; // Logical Destination Register
pub const LAPIC_DFR: u32            = 0x0E0; // Destination Format Register
pub const LAPIC_SVR: u32            = 0x0F0; // Spurious Interrupt Vector Register
pub const LAPIC_ISR_BASE: u32       = 0x100;
pub const LAPIC_TMR_BASE: u32       = 0x180;
pub const LAPIC_IRR_BASE: u32       = 0x200;
pub const LAPIC_ESR: u32            = 0x280; // Error Status Register
pub const LAPIC_ICR_LOW: u32        = 0x300; // Interrupt Command Register 0-31
pub const LAPIC_ICR_HIGH: u32       = 0x310; // Interrupt Command Register 32-63
pub const LAPIC_LVT_TIMER: u32      = 0x320;
pub const LAPIC_LVT_THERMAL: u32    = 0x330;
pub const LAPIC_LVT_PERF: u32       = 0x340;
pub const LAPIC_LVT_LINT0: u32      = 0x350;
pub const LAPIC_LVT_LINT1: u32      = 0x360;
pub const LAPIC_LVT_ERROR: u32      = 0x370;
pub const LAPIC_TIMER_INIT_CNT: u32 = 0x380;
pub const LAPIC_TIMER_CURR_CNT: u32 = 0x390;
pub const LAPIC_TIMER_DIV_CFG: u32  = 0x3E0;

// ICR Delivery Modes
pub const ICR_DELIVERY_FIXED: u32   = 0 << 8;
pub const ICR_DELIVERY_SMI: u32     = 2 << 8;
pub const ICR_DELIVERY_NMI: u32     = 4 << 8;
pub const ICR_DELIVERY_INIT: u32    = 5 << 8;
pub const ICR_DELIVERY_STARTUP: u32 = 6 << 8;

pub const ICR_STATUS_PENDING: u32   = 1 << 12;
pub const ICR_LEVEL_ASSERT: u32     = 1 << 14;
pub const ICR_TRIGGER_LEVEL: u32    = 1 << 15;

static LOCAL_APIC_VIRT_BASE: AtomicU64 = AtomicU64::new(0);
static APIC_SPINLOCK: SpinLock<()> = SpinLock::new(());

/// Reads a 32-bit register from the memory-mapped Local APIC.
#[inline]
pub unsafe fn lapic_read(reg: u32) -> u32 {
    let base = LOCAL_APIC_VIRT_BASE.load(Ordering::Relaxed);
    let ptr = (base + reg as u64) as *const u32;
    core::ptr::read_volatile(ptr)
}

/// Writes a 32-bit register to the memory-mapped Local APIC.
#[inline]
pub unsafe fn lapic_write(reg: u32, val: u32) {
    let base = LOCAL_APIC_VIRT_BASE.load(Ordering::Relaxed);
    let ptr = (base + reg as u64) as *mut u32;
    core::ptr::write_volatile(ptr, val);
}

/// Initializes the Local APIC on the current CPU core.
pub unsafe fn init_local_apic(phys_base: u64, hhdm_offset: u64) {
    let _guard = APIC_SPINLOCK.lock();

    // 1. Configure and enable APIC Base MSR (0x1B)
    let apic_msr = msr::rdmsr(IA32_APIC_BASE_MSR);
    msr::wrmsr(IA32_APIC_BASE_MSR, (apic_msr & 0x0000_0000_0000_0FFF) | phys_base | IA32_APIC_BASE_ENABLE);

    let virt_base = hhdm_offset + phys_base;
    LOCAL_APIC_VIRT_BASE.store(virt_base, Ordering::SeqCst);

    // 2. Disable legacy 8259 PIC by masking all IRQs
    msr::outb(0x21, 0xFF);
    msr::outb(0xA1, 0xFF);

    // 3. Clear Task Priority Register to accept all interrupts
    lapic_write(LAPIC_TPR, 0);

    // 4. Set Destination Format Register to Flat Model
    lapic_write(LAPIC_DFR, 0xFFFF_FFFF);

    // 5. Logical Destination Register (all cores listen to logical group 1)
    let current_id = (lapic_read(LAPIC_ID) >> 24) & 0xFF;
    lapic_write(LAPIC_LDR, (1 << (current_id.min(7))) << 24);

    // 6. Spurious Interrupt Vector Register (Vector 0xFF + APIC Software Enable bit 8)
    lapic_write(LAPIC_SVR, 0x1FF);

    // 7. Clear Error Status Register
    lapic_write(LAPIC_ESR, 0);
    let _ = lapic_read(LAPIC_ESR);

    // 8. Mask LINT0 (legacy PIC route) and configure LINT1 as NMI
    lapic_write(LAPIC_LVT_LINT0, 1 << 16); // Masked
    lapic_write(LAPIC_LVT_LINT1, ICR_DELIVERY_NMI); // NMI unmasked
    lapic_write(LAPIC_LVT_ERROR, 1 << 16); // Error masked

    // 9. Send initial EOI to clean pending traps
    lapic_write(LAPIC_EOI, 0);

    // 10. Configure Local APIC Periodic Timer
    lapic_write(LAPIC_TIMER_DIV_CFG, 0x03); // Divide by 16
    lapic_write(LAPIC_LVT_TIMER, 0x20 | (1 << 17)); // Vector 32 (0x20), Periodic Mode (bit 17)
    lapic_write(LAPIC_TIMER_INIT_CNT, 0x0002_0000); // Baseline periodic counter
}

/// Acknowledges interrupt dispatch (End Of Interrupt).
#[inline]
pub unsafe fn send_eoi() {
    lapic_write(LAPIC_EOI, 0);
}

/// Returns the current Local APIC Hardware ID.
#[inline]
pub unsafe fn get_lapic_id() -> u32 {
    (lapic_read(LAPIC_ID) >> 24) & 0xFF
}

/// Dispatches an Inter-Processor Interrupt (IPI) to a target Local APIC ID.
pub unsafe fn send_ipi(target_lapic_id: u32, delivery_mode: u32, vector: u8) {
    // Wait for ICR idle status (bit 12 == 0)
    while (lapic_read(LAPIC_ICR_LOW) & ICR_STATUS_PENDING) != 0 {
        core::hint::spin_loop();
    }

    // Set destination APIC ID in high 32 bits
    lapic_write(LAPIC_ICR_HIGH, target_lapic_id << 24);

    // Trigger IPI in low 32 bits
    let icr_low = delivery_mode | (vector as u32) | ICR_LEVEL_ASSERT;
    lapic_write(LAPIC_ICR_LOW, icr_low);

    while (lapic_read(LAPIC_ICR_LOW) & ICR_STATUS_PENDING) != 0 {
        core::hint::spin_loop();
    }
}
