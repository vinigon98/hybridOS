//! Direct Rendering Manager (DRM) Core Subsystem for HybridOS.
//!
//! Provides the complete Linux DRM driver core:
//! - DRM Device Minor types: Primary (`/dev/dri/card0`) and Render node (`/dev/dri/renderD128`)
//! - GEM (Graphics Execution Manager) buffer allocator:
//!   - Dumb buffer creation and destruction
//!   - Stride/pitch calculations with cache line alignment
//!   - Pseudo mmap offset assignment (`mmap_offset`)
//! - DRM IOCTL dispatch table:
//!   - `DRM_IOCTL_VERSION`
//!   - `DRM_IOCTL_GET_UNIQUE`
//!   - `DRM_IOCTL_MODE_GETRESOURCES`
//!   - `DRM_IOCTL_MODE_GETCRTC`
//!   - `DRM_IOCTL_MODE_SETCRTC`
//!   - `DRM_IOCTL_MODE_ADDFB`
//!   - `DRM_IOCTL_MODE_RMFB`
//!   - `DRM_IOCTL_MODE_PAGE_FLIP`
//!   - `DRM_IOCTL_GEM_CLOSE`
//!   - `DRM_IOCTL_MODE_CREATE_DUMB`, `DRM_IOCTL_MODE_MAP_DUMB`, `DRM_IOCTL_MODE_DESTROY_DUMB`

#[allow(unused_imports)]
use alloc::string::{String, ToString};
#[allow(unused_imports)]
use alloc::vec::Vec;
#[allow(unused_imports)]
use alloc::boxed::Box;
#[allow(unused_imports)]
use crate::compat::FloatExt;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use super::kms::{KmsSubsystem, KmsError, DisplayMode};

// ============================================================================
// DRM Constants and Major/Minor Definitions
// ============================================================================

/// DRM Major device number in Linux
pub const DRM_MAJOR: u32 = 226;

/// DRM Minor Types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrmMinorType {
    /// Primary node (`/dev/dri/cardX`): Full KMS mode setting, master operations, rendering
    Primary,
    /// Render node (`/dev/dri/renderDX`): Client-only unprivileged offscreen rendering and compute
    Render,
    /// Legacy control node (`/dev/dri/controlDX`)
    Control,
}

impl DrmMinorType {
    pub fn base_minor(&self) -> u32 {
        match self {
            DrmMinorType::Primary => 0,
            DrmMinorType::Control => 64,
            DrmMinorType::Render => 128,
        }
    }
}

/// DRM Device Minor Representation
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrmMinor {
    pub minor_type: DrmMinorType,
    pub index: u32,
    pub major: u32,
    pub minor: u32,
}

impl DrmMinor {
    pub fn new(minor_type: DrmMinorType, index: u32) -> Self {
        let minor = minor_type.base_minor() + index;
        Self {
            minor_type,
            index,
            major: DRM_MAJOR,
            minor,
        }
    }

    /// Primary card node path (`/dev/dri/card0`)
    pub fn dev_path(&self) -> String {
        match self.minor_type {
            DrmMinorType::Primary => format!("/dev/dri/card{}", self.index),
            DrmMinorType::Control => format!("/dev/dri/controlD{}", 64 + self.index),
            DrmMinorType::Render => format!("/dev/dri/renderD{}", 128 + self.index),
        }
    }

    /// Whether this node is permitted to perform KMS modesetting operations
    pub fn allows_modesetting(&self) -> bool {
        matches!(self.minor_type, DrmMinorType::Primary)
    }

    /// Whether this node is permitted to perform rendering/compute operations
    pub fn allows_rendering(&self) -> bool {
        true
    }
}

// ============================================================================
// GEM (Graphics Execution Manager) Allocator
// ============================================================================

/// GEM Buffer Object
#[derive(Debug, Clone)]
pub struct GemObject {
    pub handle: u32,
    pub size: usize,
    pub width: u32,
    pub height: u32,
    pub bpp: u32,
    pub pitch: u32,
    pub mmap_offset: u64,
    pub ref_count: usize,
    pub memory: Vec<u8>,
}

impl GemObject {
    /// Read pixel color at coordinate (x, y) assuming 32bpp XRGB
    pub fn read_pixel_xrgb(&self, x: u32, y: u32) -> Option<u32> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let offset = (y as usize * self.pitch as usize) + (x as usize * 4);
        if offset + 4 <= self.memory.len() {
            let b0 = self.memory[offset] as u32;
            let b1 = self.memory[offset + 1] as u32;
            let b2 = self.memory[offset + 2] as u32;
            let b3 = self.memory[offset + 3] as u32;
            Some(b0 | (b1 << 8) | (b2 << 16) | (b3 << 24))
        } else {
            None
        }
    }

    /// Write pixel color at coordinate (x, y) assuming 32bpp XRGB
    pub fn write_pixel_xrgb(&mut self, x: u32, y: u32, color: u32) -> bool {
        if x >= self.width || y >= self.height {
            return false;
        }
        let offset = (y as usize * self.pitch as usize) + (x as usize * 4);
        if offset + 4 <= self.memory.len() {
            self.memory[offset] = (color & 0xFF) as u8;
            self.memory[offset + 1] = ((color >> 8) & 0xFF) as u8;
            self.memory[offset + 2] = ((color >> 16) & 0xFF) as u8;
            self.memory[offset + 3] = ((color >> 24) & 0xFF) as u8;
            true
        } else {
            false
        }
    }
}

/// Calculate pitch (stride) aligned to 64 bytes
pub fn calculate_pitch(width: u32, bpp: u32) -> u32 {
    let bytes_per_pixel = (bpp + 7) / 8;
    let unaligned_pitch = width * bytes_per_pixel;
    let align = 64;
    ((unaligned_pitch + align - 1) / align) * align
}

/// Calculate buffer allocation size aligned to 4096-byte page boundaries
pub fn calculate_aligned_size(pitch: u32, height: u32) -> usize {
    let raw_size = (pitch as usize) * (height as usize);
    let page_size = 4096;
    ((raw_size + page_size - 1) / page_size) * page_size
}

/// GEM Allocator responsible for allocating dumb buffers and managing handle lifecycles
#[derive(Debug)]
pub struct GemAllocator {
    objects: HashMap<u32, Arc<Mutex<GemObject>>>,
    offset_to_handle: HashMap<u64, u32>,
    next_handle: u32,
    next_mmap_offset: u64,
}

impl GemAllocator {
    pub fn new() -> Self {
        Self {
            objects: HashMap::new(),
            offset_to_handle: HashMap::new(),
            next_handle: 1, // DRM handle 0 is reserved as invalid
            next_mmap_offset: 0x7000_0000_0000, // Safe 64-bit high address window for DRM mmap
        }
    }

    /// Allocate a dumb buffer with the requested width, height, and bits per pixel
    pub fn allocate_dumb(
        &mut self,
        width: u32,
        height: u32,
        bpp: u32,
    ) -> Result<Arc<Mutex<GemObject>>, DrmError> {
        if width == 0 || height == 0 || bpp == 0 {
            return Err(DrmError::InvalidArgument);
        }

        let pitch = calculate_pitch(width, bpp);
        let size = calculate_aligned_size(pitch, height);
        let handle = self.next_handle;
        self.next_handle += 1;

        let mmap_offset = self.next_mmap_offset;
        self.next_mmap_offset += 0x1000_0000; // 256MB boundary spacing per buffer

        let gem_obj = GemObject {
            handle,
            size,
            width,
            height,
            bpp,
            pitch,
            mmap_offset,
            ref_count: 1,
            memory: vec![0u8; size],
        };

        let arc_obj = Arc::new(Mutex::new(gem_obj));
        self.objects.insert(handle, arc_obj.clone());
        self.offset_to_handle.insert(mmap_offset, handle);

        Ok(arc_obj)
    }

    /// Retrieve an existing GEM object by handle
    pub fn get_object(&self, handle: u32) -> Option<Arc<Mutex<GemObject>>> {
        self.objects.get(&handle).cloned()
    }

    /// Look up handle by mmap pseudo offset
    pub fn lookup_by_offset(&self, offset: u64) -> Option<Arc<Mutex<GemObject>>> {
        self.offset_to_handle
            .get(&offset)
            .and_then(|handle| self.objects.get(handle).cloned())
    }

    /// Release or close a GEM handle
    pub fn close_handle(&mut self, handle: u32) -> Result<(), DrmError> {
        if let Some(obj) = self.objects.remove(&handle) {
            let offset = obj.lock().unwrap().mmap_offset;
            self.offset_to_handle.remove(&offset);
            Ok(())
        } else {
            Err(DrmError::InvalidHandle(handle))
        }
    }
}

// ============================================================================
// DRM IOCTL Codes and Request Structures
// ============================================================================

pub const DRM_IOCTL_VERSION: u32 = 0xC0406400;
pub const DRM_IOCTL_GET_UNIQUE: u32 = 0xC0106401;
pub const DRM_IOCTL_GEM_CLOSE: u32 = 0x40086409;
pub const DRM_IOCTL_MODE_GETRESOURCES: u32 = 0xC04064A0;
pub const DRM_IOCTL_MODE_GETCRTC: u32 = 0xC06864A1;
pub const DRM_IOCTL_MODE_SETCRTC: u32 = 0xC06864A2;
pub const DRM_IOCTL_MODE_ADDFB: u32 = 0xC01C64AE;
pub const DRM_IOCTL_MODE_RMFB: u32 = 0xC00464AF;
pub const DRM_IOCTL_MODE_PAGE_FLIP: u32 = 0xC01864B0;
pub const DRM_IOCTL_MODE_CREATE_DUMB: u32 = 0xC02064B2;
pub const DRM_IOCTL_MODE_MAP_DUMB: u32 = 0xC01064B3;
pub const DRM_IOCTL_MODE_DESTROY_DUMB: u32 = 0xC00864B4;

/// Driver version descriptor
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrmVersion {
    pub version_major: i32,
    pub version_minor: i32,
    pub version_patchlevel: i32,
    pub name: String,
    pub date: String,
    pub desc: String,
}

/// GEM handle close command
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DrmGemClose {
    pub handle: u32,
}

/// DRM dumb buffer creation request
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrmModeCreateDumb {
    pub height: u32,
    pub width: u32,
    pub bpp: u32,
    pub flags: u32,
    pub handle: u32,
    pub pitch: u32,
    pub size: u64,
}

/// DRM dumb buffer map request (retrieves fake mmap offset)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DrmModeMapDumb {
    pub handle: u32,
    pub pad: u32,
    pub offset: u64,
}

/// DRM dumb buffer destruction request
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DrmModeDestroyDumb {
    pub handle: u32,
}

/// DRM KMS resources summary
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrmModeCardRes {
    pub fb_ids: Vec<u32>,
    pub crtc_ids: Vec<u32>,
    pub connector_ids: Vec<u32>,
    pub encoder_ids: Vec<u32>,
    pub min_width: u32,
    pub max_width: u32,
    pub min_height: u32,
    pub max_height: u32,
}

/// DRM CRTC inspection/configuration request
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrmModeCrtc {
    pub crtc_id: u32,
    pub fb_id: u32,
    pub x: u32,
    pub y: u32,
    pub gamma_size: u32,
    pub mode_valid: u32,
    pub mode: Option<DisplayMode>,
}

/// DRM Framebuffer addition request
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrmModeFbCmd {
    pub fb_id: u32,
    pub width: u32,
    pub height: u32,
    pub pitch: u32,
    pub bpp: u32,
    pub depth: u32,
    pub handle: u32,
}

/// DRM CRTC page flip request
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DrmModeCrtcPageFlip {
    pub crtc_id: u32,
    pub fb_id: u32,
    pub flags: u32,
    pub reserved: u32,
    pub user_data: u64,
}

// ============================================================================
// DRM Errors and Device Architecture
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DrmError {
    PermissionDenied,
    InvalidArgument,
    InvalidHandle(u32),
    InvalidIoctl(u32),
    NoMemory,
    KmsFailure(String),
}

impl From<KmsError> for DrmError {
    fn from(e: KmsError) -> Self {
        DrmError::KmsFailure(format!("{:?}", e))
    }
}

/// Direct Rendering Manager (DRM) Core Device Instance
#[derive(Debug)]
pub struct DrmDevice {
    pub driver_name: String,
    pub driver_date: String,
    pub driver_desc: String,
    pub primary_minor: DrmMinor,
    pub render_minor: DrmMinor,
    pub gem_allocator: GemAllocator,
    pub kms: KmsSubsystem,
}

impl DrmDevice {
    /// Create a new DRM device instance exposing `/dev/dri/card0` and `/dev/dri/renderD128`
    pub fn new() -> Self {
        Self {
            driver_name: "hybridos_drm".to_string(),
            driver_date: "20261109".to_string(),
            driver_desc: "HybridOS Heterogeneous DRM/KMS Acceleration Driver".to_string(),
            primary_minor: DrmMinor::new(DrmMinorType::Primary, 0),
            render_minor: DrmMinor::new(DrmMinorType::Render, 0),
            gem_allocator: GemAllocator::new(),
            kms: KmsSubsystem::new(),
        }
    }

    /// Retrieve driver version info (DRM_IOCTL_VERSION)
    pub fn ioctl_version(&self) -> DrmVersion {
        DrmVersion {
            version_major: 1,
            version_minor: 0,
            version_patchlevel: 0,
            name: self.driver_name.clone(),
            date: self.driver_date.clone(),
            desc: self.driver_desc.clone(),
        }
    }

    /// Retrieve unique driver bus string (DRM_IOCTL_GET_UNIQUE)
    pub fn ioctl_get_unique(&self) -> String {
        "pci:0000:00:02.0".to_string()
    }

    /// Allocate dumb buffer (DRM_IOCTL_MODE_CREATE_DUMB)
    pub fn ioctl_create_dumb(&mut self, req: &mut DrmModeCreateDumb) -> Result<(), DrmError> {
        let obj = self.gem_allocator.allocate_dumb(req.width, req.height, req.bpp)?;
        let locked = obj.lock().unwrap();
        req.handle = locked.handle;
        req.pitch = locked.pitch;
        req.size = locked.size as u64;
        Ok(())
    }

    /// Map dumb buffer (DRM_IOCTL_MODE_MAP_DUMB)
    pub fn ioctl_map_dumb(&mut self, req: &mut DrmModeMapDumb) -> Result<(), DrmError> {
        let obj = self
            .gem_allocator
            .get_object(req.handle)
            .ok_or(DrmError::InvalidHandle(req.handle))?;
        req.offset = obj.lock().unwrap().mmap_offset;
        Ok(())
    }

    /// Destroy dumb buffer (DRM_IOCTL_MODE_DESTROY_DUMB)
    pub fn ioctl_destroy_dumb(&mut self, req: &DrmModeDestroyDumb) -> Result<(), DrmError> {
        self.gem_allocator.close_handle(req.handle)
    }

    /// Close GEM handle (DRM_IOCTL_GEM_CLOSE)
    pub fn ioctl_gem_close(&mut self, req: &DrmGemClose) -> Result<(), DrmError> {
        self.gem_allocator.close_handle(req.handle)
    }

    /// Enumerate KMS resources (DRM_IOCTL_MODE_GETRESOURCES)
    pub fn ioctl_get_resources(&self, caller: DrmMinorType) -> Result<DrmModeCardRes, DrmError> {
        if !caller.base_minor() == 0 && caller != DrmMinorType::Primary {
            return Err(DrmError::PermissionDenied);
        }

        let mut fb_ids: Vec<u32> = self.kms.framebuffers.keys().copied().collect();
        let mut crtc_ids: Vec<u32> = self.kms.crtcs.keys().copied().collect();
        let mut connector_ids: Vec<u32> = self.kms.connectors.keys().copied().collect();
        let mut encoder_ids: Vec<u32> = self.kms.encoders.keys().copied().collect();

        fb_ids.sort();
        crtc_ids.sort();
        connector_ids.sort();
        encoder_ids.sort();

        Ok(DrmModeCardRes {
            fb_ids,
            crtc_ids,
            connector_ids,
            encoder_ids,
            min_width: 64,
            max_width: 8192,
            min_height: 64,
            max_height: 8192,
        })
    }

    /// Query CRTC details (DRM_IOCTL_MODE_GETCRTC)
    pub fn ioctl_get_crtc(&self, crtc_id: u32) -> Result<DrmModeCrtc, DrmError> {
        let crtc = self.kms.get_crtc(crtc_id)?;
        Ok(DrmModeCrtc {
            crtc_id: crtc.crtc_id,
            fb_id: crtc.fb_id.unwrap_or(0),
            x: crtc.x,
            y: crtc.y,
            gamma_size: crtc.gamma_size,
            mode_valid: if crtc.active_mode.is_some() { 1 } else { 0 },
            mode: crtc.active_mode.clone(),
        })
    }

    /// Set CRTC modesetting and scanout buffer (DRM_IOCTL_MODE_SETCRTC)
    pub fn ioctl_set_crtc(
        &mut self,
        req: &DrmModeCrtc,
        connectors: &[u32],
        caller: DrmMinorType,
    ) -> Result<(), DrmError> {
        if caller != DrmMinorType::Primary {
            return Err(DrmError::PermissionDenied);
        }
        let fb_id = if req.fb_id != 0 { Some(req.fb_id) } else { None };
        self.kms
            .set_crtc(req.crtc_id, fb_id, req.x, req.y, req.mode.clone(), connectors)?;
        Ok(())
    }

    /// Add a Framebuffer wrapping a GEM buffer handle (DRM_IOCTL_MODE_ADDFB)
    pub fn ioctl_add_fb(&mut self, req: &mut DrmModeFbCmd) -> Result<(), DrmError> {
        let obj_arc = self
            .gem_allocator
            .get_object(req.handle)
            .ok_or(DrmError::InvalidHandle(req.handle))?;

        let obj = obj_arc.lock().unwrap();
        if req.width > obj.width || req.height > obj.height {
            return Err(DrmError::InvalidArgument);
        }

        let fb_id = self
            .kms
            .add_framebuffer(req.width, req.height, req.pitch, req.handle)?;
        req.fb_id = fb_id;
        Ok(())
    }

    /// Remove a Framebuffer (DRM_IOCTL_MODE_RMFB)
    pub fn ioctl_rm_fb(&mut self, fb_id: u32) -> Result<(), DrmError> {
        self.kms.remove_framebuffer(fb_id)?;
        Ok(())
    }

    /// Queue a Page Flip on CRTC (DRM_IOCTL_MODE_PAGE_FLIP)
    pub fn ioctl_page_flip(
        &mut self,
        req: &DrmModeCrtcPageFlip,
        caller: DrmMinorType,
    ) -> Result<(), DrmError> {
        if caller != DrmMinorType::Primary {
            return Err(DrmError::PermissionDenied);
        }
        self.kms
            .page_flip(req.crtc_id, req.fb_id, req.flags, req.user_data)?;
        Ok(())
    }

    /// Universal IOCTL Dispatcher matching standard Linux DRM ioctl codes
    pub fn dispatch_ioctl(
        &mut self,
        cmd: u32,
        payload: &mut [u8],
        caller: DrmMinorType,
    ) -> Result<i32, DrmError> {
        match cmd {
            DRM_IOCTL_VERSION => {
                let ver = self.ioctl_version();
                // If payload is supplied, write back version numbers
                if payload.len() >= 12 {
                    payload[0..4].copy_from_slice(&ver.version_major.to_ne_bytes());
                    payload[4..8].copy_from_slice(&ver.version_minor.to_ne_bytes());
                    payload[8..12].copy_from_slice(&ver.version_patchlevel.to_ne_bytes());
                }
                Ok(0)
            }
            DRM_IOCTL_GET_UNIQUE => {
                let unique = self.ioctl_get_unique();
                let bytes = unique.as_bytes();
                let copy_len = bytes.len().min(payload.len());
                payload[..copy_len].copy_from_slice(&bytes[..copy_len]);
                Ok(0)
            }
            DRM_IOCTL_GEM_CLOSE => {
                if payload.len() < 4 {
                    return Err(DrmError::InvalidArgument);
                }
                let handle = u32::from_ne_bytes(payload[0..4].try_into().unwrap());
                self.ioctl_gem_close(&DrmGemClose { handle })?;
                Ok(0)
            }
            DRM_IOCTL_MODE_CREATE_DUMB => {
                if payload.len() < 32 {
                    return Err(DrmError::InvalidArgument);
                }
                let height = u32::from_ne_bytes(payload[0..4].try_into().unwrap());
                let width = u32::from_ne_bytes(payload[4..8].try_into().unwrap());
                let bpp = u32::from_ne_bytes(payload[8..12].try_into().unwrap());
                let flags = u32::from_ne_bytes(payload[12..16].try_into().unwrap());

                let mut req = DrmModeCreateDumb {
                    height,
                    width,
                    bpp,
                    flags,
                    handle: 0,
                    pitch: 0,
                    size: 0,
                };
                self.ioctl_create_dumb(&mut req)?;

                payload[16..20].copy_from_slice(&req.handle.to_ne_bytes());
                payload[20..24].copy_from_slice(&req.pitch.to_ne_bytes());
                payload[24..32].copy_from_slice(&req.size.to_ne_bytes());
                Ok(0)
            }
            DRM_IOCTL_MODE_MAP_DUMB => {
                if payload.len() < 16 {
                    return Err(DrmError::InvalidArgument);
                }
                let handle = u32::from_ne_bytes(payload[0..4].try_into().unwrap());
                let mut req = DrmModeMapDumb {
                    handle,
                    pad: 0,
                    offset: 0,
                };
                self.ioctl_map_dumb(&mut req)?;
                payload[8..16].copy_from_slice(&req.offset.to_ne_bytes());
                Ok(0)
            }
            DRM_IOCTL_MODE_DESTROY_DUMB => {
                if payload.len() < 4 {
                    return Err(DrmError::InvalidArgument);
                }
                let handle = u32::from_ne_bytes(payload[0..4].try_into().unwrap());
                self.ioctl_destroy_dumb(&DrmModeDestroyDumb { handle })?;
                Ok(0)
            }
            DRM_IOCTL_MODE_RMFB => {
                if payload.len() < 4 {
                    return Err(DrmError::InvalidArgument);
                }
                let fb_id = u32::from_ne_bytes(payload[0..4].try_into().unwrap());
                self.ioctl_rm_fb(fb_id)?;
                Ok(0)
            }
            DRM_IOCTL_MODE_PAGE_FLIP => {
                if payload.len() < 24 {
                    return Err(DrmError::InvalidArgument);
                }
                let crtc_id = u32::from_ne_bytes(payload[0..4].try_into().unwrap());
                let fb_id = u32::from_ne_bytes(payload[4..8].try_into().unwrap());
                let flags = u32::from_ne_bytes(payload[8..12].try_into().unwrap());
                let reserved = u32::from_ne_bytes(payload[12..16].try_into().unwrap());
                let user_data = u64::from_ne_bytes(payload[16..24].try_into().unwrap());

                let req = DrmModeCrtcPageFlip {
                    crtc_id,
                    fb_id,
                    flags,
                    reserved,
                    user_data,
                };
                self.ioctl_page_flip(&req, caller)?;
                Ok(0)
            }
            _ => Err(DrmError::InvalidIoctl(cmd)),
        }
    }
}
