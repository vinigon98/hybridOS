//! Kernel Mode Setting (KMS) Subsystem for HybridOS.
//!
//! Provides the complete Linux KMS display pipeline architecture:
//! - Framebuffer objects (wrapping backing GEM handles, FourCC pixel formats)
//! - Hardware Planes (Primary, Overlay, Cursor) with source/destination clipping
//! - CRTCs (Cathode Ray Tube Controllers / Scanout Engines)
//! - Encoders (TMDS, DisplayPort, eDP, Virtual)
//! - Connectors (HDMI, DP, eDP, Virtual, VGA) with EDID & hotplug state
//! - Display Modes (timing parameters, pixel clocks, 60Hz/144Hz refresh rates)
//! - Atomic state updates, non-blocking page flips, and VBlank event notifications.

#[allow(unused_imports)]
use alloc::string::{String, ToString};
#[allow(unused_imports)]
use alloc::vec::Vec;
#[allow(unused_imports)]
use alloc::boxed::Box;
#[allow(unused_imports)]
use crate::compat::FloatExt;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

// ============================================================================
// DRM FourCC Pixel Formats
// ============================================================================

/// Helper macro to build DRM FourCC format codes
pub const fn fourcc_code(a: u8, b: u8, c: u8, d: u8) -> u32 {
    (a as u32) | ((b as u32) << 8) | ((c as u32) << 16) | ((d as u32) << 24)
}

pub const DRM_FORMAT_C8: u32 = fourcc_code(b'C', b'8', b' ', b' ');
pub const DRM_FORMAT_RGB565: u32 = fourcc_code(b'R', b'G', b'1', b'6');
pub const DRM_FORMAT_BGR565: u32 = fourcc_code(b'B', b'G', b'1', b'6');
pub const DRM_FORMAT_XRGB8888: u32 = fourcc_code(b'X', b'R', b'2', b'4');
pub const DRM_FORMAT_XBGR8888: u32 = fourcc_code(b'X', b'B', b'2', b'4');
pub const DRM_FORMAT_RGBX8888: u32 = fourcc_code(b'R', b'X', b'2', b'4');
pub const DRM_FORMAT_BGRX8888: u32 = fourcc_code(b'B', b'X', b'2', b'4');
pub const DRM_FORMAT_ARGB8888: u32 = fourcc_code(b'A', b'R', b'2', b'4');
pub const DRM_FORMAT_ABGR8888: u32 = fourcc_code(b'A', b'B', b'2', b'4');
pub const DRM_FORMAT_RGBA8888: u32 = fourcc_code(b'R', b'A', b'2', b'4');
pub const DRM_FORMAT_BGRA8888: u32 = fourcc_code(b'B', b'A', b'2', b'4');

// ============================================================================
// KMS Display Modes and Timings
// ============================================================================

pub const DRM_MODE_TYPE_BUILTIN: u32 = 1 << 0;
pub const DRM_MODE_TYPE_CLOCK_C: u32 = 1 << 1;
pub const DRM_MODE_TYPE_CRTC_C: u32 = 1 << 2;
pub const DRM_MODE_TYPE_PREFERRED: u32 = 1 << 3;
pub const DRM_MODE_TYPE_DEFAULT: u32 = 1 << 4;
pub const DRM_MODE_TYPE_USERDEF: u32 = 1 << 5;
pub const DRM_MODE_TYPE_DRIVER: u32 = 1 << 6;

pub const DRM_MODE_FLAG_PHSYNC: u32 = 1 << 0;
pub const DRM_MODE_FLAG_NHSYNC: u32 = 1 << 1;
pub const DRM_MODE_FLAG_PVSYNC: u32 = 1 << 2;
pub const DRM_MODE_FLAG_NVSYNC: u32 = 1 << 3;
pub const DRM_MODE_FLAG_INTERLACE: u32 = 1 << 4;
pub const DRM_MODE_FLAG_DBLSCAN: u32 = 1 << 5;

/// Display mode timing descriptor (matching `struct drm_mode_modeinfo`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayMode {
    pub name: String,
    /// Pixel clock in kHz
    pub clock: u32,
    /// Horizontal timing
    pub hdisplay: u16,
    pub hsync_start: u16,
    pub hsync_end: u16,
    pub htotal: u16,
    pub hskew: u16,
    /// Vertical timing
    pub vdisplay: u16,
    pub vsync_start: u16,
    pub vsync_end: u16,
    pub vtotal: u16,
    pub vscan: u16,
    /// Calculated vertical refresh rate in Hz
    pub vrefresh: u32,
    /// Timing flags (e.g. sync polarity)
    pub flags: u32,
    /// Mode type flags (e.g. preferred, driver)
    pub type_flags: u32,
}

impl DisplayMode {
    /// 1920x1080 @ 60 Hz (Standard 1080p FHD)
    pub fn mode_1080p_60() -> Self {
        Self {
            name: "1920x1080@60Hz".to_string(),
            clock: 148_500, // 148.5 MHz
            hdisplay: 1920,
            hsync_start: 2008,
            hsync_end: 2052,
            htotal: 2200,
            hskew: 0,
            vdisplay: 1080,
            vsync_start: 1084,
            vsync_end: 1089,
            vtotal: 1125,
            vscan: 0,
            vrefresh: 60,
            flags: DRM_MODE_FLAG_PHSYNC | DRM_MODE_FLAG_PVSYNC,
            type_flags: DRM_MODE_TYPE_DRIVER | DRM_MODE_TYPE_PREFERRED,
        }
    }

    /// 1920x1080 @ 144 Hz (High Refresh Gaming FHD)
    pub fn mode_1080p_144() -> Self {
        Self {
            name: "1920x1080@144Hz".to_string(),
            clock: 356_400, // 356.4 MHz
            hdisplay: 1920,
            hsync_start: 1968,
            hsync_end: 2000,
            htotal: 2080,
            hskew: 0,
            vdisplay: 1080,
            vsync_start: 1083,
            vsync_end: 1088,
            vtotal: 1190,
            vscan: 0,
            vrefresh: 144,
            flags: DRM_MODE_FLAG_PHSYNC | DRM_MODE_FLAG_PVSYNC,
            type_flags: DRM_MODE_TYPE_DRIVER,
        }
    }

    /// 2560x1440 @ 144 Hz (WQHD High Refresh)
    pub fn mode_1440p_144() -> Self {
        Self {
            name: "2560x1440@144Hz".to_string(),
            clock: 592_410, // ~592.4 MHz
            hdisplay: 2560,
            hsync_start: 2568,
            hsync_end: 2600,
            htotal: 2720,
            hskew: 0,
            vdisplay: 1440,
            vsync_start: 1443,
            vsync_end: 1448,
            vtotal: 1511,
            vscan: 0,
            vrefresh: 144,
            flags: DRM_MODE_FLAG_PHSYNC | DRM_MODE_FLAG_NVSYNC,
            type_flags: DRM_MODE_TYPE_DRIVER,
        }
    }

    /// 3840x2160 @ 60 Hz (4K UHD)
    pub fn mode_4k_60() -> Self {
        Self {
            name: "3840x2160@60Hz".to_string(),
            clock: 594_000, // 594.0 MHz
            hdisplay: 3840,
            hsync_start: 4016,
            hsync_end: 4104,
            htotal: 4400,
            hskew: 0,
            vdisplay: 2160,
            vsync_start: 2168,
            vsync_end: 2178,
            vtotal: 2250,
            vscan: 0,
            vrefresh: 60,
            flags: DRM_MODE_FLAG_PHSYNC | DRM_MODE_FLAG_PVSYNC,
            type_flags: DRM_MODE_TYPE_DRIVER,
        }
    }

    /// 1280x720 @ 60 Hz (Standard 720p HD)
    pub fn mode_720p_60() -> Self {
        Self {
            name: "1280x720@60Hz".to_string(),
            clock: 74_250, // 74.25 MHz
            hdisplay: 1280,
            hsync_start: 1390,
            hsync_end: 1430,
            htotal: 1650,
            hskew: 0,
            vdisplay: 720,
            vsync_start: 725,
            vsync_end: 730,
            vtotal: 750,
            vscan: 0,
            vrefresh: 60,
            flags: DRM_MODE_FLAG_PHSYNC | DRM_MODE_FLAG_PVSYNC,
            type_flags: DRM_MODE_TYPE_DRIVER,
        }
    }

    /// Verify timing integrity (e.g. sync start/end within totals)
    pub fn is_valid(&self) -> bool {
        self.hdisplay > 0
            && self.hsync_start >= self.hdisplay
            && self.hsync_end >= self.hsync_start
            && self.htotal >= self.hsync_end
            && self.vdisplay > 0
            && self.vsync_start >= self.vdisplay
            && self.vsync_end >= self.vsync_start
            && self.vtotal >= self.vsync_end
            && self.clock > 0
    }

    /// Calculate required video transmission bandwidth in megabits per second (assuming 24 bpp)
    pub fn bandwidth_mbps(&self, bpp: u32) -> f64 {
        let total_pixels_per_second = (self.htotal as u64) * (self.vtotal as u64) * (self.vrefresh as u64);
        (total_pixels_per_second * (bpp as u64)) as f64 / 1_000_000.0
    }
}

// ============================================================================
// KMS Subsystem Entities
// ============================================================================

/// KMS Framebuffer entity wrapping a GEM buffer object
#[derive(Debug, Clone)]
pub struct KmsFramebuffer {
    pub fb_id: u32,
    pub width: u32,
    pub height: u32,
    pub format: u32,
    pub pitches: [u32; 4],
    pub offsets: [u32; 4],
    pub gem_handles: [u32; 4],
    pub depth: u32,
    pub bpp: u32,
}

impl KmsFramebuffer {
    pub fn new_rgb32(fb_id: u32, width: u32, height: u32, pitch: u32, gem_handle: u32) -> Self {
        Self {
            fb_id,
            width,
            height,
            format: DRM_FORMAT_XRGB8888,
            pitches: [pitch, 0, 0, 0],
            offsets: [0, 0, 0, 0],
            gem_handles: [gem_handle, 0, 0, 0],
            depth: 24,
            bpp: 32,
        }
    }
}

/// KMS Hardware Plane Type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaneType {
    Primary = 1,
    Cursor = 2,
    Overlay = 3,
}

/// KMS Plane (e.g. Primary scanout plane, Hardware cursor, Hardware video overlay)
#[derive(Debug, Clone)]
pub struct KmsPlane {
    pub plane_id: u32,
    pub plane_type: PlaneType,
    /// Bitmask of CRTC IDs that this plane can bind to
    pub possible_crtcs: u32,
    /// Currently bound CRTC (if active)
    pub crtc_id: Option<u32>,
    /// Currently attached Framebuffer
    pub fb_id: Option<u32>,
    /// Source viewport (16.16 fixed-point per Linux DRM standard)
    pub src_x: u32,
    pub src_y: u32,
    pub src_w: u32,
    pub src_h: u32,
    /// CRTC target rectangle (pixels)
    pub crtc_x: i32,
    pub crtc_y: i32,
    pub crtc_w: u32,
    pub crtc_h: u32,
    /// Supported pixel formats
    pub formats: Vec<u32>,
    /// Z-order priority
    pub zpos: u32,
}

impl KmsPlane {
    pub fn new_primary(plane_id: u32, possible_crtcs: u32) -> Self {
        Self {
            plane_id,
            plane_type: PlaneType::Primary,
            possible_crtcs,
            crtc_id: None,
            fb_id: None,
            src_x: 0,
            src_y: 0,
            src_w: 0,
            src_h: 0,
            crtc_x: 0,
            crtc_y: 0,
            crtc_w: 0,
            crtc_h: 0,
            formats: vec![DRM_FORMAT_XRGB8888, DRM_FORMAT_ARGB8888, DRM_FORMAT_RGB565],
            zpos: 0,
        }
    }

    pub fn new_cursor(plane_id: u32, possible_crtcs: u32) -> Self {
        Self {
            plane_id,
            plane_type: PlaneType::Cursor,
            possible_crtcs,
            crtc_id: None,
            fb_id: None,
            src_x: 0,
            src_y: 0,
            src_w: 64 << 16,
            src_h: 64 << 16,
            crtc_x: 0,
            crtc_y: 0,
            crtc_w: 64,
            crtc_h: 64,
            formats: vec![DRM_FORMAT_ARGB8888],
            zpos: 100,
        }
    }

    pub fn new_overlay(plane_id: u32, possible_crtcs: u32) -> Self {
        Self {
            plane_id,
            plane_type: PlaneType::Overlay,
            possible_crtcs,
            crtc_id: None,
            fb_id: None,
            src_x: 0,
            src_y: 0,
            src_w: 0,
            src_h: 0,
            crtc_x: 0,
            crtc_y: 0,
            crtc_w: 0,
            crtc_h: 0,
            formats: vec![DRM_FORMAT_XRGB8888, DRM_FORMAT_ARGB8888],
            zpos: 50,
        }
    }
}

/// Cathode Ray Tube Controller (CRTC) scanout engine
#[derive(Debug, Clone)]
pub struct KmsCrtc {
    pub crtc_id: u32,
    pub enabled: bool,
    pub active_mode: Option<DisplayMode>,
    pub fb_id: Option<u32>,
    pub x: u32,
    pub y: u32,
    pub primary_plane_id: u32,
    pub cursor_plane_id: Option<u32>,
    pub overlay_plane_ids: Vec<u32>,
    pub gamma_size: u32,
    pub gamma_lut_red: Vec<u16>,
    pub gamma_lut_green: Vec<u16>,
    pub gamma_lut_blue: Vec<u16>,
    /// Current vertical blank sequence count
    pub vblank_sequence: u64,
    /// Timestamp of the last vertical blank event (nanoseconds since epoch)
    pub last_vblank_time_ns: u64,
    /// Pending page flips queued for next VBlank
    pub pending_page_flips: VecDeque<PageFlipRequest>,
}

impl KmsCrtc {
    pub fn new(crtc_id: u32, primary_plane_id: u32) -> Self {
        let gamma_size = 256;
        let mut red = Vec::with_capacity(gamma_size);
        let mut green = Vec::with_capacity(gamma_size);
        let mut blue = Vec::with_capacity(gamma_size);
        // Linear gamma ramp by default
        for i in 0..gamma_size {
            let val = ((i as u32 * 65535) / (gamma_size as u32 - 1)) as u16;
            red.push(val);
            green.push(val);
            blue.push(val);
        }

        Self {
            crtc_id,
            enabled: false,
            active_mode: None,
            fb_id: None,
            x: 0,
            y: 0,
            primary_plane_id,
            cursor_plane_id: None,
            overlay_plane_ids: Vec::new(),
            gamma_size: gamma_size as u32,
            gamma_lut_red: red,
            gamma_lut_green: green,
            gamma_lut_blue: blue,
            vblank_sequence: 0,
            last_vblank_time_ns: 0,
            pending_page_flips: VecDeque::new(),
        }
    }
}

/// KMS Encoder Type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncoderType {
    None = 0,
    Dac = 1,     // VGA
    Tmds = 2,    // HDMI / DVI
    Lvds = 3,    // Laptop LCD
    Tvdac = 4,   // Composite / S-Video
    Virtual = 5, // QEMU / VirtIO / Remote
    Dsi = 6,     // Mobile MIPI DSI
    Dpmst = 7,   // DisplayPort Multi-Stream
    Dp = 8,      // DisplayPort
}

/// KMS Encoder: Converts CRTC video timings into physical wire protocols
#[derive(Debug, Clone)]
pub struct KmsEncoder {
    pub encoder_id: u32,
    pub encoder_type: EncoderType,
    pub possible_crtcs: u32,
    pub possible_clones: u32,
    pub crtc_id: Option<u32>,
}

impl KmsEncoder {
    pub fn new(encoder_id: u32, encoder_type: EncoderType, possible_crtcs: u32) -> Self {
        Self {
            encoder_id,
            encoder_type,
            possible_crtcs,
            possible_clones: 0,
            crtc_id: None,
        }
    }
}

/// KMS Connector Type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectorType {
    Unknown = 0,
    Vga = 1,
    DviI = 2,
    DviD = 3,
    DviA = 4,
    Composite = 5,
    SVideo = 6,
    Lvds = 7,
    Component = 8,
    NinePinDin = 9,
    DisplayPort = 10,
    HdmiA = 11,
    HdmiB = 12,
    Tv = 13,
    Edp = 14,
    Virtual = 15,
    Dsi = 16,
    Dpi = 17,
}

/// KMS Connection Status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectorStatus {
    Connected = 1,
    Disconnected = 2,
    Unknown = 3,
}

/// KMS Connector: Physical display port connection (HDMI, DisplayPort, eDP, Virtual)
#[derive(Debug, Clone)]
pub struct KmsConnector {
    pub connector_id: u32,
    pub connector_type: ConnectorType,
    pub status: ConnectorStatus,
    /// Physical width in millimeters
    pub mm_width: u32,
    /// Physical height in millimeters
    pub mm_height: u32,
    /// Subpixel layout (e.g. Horizontal RGB)
    pub subpixel: u32,
    /// Currently bound Encoder
    pub encoder_id: Option<u32>,
    /// Bitmask or list of possible encoders
    pub possible_encoders: Vec<u32>,
    /// Supported display modes reported via EDID
    pub modes: Vec<DisplayMode>,
}

impl KmsConnector {
    pub fn new_virtual(connector_id: u32, encoder_id: u32) -> Self {
        Self {
            connector_id,
            connector_type: ConnectorType::Virtual,
            status: ConnectorStatus::Connected,
            mm_width: 531,  // ~24 inch standard monitor
            mm_height: 298,
            subpixel: 1, // DRM_MODE_SUBPIXEL_HORIZONTAL_RGB
            encoder_id: Some(encoder_id),
            possible_encoders: vec![encoder_id],
            modes: vec![
                DisplayMode::mode_1080p_60(),
                DisplayMode::mode_1080p_144(),
                DisplayMode::mode_1440p_144(),
                DisplayMode::mode_4k_60(),
                DisplayMode::mode_720p_60(),
            ],
        }
    }

    pub fn new_hdmi(connector_id: u32, encoder_id: u32) -> Self {
        Self {
            connector_id,
            connector_type: ConnectorType::HdmiA,
            status: ConnectorStatus::Connected,
            mm_width: 597, // ~27 inch monitor
            mm_height: 336,
            subpixel: 1,
            encoder_id: Some(encoder_id),
            possible_encoders: vec![encoder_id],
            modes: vec![
                DisplayMode::mode_1080p_60(),
                DisplayMode::mode_1080p_144(),
                DisplayMode::mode_720p_60(),
            ],
        }
    }

    pub fn new_displayport(connector_id: u32, encoder_id: u32) -> Self {
        Self {
            connector_id,
            connector_type: ConnectorType::DisplayPort,
            status: ConnectorStatus::Connected,
            mm_width: 597,
            mm_height: 336,
            subpixel: 1,
            encoder_id: Some(encoder_id),
            possible_encoders: vec![encoder_id],
            modes: vec![
                DisplayMode::mode_1440p_144(),
                DisplayMode::mode_1080p_144(),
                DisplayMode::mode_1080p_60(),
                DisplayMode::mode_4k_60(),
            ],
        }
    }

    pub fn new_edp(connector_id: u32, encoder_id: u32) -> Self {
        Self {
            connector_id,
            connector_type: ConnectorType::Edp,
            status: ConnectorStatus::Connected,
            mm_width: 344, // ~15.6 inch laptop screen
            mm_height: 194,
            subpixel: 1,
            encoder_id: Some(encoder_id),
            possible_encoders: vec![encoder_id],
            modes: vec![
                DisplayMode::mode_1080p_60(),
                DisplayMode::mode_1080p_144(),
            ],
        }
    }
}

// ============================================================================
// Atomic Page Flipping & VBlank Event Notification
// ============================================================================

pub const DRM_MODE_PAGE_FLIP_EVENT: u32 = 0x01;
pub const DRM_MODE_PAGE_FLIP_ASYNC: u32 = 0x02;

/// Page Flip request dispatched by userland compositor (e.g. Wayland compositor / Xorg)
#[derive(Debug, Clone)]
pub struct PageFlipRequest {
    pub crtc_id: u32,
    pub fb_id: u32,
    pub flags: u32,
    pub user_data: u64,
}

/// DRM VBlank Event generated upon completion of vertical scanout
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VBlankEvent {
    pub crtc_id: u32,
    pub sequence: u64,
    pub tv_sec: u64,
    pub tv_usec: u64,
    pub user_data: u64,
}

// ============================================================================
// Kernel Mode Setting (KMS) Subsystem Pipeline Manager
// ============================================================================

/// Errors originating within the KMS subsystem
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KmsError {
    InvalidCrtc(u32),
    InvalidPlane(u32),
    InvalidFramebuffer(u32),
    InvalidEncoder(u32),
    InvalidConnector(u32),
    InvalidMode(String),
    PlaneIncompatibleWithCrtc,
    FormatNotSupported,
    PlaneOutOfBounds,
    AtomicCheckFailed(String),
    Busy,
}

/// Complete KMS Display Pipeline manager
#[derive(Debug)]
pub struct KmsSubsystem {
    pub framebuffers: HashMap<u32, KmsFramebuffer>,
    pub planes: HashMap<u32, KmsPlane>,
    pub crtcs: HashMap<u32, KmsCrtc>,
    pub encoders: HashMap<u32, KmsEncoder>,
    pub connectors: HashMap<u32, KmsConnector>,
    next_id: u32,
    vblank_event_queue: Arc<Mutex<VecDeque<VBlankEvent>>>,
}

impl KmsSubsystem {
    /// Create a new, fully populated KMS pipeline default topology:
    /// CRTC0 -> Encoder0 (TMDS/Virtual) -> Connector0 (Virtual/HDMI) with Primary and Cursor Planes
    pub fn new() -> Self {
        let mut kms = Self {
            framebuffers: HashMap::new(),
            planes: HashMap::new(),
            crtcs: HashMap::new(),
            encoders: HashMap::new(),
            connectors: HashMap::new(),
            next_id: 1,
            vblank_event_queue: Arc::new(Mutex::new(VecDeque::new())),
        };

        kms.initialize_default_topology();
        kms
    }

    fn alloc_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Construct default virtual KMS pipeline
    fn initialize_default_topology(&mut self) {
        let crtc_id = self.alloc_id();
        let primary_plane_id = self.alloc_id();
        let cursor_plane_id = self.alloc_id();
        let encoder_id = self.alloc_id();
        let connector_id = self.alloc_id();

        // 1. Primary & Cursor planes
        let primary_plane = KmsPlane::new_primary(primary_plane_id, 1 << 0);
        let cursor_plane = KmsPlane::new_cursor(cursor_plane_id, 1 << 0);
        self.planes.insert(primary_plane_id, primary_plane);
        self.planes.insert(cursor_plane_id, cursor_plane);

        // 2. CRTC
        let mut crtc = KmsCrtc::new(crtc_id, primary_plane_id);
        crtc.cursor_plane_id = Some(cursor_plane_id);
        self.crtcs.insert(crtc_id, crtc);

        // 3. Encoder (Virtual / TMDS bridge)
        let encoder = KmsEncoder::new(encoder_id, EncoderType::Virtual, 1 << 0);
        self.encoders.insert(encoder_id, encoder);

        // 4. Connector (Virtual HDMI-compatible)
        let connector = KmsConnector::new_virtual(connector_id, encoder_id);
        self.connectors.insert(connector_id, connector);
    }

    /// Register a new Framebuffer object backed by a GEM handle
    pub fn add_framebuffer(
        &mut self,
        width: u32,
        height: u32,
        pitch: u32,
        gem_handle: u32,
    ) -> Result<u32, KmsError> {
        let fb_id = self.alloc_id();
        let fb = KmsFramebuffer::new_rgb32(fb_id, width, height, pitch, gem_handle);
        self.framebuffers.insert(fb_id, fb);
        Ok(fb_id)
    }

    /// Remove a Framebuffer object
    pub fn remove_framebuffer(&mut self, fb_id: u32) -> Result<(), KmsError> {
        if self.framebuffers.remove(&fb_id).is_some() {
            // Disconnect from any active plane or CRTC
            for plane in self.planes.values_mut() {
                if plane.fb_id == Some(fb_id) {
                    plane.fb_id = None;
                }
            }
            for crtc in self.crtcs.values_mut() {
                if crtc.fb_id == Some(fb_id) {
                    crtc.fb_id = None;
                }
            }
            Ok(())
        } else {
            Err(KmsError::InvalidFramebuffer(fb_id))
        }
    }

    /// Set CRTC configuration (Modesetting)
    pub fn set_crtc(
        &mut self,
        crtc_id: u32,
        fb_id: Option<u32>,
        x: u32,
        y: u32,
        mode: Option<DisplayMode>,
        connectors: &[u32],
    ) -> Result<(), KmsError> {
        let crtc = self.crtcs.get_mut(&crtc_id).ok_or(KmsError::InvalidCrtc(crtc_id))?;

        if let Some(ref m) = mode {
            if !m.is_valid() {
                return Err(KmsError::InvalidMode(m.name.clone()));
            }
        }

        if let Some(f_id) = fb_id {
            if !self.framebuffers.contains_key(&f_id) {
                return Err(KmsError::InvalidFramebuffer(f_id));
            }
        }

        crtc.enabled = mode.is_some() && fb_id.is_some();
        crtc.active_mode = mode.clone();
        crtc.fb_id = fb_id;
        crtc.x = x;
        crtc.y = y;

        // Update primary plane
        let primary_id = crtc.primary_plane_id;
        if let Some(plane) = self.planes.get_mut(&primary_id) {
            plane.crtc_id = if crtc.enabled { Some(crtc_id) } else { None };
            plane.fb_id = fb_id;
            if let Some(ref m) = mode {
                plane.src_w = (m.hdisplay as u32) << 16;
                plane.src_h = (m.vdisplay as u32) << 16;
                plane.crtc_w = m.hdisplay as u32;
                plane.crtc_h = m.vdisplay as u32;
            }
        }

        // Bind connectors and encoders
        for &conn_id in connectors {
            if let Some(conn) = self.connectors.get_mut(&conn_id) {
                if let Some(enc_id) = conn.encoder_id {
                    if let Some(enc) = self.encoders.get_mut(&enc_id) {
                        enc.crtc_id = Some(crtc_id);
                    }
                }
            }
        }

        Ok(())
    }

    /// Queue or execute a Page Flip on the specified CRTC
    pub fn page_flip(
        &mut self,
        crtc_id: u32,
        fb_id: u32,
        flags: u32,
        user_data: u64,
    ) -> Result<(), KmsError> {
        if !self.framebuffers.contains_key(&fb_id) {
            return Err(KmsError::InvalidFramebuffer(fb_id));
        }

        let crtc = self.crtcs.get_mut(&crtc_id).ok_or(KmsError::InvalidCrtc(crtc_id))?;
        if !crtc.enabled {
            return Err(KmsError::InvalidCrtc(crtc_id));
        }

        let req = PageFlipRequest {
            crtc_id,
            fb_id,
            flags,
            user_data,
        };

        if (flags & DRM_MODE_PAGE_FLIP_ASYNC) != 0 {
            // Immediate flip without waiting for VBlank
            crtc.fb_id = Some(fb_id);
            let primary_id = crtc.primary_plane_id;
            if let Some(plane) = self.planes.get_mut(&primary_id) {
                plane.fb_id = Some(fb_id);
            }
            if (flags & DRM_MODE_PAGE_FLIP_EVENT) != 0 {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default();
                let event = VBlankEvent {
                    crtc_id,
                    sequence: crtc.vblank_sequence,
                    tv_sec: now.as_secs(),
                    tv_usec: now.subsec_micros() as u64,
                    user_data,
                };
                self.vblank_event_queue.lock().unwrap().push_back(event);
            }
        } else {
            // Queue for next vertical blanking interval
            crtc.pending_page_flips.push_back(req);
        }

        Ok(())
    }

    /// Trigger a Vertical Blank (VBlank) interrupt event on a given CRTC.
    /// This atomically swaps the scanout buffer for tearing-free rendering
    /// and generates DRM completion events.
    pub fn trigger_vblank(&mut self, crtc_id: u32) -> Result<Option<VBlankEvent>, KmsError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();

        let crtc = self.crtcs.get_mut(&crtc_id).ok_or(KmsError::InvalidCrtc(crtc_id))?;
        crtc.vblank_sequence += 1;
        crtc.last_vblank_time_ns = now.as_nanos() as u64;

        let mut completed_event = None;

        if let Some(flip) = crtc.pending_page_flips.pop_front() {
            crtc.fb_id = Some(flip.fb_id);
            let primary_id = crtc.primary_plane_id;
            if let Some(plane) = self.planes.get_mut(&primary_id) {
                plane.fb_id = Some(flip.fb_id);
            }

            if (flip.flags & DRM_MODE_PAGE_FLIP_EVENT) != 0 {
                let event = VBlankEvent {
                    crtc_id,
                    sequence: crtc.vblank_sequence,
                    tv_sec: now.as_secs(),
                    tv_usec: now.subsec_micros() as u64,
                    user_data: flip.user_data,
                };
                self.vblank_event_queue.lock().unwrap().push_back(event.clone());
                completed_event = Some(event);
            }
        }

        Ok(completed_event)
    }

    /// Pop the next available VBlank event for userland event polling
    pub fn pop_vblank_event(&self) -> Option<VBlankEvent> {
        self.vblank_event_queue.lock().unwrap().pop_front()
    }

    /// Retrieve connector status and supported modes
    pub fn get_connector(&self, connector_id: u32) -> Result<&KmsConnector, KmsError> {
        self.connectors.get(&connector_id).ok_or(KmsError::InvalidConnector(connector_id))
    }

    /// Retrieve CRTC state
    pub fn get_crtc(&self, crtc_id: u32) -> Result<&KmsCrtc, KmsError> {
        self.crtcs.get(&crtc_id).ok_or(KmsError::InvalidCrtc(crtc_id))
    }

    /// Retrieve Plane state
    pub fn get_plane(&self, plane_id: u32) -> Result<&KmsPlane, KmsError> {
        self.planes.get(&plane_id).ok_or(KmsError::InvalidPlane(plane_id))
    }
}


