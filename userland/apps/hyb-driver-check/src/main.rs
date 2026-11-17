//! hyb-driver-check — HybridOS Driver Integrity Checker (Ring 3 ELF64).
//!
//! Enumerates REAL PCI devices via `sys_pci_enum` (201) (kernel bus scanner
//! in `rust-kernel/src/drivers/hardware.rs`) and reports whether each device
//! has a recognized driver binding in HybridOS.

#![no_std]
#![no_main]

use core::panic::PanicInfo;
use libhybrid_sys::allocator::HybridUserAllocator;
use libhybrid_sys::linux::*;
use libhybrid_sys::println;

#[global_allocator]
static ALLOCATOR: HybridUserAllocator = HybridUserAllocator::new();

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("[hyb-driver-check] fatal: {:?}", info);
    sys_exit(1);
}

const MAX_DEVICES: usize = 64;

fn driver_for(class: u8, vendor: u16, device: u16) -> &'static str {
    // Known HybridOS kernel driver bindings (from rust-kernel/src/drivers/)
    match class {
        0x02 => match (vendor, device) {
            (0x1AF4, 0x1000) | (0x1AF4, 0x1041) => "virtio_net (smoltcp E1000 path)",
            (0x8086, 0x100E) | (0x8086, 0x10D3) => "e1000 (Intel Gigabit)",
            _ => "net_driver (generic)",
        },
        0x01 => match (vendor, device) {
            (0x1AF4, 0x1001) | (0x1AF4, 0x1042) => "virtio_blk",
            (0x8086, 0x7010) => "ata_pio",
            _ => "block_driver (generic)",
        },
        0x03 => "gop_display / bochs_vga",
        0x06 => "pci_bridge",
        _ => "none",
    }
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("");
    println!("============================================================");
    println!("      hyb-driver-check - HybridOS Driver Integrity Check     ");
    println!("============================================================");

    let mut entries = [PciEntry::default(); MAX_DEVICES];
    let ret = unsafe { sys_pci_enum(entries.as_mut_ptr(), MAX_DEVICES as u64) };
    let n = if ret < 0 { 0 } else { ret as usize };

    if n == 0 {
        println!("[hyb-driver-check] sys_pci_enum unavailable: PCI scanner not exposed by kernel");
        sys_sched_yield();
        sys_exit(0);
    }

    println!("[real PCI devices from kernel bus scanner]");
    println!("  BUS:DEV.FUNC  VENDOR:DEVICE     CLASS           DRIVER");
    println!("  ------------------------------------------------------------------------");

    let mut bound = 0u32;
    let mut unbound = 0u32;

    for i in 0..n {
        let e = &entries[i];
        let drv = driver_for(e.class_code as u8, e.vendor_id as u16, e.device_id as u16);
        let bound_flag = if drv == "none" { "unbound" } else { "bound   " };
        println!("  {:02X}:{:02X}.{:X}   {:04X}:{:<4X}      {:<15} {}",
            e.bus, e.dev, e.func,
            e.vendor_id, e.device_id,
            match e.class_code {
                0x02 => "NET",
                0x01 => "STORAGE",
                0x03 => "DISPLAY",
                0x06 => "BRIDGE",
                _ => "OTHER",
            },
            bound_flag);
        if drv == "none" { unbound += 1; } else { bound += 1; }
    }

    println!("  ------------------------------------------------------------------------");
    println!("  total devices : {}", n);
    println!("  driver bound  : {}", bound);
    println!("  driver unbound: {}", unbound);

    println!("");
    println!("[hyb-driver-check] driver check complete (real PCI data from kernel).");
    sys_sched_yield();
    sys_exit(0);
}
