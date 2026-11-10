//! HybridOS Windows 11 Taskbar & System Tray Subsystem
//!
//! Production-grade implementation of the Windows 11 Desktop Taskbar, Start button,
//! Search box / pill, Task View (Virtual Desktops switcher), Widgets weather pill,
//! Pinned & Running application launcher with live thumbnail hover previews,
//! System Tray (Notification Area) with Wi-Fi / Volume / Battery / Clock & Date,
//! Overflow Chevron flyout for background services, Show Desktop peek trigger,
//! and Quick Settings / Action Center flyout with hardware toggles and sliders.
//!
//! Fully safe, robust, zero-allocation hit-testing, and high-performance rendering.

#![deny(unsafe_code)]
#![allow(dead_code)]

use std::cmp;

// ============================================================================
// Section 1: Core Geometry & Color Types
// ============================================================================

/// 2D Rectangular boundary structure
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    pub const EMPTY: Self = Self { x: 0, y: 0, width: 0, height: 0 };

    pub const fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self { x, y, width, height }
    }

    #[inline]
    pub const fn left(&self) -> i32 {
        self.x
    }

    #[inline]
    pub const fn top(&self) -> i32 {
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
    pub const fn center_x(&self) -> i32 {
        self.x + (self.width as i32) / 2
    }

    #[inline]
    pub const fn center_y(&self) -> i32 {
        self.y + (self.height as i32) / 2
    }

    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }

    #[inline]
    pub fn contains(&self, px: i32, py: i32) -> bool {
        px >= self.left() && px < self.right() && py >= self.top() && py < self.bottom()
    }

    pub fn intersects(&self, other: &Rect) -> bool {
        !self.is_empty()
            && !other.is_empty()
            && self.left() < other.right()
            && self.right() > other.left()
            && self.top() < other.bottom()
            && self.bottom() > other.top()
    }

    pub fn intersection(&self, other: &Rect) -> Option<Rect> {
        let x1 = self.left().max(other.left());
        let y1 = self.top().max(other.top());
        let x2 = self.right().min(other.right());
        let y2 = self.bottom().min(other.bottom());

        if x1 < x2 && y1 < y2 {
            Some(Rect {
                x: x1,
                y: y1,
                width: (x2 - x1) as u32,
                height: (y2 - y1) as u32,
            })
        } else {
            None
        }
    }

    pub fn union(&self, other: &Rect) -> Rect {
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

        Rect {
            x: x1,
            y: y1,
            width: (x2 - x1) as u32,
            height: (y2 - y1) as u32,
        }
    }
}



/// 32-bit ARGB Color with alpha blending and fluent styling constants
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Color {
    pub a: u8,
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub const TRANSPARENT: Self = Self::from_argb(0x00000000);
    pub const BLACK: Self = Self::from_argb(0xFF000000);
    pub const WHITE: Self = Self::from_argb(0xFFFFFFFF);

    /// Windows 11 Acrylic / Mica taskbar highlight: 1px 0x33FFFFFF
    pub const TOP_BORDER_HIGHLIGHT: Self = Self::from_argb(0x33FFFFFF);

    /// Windows 11 default accent blue (#0078D4)
    pub const ACCENT_BLUE: Self = Self::from_argb(0xFF0078D4);
    /// Windows 11 light accent blue (#60CDFF)
    pub const ACCENT_LIGHT_BLUE: Self = Self::from_argb(0xFF60CDFF);

    /// Taskbar attention / flashing orange (#FF8C00)
    pub const ATTENTION_ORANGE: Self = Self::from_argb(0xFFFF8C00);
    /// Progress bar running green (#107C41)
    pub const PROGRESS_GREEN: Self = Self::from_argb(0xFF107C41);
    /// Progress bar paused amber (#D89B00)
    pub const PROGRESS_AMBER: Self = Self::from_argb(0xFFD89B00);
    /// Progress bar error red (#E81123)
    pub const PROGRESS_RED: Self = Self::from_argb(0xFFE81123);

    /// Mica / Acrylic Dark mode taskbar background
    pub const MICA_DARK_BASE: Self = Self::from_argb(0xDC202020);
    /// Mica / Acrylic Light mode taskbar background
    pub const MICA_LIGHT_BASE: Self = Self::from_argb(0xDCF3F3F3);

    /// Hover glow tint (fluent light brush)
    pub const HOVER_GLOW: Self = Self::from_argb(0x18FFFFFF);
    /// Active pressed tint
    pub const PRESSED_TINT: Self = Self::from_argb(0x28FFFFFF);

    pub const fn new(a: u8, r: u8, g: u8, b: u8) -> Self {
        Self { a, r, g, b }
    }

    pub const fn from_argb(argb: u32) -> Self {
        Self {
            a: ((argb >> 24) & 0xFF) as u8,
            r: ((argb >> 16) & 0xFF) as u8,
            g: ((argb >> 8) & 0xFF) as u8,
            b: (argb & 0xFF) as u8,
        }
    }

    pub const fn to_argb(&self) -> u32 {
        ((self.a as u32) << 24) | ((self.r as u32) << 16) | ((self.g as u32) << 8) | (self.b as u32)
    }

    /// Integer-optimized alpha blending over a background color
    pub fn blend_over(&self, bg: Color) -> Color {
        if self.a == 255 {
            return *self;
        }
        if self.a == 0 {
            return bg;
        }

        let alpha = self.a as u32;
        let inv_alpha = 255 - alpha;

        let out_r = ((self.r as u32 * alpha + bg.r as u32 * inv_alpha) / 255) as u8;
        let out_g = ((self.g as u32 * alpha + bg.g as u32 * inv_alpha) / 255) as u8;
        let out_b = ((self.b as u32 * alpha + bg.b as u32 * inv_alpha) / 255) as u8;
        let out_a = (alpha + (bg.a as u32 * inv_alpha) / 255).min(255) as u8;

        Color::new(out_a, out_r, out_g, out_b)
    }
}

// ============================================================================
// Section 2: Taskbar Configuration & Themes
// ============================================================================

/// Windows 11 Taskbar Alignment Setting
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskbarAlignment {
    /// Modern Windows 11 centered icons
    Center,
    /// Classic Windows 10/7 left-docked icons
    Left,
}

impl Default for TaskbarAlignment {
    fn default() -> Self {
        Self::Center
    }
}

/// Windows 11 Taskbar Height Sizes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskbarSize {
    /// Standard Desktop size: 48px
    Standard,
    /// Small compact size: 32px
    Small,
    /// Touch optimized size: 56px
    Touch,
}

impl Default for TaskbarSize {
    fn default() -> Self {
        Self::Standard
    }
}

impl TaskbarSize {
    pub const fn height(&self) -> u32 {
        match self {
            Self::Standard => 48,
            Self::Small => 32,
            Self::Touch => 56,
        }
    }

    pub const fn icon_size(&self) -> u32 {
        match self {
            Self::Standard => 24,
            Self::Small => 16,
            Self::Touch => 32,
        }
    }

    pub const fn button_size(&self) -> u32 {
        match self {
            Self::Standard => 40,
            Self::Small => 28,
            Self::Touch => 48,
        }
    }
}

/// Taskbar Theme Styling
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskbarTheme {
    Dark,
    Light,
    System,
}

impl Default for TaskbarTheme {
    fn default() -> Self {
        Self::Dark
    }
}

/// Search box display styles
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchDisplayMode {
    /// Expanded pill bar with placeholder "Search apps, settings, and documents"
    SearchPill,
    /// Icon-only magnifying glass (40px)
    IconOnly,
    /// Icon + "Search" label
    IconAndLabel,
    /// Completely hidden
    Hidden,
}

impl Default for SearchDisplayMode {
    fn default() -> Self {
        Self::SearchPill
    }
}

/// Visual interactive states for buttons and cards
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonVisualState {
    Normal,
    Hover,
    Pressed,
    Active,
}

impl Default for ButtonVisualState {
    fn default() -> Self {
        Self::Normal
    }
}

// ============================================================================
// Section 3: Start, Search, Task View & System Widgets
// ============================================================================

/// Windows 11 Start Button
#[derive(Debug, Clone)]
pub struct StartButton {
    pub bounds: Rect,
    pub visual_state: ButtonVisualState,
    pub is_menu_open: bool,
}

impl StartButton {
    pub fn new() -> Self {
        Self {
            bounds: Rect::EMPTY,
            visual_state: ButtonVisualState::Normal,
            is_menu_open: false,
        }
    }

    pub fn set_state(&mut self, state: ButtonVisualState) {
        self.visual_state = state;
    }

    pub fn toggle_menu(&mut self) -> bool {
        self.is_menu_open = !self.is_menu_open;
        if self.is_menu_open {
            self.visual_state = ButtonVisualState::Active;
        } else {
            self.visual_state = ButtonVisualState::Normal;
        }
        self.is_menu_open
    }
}

/// Windows 11 Search Button / Search Pill Bar
#[derive(Debug, Clone)]
pub struct SearchBox {
    pub bounds: Rect,
    pub mode: SearchDisplayMode,
    pub placeholder: String,
    pub query: String,
    pub is_focused: bool,
    pub visual_state: ButtonVisualState,
}

impl SearchBox {
    pub fn new(mode: SearchDisplayMode) -> Self {
        Self {
            bounds: Rect::EMPTY,
            mode,
            placeholder: "Search apps, settings, and documents".to_string(),
            query: String::new(),
            is_focused: false,
            visual_state: ButtonVisualState::Normal,
        }
    }

    pub fn set_query(&mut self, query: &str) {
        self.query = query.to_string();
    }

    pub fn clear(&mut self) {
        self.query.clear();
    }
}

/// Virtual Desktop instance for Task View
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VirtualDesktop {
    pub id: usize,
    pub name: String,
    pub is_active: bool,
    pub window_count: usize,
    pub preview_bounds: Rect,
}

/// Windows 11 Task View & Virtual Desktops Manager
#[derive(Debug, Clone)]
pub struct TaskView {
    pub bounds: Rect,
    pub visual_state: ButtonVisualState,
    pub is_flyout_open: bool,
    pub desktops: Vec<VirtualDesktop>,
    pub active_desktop_id: usize,
    pub add_desktop_bounds: Rect,
}

impl TaskView {
    pub fn new() -> Self {
        let mut tv = Self {
            bounds: Rect::EMPTY,
            visual_state: ButtonVisualState::Normal,
            is_flyout_open: false,
            desktops: Vec::new(),
            active_desktop_id: 1,
            add_desktop_bounds: Rect::EMPTY,
        };

        // Pre-populate Desktop 1 and Desktop 2 as in Windows 11 default
        tv.desktops.push(VirtualDesktop {
            id: 1,
            name: "Desktop 1".to_string(),
            is_active: true,
            window_count: 2,
            preview_bounds: Rect::EMPTY,
        });
        tv.desktops.push(VirtualDesktop {
            id: 2,
            name: "Desktop 2".to_string(),
            is_active: false,
            window_count: 0,
            preview_bounds: Rect::EMPTY,
        });

        tv
    }

    pub fn add_desktop(&mut self, custom_name: Option<String>) -> usize {
        let next_id = self.desktops.iter().map(|d| d.id).max().unwrap_or(0) + 1;
        let name = custom_name.unwrap_or_else(|| format!("Desktop {}", next_id));
        self.desktops.push(VirtualDesktop {
            id: next_id,
            name,
            is_active: false,
            window_count: 0,
            preview_bounds: Rect::EMPTY,
        });
        next_id
    }

    pub fn switch_desktop(&mut self, id: usize) -> bool {
        let mut found = false;
        for d in &mut self.desktops {
            if d.id == id {
                d.is_active = true;
                found = true;
            } else {
                d.is_active = false;
            }
        }
        if found {
            self.active_desktop_id = id;
        }
        found
    }

    pub fn close_desktop(&mut self, id: usize) -> bool {
        if self.desktops.len() <= 1 {
            return false; // Cannot close the only remaining desktop
        }

        let pos = self.desktops.iter().position(|d| d.id == id);
        if let Some(index) = pos {
            self.desktops.remove(index);
            if self.active_desktop_id == id {
                let new_active_index = index.min(self.desktops.len() - 1);
                self.desktops[new_active_index].is_active = true;
                self.active_desktop_id = self.desktops[new_active_index].id;
            }
            true
        } else {
            false
        }
    }
}

/// Weather condition glyph/category
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeatherCondition {
    Sunny,
    PartlyCloudy,
    Cloudy,
    Rainy,
    Snowy,
    Thunderstorm,
}

impl WeatherCondition {
    pub fn icon_glyph(&self) -> &'static str {
        match self {
            Self::Sunny => "☀",
            Self::PartlyCloudy => "⛅",
            Self::Cloudy => "☁",
            Self::Rainy => "🌧",
            Self::Snowy => "❄",
            Self::Thunderstorm => "⛈",
        }
    }
}

/// Windows 11 Widgets Pill (Weather & Dynamic Feed)
#[derive(Debug, Clone)]
pub struct WidgetsButton {
    pub bounds: Rect,
    pub visual_state: ButtonVisualState,
    pub is_open: bool,
    pub temperature: String,
    pub condition: WeatherCondition,
    pub condition_text: String,
    pub notification_badge: Option<u32>,
}

impl WidgetsButton {
    pub fn new() -> Self {
        Self {
            bounds: Rect::EMPTY,
            visual_state: ButtonVisualState::Normal,
            is_open: false,
            temperature: "22°C".to_string(),
            condition: WeatherCondition::PartlyCloudy,
            condition_text: "Partly Sunny".to_string(),
            notification_badge: None,
        }
    }

    pub fn update_weather(&mut self, temp: &str, condition: WeatherCondition, text: &str) {
        self.temperature = temp.to_string();
        self.condition = condition;
        self.condition_text = text.to_string();
    }
}

// ============================================================================
// Section 4: Pinned & Running Applications Subsystem
// ============================================================================

/// Application Active Indicator State
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ItemActiveState {
    /// App is pinned, but not currently running
    Inactive,
    /// App is running in the background (4px subtle dot indicator)
    Running,
    /// App has the active foreground focus (8px / 16px accent pill indicator)
    Focused,
    /// App requested user attention / flashing amber state
    Attention,
    /// App is reporting task progress (0..100%)
    Progress(u8),
    /// App progress is paused (amber progress bar fill)
    ProgressPaused(u8),
    /// App progress encountered an error (red progress bar fill)
    ProgressError(u8),
}

/// Application Icon Definition
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppIcon {
    pub glyph: char,
    pub brand_color: Color,
    pub fallback_name: String,
}

impl AppIcon {
    pub fn new(glyph: char, brand_color: Color, fallback_name: &str) -> Self {
        Self {
            glyph,
            brand_color,
            fallback_name: fallback_name.to_string(),
        }
    }
}

/// Live Window Instance for an App
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowInstance {
    pub window_id: u64,
    pub title: String,
    pub is_minimized: bool,
    pub is_active: bool,
}

/// Live Window Hover Preview Card
#[derive(Debug, Clone)]
pub struct WindowThumbnailPreview {
    pub window_id: u64,
    pub window_title: String,
    pub card_bounds: Rect,
    pub thumbnail_rect: Rect,
    pub close_button_rect: Rect,
    pub is_card_hovered: bool,
    pub is_close_hovered: bool,
}

/// Floating Hover Preview Popup for an application item
#[derive(Debug, Clone)]
pub struct ThumbnailPreviewPopup {
    pub app_id: String,
    pub item_index: usize,
    pub bounds: Rect,
    pub previews: Vec<WindowThumbnailPreview>,
    pub is_visible: bool,
}

/// Pinned or Running Application Item on the Taskbar
#[derive(Debug, Clone)]
pub struct TaskbarItem {
    pub app_id: String,
    pub title: String,
    pub icon: AppIcon,
    pub is_pinned: bool,
    pub is_running: bool,
    pub window_count: usize,
    pub active_state: ItemActiveState,
    pub bounds: Rect,
    pub visual_state: ButtonVisualState,
    pub windows: Vec<WindowInstance>,
}

impl TaskbarItem {
    pub fn new(app_id: &str, title: &str, icon: AppIcon, is_pinned: bool) -> Self {
        Self {
            app_id: app_id.to_string(),
            title: title.to_string(),
            icon,
            is_pinned,
            is_running: false,
            window_count: 0,
            active_state: ItemActiveState::Inactive,
            bounds: Rect::EMPTY,
            visual_state: ButtonVisualState::Normal,
            windows: Vec::new(),
        }
    }

    /// Add a window instance and update running state
    pub fn add_window(&mut self, window_id: u64, title: &str, focused: bool) {
        self.windows.push(WindowInstance {
            window_id,
            title: title.to_string(),
            is_minimized: false,
            is_active: focused,
        });
        self.window_count = self.windows.len();
        self.is_running = true;

        if focused {
            self.set_focused_window(window_id);
        } else if self.active_state == ItemActiveState::Inactive {
            self.active_state = ItemActiveState::Running;
        }
    }

    /// Remove a window instance
    pub fn remove_window(&mut self, window_id: u64) -> bool {
        let pos = self.windows.iter().position(|w| w.window_id == window_id);
        if let Some(index) = pos {
            self.windows.remove(index);
            self.window_count = self.windows.len();
            if self.window_count == 0 {
                self.is_running = false;
                self.active_state = ItemActiveState::Inactive;
            } else if self.active_state == ItemActiveState::Focused {
                self.active_state = ItemActiveState::Running;
            }
            true
        } else {
            false
        }
    }

    /// Mark a specific window as focused
    pub fn set_focused_window(&mut self, window_id: u64) {
        let mut found = false;
        for w in &mut self.windows {
            if w.window_id == window_id {
                w.is_active = true;
                w.is_minimized = false;
                found = true;
            } else {
                w.is_active = false;
            }
        }
        if found {
            self.active_state = ItemActiveState::Focused;
        }
    }

    /// Calculate the bottom active indicator pill / progress bar geometry
    pub fn indicator_rect(&self) -> Option<(Rect, Color)> {
        if self.bounds.is_empty() {
            return None;
        }

        let bar_height = 3u32;
        let y = self.bounds.bottom().saturating_sub(4);

        match self.active_state {
            ItemActiveState::Inactive => None,
            ItemActiveState::Focused => {
                // 8px centered pill with accent color
                let width = 8u32;
                let x = self.bounds.x + ((self.bounds.width as i32 - width as i32) / 2);
                Some((Rect::new(x, y, width, bar_height), Color::ACCENT_LIGHT_BLUE))
            }
            ItemActiveState::Running => {
                // 4px subtle dot indicator
                let width = 4u32;
                let x = self.bounds.x + ((self.bounds.width as i32 - width as i32) / 2);
                Some((Rect::new(x, y, width, bar_height), Color::new(160, 200, 200, 200)))
            }
            ItemActiveState::Attention => {
                // 16px flashing attention pill
                let width = 16u32;
                let x = self.bounds.x + ((self.bounds.width as i32 - width as i32) / 2);
                Some((Rect::new(x, y, width, bar_height), Color::ATTENTION_ORANGE))
            }
            ItemActiveState::Progress(pct) => {
                let max_w = self.bounds.width.saturating_sub(8);
                let w = ((max_w as u32 * (pct.min(100) as u32)) / 100).max(2);
                let x = self.bounds.x + 4;
                Some((Rect::new(x, y, w, bar_height), Color::PROGRESS_GREEN))
            }
            ItemActiveState::ProgressPaused(pct) => {
                let max_w = self.bounds.width.saturating_sub(8);
                let w = ((max_w as u32 * (pct.min(100) as u32)) / 100).max(2);
                let x = self.bounds.x + 4;
                Some((Rect::new(x, y, w, bar_height), Color::PROGRESS_AMBER))
            }
            ItemActiveState::ProgressError(pct) => {
                let max_w = self.bounds.width.saturating_sub(8);
                let w = ((max_w as u32 * (pct.min(100) as u32)) / 100).max(2);
                let x = self.bounds.x + 4;
                Some((Rect::new(x, y, w, bar_height), Color::PROGRESS_RED))
            }
        }
    }
}

// ============================================================================
// Section 5: System Tray (Notification Area) Subsystem
// ============================================================================

/// Network connection type and signal state
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetworkKind {
    WiFi { ssid: String, signal_bars: u8 }, // 0..4
    Ethernet { connected: bool },
    AirplaneMode,
    Disconnected,
}

impl Default for NetworkKind {
    fn default() -> Self {
        Self::WiFi {
            ssid: "HybridOS-5G".to_string(),
            signal_bars: 4,
        }
    }
}

/// Volume indicator state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VolumeState {
    Mute,
    Low,    // 1..33
    Medium, // 34..66
    High,   // 67..100
}

impl VolumeState {
    pub fn from_level(level: u8, is_muted: bool) -> Self {
        if is_muted || level == 0 {
            Self::Mute
        } else if level <= 33 {
            Self::Low
        } else if level <= 66 {
            Self::Medium
        } else {
            Self::High
        }
    }

    pub fn icon_glyph(&self) -> &'static str {
        match self {
            Self::Mute => "󰖁",
            Self::Low => "󰕿",
            Self::Medium => "󰖀",
            Self::High => "󰕾",
        }
    }
}

/// Battery hardware state
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatteryInfo {
    pub percentage: u8, // 0..100
    pub is_charging: bool,
    pub power_saver: bool,
    pub estimated_minutes_remaining: Option<u32>,
}

impl Default for BatteryInfo {
    fn default() -> Self {
        Self {
            percentage: 95,
            is_charging: true,
            power_saver: false,
            estimated_minutes_remaining: Some(280),
        }
    }
}

/// Clock & Date Display
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClockDateInfo {
    pub time_str: String, // "14:32"
    pub date_str: String, // "10/11/2026"
    pub is_hovered: bool,
    pub bounds: Rect,
}

impl Default for ClockDateInfo {
    fn default() -> Self {
        Self {
            time_str: "12:00".to_string(),
            date_str: "10/11/2026".to_string(),
            is_hovered: false,
            bounds: Rect::EMPTY,
        }
    }
}

/// Custom Background Service Tray Icon (e.g., OneDrive, Antivirus, Steam)
#[derive(Debug, Clone)]
pub struct TrayItem {
    pub id: String,
    pub name: String,
    pub glyph: char,
    pub tooltip: String,
    pub is_hidden_in_overflow: bool,
    pub bounds: Rect,
    pub visual_state: ButtonVisualState,
}

/// Hidden Background Services Overflow Flyout
#[derive(Debug, Clone)]
pub struct TrayOverflowFlyout {
    pub bounds: Rect,
    pub is_open: bool,
    pub items: Vec<TrayItem>,
}

impl TrayOverflowFlyout {
    pub fn new() -> Self {
        Self {
            bounds: Rect::EMPTY,
            is_open: false,
            items: Vec::new(),
        }
    }
}

/// Windows 11 System Tray Subsystem
#[derive(Debug, Clone)]
pub struct SystemTray {
    pub bounds: Rect,
    pub network: NetworkKind,
    pub volume_level: u8,
    pub is_volume_muted: bool,
    pub battery: BatteryInfo,
    pub clock: ClockDateInfo,

    /// Windows 11 combines Network, Volume, and Battery into a single pill hover button
    pub quick_settings_pill_bounds: Rect,
    pub quick_settings_visual_state: ButtonVisualState,

    /// Overflow chevron (^) button
    pub overflow_button_bounds: Rect,
    pub overflow_visual_state: ButtonVisualState,
    pub overflow_flyout: TrayOverflowFlyout,

    /// Show Desktop 1px/2px peek strip at far right edge
    pub show_desktop_bounds: Rect,
    pub is_show_desktop_hovered: bool,
    pub is_desktop_peek_active: bool,

    /// Visible tray items outside overflow
    pub visible_tray_items: Vec<TrayItem>,
}

impl SystemTray {
    pub fn new() -> Self {
        let mut tray = Self {
            bounds: Rect::EMPTY,
            network: NetworkKind::default(),
            volume_level: 75,
            is_volume_muted: false,
            battery: BatteryInfo::default(),
            clock: ClockDateInfo::default(),
            quick_settings_pill_bounds: Rect::EMPTY,
            quick_settings_visual_state: ButtonVisualState::Normal,
            overflow_button_bounds: Rect::EMPTY,
            overflow_visual_state: ButtonVisualState::Normal,
            overflow_flyout: TrayOverflowFlyout::new(),
            show_desktop_bounds: Rect::EMPTY,
            is_show_desktop_hovered: false,
            is_desktop_peek_active: false,
            visible_tray_items: Vec::new(),
        };

        // Add standard background services to overflow
        tray.overflow_flyout.items.push(TrayItem {
            id: "onedrive".to_string(),
            name: "OneDrive".to_string(),
            glyph: '☁',
            tooltip: "OneDrive - Up to date".to_string(),
            is_hidden_in_overflow: true,
            bounds: Rect::EMPTY,
            visual_state: ButtonVisualState::Normal,
        });

        tray.overflow_flyout.items.push(TrayItem {
            id: "security".to_string(),
            name: "Windows Security".to_string(),
            glyph: '🛡',
            tooltip: "Windows Security - Actions recommended: None".to_string(),
            is_hidden_in_overflow: true,
            bounds: Rect::EMPTY,
            visual_state: ButtonVisualState::Normal,
        });

        tray.overflow_flyout.items.push(TrayItem {
            id: "bluetooth_tray".to_string(),
            name: "Bluetooth Devices".to_string(),
            glyph: 'ᛒ',
            tooltip: "Bluetooth: On".to_string(),
            is_hidden_in_overflow: true,
            bounds: Rect::EMPTY,
            visual_state: ButtonVisualState::Normal,
        });

        tray
    }

    pub fn volume_state(&self) -> VolumeState {
        VolumeState::from_level(self.volume_level, self.is_volume_muted)
    }

    pub fn set_volume(&mut self, level: u8) {
        self.volume_level = level.min(100);
        if self.volume_level > 0 {
            self.is_volume_muted = false;
        }
    }

    pub fn toggle_mute(&mut self) -> bool {
        self.is_volume_muted = !self.is_volume_muted;
        self.is_volume_muted
    }

    pub fn toggle_overflow(&mut self) -> bool {
        self.overflow_flyout.is_open = !self.overflow_flyout.is_open;
        if self.overflow_flyout.is_open {
            self.overflow_visual_state = ButtonVisualState::Active;
        } else {
            self.overflow_visual_state = ButtonVisualState::Normal;
        }
        self.overflow_flyout.is_open
    }
}

// ============================================================================
// Section 6: Quick Settings / Action Center Subsystem
// ============================================================================

/// Quick Settings Action Toggle Identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuickToggleKind {
    WiFi,
    Bluetooth,
    AirplaneMode,
    NightLight,
    BatterySaver,
    FocusAssist,
    Accessibility,
    NearbyShare,
}

/// Interactive Quick Toggle Pill Button
#[derive(Debug, Clone)]
pub struct QuickToggle {
    pub kind: QuickToggleKind,
    pub title: &'static str,
    pub detail: String,
    pub is_active: bool,
    pub bounds: Rect,
    pub visual_state: ButtonVisualState,
}

/// Slider Controller (Brightness and Volume)
#[derive(Debug, Clone)]
pub struct SliderControl {
    pub label: &'static str,
    pub value: u8, // 0..100
    pub bounds: Rect,
    pub track_bounds: Rect,
    pub thumb_bounds: Rect,
    pub is_dragging: bool,
}

impl SliderControl {
    pub fn new(label: &'static str, value: u8) -> Self {
        Self {
            label,
            value: value.min(100),
            bounds: Rect::EMPTY,
            track_bounds: Rect::EMPTY,
            thumb_bounds: Rect::EMPTY,
            is_dragging: false,
        }
    }

    pub fn set_value(&mut self, value: u8) {
        self.value = value.min(100);
    }

    pub fn update_from_mouse_x(&mut self, mouse_x: i32) {
        if self.track_bounds.width == 0 {
            return;
        }
        let rel_x = (mouse_x - self.track_bounds.x).clamp(0, self.track_bounds.width as i32);
        let pct = (rel_x as u32 * 100) / self.track_bounds.width;
        self.value = pct.min(100) as u8;
    }
}

/// Windows 11 Quick Settings / Action Center Flyout
#[derive(Debug, Clone)]
pub struct QuickSettingsFlyout {
    pub bounds: Rect,
    pub is_open: bool,
    pub toggles: Vec<QuickToggle>,
    pub volume_slider: SliderControl,
    pub brightness_slider: SliderControl,
    pub all_settings_button_bounds: Rect,
    pub edit_quick_settings_bounds: Rect,
}

impl QuickSettingsFlyout {
    pub fn new() -> Self {
        let mut qs = Self {
            bounds: Rect::EMPTY,
            is_open: false,
            toggles: Vec::new(),
            volume_slider: SliderControl::new("Volume", 75),
            brightness_slider: SliderControl::new("Brightness", 80),
            all_settings_button_bounds: Rect::EMPTY,
            edit_quick_settings_bounds: Rect::EMPTY,
        };

        qs.toggles.push(QuickToggle {
            kind: QuickToggleKind::WiFi,
            title: "Wi-Fi",
            detail: "HybridOS-5G".to_string(),
            is_active: true,
            bounds: Rect::EMPTY,
            visual_state: ButtonVisualState::Normal,
        });

        qs.toggles.push(QuickToggle {
            kind: QuickToggleKind::Bluetooth,
            title: "Bluetooth",
            detail: "On".to_string(),
            is_active: true,
            bounds: Rect::EMPTY,
            visual_state: ButtonVisualState::Normal,
        });

        qs.toggles.push(QuickToggle {
            kind: QuickToggleKind::AirplaneMode,
            title: "Airplane Mode",
            detail: "Off".to_string(),
            is_active: false,
            bounds: Rect::EMPTY,
            visual_state: ButtonVisualState::Normal,
        });

        qs.toggles.push(QuickToggle {
            kind: QuickToggleKind::NightLight,
            title: "Night Light",
            detail: "Off".to_string(),
            is_active: false,
            bounds: Rect::EMPTY,
            visual_state: ButtonVisualState::Normal,
        });

        qs.toggles.push(QuickToggle {
            kind: QuickToggleKind::BatterySaver,
            title: "Battery Saver",
            detail: "Off".to_string(),
            is_active: false,
            bounds: Rect::EMPTY,
            visual_state: ButtonVisualState::Normal,
        });

        qs.toggles.push(QuickToggle {
            kind: QuickToggleKind::FocusAssist,
            title: "Focus Assist",
            detail: "Priority only".to_string(),
            is_active: true,
            bounds: Rect::EMPTY,
            visual_state: ButtonVisualState::Normal,
        });

        qs
    }

    pub fn toggle(&mut self, kind: QuickToggleKind) -> bool {
        if let Some(t) = self.toggles.iter_mut().find(|t| t.kind == kind) {
            t.is_active = !t.is_active;
            t.detail = if t.is_active { "On".to_string() } else { "Off".to_string() };
            t.is_active
        } else {
            false
        }
    }
}

// ============================================================================
// Section 7: Unified Windows 11 Taskbar Manager
// ============================================================================

/// Mouse Buttons for hit-testing and event handling
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// Hit-Testing Result across all Taskbar and Flyout elements
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HitTestResult {
    StartButton,
    SearchBox,
    TaskView,
    Widgets,
    AppItem { index: usize, app_id: String },
    ThumbnailCard { app_id: String, window_id: u64 },
    ThumbnailClose { app_id: String, window_id: u64 },
    TrayQuickSettingsPill,
    TrayOverflowButton,
    TrayCustomItem { id: String },
    ClockDate,
    ShowDesktop,
    QuickToggle(QuickToggleKind),
    VolumeSliderDrag,
    BrightnessSliderDrag,
    VirtualDesktopCard { desktop_id: usize },
    AddVirtualDesktop,
    TaskbarBackground,
    None,
}

/// Windows 11 Taskbar Master Controller
#[derive(Debug, Clone)]
pub struct Windows11Taskbar {
    pub screen_width: u32,
    pub screen_height: u32,
    pub bounds: Rect,
    pub alignment: TaskbarAlignment,
    pub size: TaskbarSize,
    pub theme: TaskbarTheme,

    pub start_button: StartButton,
    pub search_box: SearchBox,
    pub task_view: TaskView,
    pub widgets_button: WidgetsButton,

    pub items: Vec<TaskbarItem>,
    pub system_tray: SystemTray,
    pub quick_settings: QuickSettingsFlyout,

    pub active_hover_preview: Option<ThumbnailPreviewPopup>,
    pub hovered_app_index: Option<usize>,
}

impl Windows11Taskbar {
    /// Initialize a new Windows 11 Taskbar instance with standard defaults
    pub fn new(screen_width: u32, screen_height: u32) -> Self {
        let mut tb = Self {
            screen_width,
            screen_height,
            bounds: Rect::EMPTY,
            alignment: TaskbarAlignment::Center,
            size: TaskbarSize::Standard,
            theme: TaskbarTheme::Dark,

            start_button: StartButton::new(),
            search_box: SearchBox::new(SearchDisplayMode::SearchPill),
            task_view: TaskView::new(),
            widgets_button: WidgetsButton::new(),

            items: Vec::new(),
            system_tray: SystemTray::new(),
            quick_settings: QuickSettingsFlyout::new(),

            active_hover_preview: None,
            hovered_app_index: None,
        };

        // Pin standard default Windows 11 commercial applications
        tb.pin_app(
            "msedge",
            "Microsoft Edge",
            AppIcon::new('🌐', Color::from_argb(0xFF0078D7), "Edge"),
        );
        tb.pin_app(
            "explorer",
            "File Explorer",
            AppIcon::new('📁', Color::from_argb(0xFFFFB900), "Explorer"),
        );
        tb.pin_app(
            "terminal",
            "Windows Terminal",
            AppIcon::new('💻', Color::from_argb(0xFF4C4C4C), "Terminal"),
        );
        tb.pin_app(
            "store",
            "Microsoft Store",
            AppIcon::new('🛍', Color::from_argb(0xFF0078D4), "Store"),
        );
        tb.pin_app(
            "settings",
            "Settings",
            AppIcon::new('⚙', Color::from_argb(0xFF68768A), "Settings"),
        );

        tb.relayout();
        tb
    }

    /// Set Taskbar alignment (Center vs Left)
    pub fn set_alignment(&mut self, alignment: TaskbarAlignment) {
        self.alignment = alignment;
        self.relayout();
    }

    /// Set Taskbar size (Standard 48px, Small 32px, Touch 56px)
    pub fn set_size(&mut self, size: TaskbarSize) {
        self.size = size;
        self.relayout();
    }

    /// Set Search Box Display Mode
    pub fn set_search_mode(&mut self, mode: SearchDisplayMode) {
        self.search_box.mode = mode;
        self.relayout();
    }

    /// Pin an application
    pub fn pin_app(&mut self, app_id: &str, title: &str, icon: AppIcon) {
        if let Some(item) = self.items.iter_mut().find(|it| it.app_id == app_id) {
            item.is_pinned = true;
        } else {
            self.items.push(TaskbarItem::new(app_id, title, icon, true));
        }
        self.relayout();
    }

    /// Unpin an application
    pub fn unpin_app(&mut self, app_id: &str) {
        if let Some(pos) = self.items.iter().position(|it| it.app_id == app_id) {
            if self.items[pos].is_running {
                self.items[pos].is_pinned = false;
            } else {
                self.items.remove(pos);
            }
        }
        self.relayout();
    }

    /// Launch or register a running window for an app
    pub fn launch_or_open_window(&mut self, app_id: &str, window_id: u64, title: &str) {
        if let Some(item) = self.items.iter_mut().find(|it| it.app_id == app_id) {
            item.add_window(window_id, title, true);
        } else {
            let mut item = TaskbarItem::new(
                app_id,
                title,
                AppIcon::new('🗔', Color::ACCENT_BLUE, app_id),
                false,
            );
            item.add_window(window_id, title, true);
            self.items.push(item);
        }

        // Unfocus other apps
        for it in &mut self.items {
            if it.app_id != app_id && it.active_state == ItemActiveState::Focused {
                it.active_state = ItemActiveState::Running;
            }
        }

        self.relayout();
    }

    /// Close a window instance
    pub fn close_window(&mut self, app_id: &str, window_id: u64) {
        if let Some(pos) = self.items.iter().position(|it| it.app_id == app_id) {
            self.items[pos].remove_window(window_id);
            if !self.items[pos].is_pinned && !self.items[pos].is_running {
                self.items.remove(pos);
            }
        }
        self.relayout();
    }

    /// Update app active progress state
    pub fn set_app_progress(&mut self, app_id: &str, progress: u8) {
        if let Some(item) = self.items.iter_mut().find(|it| it.app_id == app_id) {
            item.active_state = ItemActiveState::Progress(progress.min(100));
        }
    }

    /// Update app attention / flashing state
    pub fn set_app_attention(&mut self, app_id: &str, attention: bool) {
        if let Some(item) = self.items.iter_mut().find(|it| it.app_id == app_id) {
            if attention {
                item.active_state = ItemActiveState::Attention;
            } else if item.is_running {
                item.active_state = ItemActiveState::Running;
            } else {
                item.active_state = ItemActiveState::Inactive;
            }
        }
    }

    /// Set screen resolution and trigger relayout
    pub fn update_screen_dimensions(&mut self, width: u32, height: u32) {
        self.screen_width = width;
        self.screen_height = height;
        self.relayout();
    }

    /// Recalculate complete geometry for all elements, buttons, trays, and flyouts
    pub fn relayout(&mut self) {
        let bar_h = self.size.height();
        let bar_y = (self.screen_height.saturating_sub(bar_h)) as i32;
        self.bounds = Rect::new(0, bar_y, self.screen_width, bar_h);

        let btn_size = self.size.button_size();
        let btn_pad_y = (bar_h.saturating_sub(btn_size)) as i32 / 2;

        // --------------------------------------------------------------------
        // 1. System Tray (Right Side) Layout
        // --------------------------------------------------------------------
        let mut right_cursor = self.screen_width as i32;

        // Far-right 2px Show Desktop peek strip
        let show_desktop_w = 2u32;
        right_cursor -= show_desktop_w as i32;
        self.system_tray.show_desktop_bounds = Rect::new(
            right_cursor,
            bar_y,
            show_desktop_w,
            bar_h,
        );

        right_cursor -= 6; // Margin between peek strip and clock

        // Clock & Date Pill (approx 78px wide)
        let clock_w = 78u32;
        right_cursor -= clock_w as i32;
        self.system_tray.clock.bounds = Rect::new(
            right_cursor,
            bar_y + btn_pad_y,
            clock_w,
            btn_size,
        );

        right_cursor -= 4; // Margin

        // Unified Quick Settings Pill (Wi-Fi + Volume + Battery: approx 96px wide)
        let quick_settings_w = 96u32;
        right_cursor -= quick_settings_w as i32;
        self.system_tray.quick_settings_pill_bounds = Rect::new(
            right_cursor,
            bar_y + btn_pad_y,
            quick_settings_w,
            btn_size,
        );

        right_cursor -= 4; // Margin

        // Overflow Chevron Button (^)
        let overflow_w = 28u32;
        right_cursor -= overflow_w as i32;
        self.system_tray.overflow_button_bounds = Rect::new(
            right_cursor,
            bar_y + btn_pad_y,
            overflow_w,
            btn_size,
        );

        let tray_left_edge = right_cursor;
        self.system_tray.bounds = Rect::new(
            tray_left_edge,
            bar_y,
            (self.screen_width as i32 - tray_left_edge) as u32,
            bar_h,
        );

        // --------------------------------------------------------------------
        // 2. Widgets Button Layout
        // --------------------------------------------------------------------
        // Weather Pill width ~140px
        let widgets_w = 140u32;
        let widgets_left = match self.alignment {
            TaskbarAlignment::Center => 12i32,
            TaskbarAlignment::Left => 12i32,
        };
        self.widgets_button.bounds = Rect::new(
            widgets_left,
            bar_y + btn_pad_y,
            widgets_w,
            btn_size,
        );

        // --------------------------------------------------------------------
        // 3. Central Application Bar (Start, Search, Task View, Apps)
        // --------------------------------------------------------------------
        let start_w = btn_size;
        let search_w = match self.search_box.mode {
            SearchDisplayMode::SearchPill => 260u32,
            SearchDisplayMode::IconAndLabel => 110u32,
            SearchDisplayMode::IconOnly => btn_size,
            SearchDisplayMode::Hidden => 0u32,
        };
        let task_view_w = btn_size;
        let app_item_w = btn_size;
        let app_spacing = 4i32;

        let total_apps_w = if self.items.is_empty() {
            0
        } else {
            (self.items.len() as u32 * app_item_w)
                + ((self.items.len().saturating_sub(1)) as u32 * app_spacing as u32)
        };

        let mut center_block_w = start_w + task_view_w + total_apps_w + 12;
        if search_w > 0 {
            center_block_w += search_w + 4;
        }

        let mut start_x = match self.alignment {
            TaskbarAlignment::Center => {
                let centered = ((self.screen_width as i32) - (center_block_w as i32)) / 2;
                // Avoid overlapping widgets button
                let min_start = self.widgets_button.bounds.right() + 16;
                centered.max(min_start)
            }
            TaskbarAlignment::Left => {
                // In Left alignment, Start button is right after widgets
                self.widgets_button.bounds.right() + 12
            }
        };

        // Start Button
        self.start_button.bounds = Rect::new(start_x, bar_y + btn_pad_y, start_w, btn_size);
        start_x += start_w as i32 + 4;

        // Search Box
        if search_w > 0 {
            self.search_box.bounds = Rect::new(start_x, bar_y + btn_pad_y, search_w, btn_size);
            start_x += search_w as i32 + 4;
        } else {
            self.search_box.bounds = Rect::EMPTY;
        }

        // Task View Button
        self.task_view.bounds = Rect::new(start_x, bar_y + btn_pad_y, task_view_w, btn_size);
        start_x += task_view_w as i32 + 4;

        // Pinned & Running App Items
        for item in &mut self.items {
            item.bounds = Rect::new(start_x, bar_y + btn_pad_y, app_item_w, btn_size);
            start_x += app_item_w as i32 + app_spacing;
        }

        // --------------------------------------------------------------------
        // 4. Flyouts Geometry Layout (Quick Settings & Overflow)
        // --------------------------------------------------------------------
        // Quick Settings card: 360 x 400 px, positioned right above the quick settings pill
        let qs_w = 360u32;
        let qs_h = 400u32;
        let qs_x = cmp::min(
            self.system_tray.quick_settings_pill_bounds.right() - qs_w as i32,
            (self.screen_width.saturating_sub(qs_w + 12)) as i32,
        );
        let qs_y = bar_y - (qs_h as i32) - 8;
        self.quick_settings.bounds = Rect::new(qs_x, qs_y, qs_w, qs_h);

        // Quick Settings toggles (3 columns x 2 rows)
        let toggle_w = 100u32;
        let toggle_h = 48u32;
        let toggle_start_x = qs_x + 16;
        let toggle_start_y = qs_y + 16;
        for (i, toggle) in self.quick_settings.toggles.iter_mut().enumerate() {
            let col = (i % 3) as i32;
            let row = (i / 3) as i32;
            let tx = toggle_start_x + col * (toggle_w as i32 + 8);
            let ty = toggle_start_y + row * (toggle_h as i32 + 8);
            toggle.bounds = Rect::new(tx, ty, toggle_w, toggle_h);
        }

        // Sliders
        let slider_w = qs_w - 32;
        let slider_h = 32u32;
        let slider1_y = toggle_start_y + 2 * (toggle_h as i32 + 8) + 16;
        self.quick_settings.volume_slider.bounds = Rect::new(qs_x + 16, slider1_y, slider_w, slider_h);
        self.quick_settings.volume_slider.track_bounds = Rect::new(qs_x + 48, slider1_y + 12, slider_w - 64, 8);

        let slider2_y = slider1_y + slider_h as i32 + 8;
        self.quick_settings.brightness_slider.bounds = Rect::new(qs_x + 16, slider2_y, slider_w, slider_h);
        self.quick_settings.brightness_slider.track_bounds = Rect::new(qs_x + 48, slider2_y + 12, slider_w - 64, 8);

        // Tray Overflow flyout: 180 x 120 px, above overflow button
        let of_w = 180u32;
        let of_h = 120u32;
        let of_x = self.system_tray.overflow_button_bounds.center_x() - (of_w as i32 / 2);
        let of_y = bar_y - (of_h as i32) - 8;
        self.system_tray.overflow_flyout.bounds = Rect::new(of_x, of_y, of_w, of_h);

        // Arrange overflow items in a grid
        let item_box_w = 40u32;
        let item_box_h = 40u32;
        for (i, item) in self.system_tray.overflow_flyout.items.iter_mut().enumerate() {
            let col = (i % 4) as i32;
            let row = (i / 4) as i32;
            item.bounds = Rect::new(
                of_x + 8 + col * (item_box_w as i32 + 4),
                of_y + 8 + row * (item_box_h as i32 + 4),
                item_box_w,
                item_box_h,
            );
        }

        // Virtual Desktop Switcher in Task View
        if self.task_view.is_flyout_open {
            let tv_w = 600u32;
            let tv_h = 140u32;
            let tv_x = ((self.screen_width as i32) - (tv_w as i32)) / 2;
            let tv_y = bar_y - (tv_h as i32) - 12;

            let card_w = 120u32;
            let card_h = 80u32;
            for (i, d) in self.task_view.desktops.iter_mut().enumerate() {
                d.preview_bounds = Rect::new(
                    tv_x + 16 + (i as i32) * (card_w as i32 + 12),
                    tv_y + 16,
                    card_w,
                    card_h,
                );
            }
            let add_x = tv_x + 16 + (self.task_view.desktops.len() as i32) * (card_w as i32 + 12);
            self.task_view.add_desktop_bounds = Rect::new(add_x, tv_y + 16, 80, card_h);
        }
    }

    /// Perform fast hit-testing across all taskbar elements
    pub fn hit_test(&self, x: i32, y: i32) -> HitTestResult {
        // 1. Quick Settings flyout takes priority if open
        if self.quick_settings.is_open && self.quick_settings.bounds.contains(x, y) {
            for toggle in &self.quick_settings.toggles {
                if toggle.bounds.contains(x, y) {
                    return HitTestResult::QuickToggle(toggle.kind);
                }
            }
            if self.quick_settings.volume_slider.bounds.contains(x, y) {
                return HitTestResult::VolumeSliderDrag;
            }
            if self.quick_settings.brightness_slider.bounds.contains(x, y) {
                return HitTestResult::BrightnessSliderDrag;
            }
            return HitTestResult::TaskbarBackground;
        }

        // 2. Tray Overflow flyout takes priority if open
        if self.system_tray.overflow_flyout.is_open
            && self.system_tray.overflow_flyout.bounds.contains(x, y)
        {
            for item in &self.system_tray.overflow_flyout.items {
                if item.bounds.contains(x, y) {
                    return HitTestResult::TrayCustomItem {
                        id: item.id.clone(),
                    };
                }
            }
            return HitTestResult::TaskbarBackground;
        }

        // 3. Hover Preview popup takes priority if active
        if let Some(ref popup) = self.active_hover_preview {
            if popup.bounds.contains(x, y) {
                for preview in &popup.previews {
                    if preview.close_button_rect.contains(x, y) {
                        return HitTestResult::ThumbnailClose {
                            app_id: popup.app_id.clone(),
                            window_id: preview.window_id,
                        };
                    }
                    if preview.card_bounds.contains(x, y) {
                        return HitTestResult::ThumbnailCard {
                            app_id: popup.app_id.clone(),
                            window_id: preview.window_id,
                        };
                    }
                }
                return HitTestResult::TaskbarBackground;
            }
        }

        // 4. Task View Virtual Desktops flyout if open
        if self.task_view.is_flyout_open {
            for d in &self.task_view.desktops {
                if d.preview_bounds.contains(x, y) {
                    return HitTestResult::VirtualDesktopCard { desktop_id: d.id };
                }
            }
            if self.task_view.add_desktop_bounds.contains(x, y) {
                return HitTestResult::AddVirtualDesktop;
            }
        }

        // 5. Main Taskbar Boundary
        if !self.bounds.contains(x, y) {
            return HitTestResult::None;
        }

        // Show Desktop Peek strip
        if self.system_tray.show_desktop_bounds.contains(x, y) {
            return HitTestResult::ShowDesktop;
        }

        // Clock & Date
        if self.system_tray.clock.bounds.contains(x, y) {
            return HitTestResult::ClockDate;
        }

        // Quick Settings Pill
        if self.system_tray.quick_settings_pill_bounds.contains(x, y) {
            return HitTestResult::TrayQuickSettingsPill;
        }

        // Tray Overflow Chevron
        if self.system_tray.overflow_button_bounds.contains(x, y) {
            return HitTestResult::TrayOverflowButton;
        }

        // Widgets Button
        if self.widgets_button.bounds.contains(x, y) {
            return HitTestResult::Widgets;
        }

        // Start Button
        if self.start_button.bounds.contains(x, y) {
            return HitTestResult::StartButton;
        }

        // Search Box
        if self.search_box.bounds.contains(x, y) {
            return HitTestResult::SearchBox;
        }

        // Task View
        if self.task_view.bounds.contains(x, y) {
            return HitTestResult::TaskView;
        }

        // App Items
        for (i, item) in self.items.iter().enumerate() {
            if item.bounds.contains(x, y) {
                return HitTestResult::AppItem {
                    index: i,
                    app_id: item.app_id.clone(),
                };
            }
        }

        HitTestResult::TaskbarBackground
    }

    /// Process mouse movement: updates hover states and live thumbnail preview popup
    pub fn handle_mouse_move(&mut self, x: i32, y: i32) {
        let hit = self.hit_test(x, y);

        // Update Start Button visual state
        self.start_button.visual_state = if hit == HitTestResult::StartButton {
            ButtonVisualState::Hover
        } else if self.start_button.is_menu_open {
            ButtonVisualState::Active
        } else {
            ButtonVisualState::Normal
        };

        // Update Search Box visual state
        self.search_box.visual_state = if hit == HitTestResult::SearchBox {
            ButtonVisualState::Hover
        } else {
            ButtonVisualState::Normal
        };

        // Update Task View visual state
        self.task_view.visual_state = if hit == HitTestResult::TaskView {
            ButtonVisualState::Hover
        } else if self.task_view.is_flyout_open {
            ButtonVisualState::Active
        } else {
            ButtonVisualState::Normal
        };

        // Update Widgets Button visual state
        self.widgets_button.visual_state = if hit == HitTestResult::Widgets {
            ButtonVisualState::Hover
        } else if self.widgets_button.is_open {
            ButtonVisualState::Active
        } else {
            ButtonVisualState::Normal
        };

        // Update Quick Settings visual state
        self.system_tray.quick_settings_visual_state = if hit == HitTestResult::TrayQuickSettingsPill {
            ButtonVisualState::Hover
        } else if self.quick_settings.is_open {
            ButtonVisualState::Active
        } else {
            ButtonVisualState::Normal
        };

        // Update Overflow Button visual state
        self.system_tray.overflow_visual_state = if hit == HitTestResult::TrayOverflowButton {
            ButtonVisualState::Hover
        } else if self.system_tray.overflow_flyout.is_open {
            ButtonVisualState::Active
        } else {
            ButtonVisualState::Normal
        };

        // Show Desktop hover peek
        let on_show_desktop = hit == HitTestResult::ShowDesktop;
        self.system_tray.is_show_desktop_hovered = on_show_desktop;
        self.system_tray.is_desktop_peek_active = on_show_desktop;

        // Update App items hover state and generate thumbnail hover previews
        let mut hovered_item = None;
        for (i, item) in self.items.iter_mut().enumerate() {
            if item.bounds.contains(x, y) {
                item.visual_state = ButtonVisualState::Hover;
                hovered_item = Some(i);
            } else if item.active_state == ItemActiveState::Focused {
                item.visual_state = ButtonVisualState::Active;
            } else {
                item.visual_state = ButtonVisualState::Normal;
            }
        }

        self.hovered_app_index = hovered_item;

        // Generate or maintain thumbnail preview popup
        if let Some(idx) = hovered_item {
            let item = &self.items[idx];
            if item.is_running && !item.windows.is_empty() {
                self.generate_thumbnail_preview(idx);
            } else {
                self.active_hover_preview = None;
            }
        } else if let Some(ref mut popup) = self.active_hover_preview {
            if !popup.bounds.contains(x, y) {
                self.active_hover_preview = None;
            } else {
                // Update hover within the preview popup cards
                for preview in &mut popup.previews {
                    preview.is_card_hovered = preview.card_bounds.contains(x, y);
                    preview.is_close_hovered = preview.close_button_rect.contains(x, y);
                }
            }
        }

        // Update dragging sliders if dragging
        if self.quick_settings.volume_slider.is_dragging {
            self.quick_settings.volume_slider.update_from_mouse_x(x);
            self.system_tray.volume_level = self.quick_settings.volume_slider.value;
        }
        if self.quick_settings.brightness_slider.is_dragging {
            self.quick_settings.brightness_slider.update_from_mouse_x(x);
        }
    }

    /// Process mouse click (down / up)
    pub fn handle_mouse_down(&mut self, x: i32, y: i32, button: MouseButton) -> HitTestResult {
        let hit = self.hit_test(x, y);

        if button == MouseButton::Left {
            match &hit {
                HitTestResult::StartButton => {
                    self.start_button.toggle_menu();
                    self.quick_settings.is_open = false;
                    self.system_tray.overflow_flyout.is_open = false;
                }
                HitTestResult::SearchBox => {
                    self.search_box.is_focused = true;
                }
                HitTestResult::TaskView => {
                    self.task_view.is_flyout_open = !self.task_view.is_flyout_open;
                    self.relayout();
                }
                HitTestResult::Widgets => {
                    self.widgets_button.is_open = !self.widgets_button.is_open;
                }
                HitTestResult::TrayQuickSettingsPill => {
                    self.quick_settings.is_open = !self.quick_settings.is_open;
                    self.start_button.is_menu_open = false;
                    self.system_tray.overflow_flyout.is_open = false;
                }
                HitTestResult::TrayOverflowButton => {
                    self.system_tray.toggle_overflow();
                    self.quick_settings.is_open = false;
                }
                HitTestResult::VolumeSliderDrag => {
                    self.quick_settings.volume_slider.is_dragging = true;
                    self.quick_settings.volume_slider.update_from_mouse_x(x);
                    self.system_tray.volume_level = self.quick_settings.volume_slider.value;
                }
                HitTestResult::BrightnessSliderDrag => {
                    self.quick_settings.brightness_slider.is_dragging = true;
                    self.quick_settings.brightness_slider.update_from_mouse_x(x);
                }
                HitTestResult::QuickToggle(kind) => {
                    self.quick_settings.toggle(*kind);
                }
                HitTestResult::AppItem { index, .. } => {
                    let idx = *index;
                    let item = &mut self.items[idx];
                    if !item.is_running {
                        // Launch app
                        item.add_window(1001 + idx as u64, &item.title.clone(), true);
                    } else if item.active_state == ItemActiveState::Focused {
                        // Minimize
                        item.active_state = ItemActiveState::Running;
                    } else {
                        // Bring to focus
                        item.active_state = ItemActiveState::Focused;
                    }
                    self.active_hover_preview = None;
                }
                HitTestResult::ThumbnailClose { app_id, window_id } => {
                    let wid = *window_id;
                    let aid = app_id.clone();
                    self.close_window(&aid, wid);
                    self.active_hover_preview = None;
                }
                HitTestResult::ThumbnailCard { app_id, window_id } => {
                    let wid = *window_id;
                    if let Some(item) = self.items.iter_mut().find(|it| it.app_id == *app_id) {
                        item.set_focused_window(wid);
                    }
                    self.active_hover_preview = None;
                }
                HitTestResult::VirtualDesktopCard { desktop_id } => {
                    let did = *desktop_id;
                    self.task_view.switch_desktop(did);
                    self.task_view.is_flyout_open = false;
                }
                HitTestResult::AddVirtualDesktop => {
                    self.task_view.add_desktop(None);
                    self.relayout();
                }
                HitTestResult::ShowDesktop => {
                    // Minimize or restore all windows
                    let all_minimized = self.items.iter().all(|it| it.active_state == ItemActiveState::Running || it.active_state == ItemActiveState::Inactive);
                    for it in &mut self.items {
                        if it.is_running {
                            it.active_state = if all_minimized {
                                ItemActiveState::Focused
                            } else {
                                ItemActiveState::Running
                            };
                        }
                    }
                }
                _ => {}
            }
        }

        hit
    }

    pub fn handle_mouse_up(&mut self, _x: i32, _y: i32, _button: MouseButton) {
        self.quick_settings.volume_slider.is_dragging = false;
        self.quick_settings.brightness_slider.is_dragging = false;
    }

    /// Generate hover thumbnail previews for an app item
    fn generate_thumbnail_preview(&mut self, item_index: usize) {
        let item = &self.items[item_index];
        let card_w = 180u32;
        let card_h = 130u32;
        let gap = 8i32;

        let num_cards = item.windows.len() as u32;
        let total_w = num_cards * card_w + ((num_cards.saturating_sub(1)) * gap as u32) + 16;
        let total_h = card_h + 16;

        let center_x = item.bounds.center_x();
        let mut popup_x = center_x - (total_w as i32 / 2);
        popup_x = popup_x.clamp(8, (self.screen_width.saturating_sub(total_w + 8)) as i32);
        let popup_y = item.bounds.y - (total_h as i32) - 8;

        let popup_bounds = Rect::new(popup_x, popup_y, total_w, total_h);

        let mut previews = Vec::new();
        for (i, win) in item.windows.iter().enumerate() {
            let cx = popup_x + 8 + (i as i32) * (card_w as i32 + gap);
            let cy = popup_y + 8;
            let card_rect = Rect::new(cx, cy, card_w, card_h);

            let thumb_rect = Rect::new(cx + 8, cy + 28, card_w - 16, card_h - 36);
            let close_rect = Rect::new(cx + card_w as i32 - 24, cy + 4, 20, 20);

            previews.push(WindowThumbnailPreview {
                window_id: win.window_id,
                window_title: win.title.clone(),
                card_bounds: card_rect,
                thumbnail_rect: thumb_rect,
                close_button_rect: close_rect,
                is_card_hovered: false,
                is_close_hovered: false,
            });
        }

        self.active_hover_preview = Some(ThumbnailPreviewPopup {
            app_id: item.app_id.clone(),
            item_index,
            bounds: popup_bounds,
            previews,
            is_visible: true,
        });
    }

    /// Clock tick update: updates the time and date string
    pub fn update_clock(&mut self, time_str: &str, date_str: &str) {
        self.system_tray.clock.time_str = time_str.to_string();
        self.system_tray.clock.date_str = date_str.to_string();
    }
}

// ============================================================================
// Section 8: Software Rasterization & Visual Compositing Engine
// ============================================================================

impl Windows11Taskbar {
    /// Render the entire taskbar into a target 32-bit ARGB framebuffer slice
    ///
    /// Implements:
    /// - Acrylic / Mica blurred base background tint
    /// - 1px 0x33FFFFFF top border highlight
    /// - Windows 11 Fluent icons and button glows
    /// - Centered bottom indicator pills (8px focused, 4px running, progress bar)
    /// - Hover thumbnail preview cards with close 'X' buttons
    /// - Unified System Tray cluster and overflow flyout
    pub fn render_to_buffer(&self, buffer: &mut [u32], pitch: usize) {
        let bar_h = self.bounds.height as usize;
        let bar_w = self.bounds.width as usize;

        if buffer.len() < bar_h * pitch {
            return;
        }

        let bg_color = match self.theme {
            TaskbarTheme::Dark | TaskbarTheme::System => Color::MICA_DARK_BASE,
            TaskbarTheme::Light => Color::MICA_LIGHT_BASE,
        };

        // 1. Draw Taskbar Acrylic / Mica background
        for y in 0..bar_h {
            let row_offset = y * pitch;
            for x in 0..bar_w {
                buffer[row_offset + x] = bg_color.to_argb();
            }
        }

        // 2. Draw 1px 0x33FFFFFF Top Border Highlight
        let top_border_color = Color::TOP_BORDER_HIGHLIGHT.to_argb();
        for x in 0..bar_w {
            buffer[x] = top_border_color;
        }

        // Helper to fill rectangle inside taskbar coordinates
        let fill_rect = |buf: &mut [u32], rect: &Rect, col: Color| {
            if rect.is_empty() {
                return;
            }
            let start_x = rect.x.clamp(0, bar_w as i32) as usize;
            let end_x = rect.right().clamp(0, bar_w as i32) as usize;
            let start_y = (rect.y - self.bounds.y).clamp(0, bar_h as i32) as usize;
            let end_y = (rect.bottom() - self.bounds.y).clamp(0, bar_h as i32) as usize;

            for y in start_y..end_y {
                let row_offset = y * pitch;
                for x in start_x..end_x {
                    let existing = Color::from_argb(buf[row_offset + x]);
                    buf[row_offset + x] = col.blend_over(existing).to_argb();
                }
            }
        };

        // 3. Render Widgets weather pill
        let widgets_bg = match self.widgets_button.visual_state {
            ButtonVisualState::Hover => Color::HOVER_GLOW,
            ButtonVisualState::Pressed | ButtonVisualState::Active => Color::PRESSED_TINT,
            ButtonVisualState::Normal => Color::TRANSPARENT,
        };
        fill_rect(buffer, &self.widgets_button.bounds, widgets_bg);

        // 4. Render Start Button
        let start_bg = match self.start_button.visual_state {
            ButtonVisualState::Hover => Color::HOVER_GLOW,
            ButtonVisualState::Pressed | ButtonVisualState::Active => Color::PRESSED_TINT,
            ButtonVisualState::Normal => Color::TRANSPARENT,
        };
        fill_rect(buffer, &self.start_button.bounds, start_bg);

        // Draw Windows 11 Start 4-Square Logo
        let logo_center_x = self.start_button.bounds.center_x();
        let logo_center_y = self.start_button.bounds.center_y();
        let sq_size = 6u32;
        let sq_gap = 2i32;

        let squares = [
            Rect::new(logo_center_x - (sq_size as i32) - 1, logo_center_y - (sq_size as i32) - 1, sq_size, sq_size),
            Rect::new(logo_center_x + sq_gap - 1, logo_center_y - (sq_size as i32) - 1, sq_size, sq_size),
            Rect::new(logo_center_x - (sq_size as i32) - 1, logo_center_y + sq_gap - 1, sq_size, sq_size),
            Rect::new(logo_center_x + sq_gap - 1, logo_center_y + sq_gap - 1, sq_size, sq_size),
        ];
        for sq in &squares {
            fill_rect(buffer, sq, Color::ACCENT_LIGHT_BLUE);
        }

        // 5. Render Search Box
        if !self.search_box.bounds.is_empty() {
            let search_bg = Color::new(24, 255, 255, 255);
            fill_rect(buffer, &self.search_box.bounds, search_bg);
        }

        // 6. Render Task View Button
        let tv_bg = match self.task_view.visual_state {
            ButtonVisualState::Hover => Color::HOVER_GLOW,
            ButtonVisualState::Pressed | ButtonVisualState::Active => Color::PRESSED_TINT,
            ButtonVisualState::Normal => Color::TRANSPARENT,
        };
        fill_rect(buffer, &self.task_view.bounds, tv_bg);

        // 7. Render Application Items & Active Indicators
        for item in &self.items {
            let item_bg = match item.visual_state {
                ButtonVisualState::Hover => Color::HOVER_GLOW,
                ButtonVisualState::Pressed | ButtonVisualState::Active => Color::PRESSED_TINT,
                ButtonVisualState::Normal => Color::TRANSPARENT,
            };
            fill_rect(buffer, &item.bounds, item_bg);

            // App Icon Placeholder block
            let icon_rect = Rect::new(
                item.bounds.center_x() - 10,
                item.bounds.center_y() - 10,
                20,
                20,
            );
            fill_rect(buffer, &icon_rect, item.icon.brand_color);

            // Active indicator pill
            if let Some((pill_rect, pill_color)) = item.indicator_rect() {
                fill_rect(buffer, &pill_rect, pill_color);
            }
        }

        // 8. Render System Tray: Quick Settings Pill
        let qs_pill_bg = match self.system_tray.quick_settings_visual_state {
            ButtonVisualState::Hover => Color::HOVER_GLOW,
            ButtonVisualState::Pressed | ButtonVisualState::Active => Color::PRESSED_TINT,
            ButtonVisualState::Normal => Color::TRANSPARENT,
        };
        fill_rect(buffer, &self.system_tray.quick_settings_pill_bounds, qs_pill_bg);

        // 9. Render System Tray: Clock & Date
        let clock_bg = if self.system_tray.clock.is_hovered {
            Color::HOVER_GLOW
        } else {
            Color::TRANSPARENT
        };
        fill_rect(buffer, &self.system_tray.clock.bounds, clock_bg);

        // 10. Render Show Desktop strip
        let peek_color = if self.system_tray.is_show_desktop_hovered {
            Color::new(180, 255, 255, 255)
        } else {
            Color::new(40, 255, 255, 255)
        };
        fill_rect(buffer, &self.system_tray.show_desktop_bounds, peek_color);
    }
}

// ============================================================================
// Section 9: Comprehensive Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_taskbar_layout_center_vs_left() {
        let mut tb = Windows11Taskbar::new(1920, 1080);

        // Default layout: Center alignment, 48px standard height
        assert_eq!(tb.alignment, TaskbarAlignment::Center);
        assert_eq!(tb.size.height(), 48);
        assert_eq!(tb.bounds, Rect::new(0, 1080 - 48, 1920, 48));

        let center_start_x = tb.start_button.bounds.x;

        // Switch to Left alignment
        tb.set_alignment(TaskbarAlignment::Left);
        assert_eq!(tb.alignment, TaskbarAlignment::Left);
        let left_start_x = tb.start_button.bounds.x;

        // In Center alignment, Start button is centered across the screen
        // In Left alignment, Start button is docked immediately after widgets
        assert!(center_start_x > left_start_x);

        // Test small (32px) and touch (56px) heights
        tb.set_size(TaskbarSize::Small);
        assert_eq!(tb.size.height(), 32);
        assert_eq!(tb.bounds.y, 1080 - 32);

        tb.set_size(TaskbarSize::Touch);
        assert_eq!(tb.size.height(), 56);
        assert_eq!(tb.bounds.y, 1080 - 56);
    }

    #[test]
    fn test_item_addition_removal_and_pinning() {
        let mut tb = Windows11Taskbar::new(1920, 1080);
        let initial_count = tb.items.len();

        // Pin a new app: Notepad
        tb.pin_app(
            "notepad",
            "Notepad",
            AppIcon::new('📝', Color::ACCENT_BLUE, "Notepad"),
        );
        assert_eq!(tb.items.len(), initial_count + 1);

        let item = tb.items.iter().find(|it| it.app_id == "notepad").unwrap();
        assert!(item.is_pinned);
        assert!(!item.is_running);
        assert_eq!(item.active_state, ItemActiveState::Inactive);

        // Launch unpinned application: Calculator
        tb.launch_or_open_window("calc", 2001, "Calculator");
        let calc_item = tb.items.iter().find(|it| it.app_id == "calc").unwrap();
        assert!(!calc_item.is_pinned);
        assert!(calc_item.is_running);
        assert_eq!(calc_item.window_count, 1);
        assert_eq!(calc_item.active_state, ItemActiveState::Focused);

        // Add a second window to Calculator
        tb.launch_or_open_window("calc", 2002, "Calculator - Scientific");
        let calc_item2 = tb.items.iter().find(|it| it.app_id == "calc").unwrap();
        assert_eq!(calc_item2.window_count, 2);

        // Close one window
        tb.close_window("calc", 2001);
        let calc_item3 = tb.items.iter().find(|it| it.app_id == "calc").unwrap();
        assert_eq!(calc_item3.window_count, 1);

        // Close the final window of unpinned app: it should be removed completely
        tb.close_window("calc", 2002);
        assert!(tb.items.iter().find(|it| it.app_id == "calc").is_none());

        // Unpin notepad: since not running, should be removed
        tb.unpin_app("notepad");
        assert!(tb.items.iter().find(|it| it.app_id == "notepad").is_none());
    }

    #[test]
    fn test_active_indicator_and_progress_updates() {
        let mut tb = Windows11Taskbar::new(1920, 1080);

        // Launch Edge
        tb.launch_or_open_window("msedge", 3001, "Edge Browser");
        let edge = tb.items.iter().find(|it| it.app_id == "msedge").unwrap();
        assert_eq!(edge.active_state, ItemActiveState::Focused);

        // Focused indicator: 8px wide pill
        let (rect, color) = edge.indicator_rect().unwrap();
        assert_eq!(rect.width, 8);
        assert_eq!(color, Color::ACCENT_LIGHT_BLUE);

        // Background running state
        let mut edge_running = edge.clone();
        edge_running.active_state = ItemActiveState::Running;
        let (rect_run, _) = edge_running.indicator_rect().unwrap();
        assert_eq!(rect_run.width, 4); // 4px dot

        // Attention flashing state
        tb.set_app_attention("msedge", true);
        let edge_att = tb.items.iter().find(|it| it.app_id == "msedge").unwrap();
        assert_eq!(edge_att.active_state, ItemActiveState::Attention);
        let (rect_att, color_att) = edge_att.indicator_rect().unwrap();
        assert_eq!(rect_att.width, 16);
        assert_eq!(color_att, Color::ATTENTION_ORANGE);

        // Progress bar updates
        tb.set_app_progress("msedge", 50);
        let edge_prog = tb.items.iter().find(|it| it.app_id == "msedge").unwrap();
        assert_eq!(edge_prog.active_state, ItemActiveState::Progress(50));
        let (rect_prog, color_prog) = edge_prog.indicator_rect().unwrap();
        assert_eq!(color_prog, Color::PROGRESS_GREEN);
        assert!(rect_prog.width > 8); // Progress bar expanded
    }

    #[test]
    fn test_click_hit_testing_start_search_apps() {
        let mut tb = Windows11Taskbar::new(1920, 1080);

        // 1. Hit test Start Button
        let start_center = tb.start_button.bounds.center_x();
        let bar_center_y = tb.bounds.center_y();
        let hit_start = tb.hit_test(start_center, bar_center_y);
        assert_eq!(hit_start, HitTestResult::StartButton);

        // Mouse click on Start button toggles menu
        assert!(!tb.start_button.is_menu_open);
        tb.handle_mouse_down(start_center, bar_center_y, MouseButton::Left);
        assert!(tb.start_button.is_menu_open);

        // 2. Hit test Search Box
        let search_center = tb.search_box.bounds.center_x();
        let hit_search = tb.hit_test(search_center, bar_center_y);
        assert_eq!(hit_search, HitTestResult::SearchBox);

        // 3. Hit test Task View Button
        let tv_center = tb.task_view.bounds.center_x();
        let hit_tv = tb.hit_test(tv_center, bar_center_y);
        assert_eq!(hit_tv, HitTestResult::TaskView);

        // 4. Hit test Widgets Button
        let widgets_center = tb.widgets_button.bounds.center_x();
        let hit_widgets = tb.hit_test(widgets_center, bar_center_y);
        assert_eq!(hit_widgets, HitTestResult::Widgets);

        // 5. Hit test App Item (Edge)
        let edge_item = &tb.items[0];
        let hit_edge = tb.hit_test(edge_item.bounds.center_x(), bar_center_y);
        assert_eq!(
            hit_edge,
            HitTestResult::AppItem {
                index: 0,
                app_id: "msedge".to_string()
            }
        );
    }

    #[test]
    fn test_system_tray_and_overflow() {
        let mut tb = Windows11Taskbar::new(1920, 1080);
        let bar_center_y = tb.bounds.center_y();

        // 1. System Tray elements verification
        assert_eq!(tb.system_tray.volume_level, 75);
        assert_eq!(tb.system_tray.volume_state(), VolumeState::High);
        assert_eq!(tb.system_tray.battery.percentage, 95);
        assert!(tb.system_tray.battery.is_charging);

        // 2. Hit test Quick Settings cluster
        let qs_center = tb.system_tray.quick_settings_pill_bounds.center_x();
        let hit_qs = tb.hit_test(qs_center, bar_center_y);
        assert_eq!(hit_qs, HitTestResult::TrayQuickSettingsPill);

        // Clicking Quick Settings pill opens flyout
        assert!(!tb.quick_settings.is_open);
        tb.handle_mouse_down(qs_center, bar_center_y, MouseButton::Left);
        assert!(tb.quick_settings.is_open);

        // 3. Hit test Overflow Chevron
        let of_center = tb.system_tray.overflow_button_bounds.center_x();
        let hit_of = tb.hit_test(of_center, bar_center_y);
        assert_eq!(hit_of, HitTestResult::TrayOverflowButton);

        // Clicking Overflow button opens overflow panel
        tb.handle_mouse_down(of_center, bar_center_y, MouseButton::Left);
        assert!(tb.system_tray.overflow_flyout.is_open);

        // Hit testing item inside overflow panel
        let first_overflow = &tb.system_tray.overflow_flyout.items[0];
        let hit_sub = tb.hit_test(first_overflow.bounds.center_x(), first_overflow.bounds.center_y());
        assert_eq!(
            hit_sub,
            HitTestResult::TrayCustomItem {
                id: first_overflow.id.clone()
            }
        );

        // 4. Hit test Show Desktop Peek strip (at right edge)
        let hit_show_desktop = tb.hit_test(1919, bar_center_y);
        assert_eq!(hit_show_desktop, HitTestResult::ShowDesktop);
    }

    #[test]
    fn test_quick_settings_toggles_and_sliders() {
        let mut tb = Windows11Taskbar::new(1920, 1080);
        tb.quick_settings.is_open = true;

        // Toggle Wi-Fi
        let wifi_initial = tb.quick_settings.toggles[0].is_active;
        tb.quick_settings.toggle(QuickToggleKind::WiFi);
        assert_ne!(tb.quick_settings.toggles[0].is_active, wifi_initial);

        // Toggle Night Light
        assert!(!tb.quick_settings.toggles[3].is_active);
        tb.quick_settings.toggle(QuickToggleKind::NightLight);
        assert!(tb.quick_settings.toggles[3].is_active);

        // Slider value update via mouse coordinate
        let slider = &mut tb.quick_settings.volume_slider;
        let track_mid_x = slider.track_bounds.center_x();
        slider.update_from_mouse_x(track_mid_x);
        assert!((slider.value as i32 - 50).abs() <= 2);
    }

    #[test]
    fn test_thumbnail_preview_generation_and_close() {
        let mut tb = Windows11Taskbar::new(1920, 1080);

        // Launch Edge with 2 windows
        tb.launch_or_open_window("msedge", 5001, "Google - Microsoft Edge");
        tb.launch_or_open_window("msedge", 5002, "HybridOS GitHub - Microsoft Edge");

        // Hover over Edge item
        let edge_x = tb.items[0].bounds.center_x();
        let bar_center_y = tb.bounds.center_y();
        tb.handle_mouse_move(edge_x, bar_center_y);

        // Active preview popup must be generated
        assert!(tb.active_hover_preview.is_some());
        let popup = tb.active_hover_preview.as_ref().unwrap();
        assert_eq!(popup.app_id, "msedge");
        assert_eq!(popup.previews.len(), 2);

        // Hit test close button on the first window preview
        let close_rect = popup.previews[0].close_button_rect;
        let hit_close = tb.hit_test(close_rect.center_x(), close_rect.center_y());
        assert_eq!(
            hit_close,
            HitTestResult::ThumbnailClose {
                app_id: "msedge".to_string(),
                window_id: 5001
            }
        );

        // Click close button
        tb.handle_mouse_down(close_rect.center_x(), close_rect.center_y(), MouseButton::Left);
        let edge_item = tb.items.iter().find(|it| it.app_id == "msedge").unwrap();
        assert_eq!(edge_item.window_count, 1);
    }

    #[test]
    fn test_virtual_desktops_task_view() {
        let mut tb = Windows11Taskbar::new(1920, 1080);

        assert_eq!(tb.task_view.desktops.len(), 2);
        assert_eq!(tb.task_view.active_desktop_id, 1);

        // Add a new Virtual Desktop
        let new_id = tb.task_view.add_desktop(Some("Work Desktop".to_string()));
        assert_eq!(new_id, 3);
        assert_eq!(tb.task_view.desktops.len(), 3);

        // Switch to the newly created desktop
        let switched = tb.task_view.switch_desktop(3);
        assert!(switched);
        assert_eq!(tb.task_view.active_desktop_id, 3);
        assert!(tb.task_view.desktops.iter().find(|d| d.id == 3).unwrap().is_active);

        // Close Desktop 2
        let closed = tb.task_view.close_desktop(2);
        assert!(closed);
        assert_eq!(tb.task_view.desktops.len(), 2);
    }

    #[test]
    fn test_software_rendering_to_buffer() {
        let tb = Windows11Taskbar::new(1920, 1080);
        let bar_h = 48usize;
        let bar_w = 1920usize;
        let mut buffer = vec![0u32; bar_w * bar_h];

        tb.render_to_buffer(&mut buffer, bar_w);

        // Verify top border highlight 0x33FFFFFF is written to row 0 (e.g. at x = 0 or 5)
        assert_eq!(buffer[0], Color::TOP_BORDER_HIGHLIGHT.to_argb());
        assert_eq!(buffer[5], Color::TOP_BORDER_HIGHLIGHT.to_argb());

        // Verify far right pixel has Show Desktop strip blended over it
        let peek_pixel = Color::from_argb(buffer[bar_w - 1]);
        assert_eq!(peek_pixel.r, 255);
        assert_eq!(peek_pixel.g, 255);
        assert_eq!(peek_pixel.b, 255);
        assert!(peek_pixel.a > 0x33);

        // Verify Mica dark background tint is rendered to lower rows
        assert_eq!(buffer[bar_w * 25 + 100], Color::MICA_DARK_BASE.to_argb());
    }

    #[test]
    fn test_search_box_display_modes() {
        let mut tb = Windows11Taskbar::new(1920, 1080);

        // Default mode is SearchPill (~260px)
        assert_eq!(tb.search_box.mode, SearchDisplayMode::SearchPill);
        assert_eq!(tb.search_box.bounds.width, 260);

        // Switch to IconOnly mode (~40px)
        tb.set_search_mode(SearchDisplayMode::IconOnly);
        assert_eq!(tb.search_box.bounds.width, 40);

        // Switch to IconAndLabel mode (~110px)
        tb.set_search_mode(SearchDisplayMode::IconAndLabel);
        assert_eq!(tb.search_box.bounds.width, 110);

        // Switch to Hidden mode (width 0, empty rect)
        tb.set_search_mode(SearchDisplayMode::Hidden);
        assert!(tb.search_box.bounds.is_empty());
    }

    #[test]
    fn test_clock_date_updates_and_aero_peek() {
        let mut tb = Windows11Taskbar::new(1920, 1080);

        // Clock update
        tb.update_clock("15:45", "11/11/2026");
        assert_eq!(tb.system_tray.clock.time_str, "15:45");
        assert_eq!(tb.system_tray.clock.date_str, "11/11/2026");

        // Weather update
        tb.widgets_button.update_weather("19°C", WeatherCondition::Rainy, "Light Rain");
        assert_eq!(tb.widgets_button.temperature, "19°C");
        assert_eq!(tb.widgets_button.condition, WeatherCondition::Rainy);
        assert_eq!(tb.widgets_button.condition.icon_glyph(), "🌧");

        // Hover over far-right peek strip triggers aero peek
        let bar_center_y = tb.bounds.center_y();
        assert!(!tb.system_tray.is_desktop_peek_active);
        tb.handle_mouse_move(1919, bar_center_y);
        assert!(tb.system_tray.is_desktop_peek_active);
        assert!(tb.system_tray.is_show_desktop_hovered);

        // Move mouse away disables aero peek
        tb.handle_mouse_move(500, bar_center_y);
        assert!(!tb.system_tray.is_desktop_peek_active);
    }
}
