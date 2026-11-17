//! hyb-devices — HybridOS PCI Device Enumerator (Ring 3 ELF64).
//!
//! Reads REAL PCI device data from the kernel bus scanner
//! (`rust-kernel/src/drivers/hardware.rs`) through the `sys_pci_enum` (201)
//! system call. Lists vendor/device/class/bus:device:function for every
//! discovered PCI function.

#![no_std]
#![no_main]

extern crate alloc;

use alloc::format;
use core::panic::PanicInfo;
use libhybrid_sys::allocator::HybridUserAllocator;
use libhybrid_sys::linux::*;
use libhybrid_sys::println;

#[global_allocator]
static ALLOCATOR: HybridUserAllocator = HybridUserAllocator::new();

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("[hyb-devices] fatal: {:?}", info);
    sys_exit(1);
}

const MAX_DEVICES: usize = 64;

fn vendor_name(id: u16) -> &'static str {
    match id {
        0x8086 => "Intel Corporation",
        0x1AF4 => "Red Hat / QEMU (VirtIO)",
        0x1002 => "AMD / ATI",
        0x1022 => "AMD",
        0x10DE => "NVIDIA Corporation",
        0x10EC => "Realtek",
        0x1234 => "Bochs / QEMU",
        _ => "Unknown Vendor",
    }
}

fn class_name(class: u8, subclass: u8) -> &'static str {
    match class {
        0x06 => match subclass {
            0x00 => "Host Bridge",
            0x01 => "ISA Bridge",
            0x04 => "PCI-to-PCI Bridge",
            _ => "Bridge",
        },
        0x02 => "Network Controller",
        0x03 => "Display Controller",
        0x01 => "Mass Storage Controller",
        0x04 => "Multimedia Controller",
        0x09 => "Input Device Controller",
        0x0C => "Serial Bus Controller",
        _ => "Other",
    }
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("");
    println!("============================================================");
    println!("        hyb-devices - HybridOS PCI Device Enumerator        ");
    println!("============================================================");

    let mut entries = [PciEntry::default(); MAX_DEVICES];
    let ret = unsafe { sys_pci_enum(entries.as_mut_ptr(), MAX_DEVICES as u64) };
    let n = if ret < 0 { 0 } else { ret as usize };

    if n == 0 {
        println!("[hyb-devices] sys_pci_enum returned no devices (kernel PCI scanner unavailable)");
        println!("[hyb-devices] real PCI enumeration requires rust-kernel/src/drivers/hardware.rs");
        sys_sched_yield();
        sys_exit(0);
    }

    println!("[real PCI devices from kernel bus scanner]");
    println!("  BUS:DEV.FUNC  VENDOR              DEVICE        CLASS           IRQ");
    println!("  ------------------------------------------------------------------------");

    let mut net = 0u32;
    let mut storage = 0u32;
    let mut display = 0u32;
    let mut virtio = 0u32;

    for i in 0..n {
        let e = &entries[i];
        let vendor = vendor_name(e.vendor_id as u16);
        let class = class_name(e.class_code as u8, e.subclass as u8);
        println!("  {:02X}:{:02X}.{:X}   {:<19} {:<16} {:<15} {}",
            e.bus, e.dev, e.func,
            vendor,
            format!("{:04X}:{:04X}", e.vendor_id, e.device_id),
            class,
            e.irq);
        if e.class_code == 0x02 { net += 1; }
        if e.class_code == 0x01 { storage += 1; }
        if e.class_code == 0x03 { display += 1; }
        if e.vendor_id == 0x1AF4 { virtio += 1; }
    }

    println!("  ------------------------------------------------------------------------");
    println!("  total devices enumerated: {}", n);
    println!("  network controllers : {}", net);
    println!("  storage controllers : {}", storage);
    println!("  display controllers : {}", display);
    println!("  virtio devices      : {}", virtio);

    println!("");
    println!("[hyb-devices] PCI enumeration complete (real hardware data from kernel).");
    sys_sched_yield();
    sys_exit(0);
}
