//! HybridOS Native Applications Suite — Category 6: Multimedia, Paint, Audio & Themes (42 to 47).
//!
//! Implements completely bare-metal Crystal Glass native views for:
//! 42. Visualizador de Imagens e Fotos (Photo Viewer com Zoom, Rotação, Slide Show e decodificador BMP real)
//! 43. Estúdio de Pintura (Paint & Pixel Canvas com pintura em tempo real, 16 cores vitrificadas, 6 ferramentas e gravação BMP)
//! 44. Ferramenta de Captura de Tela (Snipping Tool com captura do Framebuffer GOP e exportação para C:\screenshot.bmp)
//! 45. Reprodutor de Mídia (Crystal Media Player com espectro sonoro phosphor glow e sintetizador Intel HDA 8086:2668)
//! 46. Mixer de Volume HDA (Sliders verticais independentes, Mute, medidor VU estéreo e verbos de ganho para o codec)
//! 47. Estúdio de Personalização Crystal Glass (8 cores de vidro, slider de opacidade e seletor de papel de parede em tempo real)

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;

use crate::sync::SpinLock;
use crate::drivers::desktop::{DesktopState, draw_text_px, draw_text_clipped};
use crate::drivers::gfx2d::*;
use crate::drivers::hda::{HDA_CONTROLLER, REG_ICO, REG_ICS};
use crate::drivers::mouse::MOUSE_STATE;
use crate::vfs;

// =============================================================================
// GLOBAL THEME & VISUAL PERSONALIZATION (Program 47)
// =============================================================================

#[derive(Clone, Copy, Debug)]
pub struct GlassThemeSettings {
    pub theme_index: usize,
    pub frame_active_color: u32,
    pub frame_inactive_color: u32,
    pub accent_color: u32,
    pub glass_opacity: u8,
    pub wallpaper_preset: u8, // 0: Gradiente Aurora, 1: Abstrato Cristalino, 2: Minimalista Escuro
    pub theme_name: &'static str,
}

pub struct ThemePaletteItem {
    pub name: &'static str,
    pub active_frame: u32,
    pub inactive_frame: u32,
    pub accent: u32,
}

pub const THEME_PALETTE: [ThemePaletteItem; 8] = [
    ThemePaletteItem { name: "Azul Safira Padrao",  active_frame: 0x00163254, inactive_frame: 0x003C4B5C, accent: 0x0038BDF8 },
    ThemePaletteItem { name: "Verde Esmeralda",     active_frame: 0x00064E3B, inactive_frame: 0x00132E27, accent: 0x0034D399 },
    ThemePaletteItem { name: "Vermelho Rubi",       active_frame: 0x007F1D1D, inactive_frame: 0x00450A0A, accent: 0x00F87171 },
    ThemePaletteItem { name: "Roxo Ametista",       active_frame: 0x00581C87, inactive_frame: 0x003B0764, accent: 0x00C084FC },
    ThemePaletteItem { name: "Ambar Dourado",       active_frame: 0x0078350F, inactive_frame: 0x00451A03, accent: 0x00FBBF24 },
    ThemePaletteItem { name: "Grafite Escuro",      active_frame: 0x001E293B, inactive_frame: 0x000F172A, accent: 0x0094A3B8 },
    ThemePaletteItem { name: "Rosa Quartzo",        active_frame: 0x00831843, inactive_frame: 0x0050072B, accent: 0x00F472B6 },
    ThemePaletteItem { name: "Ciano Glacial",       active_frame: 0x00155E75, inactive_frame: 0x00083344, accent: 0x0022D3EE },
];

pub static GLOBAL_THEME: SpinLock<GlassThemeSettings> = SpinLock::new(GlassThemeSettings {
    theme_index: 0,
    frame_active_color: 0x00163254,
    frame_inactive_color: 0x003C4B5C,
    accent_color: 0x0038BDF8,
    glass_opacity: 150,
    wallpaper_preset: 0,
    theme_name: "Azul Safira Padrao",
});

#[inline]
pub fn get_current_theme() -> GlassThemeSettings {
    *GLOBAL_THEME.lock()
}

static PERSONALIZATION_PENDING_CLICK: SpinLock<Option<(usize, usize)>> = SpinLock::new(None);

// =============================================================================
// PROGRAM 42: PHOTO VIEWER STATE & DECODER
// =============================================================================

pub struct PhotoViewerState {
    pub zoom_pct: usize,      // 25, 50, 75, 100, 150, 200, 300, 400
    pub rotation_deg: usize,  // 0, 90, 180, 270
    pub fit_to_screen: bool,
    pub slideshow_active: bool,
    pub slideshow_timer: usize,
    pub current_mode: usize,  // 0: SMPTE Calibration Bars, 1: VFS C:\desenho.bmp, 2: VFS C:\screenshot.bmp, 3: Radial RGB Test
    pub pan_x: isize,
    pub pan_y: isize,
    pub status_text: String,
    pub pending_click: Option<(usize, usize)>,
}

static PHOTO_VIEWER_STATE: SpinLock<PhotoViewerState> = SpinLock::new(PhotoViewerState {
    zoom_pct: 100,
    rotation_deg: 0,
    fit_to_screen: false,
    slideshow_active: false,
    slideshow_timer: 0,
    current_mode: 0,
    pan_x: 0,
    pan_y: 0,
    status_text: String::new(),
    pending_click: None,
});

// =============================================================================
// PROGRAM 43: PAINT & PIXEL CANVAS STATE
// =============================================================================

const CANVAS_W: usize = 320;
const CANVAS_H: usize = 200;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PaintTool {
    Pencil,
    Brush,
    Line,
    Rectangle,
    Bucket,
    Eraser,
}

pub const PAINT_PALETTE_16: [u32; 16] = [
    0x00000000, // 0: Preto Ônix
    0x00FFFFFF, // 1: Branco Cristal
    0x0064748B, // 2: Cinza Chumbo
    0x00CBD5E1, // 3: Cinza Névoa
    0x00EF4444, // 4: Vermelho Rubi
    0x00F97316, // 5: Laranja Chama
    0x00FBBF24, // 6: Âmbar Dourado
    0x0010B981, // 7: Verde Esmeralda
    0x0084CC16, // 8: Verde Lima
    0x0006B6D4, // 9: Ciano Glacial
    0x000284C7, // 10: Azul Safira
    0x001D4ED8, // 11: Azul Cobalto
    0x008B5CF6, // 12: Roxo Ametista
    0x00D946EF, // 13: Magenta Neon
    0x00F472B6, // 14: Rosa Quartzo
    0x0078350F, // 15: Marrom Mogno
];

pub struct PaintStudioState {
    pub canvas: [u32; CANVAS_W * CANVAS_H],
    pub selected_tool: PaintTool,
    pub selected_color_idx: usize,
    pub stroke_width: usize, // 1, 2, 4, 8
    pub last_paint_pos: Option<(usize, usize)>,
    pub shape_start_pos: Option<(usize, usize)>,
    pub status_msg: String,
    pub status_timer: usize,
    pub initialized: bool,
    pub pending_click: Option<(usize, usize)>,
}

static PAINT_STATE: SpinLock<PaintStudioState> = SpinLock::new(PaintStudioState {
    canvas: [0x00FFFFFF; CANVAS_W * CANVAS_H],
    selected_tool: PaintTool::Pencil,
    selected_color_idx: 10, // Azul Safira
    stroke_width: 2,
    last_paint_pos: None,
    shape_start_pos: None,
    status_msg: String::new(),
    status_timer: 0,
    initialized: false,
    pending_click: None,
});

// =============================================================================
// PROGRAM 44: SNIPPING TOOL STATE
// =============================================================================

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SnipMode {
    FullScreen,
    ActiveWindow,
    Rectangular,
}

pub struct SnippingToolState {
    pub mode: SnipMode,
    pub has_capture: bool,
    pub cap_w: usize,
    pub cap_h: usize,
    pub preview_thumb: [u32; 160 * 100], // 160x100 preview buffer
    pub status_msg: String,
    pub snip_count: usize,
    pub pending_click: Option<(usize, usize)>,
}

static SNIP_STATE: SpinLock<SnippingToolState> = SpinLock::new(SnippingToolState {
    mode: SnipMode::FullScreen,
    has_capture: false,
    cap_w: 1280,
    cap_h: 800,
    preview_thumb: [0x001E293B; 160 * 100],
    status_msg: String::new(),
    snip_count: 0,
    pending_click: None,
});

// =============================================================================
// PROGRAM 45: CRYSTAL MEDIA PLAYER STATE
// =============================================================================

pub struct TrackInfo {
    pub title: &'static str,
    pub artist: &'static str,
    pub duration_sec: usize,
    pub notes: &'static [(u32, u32)], // (freq_hz, duration_ms)
}

pub const MEDIA_TRACKS: [TrackInfo; 4] = [
    TrackInfo {
        title: "HybridOS Crystal Symphony",
        artist: "Bare-Metal Sound Core",
        duration_sec: 168,
        notes: &[
            (523, 140), (659, 140), (784, 180), (1046, 260),
            (880, 140), (784, 140), (659, 200), (523, 300),
        ],
    },
    TrackInfo {
        title: "Cyberpunk 2026 Synthwave",
        artist: "Ring 0 Oscillators",
        duration_sec: 195,
        notes: &[
            (220, 110), (261, 110), (329, 130), (440, 220),
            (392, 110), (329, 110), (261, 160), (220, 260),
        ],
    },
    TrackInfo {
        title: "Bare-Metal 8-Bit Chiptune",
        artist: "Intel HDA Synthesis",
        duration_sec: 110,
        notes: &[
            (440, 80), (554, 80), (659, 90), (880, 160),
            (740, 80), (659, 80), (554, 100), (440, 180),
        ],
    },
    TrackInfo {
        title: "Ambient Space Aurora",
        artist: "Crystal Glass Harmonic",
        duration_sec: 240,
        notes: &[
            (330, 200), (392, 200), (494, 240), (587, 320),
            (523, 200), (440, 200), (349, 240), (330, 360),
        ],
    },
];

pub struct MediaPlayerState {
    pub is_playing: bool,
    pub current_track: usize,
    pub current_sec: usize,
    pub spectrum_heights: [u8; 16],
    pub spectrum_peaks: [u8; 16],
    pub anim_tick: usize,
    pub note_step: usize,
    pub pending_click: Option<(usize, usize)>,
}

static MEDIA_STATE: SpinLock<MediaPlayerState> = SpinLock::new(MediaPlayerState {
    is_playing: false,
    current_track: 0,
    current_sec: 24,
    spectrum_heights: [0; 16],
    spectrum_peaks: [0; 16],
    anim_tick: 0,
    note_step: 0,
    pending_click: None,
});

/// Returns `true` while Crystal Media Player (#45) is actively playing,
/// allowing `desktop.rs` to schedule periodic redraws for continuous spectrum animation.
#[inline]
pub fn is_media_playing() -> bool {
    MEDIA_STATE.lock().is_playing
}

// =============================================================================
// PROGRAM 46: HDA VOLUME MIXER STATE
// =============================================================================

pub struct HdaMixerState {
    pub vol_master: u8, // 0..100
    pub vol_system: u8,
    pub vol_wave: u8,
    pub vol_synth: u8,
    pub mute_master: bool,
    pub mute_system: bool,
    pub mute_wave: bool,
    pub mute_synth: bool,
    pub vu_meter_l: u8,
    pub vu_meter_r: u8,
    pub vu_peak_l: u8,
    pub vu_peak_r: u8,
    pub anim_tick: usize,
    pub pending_click: Option<(usize, usize)>,
}

static HDA_MIXER_STATE: SpinLock<HdaMixerState> = SpinLock::new(HdaMixerState {
    vol_master: 85,
    vol_system: 70,
    vol_wave: 90,
    vol_synth: 75,
    mute_master: false,
    mute_system: false,
    mute_wave: false,
    mute_synth: false,
    vu_meter_l: 60,
    vu_meter_r: 55,
    vu_peak_l: 75,
    vu_peak_r: 70,
    anim_tick: 0,
    pending_click: None,
});

// =============================================================================
// HARDWARE HDA HELPER FUNCTIONS (Gain, Verbs & Tones)
// =============================================================================

/// Syncs the software Master Volume and Mute state directly to the Intel HDA hardware.
pub fn sync_hda_volume_hardware(vol_pct: u8, is_muted: bool) {
    let guard = HDA_CONTROLLER.lock();
    if let Some(ref hda) = *guard {
        let gain_7bit = ((vol_pct as u32 * 127) / 100).min(127);
        let mute_flag = if is_muted { 0x80u32 } else { 0x00u32 };
        let amp_param = (mute_flag | gain_7bit) & 0xFF;

        let ico_ptr = (hda.mmio_base + REG_ICO as u64) as *mut u32;
        let ics_ptr = (hda.mmio_base + REG_ICS as u64) as *mut u16;

        // Verb 0x3: Set Amplifier Gain/Mute
        // Node 0x02 (DAC Output Converter) - Left + Right Output: 0x3B000 | amp_param
        let dac_cmd = ((0u32 & 0x0F) << 28) | ((2u32 & 0xFF) << 20) | 0x3B000 | amp_param;
        unsafe {
            core::ptr::write_volatile(ico_ptr, dac_cmd);
            core::ptr::write_volatile(ics_ptr, core::ptr::read_volatile(ics_ptr) | 0x01);
            for _ in 0..10_000 {
                if (core::ptr::read_volatile(ics_ptr) & 0x01) == 0 { break; }
            }
        }

        // Node 0x04 (Pin Complex) - Left + Right Output: 0x3B000 | amp_param
        let pin_cmd = ((0u32 & 0x0F) << 28) | ((4u32 & 0xFF) << 20) | 0x3B000 | amp_param;
        unsafe {
            core::ptr::write_volatile(ico_ptr, pin_cmd);
            core::ptr::write_volatile(ics_ptr, core::ptr::read_volatile(ics_ptr) | 0x01);
            for _ in 0..10_000 {
                if (core::ptr::read_volatile(ics_ptr) & 0x01) == 0 { break; }
            }
        }
    }
}

/// Plays a fast, non-blocking click tone on HDA as tactile audio feedback for UI operations.
pub fn play_ui_feedback_tone(freq: u32, duration_ms: u32) {
    let guard = HDA_CONTROLLER.lock();
    if let Some(ref hda) = *guard {
        hda.play_tone(freq, duration_ms);
    }
}

// =============================================================================
// REAL BMP 24-BPP ENCODER (Zero Mocks — VFS Integration)
// =============================================================================

/// Encodes an array of ARGB/RGB pixels into a fully compliant standard 24-bit BMP image.
pub fn encode_bmp_24bpp(width: usize, height: usize, pixels: &[u32]) -> Vec<u8> {
    let row_stride = ((width * 3 + 3) / 4) * 4;
    let image_data_size = row_stride * height;
    let file_size = 54 + image_data_size;

    let mut buf = Vec::with_capacity(file_size);

    // --- BITMAPFILEHEADER (14 bytes) ---
    buf.push(b'B');
    buf.push(b'M');
    buf.extend_from_slice(&(file_size as u32).to_le_bytes());
    buf.extend_from_slice(&0u16.to_le_bytes()); // Reserved 1
    buf.extend_from_slice(&0u16.to_le_bytes()); // Reserved 2
    buf.extend_from_slice(&54u32.to_le_bytes()); // Pixel data offset

    // --- BITMAPINFOHEADER (40 bytes) ---
    buf.extend_from_slice(&40u32.to_le_bytes()); // Header size
    buf.extend_from_slice(&(width as i32).to_le_bytes());
    buf.extend_from_slice(&(height as i32).to_le_bytes()); // Bottom-up
    buf.extend_from_slice(&1u16.to_le_bytes()); // Planes
    buf.extend_from_slice(&24u16.to_le_bytes()); // 24-bpp
    buf.extend_from_slice(&0u32.to_le_bytes()); // BI_RGB (uncompressed)
    buf.extend_from_slice(&(image_data_size as u32).to_le_bytes());
    buf.extend_from_slice(&2835u32.to_le_bytes()); // ~72 DPI horizontal
    buf.extend_from_slice(&2835u32.to_le_bytes()); // ~72 DPI vertical
    buf.extend_from_slice(&0u32.to_le_bytes()); // Colors used
    buf.extend_from_slice(&0u32.to_le_bytes()); // Important colors

    // --- PIXEL DATA (Bottom-up: row height-1 down to 0) ---
    let pad_count = row_stride - width * 3;
    for y_inv in 0..height {
        let row_idx = height - 1 - y_inv;
        let row_start = row_idx * width;
        for col in 0..width {
            let px = if row_start + col < pixels.len() { pixels[row_start + col] } else { 0 };
            let b = (px & 0xFF) as u8;
            let g = ((px >> 8) & 0xFF) as u8;
            let r = ((px >> 16) & 0xFF) as u8;
            buf.push(b);
            buf.push(g);
            buf.push(r);
        }
        for _ in 0..pad_count {
            buf.push(0);
        }
    }

    buf
}

// =============================================================================
// FAST DRAWING HELPERS
// =============================================================================

#[inline]
unsafe fn fill_rect(fb: *mut u8, pitch: usize, scr_w: usize, scr_h: usize, x: usize, y: usize, w: usize, h: usize, color: u32) {
    let x_end = (x + w).min(scr_w);
    let y_end = (y + h).min(scr_h);
    for py in y..y_end {
        let row = fb.add(py * pitch) as *mut u32;
        for px in x..x_end {
            *row.add(px) = color;
        }
    }
}

// =============================================================================
// EXPORTED CATEGORY 6 INTERFACES
// =============================================================================

/// Renders Category 6 applications content inside the window client canvas.
pub unsafe fn render_cat6_app(
    id: u8,
    fb: *mut u8, pitch: usize, scr_w: usize, scr_h: usize,
    cx: usize, cy: usize, cw: usize, ch: usize,
    desk: &mut DesktopState,
) {
    match id {
        42 => render_photo_viewer(fb, pitch, scr_w, scr_h, cx, cy, cw, ch, desk),
        43 => render_paint_studio(fb, pitch, scr_w, scr_h, cx, cy, cw, ch, desk),
        44 => render_snipping_tool(fb, pitch, scr_w, scr_h, cx, cy, cw, ch, desk),
        45 => render_crystal_media_player(fb, pitch, scr_w, scr_h, cx, cy, cw, ch, desk),
        46 => render_hda_mixer(fb, pitch, scr_w, scr_h, cx, cy, cw, ch, desk),
        47 => render_personalization_studio(fb, pitch, scr_w, scr_h, cx, cy, cw, ch, desk),
        _ => {}
    }
}

/// Dispatches mouse clicks for Category 6 applications.
pub fn handle_cat6_click(id: u8, cx: usize, cy: usize, desk: &mut DesktopState) -> bool {
    match id {
        42 => handle_photo_viewer_click(cx, cy, desk),
        43 => handle_paint_studio_click(cx, cy, desk),
        44 => handle_snipping_tool_click(cx, cy, desk),
        45 => handle_media_player_click(cx, cy, desk),
        46 => handle_hda_mixer_click(cx, cy, desk),
        47 => handle_personalization_click(cx, cy, desk),
        _ => false,
    }
}

// =============================================================================
// 42. VISUALIZADOR DE IMAGENS E FOTOS (Photo Viewer)
// =============================================================================

unsafe fn render_photo_viewer(
    fb: *mut u8, pitch: usize, scr_w: usize, scr_h: usize,
    cx: usize, cy: usize, cw: usize, ch: usize,
    desk: &mut DesktopState,
) {
    let mut state = PHOTO_VIEWER_STATE.lock();

    // 1. Top Glass Toolbar (36px)
    draw_glass_toolbar(fb, pitch, scr_w, scr_h, cx, cy, cw, 36);

    let (mx, my) = {
        let m = MOUSE_STATE.lock();
        (m.x, m.y)
    };
    let pending = state.pending_click.take();
    let (btn_mx, btn_my, btn_click) = match pending {
        Some((px, py)) => (px as i32, py as i32, true),
        None => (mx, my, false),
    };

    // Toolbar buttons with exact edge-triggered hitboxes
    let mut bx = cx + 8;
    if ui_glass_button(fb, pitch, scr_w, scr_h, bx, cy + 5, 80, 26, "Zoom In +", ButtonStyle::StandardGlass, btn_mx, btn_my, btn_click) {
        state.zoom_pct = (state.zoom_pct + 25).min(400);
        state.fit_to_screen = false;
        desk.needs_full_redraw = true;
    }
    bx += 86;

    if ui_glass_button(fb, pitch, scr_w, scr_h, bx, cy + 5, 85, 26, "Zoom Out -", ButtonStyle::StandardGlass, btn_mx, btn_my, btn_click) {
        state.zoom_pct = (state.zoom_pct.saturating_sub(25)).max(25);
        state.fit_to_screen = false;
        desk.needs_full_redraw = true;
    }
    bx += 91;

    let rot_label = format!("Girar {}*", (state.rotation_deg + 90) % 360);
    if ui_glass_button(fb, pitch, scr_w, scr_h, bx, cy + 5, 84, 26, &rot_label, ButtonStyle::StandardGlass, btn_mx, btn_my, btn_click) {
        state.rotation_deg = (state.rotation_deg + 90) % 360;
        desk.needs_full_redraw = true;
    }
    bx += 90;

    let fit_style = if state.fit_to_screen { ButtonStyle::PrimaryAction } else { ButtonStyle::StandardGlass };
    if ui_glass_button(fb, pitch, scr_w, scr_h, bx, cy + 5, 96, 26, "Ajustar Tela", fit_style, btn_mx, btn_my, btn_click) {
        state.fit_to_screen = !state.fit_to_screen;
        desk.needs_full_redraw = true;
    }
    bx += 102;

    let slide_style = if state.slideshow_active { ButtonStyle::PrimaryAction } else { ButtonStyle::StandardGlass };
    let slide_label = if state.slideshow_active { "❚❚ Pausar" } else { "▶ SlideShow" };
    if ui_glass_button(fb, pitch, scr_w, scr_h, bx, cy + 5, 94, 26, slide_label, slide_style, btn_mx, btn_my, btn_click) {
        state.slideshow_active = !state.slideshow_active;
        desk.needs_full_redraw = true;
    }
    bx += 100;

    let mode_label = match state.current_mode {
        0 => "SMPTE Bars",
        1 => "C:\\desenho.bmp",
        2 => "C:\\screen.bmp",
        _ => "Gradiente RGB",
    };
    if ui_glass_button(fb, pitch, scr_w, scr_h, bx, cy + 5, 110, 26, mode_label, ButtonStyle::ToolbarFlat, btn_mx, btn_my, btn_click) {
        state.current_mode = (state.current_mode + 1) % 4;
        desk.needs_full_redraw = true;
    }

    // 2. Main Viewport Area (Graphite & Dark Checkered Pattern)
    let vp_y = cy + 36;
    let vp_h = ch.saturating_sub(36 + 24);
    let vp_x0 = cx.min(scr_w);
    let vp_y0 = vp_y.min(scr_h);
    let vp_x1 = (cx + cw).min(scr_w);
    let vp_y1 = (vp_y + vp_h).min(scr_h);

    fill_rect(fb, pitch, scr_w, scr_h, cx, vp_y, cw, vp_h, 0x001E293B);

    // Draw subtle 16px checkerboard background
    for ty in (vp_y0..vp_y1).step_by(16) {
        for tx in (vp_x0..vp_x1).step_by(16) {
            if ((tx / 16) + (ty / 16)) % 2 == 1 {
                fill_rect(fb, pitch, scr_w, scr_h, tx, ty, 16.min(vp_x1 - tx), 16.min(vp_y1 - ty), 0x00334155);
            }
        }
    }

    // Handle Slide Show timer animation
    if state.slideshow_active {
        state.slideshow_timer += 1;
        if state.slideshow_timer > 60 {
            state.slideshow_timer = 0;
            state.current_mode = (state.current_mode + 1) % 4;
            desk.needs_full_redraw = true;
        }
    }

    // 3. Render Image based on current mode (strictly clipped to viewport)
    let (base_w, base_h) = if state.rotation_deg % 180 == 90 {
        (200usize, 320usize)
    } else {
        (320usize, 200usize)
    };
    let (target_w, target_h) = if state.fit_to_screen {
        let max_w = cw.saturating_sub(20).max(16);
        let max_h = vp_h.saturating_sub(20).max(16);
        if base_w * max_h <= base_h * max_w {
            ((base_w * max_h) / base_h, max_h)
        } else {
            (max_w, (base_h * max_w) / base_w)
        }
    } else {
        ((base_w * state.zoom_pct) / 100, (base_h * state.zoom_pct) / 100)
    };

    let img_x = cx as isize + (cw as isize - target_w as isize) / 2;
    let img_y = vp_y as isize + (vp_h as isize - target_h as isize) / 2;

    // Drop shadow around the image canvas only when strictly inside viewport bounds
    if target_w > 10
        && target_h > 10
        && img_x >= vp_x0 as isize
        && img_y >= vp_y0 as isize
        && (img_x as usize) + target_w + 6 <= vp_x1
        && (img_y as usize) + target_h + 8 <= vp_y1
    {
        draw_soft_drop_shadow(fb, pitch, scr_w, scr_h, img_x as usize, img_y as usize, target_w, target_h, 4, 6, 3, 40);
    }

    let mut loaded_from_vfs = false;
    let vfs_paths: Option<(&str, &str)> = match state.current_mode {
        1 => Some(("C:\\desenho.bmp", "/desenho.bmp")),
        2 => Some(("C:\\screenshot.bmp", "/screenshot.bmp")),
        _ => None,
    };

    if let Some((primary_path, fallback_path)) = vfs_paths {
        let file_res = vfs::vfs_read_all(primary_path)
            .or_else(|_| vfs::vfs_read_all(fallback_path));
        if let Ok(file_bytes) = file_res {
            if let Some(bmp) = BmpView::from_bytes(&file_bytes) {
                loaded_from_vfs = true;
                render_bmp_scaled_rotated(
                    fb, pitch, scr_w, scr_h,
                    img_x, img_y, target_w, target_h,
                    &bmp, state.rotation_deg,
                    vp_x0, vp_y0, vp_x1, vp_y1,
                );
            }
        }
    }

    if !loaded_from_vfs {
        if state.current_mode == 3 {
            // Radial RGB Test Pattern
            render_radial_rgb_pattern(
                fb, pitch, scr_w, scr_h,
                img_x, img_y, target_w, target_h,
                state.rotation_deg,
                vp_x0, vp_y0, vp_x1, vp_y1,
            );
        } else {
            // SMPTE/EBU Standard High-Definition Color Bar Pattern
            render_smpte_color_bars(
                fb, pitch, scr_w, scr_h,
                img_x, img_y, target_w, target_h,
                state.rotation_deg,
                vp_x0, vp_y0, vp_x1, vp_y1,
            );
        }
    }

    // 4. Bottom Glass Statusbar
    let sb_y = cy + ch.saturating_sub(24);
    let mode_desc = if loaded_from_vfs {
        "Imagem Decodificada do VFS"
    } else if state.current_mode == 3 {
        "Padrao de Teste Radial RGB"
    } else {
        "Padrao de Teste e Calibracao SMPTE"
    };
    let left_status = format!("Modo: {} | Zoom: {}% | Rotacao: {}*", mode_desc, state.zoom_pct, state.rotation_deg);
    let right_status = format!("Canvas: {}x{} px", target_w, target_h);
    draw_glass_statusbar(fb, pitch, scr_w, scr_h, cx, sb_y, cw, 24, &left_status, &right_status);
}

/// Renders SMPTE / EBU Standard Color Bars with resolution calibration lines,
/// rotation (0/90/180/270 deg), and strict viewport clipping.
unsafe fn render_smpte_color_bars(
    fb: *mut u8, pitch: usize, scr_w: usize, scr_h: usize,
    img_x: isize, img_y: isize, w: usize, h: usize,
    rot: usize,
    vp_x0: usize, vp_y0: usize, vp_x1: usize, vp_y1: usize,
) {
    let clip_x0 = vp_x0.min(scr_w);
    let clip_y0 = vp_y0.min(scr_h);
    let clip_x1 = vp_x1.min(scr_w);
    let clip_y1 = vp_y1.min(scr_h);
    if w < 10 || h < 10 || clip_x0 >= clip_x1 || clip_y0 >= clip_y1 {
        return;
    }

    let colors = [
        0x00FFFFFF, // White
        0x00FFFF00, // Yellow
        0x0000FFFF, // Cyan
        0x0000FF00, // Green
        0x00FF00FF, // Magenta
        0x00FF0000, // Red
        0x000000FF, // Blue
        0x00000000, // Black
    ];

    let draw_x0 = img_x.max(clip_x0 as isize) as usize;
    let draw_y0 = img_y.max(clip_y0 as isize) as usize;
    let draw_x1 = (img_x + w as isize).min(clip_x1 as isize).max(draw_x0 as isize) as usize;
    let draw_y1 = (img_y + h as isize).min(clip_y1 as isize).max(draw_y0 as isize) as usize;
    let rot_norm = rot % 360;

    for out_y in draw_y0..draw_y1 {
        let py = (out_y as isize - img_y) as usize;
        let row_ptr = fb.add(out_y * pitch) as *mut u32;
        let is_y_border = py == 0 || py + 1 == h;

        for out_x in draw_x0..draw_x1 {
            let px = (out_x as isize - img_x) as usize;
            if is_y_border || px == 0 || px + 1 == w {
                *row_ptr.add(out_x) = 0x00CBD5E1;
                continue;
            }

            // Map local (px, py) to rotated logical coordinates (u, v) in 0..1000
            let (u, v) = match rot_norm {
                90  => ((py * 1000) / h, ((w - 1 - px) * 1000) / w),
                180 => (((w - 1 - px) * 1000) / w, ((h - 1 - py) * 1000) / h),
                270 => (((h - 1 - py) * 1000) / h, (px * 1000) / w),
                _   => ((px * 1000) / w, (py * 1000) / h),
            };

            let col = if v < 670 {
                // Resolution calibration crosshair lines in center of primary bars
                if ((332..=338).contains(&v) && u >= 12 && u < 988)
                    || ((497..=503).contains(&u) && v >= 20 && v < 650)
                {
                    0x00000000
                } else {
                    let bar_idx = ((u * colors.len()) / 1000).min(colors.len() - 1);
                    colors[bar_idx]
                }
            } else {
                // Bottom grayscale ramp (16 steps)
                let step = ((u * 16) / 1000).min(15);
                let lum = (step * 255 / 15) as u32;
                (lum << 16) | (lum << 8) | lum
            };
            *row_ptr.add(out_x) = col;
        }
    }
}

/// Renders a radial RGB test pattern with rotation (0/90/180/270 deg) and strict viewport clipping.
unsafe fn render_radial_rgb_pattern(
    fb: *mut u8, pitch: usize, scr_w: usize, scr_h: usize,
    img_x: isize, img_y: isize, w: usize, h: usize,
    rot: usize,
    vp_x0: usize, vp_y0: usize, vp_x1: usize, vp_y1: usize,
) {
    let clip_x0 = vp_x0.min(scr_w);
    let clip_y0 = vp_y0.min(scr_h);
    let clip_x1 = vp_x1.min(scr_w);
    let clip_y1 = vp_y1.min(scr_h);
    if w < 10 || h < 10 || clip_x0 >= clip_x1 || clip_y0 >= clip_y1 {
        return;
    }

    let draw_x0 = img_x.max(clip_x0 as isize) as usize;
    let draw_y0 = img_y.max(clip_y0 as isize) as usize;
    let draw_x1 = (img_x + w as isize).min(clip_x1 as isize).max(draw_x0 as isize) as usize;
    let draw_y1 = (img_y + h as isize).min(clip_y1 as isize).max(draw_y0 as isize) as usize;
    let rot_norm = rot % 360;
    let max_r_sq = 500usize * 500 * 2;

    for out_y in draw_y0..draw_y1 {
        let py = (out_y as isize - img_y) as usize;
        let row_ptr = fb.add(out_y * pitch) as *mut u32;
        let is_y_border = py == 0 || py + 1 == h;

        for out_x in draw_x0..draw_x1 {
            let px = (out_x as isize - img_x) as usize;
            if is_y_border || px == 0 || px + 1 == w {
                *row_ptr.add(out_x) = 0x0038BDF8;
                continue;
            }

            let (u, v) = match rot_norm {
                90  => ((py * 1000) / h, ((w - 1 - px) * 1000) / w),
                180 => (((w - 1 - px) * 1000) / w, ((h - 1 - py) * 1000) / h),
                270 => (((h - 1 - py) * 1000) / h, (px * 1000) / w),
                _   => ((px * 1000) / w, (py * 1000) / h),
            };

            let du = u as isize - 500;
            let dv = v as isize - 500;
            let dist_sq = (du * du + dv * dv) as usize;
            let ratio = ((dist_sq * 255) / max_r_sq).min(255) as u32;

            let r = ratio;
            let g = 255u32.saturating_sub(ratio);
            let b = ((u * 255) / 1000).min(255) as u32;
            *row_ptr.add(out_x) = (r << 16) | (g << 8) | b;
        }
    }
}

/// Scales and rotates a BmpView onto the framebuffer viewport with strict viewport clipping.
unsafe fn render_bmp_scaled_rotated(
    fb: *mut u8, pitch: usize, scr_w: usize, scr_h: usize,
    dx: isize, dy: isize, dw: usize, dh: usize,
    bmp: &BmpView, rot: usize,
    vp_x0: usize, vp_y0: usize, vp_x1: usize, vp_y1: usize,
) {
    let src_w = bmp.width;
    let src_h = bmp.height;
    let clip_x0 = vp_x0.min(scr_w);
    let clip_y0 = vp_y0.min(scr_h);
    let clip_x1 = vp_x1.min(scr_w);
    let clip_y1 = vp_y1.min(scr_h);
    if src_w == 0 || src_h == 0 || dw == 0 || dh == 0 || clip_x0 >= clip_x1 || clip_y0 >= clip_y1 {
        return;
    }

    let draw_x0 = dx.max(clip_x0 as isize) as usize;
    let draw_y0 = dy.max(clip_y0 as isize) as usize;
    let draw_x1 = (dx + dw as isize).min(clip_x1 as isize).max(draw_x0 as isize) as usize;
    let draw_y1 = (dy + dh as isize).min(clip_y1 as isize).max(draw_y0 as isize) as usize;
    let rot_norm = rot % 360;

    for out_y in draw_y0..draw_y1 {
        let py = (out_y as isize - dy) as usize;
        let row_ptr = fb.add(out_y * pitch) as *mut u32;
        let is_y_border = py == 0 || py + 1 == dh;

        for out_x in draw_x0..draw_x1 {
            let px = (out_x as isize - dx) as usize;
            if is_y_border || px == 0 || px + 1 == dw {
                *row_ptr.add(out_x) = 0x007DD3FC;
                continue;
            }

            // Apply rotation mapping
            let (u, v) = match rot_norm {
                90  => (py * src_w / dh, (dw - 1 - px) * src_h / dw),
                180 => ((dw - 1 - px) * src_w / dw, (dh - 1 - py) * src_h / dh),
                270 => ((dh - 1 - py) * src_w / dh, px * src_h / dw),
                _   => (px * src_w / dw, py * src_h / dh),
            };

            if let Some(argb) = bmp.get_pixel(u.min(src_w - 1), v.min(src_h - 1)) {
                *row_ptr.add(out_x) = argb & 0x00FFFFFF;
            }
        }
    }
}

fn handle_photo_viewer_click(cx: usize, cy: usize, desk: &mut DesktopState) -> bool {
    let mut state = PHOTO_VIEWER_STATE.lock();
    state.pending_click = Some((cx, cy));
    desk.needs_full_redraw = true;
    true
}

// =============================================================================
// 43. ESTÚDIO DE PINTURA (Paint & Pixel Canvas)
// =============================================================================

unsafe fn render_paint_studio(
    fb: *mut u8, pitch: usize, scr_w: usize, scr_h: usize,
    cx: usize, cy: usize, cw: usize, ch: usize,
    desk: &mut DesktopState,
) {
    let mut state = PAINT_STATE.lock();
    if !state.initialized {
        state.canvas.fill(0x00FFFFFF);
        state.initialized = true;
    }

    let (mx, my, mouse_down) = {
        let m = MOUSE_STATE.lock();
        (m.x, m.y, m.left_button)
    };
    let pending = state.pending_click.take();
    let (btn_mx, btn_my, btn_click) = match pending {
        Some((px, py)) => (px as i32, py as i32, true),
        None => (mx, my, false),
    };

    // 1. Two-Row Top Glass Toolbar (66px) — all controls fit cleanly inside cw = 672 (cx..cx + cw - 6)
    let toolbar_h = 66usize;
    draw_glass_toolbar(fb, pitch, scr_w, scr_h, cx, cy, cw, toolbar_h);

    // Row 1: 6 Tool Selector Buttons (right edge: cx + 8 + 76+6+76+6+76+6+80+6+76+6+86 = cx + 508 <= cx + 666)
    let tools = [
        ("Lapis", PaintTool::Pencil, 76usize),
        ("Pincel", PaintTool::Brush, 76usize),
        ("Linha", PaintTool::Line, 76usize),
        ("Retang.", PaintTool::Rectangle, 80usize),
        ("Balde", PaintTool::Bucket, 76usize),
        ("Borracha", PaintTool::Eraser, 86usize),
    ];

    let mut bx = cx + 8;
    for &(name, tool, bw) in tools.iter() {
        let is_selected = state.selected_tool == tool;
        let style = if is_selected { ButtonStyle::PrimaryAction } else { ButtonStyle::StandardGlass };
        if ui_glass_button(fb, pitch, scr_w, scr_h, bx, cy + 5, bw, 26, name, style, btn_mx, btn_my, btn_click) {
            state.selected_tool = tool;
            desk.needs_full_redraw = true;
        }
        bx += bw + 6;
    }

    // Row 2: 4 Stroke Width Buttons [1px | 2px | 4px | 8px] + Action Buttons ["Limpar" | "Salvar BMP"]
    // Right edge: cx + 8 + 4*(44+6) + 12 + 76 + 8 + 112 = cx + 416 <= cx + 666
    let mut bx2 = cx + 8;
    for &w_val in &[1usize, 2, 4, 8] {
        let is_w_sel = state.stroke_width == w_val;
        let style = if is_w_sel { ButtonStyle::PrimaryAction } else { ButtonStyle::StandardGlass };
        let w_label = format!("{}px", w_val);
        if ui_glass_button(fb, pitch, scr_w, scr_h, bx2, cy + 35, 44, 26, &w_label, style, btn_mx, btn_my, btn_click) {
            state.stroke_width = w_val;
            desk.needs_full_redraw = true;
        }
        bx2 += 50;
    }

    bx2 += 12;
    if ui_glass_button(fb, pitch, scr_w, scr_h, bx2, cy + 35, 76, 26, "Limpar", ButtonStyle::StandardGlass, btn_mx, btn_my, btn_click) {
        state.canvas.fill(0x00FFFFFF);
        state.status_msg = String::from("Canvas limpo!");
        state.status_timer = 40;
        desk.needs_full_redraw = true;
    }
    bx2 += 84;

    if ui_glass_button(fb, pitch, scr_w, scr_h, bx2, cy + 35, 112, 26, "Salvar BMP", ButtonStyle::PrimaryAction, btn_mx, btn_my, btn_click) {
        let bmp_bytes = encode_bmp_24bpp(CANVAS_W, CANVAS_H, &state.canvas);
        let _ = vfs::vfs_unlink("C:\\desenho.bmp");
        let save_res = vfs::vfs_create_file("C:\\desenho.bmp", &bmp_bytes, vfs::inode::Permissions::from_mode(0o777))
            .or_else(|_| {
                let _ = vfs::vfs_unlink("/desenho.bmp");
                vfs::vfs_create_file("/desenho.bmp", &bmp_bytes, vfs::inode::Permissions::from_mode(0o777))
            });

        match save_res {
            Ok(_) => {
                state.status_msg = format!("Gravado: C:\\desenho.bmp ({} KB)", bmp_bytes.len() / 1024);
                play_ui_feedback_tone(1046, 35);
            }
            Err(_) => {
                state.status_msg = String::from("Erro ao gravar arquivo no VFS");
            }
        }
        state.status_timer = 60;
        desk.needs_full_redraw = true;
    }

    // 2. Left Swatch Palette (16 Colors) - 44px width strip
    let palette_w = 44usize;
    let body_y = cy + toolbar_h;
    let body_h = ch.saturating_sub(toolbar_h + 24);
    fill_rect(fb, pitch, scr_w, scr_h, cx, body_y, palette_w, body_h, 0x001E293B);
    draw_rounded_rect_border(fb, pitch, scr_w, scr_h, cx, body_y, palette_w, body_h, 0, 0x00334155);

    // Render 16 color swatch capsules (edge-triggered click selection)
    for i in 0..16 {
        let col_x = cx + 6 + (i % 2) * 16;
        let col_y = body_y + 8 + (i / 2) * 18;
        let col = PAINT_PALETTE_16[i];
        let is_sel = state.selected_color_idx == i;

        fill_rounded_rect(fb, pitch, scr_w, scr_h, col_x, col_y, 14, 14, 3, col);
        draw_rounded_rect_border(fb, pitch, scr_w, scr_h, col_x, col_y, 14, 14, 3, if is_sel { 0x0038BDF8 } else { 0x0064748B });

        if let Some((px, py)) = pending {
            if px >= col_x && px < col_x + 14 && py >= col_y && py < col_y + 14 {
                state.selected_color_idx = i;
                desk.needs_full_redraw = true;
            }
        }
    }

    // Large preview swatch of the currently selected color
    let cur_col = PAINT_PALETTE_16[state.selected_color_idx];
    let prev_y = body_y + 8 + 8 * 18 + 8;
    fill_rounded_rect(fb, pitch, scr_w, scr_h, cx + 8, prev_y, 28, 28, 4, cur_col);
    draw_rounded_rect_border(fb, pitch, scr_w, scr_h, cx + 8, prev_y, 28, 28, 4, 0x00FFFFFF);

    // 3. Central Drawing Canvas Area
    let canvas_box_x = cx + palette_w + 12;
    let canvas_box_y = body_y + 10;
    let canvas_box_w = (cw.saturating_sub(palette_w + 24)).min(CANVAS_W);
    let canvas_box_h = body_h.saturating_sub(20).min(CANVAS_H);

    // Drop shadow under canvas
    draw_soft_drop_shadow(fb, pitch, scr_w, scr_h, canvas_box_x, canvas_box_y, canvas_box_w, canvas_box_h, 4, 4, 2, 35);

    // Mouse interactive drawing directly on canvas pixels:
    // - Bucket triggers strictly once on edge click (pending)
    // - Pencil, Brush, Eraser, Line, Rectangle support continuous mouse_down drag while focused
    let allow_drag = mouse_down && desk.focused_app.as_u8() == 43 && desk.dragging_window.is_none();
    let draw_col = if state.selected_tool == PaintTool::Eraser { 0x00FFFFFF } else { cur_col };
    let radius = match state.stroke_width {
        1 => 0,
        2 => 1,
        4 => 2,
        _ => 4,
    };

    if state.selected_tool == PaintTool::Bucket {
        if let Some((px, py)) = pending {
            if px >= canvas_box_x && px < canvas_box_x + canvas_box_w
                && py >= canvas_box_y && py < canvas_box_y + canvas_box_h
            {
                let cur_px = px - canvas_box_x;
                let cur_py = py - canvas_box_y;
                flood_fill_canvas(&mut state.canvas, CANVAS_W, CANVAS_H, cur_px, cur_py, draw_col);
                desk.needs_full_redraw = true;
            }
        }
        state.last_paint_pos = None;
        state.shape_start_pos = None;
    } else if allow_drag {
        if mx >= canvas_box_x as i32 && mx < (canvas_box_x + canvas_box_w) as i32
            && my >= canvas_box_y as i32 && my < (canvas_box_y + canvas_box_h) as i32
        {
            let cur_px = (mx - canvas_box_x as i32) as usize;
            let cur_py = (my - canvas_box_y as i32) as usize;

            match state.selected_tool {
                PaintTool::Pencil => {
                    if let Some((lx, ly)) = state.last_paint_pos {
                        paint_line_bresenham(&mut state.canvas, CANVAS_W, CANVAS_H, lx, ly, cur_px, cur_py, draw_col, 0);
                    } else {
                        paint_stamp(&mut state.canvas, CANVAS_W, CANVAS_H, cur_px, cur_py, draw_col, 0);
                    }
                }
                PaintTool::Brush | PaintTool::Eraser => {
                    if let Some((lx, ly)) = state.last_paint_pos {
                        paint_line_bresenham(&mut state.canvas, CANVAS_W, CANVAS_H, lx, ly, cur_px, cur_py, draw_col, radius);
                    } else {
                        paint_stamp(&mut state.canvas, CANVAS_W, CANVAS_H, cur_px, cur_py, draw_col, radius);
                    }
                }
                PaintTool::Line | PaintTool::Rectangle => {
                    if state.shape_start_pos.is_none() {
                        state.shape_start_pos = Some((cur_px, cur_py));
                    }
                }
                PaintTool::Bucket => {}
            }
            state.last_paint_pos = Some((cur_px, cur_py));
        } else {
            state.last_paint_pos = None;
        }
    } else {
        // Finalize Line or Rectangle on mouse release
        if let Some((sx, sy)) = state.shape_start_pos {
            if let Some((ex, ey)) = state.last_paint_pos {
                if state.selected_tool == PaintTool::Line {
                    paint_line_bresenham(&mut state.canvas, CANVAS_W, CANVAS_H, sx, sy, ex, ey, cur_col, 1);
                } else if state.selected_tool == PaintTool::Rectangle {
                    paint_rect(&mut state.canvas, CANVAS_W, CANVAS_H, sx, sy, ex, ey, cur_col);
                }
            }
            state.shape_start_pos = None;
        }
        state.last_paint_pos = None;
    }

    // Fill canvas area with WHITE (0x00FFFFFF) before blitting to ensure clean background
    fill_rect(fb, pitch, scr_w, scr_h, canvas_box_x, canvas_box_y, canvas_box_w, canvas_box_h, 0x00FFFFFF);

    // Blit internal canvas buffer to GOP Framebuffer
    for py in 0..canvas_box_h {
        let out_y = canvas_box_y + py;
        if out_y >= scr_h { break; }
        let row_ptr = fb.add(out_y * pitch) as *mut u32;

        for px in 0..canvas_box_w {
            let out_x = canvas_box_x + px;
            if out_x >= scr_w { break; }
            *row_ptr.add(out_x) = state.canvas[py * CANVAS_W + px];
        }
    }
    draw_rounded_rect_border(fb, pitch, scr_w, scr_h, canvas_box_x, canvas_box_y, canvas_box_w, canvas_box_h, 2, 0x000284C7);

    // 4. Bottom Glass Statusbar
    let sb_y = cy + ch.saturating_sub(24);
    let tool_name = match state.selected_tool {
        PaintTool::Pencil => "Lapis",
        PaintTool::Brush => "Pincel",
        PaintTool::Line => "Linha",
        PaintTool::Rectangle => "Retangulo",
        PaintTool::Bucket => "Balde de Tinta",
        PaintTool::Eraser => "Borracha",
    };
    let l_status = if state.status_timer > 0 {
        state.status_timer -= 1;
        state.status_msg.clone()
    } else {
        format!("Ferramenta: {} | Espessura: {}px | 320x200 24-bpp", tool_name, state.stroke_width)
    };
    draw_glass_statusbar(fb, pitch, scr_w, scr_h, cx, sb_y, cw, 24, &l_status, "VFS: C:\\desenho.bmp");
}

fn paint_stamp(canvas: &mut [u32], w: usize, h: usize, cx: usize, cy: usize, col: u32, radius: usize) {
    if radius == 0 {
        if cx < w && cy < h {
            canvas[cy * w + cx] = col;
        }
        return;
    }
    let r = radius as isize;
    let r_sq = r * r;
    let min_x = (cx as isize - r).max(0) as usize;
    let max_x = (cx as isize + r).min(w as isize - 1) as usize;
    let min_y = (cy as isize - r).max(0) as usize;
    let max_y = (cy as isize + r).min(h as isize - 1) as usize;

    for py in min_y..=max_y {
        let dy = py as isize - cy as isize;
        for px in min_x..=max_x {
            let dx = px as isize - cx as isize;
            if dx * dx + dy * dy <= r_sq {
                canvas[py * w + px] = col;
            }
        }
    }
}

fn paint_line_bresenham(canvas: &mut [u32], w: usize, h: usize, x0: usize, y0: usize, x1: usize, y1: usize, col: u32, radius: usize) {
    let mut x = x0 as isize;
    let mut y = y0 as isize;
    let x_end = x1 as isize;
    let y_end = y1 as isize;

    let dx = (x_end - x).abs();
    let sx = if x < x_end { 1 } else { -1 };
    let dy = -(y_end - y).abs();
    let sy = if y < y_end { 1 } else { -1 };
    let mut err = dx + dy;

    loop {
        if x >= 0 && (x as usize) < w && y >= 0 && (y as usize) < h {
            paint_stamp(canvas, w, h, x as usize, y as usize, col, radius);
        }
        if x == x_end && y == y_end { break; }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}

fn paint_rect(canvas: &mut [u32], w: usize, h: usize, x0: usize, y0: usize, x1: usize, y1: usize, col: u32) {
    let min_x = x0.min(x1).min(w.saturating_sub(1));
    let max_x = x0.max(x1).min(w.saturating_sub(1));
    let min_y = y0.min(y1).min(h.saturating_sub(1));
    let max_y = y0.max(y1).min(h.saturating_sub(1));

    for py in min_y..=max_y {
        for px in min_x..=max_x {
            canvas[py * w + px] = col;
        }
    }
}

fn flood_fill_canvas(canvas: &mut [u32], w: usize, h: usize, start_x: usize, start_y: usize, fill_col: u32) {
    if start_x >= w || start_y >= h { return; }
    let target_col = canvas[start_y * w + start_x];
    if target_col == fill_col { return; }

    let mut queue = Vec::with_capacity(1024);
    queue.push((start_x, start_y));
    canvas[start_y * w + start_x] = fill_col;

    let mut iterations = 0;
    while let Some((x, y)) = queue.pop() {
        iterations += 1;
        if iterations > 65_000 { break; } // Safe guard for bare metal ring 0 execution

        if x > 0 && canvas[y * w + (x - 1)] == target_col {
            canvas[y * w + (x - 1)] = fill_col;
            queue.push((x - 1, y));
        }
        if x + 1 < w && canvas[y * w + (x + 1)] == target_col {
            canvas[y * w + (x + 1)] = fill_col;
            queue.push((x + 1, y));
        }
        if y > 0 && canvas[(y - 1) * w + x] == target_col {
            canvas[(y - 1) * w + x] = fill_col;
            queue.push((x, y - 1));
        }
        if y + 1 < h && canvas[(y + 1) * w + x] == target_col {
            canvas[(y + 1) * w + x] = fill_col;
            queue.push((x, y + 1));
        }
    }
}

fn handle_paint_studio_click(cx: usize, cy: usize, desk: &mut DesktopState) -> bool {
    let mut state = PAINT_STATE.lock();
    state.pending_click = Some((cx, cy));
    desk.needs_full_redraw = true;
    true
}

// =============================================================================
// 44. FERRAMENTA DE CAPTURA DE TELA (Snipping Tool)
// =============================================================================

unsafe fn render_snipping_tool(
    fb: *mut u8, pitch: usize, scr_w: usize, scr_h: usize,
    cx: usize, cy: usize, cw: usize, ch: usize,
    desk: &mut DesktopState,
) {
    let mut state = SNIP_STATE.lock();

    let (mx, my) = {
        let m = MOUSE_STATE.lock();
        (m.x, m.y)
    };
    let pending = state.pending_click.take();
    let (btn_mx, btn_my, btn_click) = match pending {
        Some((px, py)) => (px as i32, py as i32, true),
        None => (mx, my, false),
    };

    // 1. Top Glass Toolbar (38px)
    draw_glass_toolbar(fb, pitch, scr_w, scr_h, cx, cy, cw, 38);

    let mut bx = cx + 8;
    let mode_str = match state.mode {
        SnipMode::FullScreen => "Tela Inteira",
        SnipMode::ActiveWindow => "Janela Ativa",
        SnipMode::Rectangular => "Selecao Retang.",
    };
    if ui_glass_button(fb, pitch, scr_w, scr_h, bx, cy + 6, 120, 26, mode_str, ButtonStyle::StandardGlass, btn_mx, btn_my, btn_click) {
        state.mode = match state.mode {
            SnipMode::FullScreen => SnipMode::ActiveWindow,
            SnipMode::ActiveWindow => SnipMode::Rectangular,
            SnipMode::Rectangular => SnipMode::FullScreen,
        };
        desk.needs_full_redraw = true;
    }
    bx += 128;

    // "Novo Snip" Button
    if ui_glass_button(fb, pitch, scr_w, scr_h, bx, cy + 6, 96, 26, "✂ Novo Snip", ButtonStyle::PrimaryAction, btn_mx, btn_my, btn_click) {
        // Read real pixels directly from the GOP Framebuffer fb
        let (snip_x, snip_y, snip_w, snip_h) = match state.mode {
            SnipMode::FullScreen => (0, 0, scr_w, scr_h),
            SnipMode::ActiveWindow => (cx.saturating_sub(4), cy.saturating_sub(32), cw + 8, ch + 36),
            SnipMode::Rectangular => (scr_w / 4, scr_h / 4, scr_w / 2, scr_h / 2),
        };

        state.cap_w = snip_w;
        state.cap_h = snip_h;

        // Downsample to preview thumbnail (160x100)
        for ty in 0..100 {
            let src_y = (snip_y + (ty * snip_h) / 100).min(scr_h.saturating_sub(1));
            let row_ptr = fb.add(src_y * pitch) as *const u32;
            for tx in 0..160 {
                let src_x = (snip_x + (tx * snip_w) / 160).min(scr_w.saturating_sub(1));
                state.preview_thumb[ty * 160 + tx] = *row_ptr.add(src_x);
            }
        }

        state.has_capture = true;
        state.snip_count += 1;
        state.status_msg = format!("Snip #{} capturado: {}x{} px", state.snip_count, snip_w, snip_h);
        play_ui_feedback_tone(880, 25);
        desk.needs_full_redraw = true;
    }
    bx += 104;

    // "Salvar Captura" Button
    if ui_glass_button(fb, pitch, scr_w, scr_h, bx, cy + 6, 120, 26, "💾 Salvar Captura", ButtonStyle::StandardGlass, btn_mx, btn_my, btn_click) {
        if state.has_capture {
            // Encode the 160x100 capture to standard BMP and overwrite any previous file
            let bmp_data = encode_bmp_24bpp(160, 100, &state.preview_thumb);
            let _ = vfs::vfs_unlink("C:\\screenshot.bmp");
            let save_res = vfs::vfs_create_file("C:\\screenshot.bmp", &bmp_data, vfs::inode::Permissions::from_mode(0o777))
                .or_else(|_| {
                    let _ = vfs::vfs_unlink("/screenshot.bmp");
                    vfs::vfs_create_file("/screenshot.bmp", &bmp_data, vfs::inode::Permissions::from_mode(0o777))
                });

            match save_res {
                Ok(_) => {
                    state.status_msg = format!("Salvo em C:\\screenshot.bmp ({} KB)", bmp_data.len() / 1024);
                    play_ui_feedback_tone(1046, 35);
                }
                Err(_) => {
                    state.status_msg = String::from("Erro ao salvar captura no VFS");
                }
            }
        } else {
            state.status_msg = String::from("Nenhum snip capturado ainda.");
        }
        desk.needs_full_redraw = true;
    }

    // 2. Central Preview Panel (Vitrified Frame with Specular Highlights)
    let body_y = cy + 38;
    let body_h = ch.saturating_sub(38 + 24);
    fill_rect(fb, pitch, scr_w, scr_h, cx, body_y, cw, body_h, 0x001E293B);

    let prev_w = 320usize;
    let prev_h = 200usize;
    let prev_x = cx + (cw.saturating_sub(prev_w)) / 2;
    let prev_y = body_y + 16;

    draw_soft_drop_shadow(fb, pitch, scr_w, scr_h, prev_x, prev_y, prev_w, prev_h, 6, 8, 4, 45);
    fill_rounded_rect(fb, pitch, scr_w, scr_h, prev_x, prev_y, prev_w, prev_h, 6, 0x001E293B);

    if state.has_capture {
        // Blit 160x100 thumbnail scaled 2x to 320x200
        for ty in 0..prev_h {
            let out_y = prev_y + ty;
            if out_y >= scr_h { break; }
            let row_ptr = fb.add(out_y * pitch) as *mut u32;
            let src_y = ty / 2;

            for tx in 0..prev_w {
                let out_x = prev_x + tx;
                if out_x >= scr_w { break; }
                let src_x = tx / 2;
                *row_ptr.add(out_x) = state.preview_thumb[src_y * 160 + src_x];
            }
        }
    } else {
        draw_text_px(fb, pitch, scr_w, scr_h, prev_x + 50, prev_y + 90, "Clique em 'Novo Snip' para capturar", 0x0094A3B8, None);
    }
    draw_rounded_rect_border(fb, pitch, scr_w, scr_h, prev_x, prev_y, prev_w, prev_h, 6, 0x0038BDF8);

    // 3. Technical Metadata Box below preview
    let meta_y = prev_y + prev_h + 12;
    draw_text_px(fb, pitch, scr_w, scr_h, cx + 24, meta_y, "Resolucao: 1280x800 GOP | 32-bpp ARGB -> 24-bpp BMP", 0x00E2E8F0, None);
    draw_text_px(fb, pitch, scr_w, scr_h, cx + 24, meta_y + 20, "Destino no VFS: C:\\screenshot.bmp | Driver: Intel GOP Ring 0", 0x0038BDF8, None);

    // 4. Statusbar
    let sb_y = cy + ch.saturating_sub(24);
    let stat = if state.status_msg.is_empty() { "Pronto para Captura" } else { &state.status_msg };
    draw_glass_statusbar(fb, pitch, scr_w, scr_h, cx, sb_y, cw, 24, stat, "Zero Mocks Framebuffer");
}

fn handle_snipping_tool_click(cx: usize, cy: usize, desk: &mut DesktopState) -> bool {
    let mut state = SNIP_STATE.lock();
    state.pending_click = Some((cx, cy));
    desk.needs_full_redraw = true;
    true
}

// =============================================================================
// 45. REPRODUTOR DE MÍDIA (Crystal Media Player)
// =============================================================================

unsafe fn render_crystal_media_player(
    fb: *mut u8, pitch: usize, scr_w: usize, scr_h: usize,
    cx: usize, cy: usize, cw: usize, ch: usize,
    desk: &mut DesktopState,
) {
    let mut state = MEDIA_STATE.lock();
    state.anim_tick = state.anim_tick.wrapping_add(1);

    let (mx, my, mouse_down) = {
        let m = MOUSE_STATE.lock();
        (m.x, m.y, m.left_button)
    };
    let pending = state.pending_click.take();
    let (btn_mx, btn_my, btn_click) = match pending {
        Some((px, py)) => (px as i32, py as i32, true),
        None => (mx, my, false),
    };

    // Dark Vitrified Player Body
    fill_rounded_rect_gradient(fb, pitch, scr_w, scr_h, cx, cy, cw, ch, 6, 0x001E293B, 0x00334155);
    draw_rounded_rect_border(fb, pitch, scr_w, scr_h, cx, cy, cw, ch, 6, 0x0038BDF8);

    let track = &MEDIA_TRACKS[state.current_track];

    // Track Title and Artist Header
    draw_text_px(fb, pitch, scr_w, scr_h, cx + 20, cy + 14, track.title, 0x00FFFFFF, None);
    draw_text_px(fb, pitch, scr_w, scr_h, cx + 20, cy + 34, track.artist, 0x0038BDF8, None);

    // 1. Visor de Espectro Sonoro Animado de 16 Barras com Phosphor Glow
    let spec_x = cx + 20;
    let spec_y = cy + 60;
    let spec_w = cw.saturating_sub(40);
    let spec_h = (ch.saturating_sub(180)).max(70);

    // Spectrum display dark glass frame
    fill_rounded_rect(fb, pitch, scr_w, scr_h, spec_x, spec_y, spec_w, spec_h, 4, 0x001E293B);
    draw_rounded_rect_border(fb, pitch, scr_w, scr_h, spec_x, spec_y, spec_w, spec_h, 4, 0x001E3A8A);

    let bar_spacing = (spec_w.saturating_sub(16)) / 16;
    let bar_w = (bar_spacing.saturating_sub(4)).max(3);
    let max_bar_h = spec_h.saturating_sub(16);

    // Update real-time harmonic animation
    for i in 0..16 {
        if state.is_playing {
            // Algorithmic spectrum bounce with harmonic decay
            let phase = (state.anim_tick * 4 + i * 25) % 100;
            let h_val = if phase < 50 { phase * 2 } else { (100 - phase) * 2 };
            let raw_h = (h_val * max_bar_h) / 100;
            state.spectrum_heights[i] = raw_h.min(255) as u8;

            if state.spectrum_heights[i] >= state.spectrum_peaks[i] {
                state.spectrum_peaks[i] = state.spectrum_heights[i];
            } else if state.spectrum_peaks[i] > 0 && state.anim_tick % 2 == 0 {
                state.spectrum_peaks[i] -= 1;
            }
        } else {
            state.spectrum_heights[i] = 0;
            if state.spectrum_peaks[i] > 0 { state.spectrum_peaks[i] -= 1; }
        }

        let bx = spec_x + 8 + i * bar_spacing;
        let bh = (state.spectrum_heights[i] as usize).min(max_bar_h);

        // Render bar with Phosphor Glow color gradient
        for step_y in 0..bh {
            let py = spec_y + spec_h - 6 - step_y;
            let ratio = (step_y * 100) / max_bar_h.max(1);
            let col = if ratio < 45 {
                0x0006B6D4 // Glacial Cyan base
            } else if ratio < 75 {
                0x0010B981 // Emerald Green mid
            } else {
                0x00EF4444 // Ruby Red peak glow
            };
            fill_rect(fb, pitch, scr_w, scr_h, bx, py, bar_w, 1, col);
        }

        // Phosphor Peak Hold floating tick (white/gold)
        let peak_h = (state.spectrum_peaks[i] as usize).min(max_bar_h);
        let peak_y = spec_y + spec_h - 6 - peak_h;
        fill_rect(fb, pitch, scr_w, scr_h, bx, peak_y, bar_w, 2, 0x00FBBF24);
    }

    // Play synthesized musical tone to Intel HDA if playing
    if state.is_playing && state.anim_tick % 15 == 0 {
        let note_idx = state.note_step % track.notes.len();
        let (freq, dur) = track.notes[note_idx];
        play_ui_feedback_tone(freq, dur.min(45));
        state.note_step += 1;
        state.current_sec = (state.current_sec + 1) % track.duration_sec.max(1);
    }

    // 2. Temporal Progress Bar with continuous mouse dragging / seek
    let prog_y = spec_y + spec_h + 16;
    let prog_w = spec_w.max(1);
    let cur_pct = (state.current_sec * 100) / track.duration_sec.max(1);

    // Timeline background track
    fill_rounded_rect(fb, pitch, scr_w, scr_h, spec_x, prog_y + 4, prog_w, 6, 3, 0x00334155);

    // Liquid filled portion
    let fill_w = (prog_w * cur_pct) / 100;
    if fill_w > 0 {
        fill_rounded_rect_gradient(fb, pitch, scr_w, scr_h, spec_x, prog_y + 4, fill_w, 6, 3, 0x0038BDF8, 0x000284C7);
    }

    // Draggable glass knob
    let knob_x = spec_x + fill_w.saturating_sub(6);
    fill_rounded_rect(fb, pitch, scr_w, scr_h, knob_x, prog_y, 14, 14, 7, 0x00FFFFFF);
    draw_rounded_rect_border(fb, pitch, scr_w, scr_h, knob_x, prog_y, 14, 14, 7, 0x000284C7);

    // Click/drag to seek
    let allow_drag = mouse_down && desk.focused_app.as_u8() == 45 && desk.dragging_window.is_none();
    let (seek_mx, seek_my, seek_active) = if allow_drag {
        (mx, my, true)
    } else if let Some((px, py)) = pending {
        (px as i32, py as i32, true)
    } else {
        (mx, my, false)
    };
    if seek_active && seek_mx >= spec_x as i32 && seek_mx <= (spec_x + prog_w) as i32 && seek_my >= prog_y as i32 && seek_my <= (prog_y + 16) as i32 {
        let rel_x = (seek_mx - spec_x as i32).max(0) as usize;
        let new_sec = (rel_x * track.duration_sec) / prog_w;
        state.current_sec = new_sec.min(track.duration_sec);
        desk.needs_full_redraw = true;
    }

    // Time counters: 01:24 / 02:48
    let cur_str = format!("{:02}:{:02} / {:02}:{:02}", state.current_sec / 60, state.current_sec % 60, track.duration_sec / 60, track.duration_sec % 60);
    draw_text_px(fb, pitch, scr_w, scr_h, cx + cw.saturating_sub(140), prog_y + 20, &cur_str, 0x00CBD5E1, None);

    // 3. Transport Buttons Strip: [ ◄◄ | ▶ Play | ❚❚ Pause | ■ Stop | ►► ] (edge-triggered)
    let ctrl_y = prog_y + 22;
    let mut bx = spec_x;

    // Previous Track ◄◄
    if ui_glass_button(fb, pitch, scr_w, scr_h, bx, ctrl_y, 56, 28, "<<", ButtonStyle::StandardGlass, btn_mx, btn_my, btn_click) {
        state.current_track = if state.current_track == 0 { MEDIA_TRACKS.len() - 1 } else { state.current_track - 1 };
        state.current_sec = 0;
        desk.needs_full_redraw = true;
    }
    bx += 62;

    // Play / Pause
    let play_label = if state.is_playing { "Pause" } else { "Play" };
    let play_style = if state.is_playing { ButtonStyle::PrimaryAction } else { ButtonStyle::StandardGlass };
    if ui_glass_button(fb, pitch, scr_w, scr_h, bx, ctrl_y, 74, 28, play_label, play_style, btn_mx, btn_my, btn_click) {
        state.is_playing = !state.is_playing;
        if state.is_playing {
            play_ui_feedback_tone(523, 40);
        }
        desk.needs_full_redraw = true;
    }
    bx += 80;

    // Stop ■
    if ui_glass_button(fb, pitch, scr_w, scr_h, bx, ctrl_y, 56, 28, "Stop", ButtonStyle::StandardGlass, btn_mx, btn_my, btn_click) {
        state.is_playing = false;
        state.current_sec = 0;
        desk.needs_full_redraw = true;
    }
    bx += 62;

    // Next Track ►►
    if ui_glass_button(fb, pitch, scr_w, scr_h, bx, ctrl_y, 56, 28, ">>", ButtonStyle::StandardGlass, btn_mx, btn_my, btn_click) {
        state.current_track = (state.current_track + 1) % MEDIA_TRACKS.len();
        state.current_sec = 0;
        desk.needs_full_redraw = true;
    }
}

fn handle_media_player_click(cx: usize, cy: usize, desk: &mut DesktopState) -> bool {
    let mut state = MEDIA_STATE.lock();
    state.pending_click = Some((cx, cy));
    desk.needs_full_redraw = true;
    true
}

// =============================================================================
// 46. MIXER DE VOLUME HDA
// =============================================================================

unsafe fn render_hda_mixer(
    fb: *mut u8, pitch: usize, scr_w: usize, scr_h: usize,
    cx: usize, cy: usize, cw: usize, ch: usize,
    desk: &mut DesktopState,
) {
    let mut state = HDA_MIXER_STATE.lock();
    state.anim_tick = state.anim_tick.wrapping_add(1);

    let (mx, my, mouse_down) = {
        let m = MOUSE_STATE.lock();
        (m.x, m.y, m.left_button)
    };
    let pending = state.pending_click.take();
    let (btn_mx, btn_my, btn_click) = match pending {
        Some((px, py)) => (px as i32, py as i32, true),
        None => (mx, my, false),
    };
    let allow_drag = mouse_down && desk.focused_app.as_u8() == 46 && desk.dragging_window.is_none();
    let (sld_mx, sld_my, sld_active) = if allow_drag {
        (mx, my, true)
    } else if let Some((px, py)) = pending {
        (px as i32, py as i32, true)
    } else {
        (mx, my, false)
    };

    // Dark Glass Panel Background
    fill_rounded_rect_gradient(fb, pitch, scr_w, scr_h, cx, cy, cw, ch, 6, 0x001E293B, 0x00334155);
    draw_rounded_rect_border(fb, pitch, scr_w, scr_h, cx, cy, cw, ch, 6, 0x0038BDF8);

    // Header Title
    draw_text_px(fb, pitch, scr_w, scr_h, cx + 18, cy + 12, "Mixer de Audio Intel HDA (8086:2668)", 0x00FFFFFF, None);
    draw_text_px(fb, pitch, scr_w, scr_h, cx + 18, cy + 30, "Codec Realtek ALC - Barramento PCI Ring 0 Long Mode", 0x0094A3B8, None);

    // 4 Vertical Channels: Master, System, Wave, Synth
    let channel_w = 95usize;
    let slider_h = 120usize;
    let slider_y = cy + 60;

    let channels: [(&str, u8, bool); 4] = [
        ("Master", state.vol_master, state.mute_master),
        ("Sistema", state.vol_system, state.mute_system),
        ("Wave", state.vol_wave, state.mute_wave),
        ("Sintetiz.", state.vol_synth, state.mute_synth),
    ];

    for (idx, (name, vol_val, is_muted)) in channels.iter().enumerate() {
        let col_x = cx + 20 + idx * (channel_w + 14);

        // Channel container box
        fill_rounded_rect(fb, pitch, scr_w, scr_h, col_x, slider_y, channel_w, slider_h + 90, 4, 0x001E293B);
        draw_rounded_rect_border(fb, pitch, scr_w, scr_h, col_x, slider_y, channel_w, slider_h + 90, 4, 0x00334155);

        // Name label
        draw_text_px(fb, pitch, scr_w, scr_h, col_x + 12, slider_y + 8, name, 0x00F8FAFC, None);

        // Vertical Track (8px wide)
        let track_x = col_x + channel_w / 2 - 4;
        let track_y = slider_y + 32;
        fill_rounded_rect(fb, pitch, scr_w, scr_h, track_x, track_y, 8, slider_h, 4, 0x001E293B);

        // Liquid filled bar up to current volume
        let fill_h = (*vol_val as usize * slider_h) / 100;
        if fill_h > 0 && !*is_muted {
            let fill_y = track_y + slider_h - fill_h;
            fill_rounded_rect_gradient(fb, pitch, scr_w, scr_h, track_x, fill_y, 8, fill_h, 4, 0x0038BDF8, 0x000284C7);
        }

        // Draggable Knob
        let knob_y = track_y + slider_h.saturating_sub(fill_h).saturating_sub(8);
        fill_rounded_rect(fb, pitch, scr_w, scr_h, track_x - 6, knob_y, 20, 14, 6, 0x00FFFFFF);
        draw_rounded_rect_border(fb, pitch, scr_w, scr_h, track_x - 6, knob_y, 20, 14, 6, 0x000284C7);

        // Vertical slider continuous mouse drag interaction
        if sld_active && sld_mx >= (col_x + 10) as i32 && sld_mx <= (col_x + channel_w - 10) as i32
            && sld_my >= track_y as i32 && sld_my <= (track_y + slider_h) as i32
        {
            let rel_y = (sld_my - track_y as i32) as usize;
            let new_vol = 100usize.saturating_sub((rel_y * 100) / slider_h).min(100) as u8;

            match idx {
                0 => {
                    if state.vol_master != new_vol {
                        state.vol_master = new_vol;
                        sync_hda_volume_hardware(new_vol, state.mute_master);
                    }
                }
                1 => state.vol_system = new_vol,
                2 => state.vol_wave = new_vol,
                3 => state.vol_synth = new_vol,
                _ => {}
            }
            desk.needs_full_redraw = true;
        }

        // Volume percentage display
        let vol_str = format!("{}%", if *is_muted { 0 } else { *vol_val });
        draw_text_px(fb, pitch, scr_w, scr_h, col_x + 28, track_y + slider_h + 12, &vol_str, 0x0038BDF8, None);

        // Mute Button (strictly edge-triggered)
        let mute_btn_y = track_y + slider_h + 34;
        let mute_style = if *is_muted { ButtonStyle::DangerRuby } else { ButtonStyle::StandardGlass };
        let mute_label = if *is_muted { "MUDO" } else { "Ativo" };
        if ui_glass_button(fb, pitch, scr_w, scr_h, col_x + 12, mute_btn_y, channel_w - 24, 22, mute_label, mute_style, btn_mx, btn_my, btn_click) {
            match idx {
                0 => {
                    state.mute_master = !state.mute_master;
                    sync_hda_volume_hardware(state.vol_master, state.mute_master);
                }
                1 => { state.mute_system = !state.mute_system; }
                2 => { state.mute_wave = !state.mute_wave; }
                3 => { state.mute_synth = !state.mute_synth; }
                _ => {}
            }
            play_ui_feedback_tone(440, 20);
            desk.needs_full_redraw = true;
        }
    }

    // 5. Stereo VU Meter on the right panel
    let vu_x = cx + 20 + 4 * (channel_w + 14);
    let vu_w = cw.saturating_sub(vu_x - cx + 20);
    if vu_w >= 70 {
        fill_rounded_rect(fb, pitch, scr_w, scr_h, vu_x, slider_y, vu_w, slider_h + 90, 4, 0x001E293B);
        draw_rounded_rect_border(fb, pitch, scr_w, scr_h, vu_x, slider_y, vu_w, slider_h + 90, 4, 0x001E3A8A);

        draw_text_px(fb, pitch, scr_w, scr_h, vu_x + 10, slider_y + 8, "VU Meter", 0x00F8FAFC, None);

        // Animated Left and Right VU columns
        let base_level = if state.mute_master { 0 } else { (state.vol_master as usize * 12) / 100 };
        let tick = state.anim_tick;
        let bounce_l = (base_level + (tick % 3)).min(12);
        let bounce_r = (base_level + ((tick + 1) % 4)).min(12);

        let vu_col_w = (vu_w - 30) / 2;
        let vu_h = slider_h;

        // Render 12 discrete LED-like segments
        for step in 0..12 {
            let seg_y = slider_y + 32 + (11 - step) * (vu_h / 12);
            let seg_h = (vu_h / 12).saturating_sub(2);

            let col = if step >= 10 {
                0x00EF4444 // Red (+0dB peak)
            } else if step >= 7 {
                0x00FBBF24 // Amber (-3dB)
            } else {
                0x0010B981 // Green (-12dB to -40dB)
            };

            // Channel L
            let lit_l = step < bounce_l;
            fill_rect(fb, pitch, scr_w, scr_h, vu_x + 8, seg_y, vu_col_w, seg_h, if lit_l { col } else { 0x001E293B });

            // Channel R
            let lit_r = step < bounce_r;
            fill_rect(fb, pitch, scr_w, scr_h, vu_x + 12 + vu_col_w, seg_y, vu_col_w, seg_h, if lit_r { col } else { 0x001E293B });
        }

        draw_text_px(fb, pitch, scr_w, scr_h, vu_x + 12, slider_y + slider_h + 38, "L", 0x0038BDF8, None);
        draw_text_px(fb, pitch, scr_w, scr_h, vu_x + 16 + vu_col_w, slider_y + slider_h + 38, "R", 0x0038BDF8, None);
    }
}

fn handle_hda_mixer_click(cx: usize, cy: usize, desk: &mut DesktopState) -> bool {
    let mut state = HDA_MIXER_STATE.lock();
    state.pending_click = Some((cx, cy));
    desk.needs_full_redraw = true;
    true
}

// =============================================================================
// 47. ESTÚDIO DE PERSONALIZAÇÃO CRYSTAL GLASS
// =============================================================================

unsafe fn render_personalization_studio(
    fb: *mut u8, pitch: usize, scr_w: usize, scr_h: usize,
    cx: usize, cy: usize, cw: usize, ch: usize,
    desk: &mut DesktopState,
) {
    let mut theme = GLOBAL_THEME.lock();

    let (mx, my, mouse_down) = {
        let m = MOUSE_STATE.lock();
        (m.x, m.y, m.left_button)
    };
    let pending = PERSONALIZATION_PENDING_CLICK.lock().take();
    let allow_drag = mouse_down && desk.focused_app.as_u8() == 47 && desk.dragging_window.is_none();
    let (sld_mx, sld_my, sld_active) = if allow_drag {
        (mx, my, true)
    } else if let Some((px, py)) = pending {
        (px as i32, py as i32, true)
    } else {
        (mx, my, false)
    };

    // Dark Vitrified Background
    fill_rounded_rect_gradient(fb, pitch, scr_w, scr_h, cx, cy, cw, ch, 6, 0x001E293B, 0x00334155);
    draw_rounded_rect_border(fb, pitch, scr_w, scr_h, cx, cy, cw, ch, 6, 0x0038BDF8);

    // Section 1: Window Frame Glass Tint (8 Palettes)
    draw_text_px(fb, pitch, scr_w, scr_h, cx + 18, cy + 14, "Cores Vitrificadas das Janelas (Crystal Glass Shaders)", 0x00FFFFFF, None);

    let swatch_w = 64usize;
    let swatch_h = 36usize;
    let grid_y = cy + 36;

    for (idx, item) in THEME_PALETTE.iter().enumerate() {
        let sx = cx + 18 + (idx % 4) * (swatch_w + 14);
        let sy = grid_y + (idx / 4) * (swatch_h + 12);
        let is_sel = theme.theme_index == idx;

        // Vitrified swatch with linear gradient & specular highlight
        fill_rounded_rect_gradient(fb, pitch, scr_w, scr_h, sx, sy, swatch_w, swatch_h, 4, item.active_frame, item.inactive_frame);

        if is_sel {
            draw_rounded_rect_border(fb, pitch, scr_w, scr_h, sx - 2, sy - 2, swatch_w + 4, swatch_h + 4, 6, item.accent);
            fill_rounded_rect(fb, pitch, scr_w, scr_h, sx + swatch_w - 14, sy + 4, 8, 8, 4, 0x00FFFFFF);
        } else {
            draw_rounded_rect_border(fb, pitch, scr_w, scr_h, sx, sy, swatch_w, swatch_h, 4, 0x00334155);
        }

        // Edge-triggered click detection on theme swatch
        if let Some((px, py)) = pending {
            if px >= sx && px <= sx + swatch_w && py >= sy && py <= sy + swatch_h {
                theme.theme_index = idx;
                theme.frame_active_color = item.active_frame;
                theme.frame_inactive_color = item.inactive_frame;
                theme.accent_color = item.accent;
                theme.theme_name = item.name;
                play_ui_feedback_tone(880, 20);
                desk.needs_full_redraw = true;
            }
        }
    }

    // Section 2: Glass Opacity / Translucency Slider
    let slider_sec_y = grid_y + 2 * (swatch_h + 12) + 16;
    draw_text_px(fb, pitch, scr_w, scr_h, cx + 18, slider_sec_y, "Intensidade da Transparencia & Efeito de Vidro (Alpha Blend)", 0x00FFFFFF, None);

    let slider_x = cx + 18;
    let slider_y = slider_sec_y + 22;
    let slider_w = cw.saturating_sub(120);

    // Draggable opacity slider (50..255)
    if ui_slider(fb, pitch, scr_w, scr_h, slider_x, slider_y, slider_w, &mut theme.glass_opacity, 50, 255, sld_mx, sld_my, sld_active) {
        desk.needs_full_redraw = true;
    }

    let op_pct = (theme.glass_opacity as usize * 100) / 255;
    let op_str = format!("{}% Opacidade", op_pct);
    draw_text_px(fb, pitch, scr_w, scr_h, slider_x + slider_w + 14, slider_y + 2, &op_str, 0x0038BDF8, None);

    // Section 3: Wallpaper Preset Selector
    let wall_sec_y = slider_y + 36;
    draw_text_px(fb, pitch, scr_w, scr_h, cx + 18, wall_sec_y, "Papel de Parede do Desktop (Renderizador GOP Shader)", 0x00FFFFFF, None);

    let wallpapers = [
        ("Gradiente Aurora", 0u8, 0x000F172A, 0x000284C7),
        ("Abstrato Cristal", 1u8, 0x00163254, 0x0038BDF8),
        ("Minimalista Escuro", 2u8, 0x00050810, 0x000F172A),
    ];

    let card_w = 120usize;
    let card_h = 56usize;
    let wall_grid_y = wall_sec_y + 20;

    for (idx, (name, preset_id, c1, c2)) in wallpapers.iter().enumerate() {
        let card_x = cx + 18 + idx * (card_w + 16);
        let is_sel = theme.wallpaper_preset == *preset_id;

        // Miniature wallpaper card
        fill_rounded_rect_gradient(fb, pitch, scr_w, scr_h, card_x, wall_grid_y, card_w, card_h, 4, *c1, *c2);

        if is_sel {
            draw_rounded_rect_border(fb, pitch, scr_w, scr_h, card_x - 2, wall_grid_y - 2, card_w + 4, card_h + 4, 6, 0x0038BDF8);
            fill_rounded_rect(fb, pitch, scr_w, scr_h, card_x + card_w - 14, wall_grid_y + 4, 8, 8, 4, 0x00FFFFFF);
        } else {
            draw_rounded_rect_border(fb, pitch, scr_w, scr_h, card_x, wall_grid_y, card_w, card_h, 4, 0x00334155);
        }

        draw_text_clipped(fb, pitch, scr_w, scr_h, card_x + 8, wall_grid_y + card_h - 20, card_x + card_w - 4, name, 0x00FFFFFF, None);

        // Edge-triggered click detection on wallpaper preview card
        if let Some((px, py)) = pending {
            if px >= card_x && px <= card_x + card_w && py >= wall_grid_y && py <= wall_grid_y + card_h {
                theme.wallpaper_preset = *preset_id;
                play_ui_feedback_tone(659, 25);
                desk.needs_full_redraw = true;
            }
        }
    }
}

fn handle_personalization_click(cx: usize, cy: usize, desk: &mut DesktopState) -> bool {
    *PERSONALIZATION_PENDING_CLICK.lock() = Some((cx, cy));
    desk.needs_full_redraw = true;
    true
}
