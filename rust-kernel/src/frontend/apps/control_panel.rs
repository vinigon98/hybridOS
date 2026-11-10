//! Windows Classic Control Panel & Administrative Tools Suite for HybridOS
//!
//! A complete, production-grade, commercial Windows Classic Control Panel
//! and Administrative Tools implementation in 100% pure, safe, and robust Rust.
//!
//! # Architecture & Capabilities
//!
//! - **Classic Control Panel Core**:
//!   - View Modes: Category View (8 canonical categories with primary & secondary tasks)
//!     vs Large Icons (48x48) vs Small Icons (16x16 / compact list).
//!   - 40+ Classic Applets cataloged (.cpl, .msc, and control executables) with canonical
//!     names, localized descriptions, icons, categories, and keyword indexing.
//!   - Real-Time Search Engine: Instant filtering across titles, descriptions, categories,
//!     and keywords with match ranking.
//!   - Breadcrumb Navigation & History: Full Back/Forward/Up stack with breadcrumb segment
//!     generation.
//!
//! - **Administrative Tools Suite**:
//!   - **Event Viewer (`eventvwr.msc`)**: Standard Windows Logs (`Application`, `Security`,
//!     `Setup`, `System`, and custom channels). Multi-level severity (`Critical`, `Error`,
//!     `Warning`, `Information`, `Verbose`), timestamping, Event ID, Task Category, XML export,
//!     rich filtering, and log clearing.
//!   - **Services (`services.msc`)**: Complete Service Control Manager (SCM) emulation.
//!     Full lifecycle state machine (`Running`, `Stopped`, `Paused`, transitions), startup types
//!     (`Automatic`, `AutomaticDelayed`, `Manual`, `Disabled`), service accounts, dependency tree
//!     validation, cascading startup/shutdown, and tabbed Properties model.
//!   - **Disk Management (`diskmgmt.msc`)**: High-fidelity graphical partition layout (EFI System,
//!     OS C:, Data D:, NVMe, SATA), Volume operations (Format, Assign/Remove drive letter,
//!     Shrink, Extend, Partition creation/deletion), and ASCII graphical visualizer.
//!   - **Computer Management (`compmgmt.msc`)**: Master MMC console integrating System Tools,
//!     Storage (Disk Management), Services and Applications, Shared Folders, Local Users & Groups,
//!     and Performance Monitor.
//!
//! 100% Pure Safe Rust with zero unsafe code blocks.

#![allow(dead_code)]

use std::collections::{BTreeMap, HashSet};
use std::fmt;

// ============================================================================
// Section 1: Classic Control Panel Enums & Primitives
// ============================================================================

/// View modes available in the Windows Classic Control Panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ControlPanelViewMode {
    /// Categorized view grouping applets into 8 primary task hubs
    #[default]
    Category,
    /// Detailed large icon grid (48x48 icon layout)
    LargeIcons,
    /// Dense small icon list / grid (16x16 icon layout)
    SmallIcons,
}

impl fmt::Display for ControlPanelViewMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Category => write!(f, "Category"),
            Self::LargeIcons => write!(f, "Large icons"),
            Self::SmallIcons => write!(f, "Small icons"),
        }
    }
}

/// The 8 canonical Windows Control Panel Categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ControlPanelCategory {
    SystemAndSecurity,
    NetworkAndInternet,
    HardwareAndSound,
    Programs,
    UserAccounts,
    AppearanceAndPersonalization,
    ClockAndRegion,
    EaseOfAccess,
}

impl ControlPanelCategory {
    /// Returns the human-readable display name of the category.
    pub const fn display_name(&self) -> &'static str {
        match self {
            Self::SystemAndSecurity => "System and Security",
            Self::NetworkAndInternet => "Network and Internet",
            Self::HardwareAndSound => "Hardware and Sound",
            Self::Programs => "Programs",
            Self::UserAccounts => "User Accounts",
            Self::AppearanceAndPersonalization => "Appearance and Personalization",
            Self::ClockAndRegion => "Clock and Region",
            Self::EaseOfAccess => "Ease of Access",
        }
    }

    /// Returns the descriptive subtitle shown in Category view.
    pub const fn description(&self) -> &'static str {
        match self {
            Self::SystemAndSecurity => {
                "Review your computer's status, save backup copies of files, and check firewall protection."
            }
            Self::NetworkAndInternet => {
                "View network status and tasks, set up connections, and configure Internet options."
            }
            Self::HardwareAndSound => {
                "View devices and printers, configure audio devices, and manage power options."
            }
            Self::Programs => {
                "Uninstall programs, turn Windows features on or off, and set default applications."
            }
            Self::UserAccounts => {
                "Change account settings, manage web credentials, and configure mail profiles."
            }
            Self::AppearanceAndPersonalization => {
                "Change taskbar behavior, manage fonts, and customize File Explorer folder options."
            }
            Self::ClockAndRegion => {
                "Set date, time, and time zone, and change date, time, or number formats."
            }
            Self::EaseOfAccess => {
                "Let Windows suggest settings, optimize visual display, and configure speech recognition."
            }
        }
    }

    /// Returns the icon glyph/emoji representing this category.
    pub const fn icon_glyph(&self) -> &'static str {
        match self {
            Self::SystemAndSecurity => "🛡️",
            Self::NetworkAndInternet => "🌐",
            Self::HardwareAndSound => "🔊",
            Self::Programs => "📦",
            Self::UserAccounts => "👤",
            Self::AppearanceAndPersonalization => "🎨",
            Self::ClockAndRegion => "🕒",
            Self::EaseOfAccess => "♿",
        }
    }

    /// Returns an array of all categories.
    pub const fn all() -> &'static [ControlPanelCategory] {
        &[
            ControlPanelCategory::SystemAndSecurity,
            ControlPanelCategory::NetworkAndInternet,
            ControlPanelCategory::HardwareAndSound,
            ControlPanelCategory::Programs,
            ControlPanelCategory::UserAccounts,
            ControlPanelCategory::AppearanceAndPersonalization,
            ControlPanelCategory::ClockAndRegion,
            ControlPanelCategory::EaseOfAccess,
        ]
    }
}

/// A primary or secondary task link listed under a category in Category view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CategoryTaskLink {
    /// User-facing action title (e.g. "Review your computer's status")
    pub title: String,
    /// Associated applet ID to launch when clicked
    pub applet_id: String,
    /// Whether this is a primary highlight link (displayed prominently in green/blue)
    pub is_primary: bool,
}

impl CategoryTaskLink {
    pub fn new(title: impl Into<String>, applet_id: impl Into<String>, is_primary: bool) -> Self {
        Self {
            title: title.into(),
            applet_id: applet_id.into(),
            is_primary,
        }
    }
}

/// Category metadata containing task links.
#[derive(Debug, Clone)]
pub struct CategoryMetadata {
    pub category: ControlPanelCategory,
    pub primary_tasks: Vec<CategoryTaskLink>,
    pub sub_tasks: Vec<CategoryTaskLink>,
}

// ============================================================================
// Section 2: Classic Applet Definition & Registry (40+ Applets)
// ============================================================================

/// Metadata for a single Control Panel Applet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlPanelApplet {
    /// Unique identifier (usually the .cpl or .msc filename or canonical token)
    pub id: String,
    /// Localized display name (e.g. "Windows Defender Firewall")
    pub display_name: String,
    /// Microsoft canonical name (e.g. "Microsoft.WindowsFirewall")
    pub canonical_name: String,
    /// Executable command string (e.g. "firewall.cpl" or "mmc.exe devmgmt.msc")
    pub command: String,
    /// Rich description of functionality
    pub description: String,
    /// Parent category
    pub category: ControlPanelCategory,
    /// Visual icon glyph / symbol
    pub icon_glyph: String,
    /// Search keywords for fast discovery
    pub keywords: Vec<String>,
    /// Whether this applet belongs to Administrative Tools
    pub is_admin_tool: bool,
    /// Whether launching requires elevated administrator privileges
    pub requires_elevation: bool,
}

impl ControlPanelApplet {
    pub fn new(
        id: &str,
        display_name: &str,
        canonical_name: &str,
        command: &str,
        description: &str,
        category: ControlPanelCategory,
        icon_glyph: &str,
        keywords: &[&str],
        is_admin_tool: bool,
        requires_elevation: bool,
    ) -> Self {
        Self {
            id: id.to_string(),
            display_name: display_name.to_string(),
            canonical_name: canonical_name.to_string(),
            command: command.to_string(),
            description: description.to_string(),
            category,
            icon_glyph: icon_glyph.to_string(),
            keywords: keywords.iter().map(|&k| k.to_string()).collect(),
            is_admin_tool,
            requires_elevation,
        }
    }

    /// Checks if this applet matches a search query string.
    pub fn matches_query(&self, query: &str) -> bool {
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return true;
        }

        if self.display_name.to_lowercase().contains(&q) {
            return true;
        }
        if self.canonical_name.to_lowercase().contains(&q) {
            return true;
        }
        if self.command.to_lowercase().contains(&q) {
            return true;
        }
        if self.description.to_lowercase().contains(&q) {
            return true;
        }
        if self.category.display_name().to_lowercase().contains(&q) {
            return true;
        }
        for kw in &self.keywords {
            if kw.to_lowercase().contains(&q) {
                return true;
            }
        }
        false
    }
}

/// Build the full catalog of 40+ authentic Windows Classic Control Panel applets.
pub fn build_default_applet_catalog() -> Vec<ControlPanelApplet> {
    vec![
        // --- System and Security ---
        ControlPanelApplet::new(
            "wscui.cpl",
            "Security and Maintenance",
            "Microsoft.ActionCenter",
            "wscui.cpl",
            "Review recent security messages and resolve computer issues.",
            ControlPanelCategory::SystemAndSecurity,
            "🛡️",
            &["security", "maintenance", "action center", "antivirus", "health"],
            false,
            false,
        ),
        ControlPanelApplet::new(
            "firewall.cpl",
            "Windows Defender Firewall",
            "Microsoft.WindowsFirewall",
            "firewall.cpl",
            "Protect your computer against unauthorized access over networks or the Internet.",
            ControlPanelCategory::SystemAndSecurity,
            "🧱",
            &["firewall", "security", "network", "ports", "defender", "inbound", "outbound"],
            false,
            true,
        ),
        ControlPanelApplet::new(
            "sysdm.cpl",
            "System",
            "Microsoft.System",
            "sysdm.cpl",
            "View computer specs, processor speed, RAM, device specs, and system properties.",
            ControlPanelCategory::SystemAndSecurity,
            "💻",
            &["system", "specs", "ram", "cpu", "processor", "hardware", "computer name"],
            false,
            false,
        ),
        ControlPanelApplet::new(
            "powercfg.cpl",
            "Power Options",
            "Microsoft.PowerOptions",
            "powercfg.cpl",
            "Choose or customize power plans, display turn-off times, and sleep settings.",
            ControlPanelCategory::SystemAndSecurity,
            "🔋",
            &["power", "battery", "sleep", "screen", "energy", "performance", "hibernate"],
            false,
            false,
        ),
        ControlPanelApplet::new(
            "fhdr.cpl",
            "File History",
            "Microsoft.FileHistory",
            "control.exe /name Microsoft.FileHistory",
            "Save backup copies of your files to an external drive or network location.",
            ControlPanelCategory::SystemAndSecurity,
            "📁",
            &["backup", "restore", "file history", "versions", "external drive"],
            false,
            false,
        ),
        ControlPanelApplet::new(
            "sdclt.exe",
            "Backup and Restore (Windows 7)",
            "Microsoft.BackupAndRestore",
            "sdclt.exe",
            "Create system image backups and restore files from previous backups.",
            ControlPanelCategory::SystemAndSecurity,
            "💾",
            &["backup", "system image", "restore", "recovery disk"],
            false,
            true,
        ),
        ControlPanelApplet::new(
            "bitlocker.cpl",
            "BitLocker Drive Encryption",
            "Microsoft.BitLockerDriveEncryption",
            "control.exe /name Microsoft.BitLockerDriveEncryption",
            "Protect your files and folders by encrypting your entire drive with BitLocker.",
            ControlPanelCategory::SystemAndSecurity,
            "🔐",
            &["bitlocker", "encrypt", "tpm", "security", "drive encryption", "password"],
            false,
            true,
        ),
        ControlPanelApplet::new(
            "spaces.cpl",
            "Storage Spaces",
            "Microsoft.StorageSpaces",
            "control.exe /name Microsoft.StorageSpaces",
            "Group drives together into a storage pool to protect data against drive failures.",
            ControlPanelCategory::SystemAndSecurity,
            "🗄️",
            &["storage spaces", "pool", "raid", "mirror", "parity", "drives"],
            false,
            true,
        ),
        ControlPanelApplet::new(
            "workfolders.cpl",
            "Work Folders",
            "Microsoft.WorkFolders",
            "control.exe /name Microsoft.WorkFolders",
            "Access your work files and sync them to your computer.",
            ControlPanelCategory::SystemAndSecurity,
            "💼",
            &["work folders", "sync", "enterprise", "documents"],
            false,
            false,
        ),
        ControlPanelApplet::new(
            "admintools",
            "Administrative Tools",
            "Microsoft.AdministrativeTools",
            "control.exe admintools",
            "Configure administrative settings, event logs, services, and system tools.",
            ControlPanelCategory::SystemAndSecurity,
            "🛠️",
            &["admin", "tools", "eventvwr", "services", "management", "disks"],
            true,
            true,
        ),

        // --- Administrative Tools Specific Sub-Applets ---
        ControlPanelApplet::new(
            "eventvwr.msc",
            "Event Viewer",
            "Microsoft.EventViewer",
            "eventvwr.msc",
            "View diagnostic event logs, system errors, audit warnings, and operational events.",
            ControlPanelCategory::SystemAndSecurity,
            "📋",
            &["events", "logs", "eventvwr", "audit", "errors", "system log", "diagnostics"],
            true,
            true,
        ),
        ControlPanelApplet::new(
            "services.msc",
            "Services",
            "Microsoft.Services",
            "services.msc",
            "Manage background system services, startup types, and service dependencies.",
            ControlPanelCategory::SystemAndSecurity,
            "⚙️",
            &["services", "daemon", "background", "start", "stop", "automatic", "scm"],
            true,
            true,
        ),
        ControlPanelApplet::new(
            "diskmgmt.msc",
            "Disk Management",
            "Microsoft.DiskManagement",
            "diskmgmt.msc",
            "Manage hard disks, view partition layouts, format volumes, and assign drive letters.",
            ControlPanelCategory::SystemAndSecurity,
            "💿",
            &["disk", "partition", "volumes", "format", "drive letter", "nvme", "ssd", "gpt"],
            true,
            true,
        ),
        ControlPanelApplet::new(
            "compmgmt.msc",
            "Computer Management",
            "Microsoft.ComputerManagement",
            "compmgmt.msc",
            "Master management console for system tools, storage, and services.",
            ControlPanelCategory::SystemAndSecurity,
            "🖥️",
            &["management", "console", "mmc", "compmgmt", "system tools", "storage"],
            true,
            true,
        ),
        ControlPanelApplet::new(
            "taskschd.msc",
            "Task Scheduler",
            "Microsoft.TaskScheduler",
            "taskschd.msc",
            "Schedule automated tasks, programs, and maintenance jobs.",
            ControlPanelCategory::SystemAndSecurity,
            "⏱️",
            &["schedule", "tasks", "cron", "automation", "triggers"],
            true,
            true,
        ),
        ControlPanelApplet::new(
            "perfmon.msc",
            "Performance Monitor",
            "Microsoft.PerformanceMonitor",
            "perfmon.msc",
            "Analyze real-time CPU, RAM, disk, and network performance counters.",
            ControlPanelCategory::SystemAndSecurity,
            "📊",
            &["performance", "perfmon", "counters", "cpu usage", "ram monitor"],
            true,
            true,
        ),

        // --- Network and Internet ---
        ControlPanelApplet::new(
            "ncpa.cpl",
            "Network and Sharing Center",
            "Microsoft.NetworkAndSharingCenter",
            "control.exe /name Microsoft.NetworkAndSharingCenter",
            "View network status, connect to networks, and set up adapters or sharing.",
            ControlPanelCategory::NetworkAndInternet,
            "🌐",
            &["network", "wifi", "ethernet", "adapters", "sharing", "lan", "ip address"],
            false,
            false,
        ),
        ControlPanelApplet::new(
            "inetcpl.cpl",
            "Internet Options",
            "Microsoft.InternetOptions",
            "inetcpl.cpl",
            "Configure Internet browser properties, security zones, proxy, and privacy.",
            ControlPanelCategory::NetworkAndInternet,
            "🌍",
            &["internet", "proxy", "security zones", "tls", "certificates", "lan settings"],
            false,
            false,
        ),
        ControlPanelApplet::new(
            "irprops.cpl",
            "Infrared",
            "Microsoft.Infrared",
            "irprops.cpl",
            "Configure infrared transceiver settings and file transfer options.",
            ControlPanelCategory::NetworkAndInternet,
            "📡",
            &["infrared", "ir", "wireless", "transfer"],
            false,
            false,
        ),

        // --- Hardware and Sound ---
        ControlPanelApplet::new(
            "printers",
            "Devices and Printers",
            "Microsoft.DevicesAndPrinters",
            "control.exe printers",
            "View connected devices, printers, scanners, Bluetooth mice, and webcams.",
            ControlPanelCategory::HardwareAndSound,
            "🖨️",
            &["devices", "printers", "bluetooth", "scanners", "usb", "mouse", "hardware"],
            false,
            false,
        ),
        ControlPanelApplet::new(
            "devmgmt.msc",
            "Device Manager",
            "Microsoft.DeviceManager",
            "devmgmt.msc",
            "View and update hardware drivers, resolve driver conflicts, and check device status.",
            ControlPanelCategory::HardwareAndSound,
            "🔌",
            &["device manager", "drivers", "hardware", "gpu", "usb controller", "firmware"],
            true,
            true,
        ),
        ControlPanelApplet::new(
            "autoplay.cpl",
            "AutoPlay",
            "Microsoft.AutoPlay",
            "control.exe /name Microsoft.AutoPlay",
            "Choose default actions when removable USB drives, memory cards, or CDs are inserted.",
            ControlPanelCategory::HardwareAndSound,
            "▶️",
            &["autoplay", "removable", "usb", "flash drive", "media"],
            false,
            false,
        ),
        ControlPanelApplet::new(
            "mmsys.cpl",
            "Sound",
            "Microsoft.Audio",
            "mmsys.cpl",
            "Configure playback devices, microphones, audio levels, and sound schemes.",
            ControlPanelCategory::HardwareAndSound,
            "🔊",
            &["sound", "audio", "speakers", "headphones", "microphone", "recording", "volume"],
            false,
            false,
        ),
        ControlPanelApplet::new(
            "colorcpl.exe",
            "Color Management",
            "Microsoft.ColorManagement",
            "colorcpl.exe",
            "Manage display color profiles (ICC/ICM) for monitors and calibrated printers.",
            ControlPanelCategory::HardwareAndSound,
            "🎨",
            &["color", "icc", "profile", "monitor", "calibration", "display"],
            false,
            false,
        ),

        // --- Programs ---
        ControlPanelApplet::new(
            "appwiz.cpl",
            "Programs and Features",
            "Microsoft.ProgramsAndFeatures",
            "appwiz.cpl",
            "Uninstall, repair, or change installed applications and Windows features.",
            ControlPanelCategory::Programs,
            "📦",
            &["programs", "uninstall", "add remove", "features", "apps", "install"],
            false,
            true,
        ),
        ControlPanelApplet::new(
            "defaultprograms",
            "Default Programs",
            "Microsoft.DefaultPrograms",
            "control.exe /name Microsoft.DefaultPrograms",
            "Choose default programs for opening web links, media, and file extensions.",
            ControlPanelCategory::Programs,
            "⭐",
            &["default", "file associations", "extensions", "protocols", "browser"],
            false,
            false,
        ),

        // --- User Accounts ---
        ControlPanelApplet::new(
            "userpasswords",
            "User Accounts",
            "Microsoft.UserAccounts",
            "control.exe userpasswords",
            "Manage user account types, passwords, picture icons, and UAC settings.",
            ControlPanelCategory::UserAccounts,
            "👤",
            &["user", "accounts", "password", "administrator", "uac", "profile"],
            false,
            true,
        ),
        ControlPanelApplet::new(
            "credwiz.exe",
            "Credential Manager",
            "Microsoft.CredentialManager",
            "control.exe /name Microsoft.CredentialManager",
            "Manage saved Windows passwords, web credentials, certificates, and vaults.",
            ControlPanelCategory::UserAccounts,
            "🔑",
            &["credentials", "passwords", "vault", "tokens", "logins"],
            false,
            false,
        ),
        ControlPanelApplet::new(
            "mlcfg32.cpl",
            "Mail (Microsoft Outlook)",
            "Microsoft.Mail",
            "mlcfg32.cpl",
            "Configure email account profiles, PST/OST data files, and address books.",
            ControlPanelCategory::UserAccounts,
            "✉️",
            &["mail", "outlook", "email", "profiles", "pst", "data files"],
            false,
            false,
        ),

        // --- Appearance and Personalization ---
        ControlPanelApplet::new(
            "taskbar",
            "Taskbar and Navigation",
            "Microsoft.Taskbar",
            "control.exe /name Microsoft.Taskbar",
            "Customize taskbar alignment, notification icons, corner icons, and auto-hide.",
            ControlPanelCategory::AppearanceAndPersonalization,
            "📌",
            &["taskbar", "navigation", "start menu", "notification area", "tray"],
            false,
            false,
        ),
        ControlPanelApplet::new(
            "folders",
            "File Explorer Options",
            "Microsoft.FolderOptions",
            "control.exe folders",
            "Configure single-click opening, show hidden files/folders, and file extensions.",
            ControlPanelCategory::AppearanceAndPersonalization,
            "📂",
            &["folder options", "explorer", "hidden files", "extensions", "view"],
            false,
            false,
        ),
        ControlPanelApplet::new(
            "fonts",
            "Fonts",
            "Microsoft.Fonts",
            "control.exe fonts",
            "Preview, install, and remove TrueType and OpenType fonts on your system.",
            ControlPanelCategory::AppearanceAndPersonalization,
            "🔤",
            &["fonts", "typography", "truetype", "opentype", "typeface"],
            false,
            false,
        ),

        // --- Clock and Region ---
        ControlPanelApplet::new(
            "timedate.cpl",
            "Date and Time",
            "Microsoft.DateAndTime",
            "timedate.cpl",
            "Set system date, clock time, time zone, and synchronize with NTP internet time.",
            ControlPanelCategory::ClockAndRegion,
            "🕒",
            &["date", "time", "clock", "timezone", "ntp", "sync"],
            false,
            false,
        ),
        ControlPanelApplet::new(
            "intl.cpl",
            "Region",
            "Microsoft.RegionAndLanguage",
            "intl.cpl",
            "Configure numbers, currency formats, short/long date formats, and system locale.",
            ControlPanelCategory::ClockAndRegion,
            "🌐",
            &["region", "language", "locale", "currency", "format", "unicode"],
            false,
            false,
        ),

        // --- Ease of Access ---
        ControlPanelApplet::new(
            "access.cpl",
            "Ease of Access Center",
            "Microsoft.EaseOfAccessCenter",
            "control.exe /name Microsoft.EaseOfAccessCenter",
            "Configure accessibility tools: Magnifier, Narrator, High Contrast, On-Screen Keyboard.",
            ControlPanelCategory::EaseOfAccess,
            "♿",
            &["accessibility", "magnifier", "narrator", "high contrast", "keyboard"],
            false,
            false,
        ),
        ControlPanelApplet::new(
            "speech",
            "Speech Recognition",
            "Microsoft.SpeechRecognition",
            "control.exe /name Microsoft.SpeechRecognition",
            "Set up speech recognition, train microphone, and enable voice dictation.",
            ControlPanelCategory::EaseOfAccess,
            "🎙️",
            &["speech", "voice", "microphone", "dictation", "audio input"],
            false,
            false,
        ),

        // --- Additional Classic Applets ---
        ControlPanelApplet::new(
            "main.cpl",
            "Mouse",
            "Microsoft.Mouse",
            "main.cpl",
            "Customize mouse pointer schemes, cursor speed, scroll wheel lines, and double-click.",
            ControlPanelCategory::HardwareAndSound,
            "🖱️",
            &["mouse", "pointer", "cursor", "wheel", "buttons", "double click"],
            false,
            false,
        ),
        ControlPanelApplet::new(
            "main.cpl keyboard",
            "Keyboard",
            "Microsoft.Keyboard",
            "control.exe keyboard",
            "Set keyboard character repeat delay, repeat rate, and cursor blink speed.",
            ControlPanelCategory::HardwareAndSound,
            "⌨️",
            &["keyboard", "repeat rate", "blink rate", "input"],
            false,
            false,
        ),
        ControlPanelApplet::new(
            "telephon.cpl",
            "Phone and Modem",
            "Microsoft.PhoneAndModem",
            "telephon.cpl",
            "Configure telephone dialing rules, area codes, and analog/broadband modems.",
            ControlPanelCategory::HardwareAndSound,
            "📞",
            &["phone", "modem", "dialing rules", "telephony"],
            false,
            false,
        ),
        ControlPanelApplet::new(
            "troubleshooting",
            "Troubleshooting",
            "Microsoft.Troubleshooting",
            "control.exe /name Microsoft.Troubleshooting",
            "Automatically detect and fix problems with networks, audio, hardware, and updates.",
            ControlPanelCategory::SystemAndSecurity,
            "🩺",
            &["troubleshooting", "diagnose", "fix", "problems", "repair"],
            false,
            false,
        ),
        ControlPanelApplet::new(
            "recovery",
            "Recovery",
            "Microsoft.Recovery",
            "control.exe /name Microsoft.Recovery",
            "Create a recovery drive, open System Restore, or reset PC to factory state.",
            ControlPanelCategory::SystemAndSecurity,
            "🔄",
            &["recovery", "restore point", "system restore", "reset", "repair"],
            false,
            true,
        ),
        ControlPanelApplet::new(
            "mobsync.exe",
            "Sync Center",
            "Microsoft.SyncCenter",
            "mobsync.exe",
            "Manage file synchronization between your PC and offline network folders.",
            ControlPanelCategory::SystemAndSecurity,
            "🔄",
            &["sync center", "offline files", "network sync", "caching"],
            false,
            false,
        ),
    ]
}

/// Builds the category metadata and task hierarchy for Category view.
pub fn build_default_category_metadata() -> BTreeMap<ControlPanelCategory, CategoryMetadata> {
    let mut map = BTreeMap::new();

    map.insert(
        ControlPanelCategory::SystemAndSecurity,
        CategoryMetadata {
            category: ControlPanelCategory::SystemAndSecurity,
            primary_tasks: vec![
                CategoryTaskLink::new("Review your computer's status", "wscui.cpl", true),
                CategoryTaskLink::new("Save backup copies of your files with File History", "fhdr.cpl", true),
                CategoryTaskLink::new("Backup and Restore (Windows 7)", "sdclt.exe", true),
            ],
            sub_tasks: vec![
                CategoryTaskLink::new("Security and Maintenance", "wscui.cpl", false),
                CategoryTaskLink::new("Windows Defender Firewall", "firewall.cpl", false),
                CategoryTaskLink::new("System", "sysdm.cpl", false),
                CategoryTaskLink::new("Power Options", "powercfg.cpl", false),
                CategoryTaskLink::new("File History", "fhdr.cpl", false),
                CategoryTaskLink::new("BitLocker Drive Encryption", "bitlocker.cpl", false),
                CategoryTaskLink::new("Storage Spaces", "spaces.cpl", false),
                CategoryTaskLink::new("Work Folders", "workfolders.cpl", false),
                CategoryTaskLink::new("Administrative Tools", "admintools", false),
            ],
        },
    );

    map.insert(
        ControlPanelCategory::NetworkAndInternet,
        CategoryMetadata {
            category: ControlPanelCategory::NetworkAndInternet,
            primary_tasks: vec![
                CategoryTaskLink::new("View network status and tasks", "ncpa.cpl", true),
                CategoryTaskLink::new("Set up a new connection or network", "ncpa.cpl", true),
            ],
            sub_tasks: vec![
                CategoryTaskLink::new("Network and Sharing Center", "ncpa.cpl", false),
                CategoryTaskLink::new("Internet Options", "inetcpl.cpl", false),
                CategoryTaskLink::new("Infrared", "irprops.cpl", false),
            ],
        },
    );

    map.insert(
        ControlPanelCategory::HardwareAndSound,
        CategoryMetadata {
            category: ControlPanelCategory::HardwareAndSound,
            primary_tasks: vec![
                CategoryTaskLink::new("View devices and printers", "printers", true),
                CategoryTaskLink::new("Add a device", "printers", true),
                CategoryTaskLink::new("Adjust volume", "mmsys.cpl", true),
            ],
            sub_tasks: vec![
                CategoryTaskLink::new("Devices and Printers", "printers", false),
                CategoryTaskLink::new("Device Manager", "devmgmt.msc", false),
                CategoryTaskLink::new("AutoPlay", "autoplay.cpl", false),
                CategoryTaskLink::new("Sound", "mmsys.cpl", false),
                CategoryTaskLink::new("Power Options", "powercfg.cpl", false),
                CategoryTaskLink::new("Color Management", "colorcpl.exe", false),
                CategoryTaskLink::new("Mouse", "main.cpl", false),
                CategoryTaskLink::new("Keyboard", "main.cpl keyboard", false),
            ],
        },
    );

    map.insert(
        ControlPanelCategory::Programs,
        CategoryMetadata {
            category: ControlPanelCategory::Programs,
            primary_tasks: vec![
                CategoryTaskLink::new("Uninstall a program", "appwiz.cpl", true),
            ],
            sub_tasks: vec![
                CategoryTaskLink::new("Programs and Features", "appwiz.cpl", false),
                CategoryTaskLink::new("Default Programs", "defaultprograms", false),
            ],
        },
    );

    map.insert(
        ControlPanelCategory::UserAccounts,
        CategoryMetadata {
            category: ControlPanelCategory::UserAccounts,
            primary_tasks: vec![
                CategoryTaskLink::new("Change account type", "userpasswords", true),
                CategoryTaskLink::new("Remove user accounts", "userpasswords", true),
            ],
            sub_tasks: vec![
                CategoryTaskLink::new("User Accounts", "userpasswords", false),
                CategoryTaskLink::new("Credential Manager", "credwiz.exe", false),
                CategoryTaskLink::new("Mail (Microsoft Outlook)", "mlcfg32.cpl", false),
            ],
        },
    );

    map.insert(
        ControlPanelCategory::AppearanceAndPersonalization,
        CategoryMetadata {
            category: ControlPanelCategory::AppearanceAndPersonalization,
            primary_tasks: vec![
                CategoryTaskLink::new("Taskbar and Navigation", "taskbar", true),
                CategoryTaskLink::new("Specify single- or double-click to open", "folders", true),
            ],
            sub_tasks: vec![
                CategoryTaskLink::new("Taskbar and Navigation", "taskbar", false),
                CategoryTaskLink::new("File Explorer Options", "folders", false),
                CategoryTaskLink::new("Fonts", "fonts", false),
            ],
        },
    );

    map.insert(
        ControlPanelCategory::ClockAndRegion,
        CategoryMetadata {
            category: ControlPanelCategory::ClockAndRegion,
            primary_tasks: vec![
                CategoryTaskLink::new("Set the time and date", "timedate.cpl", true),
                CategoryTaskLink::new("Change date, time, or number formats", "intl.cpl", true),
            ],
            sub_tasks: vec![
                CategoryTaskLink::new("Date and Time", "timedate.cpl", false),
                CategoryTaskLink::new("Region", "intl.cpl", false),
            ],
        },
    );

    map.insert(
        ControlPanelCategory::EaseOfAccess,
        CategoryMetadata {
            category: ControlPanelCategory::EaseOfAccess,
            primary_tasks: vec![
                CategoryTaskLink::new("Let Windows suggest settings", "access.cpl", true),
                CategoryTaskLink::new("Optimize visual display", "access.cpl", true),
            ],
            sub_tasks: vec![
                CategoryTaskLink::new("Ease of Access Center", "access.cpl", false),
                CategoryTaskLink::new("Speech Recognition", "speech", false),
            ],
        },
    );

    map
}

// ============================================================================
// Section 3: Navigation, Breadcrumbs & Search Engine
// ============================================================================

/// Location target in the Control Panel navigation system.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavigationLocation {
    /// Control Panel root (Category view or All Items view)
    Root,
    /// Viewing a specific category (e.g. System and Security)
    Category(ControlPanelCategory),
    /// Viewing a specific Applet
    Applet(String),
    /// Viewing Administrative Tools root or a specific tool
    AdminTool(String),
    /// Active search results view
    SearchResults(String),
}

/// A breadcrumb segment with title and target navigation location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BreadcrumbSegment {
    pub label: String,
    pub location: NavigationLocation,
}

impl BreadcrumbSegment {
    pub fn new(label: impl Into<String>, location: NavigationLocation) -> Self {
        Self {
            label: label.into(),
            location,
        }
    }
}

/// Navigation history manager providing Windows Explorer-style Back/Forward/Up.
#[derive(Debug, Clone)]
pub struct NavigationHistory {
    back_stack: Vec<NavigationLocation>,
    forward_stack: Vec<NavigationLocation>,
    current: NavigationLocation,
}

impl Default for NavigationHistory {
    fn default() -> Self {
        Self::new()
    }
}

impl NavigationHistory {
    pub fn new() -> Self {
        Self {
            back_stack: Vec::new(),
            forward_stack: Vec::new(),
            current: NavigationLocation::Root,
        }
    }

    /// Returns the current location.
    pub fn current(&self) -> &NavigationLocation {
        &self.current
    }

    /// Can the user navigate Back?
    pub fn can_go_back(&self) -> bool {
        !self.back_stack.is_empty()
    }

    /// Can the user navigate Forward?
    pub fn can_go_forward(&self) -> bool {
        !self.forward_stack.is_empty()
    }

    /// Navigate to a new target location.
    pub fn navigate(&mut self, location: NavigationLocation) {
        if self.current == location {
            return;
        }
        self.back_stack.push(self.current.clone());
        self.forward_stack.clear();
        self.current = location;
    }

    /// Step backwards in history.
    pub fn go_back(&mut self) -> bool {
        if let Some(prev) = self.back_stack.pop() {
            self.forward_stack.push(self.current.clone());
            self.current = prev;
            true
        } else {
            false
        }
    }

    /// Step forwards in history.
    pub fn go_forward(&mut self) -> bool {
        if let Some(next) = self.forward_stack.pop() {
            self.back_stack.push(self.current.clone());
            self.current = next;
            true
        } else {
            false
        }
    }

    /// Navigate Up one hierarchical level.
    pub fn go_up(&mut self) -> bool {
        match &self.current {
            NavigationLocation::Root => false,
            NavigationLocation::Category(_) => {
                self.navigate(NavigationLocation::Root);
                true
            }
            NavigationLocation::Applet(_) => {
                self.navigate(NavigationLocation::Root);
                true
            }
            NavigationLocation::AdminTool(_) => {
                self.navigate(NavigationLocation::Category(ControlPanelCategory::SystemAndSecurity));
                true
            }
            NavigationLocation::SearchResults(_) => {
                self.navigate(NavigationLocation::Root);
                true
            }
        }
    }

    /// Generate the breadcrumb trail for the current location.
    pub fn breadcrumbs(&self) -> Vec<BreadcrumbSegment> {
        let mut crumbs = vec![BreadcrumbSegment::new("Control Panel", NavigationLocation::Root)];

        match &self.current {
            NavigationLocation::Root => {}
            NavigationLocation::Category(cat) => {
                crumbs.push(BreadcrumbSegment::new(
                    cat.display_name(),
                    NavigationLocation::Category(*cat),
                ));
            }
            NavigationLocation::Applet(id) => {
                crumbs.push(BreadcrumbSegment::new(
                    id.clone(),
                    NavigationLocation::Applet(id.clone()),
                ));
            }
            NavigationLocation::AdminTool(name) => {
                crumbs.push(BreadcrumbSegment::new(
                    "System and Security",
                    NavigationLocation::Category(ControlPanelCategory::SystemAndSecurity),
                ));
                crumbs.push(BreadcrumbSegment::new(
                    "Administrative Tools",
                    NavigationLocation::AdminTool("Administrative Tools".to_string()),
                ));
                if name != "Administrative Tools" {
                    crumbs.push(BreadcrumbSegment::new(
                        name.clone(),
                        NavigationLocation::AdminTool(name.clone()),
                    ));
                }
            }
            NavigationLocation::SearchResults(query) => {
                crumbs.push(BreadcrumbSegment::new(
                    format!("Search Results for \"{}\"", query),
                    NavigationLocation::SearchResults(query.clone()),
                ));
            }
        }
        crumbs
    }
}

/// Search result item containing matched applet and relevance score.
#[derive(Debug, Clone)]
pub struct SearchResultItem {
    pub applet: ControlPanelApplet,
    pub matched_field: &'static str,
    pub score: u32,
}

// ============================================================================
// Section 4: Administrative Tools - Event Viewer (eventvwr.msc)
// ============================================================================

/// Windows Event Log channels.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EventLogChannel {
    Application,
    Security,
    Setup,
    System,
    ForwardedEvents,
    Custom(String),
}

impl fmt::Display for EventLogChannel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Application => write!(f, "Application"),
            Self::Security => write!(f, "Security"),
            Self::Setup => write!(f, "Setup"),
            Self::System => write!(f, "System"),
            Self::ForwardedEvents => write!(f, "Forwarded Events"),
            Self::Custom(name) => write!(f, "{}", name),
        }
    }
}

/// Event severity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EventLevel {
    Critical = 1,
    Error = 2,
    Warning = 3,
    Information = 4,
    Verbose = 5,
}

impl EventLevel {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Critical => "Critical",
            Self::Error => "Error",
            Self::Warning => "Warning",
            Self::Information => "Information",
            Self::Verbose => "Verbose",
        }
    }

    pub const fn icon_glyph(&self) -> &'static str {
        match self {
            Self::Critical => "🛑",
            Self::Error => "❌",
            Self::Warning => "⚠️",
            Self::Information => "ℹ️",
            Self::Verbose => "🔍",
        }
    }
}

impl fmt::Display for EventLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// A structured Windows Event Log entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventLogEntry {
    pub record_id: u64,
    pub channel: EventLogChannel,
    pub level: EventLevel,
    pub timestamp_rfc3339: String,
    pub epoch_seconds: u64,
    pub source: String,
    pub event_id: u32,
    pub task_category: String,
    pub computer: String,
    pub user: String,
    pub description: String,
    pub xml_data: Option<String>,
}

impl EventLogEntry {
    pub fn new(
        record_id: u64,
        channel: EventLogChannel,
        level: EventLevel,
        timestamp_rfc3339: &str,
        epoch_seconds: u64,
        source: &str,
        event_id: u32,
        task_category: &str,
        computer: &str,
        user: &str,
        description: &str,
    ) -> Self {
        Self {
            record_id,
            channel,
            level,
            timestamp_rfc3339: timestamp_rfc3339.to_string(),
            epoch_seconds,
            source: source.to_string(),
            event_id,
            task_category: task_category.to_string(),
            computer: computer.to_string(),
            user: user.to_string(),
            description: description.to_string(),
            xml_data: None,
        }
    }

    /// Export this entry as standard Windows EVTX XML format string.
    pub fn to_xml(&self) -> String {
        format!(
            "<Event xmlns=\"http://schemas.microsoft.com/win/2004/08/events/event\">\n  \
               <System>\n    \
                 <Provider Name=\"{}\"/>\n    \
                 <EventID>{}</EventID>\n    \
                 <Level>{}</Level>\n    \
                 <Task>{}</Task>\n    \
                 <TimeCreated SystemTime=\"{}\"/>\n    \
                 <EventRecordID>{}</EventRecordID>\n    \
                 <Channel>{}</Channel>\n    \
                 <Computer>{}</Computer>\n    \
                 <Security UserID=\"{}\"/>\n  \
               </System>\n  \
               <EventData>\n    \
                 <Data Name=\"Message\">{}</Data>\n  \
               </EventData>\n\
             </Event>",
            self.source,
            self.event_id,
            self.level as u8,
            self.task_category,
            self.timestamp_rfc3339,
            self.record_id,
            self.channel,
            self.computer,
            self.user,
            self.description
        )
    }
}

/// Filter criteria for querying Event Viewer logs.
#[derive(Debug, Clone, Default)]
pub struct EventFilter {
    pub channels: Option<Vec<EventLogChannel>>,
    pub min_level: Option<EventLevel>,
    pub levels: Option<Vec<EventLevel>>,
    pub event_ids: Option<Vec<u32>>,
    pub source_pattern: Option<String>,
    pub text_pattern: Option<String>,
    pub start_epoch: Option<u64>,
    pub end_epoch: Option<u64>,
}

impl EventFilter {
    pub fn matches(&self, entry: &EventLogEntry) -> bool {
        if let Some(ref chs) = self.channels {
            if !chs.contains(&entry.channel) {
                return false;
            }
        }
        if let Some(ref lvls) = self.levels {
            if !lvls.contains(&entry.level) {
                return false;
            }
        }
        if let Some(min_lvl) = self.min_level {
            if (entry.level as u8) > (min_lvl as u8) {
                return false;
            }
        }
        if let Some(ref ids) = self.event_ids {
            if !ids.contains(&entry.event_id) {
                return false;
            }
        }
        if let Some(ref src) = self.source_pattern {
            if !entry.source.to_lowercase().contains(&src.to_lowercase()) {
                return false;
            }
        }
        if let Some(ref txt) = self.text_pattern {
            if !entry.description.to_lowercase().contains(&txt.to_lowercase()) {
                return false;
            }
        }
        if let Some(start) = self.start_epoch {
            if entry.epoch_seconds < start {
                return false;
            }
        }
        if let Some(end) = self.end_epoch {
            if entry.epoch_seconds > end {
                return false;
            }
        }
        true
    }
}

/// Complete Windows Event Viewer subsystem.
#[derive(Debug, Clone)]
pub struct EventViewer {
    logs: BTreeMap<EventLogChannel, Vec<EventLogEntry>>,
    next_record_id: u64,
}

impl Default for EventViewer {
    fn default() -> Self {
        Self::new()
    }
}

impl EventViewer {
    pub fn new() -> Self {
        let mut ev = Self {
            logs: BTreeMap::new(),
            next_record_id: 1000,
        };
        ev.populate_initial_events();
        ev
    }

    /// Logs an event to the specified channel.
    pub fn log_event(
        &mut self,
        channel: EventLogChannel,
        level: EventLevel,
        source: &str,
        event_id: u32,
        task_category: &str,
        computer: &str,
        user: &str,
        description: &str,
    ) -> u64 {
        let record_id = self.next_record_id;
        self.next_record_id += 1;

        let entry = EventLogEntry::new(
            record_id,
            channel.clone(),
            level,
            "2026-11-10T12:00:00Z",
            1794312000,
            source,
            event_id,
            task_category,
            computer,
            user,
            description,
        );

        self.logs.entry(channel).or_default().push(entry);
        record_id
    }

    /// Retrieve all events matching filter criteria.
    pub fn query_events(&self, filter: &EventFilter) -> Vec<&EventLogEntry> {
        let mut results = Vec::new();
        for entries in self.logs.values() {
            for entry in entries {
                if filter.matches(entry) {
                    results.push(entry);
                }
            }
        }
        // Most recent first
        results.sort_by(|a, b| b.record_id.cmp(&a.record_id));
        results
    }

    /// Get all entries for a specific channel.
    pub fn get_channel_entries(&self, channel: &EventLogChannel) -> &[EventLogEntry] {
        self.logs.get(channel).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Clear a specific channel log. Returns number of cleared entries.
    pub fn clear_channel(&mut self, channel: &EventLogChannel) -> usize {
        if let Some(entries) = self.logs.get_mut(channel) {
            let count = entries.len();
            entries.clear();
            count
        } else {
            0
        }
    }

    /// Export an entire channel as formatted XML string.
    pub fn export_channel_xml(&self, channel: &EventLogChannel) -> String {
        let mut xml = String::from("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<Events>\n");
        if let Some(entries) = self.logs.get(channel) {
            for entry in entries {
                xml.push_str(&entry.to_xml());
                xml.push('\n');
            }
        }
        xml.push_str("</Events>");
        xml
    }

    /// Returns log channel summary statistics (Total, Errors, Warnings, Info).
    pub fn channel_statistics(&self) -> BTreeMap<EventLogChannel, (usize, usize, usize, usize)> {
        let mut stats = BTreeMap::new();
        for (channel, entries) in &self.logs {
            let mut errors = 0;
            let mut warnings = 0;
            let mut info = 0;
            for e in entries {
                match e.level {
                    EventLevel::Critical | EventLevel::Error => errors += 1,
                    EventLevel::Warning => warnings += 1,
                    EventLevel::Information | EventLevel::Verbose => info += 1,
                }
            }
            stats.insert(channel.clone(), (entries.len(), errors, warnings, info));
        }
        stats
    }

    /// Seed authentic Windows startup, audit, and operational events.
    fn populate_initial_events(&mut self) {
        // System log events
        self.log_event(
            EventLogChannel::System,
            EventLevel::Information,
            "Microsoft-Windows-Kernel-General",
            1,
            "Time Change",
            "HYBRIDOS-PC",
            "SYSTEM",
            "The system time was synchronized with NTP server time.windows.com.",
        );
        self.log_event(
            EventLogChannel::System,
            EventLevel::Information,
            "Service Control Manager",
            7036,
            "Service State Change",
            "HYBRIDOS-PC",
            "SYSTEM",
            "The Windows Event Log service entered the running state.",
        );
        self.log_event(
            EventLogChannel::System,
            EventLevel::Information,
            "Service Control Manager",
            7036,
            "Service State Change",
            "HYBRIDOS-PC",
            "SYSTEM",
            "The HybridOS Vortex Driver Host service entered the running state.",
        );
        self.log_event(
            EventLogChannel::System,
            EventLevel::Warning,
            "Microsoft-Windows-Kernel-PnP",
            219,
            "Driver Load",
            "HYBRIDOS-PC",
            "SYSTEM",
            "The driver \\Driver\\WudfRd failed to load for device ROOT\\WPD\\0000.",
        );
        self.log_event(
            EventLogChannel::System,
            EventLevel::Error,
            "Microsoft-Windows-DistributedCOM",
            10016,
            "Security Configuration",
            "HYBRIDOS-PC",
            "LOCAL SERVICE",
            "The application-specific permission settings do not grant Local Activation permission for COM Server.",
        );

        // Security log events
        self.log_event(
            EventLogChannel::Security,
            EventLevel::Information,
            "Microsoft-Windows-Security-Auditing",
            4624,
            "Logon",
            "HYBRIDOS-PC",
            "SYSTEM",
            "An account was successfully logged on. Account Name: Vinicius, Logon Type: 2 (Interactive).",
        );
        self.log_event(
            EventLogChannel::Security,
            EventLevel::Information,
            "Microsoft-Windows-Security-Auditing",
            4672,
            "Special Logon",
            "HYBRIDOS-PC",
            "SYSTEM",
            "Special privileges assigned to new logon: SeSecurityPrivilege, SeBackupPrivilege.",
        );

        // Application log events
        self.log_event(
            EventLogChannel::Application,
            EventLevel::Information,
            "Desktop Window Manager",
            9027,
            "Composition State",
            "HYBRIDOS-PC",
            "SYSTEM",
            "The Desktop Window Manager has registered the session port and initialized Mica material pipeline.",
        );
        self.log_event(
            EventLogChannel::Application,
            EventLevel::Information,
            "HybridOS-Kernel",
            100,
            "Personality Host",
            "HYBRIDOS-PC",
            "SYSTEM",
            "Dual-personality engine activated: Windows Win32 Subsystem & Linux POSIX subsystem ready.",
        );

        // Setup log events
        self.log_event(
            EventLogChannel::Setup,
            EventLevel::Information,
            "Microsoft-Windows-Servicing",
            1,
            "Update Installation",
            "HYBRIDOS-PC",
            "SYSTEM",
            "Cumulative Update for HybridOS Commercial Desktop 24H2 successfully verified and committed.",
        );
    }
}

// ============================================================================
// Section 5: Administrative Tools - Services (services.msc)
// ============================================================================

/// Execution status of a Windows Service.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ServiceStatus {
    Running,
    #[default]
    Stopped,
    Paused,
    Starting,
    Stopping,
    Pausing,
    Continuing,
}

impl ServiceStatus {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Running => "Running",
            Self::Stopped => "Stopped",
            Self::Paused => "Paused",
            Self::Starting => "Starting",
            Self::Stopping => "Stopping",
            Self::Pausing => "Pausing",
            Self::Continuing => "Resuming",
        }
    }
}

impl fmt::Display for ServiceStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Service startup configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StartupType {
    #[default]
    Automatic,
    AutomaticDelayed,
    Manual,
    Disabled,
}

impl StartupType {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Automatic => "Automatic",
            Self::AutomaticDelayed => "Automatic (Delayed Start)",
            Self::Manual => "Manual",
            Self::Disabled => "Disabled",
        }
    }
}

impl fmt::Display for StartupType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Security context the service runs under.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceAccount {
    LocalSystem,
    LocalService,
    NetworkService,
    Custom(String),
}

impl fmt::Display for ServiceAccount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LocalSystem => write!(f, "Local System"),
            Self::LocalService => write!(f, "Local Service"),
            Self::NetworkService => write!(f, "Network Service"),
            Self::Custom(acc) => write!(f, ".\\{}", acc),
        }
    }
}

/// Recovery action on service crash/failure.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum RecoveryAction {
    #[default]
    RestartService,
    RunProgram(String),
    RebootComputer,
    TakeNoAction,
}

/// Comprehensive service entry in the Service Control Manager.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceEntry {
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub status: ServiceStatus,
    pub startup_type: StartupType,
    pub log_on_as: ServiceAccount,
    pub pid: Option<u32>,
    pub binary_path: String,
    pub dependencies: Vec<String>,
    pub dependents: Vec<String>,
    pub can_pause: bool,
    pub can_stop: bool,
    pub recovery_action_first: RecoveryAction,
    pub recovery_action_second: RecoveryAction,
}

impl ServiceEntry {
    pub fn new(
        name: &str,
        display_name: &str,
        description: &str,
        status: ServiceStatus,
        startup_type: StartupType,
        log_on_as: ServiceAccount,
        binary_path: &str,
        dependencies: &[&str],
        can_pause: bool,
    ) -> Self {
        Self {
            name: name.to_string(),
            display_name: display_name.to_string(),
            description: description.to_string(),
            status,
            startup_type,
            log_on_as,
            pid: if status == ServiceStatus::Running { Some(1024) } else { None },
            binary_path: binary_path.to_string(),
            dependencies: dependencies.iter().map(|&d| d.to_string()).collect(),
            dependents: Vec::new(),
            can_pause,
            can_stop: true,
            recovery_action_first: RecoveryAction::RestartService,
            recovery_action_second: RecoveryAction::RestartService,
        }
    }
}

/// Properties modal data representation for a service.
#[derive(Debug, Clone)]
pub struct ServiceProperties {
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub binary_path: String,
    pub startup_type: StartupType,
    pub status: ServiceStatus,
    pub log_on_as: ServiceAccount,
    pub dependencies: Vec<String>,
    pub dependents: Vec<String>,
    pub recovery_first: RecoveryAction,
}

/// Service Control Manager (services.msc) subsystem.
#[derive(Debug, Clone)]
pub struct ServicesManager {
    services: BTreeMap<String, ServiceEntry>,
    next_pid: u32,
}

impl Default for ServicesManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ServicesManager {
    pub fn new() -> Self {
        let mut sm = Self {
            services: BTreeMap::new(),
            next_pid: 2400,
        };
        sm.populate_default_services();
        sm.recalculate_dependents();
        sm
    }

    /// Returns a list of all services sorted by display name.
    pub fn list_services(&self) -> Vec<&ServiceEntry> {
        let mut list: Vec<&ServiceEntry> = self.services.values().collect();
        list.sort_by(|a, b| a.display_name.to_lowercase().cmp(&b.display_name.to_lowercase()));
        list
    }

    /// Retrieve service by internal service name.
    pub fn get_service(&self, name: &str) -> Option<&ServiceEntry> {
        self.services.get(name)
    }

    /// Start a service by name. Automatically validates startup type and dependencies.
    pub fn start_service(&mut self, name: &str) -> Result<(), String> {
        let svc = self.services.get(name).ok_or_else(|| format!("Service '{}' not found", name))?;

        if svc.status == ServiceStatus::Running {
            return Ok(());
        }
        if svc.startup_type == StartupType::Disabled {
            return Err(format!("Cannot start service '{}' because it is Disabled.", name));
        }

        // Validate dependencies
        let deps = svc.dependencies.clone();
        for dep in &deps {
            let dep_svc = self.services.get(dep).ok_or_else(|| {
                format!("Dependency '{}' for service '{}' not found", dep, name)
            })?;
            if dep_svc.status != ServiceStatus::Running {
                // Recursively start dependent service if possible
                self.start_service(dep)?;
            }
        }

        let pid = self.next_pid;
        self.next_pid += 4;

        if let Some(svc_mut) = self.services.get_mut(name) {
            svc_mut.status = ServiceStatus::Running;
            svc_mut.pid = Some(pid);
        }
        Ok(())
    }

    /// Stop a service by name. Validates running dependents.
    pub fn stop_service(&mut self, name: &str) -> Result<(), String> {
        let svc = self.services.get(name).ok_or_else(|| format!("Service '{}' not found", name))?;

        if svc.status == ServiceStatus::Stopped {
            return Ok(());
        }
        if !svc.can_stop {
            return Err(format!("Service '{}' cannot be stopped.", name));
        }

        // Check if any dependent services are running
        let running_dependents: Vec<String> = svc.dependents
            .iter()
            .filter(|dep_name| {
                self.services.get(*dep_name).map_or(false, |s| s.status == ServiceStatus::Running)
            })
            .cloned()
            .collect();

        if !running_dependents.is_empty() {
            return Err(format!(
                "Cannot stop service '{}' because the following active services depend on it: {:?}",
                name, running_dependents
            ));
        }

        if let Some(svc_mut) = self.services.get_mut(name) {
            svc_mut.status = ServiceStatus::Stopped;
            svc_mut.pid = None;
        }
        Ok(())
    }

    /// Pause a running service.
    pub fn pause_service(&mut self, name: &str) -> Result<(), String> {
        let svc = self.services.get(name).ok_or_else(|| format!("Service '{}' not found", name))?;

        if svc.status != ServiceStatus::Running {
            return Err(format!("Service '{}' must be running to pause.", name));
        }
        if !svc.can_pause {
            return Err(format!("Service '{}' does not support pausing.", name));
        }

        if let Some(svc_mut) = self.services.get_mut(name) {
            svc_mut.status = ServiceStatus::Paused;
        }
        Ok(())
    }

    /// Resume a paused service.
    pub fn resume_service(&mut self, name: &str) -> Result<(), String> {
        let svc = self.services.get(name).ok_or_else(|| format!("Service '{}' not found", name))?;

        if svc.status != ServiceStatus::Paused {
            return Err(format!("Service '{}' is not paused.", name));
        }

        if let Some(svc_mut) = self.services.get_mut(name) {
            svc_mut.status = ServiceStatus::Running;
        }
        Ok(())
    }

    /// Restart a running or stopped service.
    pub fn restart_service(&mut self, name: &str) -> Result<(), String> {
        if let Some(svc) = self.services.get(name) {
            if svc.status == ServiceStatus::Running || svc.status == ServiceStatus::Paused {
                self.stop_service(name)?;
            }
        }
        self.start_service(name)
    }

    /// Configure service startup type.
    pub fn set_startup_type(&mut self, name: &str, startup_type: StartupType) -> Result<(), String> {
        let svc = self.services.get_mut(name).ok_or_else(|| format!("Service '{}' not found", name))?;
        svc.startup_type = startup_type;
        Ok(())
    }

    /// Retrieve detailed Properties dialog data.
    pub fn get_service_properties(&self, name: &str) -> Result<ServiceProperties, String> {
        let svc = self.services.get(name).ok_or_else(|| format!("Service '{}' not found", name))?;
        Ok(ServiceProperties {
            name: svc.name.clone(),
            display_name: svc.display_name.clone(),
            description: svc.description.clone(),
            binary_path: svc.binary_path.clone(),
            startup_type: svc.startup_type,
            status: svc.status,
            log_on_as: svc.log_on_as.clone(),
            dependencies: svc.dependencies.clone(),
            dependents: svc.dependents.clone(),
            recovery_first: svc.recovery_action_first.clone(),
        })
    }

    /// Recalculate the inverse dependent graph for all services.
    fn recalculate_dependents(&mut self) {
        let mut dependents_map: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (svc_name, svc) in &self.services {
            for dep in &svc.dependencies {
                dependents_map.entry(dep.clone()).or_default().push(svc_name.clone());
            }
        }
        for (svc_name, dependents) in dependents_map {
            if let Some(svc) = self.services.get_mut(&svc_name) {
                svc.dependents = dependents;
            }
        }
    }

    /// Seed standard Windows and HybridOS services.
    fn populate_default_services(&mut self) {
        let mut add = |entry: ServiceEntry| {
            self.services.insert(entry.name.clone(), entry);
        };

        add(ServiceEntry::new(
            "RpcSs",
            "Remote Procedure Call (RPC)",
            "The RPCSS service is the Service Control Manager for COM and DCOM servers.",
            ServiceStatus::Running,
            StartupType::Automatic,
            ServiceAccount::NetworkService,
            "C:\\Windows\\system32\\svchost.exe -k rpcss",
            &[],
            false,
        ));

        add(ServiceEntry::new(
            "EventLog",
            "Windows Event Log",
            "Manages events and event logs. Supports logging events, querying events, and archiving.",
            ServiceStatus::Running,
            StartupType::Automatic,
            ServiceAccount::LocalService,
            "C:\\Windows\\System32\\svchost.exe -k LocalServiceNetworkRestricted",
            &[],
            false,
        ));

        add(ServiceEntry::new(
            "Dhcp",
            "DHCP Client",
            "Registers and updates IP addresses and DNS records for this computer.",
            ServiceStatus::Running,
            StartupType::Automatic,
            ServiceAccount::LocalService,
            "C:\\Windows\\system32\\svchost.exe -k LocalServiceNetworkRestricted",
            &["RpcSs"],
            false,
        ));

        add(ServiceEntry::new(
            "Dnscache",
            "DNS Client",
            "The DNS Client service resolves and caches Domain Name System (DNS) names.",
            ServiceStatus::Running,
            StartupType::Automatic,
            ServiceAccount::NetworkService,
            "C:\\Windows\\system32\\svchost.exe -k NetworkService",
            &[],
            false,
        ));

        add(ServiceEntry::new(
            "Spooler",
            "Print Spooler",
            "This service spools print jobs and handles interaction with the printer.",
            ServiceStatus::Running,
            StartupType::Automatic,
            ServiceAccount::LocalSystem,
            "C:\\Windows\\System32\\spoolsv.exe",
            &["RpcSs"],
            true,
        ));

        add(ServiceEntry::new(
            "AudioEndpointBuilder",
            "Windows Audio Endpoint Builder",
            "Manages audio devices for the Windows Audio service.",
            ServiceStatus::Running,
            StartupType::Automatic,
            ServiceAccount::LocalSystem,
            "C:\\Windows\\System32\\svchost.exe -k LocalSystemNetworkRestricted",
            &["RpcSs"],
            false,
        ));

        add(ServiceEntry::new(
            "AudioSrv",
            "Windows Audio",
            "Manages audio for Windows-based programs. If this service is stopped, audio will not work.",
            ServiceStatus::Running,
            StartupType::Automatic,
            ServiceAccount::LocalService,
            "C:\\Windows\\System32\\svchost.exe -k LocalServiceNetworkRestricted",
            &["AudioEndpointBuilder", "RpcSs"],
            true,
        ));

        add(ServiceEntry::new(
            "BFE",
            "Base Filtering Engine",
            "The Base Filtering Engine (BFE) is a service that manages firewall and IPsec policies.",
            ServiceStatus::Running,
            StartupType::Automatic,
            ServiceAccount::LocalService,
            "C:\\Windows\\system32\\svchost.exe -k LocalServiceNoNetwork",
            &["RpcSs"],
            false,
        ));

        add(ServiceEntry::new(
            "MpsSvc",
            "Windows Defender Firewall",
            "Windows Defender Firewall helps protect your computer by preventing unauthorized users from gaining access.",
            ServiceStatus::Running,
            StartupType::Automatic,
            ServiceAccount::LocalService,
            "C:\\Windows\\system32\\svchost.exe -k LocalServiceNoNetwork",
            &["BFE", "RpcSs"],
            false,
        ));

        add(ServiceEntry::new(
            "wuauserv",
            "Windows Update",
            "Enables the detection, download, and installation of updates for Windows and other programs.",
            ServiceStatus::Running,
            StartupType::Manual,
            ServiceAccount::LocalSystem,
            "C:\\Windows\\system32\\svchost.exe -k netsvcs",
            &["RpcSs"],
            true,
        ));

        add(ServiceEntry::new(
            "Themes",
            "Themes",
            "Provides user experience theme management including Fluent Design, Mica, and Acrylic.",
            ServiceStatus::Running,
            StartupType::Automatic,
            ServiceAccount::LocalSystem,
            "C:\\Windows\\System32\\svchost.exe -k netsvcs",
            &[],
            false,
        ));

        add(ServiceEntry::new(
            "LanmanWorkstation",
            "Workstation",
            "Creates and maintains client network connections to remote servers using the SMB protocol.",
            ServiceStatus::Running,
            StartupType::Automatic,
            ServiceAccount::NetworkService,
            "C:\\Windows\\System32\\svchost.exe -k NetworkService",
            &[],
            true,
        ));

        add(ServiceEntry::new(
            "HybridVdmHost",
            "HybridOS Vortex Driver Host",
            "Manages and hosts native Windows WDM/WDF and Linux DKMS driver isolation layers.",
            ServiceStatus::Running,
            StartupType::Automatic,
            ServiceAccount::LocalSystem,
            "C:\\Windows\\System32\\vdm_host.exe",
            &["RpcSs", "EventLog"],
            true,
        ));

        add(ServiceEntry::new(
            "HybridLinuxSubsystem",
            "HybridOS Linux Personality Subsystem",
            "Provides transparent kernel-level syscall mapping and ELF64 binary execution alongside Win32.",
            ServiceStatus::Running,
            StartupType::Automatic,
            ServiceAccount::LocalSystem,
            "C:\\Windows\\System32\\hybridos_linux.exe",
            &["HybridVdmHost"],
            true,
        ));

        add(ServiceEntry::new(
            "XboxGipSvc",
            "Xbox Accessory Management Service",
            "Manages connected Xbox gaming accessories and wireless controller adapters.",
            ServiceStatus::Stopped,
            StartupType::Manual,
            ServiceAccount::LocalSystem,
            "C:\\Windows\\system32\\svchost.exe -k netsvcs",
            &[],
            false,
        ));

        add(ServiceEntry::new(
            "RemoteRegistry",
            "Remote Registry",
            "Enables remote users to modify registry settings on this computer.",
            ServiceStatus::Stopped,
            StartupType::Disabled,
            ServiceAccount::LocalService,
            "C:\\Windows\\system32\\svchost.exe -k regsvc",
            &["RpcSs"],
            false,
        ));
    }
}

// ============================================================================
// Section 6: Administrative Tools - Disk Management (diskmgmt.msc)
// ============================================================================

/// Physical bus interconnect type for disk storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiskBusType {
    NVMe,
    SATA,
    USB,
    SCSI,
    Virtual,
}

impl fmt::Display for DiskBusType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NVMe => write!(f, "NVMe"),
            Self::SATA => write!(f, "SATA"),
            Self::USB => write!(f, "USB"),
            Self::SCSI => write!(f, "SCSI"),
            Self::Virtual => write!(f, "Virtual"),
        }
    }
}

/// Partition table architecture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiskPartitionStyle {
    GPT,
    MBR,
    RAW,
}

impl fmt::Display for DiskPartitionStyle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GPT => write!(f, "GPT (GUID Partition Table)"),
            Self::MBR => write!(f, "MBR (Master Boot Record)"),
            Self::RAW => write!(f, "RAW (Uninitialized)"),
        }
    }
}

/// Physical disk device online status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiskStatus {
    Online,
    Offline,
    ReadOnly,
    Unallocated,
}

impl fmt::Display for DiskStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Online => write!(f, "Online"),
            Self::Offline => write!(f, "Offline"),
            Self::ReadOnly => write!(f, "Read-Only"),
            Self::Unallocated => write!(f, "Unallocated"),
        }
    }
}

/// Partition filesystem format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum PartitionFileSystem {
    NTFS,
    FAT32,
    exFAT,
    ReFS,
    Ext4,
    Raw,
}

impl fmt::Display for PartitionFileSystem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NTFS => write!(f, "NTFS"),
            Self::FAT32 => write!(f, "FAT32"),
            Self::exFAT => write!(f, "exFAT"),
            Self::ReFS => write!(f, "ReFS"),
            Self::Ext4 => write!(f, "ext4"),
            Self::Raw => write!(f, "RAW"),
        }
    }
}

/// Partition classification type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartitionType {
    EfiSystem,
    Msr,
    Primary,
    Recovery,
    LinuxNative,
    Swap,
}

impl fmt::Display for PartitionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EfiSystem => write!(f, "EFI System Partition"),
            Self::Msr => write!(f, "Microsoft Reserved Partition"),
            Self::Primary => write!(f, "Basic Data Partition"),
            Self::Recovery => write!(f, "Recovery Partition"),
            Self::LinuxNative => write!(f, "Linux Filesystem"),
            Self::Swap => write!(f, "Linux Swap"),
        }
    }
}

/// A partition volume on a physical disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Partition {
    pub partition_index: u32,
    pub drive_letter: Option<char>,
    pub volume_label: String,
    pub partition_type: PartitionType,
    pub file_system: PartitionFileSystem,
    pub status_flags: Vec<String>,
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub free_bytes: u64,
    pub is_boot: bool,
    pub is_system: bool,
    pub is_read_only: bool,
}

impl Partition {
    pub fn new(
        partition_index: u32,
        drive_letter: Option<char>,
        volume_label: &str,
        partition_type: PartitionType,
        file_system: PartitionFileSystem,
        status_flags: &[&str],
        total_bytes: u64,
        used_bytes: u64,
        is_boot: bool,
        is_system: bool,
    ) -> Self {
        let free_bytes = total_bytes.saturating_sub(used_bytes);
        Self {
            partition_index,
            drive_letter,
            volume_label: volume_label.to_string(),
            partition_type,
            file_system,
            status_flags: status_flags.iter().map(|&s| s.to_string()).collect(),
            total_bytes,
            used_bytes,
            free_bytes,
            is_boot,
            is_system,
            is_read_only: false,
        }
    }

    /// Formatted capacity string (e.g. "450.00 GB" or "100 MB").
    pub fn capacity_formatted(&self) -> String {
        format_bytes(self.total_bytes)
    }

    /// Formatted free space string.
    pub fn free_formatted(&self) -> String {
        format_bytes(self.free_bytes)
    }

    /// Percentage of disk capacity currently free.
    pub fn free_percent(&self) -> f32 {
        if self.total_bytes == 0 {
            0.0
        } else {
            (self.free_bytes as f32 / self.total_bytes as f32) * 100.0
        }
    }

    /// Percentage of disk capacity currently used.
    pub fn used_percent(&self) -> f32 {
        if self.total_bytes == 0 {
            0.0
        } else {
            (self.used_bytes as f32 / self.total_bytes as f32) * 100.0
        }
    }

    /// Header string displayed in Disk Management graphical view.
    pub fn display_header(&self) -> String {
        if let Some(letter) = self.drive_letter {
            if self.volume_label.is_empty() {
                format!("({}:)", letter)
            } else {
                format!("{} ({}:)", self.volume_label, letter)
            }
        } else if self.volume_label.is_empty() {
            format!("{}", self.partition_type)
        } else {
            self.volume_label.clone()
        }
    }

    /// Complete health status description string.
    pub fn health_status_string(&self) -> String {
        let flags = self.status_flags.join(", ");
        if flags.is_empty() {
            "Healthy".to_string()
        } else {
            format!("Healthy ({})", flags)
        }
    }
}

/// A physical disk drive device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalDisk {
    pub disk_number: u32,
    pub model_name: String,
    pub bus_type: DiskBusType,
    pub partition_style: DiskPartitionStyle,
    pub status: DiskStatus,
    pub total_bytes: u64,
    pub unallocated_bytes: u64,
    pub partitions: Vec<Partition>,
}

impl PhysicalDisk {
    pub fn capacity_formatted(&self) -> String {
        format_bytes(self.total_bytes)
    }

    /// Formatted status summary for left drive header pane.
    pub fn summary_string(&self) -> String {
        format!(
            "Disk {}\nBasic\n{}\n{}\n{}",
            self.disk_number,
            self.capacity_formatted(),
            self.status,
            self.model_name
        )
    }
}

/// Complete Disk Management subsystem (`diskmgmt.msc`).
#[derive(Debug, Clone)]
pub struct DiskManagement {
    pub disks: Vec<PhysicalDisk>,
}

impl Default for DiskManagement {
    fn default() -> Self {
        Self::new()
    }
}

impl DiskManagement {
    pub fn new() -> Self {
        let mut dm = Self { disks: Vec::new() };
        dm.populate_default_storage();
        dm
    }

    /// Format a volume on a specified disk and partition index.
    pub fn format_volume(
        &mut self,
        disk_idx: usize,
        part_idx: usize,
        fs: PartitionFileSystem,
        label: &str,
        quick: bool,
    ) -> Result<(), String> {
        let disk = self.disks.get_mut(disk_idx).ok_or_else(|| format!("Disk {} not found", disk_idx))?;
        let part = disk.partitions.get_mut(part_idx).ok_or_else(|| format!("Partition {} not found", part_idx))?;

        if part.is_system || part.partition_type == PartitionType::EfiSystem || part.partition_type == PartitionType::Msr {
            return Err("Cannot format active System, EFI, or MSR partitions.".to_string());
        }

        part.file_system = fs;
        part.volume_label = label.to_string();
        // Clear used data space on format
        part.used_bytes = if quick { 128 * 1024 * 1024 } else { 0 }; // 128MB fs overhead
        part.free_bytes = part.total_bytes.saturating_sub(part.used_bytes);
        Ok(())
    }

    /// Assign or change drive letter for a partition (e.g. Some('D') or None to unassign).
    pub fn assign_drive_letter(
        &mut self,
        disk_idx: usize,
        part_idx: usize,
        letter: Option<char>,
    ) -> Result<(), String> {
        // Validate letter format and availability
        if let Some(ch) = letter {
            let upper = ch.to_ascii_uppercase();
            if !ch.is_ascii_alphabetic() {
                return Err("Drive letter must be an ASCII letter (A-Z).".to_string());
            }

            // Check for collision across all existing partitions
            for (d_idx, d) in self.disks.iter().enumerate() {
                for (p_idx, p) in d.partitions.iter().enumerate() {
                    if (d_idx != disk_idx || p_idx != part_idx) && p.drive_letter == Some(upper) {
                        return Err(format!("Drive letter '{}:' is already in use by another volume.", upper));
                    }
                }
            }

            let disk = self.disks.get_mut(disk_idx).ok_or_else(|| format!("Disk {} not found", disk_idx))?;
            let part = disk.partitions.get_mut(part_idx).ok_or_else(|| format!("Partition {} not found", part_idx))?;

            if part.partition_type == PartitionType::EfiSystem || part.partition_type == PartitionType::Msr {
                return Err("Cannot assign drive letters to EFI or MSR partitions.".to_string());
            }

            part.drive_letter = Some(upper);
        } else {
            let disk = self.disks.get_mut(disk_idx).ok_or_else(|| format!("Disk {} not found", disk_idx))?;
            let part = disk.partitions.get_mut(part_idx).ok_or_else(|| format!("Partition {} not found", part_idx))?;

            if part.is_boot {
                return Err("Cannot remove drive letter from active Boot partition.".to_string());
            }
            part.drive_letter = None;
        }
        Ok(())
    }

    /// Shrink a volume, releasing space into disk unallocated pool.
    pub fn shrink_volume(&mut self, disk_idx: usize, part_idx: usize, shrink_bytes: u64) -> Result<(), String> {
        let disk = self.disks.get_mut(disk_idx).ok_or_else(|| format!("Disk {} not found", disk_idx))?;
        let part = disk.partitions.get_mut(part_idx).ok_or_else(|| format!("Partition {} not found", part_idx))?;

        if shrink_bytes == 0 {
            return Err("Shrink amount must be greater than zero.".to_string());
        }
        if shrink_bytes >= part.free_bytes {
            return Err(format!(
                "Cannot shrink volume by {} because only {} is free space.",
                format_bytes(shrink_bytes),
                format_bytes(part.free_bytes)
            ));
        }

        part.total_bytes -= shrink_bytes;
        part.free_bytes -= shrink_bytes;
        disk.unallocated_bytes += shrink_bytes;
        Ok(())
    }

    /// Extend a volume using available unallocated disk space.
    pub fn extend_volume(&mut self, disk_idx: usize, part_idx: usize, extend_bytes: u64) -> Result<(), String> {
        let disk = self.disks.get_mut(disk_idx).ok_or_else(|| format!("Disk {} not found", disk_idx))?;

        if extend_bytes > disk.unallocated_bytes {
            return Err(format!(
                "Cannot extend volume by {} because only {} is unallocated.",
                format_bytes(extend_bytes),
                format_bytes(disk.unallocated_bytes)
            ));
        }

        let part = disk.partitions.get_mut(part_idx).ok_or_else(|| format!("Partition {} not found", part_idx))?;
        part.total_bytes += extend_bytes;
        part.free_bytes += extend_bytes;
        disk.unallocated_bytes -= extend_bytes;
        Ok(())
    }

    /// Create a new primary partition in unallocated space.
    pub fn create_partition(
        &mut self,
        disk_idx: usize,
        size_bytes: u64,
        partition_type: PartitionType,
        fs: PartitionFileSystem,
        label: &str,
        letter: Option<char>,
    ) -> Result<u32, String> {
        let disk = self.disks.get_mut(disk_idx).ok_or_else(|| format!("Disk {} not found", disk_idx))?;

        if size_bytes > disk.unallocated_bytes {
            return Err("Requested size exceeds available unallocated space.".to_string());
        }

        let new_idx = disk.partitions.len() as u32 + 1;
        let part = Partition::new(
            new_idx,
            letter,
            label,
            partition_type,
            fs,
            &["Primary Partition"],
            size_bytes,
            0,
            false,
            false,
        );

        disk.unallocated_bytes -= size_bytes;
        disk.partitions.push(part);
        Ok(new_idx)
    }

    /// Delete a partition, returning space to the unallocated pool.
    pub fn delete_partition(&mut self, disk_idx: usize, part_idx: usize) -> Result<(), String> {
        let disk = self.disks.get_mut(disk_idx).ok_or_else(|| format!("Disk {} not found", disk_idx))?;

        if part_idx >= disk.partitions.len() {
            return Err("Partition index out of bounds.".to_string());
        }

        let part = &disk.partitions[part_idx];
        if part.is_boot || part.is_system {
            return Err("Cannot delete active Boot or System partition.".to_string());
        }

        let freed_space = part.total_bytes;
        disk.partitions.remove(part_idx);
        disk.unallocated_bytes += freed_space;
        Ok(())
    }

    /// Render a high-fidelity ASCII graphical partition layout bar representing all drives.
    pub fn render_graphical_view(&self) -> String {
        let mut out = String::new();
        out.push_str("========================================================================================\n");
        out.push_str("  DISK MANAGEMENT (diskmgmt.msc) - GRAPHICAL VOLUME REPRESENTATION\n");
        out.push_str("========================================================================================\n\n");

        for disk in &self.disks {
            out.push_str(&format!(
                "┌────────────────────────────┬──────────────────────────────────────────────────────────┐\n\
                 │ Disk {} {:<20} │ Model: {:<49}│\n\
                 │ Basic ({:<8}) {:<8} │ Layout: {:<48}│\n\
                 ├────────────────────────────┴──────────────────────────────────────────────────────────┤\n",
                disk.disk_number,
                disk.status,
                disk.model_name,
                disk.capacity_formatted(),
                disk.bus_type,
                format!("{}", disk.partition_style)
            ));

            out.push_str("│ [LAYOUT]: ");
            for (i, p) in disk.partitions.iter().enumerate() {
                out.push_str(&format!(
                    "| P{}: {} [{}] ({}) {} |",
                    i + 1,
                    p.display_header(),
                    p.file_system,
                    p.capacity_formatted(),
                    p.health_status_string()
                ));
            }
            if disk.unallocated_bytes > 0 {
                out.push_str(&format!("| Unallocated: {} |", format_bytes(disk.unallocated_bytes)));
            }
            out.push_str("\n└───────────────────────────────────────────────────────────────────────────────────────┘\n\n");
        }
        out
    }

    /// Seed realistic commercial disk drives: NVMe SSD 1TB and secondary SATA SSD 512GB.
    fn populate_default_storage(&mut self) {
        // Disk 0: NVMe SSD (Samsung SSD 990 PRO 1TB)
        let disk0 = PhysicalDisk {
            disk_number: 0,
            model_name: "Samsung SSD 990 PRO 1TB NVMe PCIe 4.0".to_string(),
            bus_type: DiskBusType::NVMe,
            partition_style: DiskPartitionStyle::GPT,
            status: DiskStatus::Online,
            total_bytes: 1_000_204_886_016, // ~1000 GB
            unallocated_bytes: 0,
            partitions: vec![
                Partition::new(
                    1,
                    None,
                    "EFI",
                    PartitionType::EfiSystem,
                    PartitionFileSystem::FAT32,
                    &["Healthy", "EFI System Partition"],
                    100 * 1024 * 1024, // 100 MB
                    32 * 1024 * 1024,
                    false,
                    true,
                ),
                Partition::new(
                    2,
                    None,
                    "",
                    PartitionType::Msr,
                    PartitionFileSystem::Raw,
                    &["Healthy", "Reserved"],
                    16 * 1024 * 1024, // 16 MB
                    16 * 1024 * 1024,
                    false,
                    false,
                ),
                Partition::new(
                    3,
                    Some('C'),
                    "Windows",
                    PartitionType::Primary,
                    PartitionFileSystem::NTFS,
                    &["Healthy", "Boot", "Page File", "Crash Dump", "Basic Data Partition"],
                    480_000_000_000, // 480 GB
                    180_000_000_000, // 180 GB used
                    true,
                    false,
                ),
                Partition::new(
                    4,
                    Some('D'),
                    "Data",
                    PartitionType::Primary,
                    PartitionFileSystem::NTFS,
                    &["Healthy", "Basic Data Partition"],
                    519_000_000_000, // 519 GB
                    220_000_000_000, // 220 GB used
                    false,
                    false,
                ),
                Partition::new(
                    5,
                    None,
                    "Recovery",
                    PartitionType::Recovery,
                    PartitionFileSystem::NTFS,
                    &["Healthy", "Recovery Partition"],
                    1024 * 1024 * 1024, // 1 GB
                    650 * 1024 * 1024,
                    false,
                    false,
                ),
            ],
        };

        // Disk 1: SATA SSD (Kingston KC600 512GB)
        let disk1 = PhysicalDisk {
            disk_number: 1,
            model_name: "Kingston KC600 512GB SATA-III SSD".to_string(),
            bus_type: DiskBusType::SATA,
            partition_style: DiskPartitionStyle::GPT,
            status: DiskStatus::Online,
            total_bytes: 512_110_190_592, // ~512 GB
            unallocated_bytes: 0,
            partitions: vec![
                Partition::new(
                    1,
                    Some('E'),
                    "Backup_Storage",
                    PartitionType::Primary,
                    PartitionFileSystem::NTFS,
                    &["Healthy", "Basic Data Partition"],
                    512_000_000_000,
                    150_000_000_000,
                    false,
                    false,
                ),
            ],
        };

        self.disks.push(disk0);
        self.disks.push(disk1);
    }
}

// ============================================================================
// Section 7: Administrative Tools - Computer Management (compmgmt.msc)
// ============================================================================

/// Tree node in the Computer Management MMC hierarchy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagementTreeNode {
    pub id: String,
    pub title: String,
    pub icon_glyph: String,
    pub children: Vec<ManagementTreeNode>,
}

impl ManagementTreeNode {
    pub fn new(id: &str, title: &str, icon_glyph: &str, children: Vec<ManagementTreeNode>) -> Self {
        Self {
            id: id.to_string(),
            title: title.to_string(),
            icon_glyph: icon_glyph.to_string(),
            children,
        }
    }
}

/// Active panel view in Computer Management.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManagementActiveView {
    RootSummary,
    TaskScheduler,
    EventViewer,
    SharedFolders,
    LocalUsersAndGroups,
    Performance,
    DeviceManager,
    DiskManagement,
    Services,
    WmiControl,
}

/// Complete Computer Management MMC console subsystem (`compmgmt.msc`).
#[derive(Debug, Clone)]
pub struct ComputerManagement {
    pub root_tree: ManagementTreeNode,
    pub active_view: ManagementActiveView,
    pub event_viewer: EventViewer,
    pub services_manager: ServicesManager,
    pub disk_management: DiskManagement,
    pub computer_name: String,
    pub workgroup: String,
    pub os_name: String,
}

impl Default for ComputerManagement {
    fn default() -> Self {
        Self::new()
    }
}

impl ComputerManagement {
    pub fn new() -> Self {
        let tree = ManagementTreeNode::new(
            "root",
            "Computer Management (Local)",
            "🖥️",
            vec![
                ManagementTreeNode::new(
                    "system_tools",
                    "System Tools",
                    "📁",
                    vec![
                        ManagementTreeNode::new("task_scheduler", "Task Scheduler", "⏱️", vec![]),
                        ManagementTreeNode::new("event_viewer", "Event Viewer", "📋", vec![
                            ManagementTreeNode::new("ev_app", "Application", "📄", vec![]),
                            ManagementTreeNode::new("ev_sec", "Security", "🔒", vec![]),
                            ManagementTreeNode::new("ev_sys", "System", "⚙️", vec![]),
                        ]),
                        ManagementTreeNode::new("shared_folders", "Shared Folders", "📂", vec![
                            ManagementTreeNode::new("shares", "Shares", "📤", vec![]),
                            ManagementTreeNode::new("sessions", "Sessions", "👥", vec![]),
                            ManagementTreeNode::new("open_files", "Open Files", "📄", vec![]),
                        ]),
                        ManagementTreeNode::new("local_users", "Local Users and Groups", "👥", vec![
                            ManagementTreeNode::new("users", "Users", "👤", vec![]),
                            ManagementTreeNode::new("groups", "Groups", "👥", vec![]),
                        ]),
                        ManagementTreeNode::new("performance", "Performance", "📊", vec![]),
                        ManagementTreeNode::new("device_manager", "Device Manager", "🔌", vec![]),
                    ],
                ),
                ManagementTreeNode::new(
                    "storage",
                    "Storage",
                    "📁",
                    vec![
                        ManagementTreeNode::new("disk_management", "Disk Management", "💿", vec![]),
                    ],
                ),
                ManagementTreeNode::new(
                    "services_apps",
                    "Services and Applications",
                    "📁",
                    vec![
                        ManagementTreeNode::new("services", "Services", "⚙️", vec![]),
                        ManagementTreeNode::new("wmi_control", "WMI Control", "🔧", vec![]),
                    ],
                ),
            ],
        );

        Self {
            root_tree: tree,
            active_view: ManagementActiveView::RootSummary,
            event_viewer: EventViewer::new(),
            services_manager: ServicesManager::new(),
            disk_management: DiskManagement::new(),
            computer_name: "HYBRIDOS-PC".to_string(),
            workgroup: "WORKGROUP".to_string(),
            os_name: "HybridOS Dual-Personality Commercial Enterprise Edition".to_string(),
        }
    }

    /// Select an active tree node and update the active pane router.
    pub fn select_node(&mut self, node_id: &str) {
        self.active_view = match node_id {
            "root" => ManagementActiveView::RootSummary,
            "task_scheduler" => ManagementActiveView::TaskScheduler,
            "event_viewer" | "ev_app" | "ev_sec" | "ev_sys" => ManagementActiveView::EventViewer,
            "shared_folders" | "shares" | "sessions" | "open_files" => ManagementActiveView::SharedFolders,
            "local_users" | "users" | "groups" => ManagementActiveView::LocalUsersAndGroups,
            "performance" => ManagementActiveView::Performance,
            "device_manager" => ManagementActiveView::DeviceManager,
            "storage" | "disk_management" => ManagementActiveView::DiskManagement,
            "services" => ManagementActiveView::Services,
            "wmi_control" => ManagementActiveView::WmiControl,
            _ => ManagementActiveView::RootSummary,
        };
    }

    /// Render summary view of the active MMC right-hand panel.
    pub fn render_active_panel(&self) -> String {
        match self.active_view {
            ManagementActiveView::RootSummary => format!(
                "===================================================================\n\
                 COMPUTER MANAGEMENT (LOCAL) - SYSTEM SUMMARY\n\
                 ===================================================================\n\
                 Computer Name:   {}\n\
                 Workgroup:       {}\n\
                 Operating System:{}\n\
                 Architecture:    x86_64 Long Mode\n\
                 Kernel Engines:  Hybrid Windows NT 10.0 + Linux 6.x Subsystem\n\
                 Physical Memory: 32,768 MB Installed DDR5\n\
                 Storage Volumes: {} Disks, {} Volumes Online\n\
                 ===================================================================",
                self.computer_name,
                self.workgroup,
                self.os_name,
                self.disk_management.disks.len(),
                self.disk_management.disks.iter().map(|d| d.partitions.len()).sum::<usize>()
            ),
            ManagementActiveView::DiskManagement => self.disk_management.render_graphical_view(),
            ManagementActiveView::Services => {
                let services = self.services_manager.list_services();
                let mut out = format!(
                    "===================================================================\n\
                     SERVICES (LOCAL) - {} Total Services Registered\n\
                     ===================================================================\n\
                     {:<25} {:<12} {:<15} {:<15}\n\
                     -------------------------------------------------------------------\n",
                    services.len(),
                    "Name",
                    "Status",
                    "Startup Type",
                    "Log On As"
                );
                for s in services {
                    out.push_str(&format!(
                        "{:<25} {:<12} {:<15} {:<15}\n",
                        s.display_name,
                        s.status,
                        s.startup_type,
                        format!("{}", s.log_on_as)
                    ));
                }
                out
            }
            ManagementActiveView::EventViewer => {
                let stats = self.event_viewer.channel_statistics();
                let mut out = String::from(
                    "===================================================================\n\
                     EVENT VIEWER (LOCAL) - WINDOWS LOGS SUMMARY\n\
                     ===================================================================\n\
                     Channel           Total       Errors      Warnings    Information\n\
                     -------------------------------------------------------------------\n"
                );
                for (ch, (tot, err, warn, inf)) in stats {
                    out.push_str(&format!(
                        "{:<17} {:<11} {:<11} {:<11} {:<11}\n",
                        ch, tot, err, warn, inf
                    ));
                }
                out
            }
            ManagementActiveView::SharedFolders => {
                "===================================================================\n\
                 SHARED FOLDERS - ACTIVE SHARES (SMB/CIFS)\n\
                 ===================================================================\n\
                 Share Name    Folder Path           Type         Comment\n\
                 -------------------------------------------------------------------\n\
                 ADMIN$        C:\\Windows            Windows      Remote Admin\n\
                 C$            C:\\                   Windows      Default share\n\
                 D$            D:\\                   Windows      Default share\n\
                 IPC$                                Windows      Remote IPC\n\
                 Users         C:\\Users              Windows      Shared User Profiles\n\
                 ==================================================================="
                    .to_string()
            }
            ManagementActiveView::LocalUsersAndGroups => {
                "===================================================================\n\
                 LOCAL USERS AND GROUPS (LOCAL)\n\
                 ===================================================================\n\
                 Users:   Administrator, DefaultAccount, Guest, WDAGUtilityAccount, Vinicius\n\
                 Groups:  Administrators, Backup Operators, Distributed COM Users,\n\
                          Event Log Readers, Hyper-V Administrators, Performance Monitor Users,\n\
                          Remote Desktop Users, Users\n\
                 ==================================================================="
                    .to_string()
            }
            _ => format!("Console pane: {:?}", self.active_view),
        }
    }
}

// ============================================================================
// Section 8: Classic Control Panel Window & Coordinator
// ============================================================================

/// Control Panel Window application model.
#[derive(Debug, Clone)]
pub struct ControlPanelWindow {
    /// Window title bar text
    pub title: String,
    /// Active presentation view mode
    pub view_mode: ControlPanelViewMode,
    /// Full catalog of registered Control Panel applets
    pub applets: Vec<ControlPanelApplet>,
    /// Hierarchical category task metadata
    pub category_metadata: BTreeMap<ControlPanelCategory, CategoryMetadata>,
    /// Navigation history and breadcrumb tracking
    pub history: NavigationHistory,
    /// Current search query string
    pub search_query: String,
    /// Embedded Administrative Tools suite
    pub event_viewer: EventViewer,
    pub services_manager: ServicesManager,
    pub disk_management: DiskManagement,
    pub comp_management: ComputerManagement,
    /// Currently opened modal or embedded tool
    pub active_admin_tool: Option<String>,
}

impl Default for ControlPanelWindow {
    fn default() -> Self {
        Self::new()
    }
}

impl ControlPanelWindow {
    pub fn new() -> Self {
        Self {
            title: "Control Panel".to_string(),
            view_mode: ControlPanelViewMode::Category,
            applets: build_default_applet_catalog(),
            category_metadata: build_default_category_metadata(),
            history: NavigationHistory::new(),
            search_query: String::new(),
            event_viewer: EventViewer::new(),
            services_manager: ServicesManager::new(),
            disk_management: DiskManagement::new(),
            comp_management: ComputerManagement::new(),
            active_admin_tool: None,
        }
    }

    /// Change view mode between Category, LargeIcons, and SmallIcons.
    pub fn set_view_mode(&mut self, mode: ControlPanelViewMode) {
        self.view_mode = mode;
    }

    /// Execute search query in the Control Panel input box.
    pub fn set_search_query(&mut self, query: &str) {
        self.search_query = query.trim().to_string();
        if self.search_query.is_empty() {
            if let NavigationLocation::SearchResults(_) = self.history.current() {
                self.history.go_up();
            }
        } else {
            self.history.navigate(NavigationLocation::SearchResults(self.search_query.clone()));
        }
    }

    /// Clear the search input box.
    pub fn clear_search(&mut self) {
        self.search_query.clear();
        if let NavigationLocation::SearchResults(_) = self.history.current() {
            self.history.go_up();
        }
    }

    /// Retrieve filtered list of applets matching the search query or active category.
    pub fn get_visible_applets(&self) -> Vec<&ControlPanelApplet> {
        if !self.search_query.is_empty() {
            let mut matches: Vec<&ControlPanelApplet> = self.applets
                .iter()
                .filter(|a| a.matches_query(&self.search_query))
                .collect();
            matches.sort_by(|a, b| a.display_name.cmp(&b.display_name));
            return matches;
        }

        match self.history.current() {
            NavigationLocation::Root => {
                let mut all: Vec<&ControlPanelApplet> = self.applets.iter().collect();
                all.sort_by(|a, b| a.display_name.cmp(&b.display_name));
                all
            }
            NavigationLocation::Category(cat) => {
                let mut in_cat: Vec<&ControlPanelApplet> = self.applets
                    .iter()
                    .filter(|a| a.category == *cat)
                    .collect();
                in_cat.sort_by(|a, b| a.display_name.cmp(&b.display_name));
                in_cat
            }
            NavigationLocation::Applet(id) => {
                self.applets.iter().filter(|a| a.id == *id).collect()
            }
            NavigationLocation::AdminTool(_) => {
                self.applets.iter().filter(|a| a.is_admin_tool).collect()
            }
            NavigationLocation::SearchResults(q) => {
                let mut matches: Vec<&ControlPanelApplet> = self.applets
                    .iter()
                    .filter(|a| a.matches_query(q))
                    .collect();
                matches.sort_by(|a, b| a.display_name.cmp(&b.display_name));
                matches
            }
        }
    }

    /// Navigate directly to a category hub.
    pub fn navigate_to_category(&mut self, cat: ControlPanelCategory) {
        self.search_query.clear();
        self.active_admin_tool = None;
        self.history.navigate(NavigationLocation::Category(cat));
    }

    /// Navigate back to root.
    pub fn navigate_to_root(&mut self) {
        self.search_query.clear();
        self.active_admin_tool = None;
        self.history.navigate(NavigationLocation::Root);
    }

    /// Step Back in navigation history.
    pub fn go_back(&mut self) -> bool {
        self.active_admin_tool = None;
        self.history.go_back()
    }

    /// Step Forward in navigation history.
    pub fn go_forward(&mut self) -> bool {
        self.active_admin_tool = None;
        self.history.go_forward()
    }

    /// Step Up in navigation hierarchy.
    pub fn go_up(&mut self) -> bool {
        self.active_admin_tool = None;
        self.history.go_up()
    }

    /// Launch or open an applet by id or command.
    pub fn open_applet(&mut self, id_or_command: &str) -> Result<String, String> {
        let applet = self.applets
            .iter()
            .find(|a| a.id.eq_ignore_ascii_case(id_or_command) || a.command.eq_ignore_ascii_case(id_or_command))
            .ok_or_else(|| format!("Applet '{}' not recognized", id_or_command))?;

        match applet.id.as_str() {
            "admintools" => {
                self.history.navigate(NavigationLocation::AdminTool("Administrative Tools".to_string()));
                self.active_admin_tool = Some("Administrative Tools".to_string());
                Ok("Navigated to Administrative Tools".to_string())
            }
            "eventvwr.msc" => {
                self.active_admin_tool = Some("eventvwr.msc".to_string());
                self.comp_management.select_node("event_viewer");
                Ok("Launched Event Viewer (eventvwr.msc)".to_string())
            }
            "services.msc" => {
                self.active_admin_tool = Some("services.msc".to_string());
                self.comp_management.select_node("services");
                Ok("Launched Services (services.msc)".to_string())
            }
            "diskmgmt.msc" => {
                self.active_admin_tool = Some("diskmgmt.msc".to_string());
                self.comp_management.select_node("disk_management");
                Ok("Launched Disk Management (diskmgmt.msc)".to_string())
            }
            "compmgmt.msc" => {
                self.active_admin_tool = Some("compmgmt.msc".to_string());
                self.comp_management.select_node("root");
                Ok("Launched Computer Management (compmgmt.msc)".to_string())
            }
            _ => {
                self.history.navigate(NavigationLocation::Applet(applet.id.clone()));
                Ok(format!("Opened applet '{}' ({})", applet.display_name, applet.command))
            }
        }
    }

    /// Generates the breadcrumb path string (e.g. "Control Panel > System and Security").
    pub fn breadcrumb_string(&self) -> String {
        self.history
            .breadcrumbs()
            .into_iter()
            .map(|b| b.label)
            .collect::<Vec<String>>()
            .join(" > ")
    }

    /// Renders an authentic ASCII Windows Classic Control Panel window frame.
    pub fn render_ascii_window(&self) -> String {
        let mut out = String::new();
        out.push_str("┌────────────────────────────────────────────────────────────────────────────────────────┐\n");
        out.push_str(&format!(
            "│ [🎛️] {:<50} [-] [□] [X] │\n",
            self.title
        ));
        out.push_str("├────────────────────────────────────────────────────────────────────────────────────────┤\n");

        // Navigation Bar & Search Box
        let back_btn = if self.history.can_go_back() { "[< Back]" } else { "[< ----]" };
        let fwd_btn = if self.history.can_go_forward() { "[Fwd >]" } else { "[---- >]" };
        let search_box = if self.search_query.is_empty() {
            "🔍 Search Control Panel...        "
        } else {
            &self.search_query
        };

        out.push_str(&format!(
            "│ {} {} [^ Up] | Address: {:<38} | [{:<18}] │\n",
            back_btn,
            fwd_btn,
            truncate_str(&self.breadcrumb_string(), 38),
            truncate_str(search_box, 18)
        ));
        out.push_str("├────────────────────────────────────────────────────────────────────────────────────────┤\n");

        // Toolbar: View by selector
        out.push_str(&format!(
            "│ Organize ▼   |                                              View by: [{:<12} ▼] │\n",
            self.view_mode
        ));
        out.push_str("├────────────────────────────────────────────────────────────────────────────────────────┤\n");

        // Content Area
        if !self.search_query.is_empty() {
            let matches = self.get_visible_applets();
            out.push_str(&format!(
                "│ Search Results for \"{}\" ({} item(s) found):\n",
                self.search_query,
                matches.len()
            ));
            for a in matches.iter().take(10) {
                out.push_str(&format!(
                    "│   {} {:<30} - {:<45}\n",
                    a.icon_glyph,
                    truncate_str(&a.display_name, 30),
                    truncate_str(&a.description, 45)
                ));
            }
        } else if self.view_mode == ControlPanelViewMode::Category && *self.history.current() == NavigationLocation::Root {
            out.push_str("│ Adjust your computer's settings\n│\n");
            for cat in ControlPanelCategory::all() {
                if let Some(meta) = self.category_metadata.get(cat) {
                    out.push_str(&format!("│   {} {:<28}\n", cat.icon_glyph(), cat.display_name()));
                    for pt in &meta.primary_tasks {
                        out.push_str(&format!("│      • {}\n", pt.title));
                    }
                    out.push_str("│\n");
                }
            }
        } else {
            // Large Icons / Small Icons / Category drill-down
            let items = self.get_visible_applets();
            out.push_str(&format!("│ Applet Catalog ({} items):\n│\n", items.len()));
            for chunk in items.chunks(3) {
                let mut line = String::from("│  ");
                for item in chunk {
                    line.push_str(&format!("{} {:<24} ", item.icon_glyph, truncate_str(&item.display_name, 24)));
                }
                line.push_str("\n");
                out.push_str(&line);
            }
        }

        out.push_str("├────────────────────────────────────────────────────────────────────────────────────────┤\n");
        let visible_count = self.get_visible_applets().len();
        out.push_str(&format!(
            "│ Status: {} items | HybridOS Commercial Control Panel                          Ready │\n",
            visible_count
        ));
        out.push_str("└────────────────────────────────────────────────────────────────────────────────────────┘\n");
        out
    }
}

// ============================================================================
// Section 9: Helper Utilities
// ============================================================================

/// Formats byte counts into human-readable strings (KB, MB, GB, TB).
pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    const TB: u64 = GB * 1024;

    if bytes >= TB {
        format!("{:.2} TB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} bytes", bytes)
    }
}

/// Truncates string with ellipsis if exceeding specified length.
fn truncate_str(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        s.to_string()
    } else {
        let mut truncated: String = s.chars().take(max_len.saturating_sub(3)).collect();
        truncated.push_str("...");
        truncated
    }
}

// ============================================================================
// Section 10: Comprehensive Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_view_mode_switching_and_applet_counts() {
        let mut window = ControlPanelWindow::new();
        assert_eq!(window.view_mode, ControlPanelViewMode::Category);
        assert!(!window.applets.is_empty());
        assert!(window.applets.len() >= 35, "Must catalog 35+ classic applets");

        // Verify Category View returns all applets when at Root
        let cat_items = window.get_visible_applets();
        assert_eq!(cat_items.len(), window.applets.len());

        // Switch to Large Icons
        window.set_view_mode(ControlPanelViewMode::LargeIcons);
        assert_eq!(window.view_mode, ControlPanelViewMode::LargeIcons);
        assert_eq!(window.get_visible_applets().len(), window.applets.len());

        // Switch to Small Icons
        window.set_view_mode(ControlPanelViewMode::SmallIcons);
        assert_eq!(window.view_mode, ControlPanelViewMode::SmallIcons);
        assert_eq!(window.get_visible_applets().len(), window.applets.len());
    }

    #[test]
    fn test_search_filtering_and_breadcrumbs() {
        let mut window = ControlPanelWindow::new();

        // Search for "firewall"
        window.set_search_query("firewall");
        let results = window.get_visible_applets();
        assert!(!results.is_empty());
        assert!(results.iter().any(|a| a.id == "firewall.cpl"));
        assert_eq!(
            window.breadcrumb_string(),
            "Control Panel > Search Results for \"firewall\""
        );

        // Search for "disk"
        window.set_search_query("disk");
        let disk_results = window.get_visible_applets();
        assert!(disk_results.iter().any(|a| a.id == "diskmgmt.msc"));

        // Search for "printer"
        window.set_search_query("printer");
        let printer_results = window.get_visible_applets();
        assert!(printer_results.iter().any(|a| a.id == "printers"));

        // Clear search
        window.clear_search();
        assert_eq!(window.breadcrumb_string(), "Control Panel");
    }

    #[test]
    fn test_navigation_history_back_forward_up() {
        let mut window = ControlPanelWindow::new();
        assert!(!window.history.can_go_back());
        assert!(!window.history.can_go_forward());

        // Navigate into System and Security category
        window.navigate_to_category(ControlPanelCategory::SystemAndSecurity);
        assert_eq!(
            window.breadcrumb_string(),
            "Control Panel > System and Security"
        );
        assert!(window.history.can_go_back());
        assert!(!window.history.can_go_forward());

        // Visible applets should now be filtered to System and Security
        let sys_items = window.get_visible_applets();
        for item in sys_items {
            assert_eq!(item.category, ControlPanelCategory::SystemAndSecurity);
        }

        // Navigate Up to root
        let up_res = window.go_up();
        assert!(up_res);
        assert_eq!(window.breadcrumb_string(), "Control Panel");

        // Back should return to System and Security
        let back_res = window.go_back();
        assert!(back_res);
        assert_eq!(
            window.breadcrumb_string(),
            "Control Panel > System and Security"
        );

        // Forward should return to Root
        let fwd_res = window.go_forward();
        assert!(fwd_res);
        assert_eq!(window.breadcrumb_string(), "Control Panel");
    }

    #[test]
    fn test_event_viewer_logging_and_filtering() {
        let mut ev = EventViewer::new();

        // Verify initial seeded events
        let sys_events = ev.get_channel_entries(&EventLogChannel::System);
        assert!(!sys_events.is_empty());
        assert!(sys_events.iter().any(|e| e.event_id == 7036));

        // Log a custom Error event
        let record_id = ev.log_event(
            EventLogChannel::Application,
            EventLevel::Error,
            "VortexEngine",
            999,
            "Graphics Pipeline",
            "HYBRIDOS-PC",
            "SYSTEM",
            "Vulkan swapchain presentation timed out during compositor flip.",
        );
        assert!(record_id >= 1000);

        // Query by error level
        let filter_err = EventFilter {
            levels: Some(vec![EventLevel::Error, EventLevel::Critical]),
            ..Default::default()
        };
        let error_entries = ev.query_events(&filter_err);
        assert!(error_entries.iter().any(|e| e.event_id == 999));

        // Query by Source pattern
        let filter_src = EventFilter {
            source_pattern: Some("VortexEngine".to_string()),
            ..Default::default()
        };
        let src_entries = ev.query_events(&filter_src);
        assert_eq!(src_entries.len(), 1);
        assert_eq!(src_entries[0].event_id, 999);

        // Test XML export
        let xml = ev.export_channel_xml(&EventLogChannel::Application);
        assert!(xml.contains("<Provider Name=\"VortexEngine\"/>"));
        assert!(xml.contains("<EventID>999</EventID>"));

        // Clear channel
        let cleared = ev.clear_channel(&EventLogChannel::Application);
        assert!(cleared >= 1);
        assert_eq!(ev.get_channel_entries(&EventLogChannel::Application).len(), 0);
    }

    #[test]
    fn test_services_lifecycle_and_dependencies() {
        let mut sm = ServicesManager::new();

        // Verify Spooler is initially running
        let spooler = sm.get_service("Spooler").expect("Spooler must exist");
        assert_eq!(spooler.status, ServiceStatus::Running);
        assert!(spooler.dependencies.contains(&"RpcSs".to_string()));

        // Test pause & resume
        assert!(spooler.can_pause);
        sm.pause_service("Spooler").expect("Pause should succeed");
        assert_eq!(sm.get_service("Spooler").unwrap().status, ServiceStatus::Paused);

        sm.resume_service("Spooler").expect("Resume should succeed");
        assert_eq!(sm.get_service("Spooler").unwrap().status, ServiceStatus::Running);

        // Test stop & start
        sm.stop_service("Spooler").expect("Stop should succeed");
        assert_eq!(sm.get_service("Spooler").unwrap().status, ServiceStatus::Stopped);
        assert!(sm.get_service("Spooler").unwrap().pid.is_none());

        sm.start_service("Spooler").expect("Start should succeed");
        assert_eq!(sm.get_service("Spooler").unwrap().status, ServiceStatus::Running);
        assert!(sm.get_service("Spooler").unwrap().pid.is_some());

        // Test dependency stop prevention: Stopping RpcSs while Spooler or Dhcp is running must fail!
        let stop_rpc_res = sm.stop_service("RpcSs");
        assert!(stop_rpc_res.is_err(), "Stopping RPC with running dependents must fail");

        // Test service properties retrieval
        let props = sm.get_service_properties("Spooler").expect("Properties must exist");
        assert_eq!(props.display_name, "Print Spooler");
        assert_eq!(props.startup_type, StartupType::Automatic);
    }

    #[test]
    fn test_disk_management_partition_layout_inspection() {
        let dm = DiskManagement::new();
        assert_eq!(dm.disks.len(), 2, "Must contain Disk 0 and Disk 1");

        let disk0 = &dm.disks[0];
        assert_eq!(disk0.disk_number, 0);
        assert_eq!(disk0.bus_type, DiskBusType::NVMe);
        assert_eq!(disk0.partition_style, DiskPartitionStyle::GPT);
        assert_eq!(disk0.status, DiskStatus::Online);

        // Inspect Disk 0 Partitions
        assert_eq!(disk0.partitions.len(), 5);
        let efi = &disk0.partitions[0];
        assert_eq!(efi.partition_type, PartitionType::EfiSystem);
        assert_eq!(efi.file_system, PartitionFileSystem::FAT32);
        assert!(efi.is_system);

        let msr = &disk0.partitions[1];
        assert_eq!(msr.partition_type, PartitionType::Msr);

        let os_c = &disk0.partitions[2];
        assert_eq!(os_c.drive_letter, Some('C'));
        assert_eq!(os_c.file_system, PartitionFileSystem::NTFS);
        assert!(os_c.is_boot);
        assert!(os_c.used_percent() > 0.0);
        assert!(os_c.free_percent() > 0.0);

        let data_d = &disk0.partitions[3];
        assert_eq!(data_d.drive_letter, Some('D'));
        assert_eq!(data_d.volume_label, "Data");

        let recovery = &disk0.partitions[4];
        assert_eq!(recovery.partition_type, PartitionType::Recovery);

        // Graphical rendering test
        let graph = dm.render_graphical_view();
        assert!(graph.contains("Samsung SSD 990 PRO"));
        assert!(graph.contains("(C:)"));
        assert!(graph.contains("(D:)"));
    }

    #[test]
    fn test_disk_volume_format_and_drive_letter_reassignment() {
        let mut dm = DiskManagement::new();

        // Format volume D: (Disk 0, Part 3)
        dm.format_volume(0, 3, PartitionFileSystem::ReFS, "FastData", true)
            .expect("Quick format of Data volume should succeed");

        let disk0 = &dm.disks[0];
        assert_eq!(disk0.partitions[3].file_system, PartitionFileSystem::ReFS);
        assert_eq!(disk0.partitions[3].volume_label, "FastData");

        // Change drive letter D: -> X:
        dm.assign_drive_letter(0, 3, Some('X'))
            .expect("Assigning unused drive letter X should succeed");
        assert_eq!(dm.disks[0].partitions[3].drive_letter, Some('X'));

        // Collision check: attempting to assign 'C' to another volume must fail
        let collision_res = dm.assign_drive_letter(0, 3, Some('C'));
        assert!(collision_res.is_err(), "Duplicate drive letter assignment must fail");

        // System protection: formatting EFI partition must fail
        let efi_fmt = dm.format_volume(0, 0, PartitionFileSystem::NTFS, "Corrupt", true);
        assert!(efi_fmt.is_err(), "Formatting EFI partition must be rejected");
    }

    #[test]
    fn test_computer_management_tree_navigation() {
        let mut cm = ComputerManagement::new();
        assert_eq!(cm.active_view, ManagementActiveView::RootSummary);

        // Switch to Event Viewer
        cm.select_node("event_viewer");
        assert_eq!(cm.active_view, ManagementActiveView::EventViewer);
        let ev_panel = cm.render_active_panel();
        assert!(ev_panel.contains("EVENT VIEWER (LOCAL)"));

        // Switch to Disk Management
        cm.select_node("disk_management");
        assert_eq!(cm.active_view, ManagementActiveView::DiskManagement);
        let dm_panel = cm.render_active_panel();
        assert!(dm_panel.contains("DISK MANAGEMENT"));

        // Switch to Services
        cm.select_node("services");
        assert_eq!(cm.active_view, ManagementActiveView::Services);
        let svc_panel = cm.render_active_panel();
        assert!(svc_panel.contains("SERVICES (LOCAL)"));

        // Switch to Shared Folders
        cm.select_node("shared_folders");
        assert_eq!(cm.active_view, ManagementActiveView::SharedFolders);
        let shares_panel = cm.render_active_panel();
        assert!(shares_panel.contains("ADMIN$"));
    }

    #[test]
    fn test_applet_catalog_completeness_and_applet_launch() {
        let mut window = ControlPanelWindow::new();
        let catalog = build_default_applet_catalog();
        assert!(catalog.len() >= 35);

        // Verify distinct IDs
        let mut ids = HashSet::new();
        for applet in &catalog {
            assert!(ids.insert(&applet.id), "Duplicate applet ID: {}", applet.id);
            assert!(!applet.display_name.is_empty());
            assert!(!applet.command.is_empty());
        }

        // Test opening Administrative Tools
        let res_admin = window.open_applet("admintools").expect("Should open admintools");
        assert!(res_admin.contains("Administrative Tools"));

        // Test opening Event Viewer
        let res_ev = window.open_applet("eventvwr.msc").expect("Should open Event Viewer");
        assert!(res_ev.contains("eventvwr.msc"));

        // Test opening Services
        let res_svc = window.open_applet("services.msc").expect("Should open Services");
        assert!(res_svc.contains("services.msc"));

        // Test opening Disk Management
        let res_dm = window.open_applet("diskmgmt.msc").expect("Should open Disk Management");
        assert!(res_dm.contains("diskmgmt.msc"));

        // ASCII render output validation
        let rendered = window.render_ascii_window();
        assert!(rendered.contains("Control Panel"));
        assert!(rendered.contains("View by:"));
    }
}
