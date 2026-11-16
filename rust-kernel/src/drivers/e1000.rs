//! Real Bare-Metal Intel 82540EM (E1000) Gigabit Network Controller Driver.
//!
//! Scans the PCI bus for Vendor 0x8086, Device 0x100E, enables Bus Mastering & MMIO,
//! maps BAR0, reads the authentic physical MAC Address from RAL/RAH registers,
//! and configures DMA Receive/Transmit Descriptor Rings with contiguous 2048-byte physical buffers.

use crate::arch::msr::{inl, outl};
use crate::drivers::hardware::{PCI_CONFIG_ADDRESS, PCI_CONFIG_DATA, PciAddress};
use crate::sync::SpinLock;

pub const E1000_VENDOR_ID: u16 = 0x8086; // Intel Corporation
pub const E1000_DEVICE_ID: u16 = 0x100E; // 82540EM Gigabit Ethernet Controller

// E1000 MMIO Register Offsets
pub const REG_CTRL: usize    = 0x0000;
pub const REG_STATUS: usize  = 0x0008;
pub const REG_EECD: usize    = 0x0010;
pub const REG_EERD: usize    = 0x0014;
pub const REG_ICR: usize     = 0x00C0;
pub const REG_IMS: usize     = 0x00D0;
pub const REG_IMC: usize     = 0x00D8;
pub const REG_RCTL: usize    = 0x0100;
pub const REG_TCTL: usize    = 0x0400;
pub const REG_RDBAL: usize   = 0x2800;
pub const REG_RDBAH: usize   = 0x2804;
pub const REG_RDLEN: usize   = 0x2808;
pub const REG_RDH: usize     = 0x2810;
pub const REG_RDT: usize     = 0x2818;
pub const REG_TDBAL: usize   = 0x3800;
pub const REG_TDBAH: usize   = 0x3804;
pub const REG_TDLEN: usize   = 0x3808;
pub const REG_TDH: usize     = 0x3810;
pub const REG_TDT: usize     = 0x3818;
pub const REG_MTA: usize     = 0x5200;
pub const REG_RAL0: usize    = 0x5400;
pub const REG_RAH0: usize    = 0x5404;

pub const CTRL_SLU: u32   = 1 << 6;  // Set Link Up

pub const RCTL_EN: u32    = 1 << 1;  // Receiver Enable
pub const RCTL_SBP: u32   = 1 << 2;  // Store Bad Packets
pub const RCTL_UPE: u32   = 1 << 3;  // Unicast Promiscuous
pub const RCTL_MPE: u32   = 1 << 4;  // Multicast Promiscuous
pub const RCTL_BAM: u32   = 1 << 15; // Broadcast Accept Mode
pub const RCTL_BSIZE_2048: u32 = 0 << 16; // 2048-byte buffer
pub const RCTL_SECRC: u32 = 1 << 26; // Strip Ethernet CRC

pub const TCTL_EN: u32    = 1 << 1;  // Transmit Enable
pub const TCTL_PSP: u32   = 1 << 3;  // Pad Short Packets
pub const TCTL_CT: u32    = 0x0F << 4; // Collision Threshold
pub const TCTL_COLD: u32  = 0x40 << 12; // Collision Distance

pub const NUM_RX_DESCRIPTORS: usize = 32;
pub const NUM_TX_DESCRIPTORS: usize = 32;
pub const BUFFER_SIZE: usize = 2048;

/// Standard 16-byte E1000 Legacy Receive Descriptor
#[repr(C, packed)]
#[derive(Clone, Copy, Default)]
pub struct E1000RxDesc {
    pub buffer_addr: u64,
    pub length: u16,
    pub checksum: u16,
    pub status: u8,
    pub errors: u8,
    pub special: u16,
}

/// Standard 16-byte E1000 Legacy Transmit Descriptor
#[repr(C, packed)]
#[derive(Clone, Copy, Default)]
pub struct E1000TxDesc {
    pub buffer_addr: u64,
    pub length: u16,
    pub cso: u8,
    pub cmd: u8,
    pub status: u8,
    pub css: u8,
    pub special: u16,
}

pub struct E1000Device {
    pub pci_addr: PciAddress,
    pub mmio_base: u64,
    pub mac_address: [u8; 6],
    pub rx_ring_phys: u64,
    pub tx_ring_phys: u64,
    pub rx_bufs_phys: u64,
    pub tx_bufs_phys: u64,
    pub rx_cur: usize,
    pub tx_cur: usize,
    pub stats_rx_packets: u64,
    pub stats_rx_bytes: u64,
    pub stats_tx_packets: u64,
    pub stats_tx_bytes: u64,
}

pub static GLOBAL_E1000: SpinLock<Option<E1000Device>> = SpinLock::new(None);

impl E1000Device {
    #[inline]
    pub unsafe fn write32(&self, offset: usize, val: u32) {
        let ptr = (self.mmio_base + offset as u64) as *mut u32;
        core::ptr::write_volatile(ptr, val);
    }

    #[inline]
    pub unsafe fn read32(&self, offset: usize) -> u32 {
        let ptr = (self.mmio_base + offset as u64) as *const u32;
        core::ptr::read_volatile(ptr)
    }

    /// Reads 16-bit word from EEPROM register (EERD)
    fn read_eeprom(&self, addr: u8) -> u16 {
        unsafe {
            self.write32(REG_EERD, 1 | ((addr as u32) << 8));
            for _ in 0..100_000 {
                let val = self.read32(REG_EERD);
                if (val & (1 << 4)) != 0 {
                    return ((val >> 16) & 0xFFFF) as u16;
                }
            }
        }
        0
    }

    /// Reads physical MAC address from RAL0/RAH0 or fallback EEPROM
    pub fn read_mac(&mut self) -> [u8; 6] {
        unsafe {
            let ral = self.read32(REG_RAL0);
            let rah = self.read32(REG_RAH0);

            if ral != 0 || (rah & 0xFFFF) != 0 {
                self.mac_address = [
                    (ral & 0xFF) as u8,
                    ((ral >> 8) & 0xFF) as u8,
                    ((ral >> 16) & 0xFF) as u8,
                    ((ral >> 24) & 0xFF) as u8,
                    (rah & 0xFF) as u8,
                    ((rah >> 8) & 0xFF) as u8,
                ];
                return self.mac_address;
            }
        }

        let w0 = self.read_eeprom(0);
        let w1 = self.read_eeprom(1);
        let w2 = self.read_eeprom(2);

        self.mac_address = [
            (w0 & 0xFF) as u8,
            ((w0 >> 8) & 0xFF) as u8,
            (w1 & 0xFF) as u8,
            ((w1 >> 8) & 0xFF) as u8,
            (w2 & 0xFF) as u8,
            ((w2 >> 8) & 0xFF) as u8,
        ];

        self.mac_address
    }

    /// Initializes RX/TX DMA rings and contiguous 2048-byte DMA packet buffers
    pub fn init_rings(&mut self) -> Result<(), &'static str> {
        let hhdm = crate::boot::get_hhdm_offset();

        // 1. Allocate descriptor frames and contiguous 64KB DMA packet buffers
        let (rx_ring_phys, tx_ring_phys, rx_bufs_phys, tx_bufs_phys) = {
            let mut alloc_guard = crate::mm::GLOBAL_FRAME_ALLOCATOR.lock();
            let alloc = alloc_guard.as_mut().ok_or("PMM offline")?;

            let rx_f = alloc.allocate_frame().ok_or("OOM RX ring frame")?;
            let tx_f = alloc.allocate_frame().ok_or("OOM TX ring frame")?;

            // 32 descriptors * 2048 bytes = 65,536 bytes = 16 contiguous 4KB pages
            let rx_bufs = alloc.allocate_contiguous(16).ok_or("OOM RX contiguous DMA buffers")?;
            let tx_bufs = alloc.allocate_contiguous(16).ok_or("OOM TX contiguous DMA buffers")?;

            (
                rx_f.start_address().as_u64(),
                tx_f.start_address().as_u64(),
                rx_bufs.start_address().as_u64(),
                tx_bufs.start_address().as_u64(),
            )
        };

        self.rx_ring_phys = rx_ring_phys;
        self.tx_ring_phys = tx_ring_phys;
        self.rx_bufs_phys = rx_bufs_phys;
        self.tx_bufs_phys = tx_bufs_phys;
        self.rx_cur = 0;
        self.tx_cur = 0;

        // Zero out descriptor memory
        unsafe {
            core::ptr::write_bytes((hhdm + rx_ring_phys) as *mut u8, 0, 4096);
            core::ptr::write_bytes((hhdm + tx_ring_phys) as *mut u8, 0, 4096);
        }

        // 2. Initialize 32 Receive Descriptors with contiguous physical addresses
        let rx_descs = (hhdm + rx_ring_phys) as *mut E1000RxDesc;
        for i in 0..NUM_RX_DESCRIPTORS {
            let buf_phys = rx_bufs_phys + (i * BUFFER_SIZE) as u64;
            unsafe {
                let desc = rx_descs.add(i);
                (*desc).buffer_addr = buf_phys;
                (*desc).status = 0;
            }
        }

        // 3. Initialize 32 Transmit Descriptors with contiguous physical addresses
        let tx_descs = (hhdm + tx_ring_phys) as *mut E1000TxDesc;
        for i in 0..NUM_TX_DESCRIPTORS {
            let buf_phys = tx_bufs_phys + (i * BUFFER_SIZE) as u64;
            unsafe {
                let desc = tx_descs.add(i);
                (*desc).buffer_addr = buf_phys;
                (*desc).status = 1; // Mark initial descriptors as available (DD bit = 1)
                (*desc).cmd = 0;
            }
        }

        // 4. Force Link Up in CTRL Register & clear Multicast Filter Table
        unsafe {
            let ctrl = self.read32(REG_CTRL);
            self.write32(REG_CTRL, ctrl | CTRL_SLU);

            for i in 0..128 {
                self.write32(REG_MTA + i * 4, 0);
            }
            // Clear and disable interrupts (polled bare-metal smoltcp mode)
            self.write32(REG_IMC, 0xFFFF_FFFF);
            let _ = self.read32(REG_ICR);
        }

        // 5. Program RX Ring Registers
        unsafe {
            self.write32(REG_RDBAL, rx_ring_phys as u32);
            self.write32(REG_RDBAH, (rx_ring_phys >> 32) as u32);
            self.write32(REG_RDLEN, (NUM_RX_DESCRIPTORS * core::mem::size_of::<E1000RxDesc>()) as u32);
            self.write32(REG_RDH, 0);
            self.write32(REG_RDT, (NUM_RX_DESCRIPTORS - 1) as u32);

            // Enable Receiver
            self.write32(REG_RCTL, RCTL_EN | RCTL_BAM | RCTL_BSIZE_2048 | RCTL_SECRC);
        }

        // 6. Program TX Ring Registers
        unsafe {
            self.write32(REG_TDBAL, tx_ring_phys as u32);
            self.write32(REG_TDBAH, (tx_ring_phys >> 32) as u32);
            self.write32(REG_TDLEN, (NUM_TX_DESCRIPTORS * core::mem::size_of::<E1000TxDesc>()) as u32);
            self.write32(REG_TDH, 0);
            self.write32(REG_TDT, 0);

            // Enable Transmitter
            self.write32(REG_TCTL, TCTL_EN | TCTL_PSP | TCTL_CT | TCTL_COLD);
        }

        Ok(())
    }
}

/// Discovers, boots and initializes the Intel 82540EM E1000 controller from PCI bus
pub fn probe_and_init_e1000() -> Result<(), &'static str> {
    for bus in 0..8 {
        for dev in 0..32 {
            for func in 0..8 {
                let pci = PciAddress::new(bus, dev, func);
                let pci_cfg = pci.config_address(0);
                unsafe {
                    outl(PCI_CONFIG_ADDRESS, pci_cfg);
                    let id_dw = inl(PCI_CONFIG_DATA);
                    let vendor_id = (id_dw & 0xFFFF) as u16;
                    let device_id = ((id_dw >> 16) & 0xFFFF) as u16;

                    if vendor_id == E1000_VENDOR_ID && device_id == E1000_DEVICE_ID {
                        crate::serial_println!(
                            "[PCI] Found Network Controller: Intel Corporation 82540EM Gigabit Ethernet (8086:100E) at PCI {:02X}:{:02X}.{:X}",
                            bus, dev, func
                        );

                        // Enable Bus Mastering (bit 2) and Memory Space (bit 1)
                        let cmd_cfg = pci.config_address(0x04);
                        outl(PCI_CONFIG_ADDRESS, cmd_cfg);
                        let cmd_dw = inl(PCI_CONFIG_DATA);
                        outl(PCI_CONFIG_DATA, cmd_dw | 0x07);

                        // Read BAR0 (MMIO Base Address)
                        let bar0_cfg = pci.config_address(0x10);
                        outl(PCI_CONFIG_ADDRESS, bar0_cfg);
                        let bar0_raw = inl(PCI_CONFIG_DATA);
                        let bar0_phys = (bar0_raw & 0xFFFF_FFF0) as u64;

                        // Map MMIO space (128 KiB = 32 pages)
                        let mmio_virt = match crate::mm::map_kernel_mmio(bar0_phys, 32) {
                            Ok(v) => v,
                            Err(_) => {
                                let hhdm = crate::boot::get_hhdm_offset();
                                hhdm + bar0_phys
                            }
                        };

                        let mut device = E1000Device {
                            pci_addr: pci,
                            mmio_base: mmio_virt,
                            mac_address: [0; 6],
                            rx_ring_phys: 0,
                            tx_ring_phys: 0,
                            rx_bufs_phys: 0,
                            tx_bufs_phys: 0,
                            rx_cur: 0,
                            tx_cur: 0,
                            stats_rx_packets: 0,
                            stats_rx_bytes: 0,
                            stats_tx_packets: 0,
                            stats_tx_bytes: 0,
                        };

                        let mac = device.read_mac();
                        crate::serial_println!(
                            "[E1000] Hardware MAC Address: {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
                            mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
                        );

                        device.init_rings()?;
                        crate::serial_println!(
                            "[E1000] RX/TX DMA Descriptor Rings online: RX=0x{:08X}, TX=0x{:08X} ({} contiguous 2KB buffers).",
                            device.rx_ring_phys, device.tx_ring_phys, NUM_RX_DESCRIPTORS
                        );

                        *GLOBAL_E1000.lock() = Some(device);
                        return Ok(());
                    }
                }
            }
        }
    }

    Err("Intel 82540EM (E1000) not found on PCI bus")
}
