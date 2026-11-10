//! VirtIO-GPU 2D/3D Hardware Device Specification & Driver Subsystem.
//!
//! Conforms to the OASIS VirtIO Specification (v1.1 / v1.2, Section 5.7):
//! - VirtIO GPU PCI Architecture (Vendor ID: 0x1AF4, Device ID: 0x1050)
//! - Virtqueue ring buffer execution (`controlq` command queue)
//! - Required Command dispatching:
//!   - `VIRTIO_GPU_CMD_GET_DISPLAY_INFO` (0x0100)
//!   - `VIRTIO_GPU_CMD_RESOURCE_CREATE_2D` (0x0101)
//!   - `VIRTIO_GPU_CMD_RESOURCE_ATTACH_BACKING` (0x0106)
//!   - `VIRTIO_GPU_CMD_SET_SCANOUT` (0x0103)
//!   - `VIRTIO_GPU_CMD_TRANSFER_TO_HOST_2D` (0x0105)
//!   - `VIRTIO_GPU_CMD_RESOURCE_FLUSH` (0x0104)
//! - Host scanout simulation, 2D blitting, and multi-monitor display heads.

use std::collections::HashMap;

// ============================================================================
// VirtIO GPU Constants and Enums
// ============================================================================

pub const VIRTIO_GPU_VENDOR_ID: u16 = 0x1AF4;
pub const VIRTIO_GPU_DEVICE_ID: u16 = 0x1050; // PCI Subsystem Device ID for Modern VirtIO GPU
pub const VIRTIO_GPU_MAX_SCANOUTS: usize = 16;

// Command types
pub const VIRTIO_GPU_CMD_GET_DISPLAY_INFO: u32 = 0x0100;
pub const VIRTIO_GPU_CMD_RESOURCE_CREATE_2D: u32 = 0x0101;
pub const VIRTIO_GPU_CMD_RESOURCE_UNREF: u32 = 0x0102;
pub const VIRTIO_GPU_CMD_SET_SCANOUT: u32 = 0x0103;
pub const VIRTIO_GPU_CMD_RESOURCE_FLUSH: u32 = 0x0104;
pub const VIRTIO_GPU_CMD_TRANSFER_TO_HOST_2D: u32 = 0x0105;
pub const VIRTIO_GPU_CMD_RESOURCE_ATTACH_BACKING: u32 = 0x0106;
pub const VIRTIO_GPU_CMD_RESOURCE_DETACH_BACKING: u32 = 0x0107;
pub const VIRTIO_GPU_CMD_GET_CAPSET_INFO: u32 = 0x0108;
pub const VIRTIO_GPU_CMD_GET_CAPSET: u32 = 0x0109;
pub const VIRTIO_GPU_CMD_GET_EDID: u32 = 0x010A;
pub const VIRTIO_GPU_CMD_UPDATE_CURSOR: u32 = 0x010B;
pub const VIRTIO_GPU_CMD_MOVE_CURSOR: u32 = 0x010C;

// Response types
pub const VIRTIO_GPU_RESP_OK_NODATA: u32 = 0x1100;
pub const VIRTIO_GPU_RESP_OK_DISPLAY_INFO: u32 = 0x1101;
pub const VIRTIO_GPU_RESP_OK_CAPSET_INFO: u32 = 0x1102;
pub const VIRTIO_GPU_RESP_OK_CAPSET: u32 = 0x1103;
pub const VIRTIO_GPU_RESP_OK_EDID: u32 = 0x1104;

pub const VIRTIO_GPU_RESP_ERR_UNSPEC: u32 = 0x1200;
pub const VIRTIO_GPU_RESP_ERR_OUT_OF_MEMORY: u32 = 0x1201;
pub const VIRTIO_GPU_RESP_ERR_INVALID_SCANOUT_ID: u32 = 0x1202;
pub const VIRTIO_GPU_RESP_ERR_INVALID_RESOURCE_ID: u32 = 0x1203;
pub const VIRTIO_GPU_RESP_ERR_INVALID_CONTEXT_ID: u32 = 0x1204;
pub const VIRTIO_GPU_RESP_ERR_INVALID_PARAMETER: u32 = 0x1205;

// Formats (Section 5.7.6.8)
pub const VIRTIO_GPU_FORMAT_B8G8R8A8_UNORM: u32 = 1;
pub const VIRTIO_GPU_FORMAT_B8G8R8X8_UNORM: u32 = 2;
pub const VIRTIO_GPU_FORMAT_A8R8G8B8_UNORM: u32 = 3;
pub const VIRTIO_GPU_FORMAT_X8R8G8B8_UNORM: u32 = 4;
pub const VIRTIO_GPU_FORMAT_R8G8B8A8_UNORM: u32 = 67;
pub const VIRTIO_GPU_FORMAT_X8B8G8R8_UNORM: u32 = 68;

pub const VIRTIO_GPU_FLAG_FENCE: u32 = 1 << 0;

// ============================================================================
// VirtIO GPU Data Structures
// ============================================================================

/// VirtIO GPU Control Header (matching `struct virtio_gpu_ctrl_hdr`)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct VirtioGpuCtrlHdr {
    pub hdr_type: u32,
    pub flags: u32,
    pub fence_id: u64,
    pub ctx_id: u32,
    pub ring_idx: u8,
    pub padding: [u8; 3],
}

impl VirtioGpuCtrlHdr {
    pub fn new(hdr_type: u32) -> Self {
        Self {
            hdr_type,
            flags: 0,
            fence_id: 0,
            ctx_id: 0,
            ring_idx: 0,
            padding: [0; 3],
        }
    }

    pub fn with_fence(hdr_type: u32, fence_id: u64) -> Self {
        Self {
            hdr_type,
            flags: VIRTIO_GPU_FLAG_FENCE,
            fence_id,
            ctx_id: 0,
            ring_idx: 0,
            padding: [0; 3],
        }
    }
}

/// VirtIO GPU 2D Rectangle (matching `struct virtio_gpu_rect`)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct VirtioGpuRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl VirtioGpuRect {
    pub fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self { x, y, width, height }
    }
}

/// Memory entry representing a guest physical memory page range
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct VirtioGpuMemEntry {
    pub addr: u64,
    pub length: u32,
    pub padding: u32,
}

/// Display Mode configuration for a single scanout head
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct VirtioGpuDisplayOne {
    pub rect: VirtioGpuRect,
    pub enabled: u32,
    pub flags: u32,
}

/// Command: VIRTIO_GPU_CMD_RESOURCE_CREATE_2D
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VirtioGpuResourceCreate2d {
    pub hdr: VirtioGpuCtrlHdr,
    pub resource_id: u32,
    pub format: u32,
    pub width: u32,
    pub height: u32,
}

/// Command: VIRTIO_GPU_CMD_RESOURCE_ATTACH_BACKING
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VirtioGpuResourceAttachBacking {
    pub hdr: VirtioGpuCtrlHdr,
    pub resource_id: u32,
    pub nr_entries: u32,
    pub entries: Vec<VirtioGpuMemEntry>,
}

/// Command: VIRTIO_GPU_CMD_SET_SCANOUT
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VirtioGpuSetScanout {
    pub hdr: VirtioGpuCtrlHdr,
    pub rect: VirtioGpuRect,
    pub scanout_id: u32,
    pub resource_id: u32,
}

/// Command: VIRTIO_GPU_CMD_TRANSFER_TO_HOST_2D
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VirtioGpuTransferToHost2d {
    pub hdr: VirtioGpuCtrlHdr,
    pub rect: VirtioGpuRect,
    pub offset: u64,
    pub resource_id: u32,
    pub padding: u32,
    pub pixel_data: Vec<u8>,
}

/// Command: VIRTIO_GPU_CMD_RESOURCE_FLUSH
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VirtioGpuResourceFlush {
    pub hdr: VirtioGpuCtrlHdr,
    pub rect: VirtioGpuRect,
    pub resource_id: u32,
    pub padding: u32,
}

/// Response: VIRTIO_GPU_RESP_OK_DISPLAY_INFO
#[derive(Debug, Clone)]
pub struct VirtioGpuRespDisplayInfo {
    pub hdr: VirtioGpuCtrlHdr,
    pub pmodes: [VirtioGpuDisplayOne; VIRTIO_GPU_MAX_SCANOUTS],
}

// ============================================================================
// VirtIO GPU Host State and Device Engine
// ============================================================================

/// Host-side representation of a VirtIO GPU resource
#[derive(Debug, Clone)]
pub struct VirtioGpuHostResource {
    pub resource_id: u32,
    pub format: u32,
    pub width: u32,
    pub height: u32,
    pub backing_entries: Vec<VirtioGpuMemEntry>,
    /// Host buffer allocated for host-side pixel storage
    pub host_buffer: Vec<u8>,
    pub dirty: bool,
    pub dirty_rect: Option<VirtioGpuRect>,
}

impl VirtioGpuHostResource {
    pub fn new(resource_id: u32, format: u32, width: u32, height: u32) -> Self {
        let bpp = 4; // 32 bits per pixel standard for 2D formats
        let size = (width as usize) * (height as usize) * bpp;
        Self {
            resource_id,
            format,
            width,
            height,
            backing_entries: Vec::new(),
            host_buffer: vec![0u8; size],
            dirty: false,
            dirty_rect: None,
        }
    }
}

/// Host-side representation of a VirtIO GPU display scanout head
#[derive(Debug, Clone)]
pub struct VirtioGpuScanoutHead {
    pub scanout_id: u32,
    pub enabled: bool,
    pub rect: VirtioGpuRect,
    pub resource_id: Option<u32>,
    pub display_framebuffer: Vec<u8>,
}

impl VirtioGpuScanoutHead {
    pub fn new(scanout_id: u32, width: u32, height: u32) -> Self {
        let size = (width as usize) * (height as usize) * 4;
        Self {
            scanout_id,
            enabled: false,
            rect: VirtioGpuRect::new(0, 0, width, height),
            resource_id: None,
            display_framebuffer: vec![0u8; size],
        }
    }
}

/// VirtIO-GPU Hardware Device Simulation
#[derive(Debug)]
pub struct VirtioGpuDevice {
    pub resources: HashMap<u32, VirtioGpuHostResource>,
    pub scanouts: [VirtioGpuScanoutHead; VIRTIO_GPU_MAX_SCANOUTS],
    pub last_completed_fence: u64,
    pub total_flushes: u64,
}

impl VirtioGpuDevice {
    /// Create a new VirtIO-GPU hardware device with scanout 0 configured for 1920x1080
    pub fn new() -> Self {
        let mut scanouts = [
            VirtioGpuScanoutHead::new(0, 1920, 1080),
            VirtioGpuScanoutHead::new(1, 1920, 1080),
            VirtioGpuScanoutHead::new(2, 1920, 1080),
            VirtioGpuScanoutHead::new(3, 1920, 1080),
            VirtioGpuScanoutHead::new(4, 1920, 1080),
            VirtioGpuScanoutHead::new(5, 1920, 1080),
            VirtioGpuScanoutHead::new(6, 1920, 1080),
            VirtioGpuScanoutHead::new(7, 1920, 1080),
            VirtioGpuScanoutHead::new(8, 1920, 1080),
            VirtioGpuScanoutHead::new(9, 1920, 1080),
            VirtioGpuScanoutHead::new(10, 1920, 1080),
            VirtioGpuScanoutHead::new(11, 1920, 1080),
            VirtioGpuScanoutHead::new(12, 1920, 1080),
            VirtioGpuScanoutHead::new(13, 1920, 1080),
            VirtioGpuScanoutHead::new(14, 1920, 1080),
            VirtioGpuScanoutHead::new(15, 1920, 1080),
        ];

        // Head 0 is enabled by default
        scanouts[0].enabled = true;

        Self {
            resources: HashMap::new(),
            scanouts,
            last_completed_fence: 0,
            total_flushes: 0,
        }
    }

    /// Process: VIRTIO_GPU_CMD_GET_DISPLAY_INFO
    pub fn cmd_get_display_info(&mut self, hdr: &VirtioGpuCtrlHdr) -> VirtioGpuRespDisplayInfo {
        let mut pmodes = [VirtioGpuDisplayOne {
            rect: VirtioGpuRect::new(0, 0, 0, 0),
            enabled: 0,
            flags: 0,
        }; VIRTIO_GPU_MAX_SCANOUTS];

        for (i, scanout) in self.scanouts.iter().enumerate() {
            pmodes[i] = VirtioGpuDisplayOne {
                rect: scanout.rect,
                enabled: if scanout.enabled { 1 } else { 0 },
                flags: 0,
            };
        }

        self.complete_fence(hdr);

        VirtioGpuRespDisplayInfo {
            hdr: VirtioGpuCtrlHdr::new(VIRTIO_GPU_RESP_OK_DISPLAY_INFO),
            pmodes,
        }
    }

    /// Process: VIRTIO_GPU_CMD_RESOURCE_CREATE_2D
    pub fn cmd_resource_create_2d(
        &mut self,
        cmd: &VirtioGpuResourceCreate2d,
    ) -> VirtioGpuCtrlHdr {
        if cmd.resource_id == 0 {
            return VirtioGpuCtrlHdr::new(VIRTIO_GPU_RESP_ERR_INVALID_RESOURCE_ID);
        }
        if self.resources.contains_key(&cmd.resource_id) {
            return VirtioGpuCtrlHdr::new(VIRTIO_GPU_RESP_ERR_INVALID_PARAMETER);
        }
        if cmd.width == 0 || cmd.height == 0 {
            return VirtioGpuCtrlHdr::new(VIRTIO_GPU_RESP_ERR_INVALID_PARAMETER);
        }

        let res = VirtioGpuHostResource::new(cmd.resource_id, cmd.format, cmd.width, cmd.height);
        self.resources.insert(cmd.resource_id, res);

        self.complete_fence(&cmd.hdr);
        VirtioGpuCtrlHdr::new(VIRTIO_GPU_RESP_OK_NODATA)
    }

    /// Process: VIRTIO_GPU_CMD_RESOURCE_ATTACH_BACKING
    pub fn cmd_resource_attach_backing(
        &mut self,
        cmd: &VirtioGpuResourceAttachBacking,
    ) -> VirtioGpuCtrlHdr {
        let res = match self.resources.get_mut(&cmd.resource_id) {
            Some(r) => r,
            None => return VirtioGpuCtrlHdr::new(VIRTIO_GPU_RESP_ERR_INVALID_RESOURCE_ID),
        };

        res.backing_entries = cmd.entries.clone();
        self.complete_fence(&cmd.hdr);
        VirtioGpuCtrlHdr::new(VIRTIO_GPU_RESP_OK_NODATA)
    }

    /// Process: VIRTIO_GPU_CMD_SET_SCANOUT
    pub fn cmd_set_scanout(&mut self, cmd: &VirtioGpuSetScanout) -> VirtioGpuCtrlHdr {
        if cmd.scanout_id as usize >= VIRTIO_GPU_MAX_SCANOUTS {
            return VirtioGpuCtrlHdr::new(VIRTIO_GPU_RESP_ERR_INVALID_SCANOUT_ID);
        }

        if cmd.resource_id == 0 {
            // Disable scanout
            let scanout = &mut self.scanouts[cmd.scanout_id as usize];
            scanout.enabled = false;
            scanout.resource_id = None;
            self.complete_fence(&cmd.hdr);
            return VirtioGpuCtrlHdr::new(VIRTIO_GPU_RESP_OK_NODATA);
        }

        if !self.resources.contains_key(&cmd.resource_id) {
            return VirtioGpuCtrlHdr::new(VIRTIO_GPU_RESP_ERR_INVALID_RESOURCE_ID);
        }

        let scanout = &mut self.scanouts[cmd.scanout_id as usize];
        scanout.enabled = true;
        scanout.rect = cmd.rect;
        scanout.resource_id = Some(cmd.resource_id);

        self.complete_fence(&cmd.hdr);
        VirtioGpuCtrlHdr::new(VIRTIO_GPU_RESP_OK_NODATA)
    }

    /// Process: VIRTIO_GPU_CMD_TRANSFER_TO_HOST_2D
    pub fn cmd_transfer_to_host_2d(
        &mut self,
        cmd: &VirtioGpuTransferToHost2d,
    ) -> VirtioGpuCtrlHdr {
        let res = match self.resources.get_mut(&cmd.resource_id) {
            Some(r) => r,
            None => return VirtioGpuCtrlHdr::new(VIRTIO_GPU_RESP_ERR_INVALID_RESOURCE_ID),
        };

        let rect = cmd.rect;
        if rect.x + rect.width > res.width || rect.y + rect.height > res.height {
            return VirtioGpuCtrlHdr::new(VIRTIO_GPU_RESP_ERR_INVALID_PARAMETER);
        }

        // Copy source pixels into host resource buffer
        let bpp = 4;
        let stride = res.width as usize * bpp;

        if !cmd.pixel_data.is_empty() {
            let mut src_offset = 0;
            for row in 0..rect.height {
                let dst_y = (rect.y + row) as usize;
                let dst_x = rect.x as usize;
                let dst_offset = dst_y * stride + dst_x * bpp;
                let row_bytes = rect.width as usize * bpp;

                if src_offset + row_bytes <= cmd.pixel_data.len()
                    && dst_offset + row_bytes <= res.host_buffer.len()
                {
                    res.host_buffer[dst_offset..dst_offset + row_bytes]
                        .copy_from_slice(&cmd.pixel_data[src_offset..src_offset + row_bytes]);
                }
                src_offset += row_bytes;
            }
        }

        res.dirty = true;
        res.dirty_rect = Some(rect);

        self.complete_fence(&cmd.hdr);
        VirtioGpuCtrlHdr::new(VIRTIO_GPU_RESP_OK_NODATA)
    }

    /// Process: VIRTIO_GPU_CMD_RESOURCE_FLUSH
    pub fn cmd_resource_flush(&mut self, cmd: &VirtioGpuResourceFlush) -> VirtioGpuCtrlHdr {
        let res = match self.resources.get_mut(&cmd.resource_id) {
            Some(r) => r,
            None => return VirtioGpuCtrlHdr::new(VIRTIO_GPU_RESP_ERR_INVALID_RESOURCE_ID),
        };

        // Transfer flushed dirty pixels to any scanout bound to this resource
        for scanout in self.scanouts.iter_mut() {
            if scanout.enabled && scanout.resource_id == Some(cmd.resource_id) {
                let copy_len = res.host_buffer.len().min(scanout.display_framebuffer.len());
                scanout.display_framebuffer[..copy_len]
                    .copy_from_slice(&res.host_buffer[..copy_len]);
            }
        }

        res.dirty = false;
        res.dirty_rect = None;
        self.total_flushes += 1;

        self.complete_fence(&cmd.hdr);
        VirtioGpuCtrlHdr::new(VIRTIO_GPU_RESP_OK_NODATA)
    }

    /// Inspect pixel on active host display scanout
    pub fn read_scanout_pixel(&self, scanout_id: usize, x: u32, y: u32) -> Option<u32> {
        if scanout_id >= VIRTIO_GPU_MAX_SCANOUTS {
            return None;
        }
        let scanout = &self.scanouts[scanout_id];
        if !scanout.enabled || x >= scanout.rect.width || y >= scanout.rect.height {
            return None;
        }

        let stride = (scanout.rect.width * 4) as usize;
        let offset = (y as usize * stride) + (x as usize * 4);
        if offset + 4 <= scanout.display_framebuffer.len() {
            let b0 = scanout.display_framebuffer[offset] as u32;
            let b1 = scanout.display_framebuffer[offset + 1] as u32;
            let b2 = scanout.display_framebuffer[offset + 2] as u32;
            let b3 = scanout.display_framebuffer[offset + 3] as u32;
            Some(b0 | (b1 << 8) | (b2 << 16) | (b3 << 24))
        } else {
            None
        }
    }

    fn complete_fence(&mut self, hdr: &VirtioGpuCtrlHdr) {
        if (hdr.flags & VIRTIO_GPU_FLAG_FENCE) != 0 {
            if hdr.fence_id > self.last_completed_fence {
                self.last_completed_fence = hdr.fence_id;
            }
        }
    }
}

// ============================================================================
// VirtIO GPU Command Ring Buffer Queue Simulator
// ============================================================================

/// Virtqueue command packet submitted by the kernel driver to the hardware ring buffer
#[derive(Debug, Clone)]
pub enum VirtioGpuCommand {
    GetDisplayInfo(VirtioGpuCtrlHdr),
    ResourceCreate2d(VirtioGpuResourceCreate2d),
    ResourceAttachBacking(VirtioGpuResourceAttachBacking),
    SetScanout(VirtioGpuSetScanout),
    TransferToHost2d(VirtioGpuTransferToHost2d),
    ResourceFlush(VirtioGpuResourceFlush),
}

/// Virtqueue ring buffer executor
#[derive(Debug)]
pub struct VirtioGpuRingBuffer {
    pub device: VirtioGpuDevice,
}

impl VirtioGpuRingBuffer {
    pub fn new() -> Self {
        Self {
            device: VirtioGpuDevice::new(),
        }
    }

    /// Submit a command to the VirtIO GPU control virtqueue
    pub fn submit_command(&mut self, cmd: VirtioGpuCommand) -> VirtioGpuCtrlHdr {
        match cmd {
            VirtioGpuCommand::GetDisplayInfo(hdr) => {
                let resp = self.device.cmd_get_display_info(&hdr);
                resp.hdr
            }
            VirtioGpuCommand::ResourceCreate2d(cmd) => self.device.cmd_resource_create_2d(&cmd),
            VirtioGpuCommand::ResourceAttachBacking(cmd) => {
                self.device.cmd_resource_attach_backing(&cmd)
            }
            VirtioGpuCommand::SetScanout(cmd) => self.device.cmd_set_scanout(&cmd),
            VirtioGpuCommand::TransferToHost2d(cmd) => self.device.cmd_transfer_to_host_2d(&cmd),
            VirtioGpuCommand::ResourceFlush(cmd) => self.device.cmd_resource_flush(&cmd),
        }
    }
}
