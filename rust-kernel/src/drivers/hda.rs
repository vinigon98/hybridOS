//! HybridOS Bare-Metal Intel High Definition Audio (HDA) Controller Driver.
//!
//! PCI Vendor: 0x8086 (Intel Corporation), Device: 0x2668 (ICH6 / QEMU intel-hda).
//! Target: QEMU `-device intel-hda,id=sound0 -device hda-duplex,id=sound0-codec0`.
//!
//! Implements MMIO mapping, Controller Reset, Codec 0 initialization,
//! Output Stream DMA with Cyclic Buffer Descriptor List (BDL), and PCM tone/chime synthesis.

use crate::arch::msr::{inb, inl, outb, outl};
use crate::drivers::hardware::{PCI_CONFIG_ADDRESS, PCI_CONFIG_DATA, PciAddress};
use crate::sync::SpinLock;

pub const HDA_VENDOR_ID: u16 = 0x8086;
pub const HDA_DEVICE_ID: u16 = 0x2668;

// HDA Global Registers (MMIO offsets)
pub const REG_GCAP: usize     = 0x00; // Global Capabilities (16-bit)
pub const REG_VMIN: usize     = 0x02; // Minor Version
pub const REG_VMAJ: usize     = 0x03; // Major Version
pub const REG_GCTL: usize     = 0x08; // Global Control (32-bit)
pub const REG_WAKEEN: usize   = 0x0C; // Wake Enable (16-bit)
pub const REG_STATESTS: usize = 0x0E; // State Change Status (16-bit)
pub const REG_GSTS: usize     = 0x10; // Global Status (16-bit)
pub const REG_INTCTL: usize   = 0x20; // Interrupt Control (32-bit)
pub const REG_INTSTS: usize   = 0x24; // Interrupt Status (32-bit)

// Immediate Command Registers
pub const REG_ICO: usize      = 0x60; // Immediate Command Output (32-bit)
pub const REG_IRI: usize      = 0x64; // Immediate Response Input (32-bit)
pub const REG_ICS: usize      = 0x68; // Immediate Command Status (16-bit)

// Stream Descriptor Register Offsets (relative to SD base)
pub const SD_CTL0: usize  = 0x00; // Stream Control 0 (8-bit)
pub const SD_CTL2: usize  = 0x02; // Stream Control 2 / Tag (8-bit)
pub const SD_STS: usize   = 0x03; // Stream Status (8-bit)
pub const SD_LPIB: usize  = 0x04; // Link Position in Buffer (32-bit)
pub const SD_CBL: usize   = 0x08; // Cyclic Buffer Length (32-bit)
pub const SD_LVI: usize   = 0x0C; // Last Valid Index (16-bit)
pub const SD_FIFOS: usize = 0x10; // FIFO Size (16-bit)
pub const SD_FMT: usize   = 0x12; // Stream Format (16-bit)
pub const SD_BDPL: usize  = 0x18; // BDL Lower Address (32-bit)
pub const SD_BDPU: usize  = 0x1C; // BDL Upper Address (32-bit)

/// Buffer Descriptor List (BDL) Entry (16 bytes, hardware specified)
#[repr(C, packed)]
#[derive(Clone, Copy, Default)]
pub struct HdaBdlEntry {
    pub addr_low: u32,
    pub addr_high: u32,
    pub length: u32,
    pub flags: u32, // Bit 0: IOC (Interrupt on Completion)
}

pub struct HdaController {
    pub pci_addr: PciAddress,
    pub mmio_base: u64,
    pub iss: usize, // Input Stream Count
    pub oss: usize, // Output Stream Count
    pub bdl_phys: u64,
    pub dma_buffer_phys: [u64; 2],
    pub out_stream_base: usize,
}

pub static HDA_CONTROLLER: SpinLock<Option<HdaController>> = SpinLock::new(None);

impl HdaController {
    #[inline(always)]
    unsafe fn write8(&self, offset: usize, val: u8) {
        let ptr = (self.mmio_base + offset as u64) as *mut u8;
        core::ptr::write_volatile(ptr, val);
    }

    #[inline(always)]
    unsafe fn read8(&self, offset: usize) -> u8 {
        let ptr = (self.mmio_base + offset as u64) as *const u8;
        core::ptr::read_volatile(ptr)
    }

    #[inline(always)]
    unsafe fn write16(&self, offset: usize, val: u16) {
        let ptr = (self.mmio_base + offset as u64) as *mut u16;
        core::ptr::write_volatile(ptr, val);
    }

    #[inline(always)]
    unsafe fn read16(&self, offset: usize) -> u16 {
        let ptr = (self.mmio_base + offset as u64) as *const u16;
        core::ptr::read_volatile(ptr)
    }

    #[inline(always)]
    unsafe fn write32(&self, offset: usize, val: u32) {
        let ptr = (self.mmio_base + offset as u64) as *mut u32;
        core::ptr::write_volatile(ptr, val);
    }

    #[inline(always)]
    unsafe fn read32(&self, offset: usize) -> u32 {
        let ptr = (self.mmio_base + offset as u64) as *const u32;
        core::ptr::read_volatile(ptr)
    }

    /// Sends a 32-bit Codec Verb via the Immediate Command mechanism and retrieves the response.
    pub fn send_verb(&self, codec: u8, nid: u8, verb_id: u32, payload: u32) -> Result<u32, &'static str> {
        let cmd = ((codec as u32 & 0x0F) << 28)
            | ((nid as u32 & 0xFF) << 20)
            | ((verb_id & 0x0FFF) << 8)
            | (payload & 0xFF);

        unsafe {
            // Wait while Immediate Command is Busy (Bit 0 of ICS)
            for _ in 0..100_000 {
                if (self.read16(REG_ICS) & 0x01) == 0 {
                    break;
                }
            }

            // Clear Immediate Result Valid flag (Bit 1)
            self.write16(REG_ICS, self.read16(REG_ICS) | 0x02);

            // Write command word
            self.write32(REG_ICO, cmd);

            // Trigger command execution (Set Bit 0: ICB)
            self.write16(REG_ICS, self.read16(REG_ICS) | 0x01);

            // Poll for completion: ICB == 0 and IRV == 1
            for _ in 0..200_000 {
                let status = self.read16(REG_ICS);
                if (status & 0x01) == 0 && (status & 0x02) != 0 {
                    return Ok(self.read32(REG_IRI));
                }
            }
        }
        Err("HDA Immediate Command Timeout")
    }

    /// Configures the Codec output converter and pins (QEMU hda-duplex compatible).
    pub fn init_codec_nodes(&self) -> Result<(), &'static str> {
        // AFG (Node 0x01): Power State D0 (Verb 0x705, Param 0x00)
        self.send_verb(0, 1, 0x705, 0x00)?;

        // DAC (Node 0x02):
        // 1. Power State D0
        self.send_verb(0, 2, 0x705, 0x00)?;
        // 2. Set Stream Tag 1, Channel 0 (Verb 0x706, Param 0x10)
        self.send_verb(0, 2, 0x706, 0x10)?;
        // 3. Set Format 48kHz, 16-bit, 2 channels (Verb 0x2, Payload 0x0011)
        let fmt_cmd = 0x20011;
        unsafe {
            self.write32(REG_ICO, ((0 & 0x0F) << 28) | ((2 & 0xFF) << 20) | fmt_cmd);
            self.write16(REG_ICS, self.read16(REG_ICS) | 0x01);
            for _ in 0..100_000 {
                if (self.read16(REG_ICS) & 0x01) == 0 { break; }
            }
        }
        // 4. Set Amplifier Gain Unmuted, 100% (Verb 0x3, Param 0xB07F)
        unsafe {
            self.write32(REG_ICO, ((0 & 0x0F) << 28) | ((2 & 0xFF) << 20) | 0x3B07F);
            self.write16(REG_ICS, self.read16(REG_ICS) | 0x01);
            for _ in 0..100_000 {
                if (self.read16(REG_ICS) & 0x01) == 0 { break; }
            }
        }

        // Pin Complex (Node 0x04):
        // 1. Power State D0
        self.send_verb(0, 4, 0x705, 0x00)?;
        // 2. Set Pin Control: Audio Out Enabled (Verb 0x707, Param 0x40)
        self.send_verb(0, 4, 0x707, 0x40)?;
        // 3. Connect Pin to DAC Node 0x02 (Verb 0x701, Param 0x00)
        self.send_verb(0, 4, 0x701, 0x00)?;
        // 4. Set Output Amp Gain Unmuted, 100%
        unsafe {
            self.write32(REG_ICO, ((0 & 0x0F) << 28) | ((4 & 0xFF) << 20) | 0x3B07F);
            self.write16(REG_ICS, self.read16(REG_ICS) | 0x01);
            for _ in 0..100_000 {
                if (self.read16(REG_ICS) & 0x01) == 0 { break; }
            }
        }

        Ok(())
    }

    /// Sets up the cyclic DMA Output Stream 0 with Buffer Descriptor List (BDL).
    pub fn setup_output_stream(&mut self) -> Result<(), &'static str> {
        let hhdm = crate::boot::get_hhdm_offset();
        let (bdl_phys, buf0_phys, buf1_phys) = {
            let mut alloc_guard = crate::mm::GLOBAL_FRAME_ALLOCATOR.lock();
            let alloc = alloc_guard.as_mut().ok_or("PMM offline")?;
            let bdl_f = alloc.allocate_frame().ok_or("OOM BDL frame")?;
            let b0_f = alloc.allocate_frame().ok_or("OOM Audio buffer 0")?;
            let b1_f = alloc.allocate_frame().ok_or("OOM Audio buffer 1")?;
            (bdl_f.start_address().as_u64(), b0_f.start_address().as_u64(), b1_f.start_address().as_u64())
        };

        self.bdl_phys = bdl_phys;
        self.dma_buffer_phys = [buf0_phys, buf1_phys];

        // Prepare BDL Table (2 entries of 4096 bytes each)
        let bdl_virt = (hhdm + bdl_phys) as *mut HdaBdlEntry;
        unsafe {
            core::ptr::write_bytes(bdl_virt as *mut u8, 0, 4096);
            core::ptr::write_bytes((hhdm + buf0_phys) as *mut u8, 0, 4096);
            core::ptr::write_bytes((hhdm + buf1_phys) as *mut u8, 0, 4096);

            *bdl_virt.add(0) = HdaBdlEntry {
                addr_low: buf0_phys as u32,
                addr_high: (buf0_phys >> 32) as u32,
                length: 4096,
                flags: 1, // Interrupt on Completion
            };
            *bdl_virt.add(1) = HdaBdlEntry {
                addr_low: buf1_phys as u32,
                addr_high: (buf1_phys >> 32) as u32,
                length: 4096,
                flags: 1,
            };
        }

        let s = self.out_stream_base;
        unsafe {
            // 1. Reset Stream Descriptor
            self.write8(s + SD_CTL0, 0x01); // SRST = 1
            for _ in 0..50_000 {
                if (self.read8(s + SD_CTL0) & 0x01) != 0 { break; }
            }
            self.write8(s + SD_CTL0, 0x00); // SRST = 0
            for _ in 0..50_000 {
                if (self.read8(s + SD_CTL0) & 0x01) == 0 { break; }
            }

            // 2. Program Stream Tag = 1 in SD_CTL2
            self.write8(s + SD_CTL2, 1 << 4);

            // 3. Program Cyclic Buffer Length = 8192 bytes
            self.write32(s + SD_CBL, 8192);

            // 4. Last Valid Index = 1 (2 entries - 1)
            self.write16(s + SD_LVI, 1);

            // 5. Audio Format: 48 kHz, 16-bit, 2-channel Stereo (0x0011)
            self.write16(s + SD_FMT, 0x0011);

            // 6. Buffer Descriptor List Pointer
            self.write32(s + SD_BDPL, bdl_phys as u32);
            self.write32(s + SD_BDPU, (bdl_phys >> 32) as u32);
        }

        Ok(())
    }

    /// Plays a continuous synthesized audio tone at `freq_hz` for `duration_ms`.
    pub fn play_tone(&self, freq_hz: u32, duration_ms: u32) {
        let hhdm = crate::boot::get_hhdm_offset();
        let s = self.out_stream_base;
        let sample_rate = 48000;

        // Generate 16-bit stereo PCM samples in Buffer 0 and Buffer 1
        for buf_idx in 0..2 {
            let buf_ptr = (hhdm + self.dma_buffer_phys[buf_idx]) as *mut i16;
            let samples_in_buf = 4096 / 4; // 1024 stereo frames (L + R)

            for i in 0..samples_in_buf {
                let phase = (i * freq_hz as usize * 1024 / sample_rate) % 1024;
                let val: i16 = if phase < 512 {
                    ((phase as i32 * 20000 / 512) - 10000) as i16
                } else {
                    (((1024 - phase as i32) * 20000 / 512) - 10000) as i16
                };
                unsafe {
                    *buf_ptr.add(i * 2) = val;     // Left
                    *buf_ptr.add(i * 2 + 1) = val; // Right
                }
            }
        }

        // Start Stream DMA (Bit 1: RUN)
        unsafe {
            self.write8(s + SD_CTL0, self.read8(s + SD_CTL0) | 0x02);
        }

        // Timing loop (approximate duration)
        for _ in 0..duration_ms {
            for _ in 0..50_000 {
                core::hint::spin_loop();
            }
        }

        // Stop Stream DMA
        unsafe {
            self.write8(s + SD_CTL0, self.read8(s + SD_CTL0) & !0x02);
        }
    }

    /// Plays the iconic HybridOS Bare-Metal Startup Harmonized Chime.
    pub fn play_boot_chime(&self) {
        // Pentatonic C-major chord progression: C5 (523Hz), E5 (659Hz), G5 (784Hz), C6 (1046Hz)
        self.play_tone(523, 70);
        self.play_tone(659, 70);
        self.play_tone(784, 90);
        self.play_tone(1046, 180);
    }
}

/// Probes the PCI bus for Intel HDA (8086:2668), maps MMIO, resets controller, and boots audio engine.
pub fn probe_and_init_hda() -> Result<(), &'static str> {
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

                    if vendor_id == HDA_VENDOR_ID && device_id == HDA_DEVICE_ID {
                        crate::serial_println!(
                            "[PCI] Found Intel High Definition Audio (8086:2668) at PCI {:02X}:{:02X}.{:X}",
                            bus, dev, func
                        );

                        // Enable Bus Master (bit 2) and Memory Space (bit 1)
                        let cmd_cfg = pci.config_address(0x04);
                        outl(PCI_CONFIG_ADDRESS, cmd_cfg);
                        let cmd_dw = inl(PCI_CONFIG_DATA);
                        outl(PCI_CONFIG_DATA, cmd_dw | 0x06);

                        // Read BAR0 (MMIO Base)
                        let bar0_cfg = pci.config_address(0x10);
                        outl(PCI_CONFIG_ADDRESS, bar0_cfg);
                        let bar0_raw = inl(PCI_CONFIG_DATA);
                        let bar0_phys = (bar0_raw & 0xFFFF_FFF0) as u64;

                        // Map 16 KiB MMIO (4 pages)
                        let mmio_virt = match crate::mm::map_kernel_mmio(bar0_phys, 4) {
                            Ok(v) => v,
                            Err(_) => crate::boot::get_hhdm_offset() + bar0_phys,
                        };

                        crate::serial_println!("[HDA] MMIO BAR0 Physical: 0x{:08X}, Virtual: 0x{:016X}", bar0_phys, mmio_virt);

                        let mut controller = HdaController {
                            pci_addr: pci,
                            mmio_base: mmio_virt,
                            iss: 4,
                            oss: 4,
                            bdl_phys: 0,
                            dma_buffer_phys: [0; 2],
                            out_stream_base: 0,
                        };

                        // Controller Hardware Reset
                        controller.write32(REG_GCTL, 0); // Assert Reset
                        for _ in 0..50_000 {
                            if (controller.read32(REG_GCTL) & 0x01) == 0 { break; }
                        }
                        for _ in 0..10_000 { core::hint::spin_loop(); }
                        controller.write32(REG_GCTL, 1); // Deassert Reset
                        for _ in 0..50_000 {
                            if (controller.read32(REG_GCTL) & 0x01) != 0 { break; }
                        }
                        for _ in 0..100_000 { core::hint::spin_loop(); }

                        // Check Codec Presence (STATESTS Bit 0)
                        let statests = controller.read16(REG_STATESTS);
                        if (statests & 0x01) == 0 {
                            crate::serial_println!("[HDA] Warning: Codec 0 not signaling link ready, proceeding with init...");
                        }

                        // Read Capabilities (GCAP)
                        let gcap = controller.read16(REG_GCAP);
                        let iss = ((gcap >> 8) & 0x0F) as usize;
                        let oss = ((gcap >> 12) & 0x0F) as usize;
                        controller.iss = iss;
                        controller.oss = oss;
                        controller.out_stream_base = 0x80 + iss * 0x20;

                        crate::serial_println!("[HDA] Capabilities: ISS={}, OSS={}, Output Stream 0 Base Offset=0x{:02X}", iss, oss, controller.out_stream_base);

                        // Initialize Codec & DMA Stream
                        controller.init_codec_nodes()?;
                        controller.setup_output_stream()?;

                        crate::serial_println!("[HDA] Controller & Output Stream Descriptor online. Playing boot chime...");
                        controller.play_boot_chime();

                        *HDA_CONTROLLER.lock() = Some(controller);
                        return Ok(());
                    }
                }
            }
        }
    }
    Err("Intel HDA Controller (8086:2668) not found on PCI bus")
}
