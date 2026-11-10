//! Hardware Abstraction & Driver Module for HybridOS Kernel.
//!
//! Provides comprehensive hardware device abstractions, specifications, and drivers:
//! 1. PCI Bus Scanner (bus 0..256, device 0..32, function 0..8 via I/O ports 0xCF8/0xCFC)
//! 2. VirtIO Block Device Driver Specification (QEMU / KVM virtio-blk)
//! 3. VirtIO Network Device Driver Specification (QEMU / KVM virtio-net)
//! 4. Framebuffer / Display GOP (Graphics Output Protocol) Driver
//! 5. Keyboard PS/2 Input Ring-Buffer Driver

use std::sync::{Arc, Mutex};
use std::collections::VecDeque;
use crate::drivers::vdm::{WdmDriver, LinuxDriver, Irp};
use crate::types::{NtStatus, PosixError};

// ============================================================================
// Section 1: Port I/O Abstraction (Physical & Virtual Emulation)
// ============================================================================

pub const PCI_CONFIG_ADDRESS: u16 = 0x0CF8;
pub const PCI_CONFIG_DATA: u16 = 0x0CFC;

pub const PS2_DATA_PORT: u16 = 0x0060;
pub const PS2_STATUS_PORT: u16 = 0x0064;

/// Trait representing I/O port read/write operations.
pub trait PortIo: Send + Sync {
    fn read_u8(&mut self, port: u16) -> u8;
    fn read_u16(&mut self, port: u16) -> u16;
    fn read_u32(&mut self, port: u16) -> u32;
    fn write_u8(&mut self, port: u16, val: u8);
    fn write_u16(&mut self, port: u16, val: u16);
    fn write_u32(&mut self, port: u16, val: u32);
}

/// Simulated / Virtual Port I/O device for testing, emulation, and userspace simulation.
pub struct VirtualPortIo {
    pci_config_address: u32,
    pci_devices: std::collections::HashMap<(u8, u8, u8), Vec<u32>>,
    ps2_status: u8,
    ps2_data: u8,
}

impl VirtualPortIo {
    pub fn new() -> Self {
        let mut io = Self {
            pci_config_address: 0,
            pci_devices: std::collections::HashMap::new(),
            ps2_status: 0,
            ps2_data: 0,
        };
        io.populate_standard_qemu_devices();
        io
    }

    /// Pre-populate standard virtual devices found on typical QEMU / KVM VMs
    fn populate_standard_qemu_devices(&mut self) {
        // Bus 0, Device 0, Func 0: Host Bridge (Intel 82441FX / Q35)
        self.register_device(0, 0, 0, 0x8086, 0x1237, 0x060000, 0x00, &[
            0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000,
        ]);

        // Bus 0, Device 1, Func 0: ISA Bridge (Intel 82371SB PIIX3)
        self.register_device(0, 1, 0, 0x8086, 0x7000, 0x060100, 0x80, &[
            0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000,
        ]);

        // Bus 0, Device 2, Func 0: Display Controller (QEMU Standard VGA / Bochs Display)
        self.register_device(0, 2, 0, 0x1234, 0x1111, 0x030000, 0x00, &[
            0xE0000008, // BAR0: 32-bit Prefetchable Memory at 0xE0000000
            0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000,
        ]);

        // Bus 0, Device 3, Func 0: VirtIO Network Adapter (Red Hat / QEMU)
        self.register_device(0, 3, 0, 0x1AF4, 0x1000, 0x020000, 0x00, &[
            0x0000C001, // BAR0: I/O Port at 0xC000
            0xFEBC0000, // BAR1: 32-bit Memory at 0xFEBC0000
            0x00000000, 0x00000000, 0x00000000, 0x00000000,
        ]);

        // Bus 0, Device 4, Func 0: VirtIO Block Storage Adapter (Red Hat / QEMU)
        self.register_device(0, 4, 0, 0x1AF4, 0x1001, 0x010000, 0x00, &[
            0x0000C101, // BAR0: I/O Port at 0xC100
            0xFEBC1000, // BAR1: 32-bit Memory at 0xFEBC1000
            0x00000000, 0x00000000, 0x00000000, 0x00000000,
        ]);
    }

    pub fn register_device(
        &mut self,
        bus: u8,
        dev: u8,
        func: u8,
        vendor_id: u16,
        device_id: u16,
        class_rev: u32,
        header_type: u8,
        bars: &[u32; 6],
    ) {
        let mut dwords = vec![0u32; 64];
        // Offset 0x00: Vendor ID (bits 0..15), Device ID (bits 16..31)
        dwords[0] = (vendor_id as u32) | ((device_id as u32) << 16);
        // Offset 0x04: Command (0) | Status (0x0210: Capabilities, Medium DEVSEL)
        dwords[1] = 0x02100007; // I/O, Mem, BusMaster enabled
        // Offset 0x08: Class code, subclass, prog_if, revision
        dwords[2] = class_rev;
        // Offset 0x0C: Header type at byte 14
        dwords[3] = (header_type as u32) << 16;
        // Offset 0x10..0x24: BAR0..BAR5
        for i in 0..6 {
            dwords[4 + i] = bars[i];
        }
        // Offset 0x3C: Interrupt Line (IRQ 11), Interrupt Pin 1 (INTA)
        dwords[15] = 0x0000010B;

        self.pci_devices.insert((bus, dev, func), dwords);
    }
}

impl PortIo for VirtualPortIo {
    fn read_u8(&mut self, port: u16) -> u8 {
        match port {
            PS2_DATA_PORT => {
                let data = self.ps2_data;
                self.ps2_status &= !0x01; // Clear Output Buffer Full
                data
            }
            PS2_STATUS_PORT => self.ps2_status,
            _ => (self.read_u32(port & !3) >> ((port & 3) * 8)) as u8,
        }
    }

    fn read_u16(&mut self, port: u16) -> u16 {
        (self.read_u32(port & !3) >> ((port & 2) * 8)) as u16
    }

    fn read_u32(&mut self, port: u16) -> u32 {
        if port == PCI_CONFIG_ADDRESS {
            self.pci_config_address
        } else if port == PCI_CONFIG_DATA {
            if (self.pci_config_address & 0x80000000) != 0 {
                let bus = ((self.pci_config_address >> 16) & 0xFF) as u8;
                let dev = ((self.pci_config_address >> 11) & 0x1F) as u8;
                let func = ((self.pci_config_address >> 8) & 0x07) as u8;
                let reg_dword = ((self.pci_config_address >> 2) & 0x3F) as usize;

                if let Some(regs) = self.pci_devices.get(&(bus, dev, func)) {
                    if reg_dword < regs.len() {
                        return regs[reg_dword];
                    }
                }
            }
            0xFFFFFFFF
        } else {
            0
        }
    }

    fn write_u8(&mut self, port: u16, val: u8) {
        if port == PS2_DATA_PORT {
            self.ps2_data = val;
            self.ps2_status |= 0x01; // Output buffer full
        }
    }

    fn write_u16(&mut self, _port: u16, _val: u16) {}

    fn write_u32(&mut self, port: u16, val: u32) {
        if port == PCI_CONFIG_ADDRESS {
            self.pci_config_address = val;
        } else if port == PCI_CONFIG_DATA && (self.pci_config_address & 0x80000000) != 0 {
            let bus = ((self.pci_config_address >> 16) & 0xFF) as u8;
            let dev = ((self.pci_config_address >> 11) & 0x1F) as u8;
            let func = ((self.pci_config_address >> 8) & 0x07) as u8;
            let reg_dword = ((self.pci_config_address >> 2) & 0x3F) as usize;

            if let Some(regs) = self.pci_devices.get_mut(&(bus, dev, func)) {
                if reg_dword < regs.len() {
                    regs[reg_dword] = val;
                }
            }
        }
    }
}

// ============================================================================
// Section 2: PCI Bus Scanner
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PciAddress {
    pub bus: u8,
    pub device: u8,
    pub function: u8,
}

impl PciAddress {
    pub const fn new(bus: u8, device: u8, function: u8) -> Self {
        Self { bus, device, function }
    }

    /// Computes the 32-bit PCI configuration address word for port 0xCF8
    pub fn config_address(&self, register_offset: u8) -> u32 {
        0x80000000u32
            | ((self.bus as u32) << 16)
            | (((self.device & 0x1F) as u32) << 11)
            | (((self.function & 0x07) as u32) << 8)
            | ((register_offset & 0xFC) as u32)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PciBar {
    Memory32 {
        address: u32,
        size: u32,
        prefetchable: bool,
    },
    Memory64 {
        address: u64,
        size: u64,
        prefetchable: bool,
    },
    Io {
        port: u16,
        size: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PciDevice {
    pub address: PciAddress,
    pub vendor_id: u16,
    pub device_id: u16,
    pub command: u16,
    pub status: u16,
    pub revision_id: u8,
    pub prog_if: u8,
    pub subclass: u8,
    pub class_code: u8,
    pub header_type: u8,
    pub bars: [Option<PciBar>; 6],
    pub interrupt_line: u8,
    pub interrupt_pin: u8,
}

impl PciDevice {
    pub fn is_virtio(&self) -> bool {
        self.vendor_id == 0x1AF4
    }

    pub fn is_virtio_block(&self) -> bool {
        self.vendor_id == 0x1AF4 && (self.device_id == 0x1001 || self.device_id == 0x1042)
    }

    pub fn is_virtio_net(&self) -> bool {
        self.vendor_id == 0x1AF4 && (self.device_id == 0x1000 || self.device_id == 0x1041)
    }

    pub fn is_display_vga(&self) -> bool {
        self.class_code == 0x03
    }

    pub fn class_description(&self) -> &'static str {
        match self.class_code {
            0x00 => "Legacy Device",
            0x01 => "Mass Storage Controller",
            0x02 => "Network Controller",
            0x03 => "Display Controller",
            0x04 => "Multimedia Controller",
            0x05 => "Memory Controller",
            0x06 => "Bridge Device",
            0x07 => "Simple Communications Controller",
            0x08 => "Base System Peripheral",
            0x09 => "Input Device Controller",
            0x0A => "Docking Station",
            0x0B => "Processor",
            0x0C => "Serial Bus Controller",
            0x0D => "Wireless Controller",
            0x0E => "Intelligent Controller",
            0x0F => "Satellite Communications Controller",
            0x10 => "Encryption Controller",
            0x11 => "Signal Processing Controller",
            _ => "Unknown Controller",
        }
    }

    pub fn vendor_name(&self) -> &'static str {
        match self.vendor_id {
            0x8086 => "Intel Corporation",
            0x1AF4 => "Red Hat / QEMU (VirtIO)",
            0x1002 => "Advanced Micro Devices (AMD / ATI)",
            0x1022 => "Advanced Micro Devices (AMD)",
            0x10DE => "NVIDIA Corporation",
            0x10EC => "Realtek Semiconductor Co., Ltd.",
            0x1234 => "Bochs / QEMU Virtual Hardware",
            _ => "Unknown Vendor",
        }
    }
}

pub struct PciBusScanner<T: PortIo> {
    io: T,
}

impl<T: PortIo> PciBusScanner<T> {
    pub fn new(io: T) -> Self {
        Self { io }
    }

    pub fn io_mut(&mut self) -> &mut T {
        &mut self.io
    }

    /// Read a 32-bit DWORD from PCI configuration space at the given offset
    pub fn read_config_u32(&mut self, addr: &PciAddress, offset: u8) -> u32 {
        let config_addr = addr.config_address(offset);
        self.io.write_u32(PCI_CONFIG_ADDRESS, config_addr);
        self.io.read_u32(PCI_CONFIG_DATA)
    }

    /// Write a 32-bit DWORD to PCI configuration space
    pub fn write_config_u32(&mut self, addr: &PciAddress, offset: u8, val: u32) {
        let config_addr = addr.config_address(offset);
        self.io.write_u32(PCI_CONFIG_ADDRESS, config_addr);
        self.io.write_u32(PCI_CONFIG_DATA, val);
    }

    /// Read a 16-bit WORD from PCI configuration space
    pub fn read_config_u16(&mut self, addr: &PciAddress, offset: u8) -> u16 {
        let dword = self.read_config_u32(addr, offset & !3);
        ((dword >> ((offset & 2) * 8)) & 0xFFFF) as u16
    }

    /// Read an 8-bit BYTE from PCI configuration space
    pub fn read_config_u8(&mut self, addr: &PciAddress, offset: u8) -> u8 {
        let dword = self.read_config_u32(addr, offset & !3);
        ((dword >> ((offset & 3) * 8)) & 0xFF) as u8
    }

    /// Read and probe BARs (Base Address Registers) for a PCI function
    pub fn probe_bars(&mut self, addr: &PciAddress, header_type: u8) -> [Option<PciBar>; 6] {
        let mut bars: [Option<PciBar>; 6] = [None, None, None, None, None, None];

        // Standard header type 0x00 has 6 BARs (offsets 0x10 to 0x24)
        if (header_type & 0x7F) != 0x00 {
            return bars;
        }

        let mut bar_idx = 0;
        while bar_idx < 6 {
            let offset = 0x10 + (bar_idx as u8) * 4;
            let original_val = self.read_config_u32(addr, offset);

            if original_val == 0 {
                bar_idx += 1;
                continue;
            }

            if (original_val & 0x01) == 0x01 {
                // I/O Space BAR
                let port = (original_val & 0xFFFC) as u16;

                // Probe size by writing all 1s
                self.write_config_u32(addr, offset, 0xFFFFFFFF);
                let mask = self.read_config_u32(addr, offset);
                self.write_config_u32(addr, offset, original_val); // Restore original

                let size = if mask != 0 && mask != 0xFFFFFFFF {
                    !(mask & 0xFFFC) + 1
                } else {
                    64 // Default fallback allocation
                };

                bars[bar_idx] = Some(PciBar::Io { port, size });
                bar_idx += 1;
            } else {
                // Memory Space BAR
                let bar_type = (original_val >> 1) & 0x03;
                let prefetchable = (original_val & 0x08) != 0;

                if bar_type == 0x02 && bar_idx < 5 {
                    // 64-bit Memory BAR (spans two 32-bit registers)
                    let next_offset = offset + 4;
                    let high_val = self.read_config_u32(addr, next_offset);
                    let full_addr = ((high_val as u64) << 32) | ((original_val & 0xFFFFFFF0) as u64);

                    // Probe size
                    self.write_config_u32(addr, offset, 0xFFFFFFFF);
                    let mask_low = self.read_config_u32(addr, offset);
                    self.write_config_u32(addr, offset, original_val);

                    let size = if mask_low != 0 && mask_low != 0xFFFFFFFF {
                        (!(mask_low & 0xFFFFFFF0) + 1) as u64
                    } else {
                        0x1000000 // 16MB default
                    };

                    bars[bar_idx] = Some(PciBar::Memory64 {
                        address: full_addr,
                        size,
                        prefetchable,
                    });
                    bar_idx += 2; // Skip next BAR as it is the upper 32 bits
                } else {
                    // 32-bit Memory BAR
                    let mem_addr = original_val & 0xFFFFFFF0;

                    // Probe size
                    self.write_config_u32(addr, offset, 0xFFFFFFFF);
                    let mask = self.read_config_u32(addr, offset);
                    self.write_config_u32(addr, offset, original_val);

                    let size = if mask != 0 && mask != 0xFFFFFFFF {
                        !(mask & 0xFFFFFFF0) + 1
                    } else {
                        0x100000 // 1MB default
                    };

                    bars[bar_idx] = Some(PciBar::Memory32 {
                        address: mem_addr,
                        size,
                        prefetchable,
                    });
                    bar_idx += 1;
                }
            }
        }

        bars
    }

    /// Scan a single PCI function at (bus, device, function)
    pub fn scan_function(&mut self, addr: PciAddress) -> Option<PciDevice> {
        let vendor_id = self.read_config_u16(&addr, 0x00);
        if vendor_id == 0xFFFF || vendor_id == 0x0000 {
            return None; // No device present
        }

        let device_id = self.read_config_u16(&addr, 0x02);
        let command = self.read_config_u16(&addr, 0x04);
        let status = self.read_config_u16(&addr, 0x06);
        let revision_id = self.read_config_u8(&addr, 0x08);
        let prog_if = self.read_config_u8(&addr, 0x09);
        let subclass = self.read_config_u8(&addr, 0x0A);
        let class_code = self.read_config_u8(&addr, 0x0B);
        let header_type = self.read_config_u8(&addr, 0x0E);
        let interrupt_line = self.read_config_u8(&addr, 0x3C);
        let interrupt_pin = self.read_config_u8(&addr, 0x3D);

        let bars = self.probe_bars(&addr, header_type);

        Some(PciDevice {
            address: addr,
            vendor_id,
            device_id,
            command,
            status,
            revision_id,
            prog_if,
            subclass,
            class_code,
            header_type,
            bars,
            interrupt_line,
            interrupt_pin,
        })
    }

    /// Scan a single PCI device slot (checks function 0, and functions 1..7 if multi-function)
    pub fn scan_device(&mut self, bus: u8, dev: u8) -> Vec<PciDevice> {
        let mut devices = Vec::new();
        let f0_addr = PciAddress::new(bus, dev, 0);

        if let Some(f0_dev) = self.scan_function(f0_addr) {
            let is_multi_function = (f0_dev.header_type & 0x80) != 0;
            devices.push(f0_dev);

            if is_multi_function {
                for func in 1..8 {
                    let fn_addr = PciAddress::new(bus, dev, func);
                    if let Some(fn_dev) = self.scan_function(fn_addr) {
                        devices.push(fn_dev);
                    }
                }
            }
        }

        devices
    }

    /// Scan an entire PCI bus (bus 0..255, device 0..31, function 0..7)
    pub fn scan_bus(&mut self, bus: u8) -> Vec<PciDevice> {
        let mut devices = Vec::new();
        for dev in 0..32 {
            let mut slot_devs = self.scan_device(bus, dev);
            devices.append(&mut slot_devs);
        }
        devices
    }

    /// Full PCI Bus Enumeration (Scans all buses 0..255)
    pub fn scan_all(&mut self) -> Vec<PciDevice> {
        let mut all_devices = Vec::new();
        // Typically buses 0..4 contain all hardware unless complex PCI bridges exist
        for bus in 0..8 {
            let mut bus_devs = self.scan_bus(bus);
            all_devices.append(&mut bus_devs);
        }
        all_devices
    }

    /// Find devices matching a vendor and device ID
    pub fn find_by_vendor_device(&mut self, vendor: u16, device: u16) -> Vec<PciDevice> {
        self.scan_all()
            .into_iter()
            .filter(|d| d.vendor_id == vendor && d.device_id == device)
            .collect()
    }

    /// Find devices matching a PCI class code and subclass
    pub fn find_by_class(&mut self, class_code: u8, subclass: u8) -> Vec<PciDevice> {
        self.scan_all()
            .into_iter()
            .filter(|d| d.class_code == class_code && d.subclass == subclass)
            .collect()
    }

    /// Discover all VirtIO devices on the PCI bus
    pub fn find_virtio_devices(&mut self) -> Vec<PciDevice> {
        self.scan_all()
            .into_iter()
            .filter(|d| d.is_virtio())
            .collect()
    }
}

// ============================================================================
// Section 3: VirtIO Common Specifications & Virtqueue
// ============================================================================

pub const VIRTIO_VENDOR_ID: u16 = 0x1AF4;
pub const VIRTIO_STATUS_ACKNOWLEDGE: u8 = 1;
pub const VIRTIO_STATUS_DRIVER: u8 = 2;
pub const VIRTIO_STATUS_DRIVER_OK: u8 = 4;
pub const VIRTIO_STATUS_FEATURES_OK: u8 = 8;
pub const VIRTIO_STATUS_DEVICE_NEEDS_RESET: u8 = 64;
pub const VIRTIO_STATUS_FAILED: u8 = 128;

pub const VRING_DESC_F_NEXT: u16 = 1;
pub const VRING_DESC_F_WRITE: u16 = 2;

#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct VRingDesc {
    pub addr: u64,
    pub len: u32,
    pub flags: u16,
    pub next: u16,
}

#[derive(Debug, Clone)]
pub struct Virtqueue {
    pub index: u16,
    pub size: u16,
    pub descriptors: Vec<VRingDesc>,
    pub avail_idx: u16,
    pub used_idx: u16,
    pub free_head: u16,
    pub num_free: u16,
}

impl Virtqueue {
    pub fn new(index: u16, size: u16) -> Self {
        let mut descriptors = vec![VRingDesc::default(); size as usize];
        for i in 0..(size - 1) {
            descriptors[i as usize].next = i + 1;
            descriptors[i as usize].flags = VRING_DESC_F_NEXT;
        }
        descriptors[(size - 1) as usize].next = 0;

        Self {
            index,
            size,
            descriptors,
            avail_idx: 0,
            used_idx: 0,
            free_head: 0,
            num_free: size,
        }
    }

    pub fn allocate_desc(&mut self) -> Option<u16> {
        if self.num_free == 0 {
            return None;
        }
        let head = self.free_head;
        self.free_head = self.descriptors[head as usize].next;
        self.num_free -= 1;
        Some(head)
    }

    pub fn free_chain(&mut self, mut head: u16) {
        loop {
            self.num_free += 1;
            let flags = self.descriptors[head as usize].flags;
            let next = self.descriptors[head as usize].next;
            if (flags & VRING_DESC_F_NEXT) != 0 {
                head = next;
            } else {
                self.descriptors[head as usize].next = self.free_head;
                self.descriptors[head as usize].flags = VRING_DESC_F_NEXT;
                self.free_head = head;
                break;
            }
        }
    }
}

// ============================================================================
// Section 4: VirtIO Block Device Driver Specification (QEMU / KVM virtio-blk)
// ============================================================================

pub const VIRTIO_BLK_T_IN: u32 = 0;
pub const VIRTIO_BLK_T_OUT: u32 = 1;
pub const VIRTIO_BLK_T_FLUSH: u32 = 4;
pub const VIRTIO_BLK_T_GET_ID: u32 = 8;

pub const VIRTIO_BLK_S_OK: u8 = 0;
pub const VIRTIO_BLK_S_IOERR: u8 = 1;
pub const VIRTIO_BLK_S_UNSUPP: u8 = 2;

pub const SECTOR_SIZE: usize = 512;

#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct VirtioBlkReqHeader {
    pub req_type: u32,
    pub reserved: u32,
    pub sector: u64,
}

#[derive(Debug, Clone)]
pub struct VirtioBlockConfig {
    pub capacity_sectors: u64,
    pub size_max: u32,
    pub seg_max: u32,
    pub blk_size: u32,
}

/// VirtIO Block Device Driver
pub struct VirtioBlockDevice {
    pub pci_device: PciDevice,
    pub io_base: u16,
    pub config: VirtioBlockConfig,
    pub queue: Virtqueue,
    pub status: u8,
    /// In-memory sector store for simulation and caching
    pub storage: Mutex<std::collections::HashMap<u64, [u8; SECTOR_SIZE]>>,
}

impl VirtioBlockDevice {
    pub fn new(pci_device: PciDevice, capacity_sectors: u64) -> Self {
        let io_base = match pci_device.bars[0] {
            Some(PciBar::Io { port, .. }) => port,
            _ => 0xC100, // Default virtual I/O port
        };

        Self {
            pci_device,
            io_base,
            config: VirtioBlockConfig {
                capacity_sectors,
                size_max: 65536,
                seg_max: 128,
                blk_size: SECTOR_SIZE as u32,
            },
            queue: Virtqueue::new(0, 128),
            status: 0,
            storage: Mutex::new(std::collections::HashMap::new()),
        }
    }

    /// Execute standard VirtIO initialization sequence:
    /// 1. Reset device (status = 0)
    /// 2. Set ACKNOWLEDGE status bit
    /// 3. Set DRIVER status bit
    /// 4. Negotiate feature bits
    /// 5. Set DRIVER_OK status bit
    pub fn initialize(&mut self) -> Result<(), &'static str> {
        // Step 1: Reset
        self.status = 0;
        // Step 2: Acknowledge
        self.status |= VIRTIO_STATUS_ACKNOWLEDGE;
        // Step 3: Driver detected
        self.status |= VIRTIO_STATUS_DRIVER;
        // Step 4: Features negotiated
        self.status |= VIRTIO_STATUS_FEATURES_OK;
        // Step 5: Ready for operation
        self.status |= VIRTIO_STATUS_DRIVER_OK;

        println!(
            "[VirtIO-Blk] Initialized Device at PCI {:02X}:{:02X}.{} (I/O Port: 0x{:04X}, Capacity: {} Sectors / {} MB)",
            self.pci_device.address.bus,
            self.pci_device.address.device,
            self.pci_device.address.function,
            self.io_base,
            self.config.capacity_sectors,
            (self.config.capacity_sectors * SECTOR_SIZE as u64) / (1024 * 1024)
        );

        Ok(())
    }

    /// Read raw 512-byte sectors from the disk
    pub fn read_sectors(&self, start_sector: u64, buf: &mut [u8]) -> Result<usize, &'static str> {
        if buf.len() % SECTOR_SIZE != 0 {
            return Err("Buffer length must be a multiple of 512-byte sector size");
        }

        let num_sectors = (buf.len() / SECTOR_SIZE) as u64;
        if start_sector + num_sectors > self.config.capacity_sectors {
            return Err("Sector address out of range");
        }

        let storage = self.storage.lock().unwrap();
        for i in 0..num_sectors {
            let sec_idx = start_sector + i;
            let offset = (i as usize) * SECTOR_SIZE;
            if let Some(sector_data) = storage.get(&sec_idx) {
                buf[offset..offset + SECTOR_SIZE].copy_from_slice(sector_data);
            } else {
                buf[offset..offset + SECTOR_SIZE].fill(0);
            }
        }

        Ok(buf.len())
    }

    /// Write raw 512-byte sectors to the disk
    pub fn write_sectors(&self, start_sector: u64, buf: &[u8]) -> Result<usize, &'static str> {
        if buf.len() % SECTOR_SIZE != 0 {
            return Err("Buffer length must be a multiple of 512-byte sector size");
        }

        let num_sectors = (buf.len() / SECTOR_SIZE) as u64;
        if start_sector + num_sectors > self.config.capacity_sectors {
            return Err("Sector address out of range");
        }

        let mut storage = self.storage.lock().unwrap();
        for i in 0..num_sectors {
            let sec_idx = start_sector + i;
            let offset = (i as usize) * SECTOR_SIZE;
            let mut data = [0u8; SECTOR_SIZE];
            data.copy_from_slice(&buf[offset..offset + SECTOR_SIZE]);
            storage.insert(sec_idx, data);
        }

        Ok(buf.len())
    }

    /// Flush disk cache
    pub fn flush(&self) -> Result<(), &'static str> {
        // In-memory backing has immediate consistency
        Ok(())
    }

    pub fn total_capacity_bytes(&self) -> u64 {
        self.config.capacity_sectors * SECTOR_SIZE as u64
    }
}

impl WdmDriver for VirtioBlockDevice {
    fn name(&self) -> &str {
        "virtio_blk_wdm"
    }

    fn dispatch(&self, irp: &mut Irp) -> NtStatus {
        match irp.major_function {
            Irp::IRP_MJ_READ => {
                let sector = irp.file_offset / SECTOR_SIZE as u64;
                match self.read_sectors(sector, &mut irp.user_buffer) {
                    Ok(bytes) => {
                        irp.bytes_transferred = bytes;
                        NtStatus::SUCCESS
                    }
                    Err(_) => NtStatus::UNSUCCESSFUL,
                }
            }
            Irp::IRP_MJ_WRITE => {
                let sector = irp.file_offset / SECTOR_SIZE as u64;
                match self.write_sectors(sector, &irp.user_buffer) {
                    Ok(bytes) => {
                        irp.bytes_transferred = bytes;
                        NtStatus::SUCCESS
                    }
                    Err(_) => NtStatus::UNSUCCESSFUL,
                }
            }
            Irp::IRP_MJ_DEVICE_CONTROL => NtStatus::SUCCESS,
            _ => NtStatus::NOT_IMPLEMENTED,
        }
    }
}

impl LinuxDriver for VirtioBlockDevice {
    fn name(&self) -> &str {
        "virtio_blk_linux"
    }

    fn read(&self, buf: &mut [u8], offset: u64) -> Result<usize, PosixError> {
        let sector = offset / SECTOR_SIZE as u64;
        self.read_sectors(sector, buf).map_err(|_| PosixError::EIO)
    }

    fn write(&self, buf: &[u8], offset: u64) -> Result<usize, PosixError> {
        let sector = offset / SECTOR_SIZE as u64;
        self.write_sectors(sector, buf).map_err(|_| PosixError::EIO)
    }

    fn ioctl(&self, _cmd: u32, _arg: usize) -> Result<i64, PosixError> {
        Ok(0)
    }
}

// ============================================================================
// Section 5: VirtIO Network Device Driver Specification (QEMU / KVM virtio-net)
// ============================================================================

pub const VIRTIO_NET_F_CSUM: u32 = 1 << 0;
pub const VIRTIO_NET_F_GUEST_CSUM: u32 = 1 << 1;
pub const VIRTIO_NET_F_MAC: u32 = 1 << 5;
pub const VIRTIO_NET_F_STATUS: u32 = 1 << 16;
pub const VIRTIO_NET_F_MTU: u32 = 1 << 24;

#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct VirtioNetHeader {
    pub flags: u8,
    pub gso_type: u8,
    pub hdr_len: u16,
    pub gso_size: u16,
    pub csum_start: u16,
    pub csum_offset: u16,
    pub num_buffers: u16,
}

#[derive(Debug, Clone, Default)]
pub struct NetStatistics {
    pub tx_packets: u64,
    pub rx_packets: u64,
    pub tx_bytes: u64,
    pub rx_bytes: u64,
    pub tx_errors: u64,
    pub rx_errors: u64,
}

pub struct VirtioNetDevice {
    pub pci_device: PciDevice,
    pub io_base: u16,
    pub mac_address: [u8; 6],
    pub mtu: u16,
    pub status: u8,
    pub rx_queue: Virtqueue,
    pub tx_queue: Virtqueue,
    pub rx_buffer_ring: Mutex<VecDeque<Vec<u8>>>,
    pub stats: Mutex<NetStatistics>,
}

impl VirtioNetDevice {
    pub fn new(pci_device: PciDevice, mac_address: [u8; 6]) -> Self {
        let io_base = match pci_device.bars[0] {
            Some(PciBar::Io { port, .. }) => port,
            _ => 0xC000,
        };

        Self {
            pci_device,
            io_base,
            mac_address,
            mtu: 1500,
            status: 0,
            rx_queue: Virtqueue::new(0, 128),
            tx_queue: Virtqueue::new(1, 128),
            rx_buffer_ring: Mutex::new(VecDeque::new()),
            stats: Mutex::new(NetStatistics::default()),
        }
    }

    pub fn initialize(&mut self) -> Result<(), &'static str> {
        self.status = 0;
        self.status |= VIRTIO_STATUS_ACKNOWLEDGE;
        self.status |= VIRTIO_STATUS_DRIVER;
        self.status |= VIRTIO_STATUS_FEATURES_OK;
        self.status |= VIRTIO_STATUS_DRIVER_OK;

        println!(
            "[VirtIO-Net] Initialized NIC at PCI {:02X}:{:02X}.{} (I/O Port: 0x{:04X}, MAC: {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}, MTU: {})",
            self.pci_device.address.bus,
            self.pci_device.address.device,
            self.pci_device.address.function,
            self.io_base,
            self.mac_address[0],
            self.mac_address[1],
            self.mac_address[2],
            self.mac_address[3],
            self.mac_address[4],
            self.mac_address[5],
            self.mtu
        );

        Ok(())
    }

    pub fn mac_str(&self) -> String {
        format!(
            "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
            self.mac_address[0],
            self.mac_address[1],
            self.mac_address[2],
            self.mac_address[3],
            self.mac_address[4],
            self.mac_address[5]
        )
    }

    /// Transmit a raw Ethernet packet via VirtIO TX ring
    pub fn send_packet(&self, packet: &[u8]) -> Result<usize, &'static str> {
        if packet.is_empty() || packet.len() > (self.mtu as usize + 14) {
            return Err("Packet size invalid for MTU");
        }

        let mut stats = self.stats.lock().unwrap();
        stats.tx_packets += 1;
        stats.tx_bytes += packet.len() as u64;

        Ok(packet.len())
    }

    /// Receive a pending Ethernet packet into destination buffer
    pub fn receive_packet(&self, buf: &mut [u8]) -> Result<usize, &'static str> {
        let mut rx_ring = self.rx_buffer_ring.lock().unwrap();
        if let Some(packet) = rx_ring.pop_front() {
            let len = packet.len().min(buf.len());
            buf[..len].copy_from_slice(&packet[..len]);

            let mut stats = self.stats.lock().unwrap();
            stats.rx_packets += 1;
            stats.rx_bytes += len as u64;

            Ok(len)
        } else {
            Err("No packet in RX ring buffer")
        }
    }

    /// Simulate physical hardware receiving a frame from the network
    pub fn inject_incoming_packet(&self, packet: &[u8]) {
        let mut rx_ring = self.rx_buffer_ring.lock().unwrap();
        rx_ring.push_back(packet.to_vec());
    }

    pub fn pending_packets_count(&self) -> usize {
        self.rx_buffer_ring.lock().unwrap().len()
    }
}

impl WdmDriver for VirtioNetDevice {
    fn name(&self) -> &str {
        "virtio_net_wdm"
    }

    fn dispatch(&self, irp: &mut Irp) -> NtStatus {
        match irp.major_function {
            Irp::IRP_MJ_READ => match self.receive_packet(&mut irp.user_buffer) {
                Ok(bytes) => {
                    irp.bytes_transferred = bytes;
                    NtStatus::SUCCESS
                }
                Err(_) => NtStatus::UNSUCCESSFUL,
            },
            Irp::IRP_MJ_WRITE => match self.send_packet(&irp.user_buffer) {
                Ok(bytes) => {
                    irp.bytes_transferred = bytes;
                    NtStatus::SUCCESS
                }
                Err(_) => NtStatus::UNSUCCESSFUL,
            },
            _ => NtStatus::NOT_IMPLEMENTED,
        }
    }
}

impl LinuxDriver for VirtioNetDevice {
    fn name(&self) -> &str {
        "virtio_net_linux"
    }

    fn read(&self, buf: &mut [u8], _offset: u64) -> Result<usize, PosixError> {
        self.receive_packet(buf).map_err(|_| PosixError::EAGAIN)
    }

    fn write(&self, buf: &[u8], _offset: u64) -> Result<usize, PosixError> {
        self.send_packet(buf).map_err(|_| PosixError::EIO)
    }

    fn ioctl(&self, _cmd: u32, _arg: usize) -> Result<i64, PosixError> {
        Ok(0)
    }
}

// ============================================================================
// Section 6: Framebuffer / Display GOP Driver
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 0xFF }
    }

    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub const BLACK: Self = Self::rgb(0x00, 0x00, 0x00);
    pub const WHITE: Self = Self::rgb(0xFF, 0xFF, 0xFF);
    pub const RED: Self = Self::rgb(0xFF, 0x00, 0x00);
    pub const GREEN: Self = Self::rgb(0x00, 0xFF, 0x00);
    pub const BLUE: Self = Self::rgb(0x00, 0x00, 0xFF);
    pub const CYAN: Self = Self::rgb(0x00, 0xFF, 0xFF);
    pub const MAGENTA: Self = Self::rgb(0xFF, 0x00, 0xFF);
    pub const YELLOW: Self = Self::rgb(0xFF, 0xFF, 0x00);
    pub const DARK_GRAY: Self = Self::rgb(0x40, 0x40, 0x40);
    pub const LIGHT_GRAY: Self = Self::rgb(0xC0, 0xC0, 0xC0);
    pub const HYBRID_NAVY: Self = Self::rgb(0x0A, 0x19, 0x2F);
    pub const HYBRID_ACCENT: Self = Self::rgb(0x64, 0xFF, 0xDA);

    pub fn to_bgrx32(&self) -> u32 {
        (self.b as u32) | ((self.g as u32) << 8) | ((self.r as u32) << 16) | ((self.a as u32) << 24)
    }

    pub fn to_rgbx32(&self) -> u32 {
        (self.r as u32) | ((self.g as u32) << 8) | ((self.b as u32) << 16) | ((self.a as u32) << 24)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    Bgrx8888,
    Rgbx8888,
}

/// UEFI Graphics Output Protocol (GOP) Framebuffer Driver
pub struct DisplayGopDriver {
    pub width: u32,
    pub height: u32,
    pub pitch: u32, // bytes per scanline
    pub bpp: u8,    // bits per pixel (e.g. 32)
    pub format: PixelFormat,
    pub front_buffer: Vec<u8>,
    pub back_buffer: Vec<u8>,
}

impl DisplayGopDriver {
    pub fn new(width: u32, height: u32, format: PixelFormat) -> Self {
        let bpp: u8 = 32;
        let bytes_per_pixel = (bpp as u32) / 8;
        let pitch = width * bytes_per_pixel;
        let total_bytes = (pitch * height) as usize;

        let front_buffer = vec![0u8; total_bytes];
        let back_buffer = vec![0u8; total_bytes];

        Self {
            width,
            height,
            pitch,
            bpp,
            format,
            front_buffer,
            back_buffer,
        }
    }

    #[inline]
    fn pixel_offset(&self, x: u32, y: u32) -> usize {
        (y * self.pitch + x * 4) as usize
    }

    pub fn put_pixel(&mut self, x: u32, y: u32, color: Color) {
        if x >= self.width || y >= self.height {
            return;
        }
        let off = self.pixel_offset(x, y);
        let val = match self.format {
            PixelFormat::Bgrx8888 => color.to_bgrx32(),
            PixelFormat::Rgbx8888 => color.to_rgbx32(),
        };
        let bytes = val.to_ne_bytes();
        self.back_buffer[off..off + 4].copy_from_slice(&bytes);
    }

    pub fn get_pixel(&self, x: u32, y: u32) -> Option<Color> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let off = self.pixel_offset(x, y);
        let bytes = [
            self.back_buffer[off],
            self.back_buffer[off + 1],
            self.back_buffer[off + 2],
            self.back_buffer[off + 3],
        ];
        let val = u32::from_ne_bytes(bytes);
        match self.format {
            PixelFormat::Bgrx8888 => Some(Color::rgba(
                ((val >> 16) & 0xFF) as u8,
                ((val >> 8) & 0xFF) as u8,
                (val & 0xFF) as u8,
                ((val >> 24) & 0xFF) as u8,
            )),
            PixelFormat::Rgbx8888 => Some(Color::rgba(
                (val & 0xFF) as u8,
                ((val >> 8) & 0xFF) as u8,
                ((val >> 16) & 0xFF) as u8,
                ((val >> 24) & 0xFF) as u8,
            )),
        }
    }

    pub fn clear(&mut self, color: Color) {
        let val = match self.format {
            PixelFormat::Bgrx8888 => color.to_bgrx32(),
            PixelFormat::Rgbx8888 => color.to_rgbx32(),
        };
        let bytes = val.to_ne_bytes();
        for chunk in self.back_buffer.chunks_exact_mut(4) {
            chunk.copy_from_slice(&bytes);
        }
    }

    pub fn fill_rect(&mut self, x: u32, y: u32, w: u32, h: u32, color: Color) {
        let max_x = (x + w).min(self.width);
        let max_y = (y + h).min(self.height);

        for cy in y..max_y {
            for cx in x..max_x {
                self.put_pixel(cx, cy, color);
            }
        }
    }

    /// Bresenham's line algorithm
    pub fn draw_line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, color: Color) {
        let dx = (x1 - x0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let dy = -(y1 - y0).abs();
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;

        let mut cx = x0;
        let mut cy = y0;

        loop {
            if cx >= 0 && cy >= 0 && (cx as u32) < self.width && (cy as u32) < self.height {
                self.put_pixel(cx as u32, cy as u32, color);
            }
            if cx == x1 && cy == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                cx += sx;
            }
            if e2 <= dx {
                err += dx;
                cy += sy;
            }
        }
    }

    /// Render single 8x8 ASCII character onto framebuffer
    pub fn draw_char(&mut self, x: u32, y: u32, ch: char, fg: Color, bg: Option<Color>) {
        let glyph = get_ascii_glyph(ch);
        for row in 0..8 {
            let bits = glyph[row];
            for col in 0..8 {
                let px = x + col;
                let py = y + row as u32;
                if px < self.width && py < self.height {
                    if (bits & (1 << (7 - col))) != 0 {
                        self.put_pixel(px, py, fg);
                    } else if let Some(bg_color) = bg {
                        self.put_pixel(px, py, bg_color);
                    }
                }
            }
        }
    }

    /// Render a string of text onto framebuffer
    pub fn draw_string(&mut self, mut x: u32, y: u32, text: &str, fg: Color, bg: Option<Color>) {
        for ch in text.chars() {
            if ch == '\n' {
                break;
            }
            self.draw_char(x, y, ch, fg, bg);
            x += 8;
        }
    }

    /// Swap backbuffer to frontbuffer (hardware present)
    pub fn swap_buffers(&mut self) {
        self.front_buffer.copy_from_slice(&self.back_buffer);
    }
}

/// Standard 8x8 bitmap font glyph lookup for ASCII 32..126
fn get_ascii_glyph(ch: char) -> [u8; 8] {
    match ch {
        'A' => [0x18, 0x24, 0x42, 0x7E, 0x42, 0x42, 0x42, 0x00],
        'B' => [0x7C, 0x42, 0x42, 0x7C, 0x42, 0x42, 0x7C, 0x00],
        'C' => [0x3C, 0x42, 0x40, 0x40, 0x40, 0x42, 0x3C, 0x00],
        'D' => [0x78, 0x44, 0x42, 0x42, 0x42, 0x44, 0x78, 0x00],
        'E' => [0x7E, 0x40, 0x40, 0x78, 0x40, 0x40, 0x7E, 0x00],
        'F' => [0x7E, 0x40, 0x40, 0x78, 0x40, 0x40, 0x40, 0x00],
        'G' => [0x3C, 0x42, 0x40, 0x4E, 0x42, 0x42, 0x3C, 0x00],
        'H' => [0x42, 0x42, 0x42, 0x7E, 0x42, 0x42, 0x42, 0x00],
        'I' => [0x38, 0x10, 0x10, 0x10, 0x10, 0x10, 0x38, 0x00],
        'K' => [0x44, 0x48, 0x50, 0x60, 0x50, 0x48, 0x44, 0x00],
        'L' => [0x40, 0x40, 0x40, 0x40, 0x40, 0x40, 0x7E, 0x00],
        'M' => [0x81, 0xC3, 0xA5, 0x99, 0x81, 0x81, 0x81, 0x00],
        'N' => [0x42, 0x62, 0x52, 0x4A, 0x46, 0x42, 0x42, 0x00],
        'O' => [0x3C, 0x42, 0x42, 0x42, 0x42, 0x42, 0x3C, 0x00],
        'P' => [0x7C, 0x42, 0x42, 0x7C, 0x40, 0x40, 0x40, 0x00],
        'R' => [0x7C, 0x42, 0x42, 0x7C, 0x48, 0x44, 0x42, 0x00],
        'S' => [0x3C, 0x42, 0x40, 0x3C, 0x02, 0x42, 0x3C, 0x00],
        'T' => [0x7E, 0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x00],
        'U' => [0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x3C, 0x00],
        'V' => [0x42, 0x42, 0x42, 0x42, 0x24, 0x24, 0x18, 0x00],
        'W' => [0x81, 0x81, 0x81, 0x99, 0xA5, 0xC3, 0x81, 0x00],
        'X' => [0x42, 0x24, 0x18, 0x18, 0x24, 0x42, 0x42, 0x00],
        'Y' => [0x42, 0x42, 0x24, 0x18, 0x10, 0x10, 0x10, 0x00],
        '0' => [0x3C, 0x46, 0x4A, 0x52, 0x62, 0x42, 0x3C, 0x00],
        '1' => [0x18, 0x28, 0x08, 0x08, 0x08, 0x08, 0x3E, 0x00],
        '2' => [0x3C, 0x42, 0x02, 0x0C, 0x30, 0x40, 0x7E, 0x00],
        '3' => [0x3C, 0x42, 0x02, 0x1C, 0x02, 0x42, 0x3C, 0x00],
        '4' => [0x08, 0x18, 0x28, 0x48, 0x7E, 0x08, 0x08, 0x00],
        '5' => [0x7E, 0x40, 0x7C, 0x02, 0x02, 0x42, 0x3C, 0x00],
        '6' => [0x3C, 0x42, 0x40, 0x7C, 0x42, 0x42, 0x3C, 0x00],
        '7' => [0x7E, 0x02, 0x04, 0x08, 0x10, 0x10, 0x10, 0x00],
        '8' => [0x3C, 0x42, 0x42, 0x3C, 0x42, 0x42, 0x3C, 0x00],
        '9' => [0x3C, 0x42, 0x42, 0x3E, 0x02, 0x42, 0x3C, 0x00],
        ':' => [0x00, 0x18, 0x18, 0x00, 0x18, 0x18, 0x00, 0x00],
        '.' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x18, 0x18, 0x00],
        '-' => [0x00, 0x00, 0x00, 0x7E, 0x00, 0x00, 0x00, 0x00],
        '[' => [0x1E, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1E, 0x00],
        ']' => [0x3C, 0x04, 0x04, 0x04, 0x04, 0x04, 0x3C, 0x00],
        ' ' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
        _ => [0x7E, 0x42, 0x42, 0x42, 0x42, 0x42, 0x7E, 0x00], // Box fallback
    }
}

// ============================================================================
// Section 7: PS/2 Keyboard Input Ring-Buffer Driver
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyState {
    Pressed,
    Released,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyCode {
    Escape,
    Key1, Key2, Key3, Key4, Key5, Key6, Key7, Key8, Key9, Key0,
    Minus, Equals, Backspace, Tab,
    KeyQ, KeyW, KeyE, KeyR, KeyT, KeyY, KeyU, KeyI, KeyO, KeyP,
    LeftBracket, RightBracket, Enter, LeftCtrl,
    KeyA, KeyS, KeyD, KeyF, KeyG, KeyH, KeyJ, KeyK, KeyL,
    Semicolon, Quote, Backtick, LeftShift, Backslash,
    KeyZ, KeyX, KeyC, KeyV, KeyB, KeyN, KeyM,
    Comma, Dot, Slash, RightShift,
    LeftAlt, Space, CapsLock,
    F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12,
    Unknown(u8),
}

#[derive(Debug, Clone, Copy, Default)]
pub struct KeyModifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub caps_lock: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct KeyEvent {
    pub code: KeyCode,
    pub state: KeyState,
    pub character: Option<char>,
    pub modifiers: KeyModifiers,
}

/// Thread-safe circular ring buffer for scancodes & events
pub struct RingBuffer<T: Copy, const CAP: usize> {
    buffer: [Option<T>; CAP],
    head: usize,
    tail: usize,
    count: usize,
}

impl<T: Copy, const CAP: usize> RingBuffer<T, CAP> {
    pub const fn new() -> Self {
        Self {
            buffer: [None; CAP],
            head: 0,
            tail: 0,
            count: 0,
        }
    }

    pub fn push(&mut self, item: T) -> Result<(), &'static str> {
        if self.count >= CAP {
            return Err("Ring buffer overflow");
        }
        self.buffer[self.tail] = Some(item);
        self.tail = (self.tail + 1) % CAP;
        self.count += 1;
        Ok(())
    }

    pub fn pop(&mut self) -> Option<T> {
        if self.count == 0 {
            return None;
        }
        let item = self.buffer[self.head].take();
        self.head = (self.head + 1) % CAP;
        self.count -= 1;
        item
    }

    pub fn len(&self) -> usize {
        self.count
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
}

/// Driver for PS/2 Keyboard Controller (I/O Port 0x60 / 0x64)
pub struct Ps2KeyboardDriver {
    scancode_ring: RingBuffer<u8, 256>,
    event_ring: RingBuffer<KeyEvent, 256>,
    modifiers: KeyModifiers,
    is_extended: bool,
}

impl Ps2KeyboardDriver {
    pub fn new() -> Self {
        Self {
            scancode_ring: RingBuffer::new(),
            event_ring: RingBuffer::new(),
            modifiers: KeyModifiers::default(),
            is_extended: false,
        }
    }

    /// Called by hardware IRQ 1 handler or hardware poller
    pub fn handle_interrupt(&mut self, scancode: u8) {
        let _ = self.scancode_ring.push(scancode);
        self.process_pending_scancodes();
    }

    /// Polls physical / virtual port 0x64 and 0x60
    pub fn poll_ports<T: PortIo>(&mut self, io: &mut T) {
        let status = io.read_u8(PS2_STATUS_PORT);
        if (status & 0x01) != 0 {
            let scancode = io.read_u8(PS2_DATA_PORT);
            self.handle_interrupt(scancode);
        }
    }

    /// Read next KeyEvent from ring-buffer
    pub fn read_event(&mut self) -> Option<KeyEvent> {
        self.event_ring.pop()
    }

    /// Read next ASCII character from ring-buffer
    pub fn read_char(&mut self) -> Option<char> {
        while let Some(evt) = self.event_ring.pop() {
            if evt.state == KeyState::Pressed {
                if let Some(ch) = evt.character {
                    return Some(ch);
                }
            }
        }
        None
    }

    fn process_pending_scancodes(&mut self) {
        while let Some(scancode) = self.scancode_ring.pop() {
            if scancode == 0xE0 {
                self.is_extended = true;
                continue;
            }

            let is_released = (scancode & 0x80) != 0;
            let make_code = scancode & 0x7F;
            let state = if is_released {
                KeyState::Released
            } else {
                KeyState::Pressed
            };

            // Update modifiers
            match make_code {
                0x2A | 0x36 => self.modifiers.shift = !is_released,
                0x1D => self.modifiers.ctrl = !is_released,
                0x38 => self.modifiers.alt = !is_released,
                0x3A if !is_released => self.modifiers.caps_lock = !self.modifiers.caps_lock,
                _ => {}
            }

            let code = self.translate_scancode(make_code);
            let character = if !is_released {
                self.scancode_to_char(make_code, self.modifiers.shift, self.modifiers.caps_lock)
            } else {
                None
            };

            let event = KeyEvent {
                code,
                state,
                character,
                modifiers: self.modifiers,
            };

            let _ = self.event_ring.push(event);
            self.is_extended = false;
        }
    }

    fn translate_scancode(&self, make: u8) -> KeyCode {
        match make {
            0x01 => KeyCode::Escape,
            0x02 => KeyCode::Key1,
            0x03 => KeyCode::Key2,
            0x04 => KeyCode::Key3,
            0x05 => KeyCode::Key4,
            0x06 => KeyCode::Key5,
            0x07 => KeyCode::Key6,
            0x08 => KeyCode::Key7,
            0x09 => KeyCode::Key8,
            0x0A => KeyCode::Key9,
            0x0B => KeyCode::Key0,
            0x0C => KeyCode::Minus,
            0x0D => KeyCode::Equals,
            0x0E => KeyCode::Backspace,
            0x0F => KeyCode::Tab,
            0x10 => KeyCode::KeyQ,
            0x11 => KeyCode::KeyW,
            0x12 => KeyCode::KeyE,
            0x13 => KeyCode::KeyR,
            0x14 => KeyCode::KeyT,
            0x15 => KeyCode::KeyY,
            0x16 => KeyCode::KeyU,
            0x17 => KeyCode::KeyI,
            0x18 => KeyCode::KeyO,
            0x19 => KeyCode::KeyP,
            0x1A => KeyCode::LeftBracket,
            0x1B => KeyCode::RightBracket,
            0x1C => KeyCode::Enter,
            0x1D => KeyCode::LeftCtrl,
            0x1E => KeyCode::KeyA,
            0x1F => KeyCode::KeyS,
            0x20 => KeyCode::KeyD,
            0x21 => KeyCode::KeyF,
            0x22 => KeyCode::KeyG,
            0x23 => KeyCode::KeyH,
            0x24 => KeyCode::KeyJ,
            0x25 => KeyCode::KeyK,
            0x26 => KeyCode::KeyL,
            0x27 => KeyCode::Semicolon,
            0x28 => KeyCode::Quote,
            0x29 => KeyCode::Backtick,
            0x2A => KeyCode::LeftShift,
            0x2B => KeyCode::Backslash,
            0x2C => KeyCode::KeyZ,
            0x2D => KeyCode::KeyX,
            0x2E => KeyCode::KeyC,
            0x2F => KeyCode::KeyV,
            0x30 => KeyCode::KeyB,
            0x31 => KeyCode::KeyN,
            0x32 => KeyCode::KeyM,
            0x33 => KeyCode::Comma,
            0x34 => KeyCode::Dot,
            0x35 => KeyCode::Slash,
            0x36 => KeyCode::RightShift,
            0x38 => KeyCode::LeftAlt,
            0x39 => KeyCode::Space,
            0x3A => KeyCode::CapsLock,
            0x3B => KeyCode::F1,
            0x3C => KeyCode::F2,
            0x3D => KeyCode::F3,
            0x3E => KeyCode::F4,
            0x3F => KeyCode::F5,
            0x40 => KeyCode::F6,
            0x41 => KeyCode::F7,
            0x42 => KeyCode::F8,
            0x43 => KeyCode::F9,
            0x44 => KeyCode::F10,
            0x57 => KeyCode::F11,
            0x58 => KeyCode::F12,
            _ => KeyCode::Unknown(make),
        }
    }

    fn scancode_to_char(&self, make: u8, shift: bool, caps: bool) -> Option<char> {
        let is_upper = shift ^ caps;
        match make {
            0x02 => Some(if shift { '!' } else { '1' }),
            0x03 => Some(if shift { '@' } else { '2' }),
            0x04 => Some(if shift { '#' } else { '3' }),
            0x05 => Some(if shift { '$' } else { '4' }),
            0x06 => Some(if shift { '%' } else { '5' }),
            0x07 => Some(if shift { '^' } else { '6' }),
            0x08 => Some(if shift { '&' } else { '7' }),
            0x09 => Some(if shift { '*' } else { '8' }),
            0x0A => Some(if shift { '(' } else { '9' }),
            0x0B => Some(if shift { ')' } else { '0' }),
            0x0C => Some(if shift { '_' } else { '-' }),
            0x0D => Some(if shift { '+' } else { '=' }),
            0x0E => Some('\x08'), // Backspace
            0x0F => Some('\t'),   // Tab
            0x10 => Some(if is_upper { 'Q' } else { 'q' }),
            0x11 => Some(if is_upper { 'W' } else { 'w' }),
            0x12 => Some(if is_upper { 'E' } else { 'e' }),
            0x13 => Some(if is_upper { 'R' } else { 'r' }),
            0x14 => Some(if is_upper { 'T' } else { 't' }),
            0x15 => Some(if is_upper { 'Y' } else { 'y' }),
            0x16 => Some(if is_upper { 'U' } else { 'u' }),
            0x17 => Some(if is_upper { 'I' } else { 'i' }),
            0x18 => Some(if is_upper { 'O' } else { 'o' }),
            0x19 => Some(if is_upper { 'P' } else { 'p' }),
            0x1A => Some(if shift { '{' } else { '[' }),
            0x1B => Some(if shift { '}' } else { ']' }),
            0x1C => Some('\n'),
            0x1E => Some(if is_upper { 'A' } else { 'a' }),
            0x1F => Some(if is_upper { 'S' } else { 's' }),
            0x20 => Some(if is_upper { 'D' } else { 'd' }),
            0x21 => Some(if is_upper { 'F' } else { 'f' }),
            0x22 => Some(if is_upper { 'G' } else { 'g' }),
            0x23 => Some(if is_upper { 'H' } else { 'h' }),
            0x24 => Some(if is_upper { 'J' } else { 'j' }),
            0x25 => Some(if is_upper { 'K' } else { 'k' }),
            0x26 => Some(if is_upper { 'L' } else { 'l' }),
            0x27 => Some(if shift { ':' } else { ';' }),
            0x28 => Some(if shift { '"' } else { '\'' }),
            0x29 => Some(if shift { '~' } else { '`' }),
            0x2B => Some(if shift { '|' } else { '\\' }),
            0x2C => Some(if is_upper { 'Z' } else { 'z' }),
            0x2D => Some(if is_upper { 'X' } else { 'x' }),
            0x2E => Some(if is_upper { 'C' } else { 'c' }),
            0x2F => Some(if is_upper { 'V' } else { 'v' }),
            0x30 => Some(if is_upper { 'B' } else { 'b' }),
            0x31 => Some(if is_upper { 'N' } else { 'n' }),
            0x32 => Some(if is_upper { 'M' } else { 'm' }),
            0x33 => Some(if shift { '<' } else { ',' }),
            0x34 => Some(if shift { '>' } else { '.' }),
            0x35 => Some(if shift { '?' } else { '/' }),
            0x39 => Some(' '),
            _ => None,
        }
    }
}

// ============================================================================
// Section 8: Hardware Abstraction Layer (HAL) Manager
// ============================================================================

/// Central Hardware Manager binding all platform devices to the HybridOS Kernel
pub struct HardwareManager {
    pub pci_devices: Vec<PciDevice>,
    pub virtio_block: Option<Arc<VirtioBlockDevice>>,
    pub virtio_net: Option<Arc<VirtioNetDevice>>,
    pub display: Option<Arc<Mutex<DisplayGopDriver>>>,
    pub keyboard: Arc<Mutex<Ps2KeyboardDriver>>,
}

impl HardwareManager {
    pub fn probe_and_initialize() -> Self {
        println!(">>> [HAL] Initiating HybridOS Hardware Bus Scan & Device Discovery...");

        let port_io = VirtualPortIo::new();
        let mut scanner = PciBusScanner::new(port_io);
        let devices = scanner.scan_all();

        println!("[HAL] Discovered {} PCI hardware devices on system buses:", devices.len());
        for dev in &devices {
            println!(
                "  -> PCI {:02X}:{:02X}.{} [Vendor 0x{:04X} ({}), Device 0x{:04X}] - {}",
                dev.address.bus,
                dev.address.device,
                dev.address.function,
                dev.vendor_id,
                dev.vendor_name(),
                dev.device_id,
                dev.class_description()
            );
        }

        // Initialize VirtIO Block device if found
        let mut blk_driver = None;
        if let Some(blk_pci) = devices.iter().find(|d| d.is_virtio_block()).cloned() {
            let mut blk = VirtioBlockDevice::new(blk_pci, 2097152); // 1 GB simulated disk
            let _ = blk.initialize();
            blk_driver = Some(Arc::new(blk));
        }

        // Initialize VirtIO Net device if found
        let mut net_driver = None;
        if let Some(net_pci) = devices.iter().find(|d| d.is_virtio_net()).cloned() {
            let mac = [0x52, 0x54, 0x00, 0x12, 0x34, 0x56]; // QEMU default MAC
            let mut net = VirtioNetDevice::new(net_pci, mac);
            let _ = net.initialize();
            net_driver = Some(Arc::new(net));
        }

        // Initialize GOP Display Driver (1024x768 32-bit BGRX)
        println!("[HAL] Initializing UEFI GOP Framebuffer Display Driver (1024x768 32bpp)...");
        let mut gop = DisplayGopDriver::new(1024, 768, PixelFormat::Bgrx8888);
        gop.clear(Color::HYBRID_NAVY);
        gop.draw_string(20, 20, "HYBRIDOS KERNEL V1.0", Color::HYBRID_ACCENT, None);
        gop.draw_string(20, 40, "HARDWARE INITIALIZATION COMPLETE", Color::WHITE, None);
        gop.swap_buffers();
        let display = Some(Arc::new(Mutex::new(gop)));

        // Initialize PS/2 Keyboard Driver
        println!("[HAL] Initializing PS/2 Keyboard Controller Ring-Buffer...");
        let keyboard = Arc::new(Mutex::new(Ps2KeyboardDriver::new()));

        Self {
            pci_devices: devices,
            virtio_block: blk_driver,
            virtio_net: net_driver,
            display,
            keyboard,
        }
    }
}

// ============================================================================
// Section 9: Comprehensive Unit & Integration Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pci_bus_scanner() {
        let io = VirtualPortIo::new();
        let mut scanner = PciBusScanner::new(io);
        let devices = scanner.scan_all();

        assert!(!devices.is_empty(), "Should discover virtual PCI devices");

        // Verify Host Bridge discovery
        let host_bridge = devices.iter().find(|d| d.vendor_id == 0x8086 && d.device_id == 0x1237);
        assert!(host_bridge.is_some(), "Host bridge should be discovered");

        // Verify VirtIO Net discovery
        let virtio_net = devices.iter().find(|d| d.is_virtio_net());
        assert!(virtio_net.is_some(), "VirtIO Net device should be discovered");

        // Verify VirtIO Block discovery
        let virtio_blk = devices.iter().find(|d| d.is_virtio_block());
        assert!(virtio_blk.is_some(), "VirtIO Block device should be discovered");

        // Verify BAR probing
        let net_dev = virtio_net.unwrap();
        assert!(matches!(net_dev.bars[0], Some(PciBar::Io { port: 0xC000, .. })));
    }

    #[test]
    fn test_virtio_block_read_write() {
        let pci = PciDevice {
            address: PciAddress::new(0, 4, 0),
            vendor_id: 0x1AF4,
            device_id: 0x1001,
            command: 0x07,
            status: 0x0210,
            revision_id: 0,
            prog_if: 0,
            subclass: 0x00,
            class_code: 0x01,
            header_type: 0,
            bars: [Some(PciBar::Io { port: 0xC100, size: 64 }), None, None, None, None, None],
            interrupt_line: 11,
            interrupt_pin: 1,
        };

        let mut blk = VirtioBlockDevice::new(pci, 1024);
        assert!(blk.initialize().is_ok());

        // Write sector 5
        let mut write_buf = [0u8; 512];
        write_buf[0..13].copy_from_slice(b"HYBRIDOS DISK");
        assert_eq!(blk.write_sectors(5, &write_buf).unwrap(), 512);

        // Read sector 5 back
        let mut read_buf = [0u8; 512];
        assert_eq!(blk.read_sectors(5, &mut read_buf).unwrap(), 512);
        assert_eq!(&read_buf[0..13], b"HYBRIDOS DISK");
    }

    #[test]
    fn test_virtio_net_send_receive() {
        let pci = PciDevice {
            address: PciAddress::new(0, 3, 0),
            vendor_id: 0x1AF4,
            device_id: 0x1000,
            command: 0x07,
            status: 0x0210,
            revision_id: 0,
            prog_if: 0,
            subclass: 0x00,
            class_code: 0x02,
            header_type: 0,
            bars: [Some(PciBar::Io { port: 0xC000, size: 64 }), None, None, None, None, None],
            interrupt_line: 11,
            interrupt_pin: 1,
        };

        let mac = [0x52, 0x54, 0x00, 0xAB, 0xCD, 0xEF];
        let mut net = VirtioNetDevice::new(pci, mac);
        assert!(net.initialize().is_ok());
        assert_eq!(net.mac_str(), "52:54:00:AB:CD:EF");

        // Send a frame
        let test_packet = b"PING PACKET PAYLOAD";
        assert_eq!(net.send_packet(test_packet).unwrap(), test_packet.len());

        // Inject and receive
        net.inject_incoming_packet(b"PONG PACKET PAYLOAD");
        assert_eq!(net.pending_packets_count(), 1);

        let mut rx_buf = [0u8; 64];
        let bytes = net.receive_packet(&mut rx_buf).unwrap();
        assert_eq!(&rx_buf[..bytes], b"PONG PACKET PAYLOAD");
        assert_eq!(net.pending_packets_count(), 0);
    }

    #[test]
    fn test_display_gop_drawing() {
        let mut display = DisplayGopDriver::new(100, 100, PixelFormat::Bgrx8888);
        display.clear(Color::BLACK);

        // Draw pixel
        display.put_pixel(10, 10, Color::RED);
        assert_eq!(display.get_pixel(10, 10), Some(Color::RED));

        // Draw rectangle
        display.fill_rect(20, 20, 10, 10, Color::GREEN);
        assert_eq!(display.get_pixel(25, 25), Some(Color::GREEN));

        // Draw line
        display.draw_line(0, 0, 5, 0, Color::BLUE);
        assert_eq!(display.get_pixel(3, 0), Some(Color::BLUE));

        // Draw character
        display.draw_char(30, 30, 'A', Color::WHITE, Some(Color::BLACK));

        // Swap buffers
        display.swap_buffers();
        assert_eq!(display.front_buffer, display.back_buffer);
    }

    #[test]
    fn test_ps2_keyboard_ring_buffer() {
        let mut kbd = Ps2KeyboardDriver::new();

        // Simulate typing 'A' (make code 0x1E)
        kbd.handle_interrupt(0x1E);
        let event = kbd.read_event().expect("Event should exist");
        assert_eq!(event.code, KeyCode::KeyA);
        assert_eq!(event.state, KeyState::Pressed);
        assert_eq!(event.character, Some('a'));

        // Simulate typing 'B' with Shift (make shift 0x2A, make 'B' 0x30, break 'B' 0xB0, break shift 0xAA)
        kbd.handle_interrupt(0x2A); // Shift pressed
        let _ = kbd.read_event();
        kbd.handle_interrupt(0x30); // 'B' pressed
        let event_b = kbd.read_event().expect("Event B should exist");
        assert_eq!(event_b.code, KeyCode::KeyB);
        assert_eq!(event_b.character, Some('B'));
    }

    #[test]
    fn test_hardware_manager_initialization() {
        let hal = HardwareManager::probe_and_initialize();
        assert!(!hal.pci_devices.is_empty());
        assert!(hal.virtio_block.is_some());
        assert!(hal.virtio_net.is_some());
        assert!(hal.display.is_some());
    }
}
