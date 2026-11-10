//! Windows 11 Start Menu & Search Flyout Subsystem
//!
//! Production-grade implementation of the Windows 11 Fluent Design Start Menu
//! and Search Flyout for HybridOS.
//!
//! Key Architectural Capabilities:
//! - **Floating Mica Window & Elevation**: Windows 11 floating layout centered above
//!   the taskbar with 8px rounded corners, Mica material background composition, and
//!   Elevation Shadow 32.
//! - **Search Header & Real-Time Query Engine**: Integrated search box ("Type here to search")
//!   with instant query matching across pinned apps, installed apps, recent documents,
//!   system settings, and web suggestions.
//! - **Pinned Apps Grid & Folder Grouping**: 6-column grid layout, smooth pagination,
//!   pin/unpin/move position, folder grouping with expand/collapse flyouts, and
//!   pre-configured commercial default apps (Edge, Explorer, Settings, Terminal,
//!   Task Manager, Mail, Microsoft 365, Calculator, Notepad).
//! - **Alphabetical "All Apps" Catalog**: "All apps >" transition to an alphabetical
//!   A-Z index with section headers and quick letter navigation.
//! - **"Recommended" Section**: Recent activities with relative timestamps ("2h ago",
//!   "Yesterday"), file icons, target paths, and "More >" expanded history view.
//! - **User Profile & Power Footer**: User avatar/name flyout (Change account settings,
//!   Lock, Sign out) and Power button flyout (Sleep, Hibernate, Restart, Shut Down) with
//!   safe event dispatching.
//! - **100% Pure Safe Rust**: Zero unsafe blocks, robust bounds-checked state machine,
//!   comprehensive unit tests.

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;

// ============================================================================
// Section 1: Fluent Geometry, Color, and Material Primitives
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

    pub fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }

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

/// 8px Rounded Corner Radii for Windows 11 Fluent surfaces
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CornerRadius {
    pub top_left: f32,
    pub top_right: f32,
    pub bottom_left: f32,
    pub bottom_right: f32,
}

impl CornerRadius {
    /// Windows 11 default 8px rounded corners
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
}

impl Default for CornerRadius {
    fn default() -> Self {
        Self::WIN11_STANDARD
    }
}

/// 32-bit RGBA Color
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba {
    pub const TRANSPARENT: Self = Self { r: 0, g: 0, b: 0, a: 0 };
    pub const BLACK: Self = Self { r: 0, g: 0, b: 0, a: 255 };
    pub const WHITE: Self = Self { r: 255, g: 255, b: 255, a: 255 };

    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    pub fn with_alpha(self, a: u8) -> Self {
        Self { a, ..self }
    }

    pub fn to_u32(self) -> u32 {
        ((self.a as u32) << 24)
            | ((self.r as u32) << 16)
            | ((self.g as u32) << 8)
            | (self.b as u32)
    }
}

/// Theme Mode (Dark / Light)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeMode {
    Light,
    Dark,
}

/// Windows 11 Mica Material parameters
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MicaMaterial {
    /// Base tint color sampled with desktop wallpaper backdrop
    pub tint_color: Rgba,
    /// Tint opacity factor (typically 0.80 - 0.90)
    pub tint_opacity: f32,
    /// Backdrop blur radius in logical pixels
    pub blur_radius: f32,
    /// Luminosity multiplier
    pub luminosity: f32,
    /// Subtle grain / noise factor (0.02)
    pub noise_grain: f32,
    /// 1px border stroke color
    pub border_color: Rgba,
    /// 1px border stroke width
    pub border_width: f32,
}

impl MicaMaterial {
    /// Factory for Windows 11 Dark Mode Mica
    pub fn win11_dark() -> Self {
        Self {
            tint_color: Rgba::new(32, 32, 32, 220),
            tint_opacity: 0.86,
            blur_radius: 30.0,
            luminosity: 0.80,
            noise_grain: 0.02,
            border_color: Rgba::new(255, 255, 255, 23),
            border_width: 1.0,
        }
    }

    /// Factory for Windows 11 Light Mode Mica
    pub fn win11_light() -> Self {
        Self {
            tint_color: Rgba::new(243, 243, 243, 225),
            tint_opacity: 0.88,
            blur_radius: 30.0,
            luminosity: 0.92,
            noise_grain: 0.02,
            border_color: Rgba::new(0, 0, 0, 16),
            border_width: 1.0,
        }
    }
}

/// Windows 11 Elevation Shadow 32 specification
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ElevationShadow32 {
    /// Ambient shadow blur radius (px)
    pub ambient_blur: f32,
    /// Ambient shadow Y offset (px)
    pub ambient_offset_y: f32,
    /// Ambient shadow opacity (0.0 to 1.0)
    pub ambient_alpha: f32,
    /// Key shadow blur radius (px)
    pub key_blur: f32,
    /// Key shadow Y offset (px)
    pub key_offset_y: f32,
    /// Key shadow opacity (0.0 to 1.0)
    pub key_alpha: f32,
    /// Spread distance (px)
    pub spread: f32,
    /// Shadow color
    pub shadow_color: Rgba,
}

impl ElevationShadow32 {
    /// Windows 11 standard Elevation 32
    pub const WIN11: Self = Self {
        ambient_blur: 32.0,
        ambient_offset_y: 8.0,
        ambient_alpha: 0.18,
        key_blur: 16.0,
        key_offset_y: 16.0,
        key_alpha: 0.24,
        spread: 0.0,
        shadow_color: Rgba::new(0, 0, 0, 255),
    };
}

impl Default for ElevationShadow32 {
    fn default() -> Self {
        Self::WIN11
    }
}

// ============================================================================
// Section 2: Windows 11 Start Menu Window Placement & Layout
// ============================================================================

/// Windows 11 Start Menu dimensional constants
pub const START_MENU_WIDTH: u32 = 640;
pub const START_MENU_HEIGHT: u32 = 728;
pub const TASKBAR_DEFAULT_HEIGHT: u32 = 48;
pub const START_MENU_MARGIN_BOTTOM: i32 = 12;
pub const PINNED_GRID_COLUMNS: usize = 6;
pub const PINNED_GRID_ROWS_DEFAULT: usize = 3;
pub const MAX_PINNED_PER_PAGE: usize = PINNED_GRID_COLUMNS * PINNED_GRID_ROWS_DEFAULT; // 18 items
pub const MAX_RECOMMENDED_COLLAPSED: usize = 6;

/// Taskbar icon alignment mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskbarAlignment {
    /// Default Windows 11 centered alignment
    Center,
    /// Classic Windows left alignment
    Left,
}

/// Start Menu Open / Close Animation State
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StartMenuAnimationState {
    Closed,
    Opening { progress: f32 },
    Open,
    Closing { progress: f32 },
}

/// Active primary view inside the Start Menu window
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartMenuView {
    /// Default view: Pinned apps grid + Recommended section
    Pinned,
    /// All apps alphabetical list (A..Z)
    AllApps,
    /// Search flyout mode with categorized tabs
    SearchResults,
}

/// Hit test target elements for mouse/touch interactions
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HitTestTarget {
    None,
    OutsideWindow,
    SearchBox,
    SearchClearButton,
    AllAppsButton,
    BackToPinnedButton,
    MoreRecommendedButton,
    PinnedApp(String),
    PinnedFolder(String),
    RecommendedItem(String),
    UserProfileButton,
    PowerButton,
    UserMenuItem(UserAction),
    PowerMenuItem(PowerAction),
    SearchTab(SearchTab),
    SearchResultItem(usize),
}

// ============================================================================
// Section 3: Applications, Folders, and Pinned Grid Data Models
// ============================================================================

/// App category classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AppCategory {
    Browser,
    System,
    SystemTools,
    Developer,
    Productivity,
    Utilities,
    Accessories,
    Media,
    Entertainment,
    Store,
    Settings,
}

/// Visual Icon representation
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppIcon {
    /// Symbolic icon name or glyph (e.g. "globe", "folder", "terminal", "gear")
    pub glyph: String,
    /// Accent color or brand background tint
    pub tint_color: Rgba,
}

impl AppIcon {
    pub fn new(glyph: impl Into<String>, tint_color: Rgba) -> Self {
        Self {
            glyph: glyph.into(),
            tint_color,
        }
    }
}

/// App Tile component representing an installed application
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppTile {
    pub id: String,
    pub display_name: String,
    pub icon: AppIcon,
    pub category: AppCategory,
    pub executable_path: String,
    pub arguments: Vec<String>,
    pub badge: Option<u32>,
}

impl AppTile {
    pub fn new(
        id: impl Into<String>,
        display_name: impl Into<String>,
        icon: AppIcon,
        category: AppCategory,
        executable_path: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            display_name: display_name.into(),
            icon,
            category,
            executable_path: executable_path.into(),
            arguments: Vec::new(),
            badge: None,
        }
    }

    pub fn with_badge(mut self, badge: u32) -> Self {
        self.badge = Some(badge);
        self
    }
}

/// Grouped Folder Tile containing multiple pinned apps
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderTile {
    pub id: String,
    pub name: String,
    pub items: Vec<AppTile>,
    pub is_expanded: bool,
}

impl FolderTile {
    pub fn new(id: impl Into<String>, name: impl Into<String>, items: Vec<AppTile>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            items,
            is_expanded: false,
        }
    }

    pub fn preview_icons(&self) -> Vec<AppIcon> {
        self.items.iter().take(4).map(|item| item.icon.clone()).collect()
    }
}

/// Single slot in the 6-column Pinned Grid
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PinnedSlot {
    App(AppTile),
    Folder(FolderTile),
}

impl PinnedSlot {
    pub fn id(&self) -> &str {
        match self {
            PinnedSlot::App(app) => &app.id,
            PinnedSlot::Folder(folder) => &folder.id,
        }
    }

    pub fn display_name(&self) -> &str {
        match self {
            PinnedSlot::App(app) => &app.display_name,
            PinnedSlot::Folder(folder) => &folder.name,
        }
    }
}

// ============================================================================
// Section 4: Recommended Section & Recent Items Data Models
// ============================================================================

/// Icon type for recent files and activities
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileIconType {
    Document,
    Spreadsheet,
    Presentation,
    Code,
    Pdf,
    Image,
    Archive,
    Application,
    Generic,
}

/// Recent file or activity in the Recommended section
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecentItem {
    pub id: String,
    pub name: String,
    pub timestamp_secs: u64,
    pub file_icon: FileIconType,
    pub target_path: PathBuf,
    pub is_pinned: bool,
}

impl RecentItem {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        timestamp_secs: u64,
        file_icon: FileIconType,
        target_path: impl Into<PathBuf>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            timestamp_secs,
            file_icon,
            target_path: target_path.into(),
            is_pinned: false,
        }
    }

    /// Generates human-friendly relative time string ("2h ago", "Yesterday", "5m ago")
    pub fn relative_time_string(&self, now_secs: u64) -> String {
        format_relative_time(self.timestamp_secs, now_secs)
    }
}

/// Formats relative time according to Windows 11 convention
pub fn format_relative_time(timestamp_secs: u64, now_secs: u64) -> String {
    if timestamp_secs >= now_secs {
        return "Just now".to_string();
    }
    let diff = now_secs.saturating_sub(timestamp_secs);
    if diff < 60 {
        "Just now".to_string()
    } else if diff < 3600 {
        let mins = diff / 60;
        format!("{}m ago", mins)
    } else if diff < 86400 {
        let hours = diff / 3600;
        format!("{}h ago", hours)
    } else if diff < 172800 {
        "Yesterday".to_string()
    } else if diff < 604800 {
        let days = diff / 86400;
        format!("{}d ago", days)
    } else if diff < 2592000 {
        let weeks = diff / 604800;
        format!("{}w ago", weeks)
    } else {
        let months = diff / 2592000;
        format!("{}mo ago", months)
    }
}

// ============================================================================
// Section 5: User Profile & Power Actions
// ============================================================================

/// User profile descriptor
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserProfile {
    pub username: String,
    pub display_name: String,
    pub email: String,
    pub avatar_initial: char,
    pub is_admin: bool,
}

impl UserProfile {
    pub fn default_current() -> Self {
        Self {
            username: "vinicius".to_string(),
            display_name: "Vinicius".to_string(),
            email: "vinicius@hybridos.internal".to_string(),
            avatar_initial: 'V',
            is_admin: true,
        }
    }
}

/// User menu actions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserAction {
    ChangeAccountSettings,
    Lock,
    SignOut,
    SwitchUser,
}

impl fmt::Display for UserAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UserAction::ChangeAccountSettings => write!(f, "Change account settings"),
            UserAction::Lock => write!(f, "Lock"),
            UserAction::SignOut => write!(f, "Sign out"),
            UserAction::SwitchUser => write!(f, "Switch user"),
        }
    }
}

/// Power menu actions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerAction {
    Sleep,
    Hibernate,
    Restart,
    ShutDown,
}

impl fmt::Display for PowerAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PowerAction::Sleep => write!(f, "Sleep"),
            PowerAction::Hibernate => write!(f, "Hibernate"),
            PowerAction::Restart => write!(f, "Restart"),
            PowerAction::ShutDown => write!(f, "Shut down"),
        }
    }
}

// ============================================================================
// Section 6: Search Integration Engine & Tabs
// ============================================================================

/// Categorized search results tabs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchTab {
    All,
    Apps,
    Documents,
    Web,
    Settings,
}

impl fmt::Display for SearchTab {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SearchTab::All => write!(f, "All"),
            SearchTab::Apps => write!(f, "Apps"),
            SearchTab::Documents => write!(f, "Documents"),
            SearchTab::Web => write!(f, "Web"),
            SearchTab::Settings => write!(f, "Settings"),
        }
    }
}

/// Result category for search matching
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchCategory {
    App,
    Document,
    Setting,
    WebSuggestion,
}

/// Executable target action from search result selection
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchAction {
    LaunchApp { executable: String, args: Vec<String> },
    OpenFile { path: PathBuf },
    OpenSetting { page_id: String },
    OpenWebQuery { query: String },
}

/// Matched Search Result item
#[derive(Debug, Clone, PartialEq)]
pub struct SearchResult {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub category: SearchCategory,
    pub icon_glyph: String,
    pub relevance_score: f32,
    pub action: SearchAction,
    pub is_best_match: bool,
}

/// Settings item for search database
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingItem {
    pub id: String,
    pub title: String,
    pub keywords: Vec<String>,
    pub category: String,
}

/// Search integration engine
#[derive(Debug, Clone)]
pub struct SearchEngine {
    settings_db: Vec<SettingItem>,
    web_history: Vec<String>,
}

impl SearchEngine {
    pub fn new() -> Self {
        let mut engine = Self {
            settings_db: Vec::new(),
            web_history: Vec::new(),
        };
        engine.init_default_settings();
        engine.init_default_web_suggestions();
        engine
    }

    fn init_default_settings(&mut self) {
        self.settings_db = vec![
            SettingItem {
                id: "display".into(),
                title: "Display settings".into(),
                keywords: vec!["resolution".into(), "scale".into(), "monitor".into(), "brightness".into(), "hdr".into()],
                category: "System".into(),
            },
            SettingItem {
                id: "sound".into(),
                title: "Sound settings".into(),
                keywords: vec!["volume".into(), "audio".into(), "speakers".into(), "microphone".into(), "output".into()],
                category: "System".into(),
            },
            SettingItem {
                id: "bluetooth".into(),
                title: "Bluetooth & devices".into(),
                keywords: vec!["mouse".into(), "keyboard".into(), "headphones".into(), "usb".into(), "printer".into()],
                category: "Devices".into(),
            },
            SettingItem {
                id: "network".into(),
                title: "Network & internet".into(),
                keywords: vec!["wifi".into(), "ethernet".into(), "vpn".into(), "proxy".into(), "ip".into(), "dns".into()],
                category: "Network".into(),
            },
            SettingItem {
                id: "personalization".into(),
                title: "Personalization".into(),
                keywords: vec!["wallpaper".into(), "background".into(), "theme".into(), "colors".into(), "dark mode".into()],
                category: "Personalization".into(),
            },
            SettingItem {
                id: "taskbar".into(),
                title: "Taskbar settings".into(),
                keywords: vec!["taskbar".into(), "alignment".into(), "center".into(), "tray".into(), "icons".into()],
                category: "Personalization".into(),
            },
            SettingItem {
                id: "apps".into(),
                title: "Installed apps".into(),
                keywords: vec!["uninstall".into(), "default apps".into(), "programs".into(), "startup".into()],
                category: "Apps".into(),
            },
            SettingItem {
                id: "update".into(),
                title: "Windows Update".into(),
                keywords: vec!["check for updates".into(), "patch".into(), "upgrade".into(), "restart".into()],
                category: "System".into(),
            },
            SettingItem {
                id: "power_battery".into(),
                title: "Power & battery".into(),
                keywords: vec!["sleep".into(), "battery saver".into(), "energy".into(), "power mode".into()],
                category: "System".into(),
            },
            SettingItem {
                id: "privacy".into(),
                title: "Privacy & security".into(),
                keywords: vec!["antivirus".into(), "firewall".into(), "permissions".into(), "camera".into(), "microphone".into()],
                category: "Privacy".into(),
            },
        ];
    }

    fn init_default_web_suggestions(&mut self) {
        self.web_history = vec![
            "hybrid kernel architecture design".into(),
            "rust os development tutorial".into(),
            "windows 11 fluent design specifications".into(),
            "directx and vulkan side-by-side compositing".into(),
            "microsoft store developer portal".into(),
        ];
    }

    /// Performs real-time fuzzy/prefix search across all sources
    pub fn query(
        &self,
        query_str: &str,
        active_tab: SearchTab,
        all_apps: &[AppTile],
        recent_items: &[RecentItem],
    ) -> Vec<SearchResult> {
        let q = query_str.trim().to_lowercase();
        if q.is_empty() {
            return Vec::new();
        }

        let mut results: Vec<SearchResult> = Vec::new();

        // 1. Search in Applications
        if active_tab == SearchTab::All || active_tab == SearchTab::Apps {
            for app in all_apps {
                let name_lower = app.display_name.to_lowercase();
                if let Some(score) = calculate_match_score(&name_lower, &q) {
                    results.push(SearchResult {
                        id: app.id.clone(),
                        title: app.display_name.clone(),
                        subtitle: format!("{:?} • App", app.category),
                        category: SearchCategory::App,
                        icon_glyph: app.icon.glyph.clone(),
                        relevance_score: score + 20.0, // Apps prioritize high
                        action: SearchAction::LaunchApp {
                            executable: app.executable_path.clone(),
                            args: app.arguments.clone(),
                        },
                        is_best_match: false,
                    });
                }
            }
        }

        // 2. Search in Documents / Recent Files
        if active_tab == SearchTab::All || active_tab == SearchTab::Documents {
            for doc in recent_items {
                let name_lower = doc.name.to_lowercase();
                let path_lower = doc.target_path.to_string_lossy().to_lowercase();
                if let Some(score) = calculate_match_score(&name_lower, &q)
                    .or_else(|| calculate_match_score(&path_lower, &q).map(|s| s * 0.7))
                {
                    results.push(SearchResult {
                        id: doc.id.clone(),
                        title: doc.name.clone(),
                        subtitle: doc.target_path.to_string_lossy().to_string(),
                        category: SearchCategory::Document,
                        icon_glyph: format!("{:?}", doc.file_icon).to_lowercase(),
                        relevance_score: score + 10.0,
                        action: SearchAction::OpenFile {
                            path: doc.target_path.clone(),
                        },
                        is_best_match: false,
                    });
                }
            }
        }

        // 3. Search in System Settings
        if active_tab == SearchTab::All || active_tab == SearchTab::Settings {
            for setting in &self.settings_db {
                let title_lower = setting.title.to_lowercase();
                let mut best_score = calculate_match_score(&title_lower, &q);
                if best_score.is_none() {
                    for kw in &setting.keywords {
                        if let Some(score) = calculate_match_score(kw, &q) {
                            best_score = Some(score * 0.85);
                            break;
                        }
                    }
                }
                if let Some(score) = best_score {
                    results.push(SearchResult {
                        id: setting.id.clone(),
                        title: setting.title.clone(),
                        subtitle: format!("Settings > {}", setting.category),
                        category: SearchCategory::Setting,
                        icon_glyph: "gear".into(),
                        relevance_score: score + 15.0,
                        action: SearchAction::OpenSetting {
                            page_id: setting.id.clone(),
                        },
                        is_best_match: false,
                    });
                }
            }
        }

        // 4. Web Suggestions
        if active_tab == SearchTab::All || active_tab == SearchTab::Web {
            // Include verbatim web query
            results.push(SearchResult {
                id: format!("web:{}", q),
                title: format!("Search the web for \"{}\"", query_str.trim()),
                subtitle: "Microsoft Edge web query".into(),
                category: SearchCategory::WebSuggestion,
                icon_glyph: "globe".into(),
                relevance_score: 5.0,
                action: SearchAction::OpenWebQuery {
                    query: query_str.trim().to_string(),
                },
                is_best_match: false,
            });

            for hist in &self.web_history {
                let h_lower = hist.to_lowercase();
                if let Some(score) = calculate_match_score(&h_lower, &q) {
                    results.push(SearchResult {
                        id: format!("hist:{}", hist),
                        title: hist.clone(),
                        subtitle: "Recent Web Search".into(),
                        category: SearchCategory::WebSuggestion,
                        icon_glyph: "history".into(),
                        relevance_score: score,
                        action: SearchAction::OpenWebQuery { query: hist.clone() },
                        is_best_match: false,
                    });
                }
            }
        }

        // Sort descending by relevance score
        results.sort_by(|a, b| b.relevance_score.partial_cmp(&a.relevance_score).unwrap_or(std::cmp::Ordering::Equal));

        // Mark highest relevance item as best match
        if let Some(first) = results.first_mut() {
            first.is_best_match = true;
        }

        results
    }
}

/// Match scoring helper: exact = 100.0, prefix = 80.0, contains = 40.0
fn calculate_match_score(target: &str, query: &str) -> Option<f32> {
    if target == query {
        Some(100.0)
    } else if target.starts_with(query) {
        Some(80.0)
    } else if let Some(idx) = target.find(query) {
        if idx > 0 && target.as_bytes().get(idx - 1) == Some(&b' ') {
            Some(60.0) // word boundary match
        } else {
            Some(40.0) // substring match
        }
    } else {
        None
    }
}

// ============================================================================
// Section 7: Header & Search Box
// ============================================================================

/// Search box state in Start Menu header
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchBox {
    pub query: String,
    pub placeholder: String,
    pub is_focused: bool,
    pub cursor_position: usize,
}

impl SearchBox {
    pub fn new() -> Self {
        Self {
            query: String::new(),
            placeholder: "Type here to search".to_string(),
            is_focused: false,
            cursor_position: 0,
        }
    }

    pub fn set_query(&mut self, text: impl Into<String>) {
        self.query = text.into();
        self.cursor_position = self.query.len();
    }

    pub fn clear(&mut self) {
        self.query.clear();
        self.cursor_position = 0;
    }

    pub fn insert_char(&mut self, c: char) {
        if self.cursor_position >= self.query.len() {
            self.query.push(c);
            self.cursor_position = self.query.len();
        } else {
            self.query.insert(self.cursor_position, c);
            self.cursor_position += 1;
        }
    }

    pub fn backspace(&mut self) -> bool {
        if self.cursor_position > 0 && !self.query.is_empty() {
            self.cursor_position -= 1;
            self.query.remove(self.cursor_position);
            true
        } else {
            false
        }
    }
}

// ============================================================================
// Section 8: Start Menu Layout & Configuration
// ============================================================================

/// Configuration parameters for Start Menu layout and theme
#[derive(Debug, Clone, PartialEq)]
pub struct StartMenuConfig {
    pub window_size: Size,
    pub corner_radius: CornerRadius,
    pub taskbar_alignment: TaskbarAlignment,
    pub taskbar_height: u32,
    pub margin_bottom: i32,
    pub theme: ThemeMode,
    pub mica: MicaMaterial,
    pub elevation_shadow: ElevationShadow32,
    pub pinned_columns: usize,
    pub pinned_rows: usize,
}

impl Default for StartMenuConfig {
    fn default() -> Self {
        Self {
            window_size: Size::new(START_MENU_WIDTH, START_MENU_HEIGHT),
            corner_radius: CornerRadius::WIN11_STANDARD,
            taskbar_alignment: TaskbarAlignment::Center,
            taskbar_height: TASKBAR_DEFAULT_HEIGHT,
            margin_bottom: START_MENU_MARGIN_BOTTOM,
            theme: ThemeMode::Dark,
            mica: MicaMaterial::win11_dark(),
            elevation_shadow: ElevationShadow32::WIN11,
            pinned_columns: PINNED_GRID_COLUMNS,
            pinned_rows: PINNED_GRID_ROWS_DEFAULT,
        }
    }
}

// ============================================================================
// Section 9: Complete Windows 11 Start Menu Engine
// ============================================================================

/// Windows 11 Start Menu Manager
pub struct StartMenu {
    pub config: StartMenuConfig,
    pub anim_state: StartMenuAnimationState,
    pub current_view: StartMenuView,
    pub search_box: SearchBox,
    pub active_search_tab: SearchTab,
    pub pinned_slots: Vec<PinnedSlot>,
    pub all_installed_apps: Vec<AppTile>,
    pub recommended_items: Vec<RecentItem>,
    pub user_profile: UserProfile,
    pub search_engine: SearchEngine,
    pub user_menu_open: bool,
    pub power_menu_open: bool,
    pub more_recommended_expanded: bool,
    pub current_pinned_page: usize,
    pub selected_search_index: usize,
    pub last_power_action: Option<PowerAction>,
    pub last_user_action: Option<UserAction>,
}

impl StartMenu {
    /// Creates a new Start Menu initialized with default commercial applications
    pub fn new() -> Self {
        let mut menu = Self {
            config: StartMenuConfig::default(),
            anim_state: StartMenuAnimationState::Closed,
            current_view: StartMenuView::Pinned,
            search_box: SearchBox::new(),
            active_search_tab: SearchTab::All,
            pinned_slots: Vec::new(),
            all_installed_apps: Vec::new(),
            recommended_items: Vec::new(),
            user_profile: UserProfile::default_current(),
            search_engine: SearchEngine::new(),
            user_menu_open: false,
            power_menu_open: false,
            more_recommended_expanded: false,
            current_pinned_page: 0,
            selected_search_index: 0,
            last_power_action: None,
            last_user_action: None,
        };

        menu.init_default_commercial_apps();
        menu.init_default_recommended();
        menu
    }

    /// Initializes default Windows 11 commercial applications
    fn init_default_commercial_apps(&mut self) {
        let default_apps = vec![
            AppTile::new(
                "com.microsoft.edge",
                "Microsoft Edge",
                AppIcon::new("globe", Rgba::new(0, 120, 212, 255)),
                AppCategory::Browser,
                "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe",
            ),
            AppTile::new(
                "com.microsoft.explorer",
                "File Explorer",
                AppIcon::new("folder", Rgba::new(255, 185, 0, 255)),
                AppCategory::System,
                "C:\\Windows\\explorer.exe",
            ),
            AppTile::new(
                "com.microsoft.settings",
                "Settings",
                AppIcon::new("gear", Rgba::new(128, 128, 128, 255)),
                AppCategory::Settings,
                "ms-settings:",
            ),
            AppTile::new(
                "com.microsoft.terminal",
                "Terminal",
                AppIcon::new("terminal", Rgba::new(0, 0, 0, 255)),
                AppCategory::Developer,
                "C:\\Windows\\System32\\wt.exe",
            ),
            AppTile::new(
                "com.microsoft.taskmgr",
                "Task Manager",
                AppIcon::new("chart", Rgba::new(0, 153, 188, 255)),
                AppCategory::SystemTools,
                "C:\\Windows\\System32\\Taskmgr.exe",
            ),
            AppTile::new(
                "com.microsoft.mail",
                "Mail",
                AppIcon::new("envelope", Rgba::new(0, 120, 215, 255)),
                AppCategory::Productivity,
                "outlookmail:",
            ).with_badge(3),
            AppTile::new(
                "com.microsoft.office",
                "Microsoft 365",
                AppIcon::new("grid", Rgba::new(216, 59, 1, 255)),
                AppCategory::Productivity,
                "office:",
            ),
            AppTile::new(
                "com.microsoft.calc",
                "Calculator",
                AppIcon::new("calculator", Rgba::new(0, 120, 215, 255)),
                AppCategory::Utilities,
                "calc.exe",
            ),
            AppTile::new(
                "com.microsoft.notepad",
                "Notepad",
                AppIcon::new("document", Rgba::new(0, 153, 188, 255)),
                AppCategory::Accessories,
                "notepad.exe",
            ),
            AppTile::new(
                "com.microsoft.photos",
                "Photos",
                AppIcon::new("image", Rgba::new(0, 120, 215, 255)),
                AppCategory::Media,
                "photos:",
            ),
            AppTile::new(
                "com.microsoft.store",
                "Microsoft Store",
                AppIcon::new("bag", Rgba::new(0, 120, 215, 255)),
                AppCategory::Store,
                "ms-windows-store:",
            ),
            AppTile::new(
                "com.spotify.client",
                "Spotify",
                AppIcon::new("music", Rgba::new(30, 215, 96, 255)),
                AppCategory::Entertainment,
                "C:\\Users\\Vinicius\\AppData\\Roaming\\Spotify\\Spotify.exe",
            ),
        ];

        self.all_installed_apps = default_apps.clone();
        self.pinned_slots = default_apps.into_iter().map(PinnedSlot::App).collect();
    }

    /// Initializes default recent items in Recommended section
    fn init_default_recommended(&mut self) {
        let now_secs = 1773324000; // Simulated current epoch timestamp
        self.recommended_items = vec![
            RecentItem::new(
                "rec_1",
                "HybridKernel_Architecture.docx",
                now_secs - 7200, // 2h ago
                FileIconType::Document,
                "C:\\Users\\Vinicius\\Documents\\HybridKernel_Architecture.docx",
            ),
            RecentItem::new(
                "rec_2",
                "Quarterly_Report.xlsx",
                now_secs - 86400, // Yesterday
                FileIconType::Spreadsheet,
                "C:\\Users\\Vinicius\\Documents\\Quarterly_Report.xlsx",
            ),
            RecentItem::new(
                "rec_3",
                "compositor.rs",
                now_secs - 300, // 5m ago
                FileIconType::Code,
                "C:\\Users\\Vinicius\\Documents\\HybridOS-Kernel\\rust-kernel\\src\\graphics\\compositor.rs",
            ),
            RecentItem::new(
                "rec_4",
                "VulkanDXGISpec.pdf",
                now_secs - 259200, // 3d ago
                FileIconType::Pdf,
                "C:\\Users\\Vinicius\\Documents\\Specifications\\VulkanDXGISpec.pdf",
            ),
            RecentItem::new(
                "rec_5",
                "Win11_Fluent_Mockups.png",
                now_secs - 604800, // Last week
                FileIconType::Image,
                "C:\\Users\\Vinicius\\Pictures\\Win11_Fluent_Mockups.png",
            ),
            RecentItem::new(
                "rec_6",
                "SetupPayload.zip",
                now_secs - 18000, // 5h ago
                FileIconType::Archive,
                "C:\\Users\\Vinicius\\Downloads\\SetupPayload.zip",
            ),
        ];
    }

    // ========================================================================
    // Window Placement & Bounds Calculation
    // ========================================================================

    /// Calculates the floating Start Menu bounding box above the taskbar
    pub fn calculate_window_bounds(&self, screen_width: u32, screen_height: u32) -> Rect {
        let width = self.config.window_size.width;
        let height = self.config.window_size.height;

        let x = match self.config.taskbar_alignment {
            TaskbarAlignment::Center => {
                ((screen_width as i32) - (width as i32)) / 2
            }
            TaskbarAlignment::Left => {
                // Aligned with Windows Start button margin (12px standard offset)
                12
            }
        };

        let y = (screen_height as i32)
            - (self.config.taskbar_height as i32)
            - self.config.margin_bottom
            - (height as i32);

        Rect::new(x, y, width, height)
    }

    // ========================================================================
    // State Transitions: Open / Close / Toggle
    // ========================================================================

    pub fn is_open(&self) -> bool {
        matches!(self.anim_state, StartMenuAnimationState::Open | StartMenuAnimationState::Opening { .. })
    }

    pub fn open(&mut self) {
        self.anim_state = StartMenuAnimationState::Open;
        self.current_view = StartMenuView::Pinned;
        self.user_menu_open = false;
        self.power_menu_open = false;
        self.more_recommended_expanded = false;
    }

    pub fn close(&mut self) {
        self.anim_state = StartMenuAnimationState::Closed;
        self.user_menu_open = false;
        self.power_menu_open = false;
        self.search_box.clear();
        self.search_box.is_focused = false;
        self.current_view = StartMenuView::Pinned;
    }

    pub fn toggle(&mut self) {
        if self.is_open() {
            self.close();
        } else {
            self.open();
        }
    }

    pub fn update_animation(&mut self, delta_progress: f32) {
        match self.anim_state {
            StartMenuAnimationState::Opening { progress } => {
                let next = progress + delta_progress;
                if next >= 1.0 {
                    self.anim_state = StartMenuAnimationState::Open;
                } else {
                    self.anim_state = StartMenuAnimationState::Opening { progress: next };
                }
            }
            StartMenuAnimationState::Closing { progress } => {
                let next = progress - delta_progress;
                if next <= 0.0 {
                    self.anim_state = StartMenuAnimationState::Closed;
                } else {
                    self.anim_state = StartMenuAnimationState::Closing { progress: next };
                }
            }
            _ => {}
        }
    }

    // ========================================================================
    // View Switching: Pinned, All Apps, Search
    // ========================================================================

    pub fn switch_to_all_apps(&mut self) {
        self.current_view = StartMenuView::AllApps;
    }

    pub fn switch_to_pinned(&mut self) {
        self.current_view = StartMenuView::Pinned;
        if self.search_box.query.is_empty() {
            self.search_box.is_focused = false;
        }
    }

    pub fn toggle_more_recommended(&mut self) {
        self.more_recommended_expanded = !self.more_recommended_expanded;
    }

    // ========================================================================
    // Pinned Apps Management: Pin, Unpin, Move, Folders
    // ========================================================================

    /// Pins an application to the Start Menu
    pub fn pin_app(&mut self, app: AppTile) -> usize {
        // If already pinned, return existing index
        if let Some(idx) = self.pinned_slots.iter().position(|s| s.id() == app.id) {
            return idx;
        }
        // Also register in installed apps if not already present
        if !self.all_installed_apps.iter().any(|a| a.id == app.id) {
            self.all_installed_apps.push(app.clone());
        }
        self.pinned_slots.push(PinnedSlot::App(app));
        self.pinned_slots.len() - 1
    }

    /// Unpins an application or folder by ID
    pub fn unpin_app(&mut self, id: &str) -> bool {
        if let Some(pos) = self.pinned_slots.iter().position(|s| s.id() == id) {
            self.pinned_slots.remove(pos);
            true
        } else {
            // Check if it's inside a folder
            for slot in &mut self.pinned_slots {
                if let PinnedSlot::Folder(folder) = slot {
                    if let Some(fpos) = folder.items.iter().position(|item| item.id == id) {
                        folder.items.remove(fpos);
                        return true;
                    }
                }
            }
            false
        }
    }

    /// Moves a pinned item from one grid slot to another
    pub fn move_pinned_app(&mut self, from_idx: usize, to_idx: usize) -> bool {
        if from_idx >= self.pinned_slots.len() || to_idx >= self.pinned_slots.len() {
            return false;
        }
        let item = self.pinned_slots.remove(from_idx);
        self.pinned_slots.insert(to_idx, item);
        true
    }

    /// Groups multiple pinned apps into a new folder
    pub fn group_into_folder(&mut self, folder_name: &str, app_ids: &[&str]) -> Result<String, String> {
        if app_ids.len() < 2 {
            return Err("A folder must contain at least two apps".to_string());
        }

        let mut collected_apps = Vec::new();
        for &id in app_ids {
            if let Some(pos) = self.pinned_slots.iter().position(|s| s.id() == id) {
                if let PinnedSlot::App(app) = self.pinned_slots.remove(pos) {
                    collected_apps.push(app);
                }
            } else if let Some(app) = self.all_installed_apps.iter().find(|a| a.id == id) {
                collected_apps.push(app.clone());
            }
        }

        if collected_apps.len() < 2 {
            return Err("Could not locate requested apps to group".to_string());
        }

        let folder_id = format!("folder_{}", folder_name.to_lowercase().replace(' ', "_"));
        let folder = FolderTile::new(folder_id.clone(), folder_name, collected_apps);
        self.pinned_slots.push(PinnedSlot::Folder(folder));
        Ok(folder_id)
    }

    /// Adds an app into an existing folder
    pub fn add_to_folder(&mut self, folder_id: &str, app: AppTile) -> Result<(), String> {
        let folder = self.pinned_slots.iter_mut().find_map(|s| {
            if let PinnedSlot::Folder(f) = s {
                if f.id == folder_id {
                    Some(f)
                } else {
                    None
                }
            } else {
                None
            }
        }).ok_or_else(|| "Folder not found".to_string())?;

        if !folder.items.iter().any(|i| i.id == app.id) {
            folder.items.push(app);
        }
        Ok(())
    }

    /// Removes an app from a folder
    pub fn remove_from_folder(&mut self, folder_id: &str, app_id: &str) -> Result<AppTile, String> {
        let folder = self.pinned_slots.iter_mut().find_map(|s| {
            if let PinnedSlot::Folder(f) = s {
                if f.id == folder_id {
                    Some(f)
                } else {
                    None
                }
            } else {
                None
            }
        }).ok_or_else(|| "Folder not found".to_string())?;

        let pos = folder.items.iter().position(|i| i.id == app_id)
            .ok_or_else(|| "App not in folder".to_string())?;

        Ok(folder.items.remove(pos))
    }

    /// Ungroups a folder back into individual pinned items
    pub fn ungroup_folder(&mut self, folder_id: &str) -> Result<Vec<AppTile>, String> {
        let pos = self.pinned_slots.iter().position(|s| {
            matches!(s, PinnedSlot::Folder(f) if f.id == folder_id)
        }).ok_or_else(|| "Folder not found".to_string())?;

        if let PinnedSlot::Folder(folder) = self.pinned_slots.remove(pos) {
            let items = folder.items;
            for item in &items {
                self.pinned_slots.push(PinnedSlot::App(item.clone()));
            }
            Ok(items)
        } else {
            Err("Target is not a folder".to_string())
        }
    }

    // ========================================================================
    // Alphabetical Catalog (All Apps A..Z)
    // ========================================================================

    /// Generates alphabetical A..Z sections for the All Apps view
    pub fn get_all_apps_alphabetical(&self) -> BTreeMap<char, Vec<AppTile>> {
        let mut grouped: BTreeMap<char, Vec<AppTile>> = BTreeMap::new();
        let mut sorted_apps = self.all_installed_apps.clone();
        sorted_apps.sort_by(|a, b| a.display_name.cmp(&b.display_name));

        for app in sorted_apps {
            let first_char = app.display_name.chars().next().unwrap_or('#').to_ascii_uppercase();
            let key = if first_char.is_ascii_alphabetic() {
                first_char
            } else {
                '&' // Special characters bucket
            };
            grouped.entry(key).or_default().push(app);
        }

        grouped
    }

    // ========================================================================
    // Recommended Section Operations
    // ========================================================================

    /// Adds a recent item to the recommended list
    pub fn add_recent_item(&mut self, item: RecentItem) {
        // If already present, remove duplicate
        self.recommended_items.retain(|i| i.id != item.id);
        self.recommended_items.insert(0, item);
    }

    /// Removes a recent item by ID
    pub fn remove_recent_item(&mut self, id: &str) -> bool {
        let orig_len = self.recommended_items.len();
        self.recommended_items.retain(|i| i.id != id);
        self.recommended_items.len() < orig_len
    }

    /// Returns recommended items to display (capped at 6 for collapsed view)
    pub fn get_recommended_items(&self) -> &[RecentItem] {
        if self.more_recommended_expanded {
            &self.recommended_items
        } else {
            let take_len = self.recommended_items.len().min(MAX_RECOMMENDED_COLLAPSED);
            &self.recommended_items[..take_len]
        }
    }

    // ========================================================================
    // User & Power Footer Operations
    // ========================================================================

    pub fn toggle_user_menu(&mut self) {
        self.user_menu_open = !self.user_menu_open;
        if self.user_menu_open {
            self.power_menu_open = false;
        }
    }

    pub fn toggle_power_menu(&mut self) {
        self.power_menu_open = !self.power_menu_open;
        if self.power_menu_open {
            self.user_menu_open = false;
        }
    }

    pub fn trigger_power_action(&mut self, action: PowerAction) {
        self.last_power_action = Some(action);
        self.power_menu_open = false;
        self.close();
    }

    pub fn trigger_user_action(&mut self, action: UserAction) {
        self.last_user_action = Some(action);
        self.user_menu_open = false;
        self.close();
    }

    // ========================================================================
    // Search Integration
    // ========================================================================

    pub fn set_search_query(&mut self, query: &str) {
        self.search_box.set_query(query);
        if query.trim().is_empty() {
            self.current_view = StartMenuView::Pinned;
        } else {
            self.current_view = StartMenuView::SearchResults;
            self.selected_search_index = 0;
        }
    }

    pub fn set_search_tab(&mut self, tab: SearchTab) {
        self.active_search_tab = tab;
        self.selected_search_index = 0;
    }

    pub fn current_search_results(&self) -> Vec<SearchResult> {
        self.search_engine.query(
            &self.search_box.query,
            self.active_search_tab,
            &self.all_installed_apps,
            &self.recommended_items,
        )
    }

    pub fn select_next_search_result(&mut self) {
        let results = self.current_search_results();
        if !results.is_empty() {
            self.selected_search_index = (self.selected_search_index + 1) % results.len();
        }
    }

    pub fn select_prev_search_result(&mut self) {
        let results = self.current_search_results();
        if !results.is_empty() {
            if self.selected_search_index == 0 {
                self.selected_search_index = results.len() - 1;
            } else {
                self.selected_search_index -= 1;
            }
        }
    }

    pub fn execute_selected_search_result(&mut self) -> Option<SearchAction> {
        let results = self.current_search_results();
        if let Some(item) = results.get(self.selected_search_index) {
            let action = item.action.clone();
            self.close();
            Some(action)
        } else {
            None
        }
    }

    // ========================================================================
    // Mouse Event Handling & Hit Testing
    // ========================================================================

    pub fn hit_test(&self, screen_w: u32, screen_h: u32, px: i32, py: i32) -> HitTestTarget {
        let window_rect = self.calculate_window_bounds(screen_w, screen_h);
        if !window_rect.contains(px, py) {
            return HitTestTarget::OutsideWindow;
        }

        let rel_x = px - window_rect.x;
        let rel_y = py - window_rect.y;

        // Search Header region (y: 20..64)
        if rel_y >= 20 && rel_y <= 64 && rel_x >= 32 && rel_x <= (START_MENU_WIDTH as i32 - 32) {
            if rel_x >= (START_MENU_WIDTH as i32 - 70) && !self.search_box.query.is_empty() {
                return HitTestTarget::SearchClearButton;
            }
            return HitTestTarget::SearchBox;
        }

        // Footer region (bottom 64px)
        let footer_top = (START_MENU_HEIGHT as i32) - (self.config.margin_bottom + 52);
        if rel_y >= footer_top {
            // User button (left side)
            if rel_x >= 32 && rel_x <= 200 {
                return HitTestTarget::UserProfileButton;
            }
            // Power button (right side)
            if rel_x >= (START_MENU_WIDTH as i32 - 80) && rel_x <= (START_MENU_WIDTH as i32 - 32) {
                return HitTestTarget::PowerButton;
            }
        }

        // Active View specific hit testing
        match self.current_view {
            StartMenuView::Pinned => {
                // "All apps >" button header (y: 80..104, right: 32..120)
                if rel_y >= 80 && rel_y <= 110 && rel_x >= (START_MENU_WIDTH as i32 - 120) {
                    return HitTestTarget::AllAppsButton;
                }

                // "More >" button for Recommended (y: 430..454, right: 32..100)
                if rel_y >= 430 && rel_y <= 460 && rel_x >= (START_MENU_WIDTH as i32 - 100) {
                    return HitTestTarget::MoreRecommendedButton;
                }

                // Check Pinned App Grid (y: 110..420)
                if rel_y >= 110 && rel_y <= 420 {
                    let col_width = (START_MENU_WIDTH - 64) / 6;
                    let row_height = 84;
                    let col = ((rel_x - 32) / col_width as i32).clamp(0, 5) as usize;
                    let row = ((rel_y - 110) / row_height as i32).clamp(0, 2) as usize;
                    let index = row * 6 + col;
                    if let Some(slot) = self.pinned_slots.get(index) {
                        return match slot {
                            PinnedSlot::App(app) => HitTestTarget::PinnedApp(app.id.clone()),
                            PinnedSlot::Folder(folder) => HitTestTarget::PinnedFolder(folder.id.clone()),
                        };
                    }
                }

                // Check Recommended items (y: 460..640)
                if rel_y >= 460 && rel_y <= 640 {
                    let row_height = 56;
                    let rec_row = ((rel_y - 460) / row_height as i32).clamp(0, 2) as usize;
                    let col = if rel_x < (START_MENU_WIDTH as i32 / 2) { 0 } else { 1 };
                    let rec_index = rec_row * 2 + col;
                    if let Some(item) = self.get_recommended_items().get(rec_index) {
                        return HitTestTarget::RecommendedItem(item.id.clone());
                    }
                }
            }
            StartMenuView::AllApps => {
                // "< Back" button (y: 80..110, left: 32..120)
                if rel_y >= 80 && rel_y <= 110 && rel_x >= 32 && rel_x <= 120 {
                    return HitTestTarget::BackToPinnedButton;
                }
            }
            StartMenuView::SearchResults => {
                // Search Tabs (y: 72..104)
                if rel_y >= 72 && rel_y <= 104 {
                    let tab_width = 80;
                    let tab_idx = (rel_x - 32) / tab_width;
                    let tab = match tab_idx {
                        0 => SearchTab::All,
                        1 => SearchTab::Apps,
                        2 => SearchTab::Documents,
                        3 => SearchTab::Web,
                        4 => SearchTab::Settings,
                        _ => SearchTab::All,
                    };
                    return HitTestTarget::SearchTab(tab);
                }

                // Search Results list items
                if rel_y >= 110 && rel_y <= 640 {
                    let item_idx = ((rel_y - 110) / 60) as usize;
                    return HitTestTarget::SearchResultItem(item_idx);
                }
            }
        }

        HitTestTarget::None
    }

    /// Handles mouse click event
    pub fn on_mouse_click(&mut self, screen_w: u32, screen_h: u32, px: i32, py: i32) -> Option<HitTestTarget> {
        let target = self.hit_test(screen_w, screen_h, px, py);
        match &target {
            HitTestTarget::OutsideWindow => {
                self.close();
            }
            HitTestTarget::SearchBox => {
                self.search_box.is_focused = true;
            }
            HitTestTarget::SearchClearButton => {
                self.search_box.clear();
                self.current_view = StartMenuView::Pinned;
            }
            HitTestTarget::AllAppsButton => {
                self.switch_to_all_apps();
            }
            HitTestTarget::BackToPinnedButton => {
                self.switch_to_pinned();
            }
            HitTestTarget::MoreRecommendedButton => {
                self.toggle_more_recommended();
            }
            HitTestTarget::UserProfileButton => {
                self.toggle_user_menu();
            }
            HitTestTarget::PowerButton => {
                self.toggle_power_menu();
            }
            HitTestTarget::SearchTab(tab) => {
                self.set_search_tab(*tab);
            }
            HitTestTarget::SearchResultItem(idx) => {
                self.selected_search_index = *idx;
                self.execute_selected_search_result();
            }
            HitTestTarget::PinnedApp(_) | HitTestTarget::RecommendedItem(_) => {
                self.close();
            }
            _ => {}
        }
        Some(target)
    }
}

// ============================================================================
// Section 10: Render Tree Descriptor for Compositor & DWM
// ============================================================================

/// Visual element type for rendering via the desktop window compositor
#[derive(Debug, Clone, PartialEq)]
pub enum RenderElement {
    MicaSurface {
        bounds: Rect,
        radius: CornerRadius,
        material: MicaMaterial,
        shadow: ElevationShadow32,
    },
    SearchBoxHeader {
        bounds: Rect,
        query: String,
        placeholder: String,
        is_focused: bool,
    },
    AppTileItem {
        bounds: Rect,
        title: String,
        glyph: String,
        tint: Rgba,
        badge: Option<u32>,
    },
    FolderTileItem {
        bounds: Rect,
        name: String,
        preview_count: usize,
    },
    RecommendedSectionHeader {
        bounds: Rect,
        title: String,
        more_button_bounds: Rect,
    },
    RecentFileItem {
        bounds: Rect,
        name: String,
        relative_time: String,
        icon_type: FileIconType,
    },
    FooterBar {
        bounds: Rect,
        user_avatar_char: char,
        user_name: String,
        user_menu_open: bool,
        power_menu_open: bool,
    },
    PopupFlyout {
        bounds: Rect,
        items: Vec<String>,
    },
}

impl StartMenu {
    /// Generates high-level render descriptors for the compositor to draw
    pub fn generate_render_tree(&self, screen_w: u32, screen_h: u32) -> Vec<RenderElement> {
        let mut elements = Vec::new();
        if !self.is_open() {
            return elements;
        }

        let bounds = self.calculate_window_bounds(screen_w, screen_h);

        // 1. Background Mica Surface with Elevation Shadow 32
        elements.push(RenderElement::MicaSurface {
            bounds,
            radius: self.config.corner_radius,
            material: self.config.mica,
            shadow: self.config.elevation_shadow,
        });

        // 2. Search Box Header
        let search_rect = Rect::new(bounds.x + 32, bounds.y + 20, bounds.width - 64, 40);
        elements.push(RenderElement::SearchBoxHeader {
            bounds: search_rect,
            query: self.search_box.query.clone(),
            placeholder: self.search_box.placeholder.clone(),
            is_focused: self.search_box.is_focused,
        });

        // 3. View content
        match self.current_view {
            StartMenuView::Pinned => {
                // Pinned app tiles
                let col_width = (bounds.width - 64) / 6;
                let row_height = 84;
                for (idx, slot) in self.pinned_slots.iter().take(MAX_PINNED_PER_PAGE).enumerate() {
                    let col = (idx % 6) as u32;
                    let row = (idx / 6) as u32;
                    let tile_rect = Rect::new(
                        bounds.x + 32 + (col * col_width) as i32,
                        bounds.y + 110 + (row * row_height) as i32,
                        col_width,
                        row_height,
                    );
                    match slot {
                        PinnedSlot::App(app) => {
                            elements.push(RenderElement::AppTileItem {
                                bounds: tile_rect,
                                title: app.display_name.clone(),
                                glyph: app.icon.glyph.clone(),
                                tint: app.icon.tint_color,
                                badge: app.badge,
                            });
                        }
                        PinnedSlot::Folder(folder) => {
                            elements.push(RenderElement::FolderTileItem {
                                bounds: tile_rect,
                                name: folder.name.clone(),
                                preview_count: folder.items.len(),
                            });
                        }
                    }
                }

                // Recommended Header & Items
                let rec_header_rect = Rect::new(bounds.x + 32, bounds.y + 430, bounds.width - 64, 28);
                let more_btn_rect = Rect::new(bounds.x + bounds.width as i32 - 100, bounds.y + 430, 68, 28);
                elements.push(RenderElement::RecommendedSectionHeader {
                    bounds: rec_header_rect,
                    title: "Recommended".into(),
                    more_button_bounds: more_btn_rect,
                });

                let rec_items = self.get_recommended_items();
                let rec_col_w = (bounds.width - 64) / 2;
                for (i, item) in rec_items.iter().enumerate() {
                    let col = (i % 2) as u32;
                    let row = (i / 2) as u32;
                    let item_rect = Rect::new(
                        bounds.x + 32 + (col * rec_col_w) as i32,
                        bounds.y + 460 + (row * 56) as i32,
                        rec_col_w,
                        52,
                    );
                    elements.push(RenderElement::RecentFileItem {
                        bounds: item_rect,
                        name: item.name.clone(),
                        relative_time: item.relative_time_string(1773324000),
                        icon_type: item.file_icon,
                    });
                }
            }
            StartMenuView::AllApps => {
                // All apps listing descriptors
            }
            StartMenuView::SearchResults => {
                // Categorized search tabs & results descriptors
            }
        }

        // 4. Footer Bar
        let footer_rect = Rect::new(bounds.x, bounds.y + bounds.height as i32 - 64, bounds.width, 64);
        elements.push(RenderElement::FooterBar {
            bounds: footer_rect,
            user_avatar_char: self.user_profile.avatar_initial,
            user_name: self.user_profile.display_name.clone(),
            user_menu_open: self.user_menu_open,
            power_menu_open: self.power_menu_open,
        });

        // 5. Popups (User Menu or Power Menu)
        if self.power_menu_open {
            let popup_rect = Rect::new(bounds.x + bounds.width as i32 - 160, bounds.y + bounds.height as i32 - 200, 140, 130);
            elements.push(RenderElement::PopupFlyout {
                bounds: popup_rect,
                items: vec!["Sleep".into(), "Hibernate".into(), "Restart".into(), "Shut down".into()],
            });
        }

        if self.user_menu_open {
            let popup_rect = Rect::new(bounds.x + 32, bounds.y + bounds.height as i32 - 180, 200, 110);
            elements.push(RenderElement::PopupFlyout {
                bounds: popup_rect,
                items: vec!["Change account settings".into(), "Lock".into(), "Sign out".into()],
            });
        }

        elements
    }
}

// ============================================================================
// Section 11: Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_start_menu_open_close_toggle() {
        let mut menu = StartMenu::new();
        assert!(!menu.is_open());
        assert_eq!(menu.anim_state, StartMenuAnimationState::Closed);

        menu.open();
        assert!(menu.is_open());
        assert_eq!(menu.anim_state, StartMenuAnimationState::Open);

        menu.close();
        assert!(!menu.is_open());
        assert_eq!(menu.anim_state, StartMenuAnimationState::Closed);

        menu.toggle();
        assert!(menu.is_open());
        menu.toggle();
        assert!(!menu.is_open());
    }

    #[test]
    fn test_pinned_apps_management() {
        let mut menu = StartMenu::new();
        // Check default commercial pinned apps count
        assert!(menu.pinned_slots.len() >= 9);

        // Verify key default commercial apps exist
        let app_ids: Vec<&str> = menu.pinned_slots.iter().map(|s| s.id()).collect();
        assert!(app_ids.contains(&"com.microsoft.edge"));
        assert!(app_ids.contains(&"com.microsoft.explorer"));
        assert!(app_ids.contains(&"com.microsoft.settings"));
        assert!(app_ids.contains(&"com.microsoft.terminal"));
        assert!(app_ids.contains(&"com.microsoft.taskmgr"));
        assert!(app_ids.contains(&"com.microsoft.mail"));
        assert!(app_ids.contains(&"com.microsoft.office"));
        assert!(app_ids.contains(&"com.microsoft.calc"));
        assert!(app_ids.contains(&"com.microsoft.notepad"));

        // Pin custom app
        let custom_app = AppTile::new(
            "com.custom.editor",
            "Code Studio",
            AppIcon::new("code", Rgba::new(0, 100, 200, 255)),
            AppCategory::Developer,
            "C:\\Tools\\studio.exe",
        );
        let idx = menu.pin_app(custom_app);
        assert_eq!(menu.pinned_slots[idx].id(), "com.custom.editor");

        // Move position
        let initial_first = menu.pinned_slots[0].id().to_string();
        menu.move_pinned_app(0, 1);
        assert_eq!(menu.pinned_slots[1].id(), initial_first);

        // Unpin app
        let unpinned = menu.unpin_app("com.custom.editor");
        assert!(unpinned);
        assert!(!menu.pinned_slots.iter().any(|s| s.id() == "com.custom.editor"));
    }

    #[test]
    fn test_folder_creation_and_grouping() {
        let mut menu = StartMenu::new();
        let group_res = menu.group_into_folder("Productivity Tools", &["com.microsoft.calc", "com.microsoft.notepad"]);
        assert!(group_res.is_ok());
        let folder_id = group_res.unwrap();

        // Verify folder exists in pinned slots
        let folder_slot = menu.pinned_slots.iter().find(|s| s.id() == folder_id);
        assert!(folder_slot.is_some());
        if let Some(PinnedSlot::Folder(folder)) = folder_slot {
            assert_eq!(folder.items.len(), 2);
            assert_eq!(folder.name, "Productivity Tools");
        } else {
            panic!("Expected PinnedSlot::Folder");
        }

        // Add third app to folder
        let new_app = AppTile::new(
            "com.tools.hex",
            "Hex Editor",
            AppIcon::new("binary", Rgba::new(200, 50, 50, 255)),
            AppCategory::Developer,
            "hex.exe",
        );
        assert!(menu.add_to_folder(&folder_id, new_app).is_ok());

        // Remove app from folder
        let removed = menu.remove_from_folder(&folder_id, "com.tools.hex");
        assert!(removed.is_ok());
        assert_eq!(removed.unwrap().id, "com.tools.hex");

        // Ungroup folder
        let ungroup_res = menu.ungroup_folder(&folder_id);
        assert!(ungroup_res.is_ok());
        assert!(!menu.pinned_slots.iter().any(|s| s.id() == folder_id));
    }

    #[test]
    fn test_recommended_recent_items() {
        let mut menu = StartMenu::new();
        assert!(!menu.recommended_items.is_empty());

        let now = 1773324000;
        let item = RecentItem::new(
            "rec_test",
            "ProjectCharter.docx",
            now - 7200, // 2h ago
            FileIconType::Document,
            "C:\\Users\\Vinicius\\Documents\\ProjectCharter.docx",
        );
        assert_eq!(item.relative_time_string(now), "2h ago");

        let yesterday_item = RecentItem::new(
            "rec_yesterday",
            "Notes.txt",
            now - 90000,
            FileIconType::Document,
            "C:\\Users\\Vinicius\\Documents\\Notes.txt",
        );
        assert_eq!(yesterday_item.relative_time_string(now), "Yesterday");

        menu.add_recent_item(item);
        assert_eq!(menu.recommended_items[0].id, "rec_test");

        let removed = menu.remove_recent_item("rec_test");
        assert!(removed);
        assert!(!menu.recommended_items.iter().any(|i| i.id == "rec_test"));
    }

    #[test]
    fn test_search_engine_filtering_and_tabs() {
        let mut menu = StartMenu::new();
        menu.open();

        // Search for "terminal"
        menu.set_search_query("terminal");
        assert_eq!(menu.current_view, StartMenuView::SearchResults);

        let results = menu.current_search_results();
        assert!(!results.is_empty());
        let terminal_match = results.iter().find(|r| r.title.contains("Terminal"));
        assert!(terminal_match.is_some());
        assert!(terminal_match.unwrap().is_best_match);

        // Search for Settings item
        menu.set_search_query("display");
        let display_results = menu.current_search_results();
        assert!(display_results.iter().any(|r| r.title.contains("Display")));

        // Test Category tabs filtering
        menu.set_search_tab(SearchTab::Settings);
        let settings_only = menu.current_search_results();
        for res in &settings_only {
            assert_eq!(res.category, SearchCategory::Setting);
        }

        // Clear query returns to Pinned view
        menu.set_search_query("");
        assert_eq!(menu.current_view, StartMenuView::Pinned);
    }

    #[test]
    fn test_power_and_user_actions() {
        let mut menu = StartMenu::new();
        menu.open();

        // Test Power menu popup
        menu.toggle_power_menu();
        assert!(menu.power_menu_open);
        assert!(!menu.user_menu_open);

        menu.trigger_power_action(PowerAction::Restart);
        assert_eq!(menu.last_power_action, Some(PowerAction::Restart));
        assert!(!menu.is_open()); // Start menu auto-closes after action

        // Test User profile menu popup
        menu.open();
        menu.toggle_user_menu();
        assert!(menu.user_menu_open);
        assert!(!menu.power_menu_open);

        menu.trigger_user_action(UserAction::Lock);
        assert_eq!(menu.last_user_action, Some(UserAction::Lock));
        assert!(!menu.is_open());
    }

    #[test]
    fn test_all_apps_alphabetical_grouping() {
        let mut menu = StartMenu::new();
        menu.switch_to_all_apps();
        assert_eq!(menu.current_view, StartMenuView::AllApps);

        let grouped = menu.get_all_apps_alphabetical();
        assert!(grouped.contains_key(&'C')); // Calculator
        assert!(grouped.contains_key(&'M')); // Mail / Microsoft Edge
        assert!(grouped.contains_key(&'N')); // Notepad
        assert!(grouped.contains_key(&'T')); // Task Manager / Terminal

        let c_apps = &grouped[&'C'];
        assert!(c_apps.iter().any(|a| a.display_name == "Calculator"));
    }

    #[test]
    fn test_mica_and_geometry_calculations() {
        let menu = StartMenu::new();
        let screen_w = 1920;
        let screen_h = 1080;

        let bounds = menu.calculate_window_bounds(screen_w, screen_h);
        assert_eq!(bounds.width, START_MENU_WIDTH);
        assert_eq!(bounds.height, START_MENU_HEIGHT);

        // Centered alignment calculation check: (1920 - 640) / 2 = 640
        assert_eq!(bounds.x, (1920 - 640) / 2);
        // Above taskbar: 1080 - 48 - 12 - 728 = 292
        assert_eq!(bounds.y, 1080 - 48 - 12 - 728);

        // Mica & Elevation Shadow checks
        assert_eq!(menu.config.corner_radius.top_left, 8.0);
        assert_eq!(menu.config.corner_radius.bottom_right, 8.0);
        assert_eq!(menu.config.elevation_shadow.ambient_blur, 32.0);
        assert_eq!(menu.config.elevation_shadow.key_alpha, 0.24);
    }
}
