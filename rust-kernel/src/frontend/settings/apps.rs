//! HybridOS Windows 11 Settings App - Apps & Default Applications Category
//!
//! Provides enterprise-grade management of installed applications across dual-personality
//! runtimes (Win32 Desktop, UWP/WinUI, and Linux ELF packages), advanced installation policies,
//! command-line execution aliases, file extension and protocol associations, and startup
//! applications with automatic synchronization to Windows NT registry Run keys:
//! `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`.

#![deny(unsafe_code)]

use std::collections::{BTreeMap, HashMap};
use std::fmt;

/// Windows NT Registry key path for user-level startup run entries.
pub const REG_KEY_HKCU_RUN: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";

/// Windows NT Registry key path for system-level startup run entries.
pub const REG_KEY_HKLM_RUN: &str = r"HKLM\Software\Microsoft\Windows\CurrentVersion\Run";

// ============================================================================
// Core Enumerations & Types
// ============================================================================

/// Application runtime classification for HybridOS dual-personality kernel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AppType {
    /// Win32 / Win64 desktop PE32+ executable.
    Win32Desktop,
    /// Universal Windows Platform / WinUI packaged AppX/MSIX app.
    UwpWinUi,
    /// Native Linux POSIX ELF64 binary / package.
    LinuxElf,
    /// Android APK package running under HybridOS Android personality.
    AndroidApk,
}

impl AppType {
    /// Returns the human-readable display name for the app runtime.
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Win32Desktop => "Win32 Desktop",
            Self::UwpWinUi => "UWP / WinUI",
            Self::LinuxElf => "Linux ELF Package",
            Self::AndroidApk => "Android Package",
        }
    }
}

/// Installation source protection policy ("Choose where to get apps").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppInstallSourcePolicy {
    /// Allow apps from anywhere on web or external media without warnings.
    Anywhere,
    /// Allow apps from anywhere, but warn before installing an app that's not from Microsoft Store.
    AnywhereWarn,
    /// Only allow apps from the Microsoft Store / verified repository to help protect your PC.
    StoreOnly,
}

impl AppInstallSourcePolicy {
    /// Returns the human-readable description of the policy.
    pub fn description(&self) -> &'static str {
        match self {
            Self::Anywhere => "Anywhere",
            Self::AnywhereWarn => "Anywhere, but warn me before installing an app that's not from the Microsoft Store",
            Self::StoreOnly => "The Microsoft Store only (Only allow apps from Store)",
        }
    }
}

/// Granular app permissions managed in Advanced Options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AppPermission {
    BackgroundApps,
    Camera,
    Microphone,
    Location,
    FileSystem,
    Notifications,
}

impl AppPermission {
    /// Returns the UI display label for the permission.
    pub fn label(&self) -> &'static str {
        match self {
            Self::BackgroundApps => "Background apps permissions",
            Self::Camera => "Camera access",
            Self::Microphone => "Microphone access",
            Self::Location => "Location services",
            Self::FileSystem => "File system access",
            Self::Notifications => "Push notifications",
        }
    }
}

/// Impact of an application on system boot and login latency.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum StartupImpact {
    None,
    Low,    // < 300 ms impact
    Medium, // 300 ms - 1000 ms impact
    High,   // > 1000 ms impact
}

impl StartupImpact {
    /// Human-readable label for startup impact badge.
    pub fn label(&self) -> &'static str {
        match self {
            Self::None => "No impact",
            Self::Low => "Low impact",
            Self::Medium => "Medium impact",
            Self::High => "High impact",
        }
    }
}

/// Sorting options for installed applications list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppSortOrder {
    NameAsc,
    NameDesc,
    SizeDesc,
    SizeAsc,
    InstallDateDesc,
    InstallDateAsc,
}

/// Drive filter for installed applications.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DriveFilter {
    AllDrives,
    SpecificDrive(String),
}

/// App runtime type filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppTypeFilter {
    AllTypes,
    Only(AppType),
}

/// Navigation subpages within the Apps settings category.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppsCategorySubpage {
    InstalledApps,
    AdvancedAppSettings,
    DefaultApps,
    StartupApps,
    OptionalFeatures,
    VideoPlayback,
}

impl AppsCategorySubpage {
    /// Returns the URI protocol associated with this subcategory.
    pub fn uri_protocol(&self) -> &'static str {
        match self {
            Self::InstalledApps => "ms-settings:appsfeatures",
            Self::AdvancedAppSettings => "ms-settings:advancedappsettings",
            Self::DefaultApps => "ms-settings:defaultapps",
            Self::StartupApps => "ms-settings:startupapps",
            Self::OptionalFeatures => "ms-settings:optionalfeatures",
            Self::VideoPlayback => "ms-settings:videoplayback",
        }
    }

    /// Returns the page title.
    pub fn title(&self) -> &'static str {
        match self {
            Self::InstalledApps => "Installed apps",
            Self::AdvancedAppSettings => "Advanced app settings",
            Self::DefaultApps => "Default apps",
            Self::StartupApps => "Startup",
            Self::OptionalFeatures => "Optional features",
            Self::VideoPlayback => "Video playback",
        }
    }
}

/// Share across devices options.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShareAcrossDevicesMode {
    MyDevicesOnly,
    EveryoneNearby,
}

// ============================================================================
// Errors
// ============================================================================

/// Error types occurring in the Apps Settings category.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppSettingsError {
    AppNotFound(String),
    AlreadyExists(String),
    UninstallBlocked(String),
    PolicyBlocked(String),
    AliasNotFound(String),
    ExtensionNotFound(String),
    ProtocolNotFound(String),
    InvalidConfiguration(String),
    ModalNotActive,
}

impl fmt::Display for AppSettingsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AppNotFound(id) => write!(f, "Application not found: {}", id),
            Self::AlreadyExists(name) => write!(f, "Application or alias already exists: {}", name),
            Self::UninstallBlocked(reason) => write!(f, "Cannot uninstall application: {}", reason),
            Self::PolicyBlocked(reason) => write!(f, "Installation policy blocked action: {}", reason),
            Self::AliasNotFound(alias) => write!(f, "Execution alias not found: {}", alias),
            Self::ExtensionNotFound(ext) => write!(f, "File extension mapping not found: {}", ext),
            Self::ProtocolNotFound(proto) => write!(f, "Protocol handler not found: {}", proto),
            Self::InvalidConfiguration(msg) => write!(f, "Invalid settings configuration: {}", msg),
            Self::ModalNotActive => write!(f, "No active modal dialog to confirm or cancel"),
        }
    }
}

impl std::error::Error for AppSettingsError {}

// ============================================================================
// Data Structures
// ============================================================================

/// Detailed record of an installed application in HybridOS.
#[derive(Debug, Clone, PartialEq)]
pub struct InstalledApp {
    /// Unique identifier / package full name.
    pub id: String,
    /// Display name of the application.
    pub name: String,
    /// Publisher or vendor name.
    pub publisher: String,
    /// Semantic version string (e.g. "128.0.2739.67").
    pub version: String,
    /// ISO-8601 formatted install date (e.g. "2026-10-15").
    pub install_date: String,
    /// Base application binary size in Megabytes.
    pub size_mb: f64,
    /// Filesystem executable path.
    pub install_path: String,
    /// Runtime personality / app architecture.
    pub app_type: AppType,
    /// Drive letter or volume prefix (e.g. "C:", "D:").
    pub drive: String,
    /// Granular permissions map.
    pub permissions: HashMap<AppPermission, bool>,
    /// Whether the process is currently running.
    pub is_running: bool,
    /// Whether the app supports reconfiguration / repair / modify.
    pub can_modify: bool,
    /// Whether the app can be uninstalled by the user.
    pub can_uninstall: bool,
    /// File extensions supported by this application.
    pub supported_extensions: Vec<String>,
    /// URI protocols supported by this application.
    pub supported_protocols: Vec<String>,
    /// User profile data size in Megabytes.
    pub data_size_mb: f64,
    /// Cache and temporary files size in Megabytes.
    pub cache_size_mb: f64,
}

impl InstalledApp {
    /// Returns total disk footprint (binary + user data + cache).
    pub fn total_size_mb(&self) -> f64 {
        self.size_mb + self.data_size_mb + self.cache_size_mb
    }

    /// Resets app user data and cache to zero, keeping the core binary intact.
    pub fn reset_data(&mut self) -> f64 {
        let freed = self.data_size_mb + self.cache_size_mb;
        self.data_size_mb = 0.0;
        self.cache_size_mb = 0.0;
        freed
    }
}

/// Command-line application execution alias (e.g. wt.exe, python.exe).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppExecutionAlias {
    /// Command alias name (e.g. "wt.exe", "python.exe", "cmd.exe", "bash").
    pub alias: String,
    /// Associated application name.
    pub app_name: String,
    /// Target binary path invoked by this alias.
    pub target_path: String,
    /// Whether this execution alias trigger is currently enabled.
    pub enabled: bool,
    /// Description of the alias behavior.
    pub description: String,
}

/// Startup entry for applications launching on user logon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartupApp {
    /// Unique identifier for the startup item.
    pub id: String,
    /// Display name of the startup application.
    pub name: String,
    /// Publisher name.
    pub publisher: String,
    /// Command line executable and arguments.
    pub command: String,
    /// Enabled status on Windows boot.
    pub enabled: bool,
    /// Impact rating on boot time.
    pub impact: StartupImpact,
    /// Target registry hive and key path.
    pub reg_path: String,
}

/// In-memory Windows NT Registry `Run` keys storage for synchronization.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RegistryRunStore {
    hkcu_run: BTreeMap<String, String>,
    hklm_run: BTreeMap<String, String>,
}

impl RegistryRunStore {
    /// Creates a new empty registry run store.
    pub fn new() -> Self {
        Self {
            hkcu_run: BTreeMap::new(),
            hklm_run: BTreeMap::new(),
        }
    }

    /// Sets or updates an entry in `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`.
    pub fn set_hkcu_run(&mut self, app_name: &str, command: &str) {
        self.hkcu_run.insert(app_name.to_string(), command.to_string());
    }

    /// Removes an entry from `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`.
    pub fn remove_hkcu_run(&mut self, app_name: &str) -> Option<String> {
        self.hkcu_run.remove(app_name)
    }

    /// Queries an entry in `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`.
    pub fn get_hkcu_run(&self, app_name: &str) -> Option<&str> {
        self.hkcu_run.get(app_name).map(|s| s.as_str())
    }

    /// Checks if a key exists in HKCU Run.
    pub fn has_hkcu_run(&self, app_name: &str) -> bool {
        self.hkcu_run.contains_key(app_name)
    }

    /// Sets an entry in `HKLM\Software\Microsoft\Windows\CurrentVersion\Run`.
    pub fn set_hklm_run(&mut self, app_name: &str, command: &str) {
        self.hklm_run.insert(app_name.to_string(), command.to_string());
    }

    /// Removes an entry from `HKLM\Software\Microsoft\Windows\CurrentVersion\Run`.
    pub fn remove_hklm_run(&mut self, app_name: &str) -> Option<String> {
        self.hklm_run.remove(app_name)
    }

    /// Returns a reference to all HKCU Run entries.
    pub fn all_hkcu(&self) -> &BTreeMap<String, String> {
        &self.hkcu_run
    }

    /// Returns a reference to all HKLM Run entries.
    pub fn all_hklm(&self) -> &BTreeMap<String, String> {
        &self.hklm_run
    }
}

/// Registry of default applications for file extensions and URI protocols.
#[derive(Debug, Clone, Default)]
pub struct DefaultAssociationsRegistry {
    /// File extension -> Default app name (e.g. ".txt" -> "Notepad")
    extensions: HashMap<String, String>,
    /// File extension -> Candidate app names
    extension_candidates: HashMap<String, Vec<String>>,
    /// Protocol -> Default app name (e.g. "http" -> "Microsoft Edge")
    protocols: HashMap<String, String>,
    /// Protocol -> Candidate app names
    protocol_candidates: HashMap<String, Vec<String>>,
}

impl DefaultAssociationsRegistry {
    /// Creates a new default associations registry.
    pub fn new() -> Self {
        Self {
            extensions: HashMap::new(),
            extension_candidates: HashMap::new(),
            protocols: HashMap::new(),
            protocol_candidates: HashMap::new(),
        }
    }

    /// Registers a file extension mapping with candidate apps.
    pub fn register_extension(&mut self, ext: &str, default_app: &str, candidates: &[&str]) {
        let normalized = normalize_ext(ext);
        self.extensions.insert(normalized.clone(), default_app.to_string());
        let mut list: Vec<String> = candidates.iter().map(|s| s.to_string()).collect();
        if !list.contains(&default_app.to_string()) {
            list.insert(0, default_app.to_string());
        }
        self.extension_candidates.insert(normalized, list);
    }

    /// Sets the default application for a specific extension.
    pub fn set_default_extension(&mut self, ext: &str, app: &str) -> Result<(), AppSettingsError> {
        let normalized = normalize_ext(ext);
        if !self.extensions.contains_key(&normalized) {
            return Err(AppSettingsError::ExtensionNotFound(normalized));
        }
        self.extensions.insert(normalized.clone(), app.to_string());
        let candidates = self.extension_candidates.entry(normalized).or_default();
        if !candidates.contains(&app.to_string()) {
            candidates.push(app.to_string());
        }
        Ok(())
    }

    /// Returns the current default application for a file extension.
    pub fn get_default_extension(&self, ext: &str) -> Option<&str> {
        let normalized = normalize_ext(ext);
        self.extensions.get(&normalized).map(|s| s.as_str())
    }

    /// Returns candidate applications for a file extension.
    pub fn get_extension_candidates(&self, ext: &str) -> &[String] {
        let normalized = normalize_ext(ext);
        self.extension_candidates.get(&normalized).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Registers a URI protocol mapping with candidate apps.
    pub fn register_protocol(&mut self, proto: &str, default_app: &str, candidates: &[&str]) {
        let normalized = proto.trim().to_lowercase();
        self.protocols.insert(normalized.clone(), default_app.to_string());
        let mut list: Vec<String> = candidates.iter().map(|s| s.to_string()).collect();
        if !list.contains(&default_app.to_string()) {
            list.insert(0, default_app.to_string());
        }
        self.protocol_candidates.insert(normalized, list);
    }

    /// Sets the default application for a URI protocol.
    pub fn set_default_protocol(&mut self, proto: &str, app: &str) -> Result<(), AppSettingsError> {
        let normalized = proto.trim().to_lowercase();
        if !self.protocols.contains_key(&normalized) {
            return Err(AppSettingsError::ProtocolNotFound(normalized));
        }
        self.protocols.insert(normalized.clone(), app.to_string());
        let candidates = self.protocol_candidates.entry(normalized).or_default();
        if !candidates.contains(&app.to_string()) {
            candidates.push(app.to_string());
        }
        Ok(())
    }

    /// Returns the current default application for a URI protocol.
    pub fn get_default_protocol(&self, proto: &str) -> Option<&str> {
        let normalized = proto.trim().to_lowercase();
        self.protocols.get(&normalized).map(|s| s.as_str())
    }

    /// Returns candidate applications for a URI protocol.
    pub fn get_protocol_candidates(&self, proto: &str) -> &[String] {
        let normalized = proto.trim().to_lowercase();
        self.protocol_candidates.get(&normalized).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Batch sets defaults by application for all its supported extensions and protocols.
    pub fn set_defaults_by_app(&mut self, app_name: &str, exts: &[String], protos: &[String]) -> usize {
        let mut count = 0;
        for ext in exts {
            let normalized = normalize_ext(ext);
            if self.extensions.contains_key(&normalized) {
                self.extensions.insert(normalized.clone(), app_name.to_string());
                let candidates = self.extension_candidates.entry(normalized).or_default();
                if !candidates.contains(&app_name.to_string()) {
                    candidates.push(app_name.to_string());
                }
                count += 1;
            }
        }
        for proto in protos {
            let normalized = proto.trim().to_lowercase();
            if self.protocols.contains_key(&normalized) {
                self.protocols.insert(normalized.clone(), app_name.to_string());
                let candidates = self.protocol_candidates.entry(normalized).or_default();
                if !candidates.contains(&app_name.to_string()) {
                    candidates.push(app_name.to_string());
                }
                count += 1;
            }
        }
        count
    }

    /// Removes an application from defaults and falls back to an alternative candidate.
    pub fn remove_app_from_defaults(&mut self, app_name: &str) {
        for (ext, default_app) in self.extensions.iter_mut() {
            if default_app == app_name {
                if let Some(candidates) = self.extension_candidates.get_mut(ext) {
                    candidates.retain(|c| c != app_name);
                    if let Some(first) = candidates.first() {
                        *default_app = first.clone();
                    } else {
                        *default_app = "None".to_string();
                    }
                }
            }
        }
        for (proto, default_app) in self.protocols.iter_mut() {
            if default_app == app_name {
                if let Some(candidates) = self.protocol_candidates.get_mut(proto) {
                    candidates.retain(|c| c != app_name);
                    if let Some(first) = candidates.first() {
                        *default_app = first.clone();
                    } else {
                        *default_app = "None".to_string();
                    }
                }
            }
        }
    }

    /// Restores Microsoft recommended defaults for standard file extensions and protocols.
    pub fn reset_to_recommended(&mut self) {
        self.extensions.insert(".txt".to_string(), "Notepad".to_string());
        self.extensions.insert(".html".to_string(), "Microsoft Edge".to_string());
        self.extensions.insert(".htm".to_string(), "Microsoft Edge".to_string());
        self.extensions.insert(".pdf".to_string(), "Microsoft Edge".to_string());
        self.extensions.insert(".png".to_string(), "Photos".to_string());
        self.extensions.insert(".jpg".to_string(), "Photos".to_string());
        self.extensions.insert(".jpeg".to_string(), "Photos".to_string());
        self.extensions.insert(".mp4".to_string(), "Media Player".to_string());
        self.extensions.insert(".mp3".to_string(), "Media Player".to_string());
        self.extensions.insert(".zip".to_string(), "File Explorer".to_string());
        self.extensions.insert(".exe".to_string(), "Windows Subsystem".to_string());
        self.extensions.insert(".elf".to_string(), "Linux Subsystem".to_string());

        self.protocols.insert("http".to_string(), "Microsoft Edge".to_string());
        self.protocols.insert("https".to_string(), "Microsoft Edge".to_string());
        self.protocols.insert("mailto".to_string(), "Mail and Calendar".to_string());
        self.protocols.insert("ms-settings".to_string(), "Settings App".to_string());
        self.protocols.insert("calculator".to_string(), "Calculator".to_string());
        self.protocols.insert("file".to_string(), "File Explorer".to_string());
        self.protocols.insert("ssh".to_string(), "Windows Terminal".to_string());
    }

    /// Returns all registered extensions sorted alphabetically.
    pub fn all_extensions(&self) -> Vec<(String, String)> {
        let mut list: Vec<(String, String)> = self
            .extensions
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        list.sort_by(|a, b| a.0.cmp(&b.0));
        list
    }

    /// Returns all registered protocols sorted alphabetically.
    pub fn all_protocols(&self) -> Vec<(String, String)> {
        let mut list: Vec<(String, String)> = self
            .protocols
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        list.sort_by(|a, b| a.0.cmp(&b.0));
        list
    }
}

/// Helper function to normalize extension strings (leading dot, lowercase).
fn normalize_ext(ext: &str) -> String {
    let trimmed = ext.trim().to_lowercase();
    if trimmed.starts_with('.') {
        trimmed
    } else {
        format!(".{}", trimmed)
    }
}

/// State representation for the Uninstall confirmation modal dialog.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UninstallModal {
    /// Whether the modal dialog is currently presented to the user.
    pub is_open: bool,
    /// Target application package ID.
    pub target_app_id: Option<String>,
    /// Display name of the application to be uninstalled.
    pub target_app_name: Option<String>,
    /// Informational warning message.
    pub message: String,
    /// Whether uninstallation is actively executing.
    pub is_in_progress: bool,
    /// Optional error encountered during uninstallation.
    pub error_message: Option<String>,
}

// ============================================================================
// Main Apps Settings Manager
// ============================================================================

/// Central controller and state manager for the Windows 11 Apps Settings Category.
#[derive(Debug, Clone)]
pub struct AppsSettingsManager {
    /// Catalog of installed applications indexed by package ID.
    installed_apps: HashMap<String, InstalledApp>,
    /// Policy determining allowed installation sources.
    install_source_policy: AppInstallSourcePolicy,
    /// Share across devices master toggle.
    share_across_devices: bool,
    /// Sharing scope when enabled.
    share_mode: ShareAcrossDevicesMode,
    /// Command execution aliases.
    aliases: Vec<AppExecutionAlias>,
    /// Default application associations for file types and protocols.
    defaults: DefaultAssociationsRegistry,
    /// Startup applications list.
    startup_apps: Vec<StartupApp>,
    /// Windows NT simulated Registry Run store (`HKCU\...\Run`).
    registry_run_keys: RegistryRunStore,
    /// Currently active settings subpage.
    active_subpage: AppsCategorySubpage,
    /// Search filter string for installed apps.
    search_query: String,
    /// Selected sort order for installed apps.
    sort_order: AppSortOrder,
    /// Selected drive filter.
    drive_filter: DriveFilter,
    /// Selected app runtime filter.
    type_filter: AppTypeFilter,
    /// Uninstall confirmation modal dialog state.
    uninstall_modal: UninstallModal,
    /// Status message from the last completed user operation.
    last_status: Option<String>,
}

impl AppsSettingsManager {
    /// Constructs a fully populated, production-grade Apps Settings Manager.
    pub fn new() -> Self {
        let mut manager = Self {
            installed_apps: HashMap::new(),
            install_source_policy: AppInstallSourcePolicy::AnywhereWarn,
            share_across_devices: true,
            share_mode: ShareAcrossDevicesMode::MyDevicesOnly,
            aliases: Vec::new(),
            defaults: DefaultAssociationsRegistry::new(),
            startup_apps: Vec::new(),
            registry_run_keys: RegistryRunStore::new(),
            active_subpage: AppsCategorySubpage::InstalledApps,
            search_query: String::new(),
            sort_order: AppSortOrder::NameAsc,
            drive_filter: DriveFilter::AllDrives,
            type_filter: AppTypeFilter::AllTypes,
            uninstall_modal: UninstallModal::default(),
            last_status: None,
        };

        manager.populate_initial_catalog();
        manager.populate_initial_aliases();
        manager.populate_initial_defaults();
        manager.populate_initial_startup_apps();

        manager
    }

    // ------------------------------------------------------------------------
    // Catalog Initialization
    // ------------------------------------------------------------------------

    fn populate_initial_catalog(&mut self) {
        let mut default_permissions = HashMap::new();
        default_permissions.insert(AppPermission::BackgroundApps, true);
        default_permissions.insert(AppPermission::Camera, false);
        default_permissions.insert(AppPermission::Microphone, false);
        default_permissions.insert(AppPermission::Location, false);
        default_permissions.insert(AppPermission::FileSystem, true);
        default_permissions.insert(AppPermission::Notifications, true);

        let apps = vec![
            InstalledApp {
                id: "Microsoft.Edge".to_string(),
                name: "Microsoft Edge".to_string(),
                publisher: "Microsoft Corporation".to_string(),
                version: "128.0.2739.67".to_string(),
                install_date: "2026-09-12".to_string(),
                size_mb: 412.5,
                install_path: r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe".to_string(),
                app_type: AppType::Win32Desktop,
                drive: "C:".to_string(),
                permissions: default_permissions.clone(),
                is_running: true,
                can_modify: true,
                can_uninstall: false, // Essential system component
                supported_extensions: vec![
                    ".html".into(),
                    ".htm".into(),
                    ".pdf".into(),
                    ".svg".into(),
                    ".xhtml".into(),
                ],
                supported_protocols: vec!["http".into(), "https".into(), "ftp".into()],
                data_size_mb: 184.0,
                cache_size_mb: 260.5,
            },
            InstalledApp {
                id: "Microsoft.WindowsNotepad".to_string(),
                name: "Notepad".to_string(),
                publisher: "Microsoft Corporation".to_string(),
                version: "11.2405.21.0".to_string(),
                install_date: "2026-09-10".to_string(),
                size_mb: 32.4,
                install_path: r"C:\Program Files\WindowsApps\Microsoft.WindowsNotepad_11.2405.21.0_x64__8wekyb3d8bbwe\Notepad.exe".to_string(),
                app_type: AppType::UwpWinUi,
                drive: "C:".to_string(),
                permissions: default_permissions.clone(),
                is_running: false,
                can_modify: false,
                can_uninstall: true,
                supported_extensions: vec![".txt".into(), ".log".into(), ".ini".into(), ".cfg".into(), ".md".into()],
                supported_protocols: vec![],
                data_size_mb: 2.1,
                cache_size_mb: 0.8,
            },
            InstalledApp {
                id: "Microsoft.WindowsPhotos".to_string(),
                name: "Photos".to_string(),
                publisher: "Microsoft Corporation".to_string(),
                version: "2024.11070.12001.0".to_string(),
                install_date: "2026-09-15".to_string(),
                size_mb: 184.2,
                install_path: r"C:\Program Files\WindowsApps\Microsoft.Windows.Photos_2024.11070.12001.0_x64__8wekyb3d8bbwe\Photos.exe".to_string(),
                app_type: AppType::UwpWinUi,
                drive: "C:".to_string(),
                permissions: default_permissions.clone(),
                is_running: false,
                can_modify: true,
                can_uninstall: true,
                supported_extensions: vec![
                    ".png".into(),
                    ".jpg".into(),
                    ".jpeg".into(),
                    ".bmp".into(),
                    ".gif".into(),
                    ".webp".into(),
                    ".ico".into(),
                ],
                supported_protocols: vec![],
                data_size_mb: 45.0,
                cache_size_mb: 120.0,
            },
            InstalledApp {
                id: "Microsoft.ZuneMusic".to_string(),
                name: "Media Player".to_string(),
                publisher: "Microsoft Corporation".to_string(),
                version: "11.2403.4.0".to_string(),
                install_date: "2026-09-18".to_string(),
                size_mb: 128.7,
                install_path: r"C:\Program Files\WindowsApps\Microsoft.ZuneMusic_11.2403.4.0_x64__8wekyb3d8bbwe\Microsoft.Media.Player.exe".to_string(),
                app_type: AppType::UwpWinUi,
                drive: "C:".to_string(),
                permissions: default_permissions.clone(),
                is_running: false,
                can_modify: true,
                can_uninstall: true,
                supported_extensions: vec![
                    ".mp4".into(),
                    ".mp3".into(),
                    ".mkv".into(),
                    ".wav".into(),
                    ".flac".into(),
                    ".avi".into(),
                    ".wmv".into(),
                ],
                supported_protocols: vec!["ms-media-player".into()],
                data_size_mb: 15.4,
                cache_size_mb: 58.2,
            },
            InstalledApp {
                id: "Microsoft.Windows.Explorer".to_string(),
                name: "File Explorer".to_string(),
                publisher: "Microsoft Corporation".to_string(),
                version: "10.0.22631.3880".to_string(),
                install_date: "2026-09-01".to_string(),
                size_mb: 64.0,
                install_path: r"C:\Windows\explorer.exe".to_string(),
                app_type: AppType::Win32Desktop,
                drive: "C:".to_string(),
                permissions: default_permissions.clone(),
                is_running: true,
                can_modify: false,
                can_uninstall: false,
                supported_extensions: vec![".zip".into(), ".tar".into(), ".gz".into()],
                supported_protocols: vec!["file".into(), "search".into()],
                data_size_mb: 30.0,
                cache_size_mb: 85.0,
            },
            InstalledApp {
                id: "HybridOS.Subsystem.Windows".to_string(),
                name: "Windows Subsystem".to_string(),
                publisher: "HybridOS Systems".to_string(),
                version: "1.0.0.1".to_string(),
                install_date: "2026-09-01".to_string(),
                size_mb: 512.0,
                install_path: r"C:\Windows\System32\ntoskrnl.exe".to_string(),
                app_type: AppType::Win32Desktop,
                drive: "C:".to_string(),
                permissions: default_permissions.clone(),
                is_running: true,
                can_modify: false,
                can_uninstall: false,
                supported_extensions: vec![".exe".into(), ".dll".into(), ".sys".into(), ".msi".into()],
                supported_protocols: vec!["win-pe".into()],
                data_size_mb: 120.0,
                cache_size_mb: 40.0,
            },
            InstalledApp {
                id: "HybridOS.Subsystem.Linux".to_string(),
                name: "Linux Subsystem".to_string(),
                publisher: "HybridOS Systems".to_string(),
                version: "6.6.32-hybridos".to_string(),
                install_date: "2026-09-01".to_string(),
                size_mb: 780.0,
                install_path: r"C:\HybridOS\linux\vmlinux.bin".to_string(),
                app_type: AppType::LinuxElf,
                drive: "C:".to_string(),
                permissions: default_permissions.clone(),
                is_running: true,
                can_modify: true,
                can_uninstall: false,
                supported_extensions: vec![".elf".into(), ".so".into(), ".bin".into(), ".sh".into()],
                supported_protocols: vec!["ssh".into(), "bash".into()],
                data_size_mb: 340.0,
                cache_size_mb: 110.0,
            },
            InstalledApp {
                id: "Microsoft.WindowsTerminal".to_string(),
                name: "Windows Terminal".to_string(),
                publisher: "Microsoft Corporation".to_string(),
                version: "1.21.2361.0".to_string(),
                install_date: "2026-09-20".to_string(),
                size_mb: 96.5,
                install_path: r"C:\Program Files\WindowsApps\Microsoft.WindowsTerminal_1.21.2361.0_x64__8wekyb3d8bbwe\wt.exe".to_string(),
                app_type: AppType::UwpWinUi,
                drive: "C:".to_string(),
                permissions: default_permissions.clone(),
                is_running: false,
                can_modify: true,
                can_uninstall: true,
                supported_extensions: vec![".ps1".into(), ".bat".into(), ".cmd".into()],
                supported_protocols: vec!["ssh".into(), "telnet".into()],
                data_size_mb: 12.0,
                cache_size_mb: 8.5,
            },
            InstalledApp {
                id: "Microsoft.Windows.Settings".to_string(),
                name: "Settings App".to_string(),
                publisher: "Microsoft Corporation".to_string(),
                version: "10.0.22631.3880".to_string(),
                install_date: "2026-09-01".to_string(),
                size_mb: 48.0,
                install_path: r"C:\Windows\ImmersiveControlPanel\SystemSettings.exe".to_string(),
                app_type: AppType::UwpWinUi,
                drive: "C:".to_string(),
                permissions: default_permissions.clone(),
                is_running: true,
                can_modify: false,
                can_uninstall: false,
                supported_extensions: vec![],
                supported_protocols: vec!["ms-settings".into()],
                data_size_mb: 18.0,
                cache_size_mb: 5.0,
            },
            InstalledApp {
                id: "Microsoft.VisualStudioCode".to_string(),
                name: "Visual Studio Code".to_string(),
                publisher: "Microsoft Corporation".to_string(),
                version: "1.92.2".to_string(),
                install_date: "2026-09-25".to_string(),
                size_mb: 380.0,
                install_path: r"D:\DevTools\VSCode\Code.exe".to_string(),
                app_type: AppType::Win32Desktop,
                drive: "D:".to_string(),
                permissions: default_permissions.clone(),
                is_running: false,
                can_modify: true,
                can_uninstall: true,
                supported_extensions: vec![
                    ".rs".into(),
                    ".c".into(),
                    ".cpp".into(),
                    ".js".into(),
                    ".ts".into(),
                    ".json".into(),
                    ".py".into(),
                    ".txt".into(),
                ],
                supported_protocols: vec!["vscode".into()],
                data_size_mb: 140.0,
                cache_size_mb: 220.0,
            },
            InstalledApp {
                id: "Canonical.Ubuntu2404".to_string(),
                name: "Ubuntu 24.04 LTS".to_string(),
                publisher: "Canonical Ltd.".to_string(),
                version: "24.04.1".to_string(),
                install_date: "2026-10-01".to_string(),
                size_mb: 1850.0,
                install_path: r"/usr/bin/bash".to_string(),
                app_type: AppType::LinuxElf,
                drive: "C:".to_string(),
                permissions: default_permissions.clone(),
                is_running: false,
                can_modify: true,
                can_uninstall: true,
                supported_extensions: vec![".deb".into(), ".rpm".into(), ".tar.xz".into(), ".sh".into()],
                supported_protocols: vec!["apt".into(), "snap".into()],
                data_size_mb: 520.0,
                cache_size_mb: 310.0,
            },
            InstalledApp {
                id: "PythonSoftwareFoundation.Python312".to_string(),
                name: "Python 3.12".to_string(),
                publisher: "Python Software Foundation".to_string(),
                version: "3.12.5".to_string(),
                install_date: "2026-10-05".to_string(),
                size_mb: 110.0,
                install_path: r"C:\Program Files\Python312\python.exe".to_string(),
                app_type: AppType::Win32Desktop,
                drive: "C:".to_string(),
                permissions: default_permissions.clone(),
                is_running: false,
                can_modify: true,
                can_uninstall: true,
                supported_extensions: vec![".py".into(), ".pyw".into()],
                supported_protocols: vec![],
                data_size_mb: 15.0,
                cache_size_mb: 32.0,
            },
            InstalledApp {
                id: "IgorPavlov.7Zip".to_string(),
                name: "7-Zip File Manager".to_string(),
                publisher: "Igor Pavlov".to_string(),
                version: "24.07".to_string(),
                install_date: "2026-09-05".to_string(),
                size_mb: 5.8,
                install_path: r"D:\Utilities\7-Zip\7zFM.exe".to_string(),
                app_type: AppType::Win32Desktop,
                drive: "D:".to_string(),
                permissions: default_permissions.clone(),
                is_running: false,
                can_modify: false,
                can_uninstall: true,
                supported_extensions: vec![
                    ".7z".into(),
                    ".rar".into(),
                    ".zip".into(),
                    ".tar".into(),
                    ".gz".into(),
                    ".bz2".into(),
                ],
                supported_protocols: vec![],
                data_size_mb: 0.5,
                cache_size_mb: 0.1,
            },
            InstalledApp {
                id: "Microsoft.WindowsCalculator".to_string(),
                name: "Calculator".to_string(),
                publisher: "Microsoft Corporation".to_string(),
                version: "11.2401.0.0".to_string(),
                install_date: "2026-09-10".to_string(),
                size_mb: 18.5,
                install_path: r"C:\Program Files\WindowsApps\Microsoft.WindowsCalculator_11.2401.0.0_x64__8wekyb3d8bbwe\CalculatorApp.exe".to_string(),
                app_type: AppType::UwpWinUi,
                drive: "C:".to_string(),
                permissions: default_permissions,
                is_running: false,
                can_modify: false,
                can_uninstall: true,
                supported_extensions: vec![],
                supported_protocols: vec!["calculator".into()],
                data_size_mb: 1.2,
                cache_size_mb: 0.4,
            },
        ];

        for app in apps {
            self.installed_apps.insert(app.id.clone(), app);
        }
    }

    fn populate_initial_aliases(&mut self) {
        self.aliases = vec![
            AppExecutionAlias {
                alias: "wt.exe".to_string(),
                app_name: "Windows Terminal".to_string(),
                target_path: r"C:\Program Files\WindowsApps\Microsoft.WindowsTerminal_1.21.2361.0_x64__8wekyb3d8bbwe\wt.exe".to_string(),
                enabled: true,
                description: "Launches the Windows Terminal modern tabbed console".to_string(),
            },
            AppExecutionAlias {
                alias: "python.exe".to_string(),
                app_name: "Python 3.12".to_string(),
                target_path: r"C:\Program Files\Python312\python.exe".to_string(),
                enabled: true,
                description: "Launches the Python 3.12 interpreter runtime".to_string(),
            },
            AppExecutionAlias {
                alias: "python3.exe".to_string(),
                app_name: "Python 3.12".to_string(),
                target_path: r"C:\Program Files\Python312\python.exe".to_string(),
                enabled: true,
                description: "POSIX compatibility alias for Python 3".to_string(),
            },
            AppExecutionAlias {
                alias: "cmd.exe".to_string(),
                app_name: "Windows Subsystem".to_string(),
                target_path: r"C:\Windows\System32\cmd.exe".to_string(),
                enabled: true,
                description: "Windows Command Processor shell".to_string(),
            },
            AppExecutionAlias {
                alias: "bash".to_string(),
                app_name: "Linux Subsystem".to_string(),
                target_path: r"/usr/bin/bash".to_string(),
                enabled: true,
                description: "POSIX Bourne-Again Shell environment".to_string(),
            },
            AppExecutionAlias {
                alias: "code.exe".to_string(),
                app_name: "Visual Studio Code".to_string(),
                target_path: r"D:\DevTools\VSCode\Code.exe".to_string(),
                enabled: true,
                description: "Visual Studio Code editor launcher".to_string(),
            },
        ];
    }

    fn populate_initial_defaults(&mut self) {
        // File extension associations
        self.defaults.register_extension(
            ".txt",
            "Notepad",
            &["Notepad", "Visual Studio Code", "Ubuntu 24.04 LTS"],
        );
        self.defaults.register_extension(
            ".html",
            "Microsoft Edge",
            &["Microsoft Edge", "Visual Studio Code"],
        );
        self.defaults.register_extension(
            ".htm",
            "Microsoft Edge",
            &["Microsoft Edge", "Visual Studio Code"],
        );
        self.defaults.register_extension(".pdf", "Microsoft Edge", &["Microsoft Edge"]);
        self.defaults.register_extension(
            ".png",
            "Photos",
            &["Photos", "Microsoft Edge"],
        );
        self.defaults.register_extension(
            ".jpg",
            "Photos",
            &["Photos", "Microsoft Edge"],
        );
        self.defaults.register_extension(
            ".jpeg",
            "Photos",
            &["Photos", "Microsoft Edge"],
        );
        self.defaults.register_extension(".mp4", "Media Player", &["Media Player"]);
        self.defaults.register_extension(".mp3", "Media Player", &["Media Player"]);
        self.defaults.register_extension(
            ".zip",
            "File Explorer",
            &["File Explorer", "7-Zip File Manager"],
        );
        self.defaults.register_extension(
            ".exe",
            "Windows Subsystem",
            &["Windows Subsystem"],
        );
        self.defaults.register_extension(
            ".elf",
            "Linux Subsystem",
            &["Linux Subsystem", "Ubuntu 24.04 LTS"],
        );
        self.defaults.register_extension(
            ".py",
            "Python 3.12",
            &["Python 3.12", "Visual Studio Code"],
        );
        self.defaults.register_extension(
            ".rs",
            "Visual Studio Code",
            &["Visual Studio Code", "Notepad"],
        );
        self.defaults.register_extension(
            ".7z",
            "7-Zip File Manager",
            &["7-Zip File Manager", "File Explorer"],
        );

        // Protocol handlers
        self.defaults.register_protocol("http", "Microsoft Edge", &["Microsoft Edge"]);
        self.defaults.register_protocol("https", "Microsoft Edge", &["Microsoft Edge"]);
        self.defaults.register_protocol("mailto", "Mail and Calendar", &["Mail and Calendar", "Microsoft Edge"]);
        self.defaults.register_protocol("ms-settings", "Settings App", &["Settings App"]);
        self.defaults.register_protocol("calculator", "Calculator", &["Calculator"]);
        self.defaults.register_protocol("file", "File Explorer", &["File Explorer"]);
        self.defaults.register_protocol("ssh", "Windows Terminal", &["Windows Terminal", "Ubuntu 24.04 LTS"]);
    }

    fn populate_initial_startup_apps(&mut self) {
        let startup_items = vec![
            StartupApp {
                id: "startup_edge".to_string(),
                name: "Microsoft Edge Auto-Launcher".to_string(),
                publisher: "Microsoft Corporation".to_string(),
                command: r#""C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe" --no-startup-window --win-session-start"#.to_string(),
                enabled: false,
                impact: StartupImpact::Low,
                reg_path: REG_KEY_HKCU_RUN.to_string(),
            },
            StartupApp {
                id: "startup_security".to_string(),
                name: "Windows Security notification icon".to_string(),
                publisher: "Microsoft Corporation".to_string(),
                command: r#""C:\Program Files\Windows Defender\MSASCuiL.exe""#.to_string(),
                enabled: true,
                impact: StartupImpact::Low,
                reg_path: REG_KEY_HKCU_RUN.to_string(),
            },
            StartupApp {
                id: "startup_onedrive".to_string(),
                name: "OneDrive".to_string(),
                publisher: "Microsoft Corporation".to_string(),
                command: r#""C:\Users\Vinicius\AppData\Local\Microsoft\OneDrive\OneDrive.exe" /background"#.to_string(),
                enabled: true,
                impact: StartupImpact::Medium,
                reg_path: REG_KEY_HKCU_RUN.to_string(),
            },
            StartupApp {
                id: "startup_spotify".to_string(),
                name: "Spotify".to_string(),
                publisher: "Spotify AB".to_string(),
                command: r#""C:\Users\Vinicius\AppData\Roaming\Spotify\Spotify.exe" --minimized"#.to_string(),
                enabled: false,
                impact: StartupImpact::High,
                reg_path: REG_KEY_HKCU_RUN.to_string(),
            },
            StartupApp {
                id: "startup_hybridd".to_string(),
                name: "HybridOS POSIX Daemon".to_string(),
                publisher: "HybridOS Systems".to_string(),
                command: r#""C:\HybridOS\bin\hybridd.exe" --daemon"#.to_string(),
                enabled: true,
                impact: StartupImpact::None,
                reg_path: REG_KEY_HKCU_RUN.to_string(),
            },
        ];

        for item in startup_items {
            if item.enabled {
                self.registry_run_keys.set_hkcu_run(&item.name, &item.command);
            }
            self.startup_apps.push(item);
        }
    }

    // ========================================================================
    // 1. Installed Apps Management & Actions
    // ========================================================================

    /// Sets the search text query to filter installed apps.
    pub fn set_search_query(&mut self, query: &str) {
        self.search_query = query.trim().to_string();
    }

    /// Sets the sort order for installed applications.
    pub fn set_sort_order(&mut self, order: AppSortOrder) {
        self.sort_order = order;
    }

    /// Sets the drive filter (e.g. All drives, C:, D:).
    pub fn set_drive_filter(&mut self, filter: DriveFilter) {
        self.drive_filter = filter;
    }

    /// Sets the app runtime architecture filter.
    pub fn set_type_filter(&mut self, filter: AppTypeFilter) {
        self.type_filter = filter;
    }

    /// Returns a filtered and sorted slice of installed applications.
    pub fn get_filtered_apps(&self) -> Vec<&InstalledApp> {
        let mut apps: Vec<&InstalledApp> = self
            .installed_apps
            .values()
            .filter(|app| {
                // 1. Search text filter
                if !self.search_query.is_empty() {
                    let q = self.search_query.to_lowercase();
                    let name_match = app.name.to_lowercase().contains(&q);
                    let pub_match = app.publisher.to_lowercase().contains(&q);
                    let path_match = app.install_path.to_lowercase().contains(&q);
                    if !name_match && !pub_match && !path_match {
                        return false;
                    }
                }
                // 2. Drive filter
                if let DriveFilter::SpecificDrive(ref drive) = self.drive_filter {
                    if !app.drive.eq_ignore_ascii_case(drive) {
                        return false;
                    }
                }
                // 3. App Type filter
                if let AppTypeFilter::Only(expected_type) = self.type_filter {
                    if app.app_type != expected_type {
                        return false;
                    }
                }
                true
            })
            .collect();

        // Sort items
        match self.sort_order {
            AppSortOrder::NameAsc => {
                apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
            }
            AppSortOrder::NameDesc => {
                apps.sort_by(|a, b| b.name.to_lowercase().cmp(&a.name.to_lowercase()));
            }
            AppSortOrder::SizeDesc => {
                apps.sort_by(|a, b| b.total_size_mb().partial_cmp(&a.total_size_mb()).unwrap());
            }
            AppSortOrder::SizeAsc => {
                apps.sort_by(|a, b| a.total_size_mb().partial_cmp(&b.total_size_mb()).unwrap());
            }
            AppSortOrder::InstallDateDesc => {
                apps.sort_by(|a, b| b.install_date.cmp(&a.install_date));
            }
            AppSortOrder::InstallDateAsc => {
                apps.sort_by(|a, b| a.install_date.cmp(&b.install_date));
            }
        }

        apps
    }

    /// Queries an installed app by package ID.
    pub fn get_app(&self, id: &str) -> Option<&InstalledApp> {
        self.installed_apps.get(id)
    }

    /// Queries a mutable installed app by package ID.
    pub fn get_app_mut(&mut self, id: &str) -> Option<&mut InstalledApp> {
        self.installed_apps.get_mut(id)
    }

    /// Opens the uninstallation confirmation modal for a given application.
    pub fn open_uninstall_modal(&mut self, app_id: &str) -> Result<(), AppSettingsError> {
        let app = self
            .installed_apps
            .get(app_id)
            .ok_or_else(|| AppSettingsError::AppNotFound(app_id.to_string()))?;

        if !app.can_uninstall {
            return Err(AppSettingsError::UninstallBlocked(format!(
                "{} is an essential HybridOS component and cannot be uninstalled.",
                app.name
            )));
        }

        self.uninstall_modal = UninstallModal {
            is_open: true,
            target_app_id: Some(app.id.clone()),
            target_app_name: Some(app.name.clone()),
            message: format!(
                "This app and its related info will be uninstalled. Are you sure you want to uninstall {}?",
                app.name
            ),
            is_in_progress: false,
            error_message: None,
        };

        Ok(())
    }

    /// Cancels and dismisses the uninstallation confirmation modal.
    pub fn cancel_uninstall_modal(&mut self) {
        self.uninstall_modal = UninstallModal::default();
        self.last_status = Some("Uninstallation cancelled by user.".to_string());
    }

    /// Confirms and completes the uninstallation of the target application in the active modal.
    pub fn confirm_uninstall(&mut self) -> Result<String, AppSettingsError> {
        if !self.uninstall_modal.is_open {
            return Err(AppSettingsError::ModalNotActive);
        }

        let app_id = self
            .uninstall_modal
            .target_app_id
            .clone()
            .ok_or(AppSettingsError::ModalNotActive)?;

        let app = self
            .installed_apps
            .remove(&app_id)
            .ok_or_else(|| AppSettingsError::AppNotFound(app_id.clone()))?;

        // 1. Remove from startup apps inventory and registry Run keys
        let app_name = app.name.clone();
        self.startup_apps.retain(|s| s.name != app_name);
        self.registry_run_keys.remove_hkcu_run(&app_name);
        self.registry_run_keys.remove_hklm_run(&app_name);

        // 2. Remove from execution aliases
        self.aliases.retain(|a| a.app_name != app_name);

        // 3. Deregister default associations and fallback to candidate apps
        self.defaults.remove_app_from_defaults(&app_name);

        let freed_mb = app.total_size_mb();
        let message = format!(
            "Successfully uninstalled {} and freed {:.1} MB.",
            app.name, freed_mb
        );

        self.uninstall_modal = UninstallModal::default();
        self.last_status = Some(message.clone());
        Ok(message)
    }

    /// Terminates all running process instances of an application.
    pub fn terminate_app(&mut self, app_id: &str) -> Result<String, AppSettingsError> {
        let app = self
            .installed_apps
            .get_mut(app_id)
            .ok_or_else(|| AppSettingsError::AppNotFound(app_id.to_string()))?;

        app.is_running = false;
        let msg = format!("Terminated all active processes for {}.", app.name);
        self.last_status = Some(msg.clone());
        Ok(msg)
    }

    /// Resets app user data and cache back to initial installation defaults.
    pub fn reset_app(&mut self, app_id: &str) -> Result<String, AppSettingsError> {
        let app = self
            .installed_apps
            .get_mut(app_id)
            .ok_or_else(|| AppSettingsError::AppNotFound(app_id.to_string()))?;

        let freed_mb = app.reset_data();
        let msg = format!(
            "App {} was reset. Reclaimed {:.1} MB of user data and cache.",
            app.name, freed_mb
        );
        self.last_status = Some(msg.clone());
        Ok(msg)
    }

    /// Triggers modification / repair on an application.
    pub fn modify_app(&mut self, app_id: &str) -> Result<String, AppSettingsError> {
        let app = self
            .installed_apps
            .get(app_id)
            .ok_or_else(|| AppSettingsError::AppNotFound(app_id.to_string()))?;

        if !app.can_modify {
            return Err(AppSettingsError::InvalidConfiguration(format!(
                "{} does not support repair or modification.",
                app.name
            )));
        }

        let msg = format!("Launched installer repair and configuration wizard for {}.", app.name);
        self.last_status = Some(msg.clone());
        Ok(msg)
    }

    /// Toggles a granular permission for an application.
    pub fn set_app_permission(
        &mut self,
        app_id: &str,
        permission: AppPermission,
        allowed: bool,
    ) -> Result<(), AppSettingsError> {
        let app = self
            .installed_apps
            .get_mut(app_id)
            .ok_or_else(|| AppSettingsError::AppNotFound(app_id.to_string()))?;

        app.permissions.insert(permission, allowed);
        Ok(())
    }

    /// Installs a new application respecting the installation source policy.
    pub fn install_app(&mut self, app: InstalledApp) -> Result<(), AppSettingsError> {
        if self.installed_apps.contains_key(&app.id) {
            return Err(AppSettingsError::AlreadyExists(app.name));
        }

        // Validate policy
        if self.install_source_policy == AppInstallSourcePolicy::StoreOnly
            && app.app_type != AppType::UwpWinUi
        {
            return Err(AppSettingsError::PolicyBlocked(
                "Windows is set to allow apps from the Microsoft Store only to help protect your PC."
                    .to_string(),
            ));
        }

        self.installed_apps.insert(app.id.clone(), app);
        Ok(())
    }

    // ========================================================================
    // 2. Advanced App Settings
    // ========================================================================

    /// Updates the application installation source policy.
    pub fn set_install_source_policy(&mut self, policy: AppInstallSourcePolicy) {
        self.install_source_policy = policy;
    }

    /// Returns the current application installation source policy.
    pub fn get_install_source_policy(&self) -> AppInstallSourcePolicy {
        self.install_source_policy
    }

    /// Toggles the "Share across devices" feature.
    pub fn set_share_across_devices(&mut self, enabled: bool) {
        self.share_across_devices = enabled;
    }

    /// Returns whether "Share across devices" is enabled.
    pub fn is_share_across_devices_enabled(&self) -> bool {
        self.share_across_devices
    }

    /// Sets the scope for device sharing.
    pub fn set_share_mode(&mut self, mode: ShareAcrossDevicesMode) {
        self.share_mode = mode;
    }

    /// Returns the current scope for device sharing.
    pub fn get_share_mode(&self) -> ShareAcrossDevicesMode {
        self.share_mode
    }

    /// Returns all command-line application execution aliases.
    pub fn get_execution_aliases(&self) -> &[AppExecutionAlias] {
        &self.aliases
    }

    /// Toggles the enabled status of a command-line execution alias.
    pub fn toggle_execution_alias(&mut self, alias_name: &str) -> Result<bool, AppSettingsError> {
        let alias_lower = alias_name.trim().to_lowercase();
        let alias = self
            .aliases
            .iter_mut()
            .find(|a| a.alias.eq_ignore_ascii_case(&alias_lower))
            .ok_or_else(|| AppSettingsError::AliasNotFound(alias_name.to_string()))?;

        alias.enabled = !alias.enabled;
        let new_state = alias.enabled;
        let msg = format!(
            "Execution alias '{}' for {} is now {}.",
            alias.alias,
            alias.app_name,
            if new_state { "enabled" } else { "disabled" }
        );
        self.last_status = Some(msg);
        Ok(new_state)
    }

    /// Resolves an alias command into its target binary path if enabled.
    pub fn resolve_execution_alias(&self, command: &str) -> Option<&AppExecutionAlias> {
        let cmd_lower = command.trim().to_lowercase();
        self.aliases
            .iter()
            .find(|a| a.enabled && a.alias.eq_ignore_ascii_case(&cmd_lower))
    }

    /// Adds a new application execution alias.
    pub fn add_execution_alias(&mut self, alias: AppExecutionAlias) -> Result<(), AppSettingsError> {
        if self
            .aliases
            .iter()
            .any(|a| a.alias.eq_ignore_ascii_case(&alias.alias))
        {
            return Err(AppSettingsError::AlreadyExists(alias.alias));
        }
        self.aliases.push(alias);
        Ok(())
    }

    // ========================================================================
    // 3. Default Apps & Protocol Associations
    // ========================================================================

    /// Queries the default application configured for a file extension.
    pub fn get_default_for_extension(&self, ext: &str) -> Option<&str> {
        self.defaults.get_default_extension(ext)
    }

    /// Sets the default application for a file extension.
    pub fn set_default_for_extension(
        &mut self,
        ext: &str,
        app_name: &str,
    ) -> Result<(), AppSettingsError> {
        self.defaults.set_default_extension(ext, app_name)
    }

    /// Queries candidate applications capable of opening a file extension.
    pub fn get_candidates_for_extension(&self, ext: &str) -> &[String] {
        self.defaults.get_extension_candidates(ext)
    }

    /// Queries the default application configured for a URI protocol.
    pub fn get_default_for_protocol(&self, proto: &str) -> Option<&str> {
        self.defaults.get_default_protocol(proto)
    }

    /// Sets the default application for a URI protocol.
    pub fn set_default_for_protocol(
        &mut self,
        proto: &str,
        app_name: &str,
    ) -> Result<(), AppSettingsError> {
        self.defaults.set_default_protocol(proto, app_name)
    }

    /// Queries candidate applications capable of handling a URI protocol.
    pub fn get_candidates_for_protocol(&self, proto: &str) -> &[String] {
        self.defaults.get_protocol_candidates(proto)
    }

    /// One-click batch sets all associated file extensions and protocols for an application.
    pub fn set_defaults_by_application(&mut self, app_name: &str) -> Result<usize, AppSettingsError> {
        let app = self
            .installed_apps
            .values()
            .find(|a| a.name.eq_ignore_ascii_case(app_name))
            .ok_or_else(|| AppSettingsError::AppNotFound(app_name.to_string()))?;

        let exts = app.supported_extensions.clone();
        let protos = app.supported_protocols.clone();

        let count = self.defaults.set_defaults_by_app(&app.name, &exts, &protos);
        let msg = format!(
            "Set {} as default for {} file types and link protocols.",
            app.name, count
        );
        self.last_status = Some(msg);
        Ok(count)
    }

    /// Restores Microsoft recommended defaults for file types and protocols.
    pub fn reset_defaults_to_recommended(&mut self) {
        self.defaults.reset_to_recommended();
        self.last_status = Some("Reset all default applications to recommended defaults.".to_string());
    }

    /// Returns all registered file extensions and their current default handlers.
    pub fn get_all_extension_mappings(&self) -> Vec<(String, String)> {
        self.defaults.all_extensions()
    }

    /// Returns all registered protocols and their current default handlers.
    pub fn get_all_protocol_mappings(&self) -> Vec<(String, String)> {
        self.defaults.all_protocols()
    }

    // ========================================================================
    // 4. Startup Applications & Registry Synchronization
    // ========================================================================

    /// Returns the complete inventory of startup applications.
    pub fn get_startup_apps(&self) -> &[StartupApp] {
        &self.startup_apps
    }

    /// Toggles the startup enabled state of an application with automatic Registry Run key synchronization.
    pub fn toggle_startup_app(&mut self, id: &str) -> Result<bool, AppSettingsError> {
        let item = self
            .startup_apps
            .iter_mut()
            .find(|s| s.id == id)
            .ok_or_else(|| AppSettingsError::AppNotFound(id.to_string()))?;

        item.enabled = !item.enabled;
        let new_state = item.enabled;
        let app_name = item.name.clone();
        let command = item.command.clone();

        // Automatic synchronization with HKCU\Software\Microsoft\Windows\CurrentVersion\Run
        if new_state {
            self.registry_run_keys.set_hkcu_run(&app_name, &command);
        } else {
            self.registry_run_keys.remove_hkcu_run(&app_name);
        }

        let msg = format!(
            "Startup item '{}' is now {} (Registry key synced: {}).",
            app_name,
            if new_state { "Enabled" } else { "Disabled" },
            REG_KEY_HKCU_RUN
        );
        self.last_status = Some(msg);
        Ok(new_state)
    }

    /// Sets the startup status explicitly with automatic Registry Run key sync.
    pub fn set_startup_status(&mut self, id: &str, enabled: bool) -> Result<(), AppSettingsError> {
        let item = self
            .startup_apps
            .iter_mut()
            .find(|s| s.id == id)
            .ok_or_else(|| AppSettingsError::AppNotFound(id.to_string()))?;

        if item.enabled != enabled {
            item.enabled = enabled;
            let app_name = item.name.clone();
            let command = item.command.clone();

            if enabled {
                self.registry_run_keys.set_hkcu_run(&app_name, &command);
            } else {
                self.registry_run_keys.remove_hkcu_run(&app_name);
            }
        }

        Ok(())
    }

    /// Adds a new application to the startup inventory and synchronizes registry if enabled.
    pub fn add_startup_app(&mut self, app: StartupApp) -> Result<(), AppSettingsError> {
        if self.startup_apps.iter().any(|s| s.id == app.id) {
            return Err(AppSettingsError::AlreadyExists(app.id));
        }

        if app.enabled {
            self.registry_run_keys.set_hkcu_run(&app.name, &app.command);
        }
        self.startup_apps.push(app);
        Ok(())
    }

    /// Removes an application from the startup inventory and purges its registry entry.
    pub fn remove_startup_app(&mut self, id: &str) -> Result<(), AppSettingsError> {
        let index = self
            .startup_apps
            .iter()
            .position(|s| s.id == id)
            .ok_or_else(|| AppSettingsError::AppNotFound(id.to_string()))?;

        let item = self.startup_apps.remove(index);
        self.registry_run_keys.remove_hkcu_run(&item.name);
        Ok(())
    }

    /// Queries the registry value under `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`.
    pub fn get_registry_run_value(&self, key: &str) -> Option<&str> {
        self.registry_run_keys.get_hkcu_run(key)
    }

    /// Returns the complete map of synchronized HKCU Run entries.
    pub fn get_all_registry_run_keys(&self) -> &BTreeMap<String, String> {
        self.registry_run_keys.all_hkcu()
    }

    // ========================================================================
    // 5. Navigation & UI Rendering Representation
    // ========================================================================

    /// Navigates to a specific subpage in the Apps settings category.
    pub fn navigate_to(&mut self, subpage: AppsCategorySubpage) {
        self.active_subpage = subpage;
    }

    /// Returns the currently active subpage.
    pub fn current_subpage(&self) -> AppsCategorySubpage {
        self.active_subpage
    }

    /// Returns the current modal state.
    pub fn uninstall_modal(&self) -> &UninstallModal {
        &self.uninstall_modal
    }

    /// Renders a structured, clean text/ASCII Fluent representation of the active page.
    pub fn render_ui_view(&self) -> String {
        let mut out = String::new();

        out.push_str("╔════════════════════════════════════════════════════════════════════════════╗\n");
        out.push_str(&format!(
            "║  Windows 11 Settings > Apps > {:<44} ║\n",
            self.active_subpage.title()
        ));
        out.push_str("╠════════════════════════════════════════════════════════════════════════════╣\n");

        match self.active_subpage {
            AppsCategorySubpage::InstalledApps => {
                let filtered = self.get_filtered_apps();
                out.push_str(&format!(
                    "║  Search: [{:<20}]  Sort: {:<12}  Count: {:<3} apps   ║\n",
                    if self.search_query.is_empty() { "Search apps..." } else { &self.search_query },
                    format!("{:?}", self.sort_order),
                    filtered.len()
                ));
                out.push_str("╟────────────────────────────────────────────────────────────────────────────╢\n");

                for app in filtered.iter().take(6) {
                    out.push_str(&format!(
                        "║  • {:<28} {:<12} {:>8.1} MB  [{:<14}]  ║\n",
                        app.name,
                        app.version,
                        app.total_size_mb(),
                        app.app_type.display_name()
                    ));
                    out.push_str(&format!(
                        "║    Pub: {:<20} Path: {:<35} ║\n",
                        app.publisher,
                        if app.install_path.len() > 35 { &app.install_path[..35] } else { &app.install_path }
                    ));
                    out.push_str(&format!(
                        "║    Actions: [Advanced options]  {}  {} ║\n",
                        if app.can_modify { "[Modify]" } else { "        " },
                        if app.can_uninstall { "[Uninstall]" } else { "           " }
                    ));
                    out.push_str("╟┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄╢\n");
                }

                if self.uninstall_modal.is_open {
                    out.push_str("║  ┌──────────────────────────────────────────────────────────────────────┐  ║\n");
                    out.push_str(&format!(
                        "║  │  UNINSTALL CONFIRMATION: {:<40} │  ║\n",
                        self.uninstall_modal.target_app_name.as_deref().unwrap_or("")
                    ));
                    out.push_str("║  │  This app and its related info will be uninstalled.                  │  ║\n");
                    out.push_str("║  │                                                                      │  ║\n");
                    out.push_str("║  │           [ Confirm Uninstall ]         [ Cancel ]                   │  ║\n");
                    out.push_str("║  └──────────────────────────────────────────────────────────────────────┘  ║\n");
                }
            }
            AppsCategorySubpage::AdvancedAppSettings => {
                out.push_str("║  CHOOSE WHERE TO GET APPS:                                                 ║\n");
                out.push_str(&format!(
                    "║  Current Policy: {:<58} ║\n",
                    self.install_source_policy.description()
                ));
                out.push_str("╟────────────────────────────────────────────────────────────────────────────╢\n");
                out.push_str(&format!(
                    "║  Share across devices: [{:<3}]  (Mode: {:<20})              ║\n",
                    if self.share_across_devices { "ON" } else { "OFF" },
                    format!("{:?}", self.share_mode)
                ));
                out.push_str("╟────────────────────────────────────────────────────────────────────────────╢\n");
                out.push_str("║  APP EXECUTION ALIASES:                                                    ║\n");
                for alias in &self.aliases {
                    out.push_str(&format!(
                        "║  • {:<12} -> {:<22} [{:<3}]                             ║\n",
                        alias.alias,
                        alias.app_name,
                        if alias.enabled { "ON" } else { "OFF" }
                    ));
                }
            }
            AppsCategorySubpage::DefaultApps => {
                out.push_str("║  FILE EXTENSION ASSOCIATIONS:                                              ║\n");
                let exts = self.defaults.all_extensions();
                for (ext, app) in exts.iter() {
                    out.push_str(&format!(
                        "║  • {:<8} -> {:<30} [Change default]  ║\n",
                        ext, app
                    ));
                }
                out.push_str("╟────────────────────────────────────────────────────────────────────────────╢\n");
                out.push_str("║  PROTOCOL HANDLERS:                                                        ║\n");
                let protos = self.defaults.all_protocols();
                for (proto, app) in protos.iter() {
                    out.push_str(&format!(
                        "║  • {:<12} -> {:<26} [Change default]  ║\n",
                        proto, app
                    ));
                }
            }
            AppsCategorySubpage::StartupApps => {
                out.push_str("║  STARTUP APPLICATIONS (Synced with HKCU Run key):                          ║\n");
                for app in &self.startup_apps {
                    out.push_str(&format!(
                        "║  • {:<28} [{:<3}]  Impact: {:<12}                ║\n",
                        app.name,
                        if app.enabled { "ON" } else { "OFF" },
                        app.impact.label()
                    ));
                }
            }
            AppsCategorySubpage::OptionalFeatures => {
                out.push_str("║  OPTIONAL FEATURES & CAPABILITIES:                                         ║\n");
                out.push_str("║  • OpenSSH Client, Windows Media Player Legacy, WSL2 Kernel.              ║\n");
            }
            AppsCategorySubpage::VideoPlayback => {
                out.push_str("║  VIDEO PLAYBACK ENHANCEMENTS:                                              ║\n");
                out.push_str("║  • Automatically process video to enhance it: [ON]                         ║\n");
            }
        }

        out.push_str("╚════════════════════════════════════════════════════════════════════════════╝\n");
        out
    }
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_search_and_filters() {
        let mut manager = AppsSettingsManager::new();

        // 1. Initial full catalog count
        let all_apps = manager.get_filtered_apps();
        assert!(all_apps.len() >= 10, "Should have rich catalog of default applications");

        // 2. Search by substring "edge"
        manager.set_search_query("edge");
        let results = manager.get_filtered_apps();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Microsoft Edge");

        // 3. Search case-insensitive "cOdE"
        manager.set_search_query("cOdE");
        let results = manager.get_filtered_apps();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Visual Studio Code");

        // 4. Reset search query
        manager.set_search_query("");

        // 5. Filter by Drive "D:"
        manager.set_drive_filter(DriveFilter::SpecificDrive("D:".to_string()));
        let d_apps = manager.get_filtered_apps();
        assert!(!d_apps.is_empty());
        for app in d_apps {
            assert_eq!(app.drive, "D:");
        }

        // 6. Filter by AppType::LinuxElf
        manager.set_drive_filter(DriveFilter::AllDrives);
        manager.set_type_filter(AppTypeFilter::Only(AppType::LinuxElf));
        let elf_apps = manager.get_filtered_apps();
        assert!(elf_apps.len() >= 2); // Linux Subsystem and Ubuntu 24.04
        for app in elf_apps {
            assert_eq!(app.app_type, AppType::LinuxElf);
        }
    }

    #[test]
    fn test_app_sorting() {
        let mut manager = AppsSettingsManager::new();

        // 1. Sort Name Ascending
        manager.set_sort_order(AppSortOrder::NameAsc);
        let names_asc = manager.get_filtered_apps();
        for i in 0..names_asc.len() - 1 {
            assert!(names_asc[i].name.to_lowercase() <= names_asc[i + 1].name.to_lowercase());
        }

        // 2. Sort Name Descending
        manager.set_sort_order(AppSortOrder::NameDesc);
        let names_desc = manager.get_filtered_apps();
        for i in 0..names_desc.len() - 1 {
            assert!(names_desc[i].name.to_lowercase() >= names_desc[i + 1].name.to_lowercase());
        }

        // 3. Sort Size Descending (largest first)
        manager.set_sort_order(AppSortOrder::SizeDesc);
        let sizes_desc = manager.get_filtered_apps();
        for i in 0..sizes_desc.len() - 1 {
            assert!(sizes_desc[i].total_size_mb() >= sizes_desc[i + 1].total_size_mb());
        }

        // 4. Sort Size Ascending (smallest first)
        manager.set_sort_order(AppSortOrder::SizeAsc);
        let sizes_asc = manager.get_filtered_apps();
        for i in 0..sizes_asc.len() - 1 {
            assert!(sizes_asc[i].total_size_mb() <= sizes_asc[i + 1].total_size_mb());
        }
    }

    #[test]
    fn test_uninstall_flow_and_modal() {
        let mut manager = AppsSettingsManager::new();

        // 1. Attempt to uninstall an essential system app -> blocked
        let err = manager.open_uninstall_modal("Microsoft.Edge");
        assert!(err.is_err());
        match err {
            Err(AppSettingsError::UninstallBlocked(msg)) => {
                assert!(msg.contains("essential HybridOS component"));
            }
            _ => panic!("Expected UninstallBlocked error"),
        }

        // 2. Open uninstall modal for Calculator
        let calc_id = "Microsoft.WindowsCalculator";
        let open_res = manager.open_uninstall_modal(calc_id);
        assert!(open_res.is_ok());
        assert!(manager.uninstall_modal().is_open);
        assert_eq!(
            manager.uninstall_modal().target_app_id.as_deref(),
            Some(calc_id)
        );

        // 3. Cancel modal
        manager.cancel_uninstall_modal();
        assert!(!manager.uninstall_modal().is_open);
        assert!(manager.get_app(calc_id).is_some(), "App should still exist after cancel");

        // 4. Open modal again and confirm uninstallation
        manager.open_uninstall_modal(calc_id).unwrap();
        let confirm_res = manager.confirm_uninstall();
        assert!(confirm_res.is_ok());
        assert!(!manager.uninstall_modal().is_open);
        assert!(manager.get_app(calc_id).is_none(), "App must be deleted from registry");
    }

    #[test]
    fn test_file_extension_associations() {
        let mut manager = AppsSettingsManager::new();

        // 1. Initial mappings
        assert_eq!(manager.get_default_for_extension(".txt"), Some("Notepad"));
        assert_eq!(manager.get_default_for_extension(".pdf"), Some("Microsoft Edge"));
        assert_eq!(manager.get_default_for_extension(".elf"), Some("Linux Subsystem"));
        assert_eq!(manager.get_default_for_extension(".exe"), Some("Windows Subsystem"));

        // 2. Candidate inspection
        let candidates = manager.get_candidates_for_extension(".txt");
        assert!(candidates.contains(&"Notepad".to_string()));
        assert!(candidates.contains(&"Visual Studio Code".to_string()));

        // 3. Reassign default application for .txt to VS Code
        let set_res = manager.set_default_for_extension(".txt", "Visual Studio Code");
        assert!(set_res.is_ok());
        assert_eq!(
            manager.get_default_for_extension(".txt"),
            Some("Visual Studio Code")
        );

        // 4. Invalid extension lookup
        let unknown = manager.set_default_for_extension(".xyz123", "SomeApp");
        assert!(matches!(unknown, Err(AppSettingsError::ExtensionNotFound(_))));
    }

    #[test]
    fn test_protocol_mappings() {
        let mut manager = AppsSettingsManager::new();

        // 1. Initial protocol defaults
        assert_eq!(manager.get_default_for_protocol("http"), Some("Microsoft Edge"));
        assert_eq!(manager.get_default_for_protocol("https"), Some("Microsoft Edge"));
        assert_eq!(manager.get_default_for_protocol("mailto"), Some("Mail and Calendar"));
        assert_eq!(manager.get_default_for_protocol("ms-settings"), Some("Settings App"));

        // 2. Reassign protocol
        let set_res = manager.set_default_for_protocol("mailto", "Microsoft Edge");
        assert!(set_res.is_ok());
        assert_eq!(manager.get_default_for_protocol("mailto"), Some("Microsoft Edge"));
    }

    #[test]
    fn test_set_defaults_by_application() {
        let mut manager = AppsSettingsManager::new();

        // Reassign all Microsoft Edge supported extensions and protocols in one operation
        let count = manager.set_defaults_by_application("Microsoft Edge").unwrap();
        assert!(count >= 3);

        assert_eq!(manager.get_default_for_extension(".html"), Some("Microsoft Edge"));
        assert_eq!(manager.get_default_for_extension(".pdf"), Some("Microsoft Edge"));
        assert_eq!(manager.get_default_for_protocol("http"), Some("Microsoft Edge"));

        // Change .txt to VS Code, then reset to recommended defaults
        manager.set_default_for_extension(".txt", "Visual Studio Code").unwrap();
        assert_eq!(manager.get_default_for_extension(".txt"), Some("Visual Studio Code"));

        manager.reset_defaults_to_recommended();
        assert_eq!(manager.get_default_for_extension(".txt"), Some("Notepad"));
        assert_eq!(manager.get_default_for_protocol("http"), Some("Microsoft Edge"));
    }

    #[test]
    fn test_startup_app_toggle_and_registry_sync() {
        let mut manager = AppsSettingsManager::new();

        // 1. Find an initially disabled item: "startup_edge"
        let edge_startup = manager
            .get_startup_apps()
            .iter()
            .find(|s| s.id == "startup_edge")
            .unwrap();
        assert!(!edge_startup.enabled);

        // Registry should NOT have an entry for Edge initially
        assert_eq!(
            manager.get_registry_run_value("Microsoft Edge Auto-Launcher"),
            None
        );

        // 2. Toggle to Enabled
        let toggle_res = manager.toggle_startup_app("startup_edge").unwrap();
        assert!(toggle_res); // Now true

        // Registry Run key MUST be automatically synchronized!
        let reg_val = manager.get_registry_run_value("Microsoft Edge Auto-Launcher");
        assert!(reg_val.is_some());
        assert!(reg_val.unwrap().contains("msedge.exe"));

        // 3. Toggle back to Disabled
        let toggle_back = manager.toggle_startup_app("startup_edge").unwrap();
        assert!(!toggle_back); // Now false

        // Registry Run key MUST be deleted!
        assert_eq!(
            manager.get_registry_run_value("Microsoft Edge Auto-Launcher"),
            None
        );
    }

    #[test]
    fn test_app_execution_aliases() {
        let mut manager = AppsSettingsManager::new();

        // 1. Initial alias lookup
        let wt = manager.resolve_execution_alias("wt.exe");
        assert!(wt.is_some());
        assert_eq!(wt.unwrap().app_name, "Windows Terminal");

        // 2. Disable alias
        let toggled = manager.toggle_execution_alias("wt.exe").unwrap();
        assert!(!toggled);

        // Alias resolution should now fail because it is disabled
        assert!(manager.resolve_execution_alias("wt.exe").is_none());

        // 3. Re-enable alias
        let re_enabled = manager.toggle_execution_alias("wt.exe").unwrap();
        assert!(re_enabled);
        assert!(manager.resolve_execution_alias("wt.exe").is_some());
    }

    #[test]
    fn test_advanced_options_permissions_and_reset() {
        let mut manager = AppsSettingsManager::new();

        let app_id = "Microsoft.WindowsPhotos";
        let app = manager.get_app(app_id).unwrap();
        assert_eq!(app.permissions.get(&AppPermission::Camera), Some(&false));
        assert!(app.cache_size_mb > 0.0);

        // 1. Toggle camera permission
        manager
            .set_app_permission(app_id, AppPermission::Camera, true)
            .unwrap();
        let updated = manager.get_app(app_id).unwrap();
        assert_eq!(updated.permissions.get(&AppPermission::Camera), Some(&true));

        // 2. Reset app data & cache
        let reset_res = manager.reset_app(app_id);
        assert!(reset_res.is_ok());
        let reset_app = manager.get_app(app_id).unwrap();
        assert_eq!(reset_app.data_size_mb, 0.0);
        assert_eq!(reset_app.cache_size_mb, 0.0);
        assert!(reset_app.size_mb > 0.0); // Core binary intact
    }

    #[test]
    fn test_install_source_policy_enforcement() {
        let mut manager = AppsSettingsManager::new();

        // Configure policy to Microsoft Store Only
        manager.set_install_source_policy(AppInstallSourcePolicy::StoreOnly);

        let third_party_win32 = InstalledApp {
            id: "Custom.Win32App".to_string(),
            name: "Untrusted Win32 App".to_string(),
            publisher: "Unknown".to_string(),
            version: "1.0.0".to_string(),
            install_date: "2026-11-10".to_string(),
            size_mb: 50.0,
            install_path: r"C:\Temp\app.exe".to_string(),
            app_type: AppType::Win32Desktop,
            drive: "C:".to_string(),
            permissions: HashMap::new(),
            is_running: false,
            can_modify: false,
            can_uninstall: true,
            supported_extensions: vec![],
            supported_protocols: vec![],
            data_size_mb: 0.0,
            cache_size_mb: 0.0,
        };

        // Installation must be blocked by StoreOnly policy!
        let install_err = manager.install_app(third_party_win32.clone());
        assert!(matches!(install_err, Err(AppSettingsError::PolicyBlocked(_))));

        // Change policy to Anywhere
        manager.set_install_source_policy(AppInstallSourcePolicy::Anywhere);
        let install_ok = manager.install_app(third_party_win32);
        assert!(install_ok.is_ok());
        assert!(manager.get_app("Custom.Win32App").is_some());
    }

    #[test]
    fn test_ui_render_view() {
        let mut manager = AppsSettingsManager::new();

        // Installed apps page render
        manager.navigate_to(AppsCategorySubpage::InstalledApps);
        let view1 = manager.render_ui_view();
        assert!(view1.contains("Installed apps"));
        assert!(view1.contains("Microsoft Edge"));

        // Startup apps page render
        manager.navigate_to(AppsCategorySubpage::StartupApps);
        let view2 = manager.render_ui_view();
        assert!(view2.contains("STARTUP APPLICATIONS"));
        assert!(view2.contains("Synced with HKCU Run key"));

        // Default apps page render
        manager.navigate_to(AppsCategorySubpage::DefaultApps);
        let view3 = manager.render_ui_view();
        assert!(view3.contains("FILE EXTENSION ASSOCIATIONS"));
        assert!(view3.contains(".txt"));
    }
}
