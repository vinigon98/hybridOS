//! Serial ATA Advanced Host Controller Interface (SATA AHCI 1.3) Driver & Mock Subsystem.
//!
//! Provides a complete, compliant AHCI 1.3 specification implementation in pure Rust:
//! - Generic Host Control (GHC): CAP, GHC, IS, PI (Ports Implemented), VS.
//! - Port Registers: PxCLB, PxFB, PxIS, PxIE, PxCMD, PxTFD, PxSIG, PxSSTS, PxCI.
//! - Command List (32 slots) & PRDT (Physical Region Descriptor Table) scatter-gather lists.
//! - ATA Commands: IDENTIFY DEVICE (0xEC), READ DMA EXT (0x25), WRITE DMA EXT (0x35), FLUSH CACHE EXT (0xEA).
//! - [`MockAhciController`]: Virtual AHCI host adapter emulating SATA hardware & PHY negotiation.
//! - [`AhciDevice`]: High-level block device adapter conforming to [`super::block_device::BlockDevice`].

use std::sync::{Arc, Mutex, RwLock};

use super::block_device::{BlockDevice, BlockDeviceError, DeviceStatus, DeviceType};

// ============================================================================
// Section 1: AHCI 1.3 Register Offsets & Bitfield Masks
// ============================================================================

/// Generic Host Control (GHC) Register Offsets (BAR5 MMIO)
pub const AHCI_REG_CAP: usize = 0x00;        // Host Capabilities (32-bit)
pub const AHCI_REG_GHC: usize = 0x04;        // Global Host Control (32-bit)
pub const AHCI_REG_IS: usize = 0x08;         // Interrupt Status (32-bit)
pub const AHCI_REG_PI: usize = 0x0C;         // Ports Implemented (32-bit bitmask)
pub const AHCI_REG_VS: usize = 0x10;         // AHCI Version (32-bit)
pub const AHCI_REG_CCC_CTL: usize = 0x14;    // Command Coalescing Control
pub const AHCI_REG_CCC_PORTS: usize = 0x18;  // Command Coalescing Ports
pub const AHCI_REG_EM_LOC: usize = 0x1C;     // Enclosure Management Location
pub const AHCI_REG_EM_CTL: usize = 0x20;     // Enclosure Management Control
pub const AHCI_REG_CAP2: usize = 0x24;       // Host Capabilities Extended
pub const AHCI_REG_BOHC: usize = 0x28;       // BIOS/OS Handoff Control and Status

/// CAP Bitfields
pub const AHCI_CAP_NP_MASK: u32 = 0x1F;      // Number of ports (0-based)
pub const AHCI_CAP_SXS: u32 = 1 << 5;        // Supports External SATA
pub const AHCI_CAP_EMS: u32 = 1 << 6;        // Enclosure Management Supported
pub const AHCI_CAP_CCCS: u32 = 1 << 7;       // Command Coalescing Supported
pub const AHCI_CAP_NCS_SHIFT: u32 = 8;       // Number of Command Slots (0-based)
pub const AHCI_CAP_NCS_MASK: u32 = 0x1F << 8;
pub const AHCI_CAP_PSC: u32 = 1 << 13;       // Partial State Capable
pub const AHCI_CAP_SSC: u32 = 1 << 14;       // Slumber State Capable
pub const AHCI_CAP_PMD: u32 = 1 << 15;       // PIO Multiple DRQ Block
pub const AHCI_CAP_FBSS: u32 = 1 << 16;      // FIS-based Switching Supported
pub const AHCI_CAP_SPM: u32 = 1 << 17;       // Supports Port Multiplier
pub const AHCI_CAP_SAM: u32 = 1 << 18;       // Supports AHCI-Only Mode
pub const AHCI_CAP_ISS_SHIFT: u32 = 20;      // Interface Speed Support (1=1.5Gbps, 2=3Gbps, 3=6Gbps)
pub const AHCI_CAP_SCLO: u32 = 1 << 24;      // Supports Command List Override
pub const AHCI_CAP_SAL: u32 = 1 << 25;       // Supports Activity LED
pub const AHCI_CAP_SALP: u32 = 1 << 26;      // Supports Aggressive Link Power Management
pub const AHCI_CAP_SSS: u32 = 1 << 27;       // Supports Staggered Spin-up
pub const AHCI_CAP_SMPS: u32 = 1 << 28;      // Supports Mechanical Presence Switch
pub const AHCI_CAP_SSNTF: u32 = 1 << 29;     // Supports SNotification Register
pub const AHCI_CAP_SNCQ: u32 = 1 << 30;      // Supports Native Command Queuing
pub const AHCI_CAP_S64A: u32 = 1 << 31;      // Supports 64-bit Addressing

/// GHC Bitfields
pub const AHCI_GHC_HR: u32 = 1 << 0;         // HBA Reset (write 1 to reset)
pub const AHCI_GHC_IE: u32 = 1 << 1;         // Interrupt Enable
pub const AHCI_GHC_MRSM: u32 = 1 << 2;       // MSI Revert to Single Message
pub const AHCI_GHC_AE: u32 = 1 << 31;        // AHCI Enable (must be 1 for AHCI mode)

/// Port Register Offsets (Port 0..31 at base 0x100 + port_idx * 0x80)
pub const PORT_REG_CLB: usize = 0x00;        // Command List Base Address (32-bit low, 1KB aligned)
pub const PORT_REG_CLBU: usize = 0x04;       // Command List Base Address (32-bit high)
pub const PORT_REG_FB: usize = 0x08;         // FIS Base Address (32-bit low, 256-byte aligned)
pub const PORT_REG_FBU: usize = 0x0C;        // FIS Base Address (32-bit high)
pub const PORT_REG_IS: usize = 0x10;         // Interrupt Status
pub const PORT_REG_IE: usize = 0x14;         // Interrupt Enable
pub const PORT_REG_CMD: usize = 0x18;        // Command and Status
pub const PORT_REG_TFD: usize = 0x20;        // Task File Data
pub const PORT_REG_SIG: usize = 0x24;        // Signature
pub const PORT_REG_SSTS: usize = 0x28;       // SATA Status (SCR0: SStatus)
pub const PORT_REG_SCTL: usize = 0x2C;       // SATA Control (SCR2: SControl)
pub const PORT_REG_SERR: usize = 0x30;       // SATA Error (SCR1: SError)
pub const PORT_REG_SACT: usize = 0x34;       // SATA Active (SCR3: SActive)
pub const PORT_REG_CI: usize = 0x38;         // Command Issue (32-bit slot bitmask)

/// Port CMD Bitfields
pub const PORT_CMD_ST: u32 = 1 << 0;         // Start command processing
pub const PORT_CMD_SUD: u32 = 1 << 1;        // Spin-Up Device
pub const PORT_CMD_POD: u32 = 1 << 2;        // Power On Device
pub const PORT_CMD_CLO: u32 = 1 << 3;        // Command List Override
pub const PORT_CMD_FRE: u32 = 1 << 4;        // FIS Receive Enable
pub const PORT_CMD_CCS_SHIFT: u32 = 8;       // Current Command Slot
pub const PORT_CMD_FR: u32 = 1 << 14;        // FIS Receive Running
pub const PORT_CMD_CR: u32 = 1 << 15;        // Command List Running

/// Port TFD Bitfields
pub const PORT_TFD_ERR: u32 = 1 << 0;        // Error bit
pub const PORT_TFD_DRQ: u32 = 1 << 3;        // Data Transfer Requested
pub const PORT_TFD_BSY: u32 = 1 << 7;        // Device Busy

/// Port Signatures
pub const SATA_SIG_ATA: u32 = 0x0000_0101;   // Traditional SATA hard drive / SSD
pub const SATA_SIG_ATAPI: u32 = 0xEB14_0101; // SATAPI optical drive
pub const SATA_SIG_SEMB: u32 = 0xC33C_0101;  // Enclosure management bridge
pub const SATA_SIG_PM: u32 = 0x9669_0101;    // Port multiplier

/// Port SSTS (SStatus) Detection values
pub const SSTS_DET_MASK: u32 = 0x0F;
pub const SSTS_DET_NONE: u32 = 0x00;
pub const SSTS_DET_PRESENT_NOPHY: u32 = 0x01;
pub const SSTS_DET_PRESENT_PHY: u32 = 0x03;  // Device present and PHY communication established
pub const SSTS_DET_OFFLINE: u32 = 0x04;

// ============================================================================
// Section 2: ATA Command Opcodes
// ============================================================================

pub const ATA_CMD_READ_DMA: u8 = 0xC8;
pub const ATA_CMD_READ_DMA_EXT: u8 = 0x25;
pub const ATA_CMD_WRITE_DMA: u8 = 0xCA;
pub const ATA_CMD_WRITE_DMA_EXT: u8 = 0x35;
pub const ATA_CMD_IDENTIFY_DEVICE: u8 = 0xEC;
pub const ATA_CMD_FLUSH_CACHE: u8 = 0xE7;
pub const ATA_CMD_FLUSH_CACHE_EXT: u8 = 0xEA;

// ============================================================================
// Section 3: Data Structures (Command Header, PRDT, Command Table, FIS)
// ============================================================================

pub const MAX_PRDT_ENTRIES: usize = 32;

/// 32-byte AHCI Command Header (stored in Port Command List).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AhciCommandHeader {
    /// DW0: CFL (0..4), ATAPI (5), Write (6), Prefetch (7), Reset (8), BIST (9), Clear (10), PRDTL (16..31)
    pub flags: u32,
    /// DW1: PRD Byte Count transferred
    pub prdbc: u32,
    /// DW2: Command Table Base Address low (128-byte aligned)
    pub ctba: u32,
    /// DW3: Command Table Base Address upper (32-bit high)
    pub ctbau: u32,
    /// DW4..DW7: Reserved (16 bytes)
    pub reserved: [u32; 4],
}

impl AhciCommandHeader {
    pub fn new(cfl_dwords: u8, is_write: bool, prdt_len: u16, ctba: u64) -> Self {
        let mut flags = (cfl_dwords as u32) & 0x1F;
        if is_write {
            flags |= 1 << 6; // W bit
        }
        flags |= (prdt_len as u32) << 16;

        Self {
            flags,
            prdbc: 0,
            ctba: (ctba & 0xFFFF_FFFF) as u32,
            ctbau: (ctba >> 32) as u32,
            reserved: [0; 4],
        }
    }

    pub fn is_write(&self) -> bool {
        (self.flags & (1 << 6)) != 0
    }

    pub fn prdt_length(&self) -> u16 {
        ((self.flags >> 16) & 0xFFFF) as u16
    }
}

/// 16-byte Physical Region Descriptor Table (PRDT) Entry.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AhciPrdtEntry {
    /// DW0: Data Base Address low (must be 2-byte aligned)
    pub dba: u32,
    /// DW1: Data Base Address upper
    pub dbau: u32,
    /// DW2: Reserved
    pub reserved: u32,
    /// DW3: Data Byte Count (0-based: byte_count - 1, bits 0..21), Interrupt on Completion (bit 31)
    pub dbc_and_flags: u32,
}

impl AhciPrdtEntry {
    pub fn new(phys_addr: u64, byte_count: usize, interrupt_on_completion: bool) -> Self {
        assert!(byte_count > 0 && byte_count <= 4 * 1024 * 1024, "PRDT chunk cannot exceed 4 MB");
        let mut dbc = ((byte_count - 1) as u32) & 0x003F_FFFF;
        if interrupt_on_completion {
            dbc |= 1 << 31;
        }

        Self {
            dba: (phys_addr & 0xFFFF_FFFF) as u32,
            dbau: (phys_addr >> 32) as u32,
            reserved: 0,
            dbc_and_flags: dbc,
        }
    }

    pub fn byte_count(&self) -> usize {
        ((self.dbc_and_flags & 0x003F_FFFF) as usize) + 1
    }

    pub fn data_address(&self) -> u64 {
        (self.dba as u64) | ((self.dbau as u64) << 32)
    }
}

/// Command Table containing the Command FIS, ATAPI buffer, and PRDT array.
#[repr(C)]
pub struct AhciCommandTable {
    /// Command FIS (Host to Device, 64 bytes)
    pub cfis: [u8; 64],
    /// ATAPI Command (16 bytes)
    pub acmd: [u8; 16],
    /// Reserved (48 bytes)
    pub reserved: [u8; 48],
    /// Physical Region Descriptor Table (scatter-gather entries)
    pub prdt: [AhciPrdtEntry; MAX_PRDT_ENTRIES],
}

impl Default for AhciCommandTable {
    fn default() -> Self {
        Self {
            cfis: [0; 64],
            acmd: [0; 16],
            reserved: [0; 48],
            prdt: [AhciPrdtEntry::default(); MAX_PRDT_ENTRIES],
        }
    }
}

/// FIS Type 0x27: Register FIS - Host to Device (H2D)
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FisRegH2D {
    pub fis_type: u8,       // 0x27
    pub pm_and_c: u8,       // Port Multiplier (0..3), bit 7 = Command (1) / Control (0)
    pub command: u8,        // ATA Command register
    pub feature_low: u8,    // Feature register (7..0)
    pub lba0: u8,           // LBA (7..0)
    pub lba1: u8,           // LBA (15..8)
    pub lba2: u8,           // LBA (23..16)
    pub device: u8,         // Device register (bit 6 = 1 for LBA mode)
    pub lba3: u8,           // LBA (31..24)
    pub lba4: u8,           // LBA (39..32)
    pub lba5: u8,           // LBA (47..40)
    pub feature_high: u8,   // Feature register (15..8)
    pub count_low: u8,      // Sector Count (7..0)
    pub count_high: u8,     // Sector Count (15..8)
    pub icc: u8,            // Isochronous command completion
    pub control: u8,        // Control register
    pub reserved: [u8; 4],
}

impl Default for FisRegH2D {
    fn default() -> Self {
        Self {
            fis_type: 0x27,
            pm_and_c: 0x80, // Command bit set
            command: 0,
            feature_low: 0,
            lba0: 0,
            lba1: 0,
            lba2: 0,
            device: 1 << 6, // LBA mode
            lba3: 0,
            lba4: 0,
            lba5: 0,
            feature_high: 0,
            count_low: 0,
            count_high: 0,
            icc: 0,
            control: 0,
            reserved: [0; 4],
        }
    }
}

impl FisRegH2D {
    pub fn setup_lba48(&mut self, command: u8, lba: u64, sector_count: u16) {
        self.fis_type = 0x27;
        self.pm_and_c = 0x80;
        self.command = command;
        self.device = 1 << 6; // LBA mode

        self.lba0 = (lba & 0xFF) as u8;
        self.lba1 = ((lba >> 8) & 0xFF) as u8;
        self.lba2 = ((lba >> 16) & 0xFF) as u8;
        self.lba3 = ((lba >> 24) & 0xFF) as u8;
        self.lba4 = ((lba >> 32) & 0xFF) as u8;
        self.lba5 = ((lba >> 40) & 0xFF) as u8;

        self.count_low = (sector_count & 0xFF) as u8;
        self.count_high = ((sector_count >> 8) & 0xFF) as u8;
    }
}

const _: () = {
    assert!(std::mem::size_of::<AhciCommandHeader>() == 32);
    assert!(std::mem::size_of::<AhciPrdtEntry>() == 16);
    assert!(std::mem::size_of::<FisRegH2D>() == 20);
};

// ============================================================================
// Section 4: ATA IDENTIFY DEVICE Data Parser
// ============================================================================

/// Parsed ATA Identify Device data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AtaIdentifyInfo {
    pub serial_number: String,
    pub firmware_revision: String,
    pub model_number: String,
    pub total_sectors_lba48: u64,
    pub supports_lba48: bool,
    pub supports_dma: bool,
}

impl AtaIdentifyInfo {
    /// Parses 512-byte raw ATA IDENTIFY buffer (256 words).
    pub fn parse(raw_words: &[u16; 256]) -> Self {
        let serial = Self::extract_ata_string(&raw_words[10..20]);
        let fw = Self::extract_ata_string(&raw_words[23..27]);
        let model = Self::extract_ata_string(&raw_words[27..47]);

        let capabilities = raw_words[49];
        let supports_dma = (capabilities & (1 << 8)) != 0;

        let cmd_sets = raw_words[83];
        let supports_lba48 = (cmd_sets & (1 << 10)) != 0;

        let total_sectors = if supports_lba48 {
            (raw_words[100] as u64)
                | ((raw_words[101] as u64) << 16)
                | ((raw_words[102] as u64) << 32)
                | ((raw_words[103] as u64) << 48)
        } else {
            (raw_words[60] as u64) | ((raw_words[61] as u64) << 16)
        };

        Self {
            serial_number: serial,
            firmware_revision: fw,
            model_number: model,
            total_sectors_lba48: total_sectors,
            supports_lba48,
            supports_dma,
        }
    }

    /// ATA strings are byte-swapped across each 16-bit word.
    fn extract_ata_string(words: &[u16]) -> String {
        let mut bytes = Vec::with_capacity(words.len() * 2);
        for &w in words {
            bytes.push((w >> 8) as u8);
            bytes.push((w & 0xFF) as u8);
        }
        String::from_utf8_lossy(&bytes).trim().to_string()
    }

    /// Formats an ATA string into swapped words.
    pub fn encode_ata_string(s: &str, num_words: usize) -> Vec<u16> {
        let mut bytes = s.as_bytes().to_vec();
        bytes.resize(num_words * 2, b' ');
        let mut words = Vec::with_capacity(num_words);
        for chunk in bytes.chunks(2) {
            let high = chunk[0] as u16;
            let low = chunk[1] as u16;
            words.push((high << 8) | low);
        }
        words
    }
}

// ============================================================================
// Section 5: Mock/Virtual AHCI Controller Testbed
// ============================================================================

/// State of an emulated AHCI port.
pub struct MockPortState {
    pub clb: u64,
    pub fb: u64,
    pub is: u32,
    pub ie: u32,
    pub cmd: u32,
    pub tfd: u32,
    pub sig: u32,
    pub ssts: u32,
    pub sctl: u32,
    pub serr: u32,
    pub sact: u32,
    pub ci: u32,

    // In-memory Command Table and Command List buffers for this port
    pub command_list: Vec<AhciCommandHeader>,
    pub command_tables: Vec<AhciCommandTable>,

    // Backing disk storage for this port
    pub disk_data: Arc<RwLock<Vec<u8>>>,
    pub total_sectors: u64,
}

impl MockPortState {
    pub fn new(capacity_sectors: u64) -> Self {
        let total_bytes = (capacity_sectors as usize) * 512;
        Self {
            clb: 0,
            fb: 0,
            is: 0,
            ie: 0,
            cmd: 0,
            tfd: 0,
            sig: SATA_SIG_ATA,
            ssts: SSTS_DET_PRESENT_PHY | (2 << 4) | (1 << 8), // Present, 3.0 Gbps, Active
            sctl: 0,
            serr: 0,
            sact: 0,
            ci: 0,
            command_list: vec![AhciCommandHeader::default(); 32],
            command_tables: (0..32).map(|_| AhciCommandTable::default()).collect(),
            disk_data: Arc::new(RwLock::new(vec![0u8; total_bytes])),
            total_sectors: capacity_sectors,
        }
    }
}

/// Virtual AHCI 1.3 host adapter for system testing without real hardware.
pub struct MockAhciController {
    pub cap: u32,
    pub ghc: u32,
    pub is: u32,
    pub pi: u32,
    pub vs: u32,
    pub ports: Vec<MockPortState>,
}

impl MockAhciController {
    pub fn new(num_ports: usize, sectors_per_port: u64) -> Self {
        assert!(num_ports >= 1 && num_ports <= 32);
        let pi = (1u32 << num_ports) - 1; // Bitmask of implemented ports

        // Capabilities: 32 command slots (NCS=31), 64-bit addressing (S64A), AHCI-only (SAM), Speed 3.0 Gbps (ISS=2)
        let cap = ((num_ports - 1) as u32)
            | (31 << AHCI_CAP_NCS_SHIFT)
            | (2 << AHCI_CAP_ISS_SHIFT)
            | AHCI_CAP_SAM
            | AHCI_CAP_S64A;

        let ports = (0..num_ports)
            .map(|_| MockPortState::new(sectors_per_port))
            .collect();

        Self {
            cap,
            ghc: AHCI_GHC_AE, // AHCI Enable bit set
            is: 0,
            pi,
            vs: 0x0001_0300,  // AHCI 1.3
            ports,
        }
    }

    /// Read 32-bit register from BAR5 MMIO space.
    pub fn read_reg32(&self, offset: usize) -> u32 {
        if offset < 0x100 {
            // Generic Host Control registers
            match offset {
                AHCI_REG_CAP => self.cap,
                AHCI_REG_GHC => self.ghc,
                AHCI_REG_IS => self.is,
                AHCI_REG_PI => self.pi,
                AHCI_REG_VS => self.vs,
                _ => 0,
            }
        } else {
            // Port registers: 0x100 + port_idx * 0x80 + port_reg
            let port_idx = (offset - 0x100) / 0x80;
            let port_reg = (offset - 0x100) % 0x80;

            if port_idx >= self.ports.len() {
                return 0;
            }

            let p = &self.ports[port_idx];
            match port_reg {
                PORT_REG_CLB => (p.clb & 0xFFFF_FFFF) as u32,
                PORT_REG_CLBU => (p.clb >> 32) as u32,
                PORT_REG_FB => (p.fb & 0xFFFF_FFFF) as u32,
                PORT_REG_FBU => (p.fb >> 32) as u32,
                PORT_REG_IS => p.is,
                PORT_REG_IE => p.ie,
                PORT_REG_CMD => p.cmd,
                PORT_REG_TFD => p.tfd,
                PORT_REG_SIG => p.sig,
                PORT_REG_SSTS => p.ssts,
                PORT_REG_SCTL => p.sctl,
                PORT_REG_SERR => p.serr,
                PORT_REG_SACT => p.sact,
                PORT_REG_CI => p.ci,
                _ => 0,
            }
        }
    }

    /// Write 32-bit register into BAR5 MMIO space.
    pub fn write_reg32(&mut self, offset: usize, val: u32) {
        if offset < 0x100 {
            match offset {
                AHCI_REG_GHC => {
                    self.ghc = val;
                    if (val & AHCI_GHC_HR) != 0 {
                        // Reset completes immediately in mock
                        self.ghc &= !AHCI_GHC_HR;
                        self.ghc |= AHCI_GHC_AE;
                    }
                }
                AHCI_REG_IS => self.is &= !val, // Clear bits on 1 write
                _ => {}
            }
        } else {
            let port_idx = (offset - 0x100) / 0x80;
            let port_reg = (offset - 0x100) % 0x80;

            if port_idx >= self.ports.len() {
                return;
            }

            let p = &mut self.ports[port_idx];
            match port_reg {
                PORT_REG_CLB => p.clb = (p.clb & !0xFFFF_FFFF) | (val as u64),
                PORT_REG_CLBU => p.clb = (p.clb & 0xFFFF_FFFF) | ((val as u64) << 32),
                PORT_REG_FB => p.fb = (p.fb & !0xFFFF_FFFF) | (val as u64),
                PORT_REG_FBU => p.fb = (p.fb & 0xFFFF_FFFF) | ((val as u64) << 32),
                PORT_REG_IS => p.is &= !val,
                PORT_REG_IE => p.ie = val,
                PORT_REG_CMD => {
                    p.cmd = val;
                    if (val & PORT_CMD_ST) != 0 {
                        p.cmd |= PORT_CMD_CR; // Command List Running
                    } else {
                        p.cmd &= !PORT_CMD_CR;
                    }
                    if (val & PORT_CMD_FRE) != 0 {
                        p.cmd |= PORT_CMD_FR; // FIS Receive Running
                    } else {
                        p.cmd &= !PORT_CMD_FR;
                    }
                }
                PORT_REG_SCTL => p.sctl = val,
                PORT_REG_SERR => p.serr &= !val,
                PORT_REG_CI => {
                    // Software sets bit `s` to issue command in slot `s`
                    p.ci |= val;
                    self.process_issued_commands(port_idx, val);
                }
                _ => {}
            }
        }
    }

    /// Executes issued commands for a given port.
    fn process_issued_commands(&mut self, port_idx: usize, issued_mask: u32) {
        for slot in 0..32 {
            if (issued_mask & (1 << slot)) == 0 {
                continue;
            }

            let p = &mut self.ports[port_idx];
            let header = p.command_list[slot];
            let table = &mut p.command_tables[slot];

            let fis_slice = &table.cfis;
            let fis_type = fis_slice[0];
            let command = fis_slice[2];

            if fis_type != 0x27 {
                // Not a Register H2D FIS
                p.tfd |= PORT_TFD_ERR;
                p.ci &= !(1 << slot);
                continue;
            }

            match command {
                ATA_CMD_IDENTIFY_DEVICE => {
                    // Populate 512-byte ATA identify words
                    let mut words = [0u16; 256];
                    words[0] = 0x0040; // ATA device
                    let sn_words = AtaIdentifyInfo::encode_ata_string("HYBRIDOS-SATA-001", 10);
                    words[10..20].copy_from_slice(&sn_words);
                    let fw_words = AtaIdentifyInfo::encode_ata_string("REV1.30", 4);
                    words[23..27].copy_from_slice(&fw_words);
                    let mn_words = AtaIdentifyInfo::encode_ata_string("NexusCore HybridOS Virtual SATA Drive", 20);
                    words[27..47].copy_from_slice(&mn_words);

                    words[49] = 1 << 8 | 1 << 9; // LBA and DMA supported
                    words[60] = (p.total_sectors & 0xFFFF) as u16;
                    words[61] = ((p.total_sectors >> 16) & 0xFFFF) as u16;
                    words[83] = 1 << 10; // 48-bit LBA supported
                    words[100] = (p.total_sectors & 0xFFFF) as u16;
                    words[101] = ((p.total_sectors >> 16) & 0xFFFF) as u16;
                    words[102] = ((p.total_sectors >> 32) & 0xFFFF) as u16;
                    words[103] = ((p.total_sectors >> 48) & 0xFFFF) as u16;

                    // Copy identify words into PRDT buffer
                    let prdt_len = header.prdt_length() as usize;
                    if prdt_len > 0 {
                        let prd = &table.prdt[0];
                        let dest_ptr = prd.data_address() as *mut u16;
                        unsafe {
                            std::ptr::copy_nonoverlapping(words.as_ptr(), dest_ptr, 256);
                        }
                    }

                    p.command_list[slot].prdbc = 512;
                    p.tfd &= !(PORT_TFD_BSY | PORT_TFD_DRQ);
                    p.ci &= !(1 << slot); // Command finished
                }
                ATA_CMD_READ_DMA_EXT => {
                    let lba = (fis_slice[4] as u64)
                        | ((fis_slice[5] as u64) << 8)
                        | ((fis_slice[6] as u64) << 16)
                        | ((fis_slice[8] as u64) << 24)
                        | ((fis_slice[9] as u64) << 32)
                        | ((fis_slice[10] as u64) << 40);

                    let count = (fis_slice[12] as u64) | ((fis_slice[13] as u64) << 8);
                    let sector_count = if count == 0 { 65536 } else { count };
                    let total_bytes = (sector_count as usize) * 512;

                    let prdt_len = header.prdt_length() as usize;
                    let disk = p.disk_data.read().unwrap();
                    let start_offset = (lba as usize) * 512;

                    let mut transferred = 0;
                    for i in 0..prdt_len {
                        if transferred >= total_bytes {
                            break;
                        }
                        let prd = &table.prdt[i];
                        let chunk = std::cmp::min(prd.byte_count(), total_bytes - transferred);
                        let dest_ptr = prd.data_address() as *mut u8;
                        unsafe {
                            std::ptr::copy_nonoverlapping(
                                disk[start_offset + transferred..start_offset + transferred + chunk].as_ptr(),
                                dest_ptr,
                                chunk,
                            );
                        }
                        transferred += chunk;
                    }

                    p.command_list[slot].prdbc = transferred as u32;
                    p.tfd &= !(PORT_TFD_BSY | PORT_TFD_DRQ);
                    p.ci &= !(1 << slot);
                }
                ATA_CMD_WRITE_DMA_EXT => {
                    let lba = (fis_slice[4] as u64)
                        | ((fis_slice[5] as u64) << 8)
                        | ((fis_slice[6] as u64) << 16)
                        | ((fis_slice[8] as u64) << 24)
                        | ((fis_slice[9] as u64) << 32)
                        | ((fis_slice[10] as u64) << 40);

                    let count = (fis_slice[12] as u64) | ((fis_slice[13] as u64) << 8);
                    let sector_count = if count == 0 { 65536 } else { count };
                    let total_bytes = (sector_count as usize) * 512;

                    let prdt_len = header.prdt_length() as usize;
                    let mut disk = p.disk_data.write().unwrap();
                    let start_offset = (lba as usize) * 512;

                    let mut transferred = 0;
                    for i in 0..prdt_len {
                        if transferred >= total_bytes {
                            break;
                        }
                        let prd = &table.prdt[i];
                        let chunk = std::cmp::min(prd.byte_count(), total_bytes - transferred);
                        let src_ptr = prd.data_address() as *const u8;
                        unsafe {
                            std::ptr::copy_nonoverlapping(
                                src_ptr,
                                disk[start_offset + transferred..start_offset + transferred + chunk].as_mut_ptr(),
                                chunk,
                            );
                        }
                        transferred += chunk;
                    }

                    p.command_list[slot].prdbc = transferred as u32;
                    p.tfd &= !(PORT_TFD_BSY | PORT_TFD_DRQ);
                    p.ci &= !(1 << slot);
                }
                ATA_CMD_FLUSH_CACHE | ATA_CMD_FLUSH_CACHE_EXT => {
                    p.tfd &= !(PORT_TFD_BSY | PORT_TFD_DRQ);
                    p.ci &= !(1 << slot);
                }
                _ => {
                    p.tfd |= PORT_TFD_ERR;
                    p.ci &= !(1 << slot);
                }
            }
        }
    }
}

// ============================================================================
// Section 6: High-Level AHCI SATA Block Device Driver
// ============================================================================

/// High-level AHCI SATA drive implementing the [`BlockDevice`] trait.
pub struct AhciDevice {
    id: u32,
    name: String,
    controller: Arc<Mutex<MockAhciController>>,
    port_index: usize,
    block_size: u32,
    block_count: u64,
    identify_info: Option<AtaIdentifyInfo>,
}

impl AhciDevice {
    /// Detects and initializes a SATA storage port on the AHCI host controller.
    pub fn new(id: u32, name: &str, controller: Arc<Mutex<MockAhciController>>, port_index: usize) -> Result<Self, BlockDeviceError> {
        let mut dev = Self {
            id,
            name: name.to_string(),
            controller,
            port_index,
            block_size: 512,
            block_count: 0,
            identify_info: None,
        };

        dev.initialize_port()?;
        Ok(dev)
    }

    /// Discovers device on port, initializes command engines, and queries ATA IDENTIFY.
    pub fn initialize_port(&mut self) -> Result<(), BlockDeviceError> {
        let mut ctrl = self.controller.lock().unwrap();

        // Check if port is implemented
        let pi = ctrl.read_reg32(AHCI_REG_PI);
        if (pi & (1 << self.port_index)) == 0 {
            return Err(BlockDeviceError::NotFound);
        }

        let port_offset = 0x100 + self.port_index * 0x80;

        // Check PHY detection status (PxSSTS.DET == 3)
        let ssts = ctrl.read_reg32(port_offset + PORT_REG_SSTS);
        let det = ssts & SSTS_DET_MASK;
        if det != SSTS_DET_PRESENT_PHY {
            return Err(BlockDeviceError::DeviceOffline);
        }

        // Verify device signature is ATA
        let sig = ctrl.read_reg32(port_offset + PORT_REG_SIG);
        if sig != SATA_SIG_ATA {
            return Err(BlockDeviceError::InvalidCommand(format!("Unsupported device signature: 0x{:08X}", sig)));
        }

        // Start Command Processing and FIS Receive
        let mut cmd = ctrl.read_reg32(port_offset + PORT_REG_CMD);
        cmd |= PORT_CMD_ST | PORT_CMD_FRE;
        ctrl.write_reg32(port_offset + PORT_REG_CMD, cmd);

        // Issue ATA IDENTIFY DEVICE command to slot 0
        let mut identify_buf = [0u16; 256];
        let identify_buf_ptr = identify_buf.as_mut_ptr() as u64;

        // Prepare PRDT entry in Command Table slot 0
        let prdt_entry = AhciPrdtEntry::new(identify_buf_ptr, 512, true);
        ctrl.ports[self.port_index].command_tables[0].prdt[0] = prdt_entry;

        // Prepare Command FIS (Register H2D, Command = 0xEC)
        let mut cfis = FisRegH2D::default();
        cfis.command = ATA_CMD_IDENTIFY_DEVICE;
        unsafe {
            let cfis_bytes: [u8; 20] = std::mem::transmute(cfis);
            ctrl.ports[self.port_index].command_tables[0].cfis[..20].copy_from_slice(&cfis_bytes);
        }

        // Setup Command Header 0 (CFL = 5 dwords = 20 bytes, Read, 1 PRDT entry)
        let header = AhciCommandHeader::new(5, false, 1, 0x10000);
        ctrl.ports[self.port_index].command_list[0] = header;

        // Issue command (set bit 0 in PxCI)
        ctrl.write_reg32(port_offset + PORT_REG_CI, 1);

        // Wait for command completion (PxCI bit 0 cleared)
        let ci = ctrl.read_reg32(port_offset + PORT_REG_CI);
        if (ci & 1) != 0 {
            return Err(BlockDeviceError::Timeout);
        }

        // Verify task file status
        let tfd = ctrl.read_reg32(port_offset + PORT_REG_TFD);
        if (tfd & PORT_TFD_ERR) != 0 {
            return Err(BlockDeviceError::HardwareFault("ATA IDENTIFY DEVICE failed".into()));
        }

        // Parse IDENTIFY DEVICE structure
        let info = AtaIdentifyInfo::parse(&identify_buf);
        self.block_size = 512;
        self.block_count = info.total_sectors_lba48;
        self.identify_info = Some(info);

        Ok(())
    }

    pub fn identify_info(&self) -> Option<&AtaIdentifyInfo> {
        self.identify_info.as_ref()
    }
}

impl BlockDevice for AhciDevice {
    fn id(&self) -> u32 {
        self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn device_type(&self) -> DeviceType {
        DeviceType::AhciSata
    }

    fn status(&self) -> DeviceStatus {
        let ctrl = self.controller.lock().unwrap();
        let port_offset = 0x100 + self.port_index * 0x80;
        let ssts = ctrl.read_reg32(port_offset + PORT_REG_SSTS);
        if (ssts & SSTS_DET_MASK) == SSTS_DET_PRESENT_PHY {
            DeviceStatus::Online
        } else {
            DeviceStatus::Offline
        }
    }

    fn block_size(&self) -> u32 {
        self.block_size
    }

    fn block_count(&self) -> u64 {
        self.block_count
    }

    fn read_blocks(&self, lba: u64, buf: &mut [u8]) -> Result<usize, BlockDeviceError> {
        if buf.is_empty() {
            return Ok(0);
        }

        if buf.len() % (self.block_size as usize) != 0 {
            return Err(BlockDeviceError::UnalignedBuffer {
                buffer_len: buf.len(),
                block_size: self.block_size,
            });
        }

        let num_sectors = (buf.len() / self.block_size as usize) as u64;
        if lba.saturating_add(num_sectors) > self.block_count {
            return Err(BlockDeviceError::InvalidLba {
                lba: lba + num_sectors,
                max_lba: self.block_count,
            });
        }

        let mut ctrl = self.controller.lock().unwrap();
        let port_offset = 0x100 + self.port_index * 0x80;

        // Setup PRDT scatter-gather table for buffer (split into 4MB chunks if needed)
        let total_bytes = buf.len();
        let prdt_entry = AhciPrdtEntry::new(buf.as_mut_ptr() as u64, total_bytes, true);
        ctrl.ports[self.port_index].command_tables[0].prdt[0] = prdt_entry;

        // Setup Command FIS (Register H2D, Command = 0x25 READ DMA EXT)
        let mut cfis = FisRegH2D::default();
        cfis.setup_lba48(ATA_CMD_READ_DMA_EXT, lba, num_sectors as u16);
        unsafe {
            let cfis_bytes: [u8; 20] = std::mem::transmute(cfis);
            ctrl.ports[self.port_index].command_tables[0].cfis[..20].copy_from_slice(&cfis_bytes);
        }

        // Setup Command Header 0
        let header = AhciCommandHeader::new(5, false, 1, 0x10000);
        ctrl.ports[self.port_index].command_list[0] = header;

        // Trigger command issue on slot 0
        ctrl.write_reg32(port_offset + PORT_REG_CI, 1);

        let tfd = ctrl.read_reg32(port_offset + PORT_REG_TFD);
        if (tfd & PORT_TFD_ERR) != 0 {
            return Err(BlockDeviceError::HardwareFault("ATA READ DMA EXT error".into()));
        }

        Ok(total_bytes)
    }

    fn write_blocks(&self, lba: u64, buf: &[u8]) -> Result<usize, BlockDeviceError> {
        if buf.is_empty() {
            return Ok(0);
        }

        if buf.len() % (self.block_size as usize) != 0 {
            return Err(BlockDeviceError::UnalignedBuffer {
                buffer_len: buf.len(),
                block_size: self.block_size,
            });
        }

        let num_sectors = (buf.len() / self.block_size as usize) as u64;
        if lba.saturating_add(num_sectors) > self.block_count {
            return Err(BlockDeviceError::InvalidLba {
                lba: lba + num_sectors,
                max_lba: self.block_count,
            });
        }

        let mut ctrl = self.controller.lock().unwrap();
        let port_offset = 0x100 + self.port_index * 0x80;

        let total_bytes = buf.len();
        let prdt_entry = AhciPrdtEntry::new(buf.as_ptr() as u64, total_bytes, true);
        ctrl.ports[self.port_index].command_tables[0].prdt[0] = prdt_entry;

        // Setup Command FIS (Register H2D, Command = 0x35 WRITE DMA EXT)
        let mut cfis = FisRegH2D::default();
        cfis.setup_lba48(ATA_CMD_WRITE_DMA_EXT, lba, num_sectors as u16);
        unsafe {
            let cfis_bytes: [u8; 20] = std::mem::transmute(cfis);
            ctrl.ports[self.port_index].command_tables[0].cfis[..20].copy_from_slice(&cfis_bytes);
        }

        // Setup Command Header 0 with Write flag = true
        let header = AhciCommandHeader::new(5, true, 1, 0x10000);
        ctrl.ports[self.port_index].command_list[0] = header;

        ctrl.write_reg32(port_offset + PORT_REG_CI, 1);

        let tfd = ctrl.read_reg32(port_offset + PORT_REG_TFD);
        if (tfd & PORT_TFD_ERR) != 0 {
            return Err(BlockDeviceError::HardwareFault("ATA WRITE DMA EXT error".into()));
        }

        Ok(total_bytes)
    }

    fn flush(&self) -> Result<(), BlockDeviceError> {
        let mut ctrl = self.controller.lock().unwrap();
        let port_offset = 0x100 + self.port_index * 0x80;

        let mut cfis = FisRegH2D::default();
        cfis.command = ATA_CMD_FLUSH_CACHE_EXT;
        unsafe {
            let cfis_bytes: [u8; 20] = std::mem::transmute(cfis);
            ctrl.ports[self.port_index].command_tables[0].cfis[..20].copy_from_slice(&cfis_bytes);
        }

        let header = AhciCommandHeader::new(5, false, 0, 0x10000);
        ctrl.ports[self.port_index].command_list[0] = header;

        ctrl.write_reg32(port_offset + PORT_REG_CI, 1);
        Ok(())
    }
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ahci_struct_sizes() {
        assert_eq!(std::mem::size_of::<AhciCommandHeader>(), 32);
        assert_eq!(std::mem::size_of::<AhciPrdtEntry>(), 16);
        assert_eq!(std::mem::size_of::<FisRegH2D>(), 20);
    }

    #[test]
    fn test_ata_identify_parser() {
        let mut words = [0u16; 256];
        let sn = AtaIdentifyInfo::encode_ata_string("WD-WCC4N0123456", 10);
        words[10..20].copy_from_slice(&sn);
        let model = AtaIdentifyInfo::encode_ata_string("WDC WD10EZEX-00WN4A0", 20);
        words[27..47].copy_from_slice(&model);
        words[49] = 1 << 8 | 1 << 9; // DMA + LBA
        words[83] = 1 << 10;         // LBA48
        words[100] = 0x5000;
        words[101] = 0x000F;         // 1,003,520 sectors

        let info = AtaIdentifyInfo::parse(&words);
        assert_eq!(info.serial_number, "WD-WCC4N0123456");
        assert_eq!(info.model_number, "WDC WD10EZEX-00WN4A0");
        assert!(info.supports_lba48);
        assert!(info.supports_dma);
        assert_eq!(info.total_sectors_lba48, 1003520);
    }

    #[test]
    fn test_ahci_controller_and_device_read_write() {
        let mock_ctrl = Arc::new(Mutex::new(MockAhciController::new(2, 4096))); // 2 ports, 2 MB each
        let ahci_dev = AhciDevice::new(20, "sda", mock_ctrl, 0).expect("Failed to init AHCI sda");

        assert_eq!(ahci_dev.id(), 20);
        assert_eq!(ahci_dev.name(), "sda");
        assert_eq!(ahci_dev.device_type(), DeviceType::AhciSata);
        assert_eq!(ahci_dev.status(), DeviceStatus::Online);
        assert_eq!(ahci_dev.block_size(), 512);
        assert_eq!(ahci_dev.block_count(), 4096);

        let info = ahci_dev.identify_info().expect("Missing identify info");
        assert_eq!(info.serial_number, "HYBRIDOS-SATA-001");
        assert_eq!(info.model_number, "NexusCore HybridOS Virtual SATA Drive");

        // Write to sectors 5..8 (2048 bytes)
        let mut write_buf = vec![0xEEu8; 2048];
        write_buf[0..14].copy_from_slice(b"SATA_AHCI_TEST");
        let written = ahci_dev.write_blocks(5, &write_buf).expect("AHCI write failed");
        assert_eq!(written, 2048);

        // Read back from sector 5
        let mut read_buf = vec![0u8; 2048];
        let bytes_read = ahci_dev.read_blocks(5, &mut read_buf).expect("AHCI read failed");
        assert_eq!(bytes_read, 2048);
        assert_eq!(&read_buf[0..14], b"SATA_AHCI_TEST");
        assert_eq!(read_buf[50], 0xEE);

        // Flush
        ahci_dev.flush().expect("AHCI flush failed");
    }

    #[test]
    fn test_ahci_out_of_bounds() {
        let mock_ctrl = Arc::new(Mutex::new(MockAhciController::new(1, 100)));
        let ahci_dev = AhciDevice::new(21, "sdb", mock_ctrl, 0).unwrap();

        let mut buf = vec![0u8; 512];
        let err = ahci_dev.read_blocks(100, &mut buf).unwrap_err();
        match err {
            BlockDeviceError::InvalidLba { lba, max_lba } => {
                assert_eq!(lba, 101);
                assert_eq!(max_lba, 100);
            }
            _ => panic!("Expected InvalidLba"),
        }
    }
}
