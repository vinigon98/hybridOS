//! Windows 11 Desktop Window Manager (DWM) & Advanced Surface Compositor
//!
//! Production-grade DWM Compositor for HybridOS:
//! 1. Window State & Hierarchy: `DwmWindow`, `DwmWindowId`, Z-order hierarchy, opacity,
//!    Fluent Mica/Acrylic material flags, window states (Normal, Minimized, Maximized, Snapped),
//!    borderless/decorated frames, rounded corners and drop shadows.
//! 2. Snap Assist & Snap Layouts Engine:
//!    - Grid types: TwoColumn (50/50, 70/30, 30/70), ThreeColumn (25/50/25), FourQuarter (2x2),
//!      LeftSplitWithTwoStacked, RightSplitWithTwoStacked.
//!    - Screen zone snapping triggers (top edge for maximize, left/right edges for half splits,
//!      4 screen corners for quarters).
//!    - Snap Assist suggested window picker slots and allocation.
//! 3. Window Animation Physics:
//!    - Physics-based Spring models (underdamped, critically damped, overdamped damped harmonic oscillator).
//!    - Windows 11 Cubic-Bezier easing models (Standard Ease, Accelerate, Decelerate, Expressive).
//!    - Transitions: Minimize to taskbar icon rect, Maximize to work area, Restore, Open/Close scale fade.
//!    - "Aero Shake" gesture detection to minimize all background windows and secondary shake to restore.
//! 4. Dirty Region Tracking & Composition Pipeline:
//!    - `DirtyRectTracker` maintaining non-overlapping bounding rectangles with coalescing.
//!    - Direct composition pass combining wallpaper background plane, window surface plane,
//!      top-level flyouts / snap ghost overlay plane, and hardware cursor plane.
//!    - 60 FPS / 120 FPS frame timing and VSync tick generator with dropped frame telemetry.
//! 5. Comprehensive Unit Test Suite verifying snapping, z-ordering, physics animations,
//!    dirty rect merging/disjoint partitioning, Aero Shake, and composition passes.

#![allow(dead_code)]

#[allow(unused_imports)]
use alloc::string::{String, ToString};
#[allow(unused_imports)]
use alloc::vec::Vec;
#[allow(unused_imports)]
use alloc::boxed::Box;
#[allow(unused_imports)]
use crate::compat::FloatExt;
use std::collections::HashMap;

// ============================================================================
// Section 1: Geometry, Colors & 2D Surface Primitives
// ============================================================================

/// Unique identifier for a DWM managed window (Win32 HWND compatibility).
pub type DwmWindowId = u64;

/// 2D Integer Rectangle representing window geometries and dirty regions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DwmRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl DwmRect {
    pub const EMPTY: Self = Self { x: 0, y: 0, width: 0, height: 0 };

    #[inline]
    pub const fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self { x, y, width, height }
    }

    #[inline]
    pub fn left(&self) -> i32 {
        self.x
    }

    #[inline]
    pub fn top(&self) -> i32 {
        self.y
    }

    #[inline]
    pub fn right(&self) -> i32 {
        self.x.saturating_add(self.width as i32)
    }

    #[inline]
    pub fn bottom(&self) -> i32 {
        self.y.saturating_add(self.height as i32)
    }

    #[inline]
    pub fn center_x(&self) -> i32 {
        self.x.saturating_add((self.width / 2) as i32)
    }

    #[inline]
    pub fn center_y(&self) -> i32 {
        self.y.saturating_add((self.height / 2) as i32)
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }

    #[inline]
    pub fn area(&self) -> u64 {
        (self.width as u64) * (self.height as u64)
    }

    #[inline]
    pub fn contains_point(&self, px: i32, py: i32) -> bool {
        px >= self.left() && px < self.right() && py >= self.top() && py < self.bottom()
    }

    #[inline]
    pub fn contains_rect(&self, other: &DwmRect) -> bool {
        if other.is_empty() {
            return true;
        }
        if self.is_empty() {
            return false;
        }
        other.left() >= self.left()
            && other.right() <= self.right()
            && other.top() >= self.top()
            && other.bottom() <= self.bottom()
    }

    #[inline]
    pub fn intersects(&self, other: &DwmRect) -> bool {
        !self.is_empty()
            && !other.is_empty()
            && self.left() < other.right()
            && self.right() > other.left()
            && self.top() < other.bottom()
            && self.bottom() > other.top()
    }

    pub fn intersection(&self, other: &DwmRect) -> Option<DwmRect> {
        let x1 = self.left().max(other.left());
        let y1 = self.top().max(other.top());
        let x2 = self.right().min(other.right());
        let y2 = self.bottom().min(other.bottom());

        if x1 < x2 && y1 < y2 {
            Some(DwmRect {
                x: x1,
                y: y1,
                width: (x2 - x1) as u32,
                height: (y2 - y1) as u32,
            })
        } else {
            None
        }
    }

    pub fn union(&self, other: &DwmRect) -> DwmRect {
        if self.is_empty() {
            return *other;
        }
        if other.is_empty() {
            return *self;
        }

        let x1 = self.left().min(other.left());
        let y1 = self.top().min(other.top());
        let x2 = self.right().max(other.right());
        let y2 = self.bottom().max(other.bottom());

        DwmRect {
            x: x1,
            y: y1,
            width: (x2 - x1) as u32,
            height: (y2 - y1) as u32,
        }
    }

    #[inline]
    pub fn offset(&self, dx: i32, dy: i32) -> DwmRect {
        DwmRect {
            x: self.x.saturating_add(dx),
            y: self.y.saturating_add(dy),
            width: self.width,
            height: self.height,
        }
    }

    pub fn inset(&self, dx: i32, dy: i32) -> DwmRect {
        let double_dx = (dx.max(0) * 2) as u32;
        let double_dy = (dy.max(0) * 2) as u32;

        if self.width <= double_dx || self.height <= double_dy {
            DwmRect::EMPTY
        } else {
            DwmRect {
                x: self.x + dx,
                y: self.y + dy,
                width: self.width - double_dx,
                height: self.height - double_dy,
            }
        }
    }
}

/// 32-bit ARGB/sRGB Color for DWM surface blending and Fluent materials.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DwmColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl DwmColor {
    pub const TRANSPARENT: Self = Self { r: 0, g: 0, b: 0, a: 0 };
    pub const BLACK: Self = Self { r: 0, g: 0, b: 0, a: 255 };
    pub const WHITE: Self = Self { r: 255, g: 255, b: 255, a: 255 };

    /// Windows 11 Signature Accent Blue (#0078D4)
    pub const WIN11_ACCENT_BLUE: Self = Self { r: 0, g: 120, b: 212, a: 255 };
    /// Windows 11 Snap Zone Ghost Highlight (#0078D4 with 28% alpha)
    pub const SNAP_GHOST_BLUE: Self = Self { r: 0, g: 120, b: 212, a: 72 };
    /// Windows 11 Snap Ghost Border (#0078D4 with 80% alpha)
    pub const SNAP_GHOST_BORDER: Self = Self { r: 0, g: 120, b: 212, a: 204 };

    /// Windows 11 Mica Dark Window Tint (#202020)
    pub const MICA_DARK: Self = Self { r: 32, g: 32, b: 32, a: 255 };
    /// Windows 11 Mica Light Window Tint (#F3F3F3)
    pub const MICA_LIGHT: Self = Self { r: 243, g: 243, b: 243, a: 255 };

    /// Fluent Dark Titlebar Background (#1F1F1F)
    pub const TITLEBAR_DARK: Self = Self { r: 31, g: 31, b: 31, a: 255 };
    /// Fluent Light Titlebar Background (#EAEAEA)
    pub const TITLEBAR_LIGHT: Self = Self { r: 234, g: 234, b: 234, a: 255 };

    /// Subtle 1px Window Border in Dark Theme (#FFFFFF with 10% alpha)
    pub const BORDER_DARK: Self = Self { r: 255, g: 255, b: 255, a: 26 };
    /// Subtle 1px Window Border in Light Theme (#000000 with 8% alpha)
    pub const BORDER_LIGHT: Self = Self { r: 0, g: 0, b: 0, a: 20 };

    #[inline]
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    #[inline]
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    #[inline]
    pub const fn from_argb_u32(val: u32) -> Self {
        Self {
            a: ((val >> 24) & 0xFF) as u8,
            r: ((val >> 16) & 0xFF) as u8,
            g: ((val >> 8) & 0xFF) as u8,
            b: (val & 0xFF) as u8,
        }
    }

    #[inline]
    pub const fn to_argb_u32(&self) -> u32 {
        ((self.a as u32) << 24) | ((self.r as u32) << 16) | ((self.g as u32) << 8) | (self.b as u32)
    }

    #[inline]
    pub fn with_alpha(&self, a: u8) -> Self {
        Self { r: self.r, g: self.g, b: self.b, a }
    }

    #[inline]
    pub fn with_alpha_f32(&self, alpha: f32) -> Self {
        let clamped = alpha.clamp(0.0, 1.0);
        Self {
            r: self.r,
            g: self.g,
            b: self.b,
            a: (clamped * 255.0).round() as u8,
        }
    }

    pub fn lerp(&self, target: Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        let inv_t = 1.0 - t;
        Self {
            r: (self.r as f32 * inv_t + target.r as f32 * t).round() as u8,
            g: (self.g as f32 * inv_t + target.g as f32 * t).round() as u8,
            b: (self.b as f32 * inv_t + target.b as f32 * t).round() as u8,
            a: (self.a as f32 * inv_t + target.a as f32 * t).round() as u8,
        }
    }

    /// Fast Porter-Duff source-over integer alpha blending.
    #[inline]
    pub fn blend_over(dst: Self, src: Self) -> Self {
        if src.a == 255 {
            return src;
        }
        if src.a == 0 {
            return dst;
        }

        let sa = src.a as u32;
        let da = dst.a as u32;
        let inv_sa = 255 - sa;

        let out_a = sa + (da * inv_sa + 127) / 255;
        if out_a == 0 {
            return Self::TRANSPARENT;
        }

        let out_r = ((src.r as u32 * sa + (dst.r as u32 * da * inv_sa + 127) / 255) + (out_a / 2)) / out_a;
        let out_g = ((src.g as u32 * sa + (dst.g as u32 * da * inv_sa + 127) / 255) + (out_a / 2)) / out_a;
        let out_b = ((src.b as u32 * sa + (dst.b as u32 * da * inv_sa + 127) / 255) + (out_a / 2)) / out_a;

        Self {
            r: out_r.min(255) as u8,
            g: out_g.min(255) as u8,
            b: out_b.min(255) as u8,
            a: out_a.min(255) as u8,
        }
    }
}

/// 2D Pixel Buffer Surface for window contents and composition targets.
#[derive(Debug, Clone)]
pub struct DwmSurface {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u32>, // Packed 0xAARRGGBB
}

impl DwmSurface {
    pub fn new(width: usize, height: usize, clear_color: DwmColor) -> Self {
        let packed = clear_color.to_argb_u32();
        Self {
            width,
            height,
            pixels: vec![packed; width * height],
        }
    }

    #[inline]
    pub fn get_pixel(&self, x: usize, y: usize) -> DwmColor {
        if x < self.width && y < self.height {
            DwmColor::from_argb_u32(self.pixels[y * self.width + x])
        } else {
            DwmColor::TRANSPARENT
        }
    }

    #[inline]
    pub fn set_pixel(&mut self, x: usize, y: usize, color: DwmColor) {
        if x < self.width && y < self.height {
            self.pixels[y * self.width + x] = color.to_argb_u32();
        }
    }

    pub fn fill_rect(&mut self, rect: DwmRect, color: DwmColor) {
        if rect.is_empty() {
            return;
        }
        let x_start = rect.left().max(0) as usize;
        let y_start = rect.top().max(0) as usize;
        let x_end = (rect.right().max(0) as usize).min(self.width);
        let y_end = (rect.bottom().max(0) as usize).min(self.height);

        if x_start >= x_end || y_start >= y_end {
            return;
        }

        if color.a == 255 {
            let packed = color.to_argb_u32();
            for y in y_start..y_end {
                let row_offset = y * self.width;
                for x in x_start..x_end {
                    self.pixels[row_offset + x] = packed;
                }
            }
        } else {
            for y in y_start..y_end {
                let row_offset = y * self.width;
                for x in x_start..x_end {
                    let dst = DwmColor::from_argb_u32(self.pixels[row_offset + x]);
                    let blended = DwmColor::blend_over(dst, color);
                    self.pixels[row_offset + x] = blended.to_argb_u32();
                }
            }
        }
    }

    /// Blit source surface onto this target surface with clipping and global opacity.
    pub fn blit(
        &mut self,
        src: &DwmSurface,
        dst_x: i32,
        dst_y: i32,
        clip: Option<DwmRect>,
        opacity: f32,
    ) {
        if opacity <= 0.0 {
            return;
        }

        let clip_rect = clip.unwrap_or(DwmRect::new(0, 0, self.width as u32, self.height as u32));
        let src_rect = DwmRect::new(dst_x, dst_y, src.width as u32, src.height as u32);

        let visible_rect = match src_rect.intersection(&clip_rect) {
            Some(r) => r,
            None => return,
        };

        let clamped_opacity = opacity.clamp(0.0, 1.0);

        for y in visible_rect.top()..visible_rect.bottom() {
            if y < 0 || y >= self.height as i32 {
                continue;
            }
            let src_y = (y - dst_y) as usize;
            if src_y >= src.height {
                continue;
            }

            let dst_row = (y as usize) * self.width;
            let src_row = src_y * src.width;

            for x in visible_rect.left()..visible_rect.right() {
                if x < 0 || x >= self.width as i32 {
                    continue;
                }
                let src_x = (x - dst_x) as usize;
                if src_x >= src.width {
                    continue;
                }

                let mut src_pixel = DwmColor::from_argb_u32(src.pixels[src_row + src_x]);
                if clamped_opacity < 1.0 {
                    src_pixel.a = ((src_pixel.a as f32) * clamped_opacity).round() as u8;
                }

                if src_pixel.a == 0 {
                    continue;
                }

                let dst_idx = dst_row + (x as usize);
                let dst_pixel = DwmColor::from_argb_u32(self.pixels[dst_idx]);
                let blended = DwmColor::blend_over(dst_pixel, src_pixel);
                self.pixels[dst_idx] = blended.to_argb_u32();
            }
        }
    }
}

// ============================================================================
// Section 2: Window State & Hierarchy
// ============================================================================

/// Windows 11 DWM Window State
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowState {
    /// Window displayed normally at user-defined geometry.
    Normal,
    /// Window minimized to taskbar icon.
    Minimized,
    /// Window maximized to cover full work area (minus taskbar).
    Maximized,
    /// Window snapped to a screen zone / snap layout slot.
    Snapped(SnapZone),
}

/// Fluent Materials supported by the DWM Compositor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowMaterial {
    /// Standard solid opaque or transparent window surface.
    None,
    /// Windows 11 Mica Base (samples desktop wallpaper with blur and tint).
    MicaBase,
    /// Windows 11 Mica Alt (higher contrast variant for tabs and chrome).
    MicaAlt,
    /// Windows 11 Background Acrylic (samples backdrop behind window).
    AcrylicBackground,
    /// Windows 11 In-App Acrylic (sidebar navigation view material).
    AcrylicInApp,
}

/// Window styling and behavior flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowFlags {
    pub borderless: bool,
    pub decorated: bool,
    pub always_on_top: bool,
    pub transparent: bool,
    pub resizable: bool,
    pub aero_shake_immune: bool,
}

impl Default for WindowFlags {
    fn default() -> Self {
        Self {
            borderless: false,
            decorated: true,
            always_on_top: false,
            transparent: false,
            resizable: true,
            aero_shake_immune: false,
        }
    }
}

/// Production DWM Window representing a desktop surface in the compositor hierarchy.
#[derive(Debug, Clone)]
pub struct DwmWindow {
    /// Window identifier / Win32 HWND.
    pub id: DwmWindowId,
    /// Window title text for Fluent titlebar.
    pub title: String,
    /// Current rendered rectangle on screen.
    pub rect: DwmRect,
    /// Normal/Restore rectangle before maximize, snap, or minimize.
    pub restore_rect: DwmRect,
    /// Z-order index (higher value = closer to user/foreground).
    pub z_order: usize,
    /// Overall window opacity (0.0 = fully transparent, 1.0 = fully opaque).
    pub opacity: f32,
    /// Rendering scale factor (1.0 = normal, used in open/close/minimize physics).
    pub scale: f32,
    /// Current window presentation state.
    pub state: WindowState,
    /// Fluent material effect applied to window surface.
    pub material: WindowMaterial,
    /// Mica flag.
    pub is_mica: bool,
    /// Acrylic flag.
    pub is_acrylic: bool,
    /// Whether window is borderless (e.g. games, video players).
    pub borderless: bool,
    /// Whether window has titlebar and system frame decorations.
    pub decorated: bool,
    /// Whether window is currently active / focused.
    pub active: bool,
    /// Whether window is visible.
    pub visible: bool,
    /// Windows 11 corner radius (8.0px standard, 0.0px when maximized).
    pub corner_radius: f32,
    /// Pixel buffer containing client surface content.
    pub surface: Option<DwmSurface>,
    /// Active physics animation if transitioning.
    pub animation: Option<WindowAnimation>,
    /// Minimum allowed window size.
    pub min_size: (u32, u32),
    /// Maximum allowed window size.
    pub max_size: Option<(u32, u32)>,
    /// Flags.
    pub flags: WindowFlags,
}

impl DwmWindow {
    /// Create a new managed DWM Window.
    pub fn new(id: DwmWindowId, title: impl Into<String>, rect: DwmRect) -> Self {
        Self {
            id,
            title: title.into(),
            rect,
            restore_rect: rect,
            z_order: 0,
            opacity: 1.0,
            scale: 1.0,
            state: WindowState::Normal,
            material: WindowMaterial::None,
            is_mica: false,
            is_acrylic: false,
            borderless: false,
            decorated: true,
            active: false,
            visible: true,
            corner_radius: 8.0,
            surface: None,
            animation: None,
            min_size: (160, 100),
            max_size: None,
            flags: WindowFlags::default(),
        }
    }

    #[inline]
    pub fn hwnd(&self) -> u64 {
        self.id
    }

    pub fn set_material(&mut self, material: WindowMaterial) {
        self.material = material;
        self.is_mica = matches!(material, WindowMaterial::MicaBase | WindowMaterial::MicaAlt);
        self.is_acrylic = matches!(material, WindowMaterial::AcrylicBackground | WindowMaterial::AcrylicInApp);
    }

    pub fn set_rect(&mut self, new_rect: DwmRect) {
        let (min_w, min_h) = self.min_size;
        let clamped_w = new_rect.width.max(min_w);
        let clamped_h = new_rect.height.max(min_h);

        self.rect = DwmRect::new(new_rect.x, new_rect.y, clamped_w, clamped_h);
        if self.state == WindowState::Normal {
            self.restore_rect = self.rect;
        }
    }

    pub fn set_active(&mut self, active: bool) {
        self.active = active;
    }

    pub fn set_opacity(&mut self, opacity: f32) {
        self.opacity = opacity.clamp(0.0, 1.0);
    }

    /// Trigger minimize animation to target taskbar icon rect.
    pub fn minimize(&mut self, taskbar_icon_rect: Option<DwmRect>) {
        if self.state == WindowState::Minimized {
            return;
        }

        if self.state == WindowState::Normal {
            self.restore_rect = self.rect;
        }

        let target_icon = taskbar_icon_rect.unwrap_or(DwmRect::new(
            self.rect.center_x() - 20,
            self.rect.bottom() + 100,
            40,
            40,
        ));

        self.animation = Some(WindowAnimation::new_minimize(self.rect, target_icon));
        self.state = WindowState::Minimized;
        self.active = false;
    }

    /// Maximize window to fill entire desktop work area.
    pub fn maximize(&mut self, work_area: DwmRect) {
        if self.state == WindowState::Maximized {
            return;
        }

        if self.state == WindowState::Normal {
            self.restore_rect = self.rect;
        }

        self.animation = Some(WindowAnimation::new_maximize(self.rect, work_area));
        self.state = WindowState::Maximized;
        self.corner_radius = 0.0;
    }

    /// Restore window from Minimized or Maximized or Snapped state.
    pub fn restore(&mut self) {
        if self.state == WindowState::Normal && self.animation.is_none() {
            return;
        }

        let target_rect = self.restore_rect;
        let current_rect = self.rect;

        self.animation = Some(WindowAnimation::new_restore(current_rect, target_rect));
        self.state = WindowState::Normal;
        self.visible = true;
        self.corner_radius = 8.0;
    }

    /// Snap window into a specific screen zone or layout slot.
    pub fn snap(&mut self, zone: SnapZone, target_rect: DwmRect) {
        if self.state == WindowState::Normal {
            self.restore_rect = self.rect;
        }

        self.animation = Some(WindowAnimation::new_snap_transition(self.rect, target_rect));
        self.state = WindowState::Snapped(zone);
        self.corner_radius = 8.0;
    }

    /// Step active physics animation by delta milliseconds.
    pub fn step_animation(&mut self, delta_ms: f32) -> bool {
        if let Some(anim) = &mut self.animation {
            let still_running = anim.step(delta_ms);
            self.rect = anim.current_rect;
            self.opacity = anim.current_opacity;
            self.scale = anim.current_scale;
            self.corner_radius = anim.current_corner_radius;

            if !still_running {
                let is_minimized = self.state == WindowState::Minimized;
                self.animation = None;
                if is_minimized {
                    self.visible = false;
                }
                return false;
            }
            true
        } else {
            false
        }
    }
}

// ============================================================================
// Section 3: Snap Assist & Snap Layouts Engine
// ============================================================================

/// Windows 11 Snap Layout Grid Configurations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SnapLayoutType {
    /// 50 / 50 two equal vertical columns.
    TwoColumnEqual,
    /// 70 / 30 left-major two column split.
    TwoColumnLeftMajor,
    /// 30 / 70 right-major two column split.
    TwoColumnRightMajor,
    /// 25 / 50 / 25 three column layout (center major).
    ThreeColumn,
    /// 2x2 grid (four equal quarter tiles).
    FourQuarter,
    /// Left full-height half (50%) with right column split into two stacked quarters (25% each).
    LeftSplitWithTwoStacked,
    /// Right full-height half (50%) with left column split into two stacked quarters (25% each).
    RightSplitWithTwoStacked,
}

/// Screen zone triggers for edge and corner dragging.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SnapZone {
    Maximize,
    LeftHalf,
    RightHalf,
    TopLeftQuarter,
    TopRightQuarter,
    BottomLeftQuarter,
    BottomRightQuarter,
    LayoutSlot { layout: SnapLayoutType, slot_index: usize },
}

/// Core Snap Layouts and Screen Edge/Corner Trigger Engine.
#[derive(Debug, Clone)]
pub struct SnapEngine {
    /// Current display work area (screen minus taskbar).
    pub work_area: DwmRect,
    /// Screen edge trigger threshold in pixels (e.g. 12px from border).
    pub edge_threshold: i32,
    /// Screen corner trigger radius in pixels (e.g. 36px from corners).
    pub corner_threshold: i32,
}

impl SnapEngine {
    pub fn new(work_area: DwmRect) -> Self {
        Self {
            work_area,
            edge_threshold: 12,
            corner_threshold: 36,
        }
    }

    /// Calculate all sub-rectangles for a specified Snap Layout inside the work area.
    pub fn calculate_layout_rects(&self, layout: SnapLayoutType) -> Vec<DwmRect> {
        let wa = self.work_area;
        if wa.is_empty() {
            return Vec::new();
        }

        match layout {
            SnapLayoutType::TwoColumnEqual => {
                let half_w = wa.width / 2;
                vec![
                    DwmRect::new(wa.x, wa.y, half_w, wa.height),
                    DwmRect::new(wa.x + half_w as i32, wa.y, wa.width - half_w, wa.height),
                ]
            }

            SnapLayoutType::TwoColumnLeftMajor => {
                let left_w = (wa.width * 70) / 100;
                vec![
                    DwmRect::new(wa.x, wa.y, left_w, wa.height),
                    DwmRect::new(wa.x + left_w as i32, wa.y, wa.width - left_w, wa.height),
                ]
            }

            SnapLayoutType::TwoColumnRightMajor => {
                let left_w = (wa.width * 30) / 100;
                vec![
                    DwmRect::new(wa.x, wa.y, left_w, wa.height),
                    DwmRect::new(wa.x + left_w as i32, wa.y, wa.width - left_w, wa.height),
                ]
            }

            SnapLayoutType::ThreeColumn => {
                let left_w = (wa.width * 25) / 100;
                let center_w = (wa.width * 50) / 100;
                let right_w = wa.width - left_w - center_w;
                vec![
                    DwmRect::new(wa.x, wa.y, left_w, wa.height),
                    DwmRect::new(wa.x + left_w as i32, wa.y, center_w, wa.height),
                    DwmRect::new(wa.x + (left_w + center_w) as i32, wa.y, right_w, wa.height),
                ]
            }

            SnapLayoutType::FourQuarter => {
                let half_w = wa.width / 2;
                let half_h = wa.height / 2;
                let right_w = wa.width - half_w;
                let bottom_h = wa.height - half_h;

                vec![
                    // Slot 0: Top-Left
                    DwmRect::new(wa.x, wa.y, half_w, half_h),
                    // Slot 1: Top-Right
                    DwmRect::new(wa.x + half_w as i32, wa.y, right_w, half_h),
                    // Slot 2: Bottom-Left
                    DwmRect::new(wa.x, wa.y + half_h as i32, half_w, bottom_h),
                    // Slot 3: Bottom-Right
                    DwmRect::new(wa.x + half_w as i32, wa.y + half_h as i32, right_w, bottom_h),
                ]
            }

            SnapLayoutType::LeftSplitWithTwoStacked => {
                let half_w = wa.width / 2;
                let right_w = wa.width - half_w;
                let half_h = wa.height / 2;
                let bottom_h = wa.height - half_h;

                vec![
                    // Slot 0: Full height left half
                    DwmRect::new(wa.x, wa.y, half_w, wa.height),
                    // Slot 1: Top-Right quarter
                    DwmRect::new(wa.x + half_w as i32, wa.y, right_w, half_h),
                    // Slot 2: Bottom-Right quarter
                    DwmRect::new(wa.x + half_w as i32, wa.y + half_h as i32, right_w, bottom_h),
                ]
            }

            SnapLayoutType::RightSplitWithTwoStacked => {
                let half_w = wa.width / 2;
                let right_w = wa.width - half_w;
                let half_h = wa.height / 2;
                let bottom_h = wa.height - half_h;

                vec![
                    // Slot 0: Full height right half
                    DwmRect::new(wa.x + half_w as i32, wa.y, right_w, wa.height),
                    // Slot 1: Top-Left quarter
                    DwmRect::new(wa.x, wa.y, half_w, half_h),
                    // Slot 2: Bottom-Left quarter
                    DwmRect::new(wa.x, wa.y + half_h as i32, half_w, bottom_h),
                ]
            }
        }
    }

    /// Detect if a cursor position triggers a screen zone snap during window drag.
    pub fn detect_snap_zone(&self, cursor_x: i32, cursor_y: i32) -> Option<SnapZone> {
        let wa = self.work_area;
        if wa.is_empty() {
            return None;
        }

        let at_top = cursor_y <= wa.top() + self.edge_threshold;
        let at_bottom = cursor_y >= wa.bottom() - self.edge_threshold;
        let at_left = cursor_x <= wa.left() + self.edge_threshold;
        let at_right = cursor_x >= wa.right() - self.edge_threshold;

        let near_left_corner = cursor_x <= wa.left() + self.corner_threshold;
        let near_right_corner = cursor_x >= wa.right() - self.corner_threshold;
        let near_top_corner = cursor_y <= wa.top() + self.corner_threshold;
        let near_bottom_corner = cursor_y >= wa.bottom() - self.corner_threshold;

        // Check top edge: Corner quarters take precedence over maximize
        if at_top {
            if near_left_corner {
                return Some(SnapZone::TopLeftQuarter);
            }
            if near_right_corner {
                return Some(SnapZone::TopRightQuarter);
            }
            return Some(SnapZone::Maximize);
        }

        // Check left edge: Corner quarters vs 50% left split
        if at_left {
            if near_top_corner {
                return Some(SnapZone::TopLeftQuarter);
            }
            if near_bottom_corner {
                return Some(SnapZone::BottomLeftQuarter);
            }
            return Some(SnapZone::LeftHalf);
        }

        // Check right edge: Corner quarters vs 50% right split
        if at_right {
            if near_top_corner {
                return Some(SnapZone::TopRightQuarter);
            }
            if near_bottom_corner {
                return Some(SnapZone::BottomRightQuarter);
            }
            return Some(SnapZone::RightHalf);
        }

        // Check bottom edge corners
        if at_bottom {
            if near_left_corner {
                return Some(SnapZone::BottomLeftQuarter);
            }
            if near_right_corner {
                return Some(SnapZone::BottomRightQuarter);
            }
        }

        None
    }

    /// Compute the target snap rectangle for a given SnapZone.
    pub fn snap_zone_to_rect(&self, zone: SnapZone) -> DwmRect {
        let wa = self.work_area;
        match zone {
            SnapZone::Maximize => wa,

            SnapZone::LeftHalf => {
                let half_w = wa.width / 2;
                DwmRect::new(wa.x, wa.y, half_w, wa.height)
            }

            SnapZone::RightHalf => {
                let half_w = wa.width / 2;
                DwmRect::new(wa.x + half_w as i32, wa.y, wa.width - half_w, wa.height)
            }

            SnapZone::TopLeftQuarter => {
                let half_w = wa.width / 2;
                let half_h = wa.height / 2;
                DwmRect::new(wa.x, wa.y, half_w, half_h)
            }

            SnapZone::TopRightQuarter => {
                let half_w = wa.width / 2;
                let half_h = wa.height / 2;
                DwmRect::new(wa.x + half_w as i32, wa.y, wa.width - half_w, half_h)
            }

            SnapZone::BottomLeftQuarter => {
                let half_w = wa.width / 2;
                let half_h = wa.height / 2;
                DwmRect::new(wa.x, wa.y + half_h as i32, half_w, wa.height - half_h)
            }

            SnapZone::BottomRightQuarter => {
                let half_w = wa.width / 2;
                let half_h = wa.height / 2;
                DwmRect::new(wa.x + half_w as i32, wa.y + half_h as i32, wa.width - half_w, wa.height - half_h)
            }

            SnapZone::LayoutSlot { layout, slot_index } => {
                let rects = self.calculate_layout_rects(layout);
                rects.get(slot_index).copied().unwrap_or(wa)
            }
        }
    }
}

/// Snap Assist picker suggestion slot representing an unoccupied layout quadrant.
#[derive(Debug, Clone)]
pub struct SnapAssistSlot {
    pub slot_index: usize,
    pub rect: DwmRect,
    pub candidate_windows: Vec<DwmWindowId>,
}

/// Snap Assist suggestions engine triggered when a window snaps into a layout slot.
#[derive(Debug, Clone)]
pub struct SnapAssist {
    pub active: bool,
    pub active_layout: Option<SnapLayoutType>,
    pub slots: Vec<SnapAssistSlot>,
    pub selected_slot: usize,
}

impl SnapAssist {
    pub fn new() -> Self {
        Self {
            active: false,
            active_layout: None,
            slots: Vec::new(),
            selected_slot: 0,
        }
    }

    /// Activate Snap Assist after a window snaps into a slot.
    pub fn activate(
        &mut self,
        engine: &SnapEngine,
        layout: SnapLayoutType,
        snapped_slot: usize,
        snapped_window: DwmWindowId,
        all_windows: &[DwmWindow],
    ) {
        let rects = engine.calculate_layout_rects(layout);
        if rects.len() <= 1 {
            self.dismiss();
            return;
        }

        // Collect other eligible unminimized visible windows in descending Z-order
        let mut candidates: Vec<DwmWindowId> = all_windows
            .iter()
            .filter(|w| w.id != snapped_window && w.visible && w.state != WindowState::Minimized)
            .map(|w| w.id)
            .collect();

        candidates.reverse(); // Most recent first

        let mut assist_slots = Vec::new();
        for (idx, rect) in rects.iter().enumerate() {
            if idx == snapped_slot {
                continue;
            }
            assist_slots.push(SnapAssistSlot {
                slot_index: idx,
                rect: *rect,
                candidate_windows: candidates.clone(),
            });
        }

        if assist_slots.is_empty() {
            self.dismiss();
        } else {
            self.active = true;
            self.active_layout = Some(layout);
            self.slots = assist_slots;
            self.selected_slot = 0;
        }
    }

    /// Select a candidate window to occupy the next open Snap Assist slot.
    pub fn pick_candidate_for_slot(
        &mut self,
        slot_index: usize,
        window_id: DwmWindowId,
    ) -> Option<DwmRect> {
        let pos = self.slots.iter().position(|s| s.slot_index == slot_index)?;
        let target_rect = self.slots[pos].rect;
        self.slots.remove(pos);

        // Remove the chosen window from candidate lists of remaining slots
        for s in &mut self.slots {
            s.candidate_windows.retain(|&id| id != window_id);
        }

        if self.slots.is_empty() {
            self.dismiss();
        }

        Some(target_rect)
    }

    pub fn dismiss(&mut self) {
        self.active = false;
        self.active_layout = None;
        self.slots.clear();
        self.selected_slot = 0;
    }
}

// ============================================================================
// Section 4: Window Animation Physics (Spring & Cubic-Bezier)
// ============================================================================

/// Physics-based Damped Harmonic Oscillator (Spring) Easing Model.
///
/// Differential Equation:
///   `m * x''(t) + c * x'(t) + k * x(t) = 0`
///
/// Closed-form analytical solution evaluating exact position without numerical drift.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpringModel {
    pub stiffness: f32, // Spring constant k
    pub damping: f32,   // Damping coefficient c
    pub mass: f32,      // Mass m (default = 1.0)
}

impl SpringModel {
    pub const fn new(stiffness: f32, damping: f32, mass: f32) -> Self {
        Self { stiffness, damping, mass }
    }

    /// Windows 11 Gentle Spring (Window Resize & Snapping)
    pub const fn gentle() -> Self {
        Self::new(180.0, 24.0, 1.0)
    }

    /// Windows 11 Snappy Spring (Taskbar & Rapid UI Feedback)
    pub const fn snappy() -> Self {
        Self::new(320.0, 28.0, 1.0)
    }

    /// Windows 11 Bouncy Spring (Touch Gestures & Dynamic Feedback)
    pub const fn bouncy() -> Self {
        Self::new(220.0, 14.0, 1.0)
    }

    /// Evaluate spring position at time `t_sec` starting from 0.0 with target 1.0.
    pub fn evaluate(&self, t_sec: f32) -> f32 {
        if t_sec <= 0.0 {
            return 0.0;
        }

        let k = self.stiffness;
        let c = self.damping;
        let m = self.mass.max(0.001);

        let omega_0 = (k / m).sqrt(); // Natural undamped angular frequency
        let zeta = c / (2.0 * (k * m).sqrt()); // Damping ratio

        if zeta < 0.9999 {
            // Underdamped oscillation
            let omega_d = omega_0 * (1.0 - zeta * zeta).sqrt();
            let exp_term = (-zeta * omega_0 * t_sec).exp();
            let cos_term = (omega_d * t_sec).cos();
            let sin_term = (omega_d * t_sec).sin();

            1.0 - exp_term * (cos_term + (zeta * omega_0 / omega_d) * sin_term)
        } else if zeta <= 1.0001 {
            // Critically damped decay
            let exp_term = (-omega_0 * t_sec).exp();
            1.0 - exp_term * (1.0 + omega_0 * t_sec)
        } else {
            // Overdamped decay
            let omega_d = omega_0 * (zeta * zeta - 1.0).sqrt();
            let exp_term = (-zeta * omega_0 * t_sec).exp();
            let cosh_term = (omega_d * t_sec).cosh();
            let sinh_term = (omega_d * t_sec).sinh();

            1.0 - exp_term * (cosh_term + (zeta * omega_0 / omega_d) * sinh_term)
        }
    }
}

/// Windows 11 Cubic-Bezier Easing Curve Model.
///
/// Solves `progress = X(t)` via high-precision Newton-Raphson iteration with bisection fallback,
/// then evaluates `Y(t)` on the cubic spline.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CubicBezier {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
}

impl CubicBezier {
    pub const fn new(x1: f32, y1: f32, x2: f32, y2: f32) -> Self {
        Self { x1, y1, x2, y2 }
    }

    /// Windows 11 Standard Easing Curve: (0.8, 0.0, 0.2, 1.0)
    pub const STANDARD_EASE: Self = Self::new(0.8, 0.0, 0.2, 1.0);
    /// Windows 11 Decelerate Curve: (0.1, 0.9, 0.2, 1.0)
    pub const DECELERATE: Self = Self::new(0.1, 0.9, 0.2, 1.0);
    /// Windows 11 Accelerate Curve: (0.7, 0.0, 1.0, 1.0)
    pub const ACCELERATE: Self = Self::new(0.7, 0.0, 1.0, 1.0);
    /// Windows 11 Expressive Entrance Curve: (0.0, 0.0, 0.0, 1.0)
    pub const EXPRESSIVE: Self = Self::new(0.0, 0.0, 0.0, 1.0);

    #[inline]
    fn sample_curve_x(&self, t: f32) -> f32 {
        let inv_t = 1.0 - t;
        3.0 * inv_t * inv_t * t * self.x1 + 3.0 * inv_t * t * t * self.x2 + t * t * t
    }

    #[inline]
    fn sample_curve_y(&self, t: f32) -> f32 {
        let inv_t = 1.0 - t;
        3.0 * inv_t * inv_t * t * self.y1 + 3.0 * inv_t * t * t * self.y2 + t * t * t
    }

    #[inline]
    fn sample_curve_derivative_x(&self, t: f32) -> f32 {
        let inv_t = 1.0 - t;
        3.0 * inv_t * inv_t * self.x1
            + 6.0 * inv_t * t * (self.x2 - self.x1)
            + 3.0 * t * t * (1.0 - self.x2)
    }

    /// Solve for parameter `t` given x-axis progress using Newton-Raphson.
    pub fn solve_curve_x(&self, x: f32) -> f32 {
        if x <= 0.0 {
            return 0.0;
        }
        if x >= 1.0 {
            return 1.0;
        }

        let mut t = x; // Initial estimate
        for _ in 0..8 {
            let current_x = self.sample_curve_x(t) - x;
            if current_x.abs() < 1e-5 {
                return t;
            }
            let d_x = self.sample_curve_derivative_x(t);
            if d_x.abs() < 1e-6 {
                break;
            }
            t -= current_x / d_x;
            if t < 0.0 || t > 1.0 {
                break;
            }
        }

        // Bisection fallback
        let mut t_low = 0.0f32;
        let mut t_high = 1.0f32;
        t = x;

        while t_low < t_high {
            let current_x = self.sample_curve_x(t);
            if (current_x - x).abs() < 1e-4 {
                return t;
            }
            if x > current_x {
                t_low = t;
            } else {
                t_high = t;
            }
            t = (t_high + t_low) * 0.5;
        }

        t
    }

    /// Evaluate normalized easing progress (0.0 to 1.0).
    pub fn evaluate(&self, progress: f32) -> f32 {
        if progress <= 0.0 {
            return 0.0;
        }
        if progress >= 1.0 {
            return 1.0;
        }
        let t = self.solve_curve_x(progress);
        self.sample_curve_y(t)
    }
}

/// Curve selector for window animations.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AnimationCurve {
    Bezier(CubicBezier),
    Spring(SpringModel),
}

impl AnimationCurve {
    pub fn evaluate(&self, progress: f32, elapsed_sec: f32) -> f32 {
        match self {
            AnimationCurve::Bezier(b) => b.evaluate(progress),
            AnimationCurve::Spring(s) => s.evaluate(elapsed_sec).clamp(0.0, 1.15),
        }
    }
}

/// Specific window animation transition kinds.
#[derive(Debug, Clone, PartialEq)]
pub enum WindowAnimationType {
    Minimize {
        start_rect: DwmRect,
        target_icon_rect: DwmRect,
        start_opacity: f32,
        target_opacity: f32,
        start_scale: f32,
        target_scale: f32,
    },
    Maximize {
        start_rect: DwmRect,
        target_rect: DwmRect,
        start_radius: f32,
        target_radius: f32,
    },
    Restore {
        start_rect: DwmRect,
        target_rect: DwmRect,
        start_opacity: f32,
        target_opacity: f32,
        start_scale: f32,
        target_scale: f32,
        start_radius: f32,
        target_radius: f32,
    },
    OpenScaleFade {
        start_scale: f32,
        target_scale: f32,
        start_opacity: f32,
        target_opacity: f32,
        rect: DwmRect,
    },
    CloseScaleFade {
        start_scale: f32,
        target_scale: f32,
        start_opacity: f32,
        target_opacity: f32,
        rect: DwmRect,
    },
    SnapTransition {
        start_rect: DwmRect,
        target_rect: DwmRect,
    },
}

/// Active physics animation controller for a DWM Window.
#[derive(Debug, Clone)]
pub struct WindowAnimation {
    pub anim_type: WindowAnimationType,
    pub curve: AnimationCurve,
    pub duration_ms: f32,
    pub elapsed_ms: f32,
    pub is_finished: bool,
    pub current_rect: DwmRect,
    pub current_opacity: f32,
    pub current_scale: f32,
    pub current_corner_radius: f32,
}

impl WindowAnimation {
    pub fn new_minimize(start_rect: DwmRect, target_icon_rect: DwmRect) -> Self {
        Self {
            anim_type: WindowAnimationType::Minimize {
                start_rect,
                target_icon_rect,
                start_opacity: 1.0,
                target_opacity: 0.0,
                start_scale: 1.0,
                target_scale: 0.65,
            },
            curve: AnimationCurve::Bezier(CubicBezier::STANDARD_EASE),
            duration_ms: 220.0,
            elapsed_ms: 0.0,
            is_finished: false,
            current_rect: start_rect,
            current_opacity: 1.0,
            current_scale: 1.0,
            current_corner_radius: 8.0,
        }
    }

    pub fn new_maximize(start_rect: DwmRect, target_rect: DwmRect) -> Self {
        Self {
            anim_type: WindowAnimationType::Maximize {
                start_rect,
                target_rect,
                start_radius: 8.0,
                target_radius: 0.0,
            },
            curve: AnimationCurve::Spring(SpringModel::gentle()),
            duration_ms: 260.0,
            elapsed_ms: 0.0,
            is_finished: false,
            current_rect: start_rect,
            current_opacity: 1.0,
            current_scale: 1.0,
            current_corner_radius: 8.0,
        }
    }

    pub fn new_restore(start_rect: DwmRect, target_rect: DwmRect) -> Self {
        Self {
            anim_type: WindowAnimationType::Restore {
                start_rect,
                target_rect,
                start_opacity: 0.2,
                target_opacity: 1.0,
                start_scale: 0.85,
                target_scale: 1.0,
                start_radius: 0.0,
                target_radius: 8.0,
            },
            curve: AnimationCurve::Spring(SpringModel::gentle()),
            duration_ms: 240.0,
            elapsed_ms: 0.0,
            is_finished: false,
            current_rect: start_rect,
            current_opacity: 0.2,
            current_scale: 0.85,
            current_corner_radius: 0.0,
        }
    }

    pub fn new_open_scale_fade(rect: DwmRect) -> Self {
        Self {
            anim_type: WindowAnimationType::OpenScaleFade {
                start_scale: 0.92,
                target_scale: 1.0,
                start_opacity: 0.0,
                target_opacity: 1.0,
                rect,
            },
            curve: AnimationCurve::Bezier(CubicBezier::DECELERATE),
            duration_ms: 180.0,
            elapsed_ms: 0.0,
            is_finished: false,
            current_rect: rect,
            current_opacity: 0.0,
            current_scale: 0.92,
            current_corner_radius: 8.0,
        }
    }

    pub fn new_close_scale_fade(rect: DwmRect) -> Self {
        Self {
            anim_type: WindowAnimationType::CloseScaleFade {
                start_scale: 1.0,
                target_scale: 0.94,
                start_opacity: 1.0,
                target_opacity: 0.0,
                rect,
            },
            curve: AnimationCurve::Bezier(CubicBezier::ACCELERATE),
            duration_ms: 140.0,
            elapsed_ms: 0.0,
            is_finished: false,
            current_rect: rect,
            current_opacity: 1.0,
            current_scale: 1.0,
            current_corner_radius: 8.0,
        }
    }

    pub fn new_snap_transition(start_rect: DwmRect, target_rect: DwmRect) -> Self {
        Self {
            anim_type: WindowAnimationType::SnapTransition {
                start_rect,
                target_rect,
            },
            curve: AnimationCurve::Spring(SpringModel::snappy()),
            duration_ms: 200.0,
            elapsed_ms: 0.0,
            is_finished: false,
            current_rect: start_rect,
            current_opacity: 1.0,
            current_scale: 1.0,
            current_corner_radius: 8.0,
        }
    }

    /// Advance animation by delta milliseconds.
    pub fn step(&mut self, delta_ms: f32) -> bool {
        if self.is_finished {
            return false;
        }

        self.elapsed_ms += delta_ms;
        let progress = (self.elapsed_ms / self.duration_ms).clamp(0.0, 1.0);
        let elapsed_sec = self.elapsed_ms / 1000.0;
        let t = self.curve.evaluate(progress, elapsed_sec);

        match &self.anim_type {
            WindowAnimationType::Minimize {
                start_rect,
                target_icon_rect,
                start_opacity,
                target_opacity,
                start_scale,
                target_scale,
            } => {
                self.current_rect = interpolate_rect(start_rect, target_icon_rect, t);
                self.current_opacity = start_opacity + (target_opacity - start_opacity) * t;
                self.current_scale = start_scale + (target_scale - start_scale) * t;
            }

            WindowAnimationType::Maximize {
                start_rect,
                target_rect,
                start_radius,
                target_radius,
            } => {
                self.current_rect = interpolate_rect(start_rect, target_rect, t);
                self.current_corner_radius = start_radius + (target_radius - start_radius) * t;
                self.current_opacity = 1.0;
                self.current_scale = 1.0;
            }

            WindowAnimationType::Restore {
                start_rect,
                target_rect,
                start_opacity,
                target_opacity,
                start_scale,
                target_scale,
                start_radius,
                target_radius,
            } => {
                self.current_rect = interpolate_rect(start_rect, target_rect, t);
                self.current_opacity = start_opacity + (target_opacity - start_opacity) * t;
                self.current_scale = start_scale + (target_scale - start_scale) * t;
                self.current_corner_radius = start_radius + (target_radius - start_radius) * t;
            }

            WindowAnimationType::OpenScaleFade {
                start_scale,
                target_scale,
                start_opacity,
                target_opacity,
                rect,
            } => {
                self.current_rect = *rect;
                self.current_scale = start_scale + (target_scale - start_scale) * t;
                self.current_opacity = start_opacity + (target_opacity - start_opacity) * t;
            }

            WindowAnimationType::CloseScaleFade {
                start_scale,
                target_scale,
                start_opacity,
                target_opacity,
                rect,
            } => {
                self.current_rect = *rect;
                self.current_scale = start_scale + (target_scale - start_scale) * t;
                self.current_opacity = start_opacity + (target_opacity - start_opacity) * t;
            }

            WindowAnimationType::SnapTransition {
                start_rect,
                target_rect,
            } => {
                self.current_rect = interpolate_rect(start_rect, target_rect, t);
                self.current_opacity = 1.0;
                self.current_scale = 1.0;
                self.current_corner_radius = 8.0;
            }
        }

        if self.elapsed_ms >= self.duration_ms {
            self.is_finished = true;
            // Snap final parameters to target
            match &self.anim_type {
                WindowAnimationType::Minimize { target_icon_rect, target_opacity, target_scale, .. } => {
                    self.current_rect = *target_icon_rect;
                    self.current_opacity = *target_opacity;
                    self.current_scale = *target_scale;
                }
                WindowAnimationType::Maximize { target_rect, target_radius, .. } => {
                    self.current_rect = *target_rect;
                    self.current_corner_radius = *target_radius;
                }
                WindowAnimationType::Restore { target_rect, target_opacity, target_scale, target_radius, .. } => {
                    self.current_rect = *target_rect;
                    self.current_opacity = *target_opacity;
                    self.current_scale = *target_scale;
                    self.current_corner_radius = *target_radius;
                }
                WindowAnimationType::OpenScaleFade { target_scale, target_opacity, .. } => {
                    self.current_scale = *target_scale;
                    self.current_opacity = *target_opacity;
                }
                WindowAnimationType::CloseScaleFade { target_scale, target_opacity, .. } => {
                    self.current_scale = *target_scale;
                    self.current_opacity = *target_opacity;
                }
                WindowAnimationType::SnapTransition { target_rect, .. } => {
                    self.current_rect = *target_rect;
                }
            }
            false
        } else {
            true
        }
    }
}

fn interpolate_rect(start: &DwmRect, target: &DwmRect, t: f32) -> DwmRect {
    let x = (start.x as f32 + (target.x as f32 - start.x as f32) * t).round() as i32;
    let y = (start.y as f32 + (target.y as f32 - start.y as f32) * t).round() as i32;
    let w = ((start.width as f32 + (target.width as f32 - start.width as f32) * t).round() as u32).max(1);
    let h = ((start.height as f32 + (target.height as f32 - start.height as f32) * t).round() as u32).max(1);
    DwmRect::new(x, y, w, h)
}

// ============================================================================
// Section 5: Aero Shake Detection Engine
// ============================================================================

/// Motion sample point for tracking mouse trajectory during window dragging.
#[derive(Debug, Clone, Copy, PartialEq)]
struct DragSample {
    timestamp_ms: f64,
    x: i32,
    y: i32,
}

/// Aero Shake Gesture Detector.
///
/// Detects rapid back-and-forth movement while dragging an active window's titlebar.
/// First shake minimizes all background windows.
/// Second shake restores the previously minimized windows!
#[derive(Debug, Clone)]
pub struct AeroShakeDetector {
    /// Window currently being dragged by its titlebar.
    pub dragging_window: Option<DwmWindowId>,
    /// Recent motion trajectory points.
    samples: Vec<DragSample>,
    /// Background windows minimized by the first shake gesture.
    pub shaken_minimized_windows: Vec<DwmWindowId>,
    /// Last detected shake gesture timestamp to debounce triggers.
    pub last_shake_trigger_ms: f64,
    /// Minimum direction reversals required (e.g. 3 direction changes).
    pub min_reversals: usize,
    /// Minimum travel displacement per stroke in pixels.
    pub stroke_threshold: i32,
    /// Time window for shake gesture detection in milliseconds.
    pub time_window_ms: f64,
}

impl AeroShakeDetector {
    pub fn new() -> Self {
        Self {
            dragging_window: None,
            samples: Vec::with_capacity(32),
            shaken_minimized_windows: Vec::new(),
            last_shake_trigger_ms: 0.0,
            min_reversals: 3,
            stroke_threshold: 24,
            time_window_ms: 500.0,
        }
    }

    /// Begin tracking mouse motion when user drags a window's titlebar.
    pub fn start_drag(&mut self, window_id: DwmWindowId, x: i32, y: i32, timestamp_ms: f64) {
        self.dragging_window = Some(window_id);
        self.samples.clear();
        self.samples.push(DragSample { timestamp_ms, x, y });
    }

    /// Update mouse motion while dragging. Returns `true` if an Aero Shake gesture was recognized.
    pub fn update_drag(&mut self, x: i32, y: i32, timestamp_ms: f64) -> bool {
        if self.dragging_window.is_none() {
            return false;
        }

        // Debounce: don't trigger multiple shakes within 600ms
        if timestamp_ms - self.last_shake_trigger_ms < 600.0 {
            return false;
        }

        self.samples.push(DragSample { timestamp_ms, x, y });

        // Prune older samples outside detection window
        let cutoff = timestamp_ms - self.time_window_ms;
        self.samples.retain(|s| s.timestamp_ms >= cutoff);

        if self.samples.len() < 4 {
            return false;
        }

        // Count direction reversals in X axis
        let mut reversals_x = 0;
        let mut last_dir_x = 0; // -1: left, 1: right, 0: initial

        for i in 1..self.samples.len() {
            let dx = self.samples[i].x - self.samples[i - 1].x;
            if dx.abs() >= self.stroke_threshold {
                let current_dir = if dx > 0 { 1 } else { -1 };
                if last_dir_x != 0 && current_dir != last_dir_x {
                    reversals_x += 1;
                }
                last_dir_x = current_dir;
            }
        }

        if reversals_x >= self.min_reversals {
            self.last_shake_trigger_ms = timestamp_ms;
            self.samples.clear();
            return true;
        }

        false
    }

    /// Finish window dragging gesture.
    pub fn end_drag(&mut self) {
        self.dragging_window = None;
        self.samples.clear();
    }
}

// ============================================================================
// Section 6: Dirty Region Tracking & Coalescing Engine
// ============================================================================

/// Tracks damaged/invalidated screen areas and maintains a list of non-overlapping rectangles.
#[derive(Debug, Clone)]
pub struct DirtyRectTracker {
    /// Screen or viewport boundaries.
    pub bounds: DwmRect,
    /// Disjoint (non-overlapping) damaged rectangles scheduled for compositing.
    rects: Vec<DwmRect>,
    /// Maximum count of disjoint rects before automatic coalescing to bounding box.
    pub max_disjoint_count: usize,
}

impl DirtyRectTracker {
    pub fn new(bounds: DwmRect) -> Self {
        Self {
            bounds,
            rects: Vec::with_capacity(16),
            max_disjoint_count: 24,
        }
    }

    #[inline]
    pub fn is_dirty(&self) -> bool {
        !self.rects.is_empty()
    }

    #[inline]
    pub fn dirty_rects(&self) -> &[DwmRect] {
        &self.rects
    }

    #[inline]
    pub fn clear(&mut self) {
        self.rects.clear();
    }

    /// Invalidate entire screen bounds.
    pub fn invalidate_all(&mut self) {
        self.rects.clear();
        if !self.bounds.is_empty() {
            self.rects.push(self.bounds);
        }
    }

    /// Invalidate a specific window's rendered bounding box.
    pub fn invalidate_window(&mut self, window: &DwmWindow) {
        if window.visible && !window.rect.is_empty() {
            self.add_dirty_rect(window.rect);
        }
    }

    /// Add a dirty rectangle, clipping to screen bounds and merging with overlapping rects.
    pub fn add_dirty_rect(&mut self, rect: DwmRect) {
        let clipped = match rect.intersection(&self.bounds) {
            Some(r) => r,
            None => return,
        };

        if clipped.is_empty() {
            return;
        }

        // If whole screen is already dirty, nothing more to do
        if self.rects.len() == 1 && self.rects[0] == self.bounds {
            return;
        }

        // Check if `clipped` is already contained inside an existing rect
        for existing in &self.rects {
            if existing.contains_rect(&clipped) {
                return;
            }
        }

        // Partition or merge with intersecting rectangles
        let mut new_rect = clipped;
        let mut i = 0;
        while i < self.rects.len() {
            if self.rects[i].intersects(&new_rect) {
                let merged = self.rects[i].union(&new_rect);
                // Merge if union doesn't introduce excessive empty wasted area (>35% overhead)
                let combined_individual_area = self.rects[i].area() + new_rect.area();
                let merged_area = merged.area();
                if merged_area <= (combined_individual_area * 135) / 100 {
                    new_rect = merged;
                    self.rects.swap_remove(i);
                    continue;
                }
            }
            i += 1;
        }

        self.rects.push(new_rect);

        // Coalesce if fragmentation exceeds threshold
        if self.rects.len() > self.max_disjoint_count {
            self.coalesce();
        }
    }

    /// Coalesce all dirty rectangles into their minimal encompassing bounding box.
    pub fn coalesce(&mut self) {
        if self.rects.len() <= 1 {
            return;
        }

        let mut unified = self.rects[0];
        for r in &self.rects[1..] {
            unified = unified.union(r);
        }

        self.rects.clear();
        self.rects.push(unified);
    }

    /// Calculate total accumulated dirty pixel area.
    pub fn total_dirty_area(&self) -> u64 {
        self.rects.iter().map(|r| r.area()).sum()
    }
}

// ============================================================================
// Section 7: Hardware Cursor Plane & Fluent Direct Composition Pipeline
// ============================================================================

/// Hardware Cursor Shape Type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorShape {
    Arrow,
    Hand,
    IBeam,
    Crosshair,
    ResizeHorizontal,
    ResizeVertical,
    Move,
}

/// Independent Hardware Cursor Plane.
///
/// Rendered directly onto the final frame without forcing underlying window invalidations.
#[derive(Debug, Clone)]
pub struct HardwareCursor {
    pub x: i32,
    pub y: i32,
    pub visible: bool,
    pub shape: CursorShape,
    pub sprite: DwmSurface,
}

impl HardwareCursor {
    pub fn new() -> Self {
        let mut sprite = DwmSurface::new(16, 16, DwmColor::TRANSPARENT);
        // Draw standard Windows 11 Fluent arrow cursor sprite
        for y in 0..16 {
            for x in 0..=y.min(10) {
                if x == 0 || x == y || y == 10 {
                    sprite.set_pixel(x, y, DwmColor::BLACK);
                } else {
                    sprite.set_pixel(x, y, DwmColor::WHITE);
                }
            }
        }

        Self {
            x: 0,
            y: 0,
            visible: true,
            shape: CursorShape::Arrow,
            sprite,
        }
    }

    pub fn set_position(&mut self, x: i32, y: i32) {
        self.x = x;
        self.y = y;
    }

    pub fn set_shape(&mut self, shape: CursorShape) {
        self.shape = shape;
    }
}

/// Frame timing and VSync tick telemetry generator.
#[derive(Debug, Clone)]
pub struct VSyncTickGenerator {
    /// Target frame rate (e.g. 60 or 120 FPS).
    pub target_fps: u32,
    /// Nominal frame period in milliseconds (16.666 ms for 60Hz, 8.333 ms for 120Hz).
    pub target_interval_ms: f64,
    /// Monotonically increasing frame sequence counter.
    pub frame_counter: u64,
    /// Timestamp of the last frame in milliseconds.
    pub last_tick_timestamp_ms: f64,
    /// Total count of dropped / missed frames.
    pub dropped_frames: u64,
    /// Measured moving average FPS over recent ticks.
    pub measured_fps: f64,
}

/// Telemetry emitted on each VSync tick.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VSyncTickEvent {
    pub frame_index: u64,
    pub delta_ms: f64,
    pub timestamp_ms: f64,
    pub dropped_frames_count: u64,
    pub measured_fps: f64,
}

impl VSyncTickGenerator {
    pub fn new(target_fps: u32) -> Self {
        let fps = target_fps.max(1);
        let interval = 1000.0 / (fps as f64);
        Self {
            target_fps: fps,
            target_interval_ms: interval,
            frame_counter: 0,
            last_tick_timestamp_ms: 0.0,
            dropped_frames: 0,
            measured_fps: fps as f64,
        }
    }

    pub fn set_fps(&mut self, target_fps: u32) {
        let fps = target_fps.max(1);
        self.target_fps = fps;
        self.target_interval_ms = 1000.0 / (fps as f64);
    }

    /// Advance time and evaluate VSync ticks.
    pub fn advance_time(&mut self, timestamp_ms: f64) -> Option<VSyncTickEvent> {
        if self.frame_counter == 0 {
            self.last_tick_timestamp_ms = timestamp_ms;
            self.frame_counter = 1;
            return Some(VSyncTickEvent {
                frame_index: 1,
                delta_ms: self.target_interval_ms,
                timestamp_ms,
                dropped_frames_count: 0,
                measured_fps: self.target_fps as f64,
            });
        }

        let delta = timestamp_ms - self.last_tick_timestamp_ms;
        if delta < (self.target_interval_ms * 0.8) {
            return None; // Too early for next frame
        }

        // Check for dropped frames
        if delta >= (self.target_interval_ms * 1.8) {
            let missed = ((delta / self.target_interval_ms).floor() as u64).saturating_sub(1);
            self.dropped_frames += missed;
        }

        self.frame_counter += 1;
        self.last_tick_timestamp_ms = timestamp_ms;

        // Smooth moving average FPS
        let instant_fps = if delta > 0.0 { 1000.0 / delta } else { self.target_fps as f64 };
        self.measured_fps = self.measured_fps * 0.9 + instant_fps * 0.1;

        Some(VSyncTickEvent {
            frame_index: self.frame_counter,
            delta_ms: delta,
            timestamp_ms,
            dropped_frames_count: self.dropped_frames,
            measured_fps: self.measured_fps,
        })
    }
}

// ============================================================================
// Section 8: Top-Level DwmCompositor Engine
// ============================================================================

/// Production Windows 11 Desktop Window Manager & Advanced Surface Compositor.
#[derive(Debug)]
pub struct DwmCompositor {
    /// Screen dimensions.
    pub width: usize,
    pub height: usize,
    /// Usable desktop work area excluding taskbar.
    pub work_area: DwmRect,
    /// Desktop window dictionary indexed by Window ID / Win32 HWND.
    pub windows: HashMap<DwmWindowId, DwmWindow>,
    /// Global back-to-front Z-order list of window IDs (first = back, last = front).
    pub z_order: Vec<DwmWindowId>,
    /// Currently focused / active window.
    pub focused_window: Option<DwmWindowId>,
    /// Next window ID allocator.
    next_window_id: DwmWindowId,
    /// Windows 11 Snap Engine.
    pub snap_engine: SnapEngine,
    /// Windows 11 Snap Assist picker engine.
    pub snap_assist: SnapAssist,
    /// Active snap preview ghost rectangle if user is dragging near snap zones.
    pub active_snap_preview: Option<SnapZone>,
    /// Aero Shake gesture detector.
    pub aero_shake: AeroShakeDetector,
    /// Dirty rectangle tracker for optimized partial recomposition.
    pub dirty_tracker: DirtyRectTracker,
    /// Hardware cursor plane.
    pub cursor: HardwareCursor,
    /// Frame timing and VSync tick telemetry generator.
    pub vsync: VSyncTickGenerator,
    /// Desktop background wallpaper surface.
    pub wallpaper: DwmSurface,
    /// Primary direct composition target buffer (front buffer).
    pub framebuffer: DwmSurface,
}

impl DwmCompositor {
    /// Initialize a new DWM Compositor instance.
    pub fn new(width: usize, height: usize, taskbar_height: u32, target_fps: u32) -> Self {
        let screen_rect = DwmRect::new(0, 0, width as u32, height as u32);
        let work_area = DwmRect::new(
            0,
            0,
            width as u32,
            (height as u32).saturating_sub(taskbar_height),
        );

        // Windows 11 signature desktop wallpaper gradient (Deep Blue to Soft Teal)
        let mut wallpaper = DwmSurface::new(width, height, DwmColor::BLACK);
        let top_color = DwmColor::rgb(14, 27, 49);
        let bottom_color = DwmColor::rgb(19, 78, 94);

        for y in 0..height {
            let t = y as f32 / height.max(1) as f32;
            let row_color = top_color.lerp(bottom_color, t);
            for x in 0..width {
                wallpaper.set_pixel(x, y, row_color);
            }
        }

        let framebuffer = DwmSurface::new(width, height, DwmColor::BLACK);

        let mut compositor = Self {
            width,
            height,
            work_area,
            windows: HashMap::new(),
            z_order: Vec::new(),
            focused_window: None,
            next_window_id: 1000,
            snap_engine: SnapEngine::new(work_area),
            snap_assist: SnapAssist::new(),
            active_snap_preview: None,
            aero_shake: AeroShakeDetector::new(),
            dirty_tracker: DirtyRectTracker::new(screen_rect),
            cursor: HardwareCursor::new(),
            vsync: VSyncTickGenerator::new(target_fps),
            wallpaper,
            framebuffer,
        };

        compositor.dirty_tracker.invalidate_all();
        compositor
    }

    /// Create and register a new managed window.
    pub fn create_window(&mut self, title: impl Into<String>, rect: DwmRect) -> DwmWindowId {
        let id = self.next_window_id;
        self.next_window_id += 1;

        let mut window = DwmWindow::new(id, title, rect);
        // Create initial default surface for window
        let mut surface = DwmSurface::new(rect.width as usize, rect.height as usize, DwmColor::MICA_DARK);
        // Draw standard Fluent window titlebar area
        surface.fill_rect(DwmRect::new(0, 0, rect.width, 32), DwmColor::TITLEBAR_DARK);
        window.surface = Some(surface);

        // Place at top of Z-order
        let z = self.z_order.len();
        window.z_order = z;

        self.dirty_tracker.invalidate_window(&window);
        self.windows.insert(id, window);
        self.z_order.push(id);

        self.set_focus(id);
        id
    }

    /// Destroy and unregister a window.
    pub fn destroy_window(&mut self, id: DwmWindowId) -> bool {
        if let Some(w) = self.windows.remove(&id) {
            self.dirty_tracker.invalidate_window(&w);
            self.z_order.retain(|&wid| wid != id);

            if self.focused_window == Some(id) {
                self.focused_window = self.z_order.last().copied();
                if let Some(new_focus) = self.focused_window {
                    if let Some(w) = self.windows.get_mut(&new_focus) {
                        w.set_active(true);
                        self.dirty_tracker.invalidate_window(w);
                    }
                }
            }
            self.reindex_z_orders();
            true
        } else {
            false
        }
    }

    /// Bring window to the front of the Z-order hierarchy.
    pub fn bring_to_front(&mut self, id: DwmWindowId) {
        if let Some(pos) = self.z_order.iter().position(|&wid| wid == id) {
            self.z_order.remove(pos);
            self.z_order.push(id);
            self.reindex_z_orders();

            if let Some(w) = self.windows.get(&id) {
                self.dirty_tracker.invalidate_window(w);
            }
            self.set_focus(id);
        }
    }

    /// Send window to the bottom of the Z-order hierarchy.
    pub fn send_to_back(&mut self, id: DwmWindowId) {
        if let Some(pos) = self.z_order.iter().position(|&wid| wid == id) {
            self.z_order.remove(pos);
            self.z_order.insert(0, id);
            self.reindex_z_orders();

            if let Some(w) = self.windows.get(&id) {
                self.dirty_tracker.invalidate_window(w);
            }

            if self.focused_window == Some(id) {
                self.focused_window = self.z_order.last().copied();
                if let Some(new_focus) = self.focused_window {
                    if let Some(w) = self.windows.get_mut(&new_focus) {
                        w.set_active(true);
                        self.dirty_tracker.invalidate_window(w);
                    }
                }
            }
        }
    }

    /// Set active focus to a window.
    pub fn set_focus(&mut self, id: DwmWindowId) {
        if self.focused_window == Some(id) {
            return;
        }

        if let Some(prev) = self.focused_window {
            if let Some(prev_window) = self.windows.get_mut(&prev) {
                prev_window.set_active(false);
                self.dirty_tracker.invalidate_window(prev_window);
            }
        }

        self.focused_window = Some(id);
        if let Some(window) = self.windows.get_mut(&id) {
            window.set_active(true);
            self.dirty_tracker.invalidate_window(window);
        }
    }

    fn reindex_z_orders(&mut self) {
        for (idx, &id) in self.z_order.iter().enumerate() {
            if let Some(w) = self.windows.get_mut(&id) {
                w.z_order = idx;
            }
        }
    }

    /// Minimize window to taskbar.
    pub fn minimize_window(&mut self, id: DwmWindowId, taskbar_rect: Option<DwmRect>) {
        if let Some(w) = self.windows.get_mut(&id) {
            self.dirty_tracker.invalidate_window(w);
            w.minimize(taskbar_rect);
            self.dirty_tracker.invalidate_window(w);

            if self.focused_window == Some(id) {
                self.focused_window = self.z_order
                    .iter()
                    .rev()
                    .find(|&&wid| wid != id && self.windows.get(&wid).map_or(false, |w| w.visible && w.state != WindowState::Minimized))
                    .copied();

                if let Some(new_focus) = self.focused_window {
                    if let Some(fw) = self.windows.get_mut(&new_focus) {
                        fw.set_active(true);
                        self.dirty_tracker.invalidate_window(fw);
                    }
                }
            }
        }
    }

    /// Maximize window to cover the work area.
    pub fn maximize_window(&mut self, id: DwmWindowId) {
        let work_area = self.work_area;
        if let Some(w) = self.windows.get_mut(&id) {
            self.dirty_tracker.invalidate_window(w);
            w.maximize(work_area);
            self.dirty_tracker.invalidate_window(w);
        }
    }

    /// Restore window from minimized/maximized/snapped state.
    pub fn restore_window(&mut self, id: DwmWindowId) {
        if let Some(w) = self.windows.get_mut(&id) {
            self.dirty_tracker.invalidate_window(w);
            w.restore();
            self.dirty_tracker.invalidate_window(w);
            self.set_focus(id);
        }
    }

    /// Snap window directly into a SnapZone.
    pub fn snap_window(&mut self, id: DwmWindowId, zone: SnapZone) {
        let target_rect = self.snap_engine.snap_zone_to_rect(zone);
        if let Some(w) = self.windows.get_mut(&id) {
            self.dirty_tracker.invalidate_window(w);
            w.snap(zone, target_rect);
            self.dirty_tracker.invalidate_window(w);
        }
    }

    /// Handle pointer drag during titlebar dragging: checks for Aero Shake and edge snapping zones.
    pub fn handle_window_drag_motion(
        &mut self,
        window_id: DwmWindowId,
        cursor_x: i32,
        cursor_y: i32,
        timestamp_ms: f64,
    ) {
        self.cursor.set_position(cursor_x, cursor_y);

        // 1. Aero Shake check
        let shaken = self.aero_shake.update_drag(cursor_x, cursor_y, timestamp_ms);
        if shaken {
            self.toggle_aero_shake(window_id);
        }

        // 2. Snap Zone trigger detection
        let detected_zone = self.snap_engine.detect_snap_zone(cursor_x, cursor_y);
        if detected_zone != self.active_snap_preview {
            if let Some(prev) = self.active_snap_preview {
                let rect = self.snap_engine.snap_zone_to_rect(prev);
                self.dirty_tracker.add_dirty_rect(rect);
            }
            if let Some(next_zone) = detected_zone {
                let rect = self.snap_engine.snap_zone_to_rect(next_zone);
                self.dirty_tracker.add_dirty_rect(rect);
            }
            self.active_snap_preview = detected_zone;
        }
    }

    /// Toggle Aero Shake: minimizes all background windows on first shake, restores on second shake.
    pub fn toggle_aero_shake(&mut self, active_window_id: DwmWindowId) {
        if self.aero_shake.shaken_minimized_windows.is_empty() {
            // First shake: Minimize all other visible windows
            let mut to_minimize = Vec::new();
            for (&id, window) in &self.windows {
                if id != active_window_id
                    && window.visible
                    && window.state != WindowState::Minimized
                    && !window.flags.aero_shake_immune
                {
                    to_minimize.push(id);
                }
            }

            for &id in &to_minimize {
                self.minimize_window(id, None);
            }
            self.aero_shake.shaken_minimized_windows = to_minimize;
        } else {
            // Second shake: Restore previously minimized windows
            let to_restore = std::mem::take(&mut self.aero_shake.shaken_minimized_windows);
            for id in to_restore {
                self.restore_window(id);
            }
        }
    }

    /// Finish window drag: applies snap if preview was active.
    pub fn handle_window_drag_end(&mut self, window_id: DwmWindowId) {
        self.aero_shake.end_drag();

        if let Some(zone) = self.active_snap_preview.take() {
            let ghost_rect = self.snap_engine.snap_zone_to_rect(zone);
            self.dirty_tracker.add_dirty_rect(ghost_rect);

            self.snap_window(window_id, zone);

            // Trigger Snap Assist if snapping into a two-column or quarter layout
            let layout = match zone {
                SnapZone::LeftHalf | SnapZone::RightHalf => Some((SnapLayoutType::TwoColumnEqual, if zone == SnapZone::LeftHalf { 0 } else { 1 })),
                SnapZone::TopLeftQuarter => Some((SnapLayoutType::FourQuarter, 0)),
                SnapZone::TopRightQuarter => Some((SnapLayoutType::FourQuarter, 1)),
                SnapZone::BottomLeftQuarter => Some((SnapLayoutType::FourQuarter, 2)),
                SnapZone::BottomRightQuarter => Some((SnapLayoutType::FourQuarter, 3)),
                SnapZone::LayoutSlot { layout, slot_index } => Some((layout, slot_index)),
                SnapZone::Maximize => None,
            };

            if let Some((snap_layout, slot_idx)) = layout {
                let windows_vec: Vec<DwmWindow> = self.windows.values().cloned().collect();
                self.snap_assist.activate(&self.snap_engine, snap_layout, slot_idx, window_id, &windows_vec);
            }
        }
    }

    /// Advance active animations by delta milliseconds.
    pub fn tick_animations(&mut self, delta_ms: f32) {
        let mut dirty_rects_to_add = Vec::new();

        for window in self.windows.values_mut() {
            if window.animation.is_some() {
                dirty_rects_to_add.push(window.rect);
                window.step_animation(delta_ms);
                dirty_rects_to_add.push(window.rect);
            }
        }

        for r in dirty_rects_to_add {
            self.dirty_tracker.add_dirty_rect(r);
        }
    }

    /// Direct Composition Pass: Composites all planes into the final framebuffer.
    ///
    /// Plane Order:
    ///   0. Background Wallpaper Plane
    ///   1. Window Surface Plane (Z-ordered, with shadows, rounded corners, and materials)
    ///   2. Top-Level Flyouts & Snap Assist Overlays
    ///   3. Hardware Cursor Plane
    ///
    /// Returns the count of dirty rectangles recomposited.
    pub fn compose_frame(&mut self) -> usize {
        if !self.dirty_tracker.is_dirty() {
            return 0;
        }

        let dirty_rects = self.dirty_tracker.dirty_rects().to_vec();
        let rect_count = dirty_rects.len();

        for dirty_rect in &dirty_rects {
            // Plane 0: Background Wallpaper
            self.framebuffer.blit(&self.wallpaper, 0, 0, Some(*dirty_rect), 1.0);

            // Plane 1: Window Surfaces in ascending Z-order
            for &id in &self.z_order {
                if let Some(window) = self.windows.get(&id) {
                    if !window.visible || window.state == WindowState::Minimized {
                        continue;
                    }

                    if !window.rect.intersects(dirty_rect) {
                        continue;
                    }

                    let w_rect = window.rect;

                    // Draw elevation drop shadow if decorated
                    if window.decorated && !window.borderless && window.state != WindowState::Maximized {
                        let shadow_rect = DwmRect::new(w_rect.x + 2, w_rect.y + 4, w_rect.width + 4, w_rect.height + 4);
                        if shadow_rect.intersects(dirty_rect) {
                            let shadow_color = DwmColor::rgba(0, 0, 0, (40.0 * window.opacity) as u8);
                            self.framebuffer.fill_rect(shadow_rect, shadow_color);
                        }
                    }

                    // Blit window surface content
                    if let Some(surf) = &window.surface {
                        self.framebuffer.blit(
                            surf,
                            w_rect.x,
                            w_rect.y,
                            Some(*dirty_rect),
                            window.opacity,
                        );
                    } else {
                        // Default solid surface tint
                        let win_color = if window.is_mica {
                            DwmColor::MICA_DARK.with_alpha_f32(window.opacity)
                        } else {
                            DwmColor::rgb(28, 28, 28).with_alpha_f32(window.opacity)
                        };
                        self.framebuffer.fill_rect(w_rect, win_color);
                    }

                    // Render subtle 1px border if decorated
                    if window.decorated && !window.borderless && window.corner_radius > 0.0 {
                        let border_color = if window.active {
                            DwmColor::BORDER_DARK.with_alpha(50)
                        } else {
                            DwmColor::BORDER_DARK
                        };
                        // Draw 1px border lines
                        self.framebuffer.fill_rect(DwmRect::new(w_rect.x, w_rect.y, w_rect.width, 1), border_color);
                        self.framebuffer.fill_rect(DwmRect::new(w_rect.x, w_rect.bottom() - 1, w_rect.width, 1), border_color);
                        self.framebuffer.fill_rect(DwmRect::new(w_rect.x, w_rect.y, 1, w_rect.height), border_color);
                        self.framebuffer.fill_rect(DwmRect::new(w_rect.right() - 1, w_rect.y, 1, w_rect.height), border_color);
                    }
                }
            }

            // Plane 2: Snap Preview Ghost Overlay
            if let Some(zone) = self.active_snap_preview {
                let ghost_rect = self.snap_engine.snap_zone_to_rect(zone);
                if ghost_rect.intersects(dirty_rect) {
                    // Fill translucent blue ghost acrylic box
                    self.framebuffer.fill_rect(ghost_rect, DwmColor::SNAP_GHOST_BLUE);
                    // 2px accent border
                    let b = DwmColor::SNAP_GHOST_BORDER;
                    self.framebuffer.fill_rect(DwmRect::new(ghost_rect.x, ghost_rect.y, ghost_rect.width, 2), b);
                    self.framebuffer.fill_rect(DwmRect::new(ghost_rect.x, ghost_rect.bottom() - 2, ghost_rect.width, 2), b);
                    self.framebuffer.fill_rect(DwmRect::new(ghost_rect.x, ghost_rect.y, 2, ghost_rect.height), b);
                    self.framebuffer.fill_rect(DwmRect::new(ghost_rect.right() - 2, ghost_rect.y, 2, ghost_rect.height), b);
                }
            }

            // Plane 2.5: Snap Assist suggestion cards
            if self.snap_assist.active {
                for slot in &self.snap_assist.slots {
                    if slot.rect.intersects(dirty_rect) {
                        let card_rect = slot.rect.inset(16, 16);
                        self.framebuffer.fill_rect(card_rect, DwmColor::rgba(40, 40, 40, 190));
                        self.framebuffer.fill_rect(
                            DwmRect::new(card_rect.x, card_rect.y, card_rect.width, 28),
                            DwmColor::rgba(25, 25, 25, 220),
                        );
                    }
                }
            }

            // Plane 3: Hardware Cursor Plane
            if self.cursor.visible {
                let cursor_rect = DwmRect::new(self.cursor.x, self.cursor.y, 16, 16);
                if cursor_rect.intersects(dirty_rect) {
                    self.framebuffer.blit(
                        &self.cursor.sprite,
                        self.cursor.x,
                        self.cursor.y,
                        Some(*dirty_rect),
                        1.0,
                    );
                }
            }
        }

        self.dirty_tracker.clear();
        rect_count
    }
}

// ============================================================================
// Section 9: Comprehensive Unit Test Suite
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Test 1: Verifies all Snap Layout grid geometries and screen edge/corner trigger zones.
    #[test]
    fn test_snap_layouts_geometry_and_edge_detection() {
        let work_area = DwmRect::new(0, 0, 1920, 1032);
        let engine = SnapEngine::new(work_area);

        // 1. TwoColumnEqual (50 / 50)
        let rects_50_50 = engine.calculate_layout_rects(SnapLayoutType::TwoColumnEqual);
        assert_eq!(rects_50_50.len(), 2);
        assert_eq!(rects_50_50[0], DwmRect::new(0, 0, 960, 1032));
        assert_eq!(rects_50_50[1], DwmRect::new(960, 0, 960, 1032));

        // 2. TwoColumnLeftMajor (70 / 30)
        let rects_70_30 = engine.calculate_layout_rects(SnapLayoutType::TwoColumnLeftMajor);
        assert_eq!(rects_70_30.len(), 2);
        assert_eq!(rects_70_30[0].width, (1920 * 70) / 100);
        assert_eq!(rects_70_30[0].width + rects_70_30[1].width, 1920);

        // 3. ThreeColumn (25 / 50 / 25)
        let rects_three = engine.calculate_layout_rects(SnapLayoutType::ThreeColumn);
        assert_eq!(rects_three.len(), 3);
        assert_eq!(rects_three[0].width, 480);
        assert_eq!(rects_three[1].width, 960);
        assert_eq!(rects_three[2].width, 480);

        // 4. FourQuarter (2x2)
        let rects_quarters = engine.calculate_layout_rects(SnapLayoutType::FourQuarter);
        assert_eq!(rects_quarters.len(), 4);
        assert_eq!(rects_quarters[0], DwmRect::new(0, 0, 960, 516));
        assert_eq!(rects_quarters[1], DwmRect::new(960, 0, 960, 516));
        assert_eq!(rects_quarters[2], DwmRect::new(0, 516, 960, 516));
        assert_eq!(rects_quarters[3], DwmRect::new(960, 516, 960, 516));

        // 5. LeftSplitWithTwoStacked
        let rects_split = engine.calculate_layout_rects(SnapLayoutType::LeftSplitWithTwoStacked);
        assert_eq!(rects_split.len(), 3);
        assert_eq!(rects_split[0], DwmRect::new(0, 0, 960, 1032)); // full height left
        assert_eq!(rects_split[1], DwmRect::new(960, 0, 960, 516)); // top-right quarter
        assert_eq!(rects_split[2], DwmRect::new(960, 516, 960, 516)); // bottom-right quarter

        // 6. Test Edge and Corner Triggers
        // Top edge center -> Maximize
        assert_eq!(engine.detect_snap_zone(960, 5), Some(SnapZone::Maximize));
        // Left edge center -> LeftHalf
        assert_eq!(engine.detect_snap_zone(5, 500), Some(SnapZone::LeftHalf));
        // Right edge center -> RightHalf
        assert_eq!(engine.detect_snap_zone(1915, 500), Some(SnapZone::RightHalf));
        // Top-Left corner -> TopLeftQuarter
        assert_eq!(engine.detect_snap_zone(5, 5), Some(SnapZone::TopLeftQuarter));
        // Top-Right corner -> TopRightQuarter
        assert_eq!(engine.detect_snap_zone(1915, 5), Some(SnapZone::TopRightQuarter));
        // Bottom-Left corner -> BottomLeftQuarter
        assert_eq!(engine.detect_snap_zone(5, 1025), Some(SnapZone::BottomLeftQuarter));
        // Bottom-Right corner -> BottomRightQuarter
        assert_eq!(engine.detect_snap_zone(1915, 1025), Some(SnapZone::BottomRightQuarter));
        // Center screen -> No snap trigger
        assert_eq!(engine.detect_snap_zone(500, 500), None);
    }

    /// Test 2: Verifies Snap Assist suggested window picker slots and slot selection.
    #[test]
    fn test_snap_assist_picker_slots() {
        let work_area = DwmRect::new(0, 0, 1920, 1080);
        let engine = SnapEngine::new(work_area);
        let mut assist = SnapAssist::new();

        let w1 = DwmWindow::new(101, "Window 1", DwmRect::new(50, 50, 600, 400));
        let w2 = DwmWindow::new(102, "Window 2", DwmRect::new(80, 80, 600, 400));
        let w3 = DwmWindow::new(103, "Window 3", DwmRect::new(100, 100, 600, 400));
        let all_windows = vec![w1, w2, w3];

        // Snap window 101 into slot 0 of TwoColumnEqual
        assist.activate(&engine, SnapLayoutType::TwoColumnEqual, 0, 101, &all_windows);

        assert!(assist.active);
        assert_eq!(assist.slots.len(), 1);
        assert_eq!(assist.slots[0].slot_index, 1);
        assert_eq!(assist.slots[0].rect, DwmRect::new(960, 0, 960, 1080));
        assert!(assist.slots[0].candidate_windows.contains(&102));
        assert!(assist.slots[0].candidate_windows.contains(&103));
        assert!(!assist.slots[0].candidate_windows.contains(&101));

        // User picks window 103 for slot 1
        let assigned_rect = assist.pick_candidate_for_slot(1, 103);
        assert_eq!(assigned_rect, Some(DwmRect::new(960, 0, 960, 1080)));
        // Once all slots filled, Snap Assist automatically dismisses
        assert!(!assist.active);
        assert!(assist.slots.is_empty());
    }

    /// Test 3: Verifies Window Hierarchy, focus management and Z-order operations.
    #[test]
    fn test_window_hierarchy_and_z_ordering() {
        let mut comp = DwmCompositor::new(1920, 1080, 48, 60);

        let w1 = comp.create_window("Window 1", DwmRect::new(100, 100, 400, 300));
        let w2 = comp.create_window("Window 2", DwmRect::new(200, 200, 400, 300));
        let w3 = comp.create_window("Window 3", DwmRect::new(300, 300, 400, 300));

        assert_eq!(comp.z_order, vec![w1, w2, w3]);
        assert_eq!(comp.focused_window, Some(w3));

        // Bring w1 to front
        comp.bring_to_front(w1);
        assert_eq!(comp.z_order, vec![w2, w3, w1]);
        assert_eq!(comp.focused_window, Some(w1));

        // Send w1 to back
        comp.send_to_back(w1);
        assert_eq!(comp.z_order, vec![w1, w2, w3]);
        assert_eq!(comp.focused_window, Some(w3));

        // Destroy w3
        comp.destroy_window(w3);
        assert_eq!(comp.z_order, vec![w1, w2]);
        assert_eq!(comp.focused_window, Some(w2));
    }

    /// Test 4: Verifies Window Animation Physics (Spring Model & Cubic-Bezier evaluation).
    #[test]
    fn test_animation_physics_spring_and_bezier() {
        // Test Cubic-Bezier boundary and easing behavior
        let bezier = CubicBezier::STANDARD_EASE;
        assert_eq!(bezier.evaluate(0.0), 0.0);
        assert_eq!(bezier.evaluate(1.0), 1.0);
        let mid = bezier.evaluate(0.5);
        assert!(mid > 0.0 && mid < 1.0);

        // Test Spring Model
        let spring = SpringModel::gentle();
        assert_eq!(spring.evaluate(0.0), 0.0);
        // At long time, harmonic oscillator converges to equilibrium 1.0
        let settled = spring.evaluate(2.0);
        assert!((settled - 1.0).abs() < 0.02);

        // Test WindowAnimation minimize stepping
        let start = DwmRect::new(100, 100, 500, 400);
        let target_icon = DwmRect::new(200, 1040, 40, 40);
        let mut anim = WindowAnimation::new_minimize(start, target_icon);

        assert!(!anim.is_finished);
        let running = anim.step(110.0); // half duration
        assert!(running);
        assert!(anim.current_opacity < 1.0);
        assert!(anim.current_scale < 1.0);

        let final_step = anim.step(150.0); // complete duration
        assert!(!final_step);
        assert!(anim.is_finished);
        assert_eq!(anim.current_rect, target_icon);
        assert_eq!(anim.current_opacity, 0.0);
    }

    /// Test 5: Verifies Aero Shake gesture detection and window minimization/restoration toggle.
    #[test]
    fn test_aero_shake_gesture_and_toggle() {
        let mut comp = DwmCompositor::new(1920, 1080, 48, 60);

        let active = comp.create_window("Active Window", DwmRect::new(200, 200, 400, 300));
        let bg1 = comp.create_window("Background 1", DwmRect::new(50, 50, 300, 200));
        let bg2 = comp.create_window("Background 2", DwmRect::new(700, 100, 300, 200));

        comp.set_focus(active);

        // Simulate shaking active window titlebar: rapid horizontal oscillations
        comp.aero_shake.start_drag(active, 300, 210, 1000.0);
        // Stroke 1: Move right (+60px)
        comp.handle_window_drag_motion(active, 360, 210, 1050.0);
        // Stroke 2: Move left (-60px) -> Reversal 1
        comp.handle_window_drag_motion(active, 300, 210, 1100.0);
        // Stroke 3: Move right (+60px) -> Reversal 2
        comp.handle_window_drag_motion(active, 360, 210, 1150.0);
        // Stroke 4: Move left (-60px) -> Reversal 3 -> triggers Aero Shake!
        comp.handle_window_drag_motion(active, 300, 210, 1200.0);

        // Background windows should now be minimized
        assert_eq!(comp.windows.get(&bg1).unwrap().state, WindowState::Minimized);
        assert_eq!(comp.windows.get(&bg2).unwrap().state, WindowState::Minimized);
        assert_eq!(comp.windows.get(&active).unwrap().state, WindowState::Normal);

        // Second shake: toggle restores them!
        comp.aero_shake.last_shake_trigger_ms = 0.0; // reset debounce for test
        comp.aero_shake.start_drag(active, 300, 210, 2000.0);
        comp.handle_window_drag_motion(active, 360, 210, 2050.0);
        comp.handle_window_drag_motion(active, 300, 210, 2100.0);
        comp.handle_window_drag_motion(active, 360, 210, 2150.0);
        comp.handle_window_drag_motion(active, 300, 210, 2200.0);

        // Background windows should now be restored
        assert_eq!(comp.windows.get(&bg1).unwrap().state, WindowState::Normal);
        assert_eq!(comp.windows.get(&bg2).unwrap().state, WindowState::Normal);
    }

    /// Test 6: Verifies DirtyRectTracker disjoint merging, intersection, and full screen invalidation.
    #[test]
    fn test_dirty_rect_tracker_coalescing_and_ops() {
        let screen = DwmRect::new(0, 0, 1920, 1080);
        let mut tracker = DirtyRectTracker::new(screen);

        assert!(!tracker.is_dirty());

        // Add first dirty rect
        tracker.add_dirty_rect(DwmRect::new(100, 100, 200, 200));
        assert!(tracker.is_dirty());
        assert_eq!(tracker.dirty_rects().len(), 1);

        // Add overlapping rect: should merge cleanly
        tracker.add_dirty_rect(DwmRect::new(150, 150, 200, 200));
        assert_eq!(tracker.dirty_rects().len(), 1);
        assert_eq!(tracker.dirty_rects()[0], DwmRect::new(100, 100, 250, 250));

        // Add separated non-overlapping rect
        tracker.add_dirty_rect(DwmRect::new(800, 800, 100, 100));
        assert_eq!(tracker.dirty_rects().len(), 2);

        // Coalesce
        tracker.coalesce();
        assert_eq!(tracker.dirty_rects().len(), 1);
        assert_eq!(tracker.dirty_rects()[0], DwmRect::new(100, 100, 800, 800));

        // Invalidate all
        tracker.invalidate_all();
        assert_eq!(tracker.dirty_rects().len(), 1);
        assert_eq!(tracker.dirty_rects()[0], screen);

        // Clear
        tracker.clear();
        assert!(!tracker.is_dirty());
    }

    /// Test 7: Verifies VSync Tick Generator for 60 FPS and 120 FPS modes and frame timing telemetry.
    #[test]
    fn test_vsync_frame_timing_60_and_120_fps() {
        // 60 FPS mode: ~16.666 ms per tick
        let mut vsync_60 = VSyncTickGenerator::new(60);
        assert_eq!(vsync_60.target_fps, 60);
        assert!((vsync_60.target_interval_ms - 16.666).abs() < 0.01);

        // First tick
        let t1 = vsync_60.advance_time(1000.0);
        assert!(t1.is_some());
        assert_eq!(t1.unwrap().frame_index, 1);

        // Too early tick (only 5ms elapsed)
        let too_early = vsync_60.advance_time(1005.0);
        assert!(too_early.is_none());

        // Valid tick (17ms elapsed)
        let t2 = vsync_60.advance_time(1017.0);
        assert!(t2.is_some());
        assert_eq!(t2.unwrap().frame_index, 2);

        // 120 FPS mode: ~8.333 ms per tick
        let mut vsync_120 = VSyncTickGenerator::new(120);
        assert_eq!(vsync_120.target_fps, 120);
        assert!((vsync_120.target_interval_ms - 8.333).abs() < 0.01);

        let _ = vsync_120.advance_time(1000.0);
        // Simulate a lag spike (40ms gap) -> dropped frames detected
        let lagged = vsync_120.advance_time(1040.0);
        assert!(lagged.is_some());
        assert!(vsync_120.dropped_frames >= 3);
    }

    /// Test 8: Verifies Direct Composition Pass combining background, window surfaces, and cursor.
    #[test]
    fn test_direct_composition_pass() {
        let mut comp = DwmCompositor::new(640, 480, 40, 60);

        let w_id = comp.create_window("Test Window", DwmRect::new(50, 50, 200, 150));
        if let Some(w) = comp.windows.get_mut(&w_id) {
            let mut surf = DwmSurface::new(200, 150, DwmColor::rgb(255, 0, 0));
            surf.fill_rect(DwmRect::new(10, 10, 50, 50), DwmColor::rgb(0, 255, 0));
            w.surface = Some(surf);
        }

        comp.dirty_tracker.invalidate_all();
        let rects_composited = comp.compose_frame();
        assert!(rects_composited > 0);

        // Verify pixel sampled in window red body (outside green subrect 10..60, 10..60)
        let pixel_inside_window = comp.framebuffer.get_pixel(150, 120);
        assert_eq!(pixel_inside_window.r, 255);
        assert_eq!(pixel_inside_window.g, 0);
        assert_eq!(pixel_inside_window.b, 0);

        // Verify pixel inside green subrect (src_x = 70-50 = 20, src_y = 70-50 = 20)
        let pixel_subrect = comp.framebuffer.get_pixel(70, 70);
        assert_eq!(pixel_subrect.r, 0);
        assert_eq!(pixel_subrect.g, 255);
        assert_eq!(pixel_subrect.b, 0);

        // Verify pixel outside window sampled wallpaper gradient
        let pixel_outside = comp.framebuffer.get_pixel(500, 400);
        assert_eq!(pixel_outside, comp.wallpaper.get_pixel(500, 400));
    }
}
