//! Windows 11 Fluent Design System & Materials Engine
//!
//! Production-grade implementation of Microsoft Fluent Design System specifications for HybridOS:
//! 1. Mica Material (Mica Base & Mica Alt) with desktop wallpaper sampling, luminosity, color tint,
//!    opacity blending, and automatic fallback for low-power / battery saver modes.
//! 2. Acrylic Material (Background & In-App Acrylic) with dual-pass Gaussian blur simulation,
//!    subtle procedural noise texture overlay, and exclusion / luminosity blending.
//! 3. Rounded Corner Engine with `CornerRadii` (TopLeft, TopRight, BottomRight, BottomLeft: 8px default outer,
//!    4px inner, pill radius for taskbar/search, and subpixel anti-aliased SDF coverage).
//! 4. Drop Shadow Engine with Elevation levels (2, 4, 8, 16, 32, 64), ambient + directional dual shadow
//!    layers, blur radius, spread, opacity, and 3D light source vector calculations.
//! 5. Accent Color Palette Generator generating the 7-shade Fluent Accent Ramp (Lightest, Lighter, Light,
//!    Normal, Dark, Darker, Darkest) and verifying WCAG 2.1 4.5:1 contrast against light and dark backgrounds.
//! 6. Segoe UI Variable Typography Metrics (Display, TitleLarge, Title, Subtitle, BodyLarge, Body,
//!    Caption, BodyStrong, CaptionStrong) with pixel size, font weight, line height, and character spacing.
//! 7. Comprehensive unit test suite validating all materials, shaders, shadows, colors, and typography metrics.

#![allow(dead_code)]

#[allow(unused_imports)]
use alloc::string::{String, ToString};
#[allow(unused_imports)]
use alloc::vec::Vec;
#[allow(unused_imports)]
use alloc::boxed::Box;
#[allow(unused_imports)]
use crate::compat::FloatExt;
use std::fmt;

// ============================================================================
// 1. Color, Linear Math & Blend Mode Primitives
// ============================================================================

/// 32-bit RGBA Color in sRGB color space with high-precision arithmetic support.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const TRANSPARENT: Self = Self { r: 0, g: 0, b: 0, a: 0 };
    pub const BLACK: Self = Self { r: 0, g: 0, b: 0, a: 255 };
    pub const WHITE: Self = Self { r: 255, g: 255, b: 255, a: 255 };

    /// Windows 11 Default Accent Blue: #0078D4
    pub const WIN11_BLUE: Self = Self { r: 0, g: 120, b: 212, a: 255 };
    /// Windows 11 Mica Light Background Tint: #F3F3F3
    pub const MICA_LIGHT_TINT: Self = Self { r: 243, g: 243, b: 243, a: 255 };
    /// Windows 11 Mica Dark Background Tint: #202020
    pub const MICA_DARK_TINT: Self = Self { r: 32, g: 32, b: 32, a: 255 };

    #[inline]
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    #[inline]
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    /// Construct from 24-bit 0xRRGGBB integer.
    #[inline]
    pub const fn from_rgb_u32(val: u32) -> Self {
        Self {
            r: ((val >> 16) & 0xFF) as u8,
            g: ((val >> 8) & 0xFF) as u8,
            b: (val & 0xFF) as u8,
            a: 255,
        }
    }

    /// Construct from 32-bit 0xAARRGGBB integer.
    #[inline]
    pub const fn from_argb_u32(val: u32) -> Self {
        Self {
            a: ((val >> 24) & 0xFF) as u8,
            r: ((val >> 16) & 0xFF) as u8,
            g: ((val >> 8) & 0xFF) as u8,
            b: (val & 0xFF) as u8,
        }
    }

    /// Pack into 32-bit 0xAARRGGBB format.
    #[inline]
    pub const fn to_argb_u32(&self) -> u32 {
        ((self.a as u32) << 24) | ((self.r as u32) << 16) | ((self.g as u32) << 8) | (self.b as u32)
    }

    /// Pack into 32-bit 0xRRGGBBAA format.
    #[inline]
    pub const fn to_rgba_u32(&self) -> u32 {
        ((self.r as u32) << 24) | ((self.g as u32) << 16) | ((self.b as u32) << 8) | (self.a as u32)
    }

    /// Returns color with a different alpha (0.0 to 1.0).
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

    /// Returns color with a different alpha byte.
    #[inline]
    pub const fn with_alpha(&self, a: u8) -> Self {
        Self {
            r: self.r,
            g: self.g,
            b: self.b,
            a,
        }
    }

    /// Linear interpolation between `self` and `target` by factor `t` (0.0..=1.0).
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

    /// Convert sRGB 8-bit channel to Linear RGB float (gamma expansion).
    #[inline]
    pub fn channel_to_linear(c: u8) -> f32 {
        let v = c as f32 / 255.0;
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    }

    /// Convert Linear RGB float to sRGB 8-bit channel (gamma compression).
    #[inline]
    pub fn linear_to_channel(v: f32) -> u8 {
        let clamped = v.clamp(0.0, 1.0);
        let srgb = if clamped <= 0.0031308 {
            clamped * 12.92
        } else {
            1.055 * clamped.powf(1.0 / 2.4) - 0.055
        };
        (srgb * 255.0).round() as u8
    }

    /// Calculate WCAG 2.1 relative luminance in linear space.
    /// Formula: Y = 0.2126 * R_lin + 0.7152 * G_lin + 0.0722 * B_lin
    pub fn relative_luminance(&self) -> f32 {
        let r_lin = Self::channel_to_linear(self.r);
        let g_lin = Self::channel_to_linear(self.g);
        let b_lin = Self::channel_to_linear(self.b);
        0.2126 * r_lin + 0.7152 * g_lin + 0.0722 * b_lin
    }

    /// Calculate WCAG 2.1 contrast ratio against another color: (L1 + 0.05) / (L2 + 0.05).
    pub fn contrast_ratio(&self, other: &Self) -> f32 {
        let l1 = self.relative_luminance();
        let l2 = other.relative_luminance();
        let (lighter, darker) = if l1 >= l2 { (l1, l2) } else { (l2, l1) };
        (lighter + 0.05) / (darker + 0.05)
    }

    /// Convert RGB to HSL (Hue: 0..360, Saturation: 0..1, Lightness: 0..1).
    pub fn to_hsl(&self) -> (f32, f32, f32) {
        let r = self.r as f32 / 255.0;
        let g = self.g as f32 / 255.0;
        let b = self.b as f32 / 255.0;

        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let delta = max - min;

        let l = (max + min) / 2.0;

        if delta.abs() < 1e-5 {
            return (0.0, 0.0, l);
        }

        let s = if l > 0.5 {
            delta / (2.0 - max - min)
        } else {
            delta / (max + min)
        };

        let mut h = if (max - r).abs() < 1e-5 {
            (g - b) / delta + (if g < b { 6.0 } else { 0.0 })
        } else if (max - g).abs() < 1e-5 {
            (b - r) / delta + 2.0
        } else {
            (r - g) / delta + 4.0
        };
        h *= 60.0;

        (h, s, l)
    }

    /// Construct Color from HSL (Hue: 0..360, Saturation: 0..1, Lightness: 0..1).
    pub fn from_hsl(h: f32, s: f32, l: f32) -> Self {
        let s = s.clamp(0.0, 1.0);
        let l = l.clamp(0.0, 1.0);
        let h = ((h % 360.0) + 360.0) % 360.0;

        if s.abs() < 1e-5 {
            let gray = (l * 255.0).round() as u8;
            return Self::rgb(gray, gray, gray);
        }

        let q = if l < 0.5 {
            l * (1.0 + s)
        } else {
            l + s - l * s
        };
        let p = 2.0 * l - q;
        let hk = h / 360.0;

        let tr = (hk + 1.0 / 3.0).fract();
        let tg = hk;
        let tb = (hk - 1.0 / 3.0 + 1.0).fract();

        let conv = |tc: f32| -> f32 {
            if tc < 1.0 / 6.0 {
                p + ((q - p) * 6.0 * tc)
            } else if tc < 1.0 / 2.0 {
                q
            } else if tc < 2.0 / 3.0 {
                p + ((q - p) * (2.0 / 3.0 - tc) * 6.0)
            } else {
                p
            }
        };

        Self::rgb(
            (conv(tr) * 255.0).round() as u8,
            (conv(tg) * 255.0).round() as u8,
            (conv(tb) * 255.0).round() as u8,
        )
    }

    /// Standard Porter-Duff Alpha Compositing: `src OVER dst`.
    pub fn blend_over(dst: Self, src: Self) -> Self {
        let src_a = src.a as f32 / 255.0;
        let dst_a = dst.a as f32 / 255.0;
        let out_a = src_a + dst_a * (1.0 - src_a);

        if out_a <= 0.0 {
            return Self::TRANSPARENT;
        }

        let out_r = (src.r as f32 * src_a + dst.r as f32 * dst_a * (1.0 - src_a)) / out_a;
        let out_g = (src.g as f32 * src_a + dst.g as f32 * dst_a * (1.0 - src_a)) / out_a;
        let out_b = (src.b as f32 * src_a + dst.b as f32 * dst_a * (1.0 - src_a)) / out_a;

        Self {
            r: out_r.round() as u8,
            g: out_g.round() as u8,
            b: out_b.round() as u8,
            a: (out_a * 255.0).round() as u8,
        }
    }

    /// Fluent Exclusion Blending Mode: `B(Cb, Cs) = Cb + Cs - 2 * Cb * Cs`.
    pub fn blend_exclusion(backdrop: Self, source: Self) -> Self {
        let b_r = backdrop.r as f32 / 255.0;
        let b_g = backdrop.g as f32 / 255.0;
        let b_b = backdrop.b as f32 / 255.0;
        let s_r = source.r as f32 / 255.0;
        let s_g = source.g as f32 / 255.0;
        let s_b = source.b as f32 / 255.0;
        let s_a = source.a as f32 / 255.0;

        let excl_r = b_r + s_r - 2.0 * b_r * s_r;
        let excl_g = b_g + s_g - 2.0 * b_g * s_g;
        let excl_b = b_b + s_b - 2.0 * b_b * s_b;

        // Interpolate backdrop with exclusion result according to source alpha
        let out_r = b_r * (1.0 - s_a) + excl_r * s_a;
        let out_g = b_g * (1.0 - s_a) + excl_g * s_a;
        let out_b = b_b * (1.0 - s_a) + excl_b * s_a;

        Self {
            r: (out_r.clamp(0.0, 1.0) * 255.0).round() as u8,
            g: (out_g.clamp(0.0, 1.0) * 255.0).round() as u8,
            b: (out_b.clamp(0.0, 1.0) * 255.0).round() as u8,
            a: backdrop.a,
        }
    }

    /// Fluent Luminosity Blending Mode: takes Hue and Saturation of backdrop and Lightness of source.
    pub fn blend_luminosity(backdrop: Self, source: Self) -> Self {
        let (h, s, _) = backdrop.to_hsl();
        let (_, _, l) = source.to_hsl();
        let lum_color = Self::from_hsl(h, s, l);
        let src_a = source.a as f32 / 255.0;
        backdrop.lerp(lum_color, src_a)
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{:02X}{:02X}{:02X}{:02X}", self.r, self.g, self.b, self.a)
    }
}

/// 2D Surface / Pixel Buffer for Fluent rendering, blur simulation, and compositing.
#[derive(Debug, Clone)]
pub struct Surface {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u32>, // 0xAARRGGBB
}

impl Surface {
    pub fn new(width: usize, height: usize, clear_color: Color) -> Self {
        let packed = clear_color.to_argb_u32();
        Self {
            width,
            height,
            pixels: vec![packed; width * height],
        }
    }

    #[inline]
    pub fn get_pixel(&self, x: usize, y: usize) -> Color {
        if x < self.width && y < self.height {
            Color::from_argb_u32(self.pixels[y * self.width + x])
        } else {
            Color::TRANSPARENT
        }
    }

    #[inline]
    pub fn set_pixel(&mut self, x: usize, y: usize, color: Color) {
        if x < self.width && y < self.height {
            self.pixels[y * self.width + x] = color.to_argb_u32();
        }
    }

    /// Perform a 2-pass separable 1D Gaussian blur on this surface buffer.
    pub fn apply_gaussian_blur(&mut self, radius: f32) {
        if radius < 0.5 || self.width == 0 || self.height == 0 {
            return;
        }

        let sigma = (radius / 2.0).max(0.5);
        let kernel_radius = (radius.ceil() as usize).clamp(1, 40);
        let kernel_size = kernel_radius * 2 + 1;

        // Compute normalized 1D Gaussian weights
        let mut kernel = Vec::with_capacity(kernel_size);
        let mut sum = 0.0f32;
        let two_sigma_sq = 2.0 * sigma * sigma;

        for i in 0..kernel_size {
            let x = i as f32 - kernel_radius as f32;
            let w = (- (x * x) / two_sigma_sq).exp();
            kernel.push(w);
            sum += w;
        }
        for w in kernel.iter_mut() {
            *w /= sum;
        }

        // Pass 1: Horizontal blur into intermediate buffer
        let mut temp = vec![0u32; self.width * self.height];
        for y in 0..self.height {
            for x in 0..self.width {
                let mut acc_r = 0.0f32;
                let mut acc_g = 0.0f32;
                let mut acc_b = 0.0f32;
                let mut acc_a = 0.0f32;

                for k in 0..kernel_size {
                    let offset = k as isize - kernel_radius as isize;
                    let sample_x = (x as isize + offset).clamp(0, (self.width - 1) as isize) as usize;
                    let color = Color::from_argb_u32(self.pixels[y * self.width + sample_x]);
                    let w = kernel[k];

                    acc_r += color.r as f32 * w;
                    acc_g += color.g as f32 * w;
                    acc_b += color.b as f32 * w;
                    acc_a += color.a as f32 * w;
                }

                let blurred = Color {
                    r: acc_r.round() as u8,
                    g: acc_g.round() as u8,
                    b: acc_b.round() as u8,
                    a: acc_a.round() as u8,
                };
                temp[y * self.width + x] = blurred.to_argb_u32();
            }
        }

        // Pass 2: Vertical blur from intermediate buffer into self
        for x in 0..self.width {
            for y in 0..self.height {
                let mut acc_r = 0.0f32;
                let mut acc_g = 0.0f32;
                let mut acc_b = 0.0f32;
                let mut acc_a = 0.0f32;

                for k in 0..kernel_size {
                    let offset = k as isize - kernel_radius as isize;
                    let sample_y = (y as isize + offset).clamp(0, (self.height - 1) as isize) as usize;
                    let color = Color::from_argb_u32(temp[sample_y * self.width + x]);
                    let w = kernel[k];

                    acc_r += color.r as f32 * w;
                    acc_g += color.g as f32 * w;
                    acc_b += color.b as f32 * w;
                    acc_a += color.a as f32 * w;
                }

                let blurred = Color {
                    r: acc_r.round() as u8,
                    g: acc_g.round() as u8,
                    b: acc_b.round() as u8,
                    a: acc_a.round() as u8,
                };
                self.pixels[y * self.width + x] = blurred.to_argb_u32();
            }
        }
    }
}

// ============================================================================
// 2. Windows 11 Theme & System Power Modes
// ============================================================================

/// Windows 11 Fluent Theme Mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeMode {
    Light,
    Dark,
}

/// System Power State affecting material rendering (e.g. Mica & Acrylic fallbacks)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerMode {
    Normal,
    HighPerformance,
    BatterySaver,
    LowPower,
}

impl PowerMode {
    #[inline]
    pub fn should_disable_transparency(&self) -> bool {
        matches!(self, PowerMode::BatterySaver | PowerMode::LowPower)
    }
}

// ============================================================================
// 3. Mica Material (Mica Base & Mica Alt)
// ============================================================================

/// Variants of the Windows 11 Mica material
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MicaKind {
    /// Mica Base: Used for window backgrounds (e.g., Settings, File Explorer background).
    Base,
    /// Mica Alt: Higher contrast variant used for tab strips, title bars, and hierarchical chrome.
    Alt,
}

/// Configuration settings for Mica Material
#[derive(Debug, Clone, PartialEq)]
pub struct MicaConfig {
    pub kind: MicaKind,
    pub theme: ThemeMode,
    pub tint_color: Color,
    pub tint_opacity: f32,
    pub luminosity_opacity: f32,
    pub blur_radius: f32,
    pub fallback_color: Color,
    pub low_power_fallback: Color,
}

impl MicaConfig {
    /// Default Windows 11 Mica Base configuration for a given theme
    pub fn default_base(theme: ThemeMode) -> Self {
        match theme {
            ThemeMode::Light => Self {
                kind: MicaKind::Base,
                theme: ThemeMode::Light,
                tint_color: Color::rgb(243, 243, 243),
                tint_opacity: 0.50,
                luminosity_opacity: 1.0,
                blur_radius: 60.0,
                fallback_color: Color::rgb(243, 243, 243),
                low_power_fallback: Color::rgb(238, 238, 238),
            },
            ThemeMode::Dark => Self {
                kind: MicaKind::Base,
                theme: ThemeMode::Dark,
                tint_color: Color::rgb(32, 32, 32),
                tint_opacity: 0.80,
                luminosity_opacity: 1.0,
                blur_radius: 60.0,
                fallback_color: Color::rgb(32, 32, 32),
                low_power_fallback: Color::rgb(24, 24, 24),
            },
        }
    }

    /// Default Windows 11 Mica Alt configuration for tab strips, title bars, etc.
    pub fn default_alt(theme: ThemeMode) -> Self {
        match theme {
            ThemeMode::Light => Self {
                kind: MicaKind::Alt,
                theme: ThemeMode::Light,
                tint_color: Color::rgb(235, 235, 235),
                tint_opacity: 0.80,
                luminosity_opacity: 1.0,
                blur_radius: 60.0,
                fallback_color: Color::rgb(235, 235, 235),
                low_power_fallback: Color::rgb(230, 230, 230),
            },
            ThemeMode::Dark => Self {
                kind: MicaKind::Alt,
                theme: ThemeMode::Dark,
                tint_color: Color::rgb(24, 24, 24),
                tint_opacity: 0.90,
                luminosity_opacity: 1.0,
                blur_radius: 60.0,
                fallback_color: Color::rgb(24, 24, 24),
                low_power_fallback: Color::rgb(18, 18, 18),
            },
        }
    }
}

/// Production-grade Windows 11 Mica Material Engine
#[derive(Debug, Clone)]
pub struct MicaMaterial {
    pub config: MicaConfig,
}

impl MicaMaterial {
    pub fn new(config: MicaConfig) -> Self {
        Self { config }
    }

    pub fn base(theme: ThemeMode) -> Self {
        Self::new(MicaConfig::default_base(theme))
    }

    pub fn alt(theme: ThemeMode) -> Self {
        Self::new(MicaConfig::default_alt(theme))
    }

    /// Compute the effective Mica output color given desktop wallpaper sample, window activity, and power mode.
    ///
    /// Fallback conditions:
    /// - Battery Saver or Low Power active: returns low_power_fallback.
    /// - Transparency disabled: returns fallback_color.
    /// - Window inactive: returns slightly desaturated fallback.
    pub fn resolve_color(
        &self,
        wallpaper_sample: Color,
        window_active: bool,
        transparency_enabled: bool,
        power_mode: PowerMode,
    ) -> Color {
        // Fallback 1: Battery Saver or Low Power
        if power_mode.should_disable_transparency() {
            return self.config.low_power_fallback;
        }

        // Fallback 2: User or System disabled transparency effects
        if !transparency_enabled {
            return self.config.fallback_color;
        }

        // Fallback 3: Inactive window state (Mica becomes neutral solid tint)
        if !window_active {
            return self.config.fallback_color;
        }

        // Step 1: Luminosity layer (applies luminance of tint onto wallpaper sample)
        let lum_blended = Color::blend_luminosity(
            wallpaper_sample,
            self.config.tint_color.with_alpha_f32(self.config.luminosity_opacity),
        );

        // Step 2: Color tint overlay
        Color::blend_over(
            lum_blended,
            self.config.tint_color.with_alpha_f32(self.config.tint_opacity),
        )
    }

    /// Render Mica material over an entire surface buffer using a desktop wallpaper surface.
    pub fn render_to_surface(
        &self,
        target: &mut Surface,
        wallpaper: &Surface,
        window_x: usize,
        window_y: usize,
        window_active: bool,
        transparency_enabled: bool,
        power_mode: PowerMode,
    ) {
        if power_mode.should_disable_transparency() || !transparency_enabled || !window_active {
            let solid = self.resolve_color(Color::BLACK, window_active, transparency_enabled, power_mode);
            for p in target.pixels.iter_mut() {
                *p = solid.to_argb_u32();
            }
            return;
        }

        // Sample wallpaper with Gaussian blur
        let mut sample_surface = Surface::new(target.width, target.height, Color::BLACK);
        for y in 0..target.height {
            for x in 0..target.width {
                let wp_x = (window_x + x) % wallpaper.width.max(1);
                let wp_y = (window_y + y) % wallpaper.height.max(1);
                sample_surface.set_pixel(x, y, wallpaper.get_pixel(wp_x, wp_y));
            }
        }
        sample_surface.apply_gaussian_blur(self.config.blur_radius);

        for y in 0..target.height {
            for x in 0..target.width {
                let sample = sample_surface.get_pixel(x, y);
                let resolved = self.resolve_color(sample, window_active, transparency_enabled, power_mode);
                target.set_pixel(x, y, resolved);
            }
        }
    }
}

// ============================================================================
// 4. Acrylic Material (Background & In-App Acrylic)
// ============================================================================

/// Variants of the Windows 11 Acrylic material
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcrylicKind {
    /// Background Acrylic: Samples content behind the app window (e.g., Start Menu, flyouts, taskbar).
    Background,
    /// In-App Acrylic: Samples content within the app surface behind the acrylic element (e.g., navigation view sidebar).
    InApp,
}

/// Configuration settings for Acrylic Material
#[derive(Debug, Clone, PartialEq)]
pub struct AcrylicConfig {
    pub kind: AcrylicKind,
    pub theme: ThemeMode,
    pub tint_color: Color,
    pub tint_opacity: f32,
    pub exclusion_color: Color,
    pub exclusion_opacity: f32,
    pub blur_radius: f32,
    pub noise_opacity: f32,
    pub fallback_color: Color,
}

impl AcrylicConfig {
    /// Default Windows 11 Background Acrylic configuration
    pub fn default_background(theme: ThemeMode) -> Self {
        match theme {
            ThemeMode::Light => Self {
                kind: AcrylicKind::Background,
                theme: ThemeMode::Light,
                tint_color: Color::rgb(255, 255, 255),
                tint_opacity: 0.70,
                exclusion_color: Color::rgb(250, 250, 250),
                exclusion_opacity: 0.85,
                blur_radius: 30.0,
                noise_opacity: 0.02,
                fallback_color: Color::rgb(240, 240, 240),
            },
            ThemeMode::Dark => Self {
                kind: AcrylicKind::Background,
                theme: ThemeMode::Dark,
                tint_color: Color::rgb(44, 44, 44),
                tint_opacity: 0.80,
                exclusion_color: Color::rgb(26, 26, 26),
                exclusion_opacity: 0.85,
                blur_radius: 30.0,
                noise_opacity: 0.03,
                fallback_color: Color::rgb(32, 32, 32),
            },
        }
    }

    /// Default Windows 11 In-App Acrylic configuration
    pub fn default_in_app(theme: ThemeMode) -> Self {
        match theme {
            ThemeMode::Light => Self {
                kind: AcrylicKind::InApp,
                theme: ThemeMode::Light,
                tint_color: Color::rgb(243, 243, 243),
                tint_opacity: 0.85,
                exclusion_color: Color::rgb(255, 255, 255),
                exclusion_opacity: 0.90,
                blur_radius: 20.0,
                noise_opacity: 0.02,
                fallback_color: Color::rgb(245, 245, 245),
            },
            ThemeMode::Dark => Self {
                kind: AcrylicKind::InApp,
                theme: ThemeMode::Dark,
                tint_color: Color::rgb(36, 36, 36),
                tint_opacity: 0.88,
                exclusion_color: Color::rgb(18, 18, 18),
                exclusion_opacity: 0.90,
                blur_radius: 20.0,
                noise_opacity: 0.03,
                fallback_color: Color::rgb(30, 30, 30),
            },
        }
    }
}

/// Windows 11 Fluent Acrylic Material Engine
#[derive(Debug, Clone)]
pub struct AcrylicMaterial {
    pub config: AcrylicConfig,
}

impl AcrylicMaterial {
    pub fn new(config: AcrylicConfig) -> Self {
        Self { config }
    }

    pub fn background(theme: ThemeMode) -> Self {
        Self::new(AcrylicConfig::default_background(theme))
    }

    pub fn in_app(theme: ThemeMode) -> Self {
        Self::new(AcrylicConfig::default_in_app(theme))
    }

    /// Fast, deterministic 2D micro-noise grain hash (simulates subtle Windows 11 Acrylic texture to prevent banding).
    #[inline]
    pub fn pseudo_noise(x: usize, y: usize, seed: u32) -> f32 {
        let mut n = (x as u32).wrapping_mul(374761393)
            ^ (y as u32).wrapping_mul(668265263)
            ^ seed.wrapping_mul(374761393);
        n = (n ^ (n >> 13)).wrapping_mul(1274126177);
        ((n ^ (n >> 16)) & 0xFFFF) as f32 / 65535.0
    }

    /// Render Acrylic effect given a backdrop surface.
    ///
    /// Pipeline:
    /// 1. Dual-pass Gaussian blur on backdrop.
    /// 2. Exclusion blend layer with exclusion color.
    /// 3. Color tint blend layer.
    /// 4. Noise texture overlay.
    pub fn render(
        &self,
        backdrop: &Surface,
        transparency_enabled: bool,
        power_mode: PowerMode,
        noise_seed: u32,
    ) -> Surface {
        // Fallback for low power or transparency disabled
        if power_mode.should_disable_transparency() || !transparency_enabled {
            return Surface::new(backdrop.width, backdrop.height, self.config.fallback_color);
        }

        // Step 1: Clone backdrop and apply dual-pass Gaussian blur
        let mut blurred = backdrop.clone();
        blurred.apply_gaussian_blur(self.config.blur_radius);

        let mut output = Surface::new(backdrop.width, backdrop.height, Color::BLACK);

        let exclusion_source = self.config.exclusion_color.with_alpha_f32(self.config.exclusion_opacity);
        let tint_source = self.config.tint_color.with_alpha_f32(self.config.tint_opacity);

        for y in 0..backdrop.height {
            for x in 0..backdrop.width {
                let blurred_pixel = blurred.get_pixel(x, y);

                // Step 2: Exclusion blending
                let excluded = Color::blend_exclusion(blurred_pixel, exclusion_source);

                // Step 3: Color tint blending (source OVER)
                let tinted = Color::blend_over(excluded, tint_source);

                // Step 4: Noise texture grain overlay
                let noise_val = Self::pseudo_noise(x, y, noise_seed);
                let noise_delta = (noise_val - 0.5) * self.config.noise_opacity * 255.0;

                let final_r = (tinted.r as f32 + noise_delta).clamp(0.0, 255.0).round() as u8;
                let final_g = (tinted.g as f32 + noise_delta).clamp(0.0, 255.0).round() as u8;
                let final_b = (tinted.b as f32 + noise_delta).clamp(0.0, 255.0).round() as u8;

                output.set_pixel(x, y, Color::rgba(final_r, final_g, final_b, tinted.a));
            }
        }

        output
    }
}

// ============================================================================
// 5. Rounded Corner Engine
// ============================================================================

/// Rounded Corner Radii for Fluent UI elements (Windows 11 geometry specification)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CornerRadii {
    pub top_left: f32,
    pub top_right: f32,
    pub bottom_right: f32,
    pub bottom_left: f32,
}

impl CornerRadii {
    /// Standard Windows 11 Outer Corner Radius: 8.0px (top-level windows, cards, dialogs, flyouts).
    pub const DEFAULT_OUTER_RADIUS: f32 = 8.0;

    /// Standard Windows 11 Inner Corner Radius: 4.0px (nested controls: buttons, text boxes, items).
    pub const DEFAULT_INNER_RADIUS: f32 = 4.0;

    #[inline]
    pub const fn new(top_left: f32, top_right: f32, bottom_right: f32, bottom_left: f32) -> Self {
        Self {
            top_left,
            top_right,
            bottom_right,
            bottom_left,
        }
    }

    /// Uniform corner radii on all 4 corners
    #[inline]
    pub const fn uniform(radius: f32) -> Self {
        Self::new(radius, radius, radius, radius)
    }

    /// Windows 11 default outer corner radius (8px)
    #[inline]
    pub const fn default_outer() -> Self {
        Self::uniform(Self::DEFAULT_OUTER_RADIUS)
    }

    /// Windows 11 default inner corner radius (4px)
    #[inline]
    pub const fn default_inner() -> Self {
        Self::uniform(Self::DEFAULT_INNER_RADIUS)
    }

    /// Zero / square corners (e.g. snapped or maximized windows)
    #[inline]
    pub const fn zero() -> Self {
        Self::uniform(0.0)
    }

    /// Windows 11 Pill / Capsule radius for search bars, taskbar search pill, badges, toggle switches.
    /// Equivalent to height / 2.0.
    #[inline]
    pub fn pill(height: f32) -> Self {
        Self::uniform(height / 2.0)
    }

    /// Tab item corner radius (rounded top corners, square bottom corners).
    #[inline]
    pub const fn tab(radius: f32) -> Self {
        Self::new(radius, radius, 0.0, 0.0)
    }

    /// Calculate exact Signed Distance Field (SDF) from point (x, y) to the rounded rectangle boundary.
    /// Negative values are INSIDE the rounded rectangle, 0.0 is on the border, positive values are OUTSIDE.
    pub fn signed_distance(&self, x: f32, y: f32, width: f32, height: f32) -> f32 {
        let half_w = width / 2.0;
        let half_h = height / 2.0;

        // Determine which corner quadrant the point is in and select its corresponding radius
        let r = if x < half_w {
            if y < half_h {
                self.top_left
            } else {
                self.bottom_left
            }
        } else {
            if y < half_h {
                self.top_right
            } else {
                self.bottom_right
            }
        };

        // Clamp radius to half dimensions so corners don't overlap
        let max_r = half_w.min(half_h);
        let r = r.clamp(0.0, max_r);

        // Standard 2D Rounded Box SDF
        let qx = (x - half_w).abs() - (half_w - r);
        let qy = (y - half_h).abs() - (half_h - r);

        let outside_dist = qx.max(0.0).hypot(qy.max(0.0));
        let inside_dist = qx.max(qy).min(0.0);

        outside_dist + inside_dist - r
    }

    /// Subpixel anti-aliased coverage at point (x, y) on a rectangle of given dimensions.
    /// Returns 0.0 (fully transparent) to 1.0 (fully opaque).
    pub fn coverage_at(&self, x: f32, y: f32, width: f32, height: f32) -> f32 {
        let dist = self.signed_distance(x, y, width, height);
        (0.5 - dist).clamp(0.0, 1.0)
    }

    /// Point containment test (true if strictly inside or on boundary).
    #[inline]
    pub fn contains_point(&self, x: f32, y: f32, width: f32, height: f32) -> bool {
        self.signed_distance(x, y, width, height) <= 0.0
    }
}

// ============================================================================
// 6. Drop Shadow Engine
// ============================================================================

/// Windows 11 Fluent Elevation Levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Elevation {
    /// Elevation 0: Flat / no shadow (canvas, background panels)
    Level0 = 0,
    /// Elevation 2: Resting controls (buttons, switches, inputs)
    Level2 = 2,
    /// Elevation 4: Hovered controls, cards, small popups, tooltips
    Level4 = 4,
    /// Elevation 8: Command bars, menus, context menus, flyouts
    Level8 = 8,
    /// Elevation 16: Dialogs, sub-windows, auto-suggest popups
    Level16 = 16,
    /// Elevation 32: Modal dialogs, taskbar jump lists, start menu
    Level32 = 32,
    /// Elevation 64: Top-level application windows, main active window
    Level64 = 64,
}

/// 3D Light Source vector for directional shadow calculations
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LightSource {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl LightSource {
    /// Default Windows 11 light source position (overhead, slightly in front, casting subtle downward shadow)
    pub const DEFAULT_OVERHEAD: Self = Self {
        x: 0.0,
        y: -1.0,
        z: 2.0,
    };
}

/// Single shadow layer configuration (Ambient or Directional)
#[derive(Debug, Clone, PartialEq)]
pub struct ShadowLayer {
    pub offset_x: f32,
    pub offset_y: f32,
    pub blur_radius: f32,
    pub spread: f32,
    pub color: Color,
    pub opacity: f32,
}

impl ShadowLayer {
    #[inline]
    pub fn effective_color(&self) -> Color {
        self.color.with_alpha_f32(self.opacity)
    }
}

/// Dual-shadow configuration representing both Ambient and Directional shadow layers
#[derive(Debug, Clone, PartialEq)]
pub struct FluentShadow {
    pub elevation: Elevation,
    pub ambient: ShadowLayer,
    pub directional: ShadowLayer,
}

impl FluentShadow {
    /// Calculate expanded bounding box margins (left, top, right, bottom) needed to render both shadows without clipping.
    pub fn shadow_margins(&self) -> (f32, f32, f32, f32) {
        let amb_extent = self.ambient.blur_radius + self.ambient.spread;
        let amb_left = (amb_extent - self.ambient.offset_x).max(0.0);
        let amb_right = (amb_extent + self.ambient.offset_x).max(0.0);
        let amb_top = (amb_extent - self.ambient.offset_y).max(0.0);
        let amb_bottom = (amb_extent + self.ambient.offset_y).max(0.0);

        let dir_extent = self.directional.blur_radius + self.directional.spread;
        let dir_left = (dir_extent - self.directional.offset_x).max(0.0);
        let dir_right = (dir_extent + self.directional.offset_x).max(0.0);
        let dir_top = (dir_extent - self.directional.offset_y).max(0.0);
        let dir_bottom = (dir_extent + self.directional.offset_y).max(0.0);

        (
            amb_left.max(dir_left),
            amb_top.max(dir_top),
            amb_right.max(dir_right),
            amb_bottom.max(dir_bottom),
        )
    }
}

/// Windows 11 Drop Shadow Engine
#[derive(Debug, Clone)]
pub struct DropShadowEngine {
    pub light_source: LightSource,
}

impl Default for DropShadowEngine {
    fn default() -> Self {
        Self {
            light_source: LightSource::DEFAULT_OVERHEAD,
        }
    }
}

impl DropShadowEngine {
    pub fn new(light_source: LightSource) -> Self {
        Self { light_source }
    }

    /// Retrieve Windows 11 Fluent dual-shadow specifications for a specified elevation level.
    pub fn get_shadow(&self, elevation: Elevation) -> FluentShadow {
        match elevation {
            Elevation::Level0 => FluentShadow {
                elevation,
                ambient: ShadowLayer {
                    offset_x: 0.0,
                    offset_y: 0.0,
                    blur_radius: 0.0,
                    spread: 0.0,
                    color: Color::BLACK,
                    opacity: 0.0,
                },
                directional: ShadowLayer {
                    offset_x: 0.0,
                    offset_y: 0.0,
                    blur_radius: 0.0,
                    spread: 0.0,
                    color: Color::BLACK,
                    opacity: 0.0,
                },
            },
            Elevation::Level2 => FluentShadow {
                elevation,
                ambient: ShadowLayer {
                    offset_x: 0.0,
                    offset_y: 1.0,
                    blur_radius: 2.0,
                    spread: 0.0,
                    color: Color::BLACK,
                    opacity: 0.08,
                },
                directional: ShadowLayer {
                    offset_x: 0.0,
                    offset_y: 2.0,
                    blur_radius: 4.0,
                    spread: 0.0,
                    color: Color::BLACK,
                    opacity: 0.14,
                },
            },
            Elevation::Level4 => FluentShadow {
                elevation,
                ambient: ShadowLayer {
                    offset_x: 0.0,
                    offset_y: 2.0,
                    blur_radius: 4.0,
                    spread: 0.0,
                    color: Color::BLACK,
                    opacity: 0.10,
                },
                directional: ShadowLayer {
                    offset_x: 0.0,
                    offset_y: 4.0,
                    blur_radius: 8.0,
                    spread: 0.0,
                    color: Color::BLACK,
                    opacity: 0.16,
                },
            },
            Elevation::Level8 => FluentShadow {
                elevation,
                ambient: ShadowLayer {
                    offset_x: 0.0,
                    offset_y: 4.0,
                    blur_radius: 8.0,
                    spread: 0.0,
                    color: Color::BLACK,
                    opacity: 0.12,
                },
                directional: ShadowLayer {
                    offset_x: 0.0,
                    offset_y: 8.0,
                    blur_radius: 16.0,
                    spread: 0.0,
                    color: Color::BLACK,
                    opacity: 0.20,
                },
            },
            Elevation::Level16 => FluentShadow {
                elevation,
                ambient: ShadowLayer {
                    offset_x: 0.0,
                    offset_y: 8.0,
                    blur_radius: 16.0,
                    spread: 0.0,
                    color: Color::BLACK,
                    opacity: 0.14,
                },
                directional: ShadowLayer {
                    offset_x: 0.0,
                    offset_y: 16.0,
                    blur_radius: 32.0,
                    spread: 0.0,
                    color: Color::BLACK,
                    opacity: 0.24,
                },
            },
            Elevation::Level32 => FluentShadow {
                elevation,
                ambient: ShadowLayer {
                    offset_x: 0.0,
                    offset_y: 12.0,
                    blur_radius: 32.0,
                    spread: 0.0,
                    color: Color::BLACK,
                    opacity: 0.18,
                },
                directional: ShadowLayer {
                    offset_x: 0.0,
                    offset_y: 24.0,
                    blur_radius: 48.0,
                    spread: 0.0,
                    color: Color::BLACK,
                    opacity: 0.28,
                },
            },
            Elevation::Level64 => FluentShadow {
                elevation,
                ambient: ShadowLayer {
                    offset_x: 0.0,
                    offset_y: 24.0,
                    blur_radius: 64.0,
                    spread: 0.0,
                    color: Color::BLACK,
                    opacity: 0.22,
                },
                directional: ShadowLayer {
                    offset_x: 0.0,
                    offset_y: 48.0,
                    blur_radius: 96.0,
                    spread: 0.0,
                    color: Color::BLACK,
                    opacity: 0.35,
                },
            },
        }
    }

    /// Render shadow bitmap for a rounded rectangle.
    pub fn render_shadow_surface(
        &self,
        elem_w: f32,
        elem_h: f32,
        radii: CornerRadii,
        elevation: Elevation,
    ) -> (Surface, f32, f32) {
        let shadow = self.get_shadow(elevation);
        let (margin_l, margin_t, margin_r, margin_b) = shadow.shadow_margins();

        let surf_w = (elem_w + margin_l + margin_r).ceil() as usize;
        let surf_h = (elem_h + margin_t + margin_b).ceil() as usize;

        let mut surface = Surface::new(surf_w, surf_h, Color::TRANSPARENT);

        // Helper to render a single blurred shadow layer
        let render_layer = |layer: &ShadowLayer| -> Surface {
            let mut layer_surf = Surface::new(surf_w, surf_h, Color::TRANSPARENT);
            let eff_color = layer.effective_color();

            for y in 0..surf_h {
                for x in 0..surf_w {
                    // Coordinates relative to element origin
                    let elem_x = x as f32 - margin_l - layer.offset_x;
                    let elem_y = y as f32 - margin_t - layer.offset_y;

                    let cov = radii.coverage_at(elem_x, elem_y, elem_w + layer.spread, elem_h + layer.spread);
                    if cov > 0.001 {
                        let alpha = (eff_color.a as f32 * cov).round() as u8;
                        layer_surf.set_pixel(x, y, Color::rgba(eff_color.r, eff_color.g, eff_color.b, alpha));
                    }
                }
            }

            if layer.blur_radius > 0.5 {
                layer_surf.apply_gaussian_blur(layer.blur_radius);
            }
            layer_surf
        };

        // Render ambient and directional layers
        let amb_surf = render_layer(&shadow.ambient);
        let dir_surf = render_layer(&shadow.directional);

        // Composite ambient + directional
        for y in 0..surf_h {
            for x in 0..surf_w {
                let amb_px = amb_surf.get_pixel(x, y);
                let dir_px = dir_surf.get_pixel(x, y);
                let combined = Color::blend_over(amb_px, dir_px);
                surface.set_pixel(x, y, combined);
            }
        }

        (surface, margin_l, margin_t)
    }
}

// ============================================================================
// 7. Accent Color Palette Generator & WCAG 2.1 Verifier
// ============================================================================

/// Windows 11 Fluent 7-Shade Accent Color Palette
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AccentPalette {
    /// Accent Lightest (Light 3 - highest contrast on dark themes, background tint)
    pub lightest: Color,
    /// Accent Lighter (Light 2)
    pub lighter: Color,
    /// Accent Light (Light 1)
    pub light: Color,
    /// Accent Normal (Base 24-bit RGB accent color)
    pub normal: Color,
    /// Accent Dark (Dark 1)
    pub dark: Color,
    /// Accent Darker (Dark 2)
    pub darker: Color,
    /// Accent Darkest (Dark 3 - highest contrast on light themes)
    pub darkest: Color,
}

/// Result of WCAG 2.1 contrast ratio verification
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ContrastEvaluation {
    pub ratio: f32,
    pub passes_aa_normal: bool, // >= 4.5:1
    pub passes_aa_large: bool,  // >= 3.0:1
    pub passes_aaa: bool,       // >= 7.0:1
}

impl ContrastEvaluation {
    pub fn evaluate(foreground: Color, background: Color) -> Self {
        let ratio = foreground.contrast_ratio(&background);
        Self {
            ratio,
            passes_aa_normal: ratio >= 4.5,
            passes_aa_large: ratio >= 3.0,
            passes_aaa: ratio >= 7.0,
        }
    }
}

/// Windows 11 Fluent Accent Color Palette Generator
#[derive(Debug, Clone, Copy)]
pub struct AccentPaletteGenerator;

impl AccentPaletteGenerator {
    /// Generate the full 7-shade Windows 11 Fluent Accent Palette from any 24-bit RGB accent.
    pub fn generate(base_accent: Color) -> AccentPalette {
        let (h, s, base_l) = base_accent.to_hsl();

        // Standard Windows 11 Fluent Lightness Ramp adjustments
        let lightest = Color::from_hsl(h, s * 0.70, (base_l + (1.0 - base_l) * 0.85).min(0.95));
        let lighter  = Color::from_hsl(h, s * 0.85, (base_l + (1.0 - base_l) * 0.55).min(0.85));
        let light    = Color::from_hsl(h, s * 0.95, (base_l + (1.0 - base_l) * 0.25).min(0.72));
        let normal   = base_accent;
        let dark     = Color::from_hsl(h, s * 0.95, (base_l * 0.75).max(0.35));
        let darker   = Color::from_hsl(h, s * 0.90, (base_l * 0.52).max(0.22));
        let darkest  = Color::from_hsl(h, s * 0.80, (base_l * 0.30).max(0.12));

        AccentPalette {
            lightest,
            lighter,
            light,
            normal,
            dark,
            darker,
            darkest,
        }
    }

    /// Verify WCAG 2.1 Level AA (4.5:1) compliance against specified background.
    pub fn verify_wcag_aa(foreground: Color, background: Color) -> bool {
        foreground.contrast_ratio(&background) >= 4.5
    }

    /// Select accessible accent shade from palette for a given background (guaranteeing >= 4.5:1 ratio if possible).
    pub fn accessible_accent_for(palette: &AccentPalette, background: Color) -> Color {
        let shades = [
            palette.normal,
            palette.dark,
            palette.darker,
            palette.darkest,
            palette.light,
            palette.lighter,
            palette.lightest,
        ];

        // First attempt to find a shade with >= 4.5:1 contrast
        for shade in shades.iter() {
            if Self::verify_wcag_aa(*shade, background) {
                return *shade;
            }
        }

        // Fallback: return the shade with highest contrast
        let mut best = palette.normal;
        let mut max_ratio = best.contrast_ratio(&background);
        for shade in shades.iter() {
            let r = shade.contrast_ratio(&background);
            if r > max_ratio {
                max_ratio = r;
                best = *shade;
            }
        }
        best
    }

    /// Determine ideal high-contrast text color (White or Black) over any background color.
    pub fn high_contrast_text_color(background: Color) -> Color {
        let white_contrast = Color::WHITE.contrast_ratio(&background);
        let black_contrast = Color::BLACK.contrast_ratio(&background);
        if white_contrast >= black_contrast {
            Color::WHITE
        } else {
            Color::BLACK
        }
    }
}

// ============================================================================
// 8. Segoe UI Variable Typography Metrics
// ============================================================================

/// Font weights supported by Segoe UI Variable
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FontWeight {
    Thin = 100,
    Light = 300,
    Regular = 400,
    Medium = 500,
    SemiBold = 600,
    Bold = 700,
    Black = 900,
}

/// Windows 11 Fluent Typography Style Specification
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TypographyStyle {
    pub name: &'static str,
    pub pixel_size: f32,
    pub weight: FontWeight,
    pub line_height: f32,
    pub character_spacing: f32, // Em units or pixels
}

impl TypographyStyle {
    /// Calculate required height for multi-line text block given line count.
    #[inline]
    pub fn calculate_block_height(&self, line_count: usize) -> f32 {
        if line_count == 0 {
            0.0
        } else {
            self.pixel_size + (line_count.saturating_sub(1) as f32 * self.line_height)
        }
    }

    /// Typical font baseline offset from top of line box
    #[inline]
    pub fn baseline_offset(&self) -> f32 {
        self.pixel_size * 0.8
    }
}

/// Windows 11 Fluent Typography Type Ramp
#[derive(Debug, Clone, Copy)]
pub struct SegoeUIVariable;

impl SegoeUIVariable {
    /// Display: 68px, Bold (700), Line Height: 92px
    pub const DISPLAY: TypographyStyle = TypographyStyle {
        name: "Display",
        pixel_size: 68.0,
        weight: FontWeight::Bold,
        line_height: 92.0,
        character_spacing: 0.0,
    };

    /// TitleLarge: 40px, Semi-Bold (600), Line Height: 52px
    pub const TITLE_LARGE: TypographyStyle = TypographyStyle {
        name: "TitleLarge",
        pixel_size: 40.0,
        weight: FontWeight::SemiBold,
        line_height: 52.0,
        character_spacing: 0.0,
    };

    /// Title: 28px, Semi-Bold (600), Line Height: 36px
    pub const TITLE: TypographyStyle = TypographyStyle {
        name: "Title",
        pixel_size: 28.0,
        weight: FontWeight::SemiBold,
        line_height: 36.0,
        character_spacing: 0.0,
    };

    /// Subtitle: 20px, Semi-Bold (600), Line Height: 28px
    pub const SUBTITLE: TypographyStyle = TypographyStyle {
        name: "Subtitle",
        pixel_size: 20.0,
        weight: FontWeight::SemiBold,
        line_height: 28.0,
        character_spacing: 0.0,
    };

    /// BodyLarge: 18px, Regular (400), Line Height: 24px
    pub const BODY_LARGE: TypographyStyle = TypographyStyle {
        name: "BodyLarge",
        pixel_size: 18.0,
        weight: FontWeight::Regular,
        line_height: 24.0,
        character_spacing: 0.0,
    };

    /// Body: 14px, Regular (400), Line Height: 20px (Windows 11 standard default body text)
    pub const BODY: TypographyStyle = TypographyStyle {
        name: "Body",
        pixel_size: 14.0,
        weight: FontWeight::Regular,
        line_height: 20.0,
        character_spacing: 0.0,
    };

    /// BodyStrong: 14px, Semi-Bold (600), Line Height: 20px
    pub const BODY_STRONG: TypographyStyle = TypographyStyle {
        name: "BodyStrong",
        pixel_size: 14.0,
        weight: FontWeight::SemiBold,
        line_height: 20.0,
        character_spacing: 0.0,
    };

    /// Caption: 12px, Regular (400), Line Height: 16px
    pub const CAPTION: TypographyStyle = TypographyStyle {
        name: "Caption",
        pixel_size: 12.0,
        weight: FontWeight::Regular,
        line_height: 16.0,
        character_spacing: 0.0,
    };

    /// CaptionStrong: 12px, Semi-Bold (600), Line Height: 16px
    pub const CAPTION_STRONG: TypographyStyle = TypographyStyle {
        name: "CaptionStrong",
        pixel_size: 12.0,
        weight: FontWeight::SemiBold,
        line_height: 16.0,
        character_spacing: 0.0,
    };

    /// Retrieve all standard styles in the type ramp.
    pub fn all_styles() -> &'static [TypographyStyle] {
        &[
            Self::DISPLAY,
            Self::TITLE_LARGE,
            Self::TITLE,
            Self::SUBTITLE,
            Self::BODY_LARGE,
            Self::BODY,
            Self::BODY_STRONG,
            Self::CAPTION,
            Self::CAPTION_STRONG,
        ]
    }
}

// ============================================================================
// 9. Comprehensive Unit Test Suite
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mica_materials_and_fallback() {
        // Test Mica Base & Mica Alt construction
        let mica_base_light = MicaMaterial::base(ThemeMode::Light);
        let mica_base_dark = MicaMaterial::base(ThemeMode::Dark);
        let mica_alt_light = MicaMaterial::alt(ThemeMode::Light);
        let mica_alt_dark = MicaMaterial::alt(ThemeMode::Dark);

        assert_eq!(mica_base_light.config.kind, MicaKind::Base);
        assert_eq!(mica_base_dark.config.kind, MicaKind::Base);
        assert_eq!(mica_alt_light.config.kind, MicaKind::Alt);
        assert_eq!(mica_alt_dark.config.kind, MicaKind::Alt);

        // Wallpaper sample color
        let sample = Color::rgb(50, 100, 150);

        // 1. Normal state: active window, transparency on, normal power mode
        let resolved_light = mica_base_light.resolve_color(sample, true, true, PowerMode::Normal);
        assert_ne!(resolved_light, mica_base_light.config.fallback_color);

        // 2. Battery saver fallback: must return low_power_fallback
        let resolved_battery = mica_base_light.resolve_color(sample, true, true, PowerMode::BatterySaver);
        assert_eq!(resolved_battery, mica_base_light.config.low_power_fallback);

        // 3. Low power mode fallback
        let resolved_low_power = mica_base_dark.resolve_color(sample, true, true, PowerMode::LowPower);
        assert_eq!(resolved_low_power, mica_base_dark.config.low_power_fallback);

        // 4. Transparency disabled fallback
        let resolved_no_trans = mica_alt_light.resolve_color(sample, true, false, PowerMode::Normal);
        assert_eq!(resolved_no_trans, mica_alt_light.config.fallback_color);

        // 5. Inactive window fallback
        let resolved_inactive = mica_alt_dark.resolve_color(sample, false, true, PowerMode::Normal);
        assert_eq!(resolved_inactive, mica_alt_dark.config.fallback_color);
    }

    #[test]
    fn test_acrylic_blur_and_exclusion() {
        let acrylic_bg = AcrylicMaterial::background(ThemeMode::Light);
        let acrylic_inapp = AcrylicMaterial::in_app(ThemeMode::Dark);

        assert_eq!(acrylic_bg.config.kind, AcrylicKind::Background);
        assert_eq!(acrylic_inapp.config.kind, AcrylicKind::InApp);

        // Create a test 16x16 backdrop with a sharp color boundary
        let mut backdrop = Surface::new(16, 16, Color::WHITE);
        for y in 0..8 {
            for x in 0..16 {
                backdrop.set_pixel(x, y, Color::BLACK);
            }
        }

        // Render acrylic
        let rendered = acrylic_bg.render(&backdrop, true, PowerMode::Normal, 12345);
        assert_eq!(rendered.width, 16);
        assert_eq!(rendered.height, 16);

        // Verify blur smoothed the sharp boundary: pixel near center should neither be pure white nor black
        let mid_pixel = rendered.get_pixel(8, 8);
        assert!(mid_pixel.r > 20 && mid_pixel.r < 240);

        // Verify battery saver returns fallback color
        let rendered_fallback = acrylic_bg.render(&backdrop, true, PowerMode::BatterySaver, 12345);
        assert_eq!(rendered_fallback.get_pixel(0, 0), acrylic_bg.config.fallback_color);

        // Test exclusion blend formula
        let b = Color::rgb(200, 100, 50);
        let s = Color::rgba(100, 150, 200, 255);
        let excl = Color::blend_exclusion(b, s);
        assert!(excl.r > 0 && excl.g > 0 && excl.b > 0);

        // Test noise generator
        let n1 = AcrylicMaterial::pseudo_noise(10, 20, 42);
        let n2 = AcrylicMaterial::pseudo_noise(10, 20, 42);
        let n3 = AcrylicMaterial::pseudo_noise(11, 20, 42);
        assert_eq!(n1, n2);
        assert_ne!(n1, n3);
        assert!((0.0..=1.0).contains(&n1));
    }

    #[test]
    fn test_corner_radii_engine() {
        // Standard presets
        let outer = CornerRadii::default_outer();
        let inner = CornerRadii::default_inner();
        let pill = CornerRadii::pill(32.0);

        assert_eq!(outer.top_left, 8.0);
        assert_eq!(outer.bottom_right, 8.0);
        assert_eq!(inner.top_left, 4.0);
        assert_eq!(inner.bottom_right, 4.0);
        assert_eq!(pill.top_left, 16.0);
        assert_eq!(pill.bottom_left, 16.0);

        // Asymmetric Tab Radii
        let tab = CornerRadii::tab(8.0);
        assert_eq!(tab.top_left, 8.0);
        assert_eq!(tab.top_right, 8.0);
        assert_eq!(tab.bottom_left, 0.0);
        assert_eq!(tab.bottom_right, 0.0);

        // Point containment & SDF
        let rect_w = 100.0;
        let rect_h = 50.0;

        // Center point is inside
        assert!(outer.contains_point(50.0, 25.0, rect_w, rect_h));
        assert!(outer.signed_distance(50.0, 25.0, rect_w, rect_h) < 0.0);

        // Far outside point is outside
        assert!(!outer.contains_point(150.0, 25.0, rect_w, rect_h));
        assert!(outer.signed_distance(150.0, 25.0, rect_w, rect_h) > 0.0);

        // Top-left corner cut-off (0,0 is outside the 8px rounded corner)
        assert!(!outer.contains_point(0.0, 0.0, rect_w, rect_h));
        assert_eq!(outer.coverage_at(0.0, 0.0, rect_w, rect_h), 0.0);

        // Center coverage is 1.0 (fully opaque)
        assert_eq!(outer.coverage_at(50.0, 25.0, rect_w, rect_h), 1.0);
    }

    #[test]
    fn test_drop_shadow_engine() {
        let engine = DropShadowEngine::default();

        let elevations = [
            Elevation::Level0,
            Elevation::Level2,
            Elevation::Level4,
            Elevation::Level8,
            Elevation::Level16,
            Elevation::Level32,
            Elevation::Level64,
        ];

        for &elev in &elevations {
            let shadow = engine.get_shadow(elev);
            assert_eq!(shadow.elevation, elev);

            if elev == Elevation::Level0 {
                assert_eq!(shadow.ambient.blur_radius, 0.0);
                assert_eq!(shadow.directional.blur_radius, 0.0);
            } else {
                // Higher elevation should have larger directional blur radius
                assert!(shadow.directional.blur_radius > shadow.ambient.blur_radius);
                assert!(shadow.directional.offset_y >= shadow.ambient.offset_y);
            }
        }

        // Test rendering a shadow surface
        let (surf, margin_x, margin_y) = engine.render_shadow_surface(
            64.0,
            32.0,
            CornerRadii::default_outer(),
            Elevation::Level8,
        );
        assert!(surf.width > 64);
        assert!(surf.height > 32);
        assert!(margin_x > 0.0);
        assert!(margin_y > 0.0);
    }

    #[test]
    fn test_accent_palette_generator_and_wcag() {
        let base_accent = Color::WIN11_BLUE; // #0078D4
        let palette = AccentPaletteGenerator::generate(base_accent);

        assert_eq!(palette.normal, base_accent);

        // Verify luminance ordering: Lightest should be brightest, Darkest should be darkest
        let lum_lightest = palette.lightest.relative_luminance();
        let lum_lighter = palette.lighter.relative_luminance();
        let lum_light = palette.light.relative_luminance();
        let lum_dark = palette.dark.relative_luminance();
        let lum_darker = palette.darker.relative_luminance();
        let lum_darkest = palette.darkest.relative_luminance();

        assert!(lum_lightest > lum_lighter);
        assert!(lum_lighter > lum_light);
        assert!(lum_dark > lum_darker);
        assert!(lum_darker > lum_darkest);

        // WCAG 4.5:1 Contrast Verification
        let white = Color::WHITE;
        let black = Color::BLACK;

        // Darkest accent against pure white must have high contrast
        let contrast_darkest_on_white = palette.darkest.contrast_ratio(&white);
        assert!(contrast_darkest_on_white >= 4.5);

        // Lightest accent against pure black must have high contrast
        let contrast_lightest_on_black = palette.lightest.contrast_ratio(&black);
        assert!(contrast_lightest_on_black >= 4.5);

        // Test accessible shade selection
        let accessible_for_white = AccentPaletteGenerator::accessible_accent_for(&palette, white);
        assert!(accessible_for_white.contrast_ratio(&white) >= 4.5);

        let accessible_for_black = AccentPaletteGenerator::accessible_accent_for(&palette, black);
        assert!(accessible_for_black.contrast_ratio(&black) >= 4.5);

        // High contrast text selection
        assert_eq!(AccentPaletteGenerator::high_contrast_text_color(white), black);
        assert_eq!(AccentPaletteGenerator::high_contrast_text_color(black), white);
    }

    #[test]
    fn test_segoe_ui_variable_typography() {
        let styles = SegoeUIVariable::all_styles();
        assert_eq!(styles.len(), 9);

        // Check Display
        assert_eq!(SegoeUIVariable::DISPLAY.pixel_size, 68.0);
        assert_eq!(SegoeUIVariable::DISPLAY.weight, FontWeight::Bold);
        assert_eq!(SegoeUIVariable::DISPLAY.line_height, 92.0);

        // Check TitleLarge
        assert_eq!(SegoeUIVariable::TITLE_LARGE.pixel_size, 40.0);
        assert_eq!(SegoeUIVariable::TITLE_LARGE.weight, FontWeight::SemiBold);
        assert_eq!(SegoeUIVariable::TITLE_LARGE.line_height, 52.0);

        // Check Title
        assert_eq!(SegoeUIVariable::TITLE.pixel_size, 28.0);
        assert_eq!(SegoeUIVariable::TITLE.weight, FontWeight::SemiBold);
        assert_eq!(SegoeUIVariable::TITLE.line_height, 36.0);

        // Check Subtitle
        assert_eq!(SegoeUIVariable::SUBTITLE.pixel_size, 20.0);
        assert_eq!(SegoeUIVariable::SUBTITLE.weight, FontWeight::SemiBold);
        assert_eq!(SegoeUIVariable::SUBTITLE.line_height, 28.0);

        // Check BodyLarge
        assert_eq!(SegoeUIVariable::BODY_LARGE.pixel_size, 18.0);
        assert_eq!(SegoeUIVariable::BODY_LARGE.weight, FontWeight::Regular);
        assert_eq!(SegoeUIVariable::BODY_LARGE.line_height, 24.0);

        // Check Body
        assert_eq!(SegoeUIVariable::BODY.pixel_size, 14.0);
        assert_eq!(SegoeUIVariable::BODY.weight, FontWeight::Regular);
        assert_eq!(SegoeUIVariable::BODY.line_height, 20.0);

        // Check Caption
        assert_eq!(SegoeUIVariable::CAPTION.pixel_size, 12.0);
        assert_eq!(SegoeUIVariable::CAPTION.weight, FontWeight::Regular);
        assert_eq!(SegoeUIVariable::CAPTION.line_height, 16.0);

        // Multi-line height calculation
        let body_height_3_lines = SegoeUIVariable::BODY.calculate_block_height(3);
        // line 1: 14px + 2 lines * 20px = 54px
        assert_eq!(body_height_3_lines, 54.0);
    }

    #[test]
    fn test_linear_color_and_blend_modes() {
        // Test gamma expansion and compression round-trip
        for v in [0u8, 12, 64, 128, 200, 255] {
            let lin = Color::channel_to_linear(v);
            let back = Color::linear_to_channel(lin);
            assert_eq!(v, back);
        }

        // Test Luminosity blending preserves hue & saturation of backdrop
        let bg = Color::rgb(255, 0, 0); // Pure Red
        let src = Color::rgb(100, 100, 100); // Gray
        let lum_blended = Color::blend_luminosity(bg, src.with_alpha(255));
        let (h, _, _) = lum_blended.to_hsl();
        assert!((h - 0.0).abs() < 1e-3 || (h - 360.0).abs() < 1e-3); // Hue is preserved as red

        // Test Alpha compositing
        let base = Color::rgb(0, 0, 255);
        let overlay = Color::rgba(255, 0, 0, 128);
        let composited = Color::blend_over(base, overlay);
        assert!(composited.r > 100);
        assert!(composited.b > 100);
        assert_eq!(composited.a, 255);
    }
}
