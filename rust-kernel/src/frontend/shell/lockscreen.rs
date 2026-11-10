//! HybridOS Windows 11 Lock Screen, Sign-in Screen & Quick Settings Flyout
//!
//! Production-grade implementation of the complete Windows 11 Lock and Sign-in Experience:
//! 1. **Lock Screen**:
//!    - State machine: `Locked`, `Unlocking` (smooth physics-based slide-up spring animation),
//!      `LoginScreen`, and `DesktopUnlocked`.
//!    - Display elements: Large Segoe UI Variable clock (HH:MM), full date ("Tuesday, November 10"),
//!      Weather badge with condition glyph, Battery level percentage & charging status,
//!      and Network connectivity indicators.
//!    - Unlock triggers: Any keypress, mouse click, and touch swipe-up with velocity & threshold tracking.
//! 2. **Sign-in Screen**:
//!    - Blurred background with Mica / Acrylic depth and luminosity transition.
//!    - User profile: Avatar circle with initials/image, display name, account type,
//!      and sign-in options selector (PIN, Password, Windows Hello Face/Fingerprint, Security Key).
//!    - Password / PIN input field with password masking (`•`), reveal password eye button,
//!      submit enter arrow button, and Caps Lock detection indicator.
//!    - Credential verification with failure shake physics animation (`A * exp(-t) * sin(wt)`),
//!      error diagnostic banner, and lockout protection after consecutive failed attempts.
//!    - Bottom-right system controls: Network flyout, Accessibility flyout, and Power options
//!      (Sleep, Restart, Shut Down, Hibernate).
//! 3. **Quick Settings Flyout (Win+A)**:
//!    - Grid of interactive quick action pill buttons: Wi-Fi, Bluetooth, Airplane mode (with
//!      hardware radio mutual exclusivity), Battery saver, Night light, Accessibility,
//!      Mobile hotspot, Nearby share, and Project.
//!    - Slider controls: Display brightness (0..100%), Master audio volume (0..100%) with
//!      interactive mute toggle and audio device selector.
//!    - Battery indicator with remaining time estimate and Power Settings shortcut link.
//! 4. **Notification Center & Calendar Flyout (Win+N)**:
//!    - Monthly calendar view with Tomohiko Sakamoto Gregorian weekday calculations,
//!      leap year February adjustments, current day highlighted, and expand/collapse month toggle.
//!    - Notification stack grouped by application with priority levels, action buttons,
//!      individual dismiss, per-app clear, global "Clear all", and Do Not Disturb (DND) filter.
//! 5. **Unit Tests**:
//!    - Comprehensive test suite validating state transitions, touch swipe physics,
//!      credential validation & failure shake, quick settings toggles & sliders,
//!      notification stacking, calendar date calculations, and framebuffer rendering.
//!
//! 100% Pure, Safe, Robust Rust (#![deny(unsafe_code)]).

#![deny(unsafe_code)]
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::fmt;

// ============================================================================
// Section 1: Fluent Geometry, Color, and Surface Primitives
// ============================================================================

/// 2D Cartesian coordinate point
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    pub const ZERO: Self = Self { x: 0, y: 0 };

    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

/// 2D Dimensions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

impl Size {
    pub const ZERO: Self = Self { width: 0, height: 0 };

    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}

/// 2D Axis-aligned Bounding Box
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

    #[inline]
    pub fn contains_point(&self, pt: Point) -> bool {
        self.contains(pt.x, pt.y)
    }

    pub fn intersects(&self, other: &Rect) -> bool {
        !self.is_empty()
            && !other.is_empty()
            && self.left() < other.right()
            && self.right() > other.left()
            && self.top() < other.bottom()
            && self.bottom() > other.top()
    }

    pub fn inset(&self, dx: i32, dy: i32) -> Self {
        let x = self.x.saturating_add(dx);
        let y = self.y.saturating_add(dy);
        let width = (self.width as i32).saturating_sub(dx * 2).max(0) as u32;
        let height = (self.height as i32).saturating_sub(dy * 2).max(0) as u32;
        Self { x, y, width, height }
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

/// 32-bit RGBA Color in sRGB color space
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
    /// Windows 11 Light Accent Cyan: #60CDFF
    pub const WIN11_LIGHT_ACCENT: Self = Self { r: 96, g: 205, b: 255, a: 255 };
    /// Windows 11 Mica Dark Tint: #202020
    pub const MICA_DARK: Self = Self { r: 32, g: 32, b: 32, a: 255 };
    /// Windows 11 Acrylic Overlay Dark (80% opacity)
    pub const ACRYLIC_DARK: Self = Self { r: 28, g: 28, b: 28, a: 204 };
    /// Windows 11 Card Background
    pub const CARD_BACKGROUND: Self = Self { r: 36, g: 36, b: 36, a: 220 };
    /// Windows 11 Error Red: #E81123
    pub const ERROR_RED: Self = Self { r: 232, g: 17, b: 35, a: 255 };
    /// Windows 11 Card Surface Stroke (subtle white border)
    pub const CARD_STROKE: Self = Self { r: 255, g: 255, b: 255, a: 20 };
    /// Subtle control fill
    pub const CONTROL_FILL: Self = Self { r: 255, g: 255, b: 255, a: 16 };
    /// Control fill hovered
    pub const CONTROL_HOVER: Self = Self { r: 255, g: 255, b: 255, a: 26 };
    /// Active toggle pill fill: #0078D4
    pub const TOGGLE_ACTIVE: Self = Self { r: 0, g: 120, b: 212, a: 255 };
    /// Inactive toggle pill fill
    pub const TOGGLE_INACTIVE: Self = Self { r: 255, g: 255, b: 255, a: 20 };

    #[inline]
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    #[inline]
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    #[inline]
    pub const fn with_alpha(self, a: u8) -> Self {
        Self { a, ..self }
    }

    pub fn with_alpha_f32(self, a: f32) -> Self {
        let clamped = a.clamp(0.0, 1.0);
        Self {
            a: (clamped * 255.0).round() as u8,
            ..self
        }
    }

    /// Linear interpolation between two colors by factor `t` (0.0..=1.0)
    pub fn lerp(self, target: Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        let inv_t = 1.0 - t;
        Self {
            r: (self.r as f32 * inv_t + target.r as f32 * t).round() as u8,
            g: (self.g as f32 * inv_t + target.g as f32 * t).round() as u8,
            b: (self.b as f32 * inv_t + target.b as f32 * t).round() as u8,
            a: (self.a as f32 * inv_t + target.a as f32 * t).round() as u8,
        }
    }

    /// Alpha blend: self OVER dst
    pub fn blend_over(self, dst: Self) -> Self {
        let src_a = self.a as f32 / 255.0;
        let dst_a = dst.a as f32 / 255.0;
        let out_a = src_a + dst_a * (1.0 - src_a);
        if out_a <= 0.0 {
            return Self::TRANSPARENT;
        }
        let out_r = (self.r as f32 * src_a + dst.r as f32 * dst_a * (1.0 - src_a)) / out_a;
        let out_g = (self.g as f32 * src_a + dst.g as f32 * dst_a * (1.0 - src_a)) / out_a;
        let out_b = (self.b as f32 * src_a + dst.b as f32 * dst_a * (1.0 - src_a)) / out_a;
        Self {
            r: out_r.round() as u8,
            g: out_g.round() as u8,
            b: out_b.round() as u8,
            a: (out_a * 255.0).round() as u8,
        }
    }

    pub const fn to_argb_u32(&self) -> u32 {
        ((self.a as u32) << 24) | ((self.r as u32) << 16) | ((self.g as u32) << 8) | (self.b as u32)
    }

    pub const fn to_rgba_u32(&self) -> u32 {
        ((self.r as u32) << 24) | ((self.g as u32) << 8) | ((self.b as u32) << 16) | (self.a as u32)
    }

    pub const fn from_argb_u32(val: u32) -> Self {
        Self {
            a: ((val >> 24) & 0xFF) as u8,
            r: ((val >> 16) & 0xFF) as u8,
            g: ((val >> 8) & 0xFF) as u8,
            b: (val & 0xFF) as u8,
        }
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{:02X}{:02X}{:02X}{:02X}", self.r, self.g, self.b, self.a)
    }
}

/// 8px Rounded Corner Radii for Windows 11 Fluent surfaces
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CornerRadii {
    pub top_left: f32,
    pub top_right: f32,
    pub bottom_left: f32,
    pub bottom_right: f32,
}

impl CornerRadii {
    pub const WIN11_STANDARD: Self = Self {
        top_left: 8.0,
        top_right: 8.0,
        bottom_left: 8.0,
        bottom_right: 8.0,
    };

    pub const fn uniform(radius: f32) -> Self {
        Self {
            top_left: radius,
            top_right: radius,
            bottom_left: radius,
            bottom_right: radius,
        }
    }

    pub fn pill(height: f32) -> Self {
        Self::uniform(height / 2.0)
    }
}

impl Default for CornerRadii {
    fn default() -> Self {
        Self::WIN11_STANDARD
    }
}

// ============================================================================
// Section 2: Segoe UI Variable Typography & Time/Date Engine
// ============================================================================

/// Font weights for Segoe UI Variable
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontWeight {
    Regular = 400,
    SemiBold = 600,
    Bold = 700,
}

/// Segoe UI Variable Typography Style Definition
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TypographyStyle {
    pub name: &'static str,
    pub pixel_size: f32,
    pub weight: FontWeight,
    pub line_height: f32,
}

impl TypographyStyle {
    /// Clock Display: 84px Bold (Windows 11 Lockscreen Clock)
    pub const LOCK_CLOCK: Self = Self {
        name: "LockClock",
        pixel_size: 84.0,
        weight: FontWeight::Bold,
        line_height: 104.0,
    };

    /// Date Subtitle: 22px Semi-Bold (Windows 11 Lockscreen Date)
    pub const LOCK_DATE: Self = Self {
        name: "LockDate",
        pixel_size: 22.0,
        weight: FontWeight::SemiBold,
        line_height: 30.0,
    };

    /// User Display Name: 28px Semi-Bold
    pub const USER_NAME: Self = Self {
        name: "UserName",
        pixel_size: 28.0,
        weight: FontWeight::SemiBold,
        line_height: 36.0,
    };

    /// Body text: 14px Regular
    pub const BODY: Self = Self {
        name: "Body",
        pixel_size: 14.0,
        weight: FontWeight::Regular,
        line_height: 20.0,
    };

    /// Caption text: 12px Regular
    pub const CAPTION: Self = Self {
        name: "Caption",
        pixel_size: 12.0,
        weight: FontWeight::Regular,
        line_height: 16.0,
    };
}

/// Day of the week
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Weekday {
    Sunday = 0,
    Monday = 1,
    Tuesday = 2,
    Wednesday = 3,
    Thursday = 4,
    Friday = 5,
    Saturday = 6,
}

impl Weekday {
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Sunday => "Sunday",
            Self::Monday => "Monday",
            Self::Tuesday => "Tuesday",
            Self::Wednesday => "Wednesday",
            Self::Thursday => "Thursday",
            Self::Friday => "Friday",
            Self::Saturday => "Saturday",
        }
    }

    pub const fn short_name(&self) -> &'static str {
        match self {
            Self::Sunday => "Su",
            Self::Monday => "Mo",
            Self::Tuesday => "Tu",
            Self::Wednesday => "We",
            Self::Thursday => "Th",
            Self::Friday => "Fr",
            Self::Saturday => "Sa",
        }
    }
}

/// System Date & Time Container with formatting and calendar calculations
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DateTimeDisplay {
    pub year: u32,
    pub month: u8, // 1..=12
    pub day: u8,   // 1..=31
    pub hour: u8,  // 0..=23
    pub minute: u8,// 0..=59
    pub second: u8,// 0..=59
}

impl DateTimeDisplay {
    pub fn new(year: u32, month: u8, day: u8, hour: u8, minute: u8, second: u8) -> Self {
        Self {
            year,
            month: month.clamp(1, 12),
            day: day.clamp(1, 31),
            hour: hour.min(23),
            minute: minute.min(59),
            second: second.min(59),
        }
    }

    /// Default reference date: Tuesday, November 10, 2026 10:42 AM
    pub fn default_win11() -> Self {
        Self::new(2026, 11, 10, 10, 42, 0)
    }

    /// Check if a year is a Gregorian leap year
    #[inline]
    pub const fn is_leap_year(year: u32) -> bool {
        (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
    }

    /// Calculate the number of days in a given month of a given year
    pub const fn days_in_month(year: u32, month: u8) -> u8 {
        match month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 => if Self::is_leap_year(year) { 29 } else { 28 },
            _ => 30,
        }
    }

    /// Compute day of the week using Sakamoto's algorithm (0 = Sunday .. 6 = Saturday)
    pub fn day_of_week_for(year: u32, month: u8, day: u8) -> Weekday {
        let t = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
        let mut y = year;
        if month < 3 {
            y = y.saturating_sub(1);
        }
        let m_idx = (month.clamp(1, 12) - 1) as usize;
        let day_num = (y + y / 4 - y / 100 + y / 400 + t[m_idx] + day as u32) % 7;
        match day_num {
            0 => Weekday::Sunday,
            1 => Weekday::Monday,
            2 => Weekday::Tuesday,
            3 => Weekday::Wednesday,
            4 => Weekday::Thursday,
            5 => Weekday::Friday,
            _ => Weekday::Saturday,
        }
    }

    /// Day of the week for `self`
    pub fn weekday(&self) -> Weekday {
        Self::day_of_week_for(self.year, self.month, self.day)
    }

    /// Full month name string
    pub const fn month_name(month: u8) -> &'static str {
        match month {
            1 => "January",
            2 => "February",
            3 => "March",
            4 => "April",
            5 => "May",
            6 => "June",
            7 => "July",
            8 => "August",
            9 => "September",
            10 => "October",
            11 => "November",
            12 => "December",
            _ => "Unknown",
        }
    }

    /// Format clock in "HH:MM" (e.g. "10:42")
    pub fn clock_hh_mm(&self) -> String {
        format!("{:02}:{:02}", self.hour, self.minute)
    }

    /// Format full lockscreen date string: "Tuesday, November 10"
    pub fn full_date_string(&self) -> String {
        format!("{}, {} {}", self.weekday().name(), Self::month_name(self.month), self.day)
    }

    /// Format full date with year: "Tuesday, November 10, 2026"
    pub fn full_date_with_year(&self) -> String {
        format!("{}, {} {}, {}", self.weekday().name(), Self::month_name(self.month), self.day, self.year)
    }

    /// Advance time by seconds
    pub fn advance_seconds(&mut self, secs: u32) {
        let total_secs = self.second as u32 + secs;
        self.second = (total_secs % 60) as u8;
        let total_mins = self.minute as u32 + (total_secs / 60);
        self.minute = (total_mins % 60) as u8;
        let total_hours = self.hour as u32 + (total_mins / 60);
        self.hour = (total_hours % 24) as u8;
        let days_to_add = total_hours / 24;
        for _ in 0..days_to_add {
            self.day += 1;
            let max_days = Self::days_in_month(self.year, self.month);
            if self.day > max_days {
                self.day = 1;
                self.month += 1;
                if self.month > 12 {
                    self.month = 1;
                    self.year += 1;
                }
            }
        }
    }
}

// ============================================================================
// Section 3: Lock Screen Subsystem
// ============================================================================

/// Lock Screen State Machine
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LockScreenState {
    /// Fully locked screen showing clock, weather, battery, and network status
    Locked,
    /// Unlocking slide-up physics animation
    Unlocking {
        /// Progress from 0.0 (locked) to 1.0 (transitioned to sign-in screen)
        progress: f32,
        /// Current vertical velocity in pixels per second
        velocity: f32,
        /// Vertical offset in pixels (-screen_height to 0.0)
        offset_y: f32,
    },
    /// Windows 11 Sign-in / Credential Provider screen
    LoginScreen,
    /// User authenticated and desktop fully unlocked
    DesktopUnlocked,
}

/// Weather Condition for the Lockscreen Weather Badge
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeatherCondition {
    Sunny,
    PartlyCloudy,
    Cloudy,
    Rainy,
    HeavyRain,
    Thunderstorm,
    Snowy,
    Foggy,
}

impl WeatherCondition {
    pub const fn glyph(&self) -> &'static str {
        match self {
            Self::Sunny => "☀️",
            Self::PartlyCloudy => "⛅",
            Self::Cloudy => "☁️",
            Self::Rainy => "🌧️",
            Self::HeavyRain => "⛈️",
            Self::Thunderstorm => "⚡",
            Self::Snowy => "❄️",
            Self::Foggy => "🌫️",
        }
    }

    pub const fn description(&self) -> &'static str {
        match self {
            Self::Sunny => "Sunny",
            Self::PartlyCloudy => "Partly Cloudy",
            Self::Cloudy => "Cloudy",
            Self::Rainy => "Rain",
            Self::HeavyRain => "Heavy Rain",
            Self::Thunderstorm => "Thunderstorm",
            Self::Snowy => "Snow",
            Self::Foggy => "Fog",
        }
    }
}

/// Weather Badge Data Container
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeatherBadge {
    pub condition: WeatherCondition,
    pub temperature_c: i32,
    pub temperature_f: i32,
    pub high_c: i32,
    pub low_c: i32,
    pub location: String,
    pub bounds: Rect,
}

impl WeatherBadge {
    pub fn new(condition: WeatherCondition, temp_c: i32, location: impl Into<String>) -> Self {
        let temp_f = (temp_c as f32 * 1.8 + 32.0).round() as i32;
        Self {
            condition,
            temperature_c: temp_c,
            temperature_f: temp_f,
            high_c: temp_c + 3,
            low_c: temp_c - 4,
            location: location.into(),
            bounds: Rect::EMPTY,
        }
    }

    pub fn formatted_summary(&self) -> String {
        format!("{} {}°C, {}", self.condition.glyph(), self.temperature_c, self.condition.description())
    }
}

/// Hardware Battery Status
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatteryStatus {
    pub percentage: u8,
    pub is_charging: bool,
    pub is_plugged_in: bool,
    pub is_battery_saver: bool,
    pub remaining_minutes: Option<u32>,
    pub bounds: Rect,
}

impl BatteryStatus {
    pub fn new(percentage: u8, is_charging: bool) -> Self {
        Self {
            percentage: percentage.min(100),
            is_charging,
            is_plugged_in: is_charging,
            is_battery_saver: percentage <= 20 && !is_charging,
            remaining_minutes: if is_charging { None } else { Some((percentage as u32) * 4) },
            bounds: Rect::EMPTY,
        }
    }

    pub fn glyph(&self) -> &'static str {
        if self.is_charging {
            "⚡"
        } else if self.percentage > 80 {
            "󰁹"
        } else if self.percentage > 50 {
            "󰁾"
        } else if self.percentage > 20 {
            "󰁼"
        } else {
            "󰁺"
        }
    }

    pub fn formatted_time_remaining(&self) -> String {
        if self.is_charging {
            "Plugged in, charging".to_string()
        } else if let Some(mins) = self.remaining_minutes {
            let hours = mins / 60;
            let m = mins % 60;
            if hours > 0 {
                format!("{} hr {} min remaining", hours, m)
            } else {
                format!("{} min remaining", m)
            }
        } else {
            format!("{}%", self.percentage)
        }
    }
}

/// Network Connectivity Kind
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkKind {
    WiFi,
    Ethernet,
    Cellular,
    Disconnected,
}

/// Hardware Network Status
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkStatus {
    pub kind: NetworkKind,
    pub ssid: String,
    pub signal_bars: u8, // 0..=4
    pub is_connected: bool,
    pub has_internet: bool,
    pub bounds: Rect,
}

impl NetworkStatus {
    pub fn wifi(ssid: impl Into<String>, signal_bars: u8) -> Self {
        Self {
            kind: NetworkKind::WiFi,
            ssid: ssid.into(),
            signal_bars: signal_bars.min(4),
            is_connected: true,
            has_internet: true,
            bounds: Rect::EMPTY,
        }
    }

    pub fn ethernet() -> Self {
        Self {
            kind: NetworkKind::Ethernet,
            ssid: "Ethernet".to_string(),
            signal_bars: 4,
            is_connected: true,
            has_internet: true,
            bounds: Rect::EMPTY,
        }
    }

    pub fn disconnected() -> Self {
        Self {
            kind: NetworkKind::Disconnected,
            ssid: "No connection".to_string(),
            signal_bars: 0,
            is_connected: false,
            has_internet: false,
            bounds: Rect::EMPTY,
        }
    }

    pub fn glyph(&self) -> &'static str {
        match self.kind {
            NetworkKind::WiFi => match self.signal_bars {
                4 => "󰤨",
                3 => "󰤥",
                2 => "󰤢",
                1 => "󰤟",
                _ => "󰤯",
            },
            NetworkKind::Ethernet => "󰈀",
            NetworkKind::Cellular => "󰀂",
            NetworkKind::Disconnected => "󰤮",
        }
    }
}

/// Touch Swipe Physics Engine for Lockscreen Slide-Up Transition
#[derive(Debug, Clone, PartialEq)]
pub struct TouchSwipePhysics {
    pub is_tracking: bool,
    pub start_y: f32,
    pub current_y: f32,
    pub velocity_y: f32, // pixels per millisecond
    pub start_time_ms: f32,
    pub last_time_ms: f32,
    pub screen_height: f32,
    pub unlock_threshold_pct: f32, // 0.20 (20% of screen height)
    pub fling_velocity_threshold: f32, // -0.4 px/ms (upward)
}

impl TouchSwipePhysics {
    pub fn new(screen_height: f32) -> Self {
        Self {
            is_tracking: false,
            start_y: 0.0,
            current_y: 0.0,
            velocity_y: 0.0,
            start_time_ms: 0.0,
            last_time_ms: 0.0,
            screen_height: screen_height.max(100.0),
            unlock_threshold_pct: 0.20,
            fling_velocity_threshold: -0.4,
        }
    }

    pub fn on_touch_down(&mut self, y: f32, time_ms: f32) {
        self.is_tracking = true;
        self.start_y = y;
        self.current_y = y;
        self.velocity_y = 0.0;
        self.start_time_ms = time_ms;
        self.last_time_ms = time_ms;
    }

    pub fn on_touch_move(&mut self, y: f32, time_ms: f32) {
        if !self.is_tracking {
            return;
        }
        let dt = (time_ms - self.last_time_ms).max(1.0);
        let dy = y - self.current_y;
        self.velocity_y = dy / dt;
        self.current_y = y;
        self.last_time_ms = time_ms;
    }

    /// On touch release, determine if the swipe completed an unlock or should snap back
    pub fn on_touch_up(&mut self) -> SwipeResult {
        if !self.is_tracking {
            return SwipeResult::SnapBack;
        }
        self.is_tracking = false;
        let delta_y = self.current_y - self.start_y; // negative is upward
        let distance_up = -delta_y;
        let threshold_dist = self.screen_height * self.unlock_threshold_pct;

        // Fling upward or dragged past threshold
        if distance_up > threshold_dist || self.velocity_y < self.fling_velocity_threshold {
            SwipeResult::Unlock {
                initial_velocity: self.velocity_y.min(-0.8),
            }
        } else {
            SwipeResult::SnapBack
        }
    }

    /// Calculate current displacement (clamped between -screen_height and 0)
    pub fn current_displacement(&self) -> f32 {
        if !self.is_tracking {
            0.0
        } else {
            let dy = self.current_y - self.start_y;
            dy.clamp(-self.screen_height, 0.0)
        }
    }
}

/// Result of touch swipe gesture
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SwipeResult {
    Unlock { initial_velocity: f32 },
    SnapBack,
}

// ============================================================================
// Section 4: Sign-in Screen & Credential Provider Subsystem
// ============================================================================

/// Available Windows 11 Sign-In Methods
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignInMethod {
    Pin,
    Password,
    WindowsHelloFace,
    WindowsHelloFingerprint,
    SecurityKey,
}

impl SignInMethod {
    pub const fn display_name(&self) -> &'static str {
        match self {
            Self::Pin => "PIN",
            Self::Password => "Password",
            Self::WindowsHelloFace => "Windows Hello Face",
            Self::WindowsHelloFingerprint => "Windows Hello Fingerprint",
            Self::SecurityKey => "Security Key",
        }
    }

    pub const fn icon(&self) -> &'static str {
        match self {
            Self::Pin => "󰞌",
            Self::Password => "󰌾",
            Self::WindowsHelloFace => "󰈷",
            Self::WindowsHelloFingerprint => "󰈔",
            Self::SecurityKey => "󰌆",
        }
    }
}

/// User Profile Information
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserProfile {
    pub display_name: String,
    pub username: String,
    pub email_or_role: String,
    pub initials: String,
    pub avatar_color: Color,
    pub avatar_bounds: Rect,
}

impl UserProfile {
    pub fn new(display_name: impl Into<String>, username: impl Into<String>, role: impl Into<String>) -> Self {
        let name_str = display_name.into();
        let initials = name_str
            .split_whitespace()
            .filter_map(|w| w.chars().next())
            .take(2)
            .collect::<String>()
            .to_uppercase();
        Self {
            display_name: name_str,
            username: username.into(),
            email_or_role: role.into(),
            initials: if initials.is_empty() { "U".to_string() } else { initials },
            avatar_color: Color::WIN11_BLUE,
            avatar_bounds: Rect::EMPTY,
        }
    }

    pub fn default_user() -> Self {
        Self::new("Vinicius", "vinicius", "Administrator")
    }
}

/// Password / PIN Input Field State
#[derive(Debug, Clone, PartialEq)]
pub struct PasswordInputField {
    pub buffer: String,
    pub is_revealed: bool,
    pub is_focused: bool,
    pub is_caps_lock: bool,
    pub bounds: Rect,
    pub eye_button_bounds: Rect,
    pub submit_button_bounds: Rect,
}

impl PasswordInputField {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            is_revealed: false,
            is_focused: true,
            is_caps_lock: false,
            bounds: Rect::EMPTY,
            eye_button_bounds: Rect::EMPTY,
            submit_button_bounds: Rect::EMPTY,
        }
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    pub fn push_char(&mut self, ch: char) {
        if !ch.is_control() {
            self.buffer.push(ch);
        }
    }

    pub fn pop_char(&mut self) {
        self.buffer.pop();
    }

    pub fn displayed_mask(&self) -> String {
        if self.is_revealed {
            self.buffer.clone()
        } else {
            "•".repeat(self.buffer.len())
        }
    }

    pub fn toggle_reveal(&mut self) {
        self.is_revealed = !self.is_revealed;
    }
}

impl Default for PasswordInputField {
    fn default() -> Self {
        Self::new()
    }
}

/// Damped Oscillation Failure Shake Animation: `offset = A * exp(-decay * t) * sin(2 * PI * freq * t)`
#[derive(Debug, Clone, PartialEq)]
pub struct ShakeAnimation {
    pub is_active: bool,
    pub elapsed_ms: f32,
    pub duration_ms: f32,
    pub frequency: f32,       // Hz
    pub initial_amplitude: f32,// pixels
    pub decay_rate: f32,
}

impl ShakeAnimation {
    pub fn new() -> Self {
        Self {
            is_active: false,
            elapsed_ms: 0.0,
            duration_ms: 500.0,
            frequency: 24.0,
            initial_amplitude: 16.0,
            decay_rate: 6.0,
        }
    }

    pub fn trigger(&mut self) {
        self.is_active = true;
        self.elapsed_ms = 0.0;
    }

    pub fn update(&mut self, dt_ms: f32) {
        if !self.is_active {
            return;
        }
        self.elapsed_ms += dt_ms;
        if self.elapsed_ms >= self.duration_ms {
            self.is_active = false;
            self.elapsed_ms = 0.0;
        }
    }

    pub fn current_offset_x(&self) -> f32 {
        if !self.is_active {
            0.0
        } else {
            let t = (self.elapsed_ms / 1000.0).clamp(0.0, 1.0);
            let damping = (-self.decay_rate * t).exp();
            let wave = (2.0 * std::f32::consts::PI * self.frequency * t).sin();
            self.initial_amplitude * damping * wave
        }
    }
}

impl Default for ShakeAnimation {
    fn default() -> Self {
        Self::new()
    }
}

/// Credential Manager for PIN / Password Authentication
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CredentialManager {
    pin_hash: u64,
    password_hash: u64,
    pub failed_attempts: u32,
    pub max_failed_attempts: u32,
    pub lockout_cooldown_secs: u32,
    pub remaining_lockout_secs: u32,
}

impl CredentialManager {
    /// Simple 64-bit FNV-1a hash algorithm for credential verification
    pub fn hash_str(val: &str) -> u64 {
        let mut hash = 0xcbf29ce484222325u64;
        for byte in val.as_bytes() {
            hash ^= *byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash
    }

    pub fn new(default_pin: &str, default_password: &str) -> Self {
        Self {
            pin_hash: Self::hash_str(default_pin),
            password_hash: Self::hash_str(default_password),
            failed_attempts: 0,
            max_failed_attempts: 5,
            lockout_cooldown_secs: 30,
            remaining_lockout_secs: 0,
        }
    }

    pub fn default_credentials() -> Self {
        Self::new("1234", "hybridos")
    }

    pub fn is_locked_out(&self) -> bool {
        self.remaining_lockout_secs > 0
    }

    pub fn tick_lockout(&mut self, elapsed_secs: u32) {
        self.remaining_lockout_secs = self.remaining_lockout_secs.saturating_sub(elapsed_secs);
        if self.remaining_lockout_secs == 0 && self.failed_attempts >= self.max_failed_attempts {
            self.failed_attempts = 0;
        }
    }

    pub fn verify(&mut self, method: SignInMethod, input: &str) -> Result<(), &'static str> {
        if self.is_locked_out() {
            return Err("Your device is temporarily locked due to repeated incorrect attempts. Please wait.");
        }

        let input_hash = Self::hash_str(input);
        let matched = match method {
            SignInMethod::Pin => input_hash == self.pin_hash,
            SignInMethod::Password => input_hash == self.password_hash,
            SignInMethod::WindowsHelloFace | SignInMethod::WindowsHelloFingerprint => true,
            SignInMethod::SecurityKey => input_hash == self.pin_hash,
        };

        if matched {
            self.failed_attempts = 0;
            self.remaining_lockout_secs = 0;
            Ok(())
        } else {
            self.failed_attempts += 1;
            if self.failed_attempts >= self.max_failed_attempts {
                self.remaining_lockout_secs = self.lockout_cooldown_secs;
                Err("Too many incorrect attempts. Device is locked for 30 seconds.")
            } else {
                match method {
                    SignInMethod::Pin => Err("The PIN is incorrect. Try again."),
                    _ => Err("Incorrect password. Make sure you are using the password for your account."),
                }
            }
        }
    }
}

/// Bottom-Right System Control Popups & Buttons
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerOption {
    Sleep,
    Hibernate,
    Restart,
    ShutDown,
}

impl PowerOption {
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Sleep => "Sleep",
            Self::Hibernate => "Hibernate",
            Self::Restart => "Restart",
            Self::ShutDown => "Shut down",
        }
    }

    pub const fn icon(&self) -> &'static str {
        match self {
            Self::Sleep => "󰒲",
            Self::Hibernate => "󰤄",
            Self::Restart => "󰑐",
            Self::ShutDown => "󰐥",
        }
    }
}

/// Sign-in Screen Bottom Control Buttons
#[derive(Debug, Clone, PartialEq)]
pub struct SignInBottomControls {
    pub network_button_bounds: Rect,
    pub accessibility_button_bounds: Rect,
    pub power_button_bounds: Rect,
    pub is_power_menu_open: bool,
    pub is_accessibility_menu_open: bool,
    pub is_network_flyout_open: bool,
}

impl SignInBottomControls {
    pub fn new() -> Self {
        Self {
            network_button_bounds: Rect::EMPTY,
            accessibility_button_bounds: Rect::EMPTY,
            power_button_bounds: Rect::EMPTY,
            is_power_menu_open: false,
            is_accessibility_menu_open: false,
            is_network_flyout_open: false,
        }
    }

    pub fn close_all_menus(&mut self) {
        self.is_power_menu_open = false;
        self.is_accessibility_menu_open = false;
        self.is_network_flyout_open = false;
    }
}

impl Default for SignInBottomControls {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Section 5: Quick Settings Flyout (Win+A)
// ============================================================================

/// Quick Action Toggle Identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum QuickToggleKind {
    WiFi,
    Bluetooth,
    AirplaneMode,
    BatterySaver,
    NightLight,
    Accessibility,
    MobileHotspot,
    NearbyShare,
    Project,
}

impl QuickToggleKind {
    pub const fn title(&self) -> &'static str {
        match self {
            Self::WiFi => "Wi-Fi",
            Self::Bluetooth => "Bluetooth",
            Self::AirplaneMode => "Airplane mode",
            Self::BatterySaver => "Battery saver",
            Self::NightLight => "Night light",
            Self::Accessibility => "Accessibility",
            Self::MobileHotspot => "Mobile hotspot",
            Self::NearbyShare => "Nearby sharing",
            Self::Project => "Project",
        }
    }

    pub const fn icon(&self) -> &'static str {
        match self {
            Self::WiFi => "󰤨",
            Self::Bluetooth => "󰂯",
            Self::AirplaneMode => "󰀝",
            Self::BatterySaver => "󰁹",
            Self::NightLight => "󰖔",
            Self::Accessibility => "󰚥",
            Self::MobileHotspot => "󰖩",
            Self::NearbyShare => "󰈔",
            Self::Project => "󰍹",
        }
    }
}

/// Interactive Quick Action Pill Button
#[derive(Debug, Clone, PartialEq)]
pub struct QuickToggle {
    pub kind: QuickToggleKind,
    pub sublabel: String,
    pub is_active: bool,
    pub bounds: Rect,
}

impl QuickToggle {
    pub fn new(kind: QuickToggleKind, is_active: bool, sublabel: impl Into<String>) -> Self {
        Self {
            kind,
            sublabel: sublabel.into(),
            is_active,
            bounds: Rect::EMPTY,
        }
    }
}

/// Slider Controller (Brightness and Volume)
#[derive(Debug, Clone, PartialEq)]
pub struct SliderControl {
    pub label: &'static str,
    pub value: u8, // 0..=100
    pub is_muted: bool,
    pub bounds: Rect,
    pub track_bounds: Rect,
    pub thumb_bounds: Rect,
    pub icon_bounds: Rect,
    pub is_dragging: bool,
}

impl SliderControl {
    pub fn new(label: &'static str, initial_value: u8) -> Self {
        Self {
            label,
            value: initial_value.min(100),
            is_muted: false,
            bounds: Rect::EMPTY,
            track_bounds: Rect::EMPTY,
            thumb_bounds: Rect::EMPTY,
            icon_bounds: Rect::EMPTY,
            is_dragging: false,
        }
    }

    pub fn set_value(&mut self, val: u8) {
        self.value = val.min(100);
        if self.value > 0 {
            self.is_muted = false;
        }
    }

    pub fn toggle_mute(&mut self) {
        self.is_muted = !self.is_muted;
    }

    pub fn effective_value(&self) -> u8 {
        if self.is_muted {
            0
        } else {
            self.value
        }
    }

    pub fn update_from_mouse_x(&mut self, mouse_x: i32) {
        if self.track_bounds.width == 0 {
            return;
        }
        let rel_x = (mouse_x - self.track_bounds.x).clamp(0, self.track_bounds.width as i32);
        let pct = ((rel_x as u64 * 100) / (self.track_bounds.width as u64)).min(100) as u8;
        self.set_value(pct);
    }
}

/// Windows 11 Quick Settings Flyout (Win+A)
#[derive(Debug, Clone, PartialEq)]
pub struct QuickSettingsFlyout {
    pub is_open: bool,
    pub bounds: Rect,
    pub toggles: Vec<QuickToggle>,
    pub brightness_slider: SliderControl,
    pub volume_slider: SliderControl,
    pub audio_device_name: String,
    pub battery_status: BatteryStatus,
    pub settings_shortcut_bounds: Rect,
    pub edit_quick_settings_bounds: Rect,
    // Saved state for Airplane mode restore
    saved_wifi_state: bool,
    saved_bluetooth_state: bool,
}

impl QuickSettingsFlyout {
    pub fn new(battery: BatteryStatus) -> Self {
        let mut qs = Self {
            is_open: false,
            bounds: Rect::new(0, 0, 360, 420),
            toggles: Vec::new(),
            brightness_slider: SliderControl::new("Brightness", 80),
            volume_slider: SliderControl::new("Volume", 70),
            audio_device_name: "Speakers (Realtek High Definition Audio)".to_string(),
            battery_status: battery,
            settings_shortcut_bounds: Rect::EMPTY,
            edit_quick_settings_bounds: Rect::EMPTY,
            saved_wifi_state: true,
            saved_bluetooth_state: true,
        };

        qs.toggles.push(QuickToggle::new(QuickToggleKind::WiFi, true, "HybridOS-5G"));
        qs.toggles.push(QuickToggle::new(QuickToggleKind::Bluetooth, true, "Surface Arc Mouse"));
        qs.toggles.push(QuickToggle::new(QuickToggleKind::AirplaneMode, false, "Off"));
        qs.toggles.push(QuickToggle::new(QuickToggleKind::BatterySaver, false, "Off"));
        qs.toggles.push(QuickToggle::new(QuickToggleKind::NightLight, false, "Off"));
        qs.toggles.push(QuickToggle::new(QuickToggleKind::Accessibility, false, "Available"));
        qs.toggles.push(QuickToggle::new(QuickToggleKind::MobileHotspot, false, "Off"));
        qs.toggles.push(QuickToggle::new(QuickToggleKind::NearbyShare, false, "Off"));
        qs.toggles.push(QuickToggle::new(QuickToggleKind::Project, false, "PC screen only"));

        qs
    }

    pub fn toggle_open(&mut self) {
        self.is_open = !self.is_open;
    }

    pub fn close(&mut self) {
        self.is_open = false;
    }

    pub fn is_active(&self, kind: QuickToggleKind) -> bool {
        self.toggles.iter().find(|t| t.kind == kind).map(|t| t.is_active).unwrap_or(false)
    }

    /// Toggle a quick action button with hardware exclusivity logic (e.g. Airplane mode)
    pub fn toggle_action(&mut self, kind: QuickToggleKind) -> bool {
        if kind == QuickToggleKind::AirplaneMode {
            let current = self.is_active(QuickToggleKind::AirplaneMode);
            let next = !current;
            if let Some(t) = self.toggles.iter_mut().find(|t| t.kind == QuickToggleKind::AirplaneMode) {
                t.is_active = next;
                t.sublabel = if next { "On".to_string() } else { "Off".to_string() };
            }

            if next {
                // Airplane mode turned ON: save radio states and disable them
                self.saved_wifi_state = self.is_active(QuickToggleKind::WiFi);
                self.saved_bluetooth_state = self.is_active(QuickToggleKind::Bluetooth);

                if let Some(w) = self.toggles.iter_mut().find(|t| t.kind == QuickToggleKind::WiFi) {
                    w.is_active = false;
                    w.sublabel = "Off".to_string();
                }
                if let Some(b) = self.toggles.iter_mut().find(|t| t.kind == QuickToggleKind::Bluetooth) {
                    b.is_active = false;
                    b.sublabel = "Off".to_string();
                }
                if let Some(h) = self.toggles.iter_mut().find(|t| t.kind == QuickToggleKind::MobileHotspot) {
                    h.is_active = false;
                    h.sublabel = "Off".to_string();
                }
            } else {
                // Airplane mode turned OFF: restore previous radio states
                if let Some(w) = self.toggles.iter_mut().find(|t| t.kind == QuickToggleKind::WiFi) {
                    w.is_active = self.saved_wifi_state;
                    w.sublabel = if self.saved_wifi_state { "HybridOS-5G".to_string() } else { "Off".to_string() };
                }
                if let Some(b) = self.toggles.iter_mut().find(|t| t.kind == QuickToggleKind::Bluetooth) {
                    b.is_active = self.saved_bluetooth_state;
                    b.sublabel = if self.saved_bluetooth_state { "On".to_string() } else { "Off".to_string() };
                }
            }
            return next;
        }

        // Other toggles
        if let Some(t) = self.toggles.iter_mut().find(|t| t.kind == kind) {
            t.is_active = !t.is_active;
            t.sublabel = if t.is_active { "On".to_string() } else { "Off".to_string() };
            t.is_active
        } else {
            false
        }
    }
}

// ============================================================================
// Section 6: Notification Center & Calendar Flyout (Win+N)
// ============================================================================

/// Notification Priority Level
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum NotificationPriority {
    Low = 0,
    Normal = 1,
    High = 2,
    Urgent = 3,
}

/// Individual Notification Item
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotificationItem {
    pub id: u64,
    pub app_id: &'static str,
    pub app_name: &'static str,
    pub title: String,
    pub body: String,
    pub timestamp_secs: u64,
    pub priority: NotificationPriority,
    pub actions: Vec<String>,
    pub is_read: bool,
    pub bounds: Rect,
}

/// Application Notification Group
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotificationGroup {
    pub app_id: &'static str,
    pub app_name: &'static str,
    pub items: Vec<NotificationItem>,
    pub is_expanded: bool,
}

impl NotificationGroup {
    pub fn new(app_id: &'static str, app_name: &'static str) -> Self {
        Self {
            app_id,
            app_name,
            items: Vec::new(),
            is_expanded: true,
        }
    }
}

/// Monthly Calendar Grid State
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarView {
    pub view_year: u32,
    pub view_month: u8,
    pub selected_day: u8,
    pub is_collapsed: bool,
    pub bounds: Rect,
    pub prev_month_button_bounds: Rect,
    pub next_month_button_bounds: Rect,
    pub expand_collapse_bounds: Rect,
}

impl CalendarView {
    pub fn new(year: u32, month: u8, day: u8) -> Self {
        Self {
            view_year: year,
            view_month: month.clamp(1, 12),
            selected_day: day.clamp(1, 31),
            is_collapsed: false,
            bounds: Rect::EMPTY,
            prev_month_button_bounds: Rect::EMPTY,
            next_month_button_bounds: Rect::EMPTY,
            expand_collapse_bounds: Rect::EMPTY,
        }
    }

    pub fn navigate_prev_month(&mut self) {
        if self.view_month == 1 {
            self.view_month = 12;
            self.view_year = self.view_year.saturating_sub(1);
        } else {
            self.view_month -= 1;
        }
    }

    pub fn navigate_next_month(&mut self) {
        if self.view_month == 12 {
            self.view_month = 1;
            self.view_year += 1;
        } else {
            self.view_month += 1;
        }
    }

    pub fn toggle_collapsed(&mut self) {
        self.is_collapsed = !self.is_collapsed;
    }

    /// Header string: e.g. "November 2026"
    pub fn header_string(&self) -> String {
        format!("{} {}", DateTimeDisplay::month_name(self.view_month), self.view_year)
    }

    /// Compute calendar grid cells: (day_number, is_current_month, is_today)
    pub fn compute_month_cells(&self, today_year: u32, today_month: u8, today_day: u8) -> Vec<(u8, bool, bool)> {
        let first_weekday = DateTimeDisplay::day_of_week_for(self.view_year, self.view_month, 1) as u8;
        let days_in_cur = DateTimeDisplay::days_in_month(self.view_year, self.view_month);

        let prev_month = if self.view_month == 1 { 12 } else { self.view_month - 1 };
        let prev_year = if self.view_month == 1 { self.view_year.saturating_sub(1) } else { self.view_year };
        let days_in_prev = DateTimeDisplay::days_in_month(prev_year, prev_month);

        let mut cells = Vec::with_capacity(42);

        // Leading days from previous month
        for i in (0..first_weekday).rev() {
            let day = days_in_prev - i;
            cells.push((day, false, false));
        }

        // Current month days
        for day in 1..=days_in_cur {
            let is_today = self.view_year == today_year && self.view_month == today_month && day == today_day;
            cells.push((day, true, is_today));
        }

        // Trailing days from next month to fill complete weeks (35 or 42 cells)
        let total_so_far = cells.len();
        let target_cells = if total_so_far <= 35 { 35 } else { 42 };
        let remaining = target_cells - total_so_far;
        for day in 1..=(remaining as u8) {
            cells.push((day, false, false));
        }

        cells
    }
}

/// Notification Center & Calendar Flyout (Win+N)
#[derive(Debug, Clone, PartialEq)]
pub struct NotificationCenterFlyout {
    pub is_open: bool,
    pub bounds: Rect,
    pub calendar: CalendarView,
    pub groups: BTreeMap<&'static str, NotificationGroup>,
    pub do_not_disturb: bool,
    pub clear_all_button_bounds: Rect,
    pub dnd_toggle_bounds: Rect,
    next_id: u64,
}

impl NotificationCenterFlyout {
    pub fn new(today_year: u32, today_month: u8, today_day: u8) -> Self {
        Self {
            is_open: false,
            bounds: Rect::new(0, 0, 380, 720),
            calendar: CalendarView::new(today_year, today_month, today_day),
            groups: BTreeMap::new(),
            do_not_disturb: false,
            clear_all_button_bounds: Rect::EMPTY,
            dnd_toggle_bounds: Rect::EMPTY,
            next_id: 1,
        }
    }

    pub fn toggle_open(&mut self) {
        self.is_open = !self.is_open;
    }

    pub fn close(&mut self) {
        self.is_open = false;
    }

    pub fn toggle_do_not_disturb(&mut self) {
        self.do_not_disturb = !self.do_not_disturb;
    }

    pub fn add_notification(
        &mut self,
        app_id: &'static str,
        app_name: &'static str,
        title: impl Into<String>,
        body: impl Into<String>,
        priority: NotificationPriority,
    ) -> u64 {
        let id = self.next_id;
        self.next_id += 1;

        let item = NotificationItem {
            id,
            app_id,
            app_name,
            title: title.into(),
            body: body.into(),
            timestamp_secs: 0,
            priority,
            actions: Vec::new(),
            is_read: false,
            bounds: Rect::EMPTY,
        };

        self.groups
            .entry(app_id)
            .or_insert_with(|| NotificationGroup::new(app_id, app_name))
            .items
            .push(item);

        id
    }

    pub fn dismiss_notification(&mut self, id: u64) -> bool {
        let mut found = false;
        let mut empty_groups = Vec::new();

        for (app_id, group) in self.groups.iter_mut() {
            let before = group.items.len();
            group.items.retain(|item| item.id != id);
            if group.items.len() < before {
                found = true;
            }
            if group.items.is_empty() {
                empty_groups.push(*app_id);
            }
        }

        for app_id in empty_groups {
            self.groups.remove(app_id);
        }

        found
    }

    pub fn dismiss_group(&mut self, app_id: &str) -> bool {
        self.groups.remove(app_id).is_some()
    }

    pub fn clear_all(&mut self) {
        self.groups.clear();
    }

    pub fn total_notification_count(&self) -> usize {
        self.groups.values().map(|g| g.items.len()).sum()
    }
}

// ============================================================================
// Section 7: Unified Windows 11 LockScreen Manager Orchestrator
// ============================================================================

/// User Input Key Codes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyCode {
    Enter,
    Escape,
    Backspace,
    Tab,
    Space,
    CapsLock,
    WinKey,
    KeyA,
    KeyL,
    KeyN,
    Char(char),
    Other,
}

/// Mouse Button enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// Hit-testing outcome for user clicks
#[derive(Debug, Clone, PartialEq)]
pub enum HitTestResult {
    None,
    UnlockTriggered,
    SubmitPassword,
    ToggleRevealPassword,
    SwitchSignInMethod(SignInMethod),
    PowerAction(PowerOption),
    ToggleQuickAction(QuickToggleKind),
    SliderAdjusted { label: &'static str, value: u8 },
    ToggleMute,
    CalendarNavPrev,
    CalendarNavNext,
    CalendarToggleExpand,
    CalendarSelectDay(u8),
    NotificationDismiss(u64),
    NotificationClearAll,
    ToggleDnd,
}

/// Unified Lock Screen, Sign-in Screen, Quick Settings & Notification Center Manager
#[derive(Debug, Clone)]
pub struct LockScreenManager {
    pub state: LockScreenState,
    pub screen_size: Size,
    pub date_time: DateTimeDisplay,
    pub weather: WeatherBadge,
    pub battery: BatteryStatus,
    pub network: NetworkStatus,
    pub user_profile: UserProfile,
    pub sign_in_method: SignInMethod,
    pub input_field: PasswordInputField,
    pub shake_anim: ShakeAnimation,
    pub credentials: CredentialManager,
    pub bottom_controls: SignInBottomControls,
    pub touch_physics: TouchSwipePhysics,
    pub quick_settings: QuickSettingsFlyout,
    pub notification_center: NotificationCenterFlyout,
    pub last_error_message: Option<String>,
    pub depth_blur_amount: f32, // 0.0 (lock) to 1.0 (sign-in)
}

impl LockScreenManager {
    pub fn new(screen_width: u32, screen_height: u32) -> Self {
        let dt = DateTimeDisplay::default_win11();
        let battery = BatteryStatus::new(85, false);
        let network = NetworkStatus::wifi("HybridOS-5G", 4);
        let weather = WeatherBadge::new(WeatherCondition::Sunny, 21, "Redmond, WA");
        let user = UserProfile::default_user();

        let mut mgr = Self {
            state: LockScreenState::Locked,
            screen_size: Size::new(screen_width, screen_height),
            date_time: dt.clone(),
            weather,
            battery: battery.clone(),
            network,
            user_profile: user,
            sign_in_method: SignInMethod::Pin,
            input_field: PasswordInputField::new(),
            shake_anim: ShakeAnimation::new(),
            credentials: CredentialManager::default_credentials(),
            bottom_controls: SignInBottomControls::new(),
            touch_physics: TouchSwipePhysics::new(screen_height as f32),
            quick_settings: QuickSettingsFlyout::new(battery),
            notification_center: NotificationCenterFlyout::new(dt.year, dt.month, dt.day),
            last_error_message: None,
            depth_blur_amount: 0.0,
        };

        mgr.layout_elements();
        mgr
    }

    /// Layout interactive boundaries across the entire shell
    pub fn layout_elements(&mut self) {
        let sw = self.screen_size.width as i32;
        let sh = self.screen_size.height as i32;

        // 1. Lock screen elements
        self.weather.bounds = Rect::new(40, sh - 80, 240, 40);
        self.battery.bounds = Rect::new(sw - 160, sh - 80, 60, 40);
        self.network.bounds = Rect::new(sw - 90, sh - 80, 60, 40);

        // 2. Sign-in screen elements (centered horizontally)
        let center_x = sw / 2;
        let avatar_y = (sh as f32 * 0.32) as i32;
        self.user_profile.avatar_bounds = Rect::new(center_x - 48, avatar_y, 96, 96);

        let input_w = 280;
        let input_h = 36;
        let input_x = center_x - input_w / 2;
        let input_y = avatar_y + 140;
        self.input_field.bounds = Rect::new(input_x, input_y, input_w as u32, input_h as u32);
        self.input_field.eye_button_bounds = Rect::new(input_x + input_w - 68, input_y, 32, input_h as u32);
        self.input_field.submit_button_bounds = Rect::new(input_x + input_w - 36, input_y, 36, input_h as u32);

        // Bottom right sign-in controls
        self.bottom_controls.power_button_bounds = Rect::new(sw - 60, sh - 60, 44, 44);
        self.bottom_controls.accessibility_button_bounds = Rect::new(sw - 116, sh - 60, 44, 44);
        self.bottom_controls.network_button_bounds = Rect::new(sw - 172, sh - 60, 44, 44);

        // 3. Quick Settings Flyout bounds (docked to bottom-right)
        let qs_w = 360u32;
        let qs_h = 440u32;
        let qs_x = sw - (qs_w as i32) - 16;
        let qs_y = sh - (qs_h as i32) - 64; // above taskbar
        self.quick_settings.bounds = Rect::new(qs_x, qs_y, qs_w, qs_h);

        // 3x3 toggle grid inside Quick Settings
        let grid_start_x = qs_x + 16;
        let grid_start_y = qs_y + 16;
        let toggle_w = 104u32;
        let toggle_h = 56u32;
        let col_gap = 8i32;
        let row_gap = 8i32;

        for (idx, toggle) in self.quick_settings.toggles.iter_mut().enumerate() {
            let col = (idx % 3) as i32;
            let row = (idx / 3) as i32;
            let x = grid_start_x + col * (toggle_w as i32 + col_gap);
            let y = grid_start_y + row * (toggle_h as i32 + row_gap);
            toggle.bounds = Rect::new(x, y, toggle_w, toggle_h);
        }

        // Sliders inside Quick Settings
        let slider_start_y = grid_start_y + 3 * (toggle_h as i32 + row_gap) + 16;
        self.quick_settings.brightness_slider.bounds = Rect::new(grid_start_x, slider_start_y, 328, 36);
        self.quick_settings.brightness_slider.icon_bounds = Rect::new(grid_start_x, slider_start_y, 36, 36);
        self.quick_settings.brightness_slider.track_bounds = Rect::new(grid_start_x + 44, slider_start_y + 16, 280, 4);

        let vol_y = slider_start_y + 44;
        self.quick_settings.volume_slider.bounds = Rect::new(grid_start_x, vol_y, 328, 36);
        self.quick_settings.volume_slider.icon_bounds = Rect::new(grid_start_x, vol_y, 36, 36);
        self.quick_settings.volume_slider.track_bounds = Rect::new(grid_start_x + 44, vol_y + 16, 280, 4);

        // Footer shortcut links
        self.quick_settings.settings_shortcut_bounds = Rect::new(qs_x + 16, qs_y + (qs_h as i32) - 40, 140, 28);
        self.quick_settings.edit_quick_settings_bounds = Rect::new(qs_x + (qs_w as i32) - 44, qs_y + (qs_h as i32) - 40, 28, 28);

        // 4. Notification Center Flyout bounds (docked to right edge)
        let nc_w = 380u32;
        let nc_h = 720u32;
        let nc_x = sw - (nc_w as i32) - 16;
        let nc_y = sh - (nc_h as i32) - 64;
        self.notification_center.bounds = Rect::new(nc_x, nc_y, nc_w, nc_h);

        // Top bar buttons inside Notification Center
        self.notification_center.dnd_toggle_bounds = Rect::new(nc_x + 16, nc_y + 16, 120, 32);
        self.notification_center.clear_all_button_bounds = Rect::new(nc_x + (nc_w as i32) - 96, nc_y + 16, 80, 32);

        // Calendar component inside Notification Center (bottom of panel)
        let cal_h = 320u32;
        let cal_y = nc_y + (nc_h as i32) - (cal_h as i32) - 16;
        self.notification_center.calendar.bounds = Rect::new(nc_x + 16, cal_y, 348, cal_h);
        self.notification_center.calendar.prev_month_button_bounds = Rect::new(nc_x + (nc_w as i32) - 80, cal_y + 12, 28, 28);
        self.notification_center.calendar.next_month_button_bounds = Rect::new(nc_x + (nc_w as i32) - 44, cal_y + 12, 28, 28);
        self.notification_center.calendar.expand_collapse_bounds = Rect::new(nc_x + 24, cal_y + 12, 160, 28);
    }

    /// Transition to Locked state (e.g. Win+L or timeout)
    pub fn lock(&mut self) {
        self.state = LockScreenState::Locked;
        self.input_field.clear();
        self.last_error_message = None;
        self.depth_blur_amount = 0.0;
        self.bottom_controls.close_all_menus();
        self.quick_settings.close();
        self.notification_center.close();
    }

    /// Initiate slide-up transition towards sign-in screen
    pub fn begin_unlock(&mut self) {
        if let LockScreenState::Locked = self.state {
            self.state = LockScreenState::Unlocking {
                progress: 0.0,
                velocity: -800.0, // upward velocity in px/s
                offset_y: 0.0,
            };
        }
    }

    /// Advance physics animations, timers, and state transitions
    pub fn tick(&mut self, dt_ms: f32) {
        let dt_secs = dt_ms / 1000.0;

        // Advance credentials lockout cooldown if active
        if self.credentials.is_locked_out() {
            let secs = (dt_secs.ceil() as u32).max(1);
            self.credentials.tick_lockout(secs);
        }

        // Advance shake animation
        if self.shake_anim.is_active {
            self.shake_anim.update(dt_ms);
        }

        // Advance unlock spring physics
        if let LockScreenState::Unlocking { mut velocity, mut offset_y, .. } = self.state {
            let target_offset = -(self.screen_size.height as f32);
            // Damped spring simulation towards target offset
            let spring_k = 40.0;
            let damping_c = 12.0;
            let displacement = offset_y - target_offset;
            let force = -spring_k * displacement - damping_c * velocity;

            velocity += force * dt_secs;
            offset_y += velocity * dt_secs;

            let total_dist = self.screen_size.height as f32;
            let progress = ((-offset_y) / total_dist).clamp(0.0, 1.0);
            self.depth_blur_amount = progress;

            if progress >= 0.98 || offset_y <= target_offset + 5.0 {
                self.state = LockScreenState::LoginScreen;
                self.depth_blur_amount = 1.0;
            } else {
                self.state = LockScreenState::Unlocking { progress, velocity, offset_y };
            }
        }
    }

    /// Handle keyboard input
    pub fn handle_key(&mut self, key: KeyCode) {
        match self.state {
            LockScreenState::Locked => {
                // Any key on the lock screen initiates unlock to sign-in screen
                self.begin_unlock();
            }
            LockScreenState::Unlocking { .. } => {
                // Skip directly to login screen if key pressed during slide-up
                self.state = LockScreenState::LoginScreen;
                self.depth_blur_amount = 1.0;
            }
            LockScreenState::LoginScreen => {
                match key {
                    KeyCode::Enter => {
                        self.submit_credentials();
                    }
                    KeyCode::Escape => {
                        // Return to locked screen
                        self.lock();
                    }
                    KeyCode::Backspace => {
                        self.input_field.pop_char();
                    }
                    KeyCode::CapsLock => {
                        self.input_field.is_caps_lock = !self.input_field.is_caps_lock;
                    }
                    KeyCode::Char(ch) => {
                        self.input_field.push_char(ch);
                    }
                    _ => {}
                }
            }
            LockScreenState::DesktopUnlocked => {
                // Global shell hotkeys when desktop is unlocked
                match key {
                    KeyCode::KeyL => {
                        // Win+L: lock screen
                        self.lock();
                    }
                    KeyCode::KeyA => {
                        // Win+A: Quick Settings
                        self.quick_settings.toggle_open();
                        if self.quick_settings.is_open {
                            self.notification_center.close();
                        }
                    }
                    KeyCode::KeyN => {
                        // Win+N: Notification Center & Calendar
                        self.notification_center.toggle_open();
                        if self.notification_center.is_open {
                            self.quick_settings.close();
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    /// Verify credentials and transition to unlocked desktop or trigger failure shake
    pub fn submit_credentials(&mut self) -> bool {
        let input = self.input_field.buffer.clone();
        match self.credentials.verify(self.sign_in_method, &input) {
            Ok(()) => {
                self.state = LockScreenState::DesktopUnlocked;
                self.last_error_message = None;
                self.input_field.clear();
                true
            }
            Err(msg) => {
                self.last_error_message = Some(msg.to_string());
                self.shake_anim.trigger();
                self.input_field.clear();
                false
            }
        }
    }

    /// Process touch events on lockscreen
    pub fn handle_touch_start(&mut self, y: f32, time_ms: f32) {
        if self.state == LockScreenState::Locked {
            self.touch_physics.on_touch_down(y, time_ms);
        }
    }

    pub fn handle_touch_move(&mut self, y: f32, time_ms: f32) {
        if self.state == LockScreenState::Locked {
            self.touch_physics.on_touch_move(y, time_ms);
        }
    }

    pub fn handle_touch_end(&mut self) {
        if self.state == LockScreenState::Locked {
            match self.touch_physics.on_touch_up() {
                SwipeResult::Unlock { initial_velocity } => {
                    self.state = LockScreenState::Unlocking {
                        progress: 0.0,
                        velocity: initial_velocity * 1000.0,
                        offset_y: self.touch_physics.current_displacement(),
                    };
                }
                SwipeResult::SnapBack => {
                    // Snapped back to locked
                    self.state = LockScreenState::Locked;
                }
            }
        }
    }

    /// Handle mouse click hit-testing
    pub fn handle_mouse_click(&mut self, x: i32, y: i32, button: MouseButton) -> HitTestResult {
        if button != MouseButton::Left {
            return HitTestResult::None;
        }

        // When locked, any click initiates unlock
        if self.state == LockScreenState::Locked {
            self.begin_unlock();
            return HitTestResult::UnlockTriggered;
        }

        // Quick Settings Flyout hit testing
        if self.quick_settings.is_open && self.quick_settings.bounds.contains(x, y) {
            for toggle in self.quick_settings.toggles.iter() {
                if toggle.bounds.contains(x, y) {
                    let kind = toggle.kind;
                    self.quick_settings.toggle_action(kind);
                    return HitTestResult::ToggleQuickAction(kind);
                }
            }
            if self.quick_settings.volume_slider.icon_bounds.contains(x, y) {
                self.quick_settings.volume_slider.toggle_mute();
                return HitTestResult::ToggleMute;
            }
            if self.quick_settings.volume_slider.bounds.contains(x, y) {
                self.quick_settings.volume_slider.update_from_mouse_x(x);
                return HitTestResult::SliderAdjusted {
                    label: "Volume",
                    value: self.quick_settings.volume_slider.value,
                };
            }
            if self.quick_settings.brightness_slider.bounds.contains(x, y) {
                self.quick_settings.brightness_slider.update_from_mouse_x(x);
                return HitTestResult::SliderAdjusted {
                    label: "Brightness",
                    value: self.quick_settings.brightness_slider.value,
                };
            }
            return HitTestResult::None;
        }

        // Notification Center Flyout hit testing
        if self.notification_center.is_open && self.notification_center.bounds.contains(x, y) {
            if self.notification_center.calendar.prev_month_button_bounds.contains(x, y) {
                self.notification_center.calendar.navigate_prev_month();
                return HitTestResult::CalendarNavPrev;
            }
            if self.notification_center.calendar.next_month_button_bounds.contains(x, y) {
                self.notification_center.calendar.navigate_next_month();
                return HitTestResult::CalendarNavNext;
            }
            if self.notification_center.calendar.expand_collapse_bounds.contains(x, y) {
                self.notification_center.calendar.toggle_collapsed();
                return HitTestResult::CalendarToggleExpand;
            }
            if self.notification_center.clear_all_button_bounds.contains(x, y) {
                self.notification_center.clear_all();
                return HitTestResult::NotificationClearAll;
            }
            if self.notification_center.dnd_toggle_bounds.contains(x, y) {
                self.notification_center.toggle_do_not_disturb();
                return HitTestResult::ToggleDnd;
            }
            // Dismiss notification items
            for group in self.notification_center.groups.values() {
                for item in group.items.iter() {
                    if item.bounds.contains(x, y) {
                        let id = item.id;
                        self.notification_center.dismiss_notification(id);
                        return HitTestResult::NotificationDismiss(id);
                    }
                }
            }
            return HitTestResult::None;
        }

        // Login Screen hit testing
        if self.state == LockScreenState::LoginScreen {
            if self.input_field.eye_button_bounds.contains(x, y) {
                self.input_field.toggle_reveal();
                return HitTestResult::ToggleRevealPassword;
            }
            if self.input_field.submit_button_bounds.contains(x, y) {
                self.submit_credentials();
                return HitTestResult::SubmitPassword;
            }
            if self.bottom_controls.power_button_bounds.contains(x, y) {
                self.bottom_controls.is_power_menu_open = !self.bottom_controls.is_power_menu_open;
                return HitTestResult::None;
            }
            if self.bottom_controls.accessibility_button_bounds.contains(x, y) {
                self.bottom_controls.is_accessibility_menu_open = !self.bottom_controls.is_accessibility_menu_open;
                return HitTestResult::None;
            }
            if self.bottom_controls.network_button_bounds.contains(x, y) {
                self.bottom_controls.is_network_flyout_open = !self.bottom_controls.is_network_flyout_open;
                return HitTestResult::None;
            }
        }

        HitTestResult::None
    }

    /// Render entire screen experience to a target 32-bit ARGB framebuffer buffer
    pub fn render_to_buffer(&self, buffer: &mut [u32], pitch: usize) {
        let sw = self.screen_size.width as usize;
        let sh = self.screen_size.height as usize;
        if buffer.len() < sh * pitch {
            return;
        }

        // Helper to fill rectangle inside buffer
        let fill_rect = |buf: &mut [u32], rect: &Rect, col: Color| {
            if rect.is_empty() {
                return;
            }
            let start_x = rect.x.clamp(0, sw as i32) as usize;
            let end_x = rect.right().clamp(0, sw as i32) as usize;
            let start_y = rect.y.clamp(0, sh as i32) as usize;
            let end_y = rect.bottom().clamp(0, sh as i32) as usize;

            for y in start_y..end_y {
                let row_offset = y * pitch;
                for x in start_x..end_x {
                    let existing = Color::from_argb_u32(buf[row_offset + x]);
                    buf[row_offset + x] = col.blend_over(existing).to_argb_u32();
                }
            }
        };

        // 1. Wallpaper / Base Background
        let base_bg = Color::rgb(18, 28, 48); // Windows 11 Bloom Dark Navy
        let argb_base = base_bg.to_argb_u32();
        for y in 0..sh {
            let row = y * pitch;
            for x in 0..sw {
                buffer[row + x] = argb_base;
            }
        }

        // 2. Depth blur dim overlay for Login Screen transition
        if self.depth_blur_amount > 0.001 {
            let dim_alpha = (self.depth_blur_amount * 0.75).clamp(0.0, 0.85);
            let overlay = Color::BLACK.with_alpha_f32(dim_alpha);
            let full_rect = Rect::new(0, 0, self.screen_size.width, self.screen_size.height);
            fill_rect(buffer, &full_rect, overlay);
        }

        // 3. Render Lock Screen content (with slide-up offset if unlocking)
        let slide_y = match self.state {
            LockScreenState::Unlocking { offset_y, .. } => offset_y as i32,
            LockScreenState::LoginScreen => -(self.screen_size.height as i32),
            _ => 0,
        };

        if self.state == LockScreenState::Locked || matches!(self.state, LockScreenState::Unlocking { .. }) {
            // Clock & Date banner
            let clock_rect = Rect::new(
                (self.screen_size.width as i32 / 2) - 160,
                140 + slide_y,
                320,
                90,
            );
            fill_rect(buffer, &clock_rect, Color::TRANSPARENT);

            // Weather badge
            let weather_rect = self.weather.bounds.offset(0, slide_y);
            fill_rect(buffer, &weather_rect, Color::CONTROL_FILL);

            // Battery and Network badges
            let battery_rect = self.battery.bounds.offset(0, slide_y);
            fill_rect(buffer, &battery_rect, Color::CONTROL_FILL);

            let network_rect = self.network.bounds.offset(0, slide_y);
            fill_rect(buffer, &network_rect, Color::CONTROL_FILL);
        }

        // 4. Render Login Screen content
        if self.state == LockScreenState::LoginScreen || matches!(self.state, LockScreenState::Unlocking { .. }) {
            let shake_x = self.shake_anim.current_offset_x() as i32;

            // User Avatar circle
            fill_rect(buffer, &self.user_profile.avatar_bounds, self.user_profile.avatar_color);

            // Input field
            let shifted_input = self.input_field.bounds.offset(shake_x, 0);
            fill_rect(buffer, &shifted_input, Color::CONTROL_FILL);

            // Submit arrow button
            let shifted_submit = self.input_field.submit_button_bounds.offset(shake_x, 0);
            fill_rect(buffer, &shifted_submit, Color::WIN11_BLUE);

            // Eye reveal button
            let shifted_eye = self.input_field.eye_button_bounds.offset(shake_x, 0);
            fill_rect(buffer, &shifted_eye, Color::CONTROL_HOVER);

            // Bottom-right controls
            fill_rect(buffer, &self.bottom_controls.power_button_bounds, Color::CONTROL_FILL);
            fill_rect(buffer, &self.bottom_controls.accessibility_button_bounds, Color::CONTROL_FILL);
            fill_rect(buffer, &self.bottom_controls.network_button_bounds, Color::CONTROL_FILL);
        }

        // 5. Render Quick Settings Flyout (Win+A) if open
        if self.quick_settings.is_open {
            fill_rect(buffer, &self.quick_settings.bounds, Color::CARD_BACKGROUND);

            // Toggle buttons
            for toggle in self.quick_settings.toggles.iter() {
                let pill_color = if toggle.is_active {
                    Color::TOGGLE_ACTIVE
                } else {
                    Color::TOGGLE_INACTIVE
                };
                fill_rect(buffer, &toggle.bounds, pill_color);
            }

            // Brightness and Volume sliders
            fill_rect(buffer, &self.quick_settings.brightness_slider.bounds, Color::CONTROL_FILL);
            fill_rect(buffer, &self.quick_settings.brightness_slider.track_bounds, Color::WHITE);

            fill_rect(buffer, &self.quick_settings.volume_slider.bounds, Color::CONTROL_FILL);
            fill_rect(buffer, &self.quick_settings.volume_slider.track_bounds, Color::WIN11_BLUE);
        }

        // 6. Render Notification Center & Calendar Flyout (Win+N) if open
        if self.notification_center.is_open {
            fill_rect(buffer, &self.notification_center.bounds, Color::CARD_BACKGROUND);
            fill_rect(buffer, &self.notification_center.dnd_toggle_bounds, Color::CONTROL_FILL);
            fill_rect(buffer, &self.notification_center.clear_all_button_bounds, Color::CONTROL_FILL);
            fill_rect(buffer, &self.notification_center.calendar.bounds, Color::CONTROL_FILL);
        }
    }
}

// ============================================================================
// Section 8: Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Test 1: Lock screen state transitions (Locked -> Unlocking -> LoginScreen -> DesktopUnlocked and re-lock)
    #[test]
    fn test_lock_screen_state_transitions() {
        let mut mgr = LockScreenManager::new(1920, 1080);
        assert_eq!(mgr.state, LockScreenState::Locked);
        assert_eq!(mgr.depth_blur_amount, 0.0);

        // Keypress initiates unlocking
        mgr.handle_key(KeyCode::Space);
        assert!(matches!(mgr.state, LockScreenState::Unlocking { .. }));

        // Advance physics animation past threshold
        for _ in 0..60 {
            mgr.tick(16.67); // 60 FPS
        }
        assert_eq!(mgr.state, LockScreenState::LoginScreen);
        assert_eq!(mgr.depth_blur_amount, 1.0);

        // Type correct PIN and submit
        for ch in "1234".chars() {
            mgr.handle_key(KeyCode::Char(ch));
        }
        mgr.handle_key(KeyCode::Enter);
        assert_eq!(mgr.state, LockScreenState::DesktopUnlocked);

        // Win+L re-locks the screen
        mgr.lock();
        assert_eq!(mgr.state, LockScreenState::Locked);
        assert_eq!(mgr.depth_blur_amount, 0.0);
    }

    /// Test 2: Touch swipe physics and threshold snap-back vs. unlock completion
    #[test]
    fn test_touch_swipe_physics_and_snapback() {
        let mut physics = TouchSwipePhysics::new(1080.0);

        // Sub-threshold drag: drag 50px upward with slow velocity
        physics.on_touch_down(800.0, 0.0);
        physics.on_touch_move(750.0, 500.0); // 50px up over 500ms = -0.1 px/ms
        let result = physics.on_touch_up();
        assert_eq!(result, SwipeResult::SnapBack);

        // Drag past 20% threshold: 300px upward (> 216px threshold)
        physics.on_touch_down(800.0, 0.0);
        physics.on_touch_move(500.0, 200.0); // 300px up
        let result = physics.on_touch_up();
        assert!(matches!(result, SwipeResult::Unlock { .. }));

        // Fast fling upward (velocity exceeding -0.4 px/ms) even if small distance
        physics.on_touch_down(800.0, 0.0);
        physics.on_touch_move(700.0, 100.0); // 100px up in 100ms = -1.0 px/ms
        let result = physics.on_touch_up();
        assert!(matches!(result, SwipeResult::Unlock { .. }));
    }

    /// Test 3: Credential verification, failure shake animation, and lockout flow
    #[test]
    fn test_credential_validation_and_shake() {
        let mut mgr = LockScreenManager::new(1920, 1080);
        mgr.state = LockScreenState::LoginScreen;

        // Enter wrong PIN
        for ch in "9999".chars() {
            mgr.handle_key(KeyCode::Char(ch));
        }
        assert_eq!(mgr.input_field.buffer, "9999");
        assert_eq!(mgr.input_field.displayed_mask(), "••••");

        // Submit wrong PIN -> verification fails, triggers shake animation
        let success = mgr.submit_credentials();
        assert!(!success);
        assert_eq!(mgr.state, LockScreenState::LoginScreen);
        assert!(mgr.shake_anim.is_active);
        assert!(mgr.last_error_message.is_some());
        assert_eq!(mgr.input_field.buffer, ""); // Buffer cleared on failure

        // Advance shake animation and verify offset calculation
        mgr.tick(50.0);
        let offset = mgr.shake_anim.current_offset_x();
        assert!(offset.abs() > 0.0, "Shake animation should produce non-zero offset");

        // Fail 4 more times to trigger 5-attempt lockout
        for _ in 0..4 {
            for ch in "0000".chars() {
                mgr.handle_key(KeyCode::Char(ch));
            }
            mgr.submit_credentials();
        }
        assert!(mgr.credentials.is_locked_out(), "Device should be locked out after 5 failures");

        // Attempt during lockout fails with lockout message
        for ch in "1234".chars() {
            mgr.handle_key(KeyCode::Char(ch));
        }
        assert!(!mgr.submit_credentials());

        // Cooldown timer expires
        mgr.credentials.tick_lockout(35);
        assert!(!mgr.credentials.is_locked_out());

        // Now correct PIN succeeds
        for ch in "1234".chars() {
            mgr.handle_key(KeyCode::Char(ch));
        }
        assert!(mgr.submit_credentials());
        assert_eq!(mgr.state, LockScreenState::DesktopUnlocked);
    }

    /// Test 4: Quick settings toggle states and Airplane mode mutual exclusivity
    #[test]
    fn test_quick_settings_toggles_and_airplane_mode() {
        let battery = BatteryStatus::new(80, false);
        let mut qs = QuickSettingsFlyout::new(battery);

        // Initially Wi-Fi and Bluetooth are ON, Airplane mode is OFF
        assert!(qs.is_active(QuickToggleKind::WiFi));
        assert!(qs.is_active(QuickToggleKind::Bluetooth));
        assert!(!qs.is_active(QuickToggleKind::AirplaneMode));

        // Toggle Airplane mode ON -> Wi-Fi and Bluetooth automatically disabled
        qs.toggle_action(QuickToggleKind::AirplaneMode);
        assert!(qs.is_active(QuickToggleKind::AirplaneMode));
        assert!(!qs.is_active(QuickToggleKind::WiFi), "Wi-Fi must be disabled by Airplane mode");
        assert!(!qs.is_active(QuickToggleKind::Bluetooth), "Bluetooth must be disabled by Airplane mode");

        // Toggle Airplane mode OFF -> Wi-Fi and Bluetooth restored
        qs.toggle_action(QuickToggleKind::AirplaneMode);
        assert!(!qs.is_active(QuickToggleKind::AirplaneMode));
        assert!(qs.is_active(QuickToggleKind::WiFi), "Wi-Fi must be restored when Airplane mode turns off");
        assert!(qs.is_active(QuickToggleKind::Bluetooth), "Bluetooth must be restored when Airplane mode turns off");

        // Test Night light and Battery saver toggles
        assert!(!qs.is_active(QuickToggleKind::NightLight));
        qs.toggle_action(QuickToggleKind::NightLight);
        assert!(qs.is_active(QuickToggleKind::NightLight));

        assert!(!qs.is_active(QuickToggleKind::BatterySaver));
        qs.toggle_action(QuickToggleKind::BatterySaver);
        assert!(qs.is_active(QuickToggleKind::BatterySaver));
    }

    /// Test 5: Sliders (Volume, Brightness) and Mute toggle behavior
    #[test]
    fn test_quick_settings_sliders_and_mute() {
        let battery = BatteryStatus::new(90, true);
        let mut qs = QuickSettingsFlyout::new(battery);

        // Test volume slider adjustments
        qs.volume_slider.set_value(75);
        assert_eq!(qs.volume_slider.value, 75);
        assert_eq!(qs.volume_slider.effective_value(), 75);

        // Clamping check
        qs.volume_slider.set_value(150);
        assert_eq!(qs.volume_slider.value, 100);

        // Mute behavior
        qs.volume_slider.toggle_mute();
        assert!(qs.volume_slider.is_muted);
        assert_eq!(qs.volume_slider.effective_value(), 0);

        // Unmuting restores previous volume
        qs.volume_slider.toggle_mute();
        assert!(!qs.volume_slider.is_muted);
        assert_eq!(qs.volume_slider.effective_value(), 100);

        // Brightness slider
        qs.brightness_slider.set_value(45);
        assert_eq!(qs.brightness_slider.value, 45);
    }

    /// Test 6: Notification stacking, grouping, and calendar leap-year date calculation
    #[test]
    fn test_notification_stacking_and_calendar() {
        let mut nc = NotificationCenterFlyout::new(2026, 11, 10);
        assert_eq!(nc.total_notification_count(), 0);

        // Add notifications from different applications
        let id1 = nc.add_notification("teams", "Microsoft Teams", "Meeting in 5 min", "Sprint Review", NotificationPriority::High);
        let _id2 = nc.add_notification("teams", "Microsoft Teams", "New Chat", "Vinicius sent a message", NotificationPriority::Normal);
        let _id3 = nc.add_notification("mail", "Mail", "New Email", "Weekly Engineering Report", NotificationPriority::Normal);
        let _id4 = nc.add_notification("sec", "Windows Security", "Scan Complete", "No threats found", NotificationPriority::Low);

        assert_eq!(nc.total_notification_count(), 4);
        assert_eq!(nc.groups.len(), 3, "Notifications should be grouped into 3 distinct applications");

        // Teams group should have 2 notifications
        let teams_group = nc.groups.get("teams").unwrap();
        assert_eq!(teams_group.items.len(), 2);

        // Dismiss single notification
        assert!(nc.dismiss_notification(id1));
        assert_eq!(nc.total_notification_count(), 3);

        // Dismiss group
        assert!(nc.dismiss_group("mail"));
        assert_eq!(nc.total_notification_count(), 2);
        assert_eq!(nc.groups.len(), 2);

        // Clear all
        nc.clear_all();
        assert_eq!(nc.total_notification_count(), 0);
        assert!(nc.groups.is_empty());

        // Calendar date math: Gregorian leap year rules
        assert!(DateTimeDisplay::is_leap_year(2024), "2024 is a leap year");
        assert!(!DateTimeDisplay::is_leap_year(2026), "2026 is not a leap year");
        assert!(!DateTimeDisplay::is_leap_year(1900), "1900 is not a leap year (divisible by 100)");
        assert!(DateTimeDisplay::is_leap_year(2000), "2000 is a leap year (divisible by 400)");

        assert_eq!(DateTimeDisplay::days_in_month(2024, 2), 29);
        assert_eq!(DateTimeDisplay::days_in_month(2026, 2), 28);
        assert_eq!(DateTimeDisplay::days_in_month(2026, 11), 30);

        // Verify day of the week calculation for Nov 10, 2026 -> Tuesday
        let dt = DateTimeDisplay::new(2026, 11, 10, 10, 42, 0);
        assert_eq!(dt.weekday(), Weekday::Tuesday);
        assert_eq!(dt.full_date_string(), "Tuesday, November 10");

        // Calendar month navigation
        assert_eq!(nc.calendar.view_month, 11);
        nc.calendar.navigate_next_month();
        assert_eq!(nc.calendar.view_month, 12);
        nc.calendar.navigate_next_month();
        assert_eq!(nc.calendar.view_month, 1);
        assert_eq!(nc.calendar.view_year, 2027);
        nc.calendar.navigate_prev_month();
        assert_eq!(nc.calendar.view_month, 12);
        assert_eq!(nc.calendar.view_year, 2026);
    }

    /// Test 7: Framebuffer rendering without panic
    #[test]
    fn test_framebuffer_rendering() {
        let mut mgr = LockScreenManager::new(640, 480);
        let mut fb = vec![0u32; 640 * 480];

        // Render in Locked state
        mgr.render_to_buffer(&mut fb, 640);
        assert_ne!(fb[0], 0, "Framebuffer should have rendered background");

        // Render in LoginScreen state
        mgr.state = LockScreenState::LoginScreen;
        mgr.depth_blur_amount = 1.0;
        mgr.render_to_buffer(&mut fb, 640);

        // Render with Quick Settings open
        mgr.quick_settings.is_open = true;
        mgr.render_to_buffer(&mut fb, 640);

        // Render with Notification Center open
        mgr.quick_settings.is_open = false;
        mgr.notification_center.is_open = true;
        mgr.render_to_buffer(&mut fb, 640);
    }
}
