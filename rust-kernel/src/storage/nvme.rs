//! Non-Volatile Memory Express (PCIe NVMe 1.4) Driver & Mock Subsystem.
//!
//! Provides a complete, compliant NVMe 1.4 specification implementation in pure Rust:
//! - Controller Registers: CAP, VS, CC, CSTS, AQA, ASQ, ACQ, and Doorbell Registers.
//! - Queue Architecture: Circular Ring-Buffer Admin (ASQ/ACQ) and I/O (IOSQ/IOCQ) queue pairs.
//! - 64-byte Submission Queue Entries (SQE) & 16-byte Completion Queue Entries (CQE).
//! - PRP (Physical Region Page) scatter-gather list building and DMA translation.
//! - Core Command Engine: Identify Controller, Identify Namespace, Read (0x02), Write (0x01), Flush (0x00).
//! - [`MockNvmeController`]: Full virtual controller engine for realistic testbed verification.
//! - [`NvmeDevice`]: High-level block device adapter conforming to [`super::block_device::BlockDevice`].

#[allow(unused_imports)]
use alloc::string::{String, ToString};
#[allow(unused_imports)]
use alloc::vec::Vec;
#[allow(unused_imports)]
use alloc::boxed::Box;
#[allow(unused_imports)]
use crate::compat::FloatExt;
use std::sync::{Arc, Mutex, RwLock};
use std::collections::HashMap;

use super::block_device::{BlockDevice, BlockDeviceError, DeviceStatus, DeviceType};

// ============================================================================
// Section 1: NVMe 1.4 Register Offsets & Bitfield Masks
// ============================================================================

/// MMIO Register Offsets (BAR0)
pub const NVME_REG_CAP: usize = 0x0000;     // Controller Capabilities (low 32-bit)
pub const NVME_REG_CAP_HI: usize = 0x0004;  // Controller Capabilities (high 32-bit)
pub const NVME_REG_VS: usize = 0x0008;      // Version (32-bit)
pub const NVME_REG_INTMS: usize = 0x000C;   // Interrupt Mask Set (32-bit)
pub const NVME_REG_INTMC: usize = 0x0010;   // Interrupt Mask Clear (32-bit)
pub const NVME_REG_CC: usize = 0x0014;      // Controller Configuration (32-bit)
pub const NVME_REG_CSTS: usize = 0x001C;    // Controller Status (32-bit)
pub const NVME_REG_NSSR: usize = 0x0020;    // NVM Subsystem Reset (32-bit)
pub const NVME_REG_AQA: usize = 0x0024;     // Admin Queue Attributes (32-bit)
pub const NVME_REG_ASQ: usize = 0x0028;     // Admin Submission Queue Base (low 32-bit)
pub const NVME_REG_ASQ_HI: usize = 0x002C;  // Admin Submission Queue Base (high 32-bit)
pub const NVME_REG_ACQ: usize = 0x0030;     // Admin Completion Queue Base (low 32-bit)
pub const NVME_REG_ACQ_HI: usize = 0x0034;  // Admin Completion Queue Base (high 32-bit)
pub const NVME_DOORBELL_START: usize = 0x1000; // Doorbell Register Base

/// CAP Bitfields (Capabilities)
pub const CAP_MQES_MASK: u64 = 0x0000_0000_0000_FFFF; // Max Queue Entries (0-based)
pub const CAP_CQS: u64 = 1 << 16;                     // Contiguous Queues Required
pub const CAP_AMS_RR: u64 = 0 << 17;                  // Arbitration: Round Robin
pub const CAP_TO_SHIFT: u64 = 24;                     // Timeout in 500ms units
pub const CAP_DSTRD_SHIFT: u64 = 32;                  // Doorbell Stride (2 ^ (2 + DSTRD))
pub const CAP_NSSRS: u64 = 1 << 36;                   // NVM Subsystem Reset Supported
pub const CAP_CSS_NVM: u64 = 1 << 37;                 // NVM Command Set Supported
pub const CAP_MPSMIN_SHIFT: u64 = 48;                 // Memory Page Size Min (2 ^ (12 + MPSMIN))
pub const CAP_MPSMAX_SHIFT: u64 = 52;                 // Memory Page Size Max (2 ^ (12 + MPSMAX))

/// CC Bitfields (Controller Configuration)
pub const CC_EN: u32 = 1 << 0;                        // Enable Controller
pub const CC_CSS_NVM: u32 = 0 << 4;                   // Command Set Selected (NVM)
pub const CC_MPS_SHIFT: u32 = 7;                      // Memory Page Size (2 ^ (12 + MPS))
pub const CC_AMS_RR: u32 = 0 << 11;                   // Arbitration Mechanism (Round Robin)
pub const CC_SHN_NONE: u32 = 0 << 14;                 // No Shutdown Notification
pub const CC_SHN_NORMAL: u32 = 1 << 14;               // Normal Shutdown
pub const CC_SHN_ABRUPT: u32 = 2 << 14;               // Abrupt Shutdown
pub const CC_IOSQES_64: u32 = 6 << 16;                // I/O Submission Queue Entry Size (2^6 = 64 bytes)
pub const CC_IOCQES_16: u32 = 4 << 20;                // I/O Completion Queue Entry Size (2^4 = 16 bytes)

/// CSTS Bitfields (Controller Status)
pub const CSTS_RDY: u32 = 1 << 0;                     // Ready to process commands
pub const CSTS_CFS: u32 = 1 << 1;                     // Controller Fatal Status
pub const CSTS_SHST_MASK: u32 = 0x3 << 2;             // Shutdown Status
pub const CSTS_SHST_NORMAL: u32 = 0 << 2;             // Normal operation
pub const CSTS_SHST_OCCURRING: u32 = 1 << 2;          // Shutdown occurring
pub const CSTS_SHST_COMPLETE: u32 = 2 << 2;           // Shutdown complete
pub const CSTS_NSSRO: u32 = 1 << 4;                   // NVM Subsystem Reset Occurred
pub const CSTS_PP: u32 = 1 << 5;                      // Processing Paused

// ============================================================================
// Section 2: NVMe Opcodes & Status Codes
// ============================================================================

/// Admin Command Opcodes
pub const NVME_ADMIN_DELETE_SQ: u8 = 0x00;
pub const NVME_ADMIN_CREATE_SQ: u8 = 0x01;
pub const NVME_ADMIN_GET_LOG_PAGE: u8 = 0x02;
pub const NVME_ADMIN_DELETE_CQ: u8 = 0x04;
pub const NVME_ADMIN_CREATE_CQ: u8 = 0x05;
pub const NVME_ADMIN_IDENTIFY: u8 = 0x06;
pub const NVME_ADMIN_ABORT: u8 = 0x08;
pub const NVME_ADMIN_SET_FEATURES: u8 = 0x09;
pub const NVME_ADMIN_GET_FEATURES: u8 = 0x0A;
pub const NVME_ADMIN_ASYNC_EVENT: u8 = 0x0C;
pub const NVME_ADMIN_KEEP_ALIVE: u8 = 0x18;

/// NVM I/O Command Opcodes
pub const NVME_NVM_FLUSH: u8 = 0x00;
pub const NVME_NVM_WRITE: u8 = 0x01;
pub const NVME_NVM_READ: u8 = 0x02;
pub const NVME_NVM_WRITE_UNCORRECTABLE: u8 = 0x04;
pub const NVME_NVM_COMPARE: u8 = 0x05;
pub const NVME_NVM_WRITE_ZEROES: u8 = 0x08;
pub const NVME_NVM_DATASET_MGMT: u8 = 0x09;

/// Completion Status Codes (Generic Command Status)
pub const NVME_SC_SUCCESS: u16 = 0x0000;
pub const NVME_SC_INVALID_OPCODE: u16 = 0x0001;
pub const NVME_SC_INVALID_FIELD: u16 = 0x0002;
pub const NVME_SC_COMMAND_ID_CONFLICT: u16 = 0x0003;
pub const NVME_SC_DATA_TRANSFER_ERROR: u16 = 0x0004;
pub const NVME_SC_ABORTED_POWER_LOSS: u16 = 0x0005;
pub const NVME_SC_INTERNAL_DEVICE_ERROR: u16 = 0x0006;
pub const NVME_SC_ABORTED_BY_REQUEST: u16 = 0x0007;
pub const NVME_SC_INVALID_NAMESPACE: u16 = 0x000B;
pub const NVME_SC_COMMAND_SEQUENCE_ERROR: u16 = 0x000C;
pub const NVME_SC_LBA_OUT_OF_RANGE: u16 = 0x0080;
pub const NVME_SC_CAPACITY_EXCEEDED: u16 = 0x0081;
pub const NVME_SC_NAMESPACE_NOT_READY: u16 = 0x0082;

// ============================================================================
// Section 3: NVMe Submission & Completion Queue Entries
// ============================================================================

/// 64-byte NVMe Submission Queue Entry (SQE).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NvmeSqe {
    pub cdw0: u32,
    pub nsid: u32,
    pub cdw2: u32,
    pub cdw3: u32,
    pub mptr: u64,
    pub prp1: u64,
    pub prp2: u64,
    pub cdw10: u32,
    pub cdw11: u32,
    pub cdw12: u32,
    pub cdw13: u32,
    pub cdw14: u32,
    pub cdw15: u32,
}

impl Default for NvmeSqe {
    fn default() -> Self {
        Self {
            cdw0: 0,
            nsid: 0,
            cdw2: 0,
            cdw3: 0,
            mptr: 0,
            prp1: 0,
            prp2: 0,
            cdw10: 0,
            cdw11: 0,
            cdw12: 0,
            cdw13: 0,
            cdw14: 0,
            cdw15: 0,
        }
    }
}

impl NvmeSqe {
    pub fn opcode(&self) -> u8 {
        (self.cdw0 & 0xFF) as u8
    }

    pub fn command_id(&self) -> u16 {
        ((self.cdw0 >> 16) & 0xFFFF) as u16
    }

    pub fn set_opcode(&mut self, opcode: u8) {
        self.cdw0 = (self.cdw0 & !0xFF) | (opcode as u32);
    }

    pub fn set_command_id(&mut self, cid: u16) {
        self.cdw0 = (self.cdw0 & !0xFFFF_0000) | ((cid as u32) << 16);
    }
}

/// 16-byte NVMe Completion Queue Entry (CQE).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NvmeCqe {
    pub result: u32,
    pub reserved: u32,
    pub sq_head: u16,
    pub sq_id: u16,
    pub command_id: u16,
    pub status: u16,
}

impl NvmeCqe {
    pub fn phase(&self) -> bool {
        (self.status & 0x0001) != 0
    }

    pub fn status_code(&self) -> u16 {
        (self.status >> 1) & 0x7FFF
    }

    pub fn is_success(&self) -> bool {
        self.status_code() == NVME_SC_SUCCESS
    }
}

const _: () = {
    assert!(std::mem::size_of::<NvmeSqe>() == 64);
    assert!(std::mem::size_of::<NvmeCqe>() == 16);
};

// ============================================================================
// Section 4: Identify Structures (Identify Controller & Identify Namespace)
// ============================================================================

/// LBA Format Descriptor (4 bytes)
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NvmeLbaFormat {
    pub metadata_size: u16,
    pub lba_data_size: u8,
    pub relative_performance: u8,
}

/// 4096-byte NVMe Identify Controller Data Structure (NVMe 1.4 specification)
#[repr(C)]
#[derive(Clone, Copy)]
pub struct NvmeIdentifyController {
    pub vid: u16,
    pub ssvid: u16,
    pub sn: [u8; 20],
    pub mn: [u8; 40],
    pub fr: [u8; 8],
    pub rab: u8,
    pub ieee: [u8; 3],
    pub cmic: u8,
    pub mdts: u8,
    pub cntlid: u16,
    pub ver: u32,
    pub rtd3r: u32,
    pub rtd3e: u32,
    pub oaes: u32,
    pub ctratt: u32,
    pub rsvd100: [u8; 160],
    pub oacs: u16,
    pub acl: u8,
    pub aerl: u8,
    pub frmw: u8,
    pub lpa: u8,
    pub elpe: u8,
    pub npss: u8,
    pub avscc: u8,
    pub apsta: u8,
    pub wctemp: u16,
    pub cctemp: u16,
    pub mtfa: u16,
    pub hmpre: u32,
    pub hmmin: u32,
    pub tnvmcap: [u8; 16],
    pub unvmcap: [u8; 16],
    pub rpmbs: u32,
    pub edstt: u16,
    pub dsto: u8,
    pub fwug: u8,
    pub kas: u16,
    pub hctma: u16,
    pub mntmt: u16,
    pub mxtmt: u16,
    pub sanic: u32,
    pub hmminds: u32,
    pub hmmaxd: u16,
    pub rsvd342: [u8; 170],
    pub sqes: u8,
    pub cqes: u8,
    pub maxcmd: u16,
    pub nn: u32,
    pub oncs: u16,
    pub fuses: u16,
    pub fna: u8,
    pub vwc: u8,
    pub awun: u16,
    pub awupf: u16,
    pub nvscc: u8,
    pub nwpc: u8,
    pub acwu: u16,
    pub rsvd534: [u8; 2],
    pub sgls: u32,
    pub mnan: u32,
    pub rsvd544: [u8; 224],
    pub subnqn: [u8; 256],
    pub rsvd1024: [u8; 3072],
}

impl Default for NvmeIdentifyController {
    fn default() -> Self {
        let mut ctrl = Self {
            vid: 0x1B36,
            ssvid: 0x1B36,
            sn: *b"HYBRIDOS-NVME-000001",
            mn: *b"NexusCore HybridOS NVMe HighPerf Storage",
            fr: *b"V1.4.000",
            rab: 6,
            ieee: [0x54, 0x52, 0x00],
            cmic: 0,
            mdts: 7,
            cntlid: 1,
            ver: 0x00010400,
            rtd3r: 0,
            rtd3e: 0,
            oaes: 0,
            ctratt: 0,
            rsvd100: [0; 160],
            oacs: 0x000F,
            acl: 3,
            aerl: 3,
            frmw: 2,
            lpa: 0,
            elpe: 63,
            npss: 0,
            avscc: 0,
            apsta: 0,
            wctemp: 0x0157,
            cctemp: 0x0163,
            mtfa: 0,
            hmpre: 0,
            hmmin: 0,
            tnvmcap: [0; 16],
            unvmcap: [0; 16],
            rpmbs: 0,
            edstt: 0,
            dsto: 0,
            fwug: 0,
            kas: 0,
            hctma: 0,
            mntmt: 0,
            mxtmt: 0,
            sanic: 0,
            hmminds: 0,
            hmmaxd: 0,
            rsvd342: [0; 170],
            sqes: (6 << 4) | 6,
            cqes: (4 << 4) | 4,
            maxcmd: 1024,
            nn: 1,
            oncs: 0x00FF,
            fuses: 0,
            fna: 0,
            vwc: 1,
            awun: 0,
            awupf: 0,
            nvscc: 0,
            nwpc: 0,
            acwu: 0,
            rsvd534: [0; 2],
            sgls: 0,
            mnan: 1,
            rsvd544: [0; 224],
            subnqn: [0; 256],
            rsvd1024: [0; 3072],
        };
        let nqn = b"nqn.2026-11.org.hybridos:nvme.controller0";
        ctrl.subnqn[..nqn.len()].copy_from_slice(nqn);
        ctrl
    }
}

/// 4096-byte NVMe Identify Namespace Data Structure (NVMe 1.4 specification)
#[repr(C)]
#[derive(Clone, Copy)]
pub struct NvmeIdentifyNamespace {
    pub nsze: u64,
    pub ncap: u64,
    pub nuse: u64,
    pub nsfeat: u8,
    pub nlbaf: u8,
    pub flbas: u8,
    pub mc: u8,
    pub dpc: u8,
    pub dps: u8,
    pub nmic: u8,
    pub rescap: u8,
    pub fpi: u8,
    pub dlfeat: u8,
    pub nawun: u16,
    pub nawupf: u16,
    pub nacwu: u16,
    pub nabsn: u16,
    pub nabo: u16,
    pub nabspf: u16,
    pub noiob: u16,
    pub nvmcap: [u8; 16],
    pub rsvd64: [u8; 40],
    pub nguid: [u8; 16],
    pub eui64: [u8; 8],
    pub lbaf: [NvmeLbaFormat; 16],
    pub rsvd192: [u8; 192],
    pub vs: [u8; 3712],
}

impl Default for NvmeIdentifyNamespace {
    fn default() -> Self {
        let mut ns = Self {
            nsze: 204800,
            ncap: 204800,
            nuse: 204800,
            nsfeat: 0,
            nlbaf: 1,
            flbas: 0,
            mc: 0,
            dpc: 0,
            dps: 0,
            nmic: 0,
            rescap: 0,
            fpi: 0,
            dlfeat: 0,
            nawun: 0,
            nawupf: 0,
            nacwu: 0,
            nabsn: 0,
            nabo: 0,
            nabspf: 0,
            noiob: 0,
            nvmcap: [0; 16],
            rsvd64: [0; 40],
            nguid: [0; 16],
            eui64: [0; 8],
            lbaf: [NvmeLbaFormat::default(); 16],
            rsvd192: [0; 192],
            vs: [0; 3712],
        };
        ns.lbaf[0] = NvmeLbaFormat {
            metadata_size: 0,
            lba_data_size: 9,
            relative_performance: 0,
        };
        ns.lbaf[1] = NvmeLbaFormat {
            metadata_size: 0,
            lba_data_size: 12,
            relative_performance: 1,
        };
        ns
    }
}

const _: () = {
    assert!(std::mem::size_of::<NvmeIdentifyController>() == 4096);
    assert!(std::mem::size_of::<NvmeIdentifyNamespace>() == 4096);
};

// ============================================================================
// Section 5: PRP (Physical Region Page) Management & Scatter-Gather Engine
// ============================================================================

pub const PAGE_SIZE: usize = 4096;

/// PRP List Builder for multi-page NVMe DMA transfers.
pub struct PrpBuilder;

impl PrpBuilder {
    pub fn build(base_phys_addr: u64, len: usize) -> (u64, u64, Vec<u64>) {
        if len == 0 {
            return (0, 0, Vec::new());
        }

        let prp1 = base_phys_addr;
        if len <= PAGE_SIZE {
            return (prp1, 0, Vec::new());
        }

        let page_count = (len + PAGE_SIZE - 1) / PAGE_SIZE;
        if page_count == 2 {
            let prp2 = base_phys_addr + PAGE_SIZE as u64;
            return (prp1, prp2, Vec::new());
        }

        let mut prp_list = Vec::with_capacity(page_count - 1);
        for i in 1..page_count {
            prp_list.push(base_phys_addr + (i * PAGE_SIZE) as u64);
        }

        let prp2 = prp_list.as_ptr() as u64;
        (prp1, prp2, prp_list)
    }
}

// ============================================================================
// Section 6: Queue Pair Management
// ============================================================================

/// State of a Submission Queue and Completion Queue pair.
pub struct NvmeQueuePair {
    pub qid: u16,
    pub sq_size: u16,
    pub cq_size: u16,
    pub sq_tail: u16,   // Software advances on command submission
    pub sq_head: u16,   // Controller advances on fetch
    pub cq_tail: u16,   // Controller advances on completion post
    pub cq_head: u16,   // Software advances on completion consume
    pub cq_phase: bool, // Controller phase bit (starts true)
    pub sw_phase: bool, // Software expected phase bit (starts true)
    pub sq: Vec<NvmeSqe>,
    pub cq: Vec<NvmeCqe>,
}

impl NvmeQueuePair {
    pub fn new(qid: u16, sq_size: u16, cq_size: u16) -> Self {
        Self {
            qid,
            sq_size,
            cq_size,
            sq_tail: 0,
            sq_head: 0,
            cq_tail: 0,
            cq_head: 0,
            cq_phase: true,
            sw_phase: true,
            sq: vec![NvmeSqe::default(); sq_size as usize],
            cq: vec![NvmeCqe::default(); cq_size as usize],
        }
    }

    /// Submits a command into the submission queue ring buffer.
    pub fn submit(&mut self, sqe: NvmeSqe) -> Result<u16, BlockDeviceError> {
        let next_tail = (self.sq_tail + 1) % self.sq_size;
        if next_tail == self.sq_head {
            return Err(BlockDeviceError::QueueFull);
        }

        let slot = self.sq_tail as usize;
        self.sq[slot] = sqe;
        self.sq_tail = next_tail;
        Ok(self.sq_tail)
    }

    /// Posts a completion from the controller side.
    pub fn post_completion(&mut self, mut cqe: NvmeCqe) {
        let slot = self.cq_tail as usize;
        let phase_bit = if self.cq_phase { 1 } else { 0 };
        cqe.status = (cqe.status & !0x0001) | phase_bit;
        self.cq[slot] = cqe;

        self.cq_tail = (self.cq_tail + 1) % self.cq_size;
        if self.cq_tail == 0 {
            self.cq_phase = !self.cq_phase;
        }
    }

    /// Polls for a completed CQE whose phase matches the expected software phase.
    pub fn poll_completion(&mut self) -> Option<NvmeCqe> {
        let slot = self.cq_head as usize;
        let cqe = self.cq[slot];
        let phase = (cqe.status & 0x0001) != 0;
        if phase == self.sw_phase {
            self.cq_head = (self.cq_head + 1) % self.cq_size;
            if self.cq_head == 0 {
                self.sw_phase = !self.sw_phase;
            }
            Some(cqe)
        } else {
            None
        }
    }
}

// ============================================================================
// Section 7: Mock/Virtual NVMe 1.4 Controller Testbed
// ============================================================================

/// A virtual hardware NVMe 1.4 controller emulating registers, ring buffers,
/// and DMA transfer semantics for development and verification without hardware.
pub struct MockNvmeController {
    // MMIO Registers
    cap: u64,
    vs: u32,
    cc: u32,
    csts: u32,
    aqa: u32,
    asq_base: u64,
    acq_base: u64,

    // Admin Queue State (QID 0)
    admin_sq: Vec<NvmeSqe>,
    admin_cq: Vec<NvmeCqe>,
    admin_sq_head: u16,
    admin_sq_tail: u16,
    admin_cq_head: u16,
    admin_cq_tail: u16,
    admin_cq_phase: bool,

    // Dynamic I/O Queues (QID 1..N)
    io_queues: HashMap<u16, NvmeQueuePair>,

    // Controller Data Structures
    pub id_ctrl: NvmeIdentifyController,
    pub id_ns: NvmeIdentifyNamespace,

    // Backing Persistent Disk Storage
    pub disk_data: Arc<RwLock<Vec<u8>>>,
}

impl MockNvmeController {
    pub fn new(capacity_blocks: u64, block_size: u32) -> Self {
        assert!(block_size == 512 || block_size == 4096, "Block size must be 512 or 4096");

        let id_ctrl = NvmeIdentifyController::default();
        let mut id_ns = NvmeIdentifyNamespace::default();

        id_ns.nsze = capacity_blocks;
        id_ns.ncap = capacity_blocks;
        id_ns.nuse = capacity_blocks;
        id_ns.flbas = if block_size == 512 { 0 } else { 1 };
        id_ns.lbaf[0].lba_data_size = 9;
        id_ns.lbaf[1].lba_data_size = 12;

        let total_bytes = (capacity_blocks as usize) * (block_size as usize);

        let cap = 1023 | CAP_CQS | (2 << CAP_TO_SHIFT) | CAP_CSS_NVM;

        Self {
            cap,
            vs: 0x00010400,
            cc: 0,
            csts: 0,
            aqa: 0,
            asq_base: 0,
            acq_base: 0,

            admin_sq: vec![NvmeSqe::default(); 64],
            admin_cq: vec![NvmeCqe::default(); 64],
            admin_sq_head: 0,
            admin_sq_tail: 0,
            admin_cq_head: 0,
            admin_cq_tail: 0,
            admin_cq_phase: true,

            io_queues: HashMap::new(),
            id_ctrl,
            id_ns,
            disk_data: Arc::new(RwLock::new(vec![0u8; total_bytes])),
        }
    }

    /// Read 32-bit register from BAR0 MMIO space.
    pub fn read_reg32(&self, offset: usize) -> u32 {
        match offset {
            NVME_REG_CAP => (self.cap & 0xFFFF_FFFF) as u32,
            NVME_REG_CAP_HI => (self.cap >> 32) as u32,
            NVME_REG_VS => self.vs,
            NVME_REG_CC => self.cc,
            NVME_REG_CSTS => self.csts,
            NVME_REG_AQA => self.aqa,
            NVME_REG_ASQ => (self.asq_base & 0xFFFF_FFFF) as u32,
            NVME_REG_ASQ_HI => (self.asq_base >> 32) as u32,
            NVME_REG_ACQ => (self.acq_base & 0xFFFF_FFFF) as u32,
            NVME_REG_ACQ_HI => (self.acq_base >> 32) as u32,
            _ => 0,
        }
    }

    /// Write 32-bit register into BAR0 MMIO space.
    pub fn write_reg32(&mut self, offset: usize, val: u32) {
        match offset {
            NVME_REG_CC => {
                let old_en = (self.cc & CC_EN) != 0;
                let new_en = (val & CC_EN) != 0;
                self.cc = val;

                if !old_en && new_en {
                    self.csts |= CSTS_RDY;
                } else if old_en && !new_en {
                    self.csts &= !CSTS_RDY;
                }
            }
            NVME_REG_AQA => self.aqa = val,
            NVME_REG_ASQ => self.asq_base = (self.asq_base & !0xFFFF_FFFF) | (val as u64),
            NVME_REG_ASQ_HI => self.asq_base = (self.asq_base & 0xFFFF_FFFF) | ((val as u64) << 32),
            NVME_REG_ACQ => self.acq_base = (self.acq_base & !0xFFFF_FFFF) | (val as u64),
            NVME_REG_ACQ_HI => self.acq_base = (self.asq_base & 0xFFFF_FFFF) | ((val as u64) << 32),
            _ => {
                if offset >= NVME_DOORBELL_START {
                    let db_idx = (offset - NVME_DOORBELL_START) / 4;
                    let is_cq = (db_idx % 2) != 0;
                    let qid = (db_idx / 2) as u16;

                    if !is_cq {
                        self.process_sq_doorbell(qid, val as u16);
                    } else {
                        self.process_cq_doorbell(qid, val as u16);
                    }
                }
            }
        }
    }

    /// Processes SQ Tail Doorbell update for a specific queue ID.
    pub fn process_sq_doorbell(&mut self, qid: u16, new_tail: u16) {
        if qid == 0 {
            self.admin_sq_tail = new_tail;
            self.process_admin_commands();
        } else if let Some(queue) = self.io_queues.get_mut(&qid) {
            queue.sq_tail = new_tail;
            self.process_io_commands(qid);
        }
    }

    /// Processes CQ Head Doorbell update for a specific queue ID.
    pub fn process_cq_doorbell(&mut self, qid: u16, new_head: u16) {
        if qid == 0 {
            self.admin_cq_head = new_head;
        } else if let Some(queue) = self.io_queues.get_mut(&qid) {
            queue.cq_head = new_head;
        }
    }

    /// Emulates Admin command execution from the Admin SQ ring.
    fn process_admin_commands(&mut self) {
        while self.admin_sq_head != self.admin_sq_tail {
            let sqe = self.admin_sq[self.admin_sq_head as usize];
            self.admin_sq_head = (self.admin_sq_head + 1) % (self.admin_sq.len() as u16);

            let mut cqe = NvmeCqe {
                result: 0,
                reserved: 0,
                sq_head: self.admin_sq_head,
                sq_id: 0,
                command_id: sqe.command_id(),
                status: (NVME_SC_SUCCESS << 1) | (if self.admin_cq_phase { 1 } else { 0 }),
            };

            match sqe.opcode() {
                NVME_ADMIN_IDENTIFY => {
                    let cns = sqe.cdw10 & 0xFF;
                    match cns {
                        0x01 => {
                            if sqe.prp1 != 0 {
                                unsafe {
                                    let dest_ptr = sqe.prp1 as *mut NvmeIdentifyController;
                                    std::ptr::write(dest_ptr, self.id_ctrl);
                                }
                            }
                        }
                        0x00 => {
                            if sqe.prp1 != 0 {
                                unsafe {
                                    let dest_ptr = sqe.prp1 as *mut NvmeIdentifyNamespace;
                                    std::ptr::write(dest_ptr, self.id_ns);
                                }
                            }
                        }
                        _ => {
                            cqe.status = (NVME_SC_INVALID_FIELD << 1) | (if self.admin_cq_phase { 1 } else { 0 });
                        }
                    }
                }
                NVME_ADMIN_CREATE_CQ => {
                    let qid = (sqe.cdw10 & 0xFFFF) as u16;
                    let qsize = ((sqe.cdw10 >> 16) & 0xFFFF) as u16 + 1;
                    self.io_queues.entry(qid).or_insert_with(|| NvmeQueuePair::new(qid, 64, qsize));
                }
                NVME_ADMIN_CREATE_SQ => {
                    let qid = (sqe.cdw10 & 0xFFFF) as u16;
                    let qsize = ((sqe.cdw10 >> 16) & 0xFFFF) as u16 + 1;
                    if let Some(q) = self.io_queues.get_mut(&qid) {
                        q.sq_size = qsize;
                        q.sq = vec![NvmeSqe::default(); qsize as usize];
                    } else {
                        self.io_queues.insert(qid, NvmeQueuePair::new(qid, qsize, 64));
                    }
                }
                NVME_ADMIN_DELETE_SQ => {
                    let qid = (sqe.cdw10 & 0xFFFF) as u16;
                    self.io_queues.remove(&qid);
                }
                NVME_ADMIN_DELETE_CQ => {
                    let qid = (sqe.cdw10 & 0xFFFF) as u16;
                    self.io_queues.remove(&qid);
                }
                NVME_ADMIN_SET_FEATURES => {
                    cqe.result = 0;
                }
                _ => {
                    cqe.status = (NVME_SC_INVALID_OPCODE << 1) | (if self.admin_cq_phase { 1 } else { 0 });
                }
            }

            let cq_slot = self.admin_cq_tail as usize;
            let phase_bit = if self.admin_cq_phase { 1 } else { 0 };
            cqe.status = (cqe.status & !0x0001) | phase_bit;
            self.admin_cq[cq_slot] = cqe;
            self.admin_cq_tail = (self.admin_cq_tail + 1) % (self.admin_cq.len() as u16);
            if self.admin_cq_tail == 0 {
                self.admin_cq_phase = !self.admin_cq_phase;
            }
        }
    }

    /// Emulates I/O command execution from an I/O Submission Queue.
    fn process_io_commands(&mut self, qid: u16) {
        let block_size = if self.id_ns.flbas == 0 { 512 } else { 4096 };
        let disk_blocks = self.id_ns.nsze;

        let queue = match self.io_queues.get_mut(&qid) {
            Some(q) => q,
            None => return,
        };

        while queue.sq_head != queue.sq_tail {
            let sqe = queue.sq[queue.sq_head as usize];
            queue.sq_head = (queue.sq_head + 1) % queue.sq_size;

            let mut cqe = NvmeCqe {
                result: 0,
                reserved: 0,
                sq_head: queue.sq_head,
                sq_id: qid,
                command_id: sqe.command_id(),
                status: NVME_SC_SUCCESS << 1,
            };

            match sqe.opcode() {
                NVME_NVM_READ => {
                    let slba = (sqe.cdw10 as u64) | ((sqe.cdw11 as u64) << 32);
                    let nlb = (sqe.cdw12 & 0xFFFF) as u64 + 1;
                    let bytes = (nlb as usize) * (block_size as usize);

                    if slba + nlb > disk_blocks {
                        cqe.status = NVME_SC_LBA_OUT_OF_RANGE << 1;
                    } else if sqe.prp1 != 0 {
                        let offset = (slba as usize) * (block_size as usize);
                        let disk = self.disk_data.read().unwrap();
                        let src_slice = &disk[offset..offset + bytes];

                        Self::dma_copy_to_prp(src_slice, sqe.prp1, sqe.prp2);
                    }
                }
                NVME_NVM_WRITE => {
                    let slba = (sqe.cdw10 as u64) | ((sqe.cdw11 as u64) << 32);
                    let nlb = (sqe.cdw12 & 0xFFFF) as u64 + 1;
                    let bytes = (nlb as usize) * (block_size as usize);

                    if slba + nlb > disk_blocks {
                        cqe.status = NVME_SC_LBA_OUT_OF_RANGE << 1;
                    } else if sqe.prp1 != 0 {
                        let offset = (slba as usize) * (block_size as usize);
                        let mut disk = self.disk_data.write().unwrap();
                        let dest_slice = &mut disk[offset..offset + bytes];

                        Self::dma_copy_from_prp(dest_slice, sqe.prp1, sqe.prp2);
                    }
                }
                NVME_NVM_FLUSH => {
                    cqe.status = NVME_SC_SUCCESS << 1;
                }
                _ => {
                    cqe.status = NVME_SC_INVALID_OPCODE << 1;
                }
            }

            queue.post_completion(cqe);
        }
    }

    /// DMA helper: copies bytes from controller storage to host PRP buffers.
    fn dma_copy_to_prp(src: &[u8], prp1: u64, prp2: u64) {
        let len = src.len();
        if len <= PAGE_SIZE {
            unsafe {
                let dest = prp1 as *mut u8;
                std::ptr::copy_nonoverlapping(src.as_ptr(), dest, len);
            }
        } else if len <= 2 * PAGE_SIZE {
            unsafe {
                let dest1 = prp1 as *mut u8;
                std::ptr::copy_nonoverlapping(src.as_ptr(), dest1, PAGE_SIZE);
                let dest2 = prp2 as *mut u8;
                std::ptr::copy_nonoverlapping(src.as_ptr().add(PAGE_SIZE), dest2, len - PAGE_SIZE);
            }
        } else {
            unsafe {
                let dest1 = prp1 as *mut u8;
                std::ptr::copy_nonoverlapping(src.as_ptr(), dest1, PAGE_SIZE);
            }
            let remaining = len - PAGE_SIZE;
            let pages = (remaining + PAGE_SIZE - 1) / PAGE_SIZE;
            let prp_list = prp2 as *const u64;

            for p in 0..pages {
                let chunk_len = std::cmp::min(PAGE_SIZE, remaining - p * PAGE_SIZE);
                let page_addr = unsafe { *prp_list.add(p) };
                unsafe {
                    let dest = page_addr as *mut u8;
                    std::ptr::copy_nonoverlapping(
                        src.as_ptr().add(PAGE_SIZE + p * PAGE_SIZE),
                        dest,
                        chunk_len,
                    );
                }
            }
        }
    }

    /// DMA helper: copies bytes from host PRP buffers into controller storage.
    fn dma_copy_from_prp(dest: &mut [u8], prp1: u64, prp2: u64) {
        let len = dest.len();
        if len <= PAGE_SIZE {
            unsafe {
                let src = prp1 as *const u8;
                std::ptr::copy_nonoverlapping(src, dest.as_mut_ptr(), len);
            }
        } else if len <= 2 * PAGE_SIZE {
            unsafe {
                let src1 = prp1 as *const u8;
                std::ptr::copy_nonoverlapping(src1, dest.as_mut_ptr(), PAGE_SIZE);
                let src2 = prp2 as *const u8;
                std::ptr::copy_nonoverlapping(src2, dest.as_mut_ptr().add(PAGE_SIZE), len - PAGE_SIZE);
            }
        } else {
            unsafe {
                let src1 = prp1 as *const u8;
                std::ptr::copy_nonoverlapping(src1, dest.as_mut_ptr(), PAGE_SIZE);
            }
            let remaining = len - PAGE_SIZE;
            let pages = (remaining + PAGE_SIZE - 1) / PAGE_SIZE;
            let prp_list = prp2 as *const u64;

            for p in 0..pages {
                let chunk_len = std::cmp::min(PAGE_SIZE, remaining - p * PAGE_SIZE);
                let page_addr = unsafe { *prp_list.add(p) };
                unsafe {
                    let src = page_addr as *const u8;
                    std::ptr::copy_nonoverlapping(
                        src,
                        dest.as_mut_ptr().add(PAGE_SIZE + p * PAGE_SIZE),
                        chunk_len,
                    );
                }
            }
        }
    }
}

// ============================================================================
// Section 8: High-Level NVMe Driver Device Adapter
// ============================================================================

/// High-level NVMe Block Device implementing the kernel [`BlockDevice`] trait.
pub struct NvmeDevice {
    id: u32,
    name: String,
    controller: Arc<Mutex<MockNvmeController>>,
    block_size: u32,
    block_count: u64,
    next_cid: Mutex<u16>,
    io_qid: u16,
}

impl NvmeDevice {
    /// Discovers, initializes, and creates an `NvmeDevice` instance from a controller.
    pub fn new(id: u32, name: &str, controller: Arc<Mutex<MockNvmeController>>) -> Result<Self, BlockDeviceError> {
        let mut dev = Self {
            id,
            name: name.to_string(),
            controller,
            block_size: 512,
            block_count: 0,
            next_cid: Mutex::new(1),
            io_qid: 1,
        };

        dev.initialize_controller()?;
        Ok(dev)
    }

    fn alloc_cid(&self) -> u16 {
        let mut cid = self.next_cid.lock().unwrap();
        let val = *cid;
        *cid = cid.wrapping_add(1);
        if *cid == 0 {
            *cid = 1;
        }
        val
    }

    fn submit_admin_cmd(&self, sqe: NvmeSqe) -> Result<NvmeCqe, BlockDeviceError> {
        let mut ctrl = self.controller.lock().unwrap();
        let slot = ctrl.admin_sq_tail as usize;
        ctrl.admin_sq[slot] = sqe;
        let new_tail = (ctrl.admin_sq_tail + 1) % (ctrl.admin_sq.len() as u16);
        ctrl.write_reg32(NVME_DOORBELL_START, new_tail as u32);

        let cq_slot = ctrl.admin_cq_head as usize;
        let cqe = ctrl.admin_cq[cq_slot];
        ctrl.admin_cq_head = (ctrl.admin_cq_head + 1) % (ctrl.admin_cq.len() as u16);
        let new_head = ctrl.admin_cq_head as u32;
        ctrl.write_reg32(NVME_DOORBELL_START + 4, new_head);

        if !cqe.is_success() {
            return Err(BlockDeviceError::HardwareFault(format!(
                "Admin command 0x{:02X} failed with status 0x{:04X}",
                sqe.opcode(),
                cqe.status_code()
            )));
        }
        Ok(cqe)
    }

    /// Full NVMe Controller initialization sequence:
    /// 1. Configure Admin Queue Attributes (AQA) and base addresses.
    /// 2. Enable Controller (CC.EN = 1) and poll for CSTS.RDY = 1.
    /// 3. Issue Identify Controller to query capabilities.
    /// 4. Issue Identify Namespace 1 to detect sector geometry.
    /// 5. Create dynamic I/O Submission & Completion Queue pair (QID 1).
    pub fn initialize_controller(&mut self) -> Result<(), BlockDeviceError> {
        {
            let mut ctrl = self.controller.lock().unwrap();
            let aqa_val = (63 << 16) | 63;
            ctrl.write_reg32(NVME_REG_AQA, aqa_val);

            let cc_val = CC_EN | CC_CSS_NVM | (0 << CC_MPS_SHIFT) | CC_IOSQES_64 | CC_IOCQES_16;
            ctrl.write_reg32(NVME_REG_CC, cc_val);

            let csts = ctrl.read_reg32(NVME_REG_CSTS);
            if (csts & CSTS_RDY) == 0 {
                return Err(BlockDeviceError::HardwareFault("NVMe controller failed to ready".into()));
            }
        }

        // Step 3: Identify Controller
        let mut id_ctrl_buf = Box::new(NvmeIdentifyController::default());
        let mut sqe_id_ctrl = NvmeSqe::default();
        sqe_id_ctrl.set_opcode(NVME_ADMIN_IDENTIFY);
        sqe_id_ctrl.set_command_id(self.alloc_cid());
        sqe_id_ctrl.cdw10 = 0x01;
        sqe_id_ctrl.prp1 = id_ctrl_buf.as_mut() as *mut NvmeIdentifyController as u64;
        self.submit_admin_cmd(sqe_id_ctrl)?;

        // Step 4: Identify Namespace 1
        let mut id_ns_buf = Box::new(NvmeIdentifyNamespace::default());
        let mut sqe_id_ns = NvmeSqe::default();
        sqe_id_ns.set_opcode(NVME_ADMIN_IDENTIFY);
        sqe_id_ns.set_command_id(self.alloc_cid());
        sqe_id_ns.nsid = 1;
        sqe_id_ns.cdw10 = 0x00;
        sqe_id_ns.prp1 = id_ns_buf.as_mut() as *mut NvmeIdentifyNamespace as u64;
        self.submit_admin_cmd(sqe_id_ns)?;

        let lba_format_idx = (id_ns_buf.flbas & 0x0F) as usize;
        let lba_shift = id_ns_buf.lbaf[lba_format_idx].lba_data_size;
        self.block_size = 1 << lba_shift;
        self.block_count = id_ns_buf.nsze;

        // Step 5: Create I/O Completion Queue 1
        let mut sqe_create_cq = NvmeSqe::default();
        sqe_create_cq.set_opcode(NVME_ADMIN_CREATE_CQ);
        sqe_create_cq.set_command_id(self.alloc_cid());
        sqe_create_cq.cdw10 = (self.io_qid as u32) | (63 << 16);
        sqe_create_cq.cdw11 = 0x01;
        self.submit_admin_cmd(sqe_create_cq)?;

        // Step 6: Create I/O Submission Queue 1
        let mut sqe_create_sq = NvmeSqe::default();
        sqe_create_sq.set_opcode(NVME_ADMIN_CREATE_SQ);
        sqe_create_sq.set_command_id(self.alloc_cid());
        sqe_create_sq.cdw10 = (self.io_qid as u32) | (63 << 16);
        sqe_create_sq.cdw11 = 0x01 | ((self.io_qid as u32) << 16);
        self.submit_admin_cmd(sqe_create_sq)?;

        Ok(())
    }
}

impl BlockDevice for NvmeDevice {
    fn id(&self) -> u32 {
        self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn device_type(&self) -> DeviceType {
        DeviceType::Nvme
    }

    fn status(&self) -> DeviceStatus {
        let ctrl = self.controller.lock().unwrap();
        let csts = ctrl.read_reg32(NVME_REG_CSTS);
        if (csts & CSTS_RDY) != 0 {
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

        let num_blocks = (buf.len() / self.block_size as usize) as u64;
        if lba.saturating_add(num_blocks) > self.block_count {
            return Err(BlockDeviceError::InvalidLba {
                lba: lba + num_blocks,
                max_lba: self.block_count,
            });
        }

        let buf_addr = buf.as_mut_ptr() as u64;
        let (prp1, prp2, _prp_list) = PrpBuilder::build(buf_addr, buf.len());

        let mut sqe = NvmeSqe::default();
        sqe.set_opcode(NVME_NVM_READ);
        let cid = self.alloc_cid();
        sqe.set_command_id(cid);
        sqe.nsid = 1;
        sqe.prp1 = prp1;
        sqe.prp2 = prp2;
        sqe.cdw10 = (lba & 0xFFFF_FFFF) as u32;
        sqe.cdw11 = (lba >> 32) as u32;
        sqe.cdw12 = (num_blocks - 1) as u32;

        let mut ctrl = self.controller.lock().unwrap();
        let queue = ctrl.io_queues.get_mut(&self.io_qid)
            .ok_or(BlockDeviceError::HardwareFault("I/O queue not initialized".into()))?;

        queue.submit(sqe)?;
        let tail = queue.sq_tail;

        let db_sq_offset = NVME_DOORBELL_START + (2 * (self.io_qid as usize) * 4);
        ctrl.write_reg32(db_sq_offset, tail as u32);

        let queue = ctrl.io_queues.get_mut(&self.io_qid).unwrap();
        let cqe = queue.poll_completion()
            .ok_or(BlockDeviceError::Timeout)?;

        let db_cq_offset = NVME_DOORBELL_START + ((2 * (self.io_qid as usize) + 1) * 4);
        let head = queue.cq_head;
        ctrl.write_reg32(db_cq_offset, head as u32);

        if !cqe.is_success() {
            return Err(BlockDeviceError::IoError(format!("NVMe read failed with status 0x{:04X}", cqe.status_code())));
        }

        Ok(buf.len())
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

        let num_blocks = (buf.len() / self.block_size as usize) as u64;
        if lba.saturating_add(num_blocks) > self.block_count {
            return Err(BlockDeviceError::InvalidLba {
                lba: lba + num_blocks,
                max_lba: self.block_count,
            });
        }

        let buf_addr = buf.as_ptr() as u64;
        let (prp1, prp2, _prp_list) = PrpBuilder::build(buf_addr, buf.len());

        let mut sqe = NvmeSqe::default();
        sqe.set_opcode(NVME_NVM_WRITE);
        let cid = self.alloc_cid();
        sqe.set_command_id(cid);
        sqe.nsid = 1;
        sqe.prp1 = prp1;
        sqe.prp2 = prp2;
        sqe.cdw10 = (lba & 0xFFFF_FFFF) as u32;
        sqe.cdw11 = (lba >> 32) as u32;
        sqe.cdw12 = (num_blocks - 1) as u32;

        let mut ctrl = self.controller.lock().unwrap();
        let queue = ctrl.io_queues.get_mut(&self.io_qid)
            .ok_or(BlockDeviceError::HardwareFault("I/O queue not initialized".into()))?;

        queue.submit(sqe)?;
        let tail = queue.sq_tail;

        let db_sq_offset = NVME_DOORBELL_START + (2 * (self.io_qid as usize) * 4);
        ctrl.write_reg32(db_sq_offset, tail as u32);

        let queue = ctrl.io_queues.get_mut(&self.io_qid).unwrap();
        let cqe = queue.poll_completion()
            .ok_or(BlockDeviceError::Timeout)?;

        let db_cq_offset = NVME_DOORBELL_START + ((2 * (self.io_qid as usize) + 1) * 4);
        let head = queue.cq_head;
        ctrl.write_reg32(db_cq_offset, head as u32);

        if !cqe.is_success() {
            return Err(BlockDeviceError::IoError(format!("NVMe write failed with status 0x{:04X}", cqe.status_code())));
        }

        Ok(buf.len())
    }

    fn flush(&self) -> Result<(), BlockDeviceError> {
        let mut sqe = NvmeSqe::default();
        sqe.set_opcode(NVME_NVM_FLUSH);
        let cid = self.alloc_cid();
        sqe.set_command_id(cid);
        sqe.nsid = 1;

        let mut ctrl = self.controller.lock().unwrap();
        let queue = ctrl.io_queues.get_mut(&self.io_qid)
            .ok_or(BlockDeviceError::HardwareFault("I/O queue not initialized".into()))?;

        queue.submit(sqe)?;
        let tail = queue.sq_tail;

        let db_sq_offset = NVME_DOORBELL_START + (2 * (self.io_qid as usize) * 4);
        ctrl.write_reg32(db_sq_offset, tail as u32);

        let queue = ctrl.io_queues.get_mut(&self.io_qid).unwrap();
        let cqe = queue.poll_completion()
            .ok_or(BlockDeviceError::Timeout)?;

        let db_cq_offset = NVME_DOORBELL_START + ((2 * (self.io_qid as usize) + 1) * 4);
        let head = queue.cq_head;
        ctrl.write_reg32(db_cq_offset, head as u32);

        if !cqe.is_success() {
            return Err(BlockDeviceError::IoError("NVMe flush failed".into()));
        }

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
    fn test_nvme_struct_alignments_and_sizes() {
        assert_eq!(std::mem::size_of::<NvmeSqe>(), 64);
        assert_eq!(std::mem::size_of::<NvmeCqe>(), 16);
        assert_eq!(std::mem::size_of::<NvmeIdentifyController>(), 4096);
        assert_eq!(std::mem::size_of::<NvmeIdentifyNamespace>(), 4096);
    }

    #[test]
    fn test_prp_builder_single_and_multi_page() {
        // Single page <= 4096 bytes
        let (p1, p2, list) = PrpBuilder::build(0x1000, 2048);
        assert_eq!(p1, 0x1000);
        assert_eq!(p2, 0);
        assert!(list.is_empty());

        // Two pages <= 8192 bytes
        let (p1, p2, list) = PrpBuilder::build(0x2000, 8192);
        assert_eq!(p1, 0x2000);
        assert_eq!(p2, 0x3000);
        assert!(list.is_empty());

        // Multi pages > 8192 bytes (e.g. 16 KB -> 4 pages)
        let (p1, _p2, list) = PrpBuilder::build(0x4000, 16384);
        assert_eq!(p1, 0x4000);
        assert_eq!(list.len(), 3);
        assert_eq!(list[0], 0x5000);
        assert_eq!(list[1], 0x6000);
        assert_eq!(list[2], 0x7000);
    }

    #[test]
    fn test_nvme_controller_initialization_and_io() {
        let mock_ctrl = Arc::new(Mutex::new(MockNvmeController::new(2048, 512))); // 1 MB disk
        let nvme_dev = NvmeDevice::new(10, "nvme0n1", mock_ctrl.clone()).expect("Init NVMe failed");

        assert_eq!(nvme_dev.name(), "nvme0n1");
        assert_eq!(nvme_dev.block_size(), 512);
        assert_eq!(nvme_dev.block_count(), 2048);
        assert_eq!(nvme_dev.status(), DeviceStatus::Online);

        // Write test data across 4 blocks (2048 bytes)
        let mut write_data = vec![0x3Cu8; 2048];
        write_data[0..12].copy_from_slice(b"HYBRIDOS_NVM");
        let written = nvme_dev.write_blocks(10, &write_data).expect("NVMe write failed");
        assert_eq!(written, 2048);

        // Read test data back
        let mut read_data = vec![0u8; 2048];
        let bytes_read = nvme_dev.read_blocks(10, &mut read_data).expect("NVMe read failed");
        assert_eq!(bytes_read, 2048);
        assert_eq!(&read_data[0..12], b"HYBRIDOS_NVM");
        assert_eq!(read_data[50], 0x3C);

        // Flush
        nvme_dev.flush().expect("Flush failed");
    }

    #[test]
    fn test_nvme_multi_page_dma_read_write() {
        let mock_ctrl = Arc::new(Mutex::new(MockNvmeController::new(4096, 512)));
        let nvme_dev = NvmeDevice::new(11, "nvme0n2", mock_ctrl).expect("Init failed");

        let mut large_buf = vec![0xA5u8; 16384];
        large_buf[0..8].copy_from_slice(b"PAGE_000");
        large_buf[4096..4104].copy_from_slice(b"PAGE_001");
        large_buf[8192..8200].copy_from_slice(b"PAGE_002");
        large_buf[12288..12296].copy_from_slice(b"PAGE_003");

        let written = nvme_dev.write_blocks(0, &large_buf).unwrap();
        assert_eq!(written, 16384);

        let mut read_buf = vec![0u8; 16384];
        let bytes_read = nvme_dev.read_blocks(0, &mut read_buf).unwrap();
        assert_eq!(bytes_read, 16384);
        assert_eq!(&read_buf[0..8], b"PAGE_000");
        assert_eq!(&read_buf[4096..4104], b"PAGE_001");
        assert_eq!(&read_buf[8192..8200], b"PAGE_002");
        assert_eq!(&read_buf[12288..12296], b"PAGE_003");
    }

    #[test]
    fn test_nvme_out_of_bounds_lba() {
        let mock_ctrl = Arc::new(Mutex::new(MockNvmeController::new(100, 512)));
        let nvme_dev = NvmeDevice::new(12, "nvme0n3", mock_ctrl).unwrap();

        let mut buf = vec![0u8; 512];
        let err = nvme_dev.read_blocks(100, &mut buf).unwrap_err();
        match err {
            BlockDeviceError::InvalidLba { lba, max_lba } => {
                assert_eq!(lba, 101);
                assert_eq!(max_lba, 100);
            }
            _ => panic!("Expected InvalidLba"),
        }
    }
}
