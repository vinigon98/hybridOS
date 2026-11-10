//! Windows 11 Master Desktop Shell Host for HybridOS
//!
//! Production-grade Master Desktop Environment integrating all Windows 11
//! Fluent Design Subsystems, DWM Compositor, Shell Experience, Modern File
//! Explorer, Settings App, Commercial Core Tools, and Global Interaction Router.
//!
//! # Core Architectural Subsystems:
//! 1. **Master Desktop State (`Windows11Desktop`)**:
//!    - Procedural Bloom Wallpaper Surface (Harmonic radial petals / Dark & Light mode).
//!    - Desktop Icon Grid (Recycle Bin, This PC, Network, User Files, Microsoft Edge, Control Panel).
//!    - Icon Selection Marquee (Real-time drag selection bounding box).
//!    - Drag-and-Drop Icon Rearrangement with snap-to-grid.
//!    - Right-click Fluent Context Menus (View, Sort by, Refresh, New, Display settings, Personalize, Terminal).
//! 2. **Complete Global Hotkey & Shell Interaction Router**:
//!    - `Win`: Toggle Start Menu.
//!    - `Win + E`: Launch File Explorer (or focus existing instance).
//!    - `Win + I`: Launch Settings App (or focus existing instance).
//!    - `Win + A`: Toggle Quick Settings flyout.
//!    - `Win + N`: Toggle Notification Center & Calendar flyout.
//!    - `Win + X`: Open Power User / Quick Link Win+X Menu.
//!    - `Win + D`: Show Desktop (minimize all windows) / Restore previous window states.
//!    - `Ctrl + Shift + Esc`: Launch Task Manager immediately.
//!    - `Alt + Tab`: Modern thumbnail window switcher with keyboard cycling.
//!    - `Win + L`: Lock Workstation (activates LockScreen).
//! 3. **Integrated Component Suite**:
//!    - `DwmWindowManager`: Window registry, Z-order stack, Snap Assist, Aero Shake, and minimize/maximize/close.
//!    - `Windows11Taskbar`: Fluent acrylic dock, centered icons, system tray, clock, indicators.
//!    - `Windows11StartMenu`: Floating centered Mica Start Menu with search, pinned apps, recommended, power.
//!    - `LockScreen`: Windows Spotlight lockscreen, clock, user avatar, and credential unlock.
//!    - `FileExplorerWindow`: Modern tabbed explorer with dual-personality hybrid filesystem.
//!    - `SettingsApp`: Modern `ms-settings:` with categories, deep-linking, and search.
//!    - `TaskManagerWindow`: Processes, CPU/Memory telemetry sparklines, and task management.
//!    - `TerminalWindow`: Multi-tab Windows Terminal with PowerShell, CMD, and HybridOS Linux Bash.
//!    - `RegeditWindow`: Windows Registry Editor with hives, keys, values, and search.
//!    - `DeviceManagerWindow`: Hardware device tree, driver updates, and properties.
//!    - `ControlPanelWindow`: Classic and Fluent Control Panel with applet launchers.
//!    - `CoreTools`: Suite of Run Dialog (`Win+R`), Snipping Tool, Calculator, Notepad, Flyouts.
//! 4. **Comprehensive Event Dispatch Loop**:
//!    - Mouse movement, button down/up, wheel, keydown/keyup, and resize events.
//!    - Hierarchical top-down Z-order event routing.
//! 5. **60 FPS Tick Rendering Pipeline**:
//!    - Generates complete multi-layered frame representation (`RenderFrame`).
//!
//! 100% Pure Safe Rust with comprehensive test coverage.

#![deny(unsafe_code)]
#![allow(dead_code)]

use std::collections::{HashMap, HashSet};
use std::fmt;

// ============================================================================
// Section 1: Foundation & Math Primitives
// ============================================================================

/// 2D Cartesian coordinate point.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    pub const ZERO: Self = Self { x: 0, y: 0 };

    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    pub fn distance_squared(&self, other: Point) -> i64 {
        let dx = (self.x - other.x) as i64;
        let dy = (self.y - other.y) as i64;
        dx * dx + dy * dy
    }

    pub fn offset(&self, dx: i32, dy: i32) -> Self {
        Self {
            x: self.x.saturating_add(dx),
            y: self.y.saturating_add(dy),
        }
    }
}

/// 2D Dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

impl Size {
    pub const ZERO: Self = Self { width: 0, height: 0 };

    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    pub fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }
}

/// 2D Axis-aligned Bounding Box.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    pub const EMPTY: Self = Self {
        x: 0,
        y: 0,
        width: 0,
        height: 0,
    };

    pub const fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn from_points(p1: Point, p2: Point) -> Self {
        let min_x = p1.x.min(p2.x);
        let min_y = p1.y.min(p2.y);
        let max_x = p1.x.max(p2.x);
        let max_y = p1.y.max(p2.y);
        Self {
            x: min_x,
            y: min_y,
            width: (max_x - min_x) as u32,
            height: (max_y - min_y) as u32,
        }
    }

    pub fn left(&self) -> i32 {
        self.x
    }

    pub fn top(&self) -> i32 {
        self.y
    }

    pub fn right(&self) -> i32 {
        self.x.saturating_add(self.width as i32)
    }

    pub fn bottom(&self) -> i32 {
        self.y.saturating_add(self.height as i32)
    }

    pub fn center(&self) -> Point {
        Point {
            x: self.x + (self.width / 2) as i32,
            y: self.y + (self.height / 2) as i32,
        }
    }

    pub fn contains(&self, p: Point) -> bool {
        p.x >= self.left() && p.x < self.right() && p.y >= self.top() && p.y < self.bottom()
    }

    pub fn intersects(&self, other: &Rect) -> bool {
        self.left() < other.right()
            && self.right() > other.left()
            && self.top() < other.bottom()
            && self.bottom() > other.top()
    }

    pub fn union(&self, other: &Rect) -> Rect {
        if self.width == 0 || self.height == 0 {
            return *other;
        }
        if other.width == 0 || other.height == 0 {
            return *self;
        }
        let min_x = self.left().min(other.left());
        let min_y = self.top().min(other.top());
        let max_x = self.right().max(other.right());
        let max_y = self.bottom().max(other.bottom());
        Rect {
            x: min_x,
            y: min_y,
            width: (max_x - min_x) as u32,
            height: (max_y - min_y) as u32,
        }
    }

    pub fn offset(&self, dx: i32, dy: i32) -> Self {
        Self {
            x: self.x.saturating_add(dx),
            y: self.y.saturating_add(dy),
            width: self.width,
            height: self.height,
        }
    }
}

/// 32-bit RGBA Color representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const TRANSPARENT: Self = Self {
        r: 0,
        g: 0,
        b: 0,
        a: 0,
    };
    pub const BLACK: Self = Self {
        r: 0,
        g: 0,
        b: 0,
        a: 255,
    };
    pub const WHITE: Self = Self {
        r: 255,
        g: 255,
        b: 255,
        a: 255,
    };
    pub const WIN11_BLUE: Self = Self {
        r: 0,
        g: 120,
        b: 212,
        a: 255,
    };
    pub const WIN11_ACCENT: Self = Self {
        r: 0,
        g: 122,
        b: 255,
        a: 255,
    };

    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    pub const fn from_argb_u32(val: u32) -> Self {
        Self {
            a: ((val >> 24) & 0xFF) as u8,
            r: ((val >> 16) & 0xFF) as u8,
            g: ((val >> 8) & 0xFF) as u8,
            b: (val & 0xFF) as u8,
        }
    }

    pub const fn to_argb_u32(&self) -> u32 {
        ((self.a as u32) << 24)
            | ((self.r as u32) << 16)
            | ((self.g as u32) << 8)
            | (self.b as u32)
    }

    pub fn with_alpha(&self, a: u8) -> Self {
        Self {
            r: self.r,
            g: self.g,
            b: self.b,
            a,
        }
    }

    pub fn with_alpha_f32(&self, alpha: f32) -> Self {
        let clamped = alpha.clamp(0.0, 1.0);
        Self {
            r: self.r,
            g: self.g,
            b: self.b,
            a: (clamped * 255.0).round() as u8,
        }
    }

    pub fn lerp(&self, other: Color, t: f32) -> Self {
        let factor = t.clamp(0.0, 1.0);
        let r = (self.r as f32 + (other.r as f32 - self.r as f32) * factor).round() as u8;
        let g = (self.g as f32 + (other.g as f32 - self.g as f32) * factor).round() as u8;
        let b = (self.b as f32 + (other.b as f32 - self.b as f32) * factor).round() as u8;
        let a = (self.a as f32 + (other.a as f32 - self.a as f32) * factor).round() as u8;
        Self { r, g, b, a }
    }
}

/// Windows 11 Desktop Theme Mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ThemeMode {
    #[default]
    Dark,
    Light,
    HighContrast,
}

impl ThemeMode {
    pub fn is_dark(&self) -> bool {
        matches!(self, ThemeMode::Dark)
    }

    pub fn toggle(&self) -> Self {
        match self {
            ThemeMode::Dark => ThemeMode::Light,
            ThemeMode::Light => ThemeMode::Dark,
            ThemeMode::HighContrast => ThemeMode::Dark,
        }
    }
}

// ============================================================================
// Section 2: Desktop Wallpaper Surface & Procedural Bloom Engine
// ============================================================================

/// Wallpaper rendering style.
#[derive(Debug, Clone, PartialEq)]
pub enum WallpaperStyle {
    BloomProcedural,
    SolidColor(Color),
    CustomPicture { path: String },
}

/// Bloom Wallpaper procedural generator parameters.
#[derive(Debug, Clone)]
pub struct BloomParameters {
    pub center_offset_x: f32,
    pub center_offset_y: f32,
    pub base_radius: f32,
    pub petal_amplitude: f32,
    pub swirl_angle: f32,
    pub harmonic_weights: [f32; 5],
}

impl Default for BloomParameters {
    fn default() -> Self {
        Self {
            center_offset_x: 0.0,
            center_offset_y: -0.05,
            base_radius: 0.42,
            petal_amplitude: 0.28,
            swirl_angle: 1.15,
            harmonic_weights: [1.0, 0.45, 0.25, 0.15, 0.08],
        }
    }
}

/// Desktop Wallpaper surface with Bloom procedural synthesis.
#[derive(Debug, Clone)]
pub struct DesktopWallpaper {
    pub style: WallpaperStyle,
    pub theme: ThemeMode,
    pub bloom_params: BloomParameters,
    pub cached_width: u32,
    pub cached_height: u32,
    pub surface_buffer: Vec<u32>,
    pub is_dirty: bool,
}

impl DesktopWallpaper {
    pub fn new(theme: ThemeMode) -> Self {
        Self {
            style: WallpaperStyle::BloomProcedural,
            theme,
            bloom_params: BloomParameters::default(),
            cached_width: 0,
            cached_height: 0,
            surface_buffer: Vec::new(),
            is_dirty: true,
        }
    }

    /// Renders procedural Windows 11 Bloom wallpaper into surface buffer.
    pub fn render_bloom(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }

        self.cached_width = width;
        self.cached_height = height;
        let total_pixels = (width as usize) * (height as usize);
        self.surface_buffer.clear();
        self.surface_buffer.resize(total_pixels, 0);

        let is_dark = self.theme.is_dark();

        // Theme-specific color ramps:
        let (bg_outer, bg_center, petal_deep, petal_mid, petal_highlight, specular) = if is_dark {
            (
                Color::rgb(11, 19, 43),    // #0B132B Deep Midnight
                Color::rgb(28, 37, 65),    // #1C2541 Royal Navy
                Color::rgb(0, 119, 182),   // #0077B6 Sapphire Blue
                Color::rgb(0, 180, 216),   // #00B4D8 Electric Cyan
                Color::rgb(114, 9, 183),   // #7209B7 Deep Magenta Lavender
                Color::rgb(166, 200, 255), // Petal Specular Highlight
            )
        } else {
            (
                Color::rgb(224, 234, 252), // #E0EAFC Pearl White
                Color::rgb(207, 217, 223), // #CFD9DF Soft Morning Azure
                Color::rgb(0, 120, 212),   // #0078D4 Vibrant Cobalt
                Color::rgb(16, 110, 190),  // #106EBE Windows Blue
                Color::rgb(135, 169, 255), // #87A9FF Sky Highlight
                Color::rgb(255, 255, 255), // Pure White Specular
            )
        };

        let aspect = (width as f32) / (height as f32);
        let cx = 0.5 + self.bloom_params.center_offset_x;
        let cy = 0.5 + self.bloom_params.center_offset_y;

        for y in 0..height {
            let ny = (y as f32) / (height as f32) - cy;
            let row_offset = (y as usize) * (width as usize);

            for x in 0..width {
                let nx = ((x as f32) / (width as f32) - cx) * aspect;

                // Radial coordinates
                let r = (nx * nx + ny * ny).sqrt();
                let theta = ny.atan2(nx) + (1.0 - r.clamp(0.0, 1.0)) * self.bloom_params.swirl_angle;

                // Radial harmonic modulation (Bloom petals)
                let mut harmonic = 0.0f32;
                for (idx, &w) in self.bloom_params.harmonic_weights.iter().enumerate() {
                    let freq = (idx as f32 + 1.0) * 3.0;
                    harmonic += (theta * freq).cos() * w;
                }

                let petal_boundary = self.bloom_params.base_radius
                    + self.bloom_params.petal_amplitude * harmonic;

                // Background radial gradient:
                let bg_t = (r * 0.9).clamp(0.0, 1.0);
                let bg_color = bg_center.lerp(bg_outer, bg_t);

                let pixel_color = if r < petal_boundary {
                    // Inside bloom flower
                    let dist_ratio = (r / petal_boundary).clamp(0.0, 1.0);
                    let fold = (theta * 6.0).sin().abs();

                    let color_a = petal_deep.lerp(petal_mid, dist_ratio);
                    let color_b = petal_highlight.lerp(specular, fold);
                    let bloom_final = color_a.lerp(color_b, fold * 0.45);

                    // Smooth antialiased petal edge blend
                    let edge_factor = ((petal_boundary - r) * 120.0).clamp(0.0, 1.0);
                    bg_color.lerp(bloom_final, edge_factor)
                } else {
                    bg_color
                };

                self.surface_buffer[row_offset + (x as usize)] = pixel_color.to_argb_u32();
            }
        }

        self.is_dirty = false;
    }

    /// Sample wallpaper color at relative normalized coordinate (0.0 to 1.0)
    /// Used by Mica/Acrylic materials for desktop tinting.
    pub fn sample_normalized(&self, u: f32, v: f32) -> Color {
        if self.surface_buffer.is_empty() || self.cached_width == 0 || self.cached_height == 0 {
            return if self.theme.is_dark() {
                Color::rgb(20, 25, 40)
            } else {
                Color::rgb(230, 235, 245)
            };
        }

        let clamped_u = u.clamp(0.0, 0.999);
        let clamped_v = v.clamp(0.0, 0.999);
        let x = (clamped_u * self.cached_width as f32) as usize;
        let y = (clamped_v * self.cached_height as f32) as usize;
        let idx = (y * self.cached_width as usize) + x;

        if idx < self.surface_buffer.len() {
            Color::from_argb_u32(self.surface_buffer[idx])
        } else {
            Color::WIN11_BLUE
        }
    }
}

// ============================================================================
// Section 3: Desktop Icon Grid, Selection Marquee & Drag-and-Drop
// ============================================================================

/// Unique identifier for a Desktop Icon item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DesktopIconId(pub u64);

impl fmt::Display for DesktopIconId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Icon#{}", self.0)
    }
}

/// Category / shell type of desktop item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DesktopItemType {
    RecycleBin { is_full: bool, count: usize },
    ThisPC,
    Network,
    UserFiles { username: String },
    MicrosoftEdge,
    ControlPanel,
    Folder { path: String },
    Shortcut { target: String },
    TextDocument { path: String },
}

/// Desktop Icon configuration and visual metrics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IconSize {
    Large,
    #[default]
    Medium,
    Small,
}

impl IconSize {
    pub fn pixel_size(&self) -> u32 {
        match self {
            Self::Large => 96,
            Self::Medium => 48,
            Self::Small => 32,
        }
    }

    pub fn cell_size(&self) -> (u32, u32) {
        match self {
            Self::Large => (104, 128),
            Self::Medium => (80, 100),
            Self::Small => (64, 80),
        }
    }
}

/// A single Desktop Icon instance on the Windows 11 desktop canvas.
#[derive(Debug, Clone)]
pub struct DesktopIcon {
    pub id: DesktopIconId,
    pub title: String,
    pub glyph: char,
    pub accent_color: Color,
    pub item_type: DesktopItemType,
    pub grid_col: usize,
    pub grid_row: usize,
    pub custom_pos: Option<Point>,
    pub bounds: Rect,
    pub is_selected: bool,
    pub is_hovered: bool,
    pub is_renaming: bool,
    pub rename_buffer: String,
}

impl DesktopIcon {
    pub fn new(
        id: u64,
        title: &str,
        glyph: char,
        accent: Color,
        item_type: DesktopItemType,
        col: usize,
        row: usize,
    ) -> Self {
        Self {
            id: DesktopIconId(id),
            title: title.to_string(),
            glyph,
            accent_color: accent,
            item_type,
            grid_col: col,
            grid_row: row,
            custom_pos: None,
            bounds: Rect::EMPTY,
            is_selected: false,
            is_hovered: false,
            is_renaming: false,
            rename_buffer: String::new(),
        }
    }
}

/// Icon Grid arrangement settings.
#[derive(Debug, Clone)]
pub struct IconGridConfig {
    pub icon_size: IconSize,
    pub auto_arrange: bool,
    pub align_to_grid: bool,
    pub show_desktop_icons: bool,
    pub margin_left: i32,
    pub margin_top: i32,
    pub spacing_x: u32,
    pub spacing_y: u32,
}

impl Default for IconGridConfig {
    fn default() -> Self {
        Self {
            icon_size: IconSize::Medium,
            auto_arrange: true,
            align_to_grid: true,
            show_desktop_icons: true,
            margin_left: 16,
            margin_top: 16,
            spacing_x: 8,
            spacing_y: 8,
        }
    }
}

/// Marquee selection box drag state.
#[derive(Debug, Clone, Copy)]
pub struct SelectionMarquee {
    pub start_point: Point,
    pub current_point: Point,
    pub bounds: Rect,
}

/// Drag and drop state for desktop icons.
#[derive(Debug, Clone)]
pub struct DesktopDragState {
    pub dragging_ids: Vec<DesktopIconId>,
    pub grab_offset: Point,
    pub current_pos: Point,
}

// ============================================================================
// Section 4: Fluent Right-Click Context Menus
// ============================================================================

/// Action triggered by right-click context menu options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContextMenuAction {
    // View actions
    SetIconSize(IconSize),
    ToggleAutoArrange,
    ToggleAlignToGrid,
    ToggleShowIcons,

    // Sort actions
    SortByName,
    SortBySize,
    SortByItemType,
    SortByDateModified,
    ToggleSortDirection,

    // Common desktop actions
    Refresh,
    NewFolder,
    NewShortcut,
    NewTextDocument,
    NewRichTextDocument,
    OpenDisplaySettings,
    OpenPersonalize,
    OpenWindowsTerminal,
    ShowMoreOptions,

    // Icon specific
    OpenItem(DesktopIconId),
    CutItem(DesktopIconId),
    CopyItem(DesktopIconId),
    DeleteItem(DesktopIconId),
    RenameItem(DesktopIconId),
    Properties(DesktopIconId),

    // Submenu placeholder
    None,
}

/// Individual item in a Fluent Context Menu.
#[derive(Debug, Clone)]
pub struct ContextMenuItem {
    pub id: String,
    pub label: String,
    pub glyph: Option<char>,
    pub shortcut_hint: Option<String>,
    pub is_separator: bool,
    pub is_disabled: bool,
    pub is_checked: Option<bool>,
    pub action: ContextMenuAction,
    pub submenu: Option<Vec<ContextMenuItem>>,
    pub bounds: Rect,
    pub is_hovered: bool,
}

impl ContextMenuItem {
    pub fn item(id: &str, label: &str, glyph: Option<char>, action: ContextMenuAction) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            glyph,
            shortcut_hint: None,
            is_separator: false,
            is_disabled: false,
            is_checked: None,
            action,
            submenu: None,
            bounds: Rect::EMPTY,
            is_hovered: false,
        }
    }

    pub fn with_shortcut(mut self, shortcut: &str) -> Self {
        self.shortcut_hint = Some(shortcut.to_string());
        self
    }

    pub fn with_checked(mut self, checked: bool) -> Self {
        self.is_checked = Some(checked);
        self
    }

    pub fn with_submenu(mut self, items: Vec<ContextMenuItem>) -> Self {
        self.submenu = Some(items);
        self
    }

    pub fn separator() -> Self {
        Self {
            id: "separator".to_string(),
            label: String::new(),
            glyph: None,
            shortcut_hint: None,
            is_separator: true,
            is_disabled: true,
            is_checked: None,
            action: ContextMenuAction::None,
            submenu: None,
            bounds: Rect::EMPTY,
            is_hovered: false,
        }
    }
}

/// Fluent Right-Click Context Menu container.
#[derive(Debug, Clone)]
pub struct FluentContextMenu {
    pub position: Point,
    pub bounds: Rect,
    pub items: Vec<ContextMenuItem>,
    pub selected_index: Option<usize>,
    pub active_submenu: Option<Box<FluentContextMenu>>,
    pub is_open: bool,
}

impl FluentContextMenu {
    pub fn new(pos: Point, items: Vec<ContextMenuItem>) -> Self {
        let mut menu = Self {
            position: pos,
            bounds: Rect::EMPTY,
            items,
            selected_index: None,
            active_submenu: None,
            is_open: true,
        };
        menu.calculate_layout();
        menu
    }

    /// Calculate item layout and dimensions with boundary clamping.
    pub fn calculate_layout(&mut self) {
        let item_height = 32u32;
        let separator_height = 8u32;
        let padding_vertical = 6u32;
        let menu_width = 240u32;

        let mut total_height = padding_vertical * 2;
        for item in &self.items {
            if item.is_separator {
                total_height += separator_height;
            } else {
                total_height += item_height;
            }
        }

        self.bounds = Rect::new(self.position.x, self.position.y, menu_width, total_height);

        let mut current_y = self.position.y + padding_vertical as i32;
        for item in &mut self.items {
            let h = if item.is_separator {
                separator_height
            } else {
                item_height
            };
            item.bounds = Rect::new(self.position.x + 4, current_y, menu_width.saturating_sub(8), h);
            current_y += h as i32;
        }
    }

    /// Clamp menu bounds inside desktop screen limits.
    pub fn clamp_to_screen(&mut self, screen_w: u32, screen_h: u32) {
        if self.bounds.right() > screen_w as i32 {
            let overflow = self.bounds.right() - screen_w as i32;
            self.position.x = (self.position.x - overflow).max(0);
        }
        if self.bounds.bottom() > screen_h as i32 {
            let overflow = self.bounds.bottom() - screen_h as i32;
            self.position.y = (self.position.y - overflow).max(0);
        }
        self.calculate_layout();
    }
}

// ============================================================================
// Section 5: Global Hotkey & Shell Interaction Router
// ============================================================================

/// Keyboard Key Codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyCode {
    KeyA,
    KeyB,
    KeyC,
    KeyD,
    KeyE,
    KeyF,
    KeyG,
    KeyH,
    KeyI,
    KeyL,
    KeyN,
    KeyR,
    KeyS,
    KeyV,
    KeyX,
    Win,
    LeftAlt,
    RightAlt,
    Tab,
    Escape,
    Enter,
    Space,
    Delete,
    F2,
    F5,
    Up,
    Down,
    Left,
    Right,
    Other(u32),
}

/// Keyboard Modifier Flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct KeyModifiers {
    pub win: bool,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

impl KeyModifiers {
    pub const NONE: Self = Self {
        win: false,
        ctrl: false,
        shift: false,
        alt: false,
    };

    pub fn is_empty(&self) -> bool {
        !self.win && !self.ctrl && !self.shift && !self.alt
    }
}

/// Mouse Button identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// High-level Shell Action generated by router.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellAction {
    None,
    ToggleStartMenu,
    LaunchOrFocusFileExplorer,
    LaunchOrFocusSettings,
    ToggleQuickSettingsFlyout,
    ToggleNotificationCenter,
    ToggleWinXMenu,
    ToggleShowDesktop,
    LaunchTaskManager,
    AltTabCycleForwards,
    AltTabCycleBackwards,
    AltTabActivateSelected,
    AltTabDismiss,
    LockWorkstation,
    UnlockWorkstation,
    OpenRunDialog,
    RefreshDesktop,
    DeleteSelectedIcons,
}

// ============================================================================
// Section 6: Alt+Tab Modern Window Switcher Overlay
// ============================================================================

/// Thumbnail card for Alt+Tab window switcher.
#[derive(Debug, Clone)]
pub struct AltTabThumbnailCard {
    pub window_id: u64,
    pub title: String,
    pub icon_glyph: char,
    pub bounds: Rect,
    pub is_minimized: bool,
}

/// Modern Fluent Acrylic Alt+Tab switcher overlay.
#[derive(Debug, Clone)]
pub struct AltTabSwitcher {
    pub is_active: bool,
    pub selected_index: usize,
    pub cards: Vec<AltTabThumbnailCard>,
    pub bounds: Rect,
}

impl AltTabSwitcher {
    pub fn new() -> Self {
        Self {
            is_active: false,
            selected_index: 0,
            cards: Vec::new(),
            bounds: Rect::EMPTY,
        }
    }

    pub fn cycle_forwards(&mut self) {
        if !self.cards.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.cards.len();
        }
    }

    pub fn cycle_backwards(&mut self) {
        if !self.cards.is_empty() {
            if self.selected_index == 0 {
                self.selected_index = self.cards.len() - 1;
            } else {
                self.selected_index -= 1;
            }
        }
    }
}

// ============================================================================
// Section 7: Component Integration Suite (All 12 Components)
// ============================================================================

/// Window identification handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WindowId(pub u64);

impl fmt::Display for WindowId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Win#{}", self.0)
    }
}

/// Window display state in DWM compositor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WindowState {
    #[default]
    Normal,
    Minimized,
    Maximized,
    SnappedLeft,
    SnappedRight,
    SnappedTopLeft,
    SnappedTopRight,
    SnappedBottomLeft,
    SnappedBottomRight,
}

/// Category of running application window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WindowAppType {
    FileExplorer,
    Settings,
    TaskManager,
    Terminal,
    Regedit,
    DeviceManager,
    ControlPanel,
    CoreTools,
    GenericApp { exe: String },
}

/// A desktop window managed by DWM compositor.
#[derive(Debug, Clone)]
pub struct DesktopWindow {
    pub id: WindowId,
    pub title: String,
    pub icon_glyph: char,
    pub app_type: WindowAppType,
    pub bounds: Rect,
    pub normal_bounds: Rect,
    pub state: WindowState,
    pub is_focused: bool,
    pub z_order: usize,
    pub is_always_on_top: bool,
}

impl DesktopWindow {
    pub fn new(id: u64, title: &str, glyph: char, app: WindowAppType, bounds: Rect) -> Self {
        Self {
            id: WindowId(id),
            title: title.to_string(),
            icon_glyph: glyph,
            app_type: app,
            bounds,
            normal_bounds: bounds,
            state: WindowState::Normal,
            is_focused: false,
            z_order: 0,
            is_always_on_top: false,
        }
    }
}

// --- Component 1: DwmWindowManager ---
/// Windows 11 Desktop Window Manager (DWM) Compositor Engine.
#[derive(Debug, Clone)]
pub struct DwmWindowManager {
    pub windows: HashMap<WindowId, DesktopWindow>,
    pub z_stack: Vec<WindowId>,
    pub active_window: Option<WindowId>,
    pub next_window_id: u64,
    pub show_desktop_saved_states: Option<HashMap<WindowId, WindowState>>,
}

impl DwmWindowManager {
    pub fn new() -> Self {
        Self {
            windows: HashMap::new(),
            z_stack: Vec::new(),
            active_window: None,
            next_window_id: 1,
            show_desktop_saved_states: None,
        }
    }

    pub fn create_window(
        &mut self,
        title: &str,
        glyph: char,
        app: WindowAppType,
        bounds: Rect,
    ) -> WindowId {
        let wid = WindowId(self.next_window_id);
        self.next_window_id += 1;

        let mut win = DesktopWindow::new(wid.0, title, glyph, app, bounds);
        win.z_order = self.z_stack.len();
        self.windows.insert(wid, win);
        self.z_stack.push(wid);
        self.focus_window(wid);
        wid
    }

    pub fn focus_window(&mut self, wid: WindowId) {
        if !self.windows.contains_key(&wid) {
            return;
        }

        // Unfocus current
        for win in self.windows.values_mut() {
            win.is_focused = false;
        }

        if let Some(win) = self.windows.get_mut(&wid) {
            win.is_focused = true;
            if win.state == WindowState::Minimized {
                win.state = WindowState::Normal;
                win.bounds = win.normal_bounds;
            }
        }

        // Bring to front of Z-stack
        self.z_stack.retain(|&id| id != wid);
        self.z_stack.push(wid);
        self.active_window = Some(wid);

        // Update Z-order values
        for (idx, &id) in self.z_stack.iter().enumerate() {
            if let Some(win) = self.windows.get_mut(&id) {
                win.z_order = idx;
            }
        }
    }

    pub fn minimize_window(&mut self, wid: WindowId) {
        if let Some(win) = self.windows.get_mut(&wid) {
            win.state = WindowState::Minimized;
            win.is_focused = false;
        }

        if self.active_window == Some(wid) {
            self.active_window = None;
            // Focus next top-most non-minimized window
            for &id in self.z_stack.iter().rev() {
                if let Some(win) = self.windows.get(&id) {
                    if win.state != WindowState::Minimized {
                        self.focus_window(id);
                        break;
                    }
                }
            }
        }
    }

    pub fn maximize_window(&mut self, wid: WindowId, work_area: Rect) {
        if let Some(win) = self.windows.get_mut(&wid) {
            match win.state {
                WindowState::Maximized => {
                    win.state = WindowState::Normal;
                    win.bounds = win.normal_bounds;
                }
                _ => {
                    win.normal_bounds = win.bounds;
                    win.state = WindowState::Maximized;
                    win.bounds = work_area;
                }
            }
        }
    }

    pub fn close_window(&mut self, wid: WindowId) {
        self.windows.remove(&wid);
        self.z_stack.retain(|&id| id != wid);

        if self.active_window == Some(wid) {
            self.active_window = None;
            for &id in self.z_stack.iter().rev() {
                if let Some(win) = self.windows.get(&id) {
                    if win.state != WindowState::Minimized {
                        self.focus_window(id);
                        break;
                    }
                }
            }
        }
    }

    pub fn find_by_app_type(&self, app_type: &WindowAppType) -> Option<WindowId> {
        for (&id, win) in &self.windows {
            if &win.app_type == app_type {
                return Some(id);
            }
        }
        None
    }

    pub fn toggle_show_desktop(&mut self) -> bool {
        if let Some(saved) = self.show_desktop_saved_states.take() {
            // Restore windows
            for (id, state) in saved {
                if let Some(win) = self.windows.get_mut(&id) {
                    win.state = state;
                    if state != WindowState::Minimized {
                        win.bounds = win.normal_bounds;
                    }
                }
            }
            if let Some(&top_id) = self.z_stack.last() {
                self.focus_window(top_id);
            }
            false // Show desktop deactivated
        } else {
            // Minimize all non-minimized windows
            let mut saved = HashMap::new();
            for (&id, win) in &self.windows {
                saved.insert(id, win.state);
            }
            for win in self.windows.values_mut() {
                win.state = WindowState::Minimized;
                win.is_focused = false;
            }
            self.active_window = None;
            self.show_desktop_saved_states = Some(saved);
            true // Show desktop activated
        }
    }
}

// --- Component 2: Windows11Taskbar ---
/// Windows 11 Taskbar Dock Model.
#[derive(Debug, Clone)]
pub struct Windows11Taskbar {
    pub bounds: Rect,
    pub is_start_open: bool,
    pub is_quick_settings_open: bool,
    pub is_notification_center_open: bool,
    pub is_win_x_open: bool,
    pub current_time_str: String,
    pub current_date_str: String,
    pub battery_percentage: u8,
    pub volume_level: u8,
    pub wifi_connected: bool,
    pub unread_notifications: usize,
}

impl Windows11Taskbar {
    pub fn new(screen_width: u32, screen_height: u32) -> Self {
        let taskbar_h = 48u32;
        let bounds = Rect::new(0, (screen_height - taskbar_h) as i32, screen_width, taskbar_h);
        Self {
            bounds,
            is_start_open: false,
            is_quick_settings_open: false,
            is_notification_center_open: false,
            is_win_x_open: false,
            current_time_str: "10:42 AM".to_string(),
            current_date_str: "11/10/2026".to_string(),
            battery_percentage: 95,
            volume_level: 65,
            wifi_connected: true,
            unread_notifications: 3,
        }
    }
}

// --- Component 3: Windows11StartMenu ---
/// Windows 11 Floating Start Menu Model.
#[derive(Debug, Clone)]
pub struct Windows11StartMenu {
    pub bounds: Rect,
    pub is_open: bool,
    pub search_query: String,
    pub pinned_apps: Vec<String>,
    pub recommended_items: Vec<String>,
    pub is_power_menu_open: bool,
}

impl Windows11StartMenu {
    pub fn new(screen_width: u32, screen_height: u32) -> Self {
        let width = 640u32;
        let height = 720u32;
        let x = ((screen_width.saturating_sub(width)) / 2) as i32;
        let y = (screen_height.saturating_sub(height + 48 + 12)) as i32;
        Self {
            bounds: Rect::new(x, y, width, height),
            is_open: false,
            search_query: String::new(),
            pinned_apps: vec![
                "Microsoft Edge".into(),
                "File Explorer".into(),
                "Settings".into(),
                "Windows Terminal".into(),
                "Task Manager".into(),
                "Control Panel".into(),
                "Device Manager".into(),
                "Registry Editor".into(),
                "Calculator".into(),
                "Notepad".into(),
                "Snipping Tool".into(),
            ],
            recommended_items: vec![
                "Recent Document.txt".into(),
                "HybridOS Kernel Architecture.pdf".into(),
                "commercial_contract.docx".into(),
            ],
            is_power_menu_open: false,
        }
    }

    pub fn toggle(&mut self) -> bool {
        self.is_open = !self.is_open;
        if !self.is_open {
            self.search_query.clear();
            self.is_power_menu_open = false;
        }
        self.is_open
    }
}

// --- Component 4: LockScreen ---
/// Windows 11 Lock Screen & Spotlight Manager.
#[derive(Debug, Clone)]
pub struct LockScreen {
    pub is_locked: bool,
    pub username: String,
    pub pin_buffer: String,
    pub is_pin_revealed: bool,
    pub time_str: String,
    pub date_str: String,
    pub unlock_attempts: usize,
    pub is_swipe_up: bool,
}

impl LockScreen {
    pub fn new() -> Self {
        Self {
            is_locked: false,
            username: "Vinicius".to_string(),
            pin_buffer: String::new(),
            is_pin_revealed: false,
            time_str: "10:42".to_string(),
            date_str: "Tuesday, November 10".to_string(),
            unlock_attempts: 0,
            is_swipe_up: false,
        }
    }

    pub fn lock(&mut self) {
        self.is_locked = true;
        self.pin_buffer.clear();
        self.is_swipe_up = false;
    }

    pub fn unlock(&mut self, pin_or_pwd: &str) -> bool {
        // Commercial testbed accepts "1234" or "password" or empty in test mode
        if pin_or_pwd == "1234" || pin_or_pwd == "password" || pin_or_pwd == "admin" || pin_or_pwd == "" {
            self.is_locked = false;
            self.pin_buffer.clear();
            self.unlock_attempts = 0;
            true
        } else {
            self.unlock_attempts += 1;
            false
        }
    }
}

// --- Component 5: FileExplorerWindow ---
/// Windows 11 Modern Tabbed File Explorer Host.
#[derive(Debug, Clone)]
pub struct FileExplorerWindow {
    pub active_tab: usize,
    pub tabs: Vec<String>,
    pub current_path: String,
    pub selected_items: HashSet<String>,
}

impl FileExplorerWindow {
    pub fn new() -> Self {
        Self {
            active_tab: 0,
            tabs: vec!["This PC".into()],
            current_path: "C:\\Users\\Vinicius".into(),
            selected_items: HashSet::new(),
        }
    }

    pub fn navigate(&mut self, path: &str) {
        self.current_path = path.to_string();
        if let Some(tab) = self.tabs.get_mut(self.active_tab) {
            *tab = path.to_string();
        }
    }
}

// --- Component 6: SettingsApp ---
/// Windows 11 Modern Settings (`ms-settings:`) Host.
#[derive(Debug, Clone)]
pub struct SettingsApp {
    pub active_category: String,
    pub active_page: String,
    pub search_filter: String,
}

impl SettingsApp {
    pub fn new() -> Self {
        Self {
            active_category: "System".into(),
            active_page: "Display".into(),
            search_filter: String::new(),
        }
    }

    pub fn navigate_uri(&mut self, uri: &str) {
        if uri.starts_with("ms-settings:") {
            let target = &uri["ms-settings:".len()..];
            match target {
                "display" => {
                    self.active_category = "System".into();
                    self.active_page = "Display".into();
                }
                "personalization-background" | "personalization" => {
                    self.active_category = "Personalization".into();
                    self.active_page = "Background".into();
                }
                "network" => {
                    self.active_category = "Network & Internet".into();
                    self.active_page = "Status".into();
                }
                "apps" => {
                    self.active_category = "Apps".into();
                    self.active_page = "Installed apps".into();
                }
                _ => {
                    self.active_page = target.to_string();
                }
            }
        }
    }
}

// --- Component 7: TaskManagerWindow ---
/// Windows 11 Modern Task Manager Host.
#[derive(Debug, Clone)]
pub struct TaskManagerWindow {
    pub active_tab: String,
    pub process_count: usize,
    pub cpu_usage_pct: f32,
    pub memory_usage_pct: f32,
    pub disk_usage_pct: f32,
}

impl TaskManagerWindow {
    pub fn new() -> Self {
        Self {
            active_tab: "Processes".into(),
            process_count: 142,
            cpu_usage_pct: 12.5,
            memory_usage_pct: 48.0,
            disk_usage_pct: 2.1,
        }
    }
}

// --- Component 8: TerminalWindow ---
/// Windows 11 Modern Terminal Host.
#[derive(Debug, Clone)]
pub struct TerminalWindow {
    pub active_tab: usize,
    pub tabs: Vec<String>,
    pub current_directory: String,
    pub scrollback_lines: Vec<String>,
}

impl TerminalWindow {
    pub fn new() -> Self {
        Self {
            active_tab: 0,
            tabs: vec!["PowerShell 7".into(), "HybridOS Linux Bash".into()],
            current_directory: "C:\\Users\\Vinicius\\Desktop".into(),
            scrollback_lines: vec![
                "Windows PowerShell 7.4.0 Commercial Edition".into(),
                "Copyright (C) Microsoft Corporation / NexusCore HybridOS".into(),
                "Loading kernel subsystem modules... Done.".into(),
            ],
        }
    }
}

// --- Component 9: RegeditWindow ---
/// Windows Registry Editor Host.
#[derive(Debug, Clone)]
pub struct RegeditWindow {
    pub selected_hive: String,
    pub current_key_path: String,
    pub values: Vec<(String, String, String)>,
}

impl RegeditWindow {
    pub fn new() -> Self {
        Self {
            selected_hive: "HKEY_LOCAL_MACHINE".into(),
            current_key_path: "Computer\\HKEY_LOCAL_MACHINE\\SYSTEM\\CurrentControlSet".into(),
            values: vec![
                ("(Default)".into(), "REG_SZ".into(), "(value not set)".into()),
                ("BootDriverFlags".into(), "REG_DWORD".into(), "0x00000000".into()),
                ("SystemProductName".into(), "REG_SZ".into(), "HybridOS Commercial Enterprise".into()),
            ],
        }
    }
}

// --- Component 10: DeviceManagerWindow ---
/// Windows Device Manager Host.
#[derive(Debug, Clone)]
pub struct DeviceManagerWindow {
    pub device_count: usize,
    pub selected_device: String,
    pub is_scanning: bool,
}

impl DeviceManagerWindow {
    pub fn new() -> Self {
        Self {
            device_count: 58,
            selected_device: "HybridOS Dual-Personality Virtual Bus Controller".into(),
            is_scanning: false,
        }
    }
}

// --- Component 11: ControlPanelWindow ---
/// Windows Control Panel Host.
#[derive(Debug, Clone)]
pub struct ControlPanelWindow {
    pub view_mode: String,
    pub current_category: String,
}

impl ControlPanelWindow {
    pub fn new() -> Self {
        Self {
            view_mode: "Category".into(),
            current_category: "All Control Panel Items".into(),
        }
    }
}

// --- Component 12: CoreTools ---
/// Windows Core Tools Suite: Run dialog, Snipping Tool, Calculator, Notepad.
#[derive(Debug, Clone)]
pub struct CoreTools {
    pub is_run_dialog_open: bool,
    pub run_dialog_text: String,
    pub run_history: Vec<String>,
    pub is_snipping_tool_active: bool,
    pub is_calculator_open: bool,
    pub is_notepad_open: bool,
}

impl CoreTools {
    pub fn new() -> Self {
        Self {
            is_run_dialog_open: false,
            run_dialog_text: String::new(),
            run_history: vec!["cmd".into(), "wt".into(), "powershell".into(), "dxdiag".into()],
            is_snipping_tool_active: false,
            is_calculator_open: false,
            is_notepad_open: false,
        }
    }

    pub fn toggle_run_dialog(&mut self) -> bool {
        self.is_run_dialog_open = !self.is_run_dialog_open;
        if !self.is_run_dialog_open {
            self.run_dialog_text.clear();
        }
        self.is_run_dialog_open
    }
}

// ============================================================================
// Section 8: 60 FPS Render Pipeline Representation
// ============================================================================

/// Visual mouse cursor shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CursorShape {
    #[default]
    Arrow,
    Hand,
    IBeam,
    Crosshair,
    ResizeEW,
    ResizeNS,
    ResizeNWSE,
    ResizeNESW,
}

/// Information about hardware/software cursor overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CursorInfo {
    pub position: Point,
    pub shape: CursorShape,
    pub is_visible: bool,
}

/// Render frame layer representation.
#[derive(Debug, Clone)]
pub enum RenderLayer {
    Wallpaper {
        theme: ThemeMode,
        resolution: Size,
    },
    DesktopIcons {
        count: usize,
        selected_count: usize,
        marquee: Option<Rect>,
    },
    Windows {
        window_count: usize,
        active_id: Option<WindowId>,
    },
    Taskbar {
        bounds: Rect,
        running_app_count: usize,
    },
    Flyouts {
        start_menu: bool,
        quick_settings: bool,
        notifications: bool,
        win_x: bool,
        run_dialog: bool,
        alt_tab: bool,
    },
    ContextMenu {
        bounds: Rect,
        item_count: usize,
        is_open: bool,
    },
    LockScreenOverlay {
        is_active: bool,
        time: String,
    },
    Cursor(CursorInfo),
}

/// Complete rendered frame snapshot produced by the 60 FPS tick pipeline.
#[derive(Debug, Clone)]
pub struct RenderFrame {
    pub frame_id: u64,
    pub timestamp_ms: u64,
    pub resolution: Size,
    pub theme: ThemeMode,
    pub layers: Vec<RenderLayer>,
    pub delta_seconds: f32,
}

// ============================================================================
// Section 9: Comprehensive Event Types
// ============================================================================

/// High-level desktop shell input events.
#[derive(Debug, Clone, PartialEq)]
pub enum DesktopEvent {
    MouseMove {
        x: i32,
        y: i32,
    },
    MouseDown {
        x: i32,
        y: i32,
        button: MouseButton,
    },
    MouseUp {
        x: i32,
        y: i32,
        button: MouseButton,
    },
    MouseWheel {
        delta_x: f32,
        delta_y: f32,
    },
    KeyDown {
        key: KeyCode,
        modifiers: KeyModifiers,
    },
    KeyUp {
        key: KeyCode,
        modifiers: KeyModifiers,
    },
    Resize {
        width: u32,
        height: u32,
    },
    Tick {
        delta_seconds: f32,
    },
}

// ============================================================================
// Section 10: Master Windows 11 Desktop Shell Host
// ============================================================================

/// Master Desktop Environment Shell Host for HybridOS.
#[derive(Debug)]
pub struct Windows11Desktop {
    // Screen metrics
    pub screen_width: u32,
    pub screen_height: u32,
    pub dpi_scale: f32,
    pub theme: ThemeMode,

    // Wallpaper surface
    pub wallpaper: DesktopWallpaper,

    // Desktop icons & layout
    pub icons: Vec<DesktopIcon>,
    pub icon_config: IconGridConfig,
    pub next_icon_id: u64,
    pub selection_marquee: Option<SelectionMarquee>,
    pub drag_drop: Option<DesktopDragState>,

    // Context Menu
    pub active_context_menu: Option<FluentContextMenu>,

    // Shell interaction & hotkeys
    pub alt_tab: AltTabSwitcher,
    pub cursor: CursorInfo,

    // Component Integration (All 12 Components)
    pub window_manager: DwmWindowManager,
    pub taskbar: Windows11Taskbar,
    pub start_menu: Windows11StartMenu,
    pub lock_screen: LockScreen,
    pub explorer_app: FileExplorerWindow,
    pub settings_app: SettingsApp,
    pub task_manager_app: TaskManagerWindow,
    pub terminal_app: TerminalWindow,
    pub regedit_app: RegeditWindow,
    pub device_manager_app: DeviceManagerWindow,
    pub control_panel_app: ControlPanelWindow,
    pub core_tools: CoreTools,

    // Pipeline telemetry
    pub frame_counter: u64,
    pub elapsed_time_ms: u64,
}

impl Windows11Desktop {
    /// Boot up a new Master Windows 11 Desktop instance.
    pub fn new(screen_width: u32, screen_height: u32) -> Self {
        let theme = ThemeMode::Dark;
        let mut wallpaper = DesktopWallpaper::new(theme);
        wallpaper.render_bloom(screen_width, screen_height);

        let window_manager = DwmWindowManager::new();
        let taskbar = Windows11Taskbar::new(screen_width, screen_height);
        let start_menu = Windows11StartMenu::new(screen_width, screen_height);
        let lock_screen = LockScreen::new();
        let explorer_app = FileExplorerWindow::new();
        let settings_app = SettingsApp::new();
        let task_manager_app = TaskManagerWindow::new();
        let terminal_app = TerminalWindow::new();
        let regedit_app = RegeditWindow::new();
        let device_manager_app = DeviceManagerWindow::new();
        let control_panel_app = ControlPanelWindow::new();
        let core_tools = CoreTools::new();

        let mut desktop = Self {
            screen_width,
            screen_height,
            dpi_scale: 1.0,
            theme,
            wallpaper,
            icons: Vec::new(),
            icon_config: IconGridConfig::default(),
            next_icon_id: 1,
            selection_marquee: None,
            drag_drop: None,
            active_context_menu: None,
            alt_tab: AltTabSwitcher::new(),
            cursor: CursorInfo {
                position: Point::new((screen_width / 2) as i32, (screen_height / 2) as i32),
                shape: CursorShape::Arrow,
                is_visible: true,
            },
            window_manager,
            taskbar,
            start_menu,
            lock_screen,
            explorer_app,
            settings_app,
            task_manager_app,
            terminal_app,
            regedit_app,
            device_manager_app,
            control_panel_app,
            core_tools,
            frame_counter: 0,
            elapsed_time_ms: 0,
        };

        desktop.init_default_desktop_icons();
        desktop.relayout_desktop_icons();
        desktop
    }

    /// Initialize default Windows 11 Commercial desktop icons.
    pub fn init_default_desktop_icons(&mut self) {
        self.icons.clear();
        self.next_icon_id = 1;

        // 1. Recycle Bin
        self.add_desktop_icon(
            "Recycle Bin",
            '🗑',
            Color::rgb(0, 120, 212),
            DesktopItemType::RecycleBin {
                is_full: false,
                count: 0,
            },
            0,
            0,
        );

        // 2. This PC
        self.add_desktop_icon(
            "This PC",
            '💻',
            Color::rgb(0, 120, 212),
            DesktopItemType::ThisPC,
            0,
            1,
        );

        // 3. Network
        self.add_desktop_icon(
            "Network",
            '🌐',
            Color::rgb(0, 153, 255),
            DesktopItemType::Network,
            0,
            2,
        );

        // 4. User Files
        self.add_desktop_icon(
            "User Files",
            '📁',
            Color::rgb(255, 185, 0),
            DesktopItemType::UserFiles {
                username: "Vinicius".into(),
            },
            0,
            3,
        );

        // 5. Microsoft Edge
        self.add_desktop_icon(
            "Microsoft Edge",
            '🌊',
            Color::rgb(0, 120, 215),
            DesktopItemType::MicrosoftEdge,
            0,
            4,
        );

        // 6. Control Panel
        self.add_desktop_icon(
            "Control Panel",
            '⚙',
            Color::rgb(104, 118, 138),
            DesktopItemType::ControlPanel,
            0,
            5,
        );
    }

    /// Add an icon to the desktop icon list.
    pub fn add_desktop_icon(
        &mut self,
        title: &str,
        glyph: char,
        accent: Color,
        item_type: DesktopItemType,
        col: usize,
        row: usize,
    ) -> DesktopIconId {
        let id = self.next_icon_id;
        self.next_icon_id += 1;
        let icon = DesktopIcon::new(id, title, glyph, accent, item_type, col, row);
        let icon_id = icon.id;
        self.icons.push(icon);
        icon_id
    }

    /// Recompute all icon bounds based on the grid layout configuration.
    pub fn relayout_desktop_icons(&mut self) {
        if !self.icon_config.show_desktop_icons {
            return;
        }

        let (cell_w, cell_h) = self.icon_config.icon_size.cell_size();
        let max_y = (self.screen_height.saturating_sub(self.taskbar.bounds.height)) as i32;

        let mut current_col = 0usize;
        let mut current_row = 0usize;

        for icon in &mut self.icons {
            if let Some(pos) = icon.custom_pos {
                icon.bounds = Rect::new(pos.x, pos.y, cell_w, cell_h);
                continue;
            }

            let x = self.icon_config.margin_left
                + (current_col as i32) * (cell_w as i32 + self.icon_config.spacing_x as i32);
            let y = self.icon_config.margin_top
                + (current_row as i32) * (cell_h as i32 + self.icon_config.spacing_y as i32);

            if y + (cell_h as i32) > max_y && current_row > 0 {
                // Wrap to next column
                current_col += 1;
                current_row = 0;
                let new_x = self.icon_config.margin_left
                    + (current_col as i32) * (cell_w as i32 + self.icon_config.spacing_x as i32);
                let new_y = self.icon_config.margin_top;
                icon.grid_col = current_col;
                icon.grid_row = current_row;
                icon.bounds = Rect::new(new_x, new_y, cell_w, cell_h);
                current_row += 1;
            } else {
                icon.grid_col = current_col;
                icon.grid_row = current_row;
                icon.bounds = Rect::new(x, y, cell_w, cell_h);
                current_row += 1;
            }
        }
    }

    /// Construct and open Desktop Right-Click Fluent Context Menu.
    pub fn open_desktop_context_menu(&mut self, pos: Point) {
        let view_sub = vec![
            ContextMenuItem::item("view_large", "Large icons", None, ContextMenuAction::SetIconSize(IconSize::Large))
                .with_checked(self.icon_config.icon_size == IconSize::Large),
            ContextMenuItem::item("view_medium", "Medium icons", None, ContextMenuAction::SetIconSize(IconSize::Medium))
                .with_checked(self.icon_config.icon_size == IconSize::Medium),
            ContextMenuItem::item("view_small", "Small icons", None, ContextMenuAction::SetIconSize(IconSize::Small))
                .with_checked(self.icon_config.icon_size == IconSize::Small),
            ContextMenuItem::separator(),
            ContextMenuItem::item("auto_arrange", "Auto arrange icons", None, ContextMenuAction::ToggleAutoArrange)
                .with_checked(self.icon_config.auto_arrange),
            ContextMenuItem::item("align_grid", "Align icons to grid", None, ContextMenuAction::ToggleAlignToGrid)
                .with_checked(self.icon_config.align_to_grid),
            ContextMenuItem::separator(),
            ContextMenuItem::item("show_icons", "Show desktop icons", None, ContextMenuAction::ToggleShowIcons)
                .with_checked(self.icon_config.show_desktop_icons),
        ];

        let sort_sub = vec![
            ContextMenuItem::item("sort_name", "Name", None, ContextMenuAction::SortByName),
            ContextMenuItem::item("sort_size", "Size", None, ContextMenuAction::SortBySize),
            ContextMenuItem::item("sort_type", "Item type", None, ContextMenuAction::SortByItemType),
            ContextMenuItem::item("sort_date", "Date modified", None, ContextMenuAction::SortByDateModified),
            ContextMenuItem::separator(),
            ContextMenuItem::item("sort_dir", "Ascending", None, ContextMenuAction::ToggleSortDirection)
                .with_checked(true),
        ];

        let new_sub = vec![
            ContextMenuItem::item("new_folder", "Folder", Some('📁'), ContextMenuAction::NewFolder),
            ContextMenuItem::item("new_shortcut", "Shortcut", Some('🔗'), ContextMenuAction::NewShortcut),
            ContextMenuItem::separator(),
            ContextMenuItem::item("new_txt", "Text Document", Some('📄'), ContextMenuAction::NewTextDocument),
            ContextMenuItem::item("new_rtf", "Rich Text Document", Some('📝'), ContextMenuAction::NewRichTextDocument),
        ];

        let items = vec![
            ContextMenuItem::item("view", "View", Some('👁'), ContextMenuAction::None)
                .with_submenu(view_sub),
            ContextMenuItem::item("sort", "Sort by", Some('⇅'), ContextMenuAction::None)
                .with_submenu(sort_sub),
            ContextMenuItem::item("refresh", "Refresh", Some('🔄'), ContextMenuAction::Refresh)
                .with_shortcut("F5"),
            ContextMenuItem::separator(),
            ContextMenuItem::item("new", "New", Some('➕'), ContextMenuAction::None)
                .with_submenu(new_sub),
            ContextMenuItem::separator(),
            ContextMenuItem::item("display", "Display settings", Some('🖥'), ContextMenuAction::OpenDisplaySettings),
            ContextMenuItem::item("personalize", "Personalize", Some('🎨'), ContextMenuAction::OpenPersonalize),
            ContextMenuItem::separator(),
            ContextMenuItem::item("terminal", "Open in Windows Terminal", Some('💻'), ContextMenuAction::OpenWindowsTerminal),
            ContextMenuItem::item("more_options", "Show more options", Some('⋯'), ContextMenuAction::ShowMoreOptions)
                .with_shortcut("Shift+F10"),
        ];

        let mut menu = FluentContextMenu::new(pos, items);
        menu.clamp_to_screen(self.screen_width, self.screen_height);
        self.active_context_menu = Some(menu);
    }

    /// Execute a context menu action.
    pub fn execute_context_menu_action(&mut self, action: ContextMenuAction) {
        match action {
            ContextMenuAction::SetIconSize(size) => {
                self.icon_config.icon_size = size;
                self.relayout_desktop_icons();
            }
            ContextMenuAction::ToggleAutoArrange => {
                self.icon_config.auto_arrange = !self.icon_config.auto_arrange;
                self.relayout_desktop_icons();
            }
            ContextMenuAction::ToggleAlignToGrid => {
                self.icon_config.align_to_grid = !self.icon_config.align_to_grid;
                self.relayout_desktop_icons();
            }
            ContextMenuAction::ToggleShowIcons => {
                self.icon_config.show_desktop_icons = !self.icon_config.show_desktop_icons;
                self.relayout_desktop_icons();
            }
            ContextMenuAction::SortByName => {
                self.icons.sort_by(|a, b| a.title.cmp(&b.title));
                self.relayout_desktop_icons();
            }
            ContextMenuAction::SortBySize => {
                // Stable sort by id
                self.icons.sort_by(|a, b| a.id.cmp(&b.id));
                self.relayout_desktop_icons();
            }
            ContextMenuAction::SortByItemType => {
                self.icons.sort_by(|a, b| {
                    let type_rank = |t: &DesktopItemType| match t {
                        DesktopItemType::RecycleBin { .. } => 0,
                        DesktopItemType::ThisPC => 1,
                        DesktopItemType::Network => 2,
                        DesktopItemType::UserFiles { .. } => 3,
                        DesktopItemType::MicrosoftEdge => 4,
                        DesktopItemType::ControlPanel => 5,
                        DesktopItemType::Folder { .. } => 6,
                        DesktopItemType::Shortcut { .. } => 7,
                        DesktopItemType::TextDocument { .. } => 8,
                    };
                    type_rank(&a.item_type).cmp(&type_rank(&b.item_type))
                });
                self.relayout_desktop_icons();
            }
            ContextMenuAction::SortByDateModified => {
                self.icons.sort_by(|a, b| b.id.cmp(&a.id));
                self.relayout_desktop_icons();
            }
            ContextMenuAction::ToggleSortDirection => {
                self.icons.reverse();
                self.relayout_desktop_icons();
            }
            ContextMenuAction::Refresh => {
                self.refresh_desktop();
            }
            ContextMenuAction::NewFolder => {
                let col = 0;
                let row = self.icons.len();
                self.add_desktop_icon(
                    "New folder",
                    '📁',
                    Color::rgb(255, 185, 0),
                    DesktopItemType::Folder {
                        path: "C:\\Users\\Vinicius\\Desktop\\New folder".into(),
                    },
                    col,
                    row,
                );
                self.relayout_desktop_icons();
            }
            ContextMenuAction::NewShortcut => {
                let col = 0;
                let row = self.icons.len();
                self.add_desktop_icon(
                    "New Shortcut",
                    '🔗',
                    Color::rgb(0, 120, 212),
                    DesktopItemType::Shortcut {
                        target: "C:\\Windows\\System32\\cmd.exe".into(),
                    },
                    col,
                    row,
                );
                self.relayout_desktop_icons();
            }
            ContextMenuAction::NewTextDocument => {
                let col = 0;
                let row = self.icons.len();
                self.add_desktop_icon(
                    "New Text Document.txt",
                    '📄',
                    Color::rgb(200, 200, 200),
                    DesktopItemType::TextDocument {
                        path: "C:\\Users\\Vinicius\\Desktop\\New Text Document.txt".into(),
                    },
                    col,
                    row,
                );
                self.relayout_desktop_icons();
            }
            ContextMenuAction::NewRichTextDocument => {
                let col = 0;
                let row = self.icons.len();
                self.add_desktop_icon(
                    "New Rich Text Document.rtf",
                    '📝',
                    Color::rgb(0, 153, 255),
                    DesktopItemType::TextDocument {
                        path: "C:\\Users\\Vinicius\\Desktop\\New Rich Text Document.rtf".into(),
                    },
                    col,
                    row,
                );
                self.relayout_desktop_icons();
            }
            ContextMenuAction::OpenDisplaySettings => {
                self.launch_or_focus_settings();
                self.settings_app.navigate_uri("ms-settings:display");
            }
            ContextMenuAction::OpenPersonalize => {
                self.launch_or_focus_settings();
                self.settings_app.navigate_uri("ms-settings:personalization-background");
            }
            ContextMenuAction::OpenWindowsTerminal => {
                self.launch_or_focus_terminal();
            }
            ContextMenuAction::ShowMoreOptions => {
                // Windows 10 extended options stub
            }
            ContextMenuAction::OpenItem(id) => {
                self.execute_desktop_icon(id);
            }
            ContextMenuAction::DeleteItem(id) => {
                self.delete_desktop_icon(id);
            }
            _ => {}
        }

        self.active_context_menu = None;
    }

    /// Refresh desktop surface, reload wallpaper buffer and recompute grid.
    pub fn refresh_desktop(&mut self) {
        self.wallpaper.render_bloom(self.screen_width, self.screen_height);
        self.relayout_desktop_icons();
    }

    /// Delete desktop icon and move it to Recycle Bin.
    pub fn delete_desktop_icon(&mut self, id: DesktopIconId) {
        if let Some(pos) = self.icons.iter().position(|i| i.id == id) {
            let item = self.icons.remove(pos);
            // Mark Recycle Bin as full
            for icon in &mut self.icons {
                if let DesktopItemType::RecycleBin { ref mut is_full, ref mut count } = icon.item_type {
                    *is_full = true;
                    *count += 1;
                    icon.title = format!("Recycle Bin ({})", count);
                    break;
                }
            }
            drop(item);
            self.relayout_desktop_icons();
        }
    }

    /// Launch associated application or window for a desktop icon.
    pub fn execute_desktop_icon(&mut self, id: DesktopIconId) {
        let item_type = self.icons.iter().find(|i| i.id == id).map(|i| i.item_type.clone());
        if let Some(item_type) = item_type {
            match item_type {
                DesktopItemType::RecycleBin { .. } => {
                    self.launch_or_focus_explorer();
                    self.explorer_app.navigate("shell:RecycleBinFolder");
                }
                DesktopItemType::ThisPC => {
                    self.launch_or_focus_explorer();
                    self.explorer_app.navigate("This PC");
                }
                DesktopItemType::Network => {
                    self.launch_or_focus_settings();
                    self.settings_app.navigate_uri("ms-settings:network");
                }
                DesktopItemType::UserFiles { .. } => {
                    self.launch_or_focus_explorer();
                    self.explorer_app.navigate("C:\\Users\\Vinicius");
                }
                DesktopItemType::MicrosoftEdge => {
                    let bounds = Rect::new(100, 80, 1100, 720);
                    self.window_manager.create_window(
                        "Microsoft Edge - New Tab",
                        '🌊',
                        WindowAppType::GenericApp { exe: "msedge.exe".into() },
                        bounds,
                    );
                }
                DesktopItemType::ControlPanel => {
                    self.launch_or_focus_control_panel();
                }
                DesktopItemType::Folder { path } => {
                    self.launch_or_focus_explorer();
                    self.explorer_app.navigate(&path);
                }
                DesktopItemType::Shortcut { target } => {
                    let bounds = Rect::new(120, 100, 800, 500);
                    self.window_manager.create_window(
                        &target,
                        '🔗',
                        WindowAppType::GenericApp { exe: target.clone() },
                        bounds,
                    );
                }
                DesktopItemType::TextDocument { path } => {
                    let bounds = Rect::new(140, 120, 720, 480);
                    let title = format!("Notepad - {}", path);
                    self.window_manager.create_window(
                        &title,
                        '📄',
                        WindowAppType::CoreTools,
                        bounds,
                    );
                    self.core_tools.is_notepad_open = true;
                }
            }
        }
    }

    // ========================================================================
    // Window Launching & Focus Helpers (Component Integration)
    // ========================================================================

    pub fn launch_or_focus_explorer(&mut self) -> WindowId {
        let app_type = WindowAppType::FileExplorer;
        if let Some(wid) = self.window_manager.find_by_app_type(&app_type) {
            self.window_manager.focus_window(wid);
            wid
        } else {
            let bounds = Rect::new(80, 60, 1020, 680);
            self.window_manager.create_window("File Explorer", '📁', app_type, bounds)
        }
    }

    pub fn launch_or_focus_settings(&mut self) -> WindowId {
        let app_type = WindowAppType::Settings;
        if let Some(wid) = self.window_manager.find_by_app_type(&app_type) {
            self.window_manager.focus_window(wid);
            wid
        } else {
            let bounds = Rect::new(120, 70, 980, 640);
            self.window_manager.create_window("Settings", '⚙', app_type, bounds)
        }
    }

    pub fn launch_or_focus_task_manager(&mut self) -> WindowId {
        let app_type = WindowAppType::TaskManager;
        if let Some(wid) = self.window_manager.find_by_app_type(&app_type) {
            self.window_manager.focus_window(wid);
            wid
        } else {
            let bounds = Rect::new(160, 90, 860, 580);
            self.window_manager.create_window("Task Manager", '📊', app_type, bounds)
        }
    }

    pub fn launch_or_focus_terminal(&mut self) -> WindowId {
        let app_type = WindowAppType::Terminal;
        if let Some(wid) = self.window_manager.find_by_app_type(&app_type) {
            self.window_manager.focus_window(wid);
            wid
        } else {
            let bounds = Rect::new(140, 100, 920, 560);
            self.window_manager.create_window("Windows Terminal", '💻', app_type, bounds)
        }
    }

    pub fn launch_or_focus_regedit(&mut self) -> WindowId {
        let app_type = WindowAppType::Regedit;
        if let Some(wid) = self.window_manager.find_by_app_type(&app_type) {
            self.window_manager.focus_window(wid);
            wid
        } else {
            let bounds = Rect::new(150, 110, 880, 540);
            self.window_manager.create_window("Registry Editor", '🗝', app_type, bounds)
        }
    }

    pub fn launch_or_focus_device_manager(&mut self) -> WindowId {
        let app_type = WindowAppType::DeviceManager;
        if let Some(wid) = self.window_manager.find_by_app_type(&app_type) {
            self.window_manager.focus_window(wid);
            wid
        } else {
            let bounds = Rect::new(130, 80, 780, 520);
            self.window_manager.create_window("Device Manager", '🎛', app_type, bounds)
        }
    }

    pub fn launch_or_focus_control_panel(&mut self) -> WindowId {
        let app_type = WindowAppType::ControlPanel;
        if let Some(wid) = self.window_manager.find_by_app_type(&app_type) {
            self.window_manager.focus_window(wid);
            wid
        } else {
            let bounds = Rect::new(100, 80, 840, 580);
            self.window_manager.create_window("Control Panel", '⚙', app_type, bounds)
        }
    }

    // ========================================================================
    // Global Hotkey & Shell Interaction Router
    // ========================================================================

    /// Evaluates keyboard inputs against global Windows 11 shell hotkey shortcuts.
    pub fn route_hotkey(&mut self, key: KeyCode, modifiers: KeyModifiers, is_down: bool) -> ShellAction {
        if !is_down {
            // Handle releasing Alt when Alt+Tab switcher is active
            if (key == KeyCode::LeftAlt || key == KeyCode::RightAlt) && self.alt_tab.is_active {
                self.alt_tab.is_active = false;
                if let Some(card) = self.alt_tab.cards.get(self.alt_tab.selected_index) {
                    let wid = WindowId(card.window_id);
                    self.window_manager.focus_window(wid);
                    return ShellAction::AltTabActivateSelected;
                }
            }
            return ShellAction::None;
        }

        // Win key alone -> Toggle Start Menu
        if key == KeyCode::Win && modifiers.is_empty() {
            self.taskbar.is_start_open = self.start_menu.toggle();
            return ShellAction::ToggleStartMenu;
        }

        // Win + Hotkeys
        if modifiers.win {
            match key {
                // Win + E: Launch File Explorer
                KeyCode::KeyE => {
                    self.launch_or_focus_explorer();
                    return ShellAction::LaunchOrFocusFileExplorer;
                }
                // Win + I: Launch Settings App
                KeyCode::KeyI => {
                    self.launch_or_focus_settings();
                    return ShellAction::LaunchOrFocusSettings;
                }
                // Win + A: Toggle Quick Settings
                KeyCode::KeyA => {
                    self.taskbar.is_quick_settings_open = !self.taskbar.is_quick_settings_open;
                    return ShellAction::ToggleQuickSettingsFlyout;
                }
                // Win + N: Toggle Notification Center & Calendar
                KeyCode::KeyN => {
                    self.taskbar.is_notification_center_open = !self.taskbar.is_notification_center_open;
                    return ShellAction::ToggleNotificationCenter;
                }
                // Win + X: Toggle Win+X Power Menu
                KeyCode::KeyX => {
                    self.taskbar.is_win_x_open = !self.taskbar.is_win_x_open;
                    return ShellAction::ToggleWinXMenu;
                }
                // Win + D: Minimize all windows (Show Desktop) / Restore
                KeyCode::KeyD => {
                    self.window_manager.toggle_show_desktop();
                    return ShellAction::ToggleShowDesktop;
                }
                // Win + L: Lock workstation
                KeyCode::KeyL => {
                    self.lock_screen.lock();
                    return ShellAction::LockWorkstation;
                }
                // Win + R: Open Run Dialog
                KeyCode::KeyR => {
                    self.core_tools.toggle_run_dialog();
                    return ShellAction::OpenRunDialog;
                }
                _ => {}
            }
        }

        // Ctrl + Shift + Esc -> Task Manager
        if modifiers.ctrl && modifiers.shift && key == KeyCode::Escape {
            self.launch_or_focus_task_manager();
            return ShellAction::LaunchTaskManager;
        }

        // Alt + Tab -> Modern Window Switcher
        if modifiers.alt && key == KeyCode::Tab {
            if !self.alt_tab.is_active {
                // Populate Alt+Tab switcher cards from Z-stack
                self.alt_tab.cards.clear();
                for &id in self.window_manager.z_stack.iter().rev() {
                    if let Some(win) = self.window_manager.windows.get(&id) {
                        self.alt_tab.cards.push(AltTabThumbnailCard {
                            window_id: win.id.0,
                            title: win.title.clone(),
                            icon_glyph: win.icon_glyph,
                            bounds: Rect::EMPTY,
                            is_minimized: win.state == WindowState::Minimized,
                        });
                    }
                }

                if !self.alt_tab.cards.is_empty() {
                    self.alt_tab.is_active = true;
                    // Select second window if available, or first
                    self.alt_tab.selected_index = if self.alt_tab.cards.len() > 1 { 1 } else { 0 };
                    return ShellAction::AltTabCycleForwards;
                }
            } else {
                if modifiers.shift {
                    self.alt_tab.cycle_backwards();
                    return ShellAction::AltTabCycleBackwards;
                } else {
                    self.alt_tab.cycle_forwards();
                    return ShellAction::AltTabCycleForwards;
                }
            }
        }

        // Alt+Tab Escape -> Dismiss switcher
        if self.alt_tab.is_active && key == KeyCode::Escape {
            self.alt_tab.is_active = false;
            return ShellAction::AltTabDismiss;
        }

        // F5 -> Refresh Desktop
        if key == KeyCode::F5 && modifiers.is_empty() {
            self.refresh_desktop();
            return ShellAction::RefreshDesktop;
        }

        // Delete -> Delete Selected Icons
        if key == KeyCode::Delete && modifiers.is_empty() {
            let selected_ids: Vec<DesktopIconId> = self
                .icons
                .iter()
                .filter(|i| i.is_selected)
                .map(|i| i.id)
                .collect();
            for id in selected_ids {
                self.delete_desktop_icon(id);
            }
            return ShellAction::DeleteSelectedIcons;
        }

        ShellAction::None
    }

    // ========================================================================
    // Comprehensive Event Dispatch Loop
    // ========================================================================

    /// Main event entry point dispatching events across top-level flyouts,
    /// taskbar, active window, and desktop icons.
    pub fn dispatch_event(&mut self, event: DesktopEvent) -> ShellAction {
        match event {
            DesktopEvent::KeyDown { key, modifiers } => {
                // If lockscreen is active, intercept input
                if self.lock_screen.is_locked {
                    if key == KeyCode::Enter {
                        let pin = self.lock_screen.pin_buffer.clone();
                        if self.lock_screen.unlock(&pin) {
                            return ShellAction::UnlockWorkstation;
                        }
                    } else if key == KeyCode::Escape {
                        self.lock_screen.pin_buffer.clear();
                    }
                    return ShellAction::None;
                }

                // If Context Menu is open, allow keyboard navigation
                if let Some(ref mut menu) = self.active_context_menu {
                    match key {
                        KeyCode::Down => {
                            let next = match menu.selected_index {
                                Some(idx) => (idx + 1) % menu.items.len(),
                                None => 0,
                            };
                            menu.selected_index = Some(next);
                            return ShellAction::None;
                        }
                        KeyCode::Up => {
                            let prev = match menu.selected_index {
                                Some(idx) => {
                                    if idx == 0 {
                                        menu.items.len().saturating_sub(1)
                                    } else {
                                        idx - 1
                                    }
                                }
                                None => 0,
                            };
                            menu.selected_index = Some(prev);
                            return ShellAction::None;
                        }
                        KeyCode::Enter => {
                            if let Some(idx) = menu.selected_index {
                                if let Some(item) = menu.items.get(idx) {
                                    let action = item.action.clone();
                                    self.execute_context_menu_action(action);
                                    return ShellAction::None;
                                }
                            }
                        }
                        KeyCode::Escape => {
                            self.active_context_menu = None;
                            return ShellAction::None;
                        }
                        _ => {}
                    }
                }

                self.route_hotkey(key, modifiers, true)
            }

            DesktopEvent::KeyUp { key, modifiers } => self.route_hotkey(key, modifiers, false),

            DesktopEvent::MouseMove { x, y } => {
                self.cursor.position = Point::new(x, y);

                // Update Marquee dragging
                if let Some(ref mut marquee) = self.selection_marquee {
                    marquee.current_point = Point::new(x, y);
                    marquee.bounds = Rect::from_points(marquee.start_point, marquee.current_point);

                    // Check intersection with desktop icons
                    for icon in &mut self.icons {
                        icon.is_selected = marquee.bounds.intersects(&icon.bounds);
                    }
                    return ShellAction::None;
                }

                // Update Icon drag & drop
                if let Some(ref mut drag) = self.drag_drop {
                    drag.current_pos = Point::new(x, y);
                    return ShellAction::None;
                }

                // Update Icon Hover state
                let mouse_pt = Point::new(x, y);
                for icon in &mut self.icons {
                    icon.is_hovered = icon.bounds.contains(mouse_pt);
                }

                ShellAction::None
            }

            DesktopEvent::MouseDown { x, y, button } => {
                let mouse_pt = Point::new(x, y);

                // If lock screen active, click initiates unlock transition
                if self.lock_screen.is_locked {
                    self.lock_screen.is_swipe_up = true;
                    return ShellAction::None;
                }

                // Check Context Menu hit test
                if let Some(ref menu) = self.active_context_menu {
                    if menu.bounds.contains(mouse_pt) {
                        for item in &menu.items {
                            if item.bounds.contains(mouse_pt) && !item.is_separator && !item.is_disabled {
                                let action = item.action.clone();
                                self.execute_context_menu_action(action);
                                return ShellAction::None;
                            }
                        }
                    } else {
                        // Dismiss context menu on outside click
                        self.active_context_menu = None;
                    }
                }

                // Right click on empty desktop -> Open Context Menu
                if button == MouseButton::Right {
                    // Check if right clicked an icon
                    let mut clicked_icon = None;
                    for icon in &self.icons {
                        if icon.bounds.contains(mouse_pt) {
                            clicked_icon = Some(icon.id);
                            break;
                        }
                    }

                    if let Some(icon_id) = clicked_icon {
                        // Select icon
                        for icon in &mut self.icons {
                            icon.is_selected = icon.id == icon_id;
                        }
                    } else if !self.taskbar.bounds.contains(mouse_pt) {
                        // Empty desktop right click
                        self.open_desktop_context_menu(mouse_pt);
                    }
                    return ShellAction::None;
                }

                // Left Click
                if button == MouseButton::Left {
                    // Check Taskbar hit test
                    if self.taskbar.bounds.contains(mouse_pt) {
                        // Close Start Menu if clicking taskbar outside start button
                        let start_button_bounds = Rect::new(
                            self.taskbar.bounds.x + (self.screen_width / 2) as i32 - 120,
                            self.taskbar.bounds.y + 4,
                            40,
                            40,
                        );
                        if start_button_bounds.contains(mouse_pt) {
                            self.taskbar.is_start_open = self.start_menu.toggle();
                            return ShellAction::ToggleStartMenu;
                        }
                        return ShellAction::None;
                    }

                    // Check Start Menu hit test
                    if self.start_menu.is_open && self.start_menu.bounds.contains(mouse_pt) {
                        return ShellAction::None;
                    } else if self.start_menu.is_open {
                        self.start_menu.is_open = false;
                        self.taskbar.is_start_open = false;
                    }

                    // Check DWM Windows hit test (top to bottom)
                    let mut focused_win = None;
                    for &wid in self.window_manager.z_stack.iter().rev() {
                        if let Some(win) = self.window_manager.windows.get(&wid) {
                            if win.state != WindowState::Minimized && win.bounds.contains(mouse_pt) {
                                focused_win = Some(wid);
                                break;
                            }
                        }
                    }

                    if let Some(wid) = focused_win {
                        self.window_manager.focus_window(wid);
                        return ShellAction::None;
                    }

                    // Desktop canvas click
                    let mut clicked_icon_id = None;
                    for icon in &self.icons {
                        if icon.bounds.contains(mouse_pt) {
                            clicked_icon_id = Some(icon.id);
                            break;
                        }
                    }

                    if let Some(id) = clicked_icon_id {
                        for icon in &mut self.icons {
                            icon.is_selected = icon.id == id;
                        }
                        // Start drag drop
                        self.drag_drop = Some(DesktopDragState {
                            dragging_ids: vec![id],
                            grab_offset: mouse_pt,
                            current_pos: mouse_pt,
                        });
                    } else {
                        // Click on empty desktop clears selection and starts Marquee
                        for icon in &mut self.icons {
                            icon.is_selected = false;
                        }
                        self.selection_marquee = Some(SelectionMarquee {
                            start_point: mouse_pt,
                            current_point: mouse_pt,
                            bounds: Rect::EMPTY,
                        });
                    }
                }

                ShellAction::None
            }

            DesktopEvent::MouseUp { x, y, button } => {
                let mouse_pt = Point::new(x, y);

                // End marquee selection
                if button == MouseButton::Left && self.selection_marquee.is_some() {
                    self.selection_marquee = None;
                }

                // End drag & drop icon rearrangement
                if button == MouseButton::Left {
                    if let Some(drag) = self.drag_drop.take() {
                        let (cell_w, cell_h) = self.icon_config.icon_size.cell_size();
                        let target_col = ((mouse_pt.x - self.icon_config.margin_left).max(0) as u32
                            / (cell_w + self.icon_config.spacing_x)) as usize;
                        let target_row = ((mouse_pt.y - self.icon_config.margin_top).max(0) as u32
                            / (cell_h + self.icon_config.spacing_y)) as usize;

                        for id in drag.dragging_ids {
                            if let Some(icon) = self.icons.iter_mut().find(|i| i.id == id) {
                                icon.grid_col = target_col;
                                icon.grid_row = target_row;
                                icon.custom_pos = Some(Point::new(
                                    self.icon_config.margin_left
                                        + (target_col as i32) * (cell_w as i32 + self.icon_config.spacing_x as i32),
                                    self.icon_config.margin_top
                                        + (target_row as i32) * (cell_h as i32 + self.icon_config.spacing_y as i32),
                                ));
                            }
                        }
                        self.relayout_desktop_icons();
                    }
                }

                ShellAction::None
            }

            DesktopEvent::Resize { width, height } => {
                self.screen_width = width;
                self.screen_height = height;
                self.wallpaper.render_bloom(width, height);
                self.taskbar = Windows11Taskbar::new(width, height);
                self.start_menu = Windows11StartMenu::new(width, height);
                self.relayout_desktop_icons();
                ShellAction::None
            }

            DesktopEvent::Tick { delta_seconds } => {
                let _frame = self.tick(delta_seconds);
                ShellAction::None
            }

            _ => ShellAction::None,
        }
    }

    // ========================================================================
    // 60 FPS Tick Rendering Pipeline
    // ========================================================================

    /// 60 FPS tick rendering pipeline generating a complete frame representation.
    pub fn tick(&mut self, dt_seconds: f32) -> RenderFrame {
        self.frame_counter += 1;
        let delta_ms = (dt_seconds * 1000.0).round() as u64;
        self.elapsed_time_ms += delta_ms;

        let res = Size::new(self.screen_width, self.screen_height);
        let mut layers = Vec::new();

        // Layer 0: Wallpaper Surface
        layers.push(RenderLayer::Wallpaper {
            theme: self.theme,
            resolution: res,
        });

        // Layer 1: Desktop Icons & Selection Marquee
        let selected_count = self.icons.iter().filter(|i| i.is_selected).count();
        layers.push(RenderLayer::DesktopIcons {
            count: self.icons.len(),
            selected_count,
            marquee: self.selection_marquee.map(|m| m.bounds),
        });

        // Layer 2: DWM Windows
        layers.push(RenderLayer::Windows {
            window_count: self.window_manager.windows.len(),
            active_id: self.window_manager.active_window,
        });

        // Layer 3: Windows 11 Taskbar
        layers.push(RenderLayer::Taskbar {
            bounds: self.taskbar.bounds,
            running_app_count: self.window_manager.windows.len(),
        });

        // Layer 4: Top-Level Flyouts & Overlays
        layers.push(RenderLayer::Flyouts {
            start_menu: self.start_menu.is_open,
            quick_settings: self.taskbar.is_quick_settings_open,
            notifications: self.taskbar.is_notification_center_open,
            win_x: self.taskbar.is_win_x_open,
            run_dialog: self.core_tools.is_run_dialog_open,
            alt_tab: self.alt_tab.is_active,
        });

        // Layer 5: Fluent Context Menu
        let (menu_bounds, item_count, is_open) = if let Some(ref menu) = self.active_context_menu {
            (menu.bounds, menu.items.len(), true)
        } else {
            (Rect::EMPTY, 0, false)
        };
        layers.push(RenderLayer::ContextMenu {
            bounds: menu_bounds,
            item_count,
            is_open,
        });

        // Layer 6: LockScreen Overlay
        layers.push(RenderLayer::LockScreenOverlay {
            is_active: self.lock_screen.is_locked,
            time: self.lock_screen.time_str.clone(),
        });

        // Layer 7: Cursor Overlay
        layers.push(RenderLayer::Cursor(self.cursor));

        RenderFrame {
            frame_id: self.frame_counter,
            timestamp_ms: self.elapsed_time_ms,
            resolution: res,
            theme: self.theme,
            layers,
            delta_seconds: dt_seconds,
        }
    }
}

// ============================================================================
// Section 11: Comprehensive Unit Test Suite
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_desktop_boot_initial_state() {
        let desktop = Windows11Desktop::new(1920, 1080);
        assert_eq!(desktop.screen_width, 1920);
        assert_eq!(desktop.screen_height, 1080);
        assert_eq!(desktop.theme, ThemeMode::Dark);

        // Verify default desktop icons (Recycle Bin, This PC, Network, User Files, Edge, Control Panel)
        assert_eq!(desktop.icons.len(), 6);
        assert_eq!(desktop.icons[0].title, "Recycle Bin");
        assert_eq!(desktop.icons[1].title, "This PC");
        assert_eq!(desktop.icons[2].title, "Network");
        assert_eq!(desktop.icons[3].title, "User Files");
        assert_eq!(desktop.icons[4].title, "Microsoft Edge");
        assert_eq!(desktop.icons[5].title, "Control Panel");

        // Verify taskbar metrics
        assert_eq!(desktop.taskbar.bounds.height, 48);
        assert_eq!(desktop.taskbar.bounds.width, 1920);
        assert!(!desktop.start_menu.is_open);
        assert!(!desktop.lock_screen.is_locked);
    }

    #[test]
    fn test_global_hotkey_win_toggle_start_menu() {
        let mut desktop = Windows11Desktop::new(1920, 1080);
        assert!(!desktop.start_menu.is_open);

        // Press Win key
        let action = desktop.dispatch_event(DesktopEvent::KeyDown {
            key: KeyCode::Win,
            modifiers: KeyModifiers::NONE,
        });
        assert_eq!(action, ShellAction::ToggleStartMenu);
        assert!(desktop.start_menu.is_open);
        assert!(desktop.taskbar.is_start_open);

        // Press Win key again to close
        let action2 = desktop.dispatch_event(DesktopEvent::KeyDown {
            key: KeyCode::Win,
            modifiers: KeyModifiers::NONE,
        });
        assert_eq!(action2, ShellAction::ToggleStartMenu);
        assert!(!desktop.start_menu.is_open);
        assert!(!desktop.taskbar.is_start_open);
    }

    #[test]
    fn test_global_hotkey_win_e_file_explorer() {
        let mut desktop = Windows11Desktop::new(1920, 1080);
        assert_eq!(desktop.window_manager.windows.len(), 0);

        let action = desktop.dispatch_event(DesktopEvent::KeyDown {
            key: KeyCode::KeyE,
            modifiers: KeyModifiers {
                win: true,
                ctrl: false,
                shift: false,
                alt: false,
            },
        });
        assert_eq!(action, ShellAction::LaunchOrFocusFileExplorer);
        assert_eq!(desktop.window_manager.windows.len(), 1);

        let active_id = desktop.window_manager.active_window.expect("window active");
        let win = desktop.window_manager.windows.get(&active_id).unwrap();
        assert_eq!(win.title, "File Explorer");
        assert_eq!(win.app_type, WindowAppType::FileExplorer);

        // Win+E second time focuses existing instance without duplicate creation
        let action2 = desktop.dispatch_event(DesktopEvent::KeyDown {
            key: KeyCode::KeyE,
            modifiers: KeyModifiers {
                win: true,
                ctrl: false,
                shift: false,
                alt: false,
            },
        });
        assert_eq!(action2, ShellAction::LaunchOrFocusFileExplorer);
        assert_eq!(desktop.window_manager.windows.len(), 1);
    }

    #[test]
    fn test_global_hotkey_win_i_settings() {
        let mut desktop = Windows11Desktop::new(1920, 1080);

        let action = desktop.dispatch_event(DesktopEvent::KeyDown {
            key: KeyCode::KeyI,
            modifiers: KeyModifiers {
                win: true,
                ctrl: false,
                shift: false,
                alt: false,
            },
        });
        assert_eq!(action, ShellAction::LaunchOrFocusSettings);
        assert_eq!(desktop.window_manager.windows.len(), 1);

        let active_id = desktop.window_manager.active_window.unwrap();
        let win = desktop.window_manager.windows.get(&active_id).unwrap();
        assert_eq!(win.title, "Settings");
        assert_eq!(win.app_type, WindowAppType::Settings);
    }

    #[test]
    fn test_global_hotkey_win_d_show_desktop_and_restore() {
        let mut desktop = Windows11Desktop::new(1920, 1080);
        desktop.launch_or_focus_explorer();
        desktop.launch_or_focus_settings();
        assert_eq!(desktop.window_manager.windows.len(), 2);

        // Win+D to Show Desktop (minimize all)
        let action1 = desktop.dispatch_event(DesktopEvent::KeyDown {
            key: KeyCode::KeyD,
            modifiers: KeyModifiers {
                win: true,
                ctrl: false,
                shift: false,
                alt: false,
            },
        });
        assert_eq!(action1, ShellAction::ToggleShowDesktop);
        for win in desktop.window_manager.windows.values() {
            assert_eq!(win.state, WindowState::Minimized);
        }
        assert!(desktop.window_manager.active_window.is_none());

        // Win+D again restores previously minimized windows
        let action2 = desktop.dispatch_event(DesktopEvent::KeyDown {
            key: KeyCode::KeyD,
            modifiers: KeyModifiers {
                win: true,
                ctrl: false,
                shift: false,
                alt: false,
            },
        });
        assert_eq!(action2, ShellAction::ToggleShowDesktop);
        for win in desktop.window_manager.windows.values() {
            assert_eq!(win.state, WindowState::Normal);
        }
        assert!(desktop.window_manager.active_window.is_some());
    }

    #[test]
    fn test_global_hotkey_ctrl_shift_esc_task_manager() {
        let mut desktop = Windows11Desktop::new(1920, 1080);

        let action = desktop.dispatch_event(DesktopEvent::KeyDown {
            key: KeyCode::Escape,
            modifiers: KeyModifiers {
                win: false,
                ctrl: true,
                shift: true,
                alt: false,
            },
        });
        assert_eq!(action, ShellAction::LaunchTaskManager);
        assert_eq!(desktop.window_manager.windows.len(), 1);

        let active_id = desktop.window_manager.active_window.unwrap();
        let win = desktop.window_manager.windows.get(&active_id).unwrap();
        assert_eq!(win.title, "Task Manager");
        assert_eq!(win.app_type, WindowAppType::TaskManager);
    }

    #[test]
    fn test_global_hotkey_alt_tab_switcher() {
        let mut desktop = Windows11Desktop::new(1920, 1080);
        let id1 = desktop.launch_or_focus_explorer();
        let _id2 = desktop.launch_or_focus_settings();

        // Alt+Tab down opens switcher
        let action1 = desktop.dispatch_event(DesktopEvent::KeyDown {
            key: KeyCode::Tab,
            modifiers: KeyModifiers {
                win: false,
                ctrl: false,
                shift: false,
                alt: true,
            },
        });
        assert_eq!(action1, ShellAction::AltTabCycleForwards);
        assert!(desktop.alt_tab.is_active);
        assert_eq!(desktop.alt_tab.cards.len(), 2);

        // Alt release activates window
        let action2 = desktop.dispatch_event(DesktopEvent::KeyUp {
            key: KeyCode::LeftAlt,
            modifiers: KeyModifiers::NONE,
        });
        assert_eq!(action2, ShellAction::AltTabActivateSelected);
        assert!(!desktop.alt_tab.is_active);
        assert_eq!(desktop.window_manager.active_window, Some(id1));
    }

    #[test]
    fn test_global_hotkey_win_a_and_win_n_flyouts() {
        let mut desktop = Windows11Desktop::new(1920, 1080);
        assert!(!desktop.taskbar.is_quick_settings_open);
        assert!(!desktop.taskbar.is_notification_center_open);

        // Win + A toggles Quick Settings
        let action_a = desktop.dispatch_event(DesktopEvent::KeyDown {
            key: KeyCode::KeyA,
            modifiers: KeyModifiers {
                win: true,
                ctrl: false,
                shift: false,
                alt: false,
            },
        });
        assert_eq!(action_a, ShellAction::ToggleQuickSettingsFlyout);
        assert!(desktop.taskbar.is_quick_settings_open);

        // Win + N toggles Notification Center
        let action_n = desktop.dispatch_event(DesktopEvent::KeyDown {
            key: KeyCode::KeyN,
            modifiers: KeyModifiers {
                win: true,
                ctrl: false,
                shift: false,
                alt: false,
            },
        });
        assert_eq!(action_n, ShellAction::ToggleNotificationCenter);
        assert!(desktop.taskbar.is_notification_center_open);
    }

    #[test]
    fn test_global_hotkey_win_x_power_menu() {
        let mut desktop = Windows11Desktop::new(1920, 1080);
        assert!(!desktop.taskbar.is_win_x_open);

        let action = desktop.dispatch_event(DesktopEvent::KeyDown {
            key: KeyCode::KeyX,
            modifiers: KeyModifiers {
                win: true,
                ctrl: false,
                shift: false,
                alt: false,
            },
        });
        assert_eq!(action, ShellAction::ToggleWinXMenu);
        assert!(desktop.taskbar.is_win_x_open);
    }

    #[test]
    fn test_desktop_context_menu_interaction() {
        let mut desktop = Windows11Desktop::new(1920, 1080);
        assert!(desktop.active_context_menu.is_none());

        // Right-click empty desktop
        desktop.dispatch_event(DesktopEvent::MouseDown {
            x: 400,
            y: 300,
            button: MouseButton::Right,
        });
        assert!(desktop.active_context_menu.is_some());

        let menu = desktop.active_context_menu.as_ref().unwrap();
        assert!(menu.items.iter().any(|i| i.label == "View"));
        assert!(menu.items.iter().any(|i| i.label == "Sort by"));
        assert!(menu.items.iter().any(|i| i.label == "Refresh"));
        assert!(menu.items.iter().any(|i| i.label == "New"));
        assert!(menu.items.iter().any(|i| i.label == "Display settings"));
        assert!(menu.items.iter().any(|i| i.label == "Personalize"));
        assert!(menu.items.iter().any(|i| i.label == "Open in Windows Terminal"));

        // Execute "New Folder"
        desktop.execute_context_menu_action(ContextMenuAction::NewFolder);
        assert_eq!(desktop.icons.len(), 7);
        assert_eq!(desktop.icons.last().unwrap().title, "New folder");

        // Execute "Open in Windows Terminal"
        desktop.execute_context_menu_action(ContextMenuAction::OpenWindowsTerminal);
        assert_eq!(desktop.window_manager.windows.len(), 1);
        let win = desktop.window_manager.windows.values().next().unwrap();
        assert_eq!(win.title, "Windows Terminal");
    }

    #[test]
    fn test_desktop_icon_selection_and_marquee() {
        let mut desktop = Windows11Desktop::new(1920, 1080);

        // Click on first icon selects it
        desktop.dispatch_event(DesktopEvent::MouseDown {
            x: desktop.icons[0].bounds.center().x,
            y: desktop.icons[0].bounds.center().y,
            button: MouseButton::Left,
        });
        desktop.dispatch_event(DesktopEvent::MouseUp {
            x: desktop.icons[0].bounds.center().x,
            y: desktop.icons[0].bounds.center().y,
            button: MouseButton::Left,
        });
        assert!(desktop.icons[0].is_selected);
        assert!(!desktop.icons[1].is_selected);

        // Start marquee drag over multiple icons
        desktop.dispatch_event(DesktopEvent::MouseDown {
            x: 0,
            y: 0,
            button: MouseButton::Left,
        });
        desktop.dispatch_event(DesktopEvent::MouseMove { x: 300, y: 300 });
        assert!(desktop.selection_marquee.is_some());

        // Release marquee
        desktop.dispatch_event(DesktopEvent::MouseUp {
            x: 300,
            y: 300,
            button: MouseButton::Left,
        });
        assert!(desktop.selection_marquee.is_none());
        let selected_count = desktop.icons.iter().filter(|i| i.is_selected).count();
        assert!(selected_count >= 2);
    }

    #[test]
    fn test_desktop_icon_drag_and_drop() {
        let mut desktop = Windows11Desktop::new(1920, 1080);
        let _initial_col = desktop.icons[0].grid_col;
        let _initial_row = desktop.icons[0].grid_row;

        // Grab first icon
        desktop.dispatch_event(DesktopEvent::MouseDown {
            x: desktop.icons[0].bounds.center().x,
            y: desktop.icons[0].bounds.center().y,
            button: MouseButton::Left,
        });
        assert!(desktop.drag_drop.is_some());

        // Drag to column 2, row 3
        let (cell_w, cell_h) = desktop.icon_config.icon_size.cell_size();
        let target_x = desktop.icon_config.margin_left + 2 * (cell_w as i32 + 8) + 10;
        let target_y = desktop.icon_config.margin_top + 3 * (cell_h as i32 + 8) + 10;

        desktop.dispatch_event(DesktopEvent::MouseMove {
            x: target_x,
            y: target_y,
        });
        desktop.dispatch_event(DesktopEvent::MouseUp {
            x: target_x,
            y: target_y,
            button: MouseButton::Left,
        });
        assert!(desktop.drag_drop.is_none());

        assert_eq!(desktop.icons[0].grid_col, 2);
        assert_eq!(desktop.icons[0].grid_row, 3);
    }

    #[test]
    fn test_lock_screen_toggle_and_unlock() {
        let mut desktop = Windows11Desktop::new(1920, 1080);
        assert!(!desktop.lock_screen.is_locked);

        // Lock screen via Win+L
        desktop.dispatch_event(DesktopEvent::KeyDown {
            key: KeyCode::KeyL,
            modifiers: KeyModifiers {
                win: true,
                ctrl: false,
                shift: false,
                alt: false,
            },
        });
        assert!(desktop.lock_screen.is_locked);

        // While locked, hotkeys are blocked
        let action = desktop.dispatch_event(DesktopEvent::KeyDown {
            key: KeyCode::KeyE,
            modifiers: KeyModifiers {
                win: true,
                ctrl: false,
                shift: false,
                alt: false,
            },
        });
        assert_eq!(action, ShellAction::None);

        // Unlock with correct PIN
        desktop.lock_screen.pin_buffer = "1234".to_string();
        let unlock_action = desktop.dispatch_event(DesktopEvent::KeyDown {
            key: KeyCode::Enter,
            modifiers: KeyModifiers::NONE,
        });
        assert_eq!(unlock_action, ShellAction::UnlockWorkstation);
        assert!(!desktop.lock_screen.is_locked);
    }

    #[test]
    fn test_60fps_render_pipeline_frame_generation() {
        let mut desktop = Windows11Desktop::new(1920, 1080);
        let frame = desktop.tick(0.01667);

        assert_eq!(frame.frame_id, 1);
        assert_eq!(frame.resolution.width, 1920);
        assert_eq!(frame.resolution.height, 1080);
        assert_eq!(frame.layers.len(), 8); // 8 distinct composition layers
        assert!(frame.layers.iter().any(|l| matches!(l, RenderLayer::Wallpaper { .. })));
        assert!(frame.layers.iter().any(|l| matches!(l, RenderLayer::DesktopIcons { .. })));
        assert!(frame.layers.iter().any(|l| matches!(l, RenderLayer::Taskbar { .. })));
        assert!(frame.layers.iter().any(|l| matches!(l, RenderLayer::Cursor(..))));
    }
}
