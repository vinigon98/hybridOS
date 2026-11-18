//! HybridOS Windows 11 Commercial Settings App - Privacy, Security & Windows Update Subsystems
//! (`ms-settings:windowsdefender`, `ms-settings:privacy`, `ms-settings:windowsupdate`)
//!
//! Conforms to Windows 11 commercial UI specifications, Windows Security Defender Dashboard,
//! App Capabilities & Privacy Matrix, and Windows Update Servicing Stack.
//!
//! 100% Pure, Safe, and Robust Rust. Zero Unsafe Code.

#![deny(unsafe_code)]

#[allow(unused_imports)]
use alloc::string::{String, ToString};
#[allow(unused_imports)]
use alloc::vec::Vec;
#[allow(unused_imports)]
use alloc::boxed::Box;
#[allow(unused_imports)]
use crate::compat::FloatExt;
use std::collections::HashMap;
use std::fmt;

// ============================================================================
// 1. WINDOWS SECURITY (DEFENDER DASHBOARD) SUB-SYSTEM
// ============================================================================

/// Type of antimalware scan triggered by the user or scheduled task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DefenderScanType {
    /// Checks the most critical system locations, memory, and startup items (~2 minutes).
    Quick,
    /// Checks all files, active memory, and running programs on all storage drives.
    Full,
    /// Checks user-specified folder or drive path.
    Custom,
    /// Reboots into a dedicated minimal environment to remediate persistent rootkits.
    Offline,
}

impl fmt::Display for DefenderScanType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Quick => write!(f, "Quick scan"),
            Self::Full => write!(f, "Full scan"),
            Self::Custom => write!(f, "Custom scan"),
            Self::Offline => write!(f, "Microsoft Defender Antivirus (offline scan)"),
        }
    }
}

/// Operational state of the antimalware scanning engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DefenderScanState {
    /// Antimalware engine is idle and monitoring via Real-time Protection.
    Idle,
    /// A scan is actively in progress.
    Scanning {
        scan_type: DefenderScanType,
        progress_percent: u8,
        files_scanned: u64,
        current_file: String,
    },
    /// A scan completed and reported final metrics.
    Completed(DefenderScanResult),
    /// A scan was aborted by the user or power management.
    Cancelled {
        scan_type: DefenderScanType,
        files_scanned: u64,
    },
}

/// Summary metrics returned upon completion of an antimalware scan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefenderScanResult {
    pub scan_type: DefenderScanType,
    pub files_scanned: u64,
    pub threats_found: u32,
    pub duration_seconds: u32,
    pub completion_timestamp: String,
}

/// Threat severity classifications per Microsoft Security Intelligence standards.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ThreatSeverity {
    Low,
    Medium,
    High,
    Severe,
}

impl fmt::Display for ThreatSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "Low"),
            Self::Medium => write!(f, "Medium"),
            Self::High => write!(f, "High"),
            Self::Severe => write!(f, "Severe"),
        }
    }
}

/// Category of malicious or unwanted software.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ThreatCategory {
    Trojan,
    Virus,
    Worm,
    Ransomware,
    Spyware,
    Adware,
    Exploit,
    Backdoor,
    PotentiallyUnwantedApp,
}

impl fmt::Display for ThreatCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Trojan => write!(f, "Trojan"),
            Self::Virus => write!(f, "Virus"),
            Self::Worm => write!(f, "Worm"),
            Self::Ransomware => write!(f, "Ransomware"),
            Self::Spyware => write!(f, "Spyware"),
            Self::Adware => write!(f, "Adware"),
            Self::Exploit => write!(f, "Exploit"),
            Self::Backdoor => write!(f, "Backdoor"),
            Self::PotentiallyUnwantedApp => write!(f, "Potentially Unwanted App (PUA)"),
        }
    }
}

/// Life-cycle status of an identified threat item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ThreatStatus {
    Active,
    Quarantined,
    Removed,
    Allowed,
}

impl fmt::Display for ThreatStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Active => write!(f, "Active"),
            Self::Quarantined => write!(f, "Quarantined"),
            Self::Removed => write!(f, "Removed"),
            Self::Allowed => write!(f, "Allowed"),
        }
    }
}

/// Log record for an identified threat in Windows Defender Protection History.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreatRecord {
    pub id: u64,
    pub name: String,
    pub severity: ThreatSeverity,
    pub category: ThreatCategory,
    pub detection_timestamp: String,
    pub affected_items: Vec<String>,
    pub status: ThreatStatus,
    pub action_taken: String,
}

/// Security intelligence / virus definition state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecurityIntelligenceInfo {
    pub version: String,
    pub last_updated_timestamp: String,
    pub engine_version: String,
    pub is_up_to_date: bool,
}

impl Default for SecurityIntelligenceInfo {
    fn default() -> Self {
        Self {
            version: "1.403.219.0".to_string(),
            last_updated_timestamp: "2026-11-10 08:30:00 UTC".to_string(),
            engine_version: "1.1.23100.2005".to_string(),
            is_up_to_date: true,
        }
    }
}

/// Virus & threat protection subpage settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VirusThreatProtection {
    pub real_time_protection: bool,
    pub cloud_delivered_protection: bool,
    pub automatic_sample_submission: bool,
    pub tamper_protection: bool,
    pub scan_state: DefenderScanState,
    pub last_scan_result: Option<DefenderScanResult>,
    pub intelligence: SecurityIntelligenceInfo,
    pub threat_history: Vec<ThreatRecord>,
    pub quarantine_vault: Vec<ThreatRecord>,
    pub allowed_threats: Vec<ThreatRecord>,
    next_threat_id: u64,
}

impl Default for VirusThreatProtection {
    fn default() -> Self {
        let sample_threat = ThreatRecord {
            id: 1001,
            name: "Trojan:Win32/Wacatac.B!ml".to_string(),
            severity: ThreatSeverity::Severe,
            category: ThreatCategory::Trojan,
            detection_timestamp: "2026-11-09 14:22:10 UTC".to_string(),
            affected_items: vec!["C:\\Users\\Vinicius\\Downloads\\setup_patch.exe".to_string()],
            status: ThreatStatus::Quarantined,
            action_taken: "Quarantined to isolation vault".to_string(),
        };

        Self {
            real_time_protection: true,
            cloud_delivered_protection: true,
            automatic_sample_submission: true,
            tamper_protection: true,
            scan_state: DefenderScanState::Idle,
            last_scan_result: Some(DefenderScanResult {
                scan_type: DefenderScanType::Quick,
                files_scanned: 64120,
                threats_found: 0,
                duration_seconds: 74,
                completion_timestamp: "2026-11-10 07:15:00 UTC".to_string(),
            }),
            intelligence: SecurityIntelligenceInfo::default(),
            threat_history: vec![sample_threat.clone()],
            quarantine_vault: vec![sample_threat],
            allowed_threats: Vec::new(),
            next_threat_id: 1002,
        }
    }
}

impl VirusThreatProtection {
    pub fn new() -> Self {
        Self::default()
    }

    /// Triggers an on-demand scan. Returns Ok(ScanResult) upon completion.
    pub fn trigger_scan(&mut self, scan_type: DefenderScanType) -> Result<DefenderScanResult, String> {
        if let DefenderScanState::Scanning { .. } = self.scan_state {
            return Err("A scan is already actively running. Cancel or wait for completion.".to_string());
        }

        // Simulate scanning progression
        let (files, duration, threats_found) = match scan_type {
            DefenderScanType::Quick => (68_450, 68, 0),
            DefenderScanType::Full => (1_245_800, 1120, 0),
            DefenderScanType::Custom => (14_200, 15, 0),
            DefenderScanType::Offline => (85_000, 300, 0),
        };

        let result = DefenderScanResult {
            scan_type,
            files_scanned: files,
            threats_found,
            duration_seconds: duration,
            completion_timestamp: "2026-11-10 10:45:00 UTC".to_string(),
        };

        self.last_scan_result = Some(result.clone());
        self.scan_state = DefenderScanState::Completed(result.clone());
        Ok(result)
    }

    /// Simulates discovery of a threat during inspection.
    pub fn inject_threat_detection(
        &mut self,
        name: &str,
        severity: ThreatSeverity,
        category: ThreatCategory,
        path: &str,
    ) -> u64 {
        let id = self.next_threat_id;
        self.next_threat_id += 1;

        let record = ThreatRecord {
            id,
            name: name.to_string(),
            severity,
            category,
            detection_timestamp: "2026-11-10 10:46:00 UTC".to_string(),
            affected_items: vec![path.to_string()],
            status: ThreatStatus::Active,
            action_taken: "Detected - Pending user remediation".to_string(),
        };

        self.threat_history.push(record);
        id
    }

    /// Cancels an in-progress scan.
    pub fn cancel_scan(&mut self) -> Result<(), String> {
        match self.scan_state {
            DefenderScanState::Scanning { scan_type, files_scanned, .. } => {
                self.scan_state = DefenderScanState::Cancelled {
                    scan_type,
                    files_scanned,
                };
                Ok(())
            }
            _ => Err("No active scan in progress to cancel.".to_string()),
        }
    }

    /// Moves an active threat into the quarantine isolation vault.
    pub fn quarantine_threat(&mut self, threat_id: u64) -> Result<(), String> {
        if let Some(pos) = self.threat_history.iter().position(|t| t.id == threat_id) {
            let mut threat = self.threat_history.remove(pos);
            threat.status = ThreatStatus::Quarantined;
            threat.action_taken = "Quarantined by administrator".to_string();
            self.quarantine_vault.push(threat.clone());
            self.threat_history.push(threat);
            Ok(())
        } else {
            Err(format!("Threat ID {} not found in protection history", threat_id))
        }
    }

    /// Permanently eliminates a quarantined or active threat.
    pub fn remove_threat(&mut self, threat_id: u64) -> Result<(), String> {
        let mut found = false;
        self.quarantine_vault.retain(|t| {
            if t.id == threat_id {
                found = true;
                false
            } else {
                true
            }
        });

        if let Some(record) = self.threat_history.iter_mut().find(|t| t.id == threat_id) {
            record.status = ThreatStatus::Removed;
            record.action_taken = "File permanently purged from storage".to_string();
            found = true;
        }

        if found {
            Ok(())
        } else {
            Err(format!("Threat ID {} not found", threat_id))
        }
    }

    /// Allows a flagged item (e.g. false positive developer tool).
    pub fn allow_threat(&mut self, threat_id: u64) -> Result<(), String> {
        if let Some(pos) = self.threat_history.iter().position(|t| t.id == threat_id) {
            let mut threat = self.threat_history[pos].clone();
            threat.status = ThreatStatus::Allowed;
            threat.action_taken = "User added exclusion / allowed on device".to_string();
            self.allowed_threats.push(threat.clone());
            self.threat_history[pos] = threat;
            self.quarantine_vault.retain(|t| t.id != threat_id);
            Ok(())
        } else {
            Err(format!("Threat ID {} not found", threat_id))
        }
    }

    /// Checks for and applies updated security intelligence definitions.
    pub fn check_intelligence_update(&mut self) -> String {
        self.intelligence.version = "1.403.250.0".to_string();
        self.intelligence.last_updated_timestamp = "2026-11-10 10:47:00 UTC".to_string();
        self.intelligence.is_up_to_date = true;
        "Security intelligence successfully updated to version 1.403.250.0".to_string()
    }
}

/// Network location profile type for Windows Defender Firewall.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NetworkProfileType {
    Domain,
    Private,
    Public,
}

impl fmt::Display for NetworkProfileType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Domain => write!(f, "Domain network"),
            Self::Private => write!(f, "Private network"),
            Self::Public => write!(f, "Public network"),
        }
    }
}

/// Configuration and telemetry for a single Windows Firewall network profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkFirewallProfile {
    pub profile_type: NetworkProfileType,
    pub is_active: bool,
    pub block_all_incoming: bool,
    pub incoming_connections_allowed: bool,
    pub notifications_enabled: bool,
    pub connected_adapter: Option<String>,
}

/// Rule determining application access through Windows Defender Firewall.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FirewallAppRule {
    pub display_name: String,
    pub binary_path: String,
    pub allow_domain: bool,
    pub allow_private: bool,
    pub allow_public: bool,
}

/// Firewall & network protection subpage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FirewallNetworkProtection {
    pub domain_profile: NetworkFirewallProfile,
    pub private_profile: NetworkFirewallProfile,
    pub public_profile: NetworkFirewallProfile,
    pub allowed_apps: Vec<FirewallAppRule>,
}

impl Default for FirewallNetworkProtection {
    fn default() -> Self {
        Self {
            domain_profile: NetworkFirewallProfile {
                profile_type: NetworkProfileType::Domain,
                is_active: false,
                block_all_incoming: false,
                incoming_connections_allowed: false,
                notifications_enabled: true,
                connected_adapter: None,
            },
            private_profile: NetworkFirewallProfile {
                profile_type: NetworkProfileType::Private,
                is_active: true,
                block_all_incoming: false,
                incoming_connections_allowed: true,
                notifications_enabled: true,
                connected_adapter: Some("Wi-Fi 6E (Intel AX210)".to_string()),
            },
            public_profile: NetworkFirewallProfile {
                profile_type: NetworkProfileType::Public,
                is_active: false,
                block_all_incoming: true,
                incoming_connections_allowed: false,
                notifications_enabled: true,
                connected_adapter: None,
            },
            allowed_apps: vec![
                FirewallAppRule {
                    display_name: "Core Networking".to_string(),
                    binary_path: "C:\\Windows\\System32\\svchost.exe".to_string(),
                    allow_domain: true,
                    allow_private: true,
                    allow_public: true,
                },
                FirewallAppRule {
                    display_name: "Microsoft Edge".to_string(),
                    binary_path: "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe".to_string(),
                    allow_domain: true,
                    allow_private: true,
                    allow_public: true,
                },
                FirewallAppRule {
                    display_name: "Remote Desktop".to_string(),
                    binary_path: "C:\\Windows\\System32\\mstsc.exe".to_string(),
                    allow_domain: true,
                    allow_private: true,
                    allow_public: false,
                },
            ],
        }
    }
}

impl FirewallNetworkProtection {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns a reference to the profile matching the specified type.
    pub fn get_profile(&self, profile_type: NetworkProfileType) -> &NetworkFirewallProfile {
        match profile_type {
            NetworkProfileType::Domain => &self.domain_profile,
            NetworkProfileType::Private => &self.private_profile,
            NetworkProfileType::Public => &self.public_profile,
        }
    }

    /// Returns a mutable reference to the profile matching the specified type.
    pub fn get_profile_mut(&mut self, profile_type: NetworkProfileType) -> &mut NetworkFirewallProfile {
        match profile_type {
            NetworkProfileType::Domain => &mut self.domain_profile,
            NetworkProfileType::Private => &mut self.private_profile,
            NetworkProfileType::Public => &mut self.public_profile,
        }
    }

    /// Toggles the active status of a firewall profile.
    pub fn set_profile_active(&mut self, profile_type: NetworkProfileType, active: bool) {
        self.get_profile_mut(profile_type).is_active = active;
    }

    /// Toggles the 'Block all incoming connections, including those in the list of allowed apps'.
    pub fn set_block_all_incoming(&mut self, profile_type: NetworkProfileType, block: bool) {
        let p = self.get_profile_mut(profile_type);
        p.block_all_incoming = block;
        if block {
            p.incoming_connections_allowed = false;
        }
    }

    /// Restores all firewall profiles and rules to original factory security defaults.
    pub fn restore_defaults(&mut self) {
        *self = Self::default();
    }

    /// Adds or updates an allowed application in the firewall exception table.
    pub fn set_app_rule(&mut self, rule: FirewallAppRule) {
        if let Some(existing) = self.allowed_apps.iter_mut().find(|r| r.binary_path == rule.binary_path) {
            *existing = rule;
        } else {
            self.allowed_apps.push(rule);
        }
    }
}

/// App & Browser control policies (SmartScreen, Exploit Protection).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppBrowserControl {
    /// SmartScreen checks unrecognized apps and files downloaded from the web.
    pub check_apps_and_files: bool,
    /// SmartScreen warns about malicious sites and downloads in Microsoft Edge.
    pub smartscreen_for_edge: bool,
    /// Phishing protection warns users when entering passwords on suspicious sites.
    pub phishing_protection: bool,
    /// Blocks potentially unwanted apps (adware, miners, unwanted bundlers).
    pub pua_blocking: bool,
    pub pua_block_downloads: bool,

    // Exploit Protection system settings
    pub control_flow_guard: bool,
    pub data_execution_prevention: bool,
    pub mandatory_aslr: bool,
    pub bottom_up_aslr: bool,
    pub validate_exception_chains_sehop: bool,
    pub validate_heap_integrity: bool,
}

impl Default for AppBrowserControl {
    fn default() -> Self {
        Self {
            check_apps_and_files: true,
            smartscreen_for_edge: true,
            phishing_protection: true,
            pua_blocking: true,
            pua_block_downloads: true,
            control_flow_guard: true,
            data_execution_prevention: true,
            mandatory_aslr: true,
            bottom_up_aslr: true,
            validate_exception_chains_sehop: true,
            validate_heap_integrity: true,
        }
    }
}

/// Hardware-enforced device security, Core Isolation HVCI, TPM 2.0, Secure Boot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceSecurity {
    /// Hypervisor-Protected Code Integrity (Memory Integrity) preventing code injection into kernel.
    pub memory_integrity_hvci: bool,
    /// Kernel DMA Protection against malicious Thunderbolt / PCIe Direct Memory Access attacks.
    pub kernel_dma_protection: bool,
    /// Local Security Authority (LSA) credential guard protecting LSASS memory.
    pub lsa_protection: bool,

    // Security Processor (TPM 2.0)
    pub tpm_present: bool,
    pub tpm_version: String,
    pub tpm_manufacturer: String,
    pub tpm_manufacturer_version: String,
    pub tpm_ppi_version: String,
    pub tpm_attestation_ready: bool,
    pub tpm_storage_ready: bool,
    pub tpm_is_cleared: bool,

    // Secure Boot
    pub secure_boot_enabled: bool,
    pub uefi_mode_active: bool,
    pub signature_db_enforced: bool,
}

impl Default for DeviceSecurity {
    fn default() -> Self {
        Self {
            memory_integrity_hvci: true,
            kernel_dma_protection: true,
            lsa_protection: true,
            tpm_present: true,
            tpm_version: "2.0".to_string(),
            tpm_manufacturer: "MSFT".to_string(),
            tpm_manufacturer_version: "7.85.4555.0".to_string(),
            tpm_ppi_version: "1.3".to_string(),
            tpm_attestation_ready: true,
            tpm_storage_ready: true,
            tpm_is_cleared: false,
            secure_boot_enabled: true,
            uefi_mode_active: true,
            signature_db_enforced: true,
        }
    }
}

impl DeviceSecurity {
    pub fn new() -> Self {
        Self::default()
    }

    /// Toggles Memory Integrity (HVCI). Returns Err if hardware virtualization is missing.
    pub fn toggle_memory_integrity(&mut self, enable: bool) -> Result<(), String> {
        if !self.uefi_mode_active {
            return Err("Core Isolation Memory Integrity requires UEFI Secure Boot environment.".to_string());
        }
        self.memory_integrity_hvci = enable;
        Ok(())
    }

    /// Triggers clearing of TPM cryptographic state.
    pub fn clear_tpm(&mut self) -> Result<(), String> {
        if !self.tpm_present {
            return Err("No hardware TPM detected to clear.".to_string());
        }
        self.tpm_is_cleared = true;
        self.tpm_attestation_ready = false;
        Ok(())
    }
}

/// Windows Security Dashboard aggregating all 4 defense pillars.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WindowsSecurityDashboard {
    pub virus_threat_protection: VirusThreatProtection,
    pub firewall_protection: FirewallNetworkProtection,
    pub app_browser_control: AppBrowserControl,
    pub device_security: DeviceSecurity,
}

impl WindowsSecurityDashboard {
    pub fn new() -> Self {
        Self::default()
    }

    /// Overall health summary text for the Windows Security status card.
    pub fn get_security_health_status(&self) -> (&'static str, &'static str) {
        if !self.virus_threat_protection.real_time_protection {
            ("Actions recommended", "Real-time protection is turned off. Your device is vulnerable.")
        } else if !self.firewall_protection.private_profile.is_active && !self.firewall_protection.public_profile.is_active {
            ("Actions recommended", "Firewall is turned off. Your network is unprotected.")
        } else if !self.device_security.memory_integrity_hvci {
            ("Check device security", "Memory integrity is off. Your device may be vulnerable.")
        } else {
            ("No actions needed", "Your device is protected with real-time antivirus, firewall, and core isolation.")
        }
    }
}

// ============================================================================
// 2. PRIVACY & APP PERMISSIONS SUB-SYSTEM
// ============================================================================

/// Diagnostic and usage telemetry data collection level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiagnosticDataLevel {
    /// Send only the info about your device, its settings and capabilities, and whether it is performing properly.
    RequiredOnly,
    /// Send all Required diagnostic data, along with info about websites you browse and how you use apps and features.
    Optional,
}

impl fmt::Display for DiagnosticDataLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RequiredOnly => write!(f, "Send required diagnostic data"),
            Self::Optional => write!(f, "Send optional diagnostic data"),
        }
    }
}

/// User feedback collection frequency.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FeedbackFrequency {
    Automatically,
    Always,
    OnceADay,
    OnceAWeek,
    Never,
}

impl fmt::Display for FeedbackFrequency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Automatically => write!(f, "Automatically (Recommended)"),
            Self::Always => write!(f, "Always"),
            Self::OnceADay => write!(f, "Once a day"),
            Self::OnceAWeek => write!(f, "Once a week"),
            Self::Never => write!(f, "Never"),
        }
    }
}

/// SafeSearch filtering level for Windows Search.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SafeSearchLevel {
    /// Filter adult text, images, and videos from web search results.
    Strict,
    /// Filter adult images and videos, but not text.
    Moderate,
    /// Don't filter adult content from search results.
    Off,
}

impl fmt::Display for SafeSearchLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Strict => write!(f, "Strict"),
            Self::Moderate => write!(f, "Moderate"),
            Self::Off => write!(f, "Off"),
        }
    }
}

/// Windows Permissions sub-category (Diagnostics, Search, Activity History, General).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsPermissions {
    pub diagnostic_data_level: DiagnosticDataLevel,
    pub diagnostic_data_viewer_enabled: bool,
    pub feedback_frequency: FeedbackFrequency,

    // Activity history
    pub store_activity_history: bool,
    pub send_activity_history_to_microsoft: bool,

    // Search permissions
    pub safesearch_level: SafeSearchLevel,
    pub cloud_search_microsoft_account: bool,
    pub cloud_search_work_school: bool,
    pub device_search_history: bool,

    // General privacy
    pub let_apps_show_personalized_ads: bool,
    pub let_websites_access_language_list: bool,
    pub let_windows_track_app_launches: bool,
    pub show_suggested_content_in_settings: bool,
}

impl Default for WindowsPermissions {
    fn default() -> Self {
        Self {
            diagnostic_data_level: DiagnosticDataLevel::RequiredOnly,
            diagnostic_data_viewer_enabled: false,
            feedback_frequency: FeedbackFrequency::Automatically,
            store_activity_history: true,
            send_activity_history_to_microsoft: false,
            safesearch_level: SafeSearchLevel::Moderate,
            cloud_search_microsoft_account: true,
            cloud_search_work_school: false,
            device_search_history: true,
            let_apps_show_personalized_ads: false,
            let_websites_access_language_list: true,
            let_windows_track_app_launches: true,
            show_suggested_content_in_settings: false,
        }
    }
}

impl WindowsPermissions {
    pub fn new() -> Self {
        Self::default()
    }

    /// Deletes diagnostic data stored on device and cached on cloud servers.
    pub fn delete_diagnostic_data(&mut self) -> &'static str {
        "Diagnostic data deletion request submitted to Microsoft telemetry servers."
    }

    /// Clears the user's activity history on this device.
    pub fn clear_activity_history(&mut self) -> &'static str {
        "Activity history cleared."
    }

    /// Clears the device Windows Search index history.
    pub fn clear_device_search_history(&mut self) -> &'static str {
        "Device search history cleared."
    }
}

/// App capabilities governed by the Windows App Permissions Matrix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AppCapability {
    Location,
    Camera,
    Microphone,
    Notifications,
    Contacts,
    Calendar,
    PhoneCalls,
    Messaging,
    FileSystem,
    BackgroundApps,
}

impl fmt::Display for AppCapability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Location => write!(f, "Location"),
            Self::Camera => write!(f, "Camera"),
            Self::Microphone => write!(f, "Microphone"),
            Self::Notifications => write!(f, "Notifications"),
            Self::Contacts => write!(f, "Contacts"),
            Self::Calendar => write!(f, "Calendar"),
            Self::PhoneCalls => write!(f, "Phone calls"),
            Self::Messaging => write!(f, "Messaging"),
            Self::FileSystem => write!(f, "File system"),
            Self::BackgroundApps => write!(f, "Background apps"),
        }
    }
}

/// Granular application permission entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppPermissionEntry {
    pub app_id: String,
    pub display_name: String,
    pub is_allowed: bool,
    pub last_accessed_timestamp: Option<String>,
}

/// Single capability group with master switches and per-application list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilitySettings {
    pub capability: AppCapability,
    /// Master device-level killswitch (e.g. "Location access for this device is On/Off").
    pub master_device_access: bool,
    /// Secondary toggle: "Let apps access your [capability]".
    pub let_apps_access: bool,
    /// Per-application permissions table.
    pub app_permissions: HashMap<String, AppPermissionEntry>,
}

impl CapabilitySettings {
    pub fn new(capability: AppCapability) -> Self {
        Self {
            capability,
            master_device_access: true,
            let_apps_access: true,
            app_permissions: HashMap::new(),
        }
    }
}

/// Evaluation result when an application requests access to a protected capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionAccessResult {
    /// Access is permitted.
    Allowed,
    /// Blocked because the master hardware/device switch is turned off globally.
    DeniedGlobally,
    /// Blocked because the "Let apps access" category toggle is disabled.
    DeniedCategoryDisabled,
    /// Blocked because this specific application was revoked by the user.
    DeniedForApp,
    /// App is not registered in the system permissions manifest.
    AppNotFound,
}

impl fmt::Display for PermissionAccessResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Allowed => write!(f, "Allowed"),
            Self::DeniedGlobally => write!(f, "Denied globally by device master switch"),
            Self::DeniedCategoryDisabled => write!(f, "Denied because app access is turned off"),
            Self::DeniedForApp => write!(f, "Denied specifically for this application"),
            Self::AppNotFound => write!(f, "Application not registered in permissions matrix"),
        }
    }
}

/// Audit log record for capability access.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityAccessEvent {
    pub app_id: String,
    pub app_name: String,
    pub capability: AppCapability,
    pub timestamp: String,
    pub outcome: PermissionAccessResult,
}

/// Privacy & App Permissions matrix manager.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivacySettingsManager {
    pub windows_permissions: WindowsPermissions,
    pub capabilities: HashMap<AppCapability, CapabilitySettings>,
    pub access_log: Vec<CapabilityAccessEvent>,
}

impl Default for PrivacySettingsManager {
    fn default() -> Self {
        let mut caps = HashMap::new();

        let all_capabilities = [
            AppCapability::Location,
            AppCapability::Camera,
            AppCapability::Microphone,
            AppCapability::Notifications,
            AppCapability::Contacts,
            AppCapability::Calendar,
            AppCapability::PhoneCalls,
            AppCapability::Messaging,
            AppCapability::FileSystem,
            AppCapability::BackgroundApps,
        ];

        for &cap in &all_capabilities {
            caps.insert(cap, CapabilitySettings::new(cap));
        }

        let mut mgr = Self {
            windows_permissions: WindowsPermissions::default(),
            capabilities: caps,
            access_log: Vec::new(),
        };

        // Seed common default apps
        mgr.register_app(
            "Microsoft.WindowsMaps_8wekyb3d8bbwe",
            "Maps",
            &[AppCapability::Location, AppCapability::BackgroundApps],
        );
        mgr.register_app(
            "Microsoft.WindowsCamera_8wekyb3d8bbwe",
            "Camera",
            &[AppCapability::Camera, AppCapability::Microphone],
        );
        mgr.register_app(
            "Microsoft.WindowsCommunicationsApps_8wekyb3d8bbwe",
            "Mail and Calendar",
            &[AppCapability::Contacts, AppCapability::Calendar, AppCapability::Notifications],
        );
        mgr.register_app(
            "Microsoft.YourPhone_8wekyb3d8bbwe",
            "Phone Link",
            &[AppCapability::PhoneCalls, AppCapability::Messaging, AppCapability::Notifications],
        );

        mgr
    }
}

impl PrivacySettingsManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers an application and its declared capability requirements.
    pub fn register_app(&mut self, app_id: &str, display_name: &str, capabilities: &[AppCapability]) {
        for &cap in capabilities {
            if let Some(setting) = self.capabilities.get_mut(&cap) {
                setting.app_permissions.insert(
                    app_id.to_string(),
                    AppPermissionEntry {
                        app_id: app_id.to_string(),
                        display_name: display_name.to_string(),
                        is_allowed: true,
                        last_accessed_timestamp: None,
                    },
                );
            }
        }
    }

    /// Sets the master global device toggle for a capability.
    pub fn set_master_toggle(&mut self, capability: AppCapability, enabled: bool) {
        if let Some(setting) = self.capabilities.get_mut(&capability) {
            setting.master_device_access = enabled;
        }
    }

    /// Sets the "Let apps access your [capability]" switch.
    pub fn set_let_apps_access(&mut self, capability: AppCapability, enabled: bool) {
        if let Some(setting) = self.capabilities.get_mut(&capability) {
            setting.let_apps_access = enabled;
        }
    }

    /// Toggles access for an individual application.
    pub fn set_app_permission(
        &mut self,
        capability: AppCapability,
        app_id: &str,
        allowed: bool,
    ) -> Result<(), String> {
        if let Some(setting) = self.capabilities.get_mut(&capability) {
            if let Some(app) = setting.app_permissions.get_mut(app_id) {
                app.is_allowed = allowed;
                Ok(())
            } else {
                Err(format!("App ID '{}' not found under {:?}", app_id, capability))
            }
        } else {
            Err(format!("Capability {:?} not supported", capability))
        }
    }

    /// Enforces capability permission check against the multi-tier policy model.
    pub fn check_permission(&mut self, app_id: &str, capability: AppCapability) -> PermissionAccessResult {
        let setting = match self.capabilities.get_mut(&capability) {
            Some(s) => s,
            None => return PermissionAccessResult::AppNotFound,
        };

        if !setting.master_device_access {
            self.access_log.push(CapabilityAccessEvent {
                app_id: app_id.to_string(),
                app_name: app_id.to_string(),
                capability,
                timestamp: "2026-11-10 10:48:00 UTC".to_string(),
                outcome: PermissionAccessResult::DeniedGlobally,
            });
            return PermissionAccessResult::DeniedGlobally;
        }

        if !setting.let_apps_access {
            self.access_log.push(CapabilityAccessEvent {
                app_id: app_id.to_string(),
                app_name: app_id.to_string(),
                capability,
                timestamp: "2026-11-10 10:48:00 UTC".to_string(),
                outcome: PermissionAccessResult::DeniedCategoryDisabled,
            });
            return PermissionAccessResult::DeniedCategoryDisabled;
        }

        let app = match setting.app_permissions.get_mut(app_id) {
            Some(a) => a,
            None => return PermissionAccessResult::AppNotFound,
        };

        let app_name = app.display_name.clone();

        if !app.is_allowed {
            self.access_log.push(CapabilityAccessEvent {
                app_id: app_id.to_string(),
                app_name,
                capability,
                timestamp: "2026-11-10 10:48:00 UTC".to_string(),
                outcome: PermissionAccessResult::DeniedForApp,
            });
            return PermissionAccessResult::DeniedForApp;
        }

        // Grant access and update last-accessed timestamp
        let ts = "2026-11-10 10:48:00 UTC".to_string();
        app.last_accessed_timestamp = Some(ts.clone());

        self.access_log.push(CapabilityAccessEvent {
            app_id: app_id.to_string(),
            app_name,
            capability,
            timestamp: ts,
            outcome: PermissionAccessResult::Allowed,
        });

        PermissionAccessResult::Allowed
    }
}

// ============================================================================
// 3. WINDOWS UPDATE SUB-SYSTEM
// ============================================================================

/// Operational status of the Windows Update client and servicing engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WindowsUpdateState {
    /// Device is up to date with all cumulative patches.
    UpToDate,
    /// Checking Windows Update servers for new packages.
    CheckingForUpdates,
    /// Updates are ready to download or install.
    UpdatesAvailable(Vec<PendingUpdate>),
    /// Active download in progress.
    Downloading { percent: u8, kb_id: String },
    /// Active installation / component-servicing in progress.
    Installing { percent: u8, kb_id: String },
    /// Updates require a system reboot to finalize offline staging.
    RestartPending { packages: Vec<String> },
    /// Update installation temporarily paused by the user.
    Paused { paused_until: String },
    /// Servicing error encountered (e.g. 0x80070002).
    Error { code: String, message: String },
}

/// Package awaiting download, staging, or installation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingUpdate {
    pub kb_id: String,
    pub title: String,
    pub category: UpdateCategory,
    pub size_mb: u32,
    pub is_critical: bool,
}

/// Classification of updates delivered through Windows Update.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UpdateCategory {
    FeatureUpdate,
    QualityUpdate,
    DriverUpdate,
    DefinitionUpdate,
    OtherUpdate,
}

impl fmt::Display for UpdateCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FeatureUpdate => write!(f, "Feature updates"),
            Self::QualityUpdate => write!(f, "Quality updates"),
            Self::DriverUpdate => write!(f, "Driver updates"),
            Self::DefinitionUpdate => write!(f, "Definition updates"),
            Self::OtherUpdate => write!(f, "Other updates"),
        }
    }
}

/// Installation status of an update entry in history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateInstallStatus {
    Successful,
    Failed { error_code: String, error_message: String },
    PendingReboot,
}

impl fmt::Display for UpdateInstallStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Successful => write!(f, "Successfully installed"),
            Self::Failed { error_code, .. } => write!(f, "Failed to install ({})", error_code),
            Self::PendingReboot => write!(f, "Pending restart"),
        }
    }
}

/// Record in the Windows Update History view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateHistoryRecord {
    pub kb_id: String,
    pub title: String,
    pub category: UpdateCategory,
    pub version: String,
    pub install_date: String,
    pub status: UpdateInstallStatus,
    pub support_url: String,
    pub can_uninstall: bool,
}

/// Duration options available in the "Pause updates" dropdown menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PauseDuration {
    OneWeek,
    TwoWeeks,
    ThreeWeeks,
    FourWeeks,
    FiveWeeks,
    CustomDays(u32),
}

impl PauseDuration {
    pub fn days(&self) -> u32 {
        match self {
            Self::OneWeek => 7,
            Self::TwoWeeks => 14,
            Self::ThreeWeeks => 21,
            Self::FourWeeks => 28,
            Self::FiveWeeks => 35,
            Self::CustomDays(d) => *d,
        }
    }

    pub fn display_label(&self) -> &'static str {
        match self {
            Self::OneWeek => "Pause for 1 week",
            Self::TwoWeeks => "Pause for 2 weeks",
            Self::ThreeWeeks => "Pause for 3 weeks",
            Self::FourWeeks => "Pause for 4 weeks",
            Self::FiveWeeks => "Pause for 5 weeks",
            Self::CustomDays(_) => "Custom pause",
        }
    }
}

/// Mode governing how active hours are managed to prevent unexpected reboots.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ActiveHoursMode {
    /// Automatically adjust active hours based on daily usage habits.
    AutomaticallyBasedOnActivity,
    /// Manually configure start and end hours (e.g. 8:00 AM - 5:00 PM).
    ManualRange,
}

/// Active hours configuration preventing restart disruption during work.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveHoursSettings {
    pub mode: ActiveHoursMode,
    /// Start hour in 24-hour format (0..23). Default 8 (8:00 AM).
    pub start_hour: u8,
    /// End hour in 24-hour format (0..23). Default 17 (5:00 PM).
    pub end_hour: u8,
}

impl Default for ActiveHoursSettings {
    fn default() -> Self {
        Self {
            mode: ActiveHoursMode::ManualRange,
            start_hour: 8,
            end_hour: 17,
        }
    }
}

impl ActiveHoursSettings {
    /// Max allowable active hours duration according to Microsoft Windows 11 specification.
    pub const MAX_ACTIVE_HOURS_SPAN: u8 = 18;

    /// Validates and sets manual active hours.
    ///
    /// Rules:
    /// 1. Start and end hours must be between 0 and 23.
    /// 2. Start and end cannot be identical (duration > 0).
    /// 3. The time span cannot exceed 18 hours.
    pub fn set_manual_range(&mut self, start: u8, end: u8) -> Result<(), String> {
        if start > 23 {
            return Err(format!("Start hour {} is invalid (must be 0..23)", start));
        }
        if end > 23 {
            return Err(format!("End hour {} is invalid (must be 0..23)", end));
        }
        if start == end {
            return Err("Start hour and end hour cannot be identical.".to_string());
        }

        let span = Self::calculate_span(start, end);
        if span > Self::MAX_ACTIVE_HOURS_SPAN {
            return Err(format!(
                "Active hours duration cannot exceed {} hours (requested duration: {} hours).",
                Self::MAX_ACTIVE_HOURS_SPAN, span
            ));
        }

        self.mode = ActiveHoursMode::ManualRange;
        self.start_hour = start;
        self.end_hour = end;
        Ok(())
    }

    /// Computes the span in hours between start and end hour, accounting for midnight wrap.
    pub fn calculate_span(start: u8, end: u8) -> u8 {
        if end >= start {
            end - start
        } else {
            (24 - start) + end
        }
    }

    /// Evaluates whether the given hour (0..23) falls within the active hours window.
    pub fn is_within_active_hours(&self, current_hour: u8) -> bool {
        let ch = current_hour % 24;
        if self.end_hour >= self.start_hour {
            // Standard range within same calendar day (e.g. 8 to 17)
            ch >= self.start_hour && ch < self.end_hour
        } else {
            // Span wraps across midnight (e.g. 22:00 to 06:00)
            ch >= self.start_hour || ch < self.end_hour
        }
    }
}

/// Advanced options for Windows Update servicing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsUpdateAdvancedOptions {
    /// Receive updates for other Microsoft products (Office, Visual Studio, etc.)
    pub receive_updates_for_other_ms_products: bool,
    /// Get me up to date (restart as soon as possible, even during active hours, with notice)
    pub get_me_up_to_date_restart_asap: bool,
    /// Download updates over metered connections (extra data charges may apply)
    pub download_over_metered_connections: bool,
    /// Notify me when a restart is required to finish updating
    pub notify_when_restart_required: bool,
    /// Active hours settings
    pub active_hours: ActiveHoursSettings,
}

impl Default for WindowsUpdateAdvancedOptions {
    fn default() -> Self {
        Self {
            receive_updates_for_other_ms_products: true,
            get_me_up_to_date_restart_asap: false,
            download_over_metered_connections: false,
            notify_when_restart_required: true,
            active_hours: ActiveHoursSettings::default(),
        }
    }
}

/// Windows Update management engine and user-interface state machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsUpdateManager {
    pub state: WindowsUpdateState,
    pub last_checked_timestamp: String,
    pub update_history: Vec<UpdateHistoryRecord>,
    pub advanced_options: WindowsUpdateAdvancedOptions,
}

impl Default for WindowsUpdateManager {
    fn default() -> Self {
        Self {
            state: WindowsUpdateState::UpToDate,
            last_checked_timestamp: "2026-11-10 08:00:00 UTC".to_string(),
            update_history: vec![
                UpdateHistoryRecord {
                    kb_id: "KB5044284".to_string(),
                    title: "2026-11 Cumulative Update for Windows 11 Version 24H2 for x64-based Systems".to_string(),
                    category: UpdateCategory::QualityUpdate,
                    version: "10.0.26100.2161".to_string(),
                    install_date: "2026-11-08".to_string(),
                    status: UpdateInstallStatus::Successful,
                    support_url: "https://support.microsoft.com/help/5044284".to_string(),
                    can_uninstall: true,
                },
                UpdateHistoryRecord {
                    kb_id: "KB5043937".to_string(),
                    title: "Windows 11, version 24H2 Feature Update".to_string(),
                    category: UpdateCategory::FeatureUpdate,
                    version: "24H2".to_string(),
                    install_date: "2026-10-15".to_string(),
                    status: UpdateInstallStatus::Successful,
                    support_url: "https://support.microsoft.com/help/5043937".to_string(),
                    can_uninstall: false,
                },
                UpdateHistoryRecord {
                    kb_id: "KB890830".to_string(),
                    title: "Windows Malicious Software Removal Tool x64 - v5.129".to_string(),
                    category: UpdateCategory::DefinitionUpdate,
                    version: "5.129.21440.1".to_string(),
                    install_date: "2026-11-09".to_string(),
                    status: UpdateInstallStatus::Successful,
                    support_url: "https://support.microsoft.com/help/890830".to_string(),
                    can_uninstall: false,
                },
                UpdateHistoryRecord {
                    kb_id: "DRIVER-INTEL-AX210".to_string(),
                    title: "Intel - Net - 23.90.0.4 (Wi-Fi 6E AX210 160MHz)".to_string(),
                    category: UpdateCategory::DriverUpdate,
                    version: "23.90.0.4".to_string(),
                    install_date: "2026-11-05".to_string(),
                    status: UpdateInstallStatus::Successful,
                    support_url: "https://support.microsoft.com/help/driver".to_string(),
                    can_uninstall: true,
                },
            ],
            advanced_options: WindowsUpdateAdvancedOptions::default(),
        }
    }
}

impl WindowsUpdateManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Triggers checking for updates against Microsoft Update servers.
    pub fn check_for_updates(&mut self) -> Result<usize, String> {
        // If updates are paused, resume or return notice
        if let WindowsUpdateState::Paused { ref paused_until } = self.state {
            return Err(format!(
                "Updates are currently paused until {}. Resume updates to check.",
                paused_until
            ));
        }

        self.last_checked_timestamp = "2026-11-10 10:50:00 UTC".to_string();

        // Sample pending update payload simulation
        let pending = vec![
            PendingUpdate {
                kb_id: "KB5045999".to_string(),
                title: "Security Update for Windows 11 (KB5045999)".to_string(),
                category: UpdateCategory::QualityUpdate,
                size_mb: 485,
                is_critical: true,
            },
            PendingUpdate {
                kb_id: "KB2267602".to_string(),
                title: "Security Intelligence Update for Microsoft Defender Antivirus".to_string(),
                category: UpdateCategory::DefinitionUpdate,
                size_mb: 48,
                is_critical: false,
            },
        ];

        let count = pending.len();
        self.state = WindowsUpdateState::UpdatesAvailable(pending);
        Ok(count)
    }

    /// Pauses updates for the specified duration.
    pub fn pause_updates(&mut self, duration: PauseDuration) -> Result<String, String> {
        let days = duration.days();
        let target_date = format!("2026-11-{} 00:00:00 UTC", 10 + (days % 20));
        self.state = WindowsUpdateState::Paused {
            paused_until: target_date.clone(),
        };
        Ok(target_date)
    }

    /// Resumes paused updates immediately.
    pub fn resume_updates(&mut self) {
        self.state = WindowsUpdateState::UpToDate;
        let _ = self.check_for_updates();
    }

    /// Checks if update servicing is currently paused.
    pub fn is_paused(&self) -> bool {
        matches!(self.state, WindowsUpdateState::Paused { .. })
    }

    /// Returns paused target date string if currently paused.
    pub fn get_paused_until(&self) -> Option<&str> {
        match &self.state {
            WindowsUpdateState::Paused { paused_until } => Some(paused_until),
            _ => None,
        }
    }

    /// Filters update history records by category.
    pub fn get_history_by_category(&self, category: UpdateCategory) -> Vec<&UpdateHistoryRecord> {
        self.update_history
            .iter()
            .filter(|r| r.category == category)
            .collect()
    }

    /// Uninstalls a quality or driver update by its KB number.
    pub fn uninstall_update(&mut self, kb_id: &str) -> Result<(), String> {
        if let Some(pos) = self.update_history.iter().position(|r| r.kb_id == kb_id) {
            let record = &self.update_history[pos];
            if !record.can_uninstall {
                return Err(format!("Update {} cannot be uninstalled (system servicing dependency).", kb_id));
            }
            self.update_history.remove(pos);
            Ok(())
        } else {
            Err(format!("Update {} not found in update history.", kb_id))
        }
    }
}

// ============================================================================
// 4. TOP-LEVEL SETTINGS CONTROLLER & URI DISPATCHER
// ============================================================================

/// Active subpage within the Security & Updates category of Windows 11 Settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SecurityUpdateSubpage {
    // Windows Security
    SecurityHome,
    VirusThreatProtection,
    FirewallNetworkProtection,
    AppBrowserControl,
    DeviceSecurity,

    // Privacy & App Permissions
    PrivacyGeneral,
    PrivacyDiagnostics,
    PrivacyActivityHistory,
    PrivacySearch,
    AppCapabilityDetail(AppCapability),

    // Windows Update
    UpdateHome,
    UpdateHistory,
    UpdateAdvancedOptions,
}

/// Commercial Settings App page controller for Privacy, Security & Windows Update.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SecurityUpdateSettingsApp {
    pub windows_security: WindowsSecurityDashboard,
    pub privacy: PrivacySettingsManager,
    pub windows_update: WindowsUpdateManager,
    pub current_subpage: SecurityUpdateSubpage,
    pub search_query: String,
}

impl Default for SecurityUpdateSubpage {
    fn default() -> Self {
        Self::SecurityHome
    }
}

impl SecurityUpdateSettingsApp {
    pub fn new() -> Self {
        Self::default()
    }

    /// Dispatches Windows 11 standard URI protocol handlers (`ms-settings:`).
    ///
    /// Supported URIs:
    /// - `ms-settings:windowsdefender`
    /// - `ms-settings:windowsdefender-virus`
    /// - `ms-settings:windowsdefender-firewall`
    /// - `ms-settings:windowsdefender-browser`
    /// - `ms-settings:windowsdefender-devicesecurity`
    /// - `ms-settings:privacy`
    /// - `ms-settings:privacy-feedback`
    /// - `ms-settings:privacy-activityhistory`
    /// - `ms-settings:privacy-search`
    /// - `ms-settings:privacy-location`
    /// - `ms-settings:privacy-webcam`
    /// - `ms-settings:privacy-microphone`
    /// - `ms-settings:privacy-notifications`
    /// - `ms-settings:privacy-contacts`
    /// - `ms-settings:privacy-calendar`
    /// - `ms-settings:privacy-phonecalls`
    /// - `ms-settings:privacy-messaging`
    /// - `ms-settings:privacy-broadfilesystem`
    /// - `ms-settings:privacy-backgroundapps`
    /// - `ms-settings:windowsupdate`
    /// - `ms-settings:windowsupdate-history`
    /// - `ms-settings:windowsupdate-options`
    /// - `ms-settings:windowsupdate-activehours`
    pub fn dispatch_uri(&mut self, uri: &str) -> bool {
        let clean = uri.trim().to_ascii_lowercase();

        match clean.as_str() {
            "ms-settings:windowsdefender" => {
                self.current_subpage = SecurityUpdateSubpage::SecurityHome;
                true
            }
            "ms-settings:windowsdefender-virus" | "ms-settings:windowsdefender-threat" => {
                self.current_subpage = SecurityUpdateSubpage::VirusThreatProtection;
                true
            }
            "ms-settings:windowsdefender-firewall" | "ms-settings:windowsdefender-network" => {
                self.current_subpage = SecurityUpdateSubpage::FirewallNetworkProtection;
                true
            }
            "ms-settings:windowsdefender-browser" | "ms-settings:windowsdefender-appbrowser" => {
                self.current_subpage = SecurityUpdateSubpage::AppBrowserControl;
                true
            }
            "ms-settings:windowsdefender-devicesecurity" | "ms-settings:coreisolation" => {
                self.current_subpage = SecurityUpdateSubpage::DeviceSecurity;
                true
            }
            "ms-settings:privacy" => {
                self.current_subpage = SecurityUpdateSubpage::PrivacyGeneral;
                true
            }
            "ms-settings:privacy-feedback" | "ms-settings:privacy-diagnostics" => {
                self.current_subpage = SecurityUpdateSubpage::PrivacyDiagnostics;
                true
            }
            "ms-settings:privacy-activityhistory" => {
                self.current_subpage = SecurityUpdateSubpage::PrivacyActivityHistory;
                true
            }
            "ms-settings:privacy-search" => {
                self.current_subpage = SecurityUpdateSubpage::PrivacySearch;
                true
            }
            "ms-settings:privacy-location" => {
                self.current_subpage = SecurityUpdateSubpage::AppCapabilityDetail(AppCapability::Location);
                true
            }
            "ms-settings:privacy-webcam" => {
                self.current_subpage = SecurityUpdateSubpage::AppCapabilityDetail(AppCapability::Camera);
                true
            }
            "ms-settings:privacy-microphone" => {
                self.current_subpage = SecurityUpdateSubpage::AppCapabilityDetail(AppCapability::Microphone);
                true
            }
            "ms-settings:privacy-notifications" => {
                self.current_subpage = SecurityUpdateSubpage::AppCapabilityDetail(AppCapability::Notifications);
                true
            }
            "ms-settings:privacy-contacts" => {
                self.current_subpage = SecurityUpdateSubpage::AppCapabilityDetail(AppCapability::Contacts);
                true
            }
            "ms-settings:privacy-calendar" => {
                self.current_subpage = SecurityUpdateSubpage::AppCapabilityDetail(AppCapability::Calendar);
                true
            }
            "ms-settings:privacy-phonecalls" => {
                self.current_subpage = SecurityUpdateSubpage::AppCapabilityDetail(AppCapability::PhoneCalls);
                true
            }
            "ms-settings:privacy-messaging" => {
                self.current_subpage = SecurityUpdateSubpage::AppCapabilityDetail(AppCapability::Messaging);
                true
            }
            "ms-settings:privacy-broadfilesystem" => {
                self.current_subpage = SecurityUpdateSubpage::AppCapabilityDetail(AppCapability::FileSystem);
                true
            }
            "ms-settings:privacy-backgroundapps" => {
                self.current_subpage = SecurityUpdateSubpage::AppCapabilityDetail(AppCapability::BackgroundApps);
                true
            }
            "ms-settings:windowsupdate" => {
                self.current_subpage = SecurityUpdateSubpage::UpdateHome;
                true
            }
            "ms-settings:windowsupdate-history" => {
                self.current_subpage = SecurityUpdateSubpage::UpdateHistory;
                true
            }
            "ms-settings:windowsupdate-options" | "ms-settings:windowsupdate-activehours" => {
                self.current_subpage = SecurityUpdateSubpage::UpdateAdvancedOptions;
                true
            }
            _ => false,
        }
    }

    /// Searches settings indexing keywords and returns matched page paths.
    pub fn search(&self, keyword: &str) -> Vec<(&'static str, &'static str, &'static str)> {
        let q = keyword.trim().to_ascii_lowercase();
        if q.is_empty() {
            return Vec::new();
        }

        let mut results = Vec::new();

        if q.contains("virus") || q.contains("threat") || q.contains("defender") || q.contains("scan") {
            results.push((
                "Virus & threat protection",
                "Windows Security > Virus & threat protection",
                "ms-settings:windowsdefender-virus",
            ));
        }
        if q.contains("firewall") || q.contains("network") || q.contains("port") {
            results.push((
                "Firewall & network protection",
                "Windows Security > Firewall & network protection",
                "ms-settings:windowsdefender-firewall",
            ));
        }
        if q.contains("core isolation") || q.contains("hvci") || q.contains("tpm") || q.contains("secure boot") {
            results.push((
                "Device security (Core isolation & TPM)",
                "Windows Security > Device security",
                "ms-settings:windowsdefender-devicesecurity",
            ));
        }
        if q.contains("location") || q.contains("gps") {
            results.push((
                "Location privacy settings",
                "Privacy & security > Location",
                "ms-settings:privacy-location",
            ));
        }
        if q.contains("camera") || q.contains("webcam") {
            results.push((
                "Camera permissions",
                "Privacy & security > Camera",
                "ms-settings:privacy-webcam",
            ));
        }
        if q.contains("mic") || q.contains("microphone") {
            results.push((
                "Microphone permissions",
                "Privacy & security > Microphone",
                "ms-settings:privacy-microphone",
            ));
        }
        if q.contains("update") || q.contains("patch") || q.contains("kb") {
            results.push((
                "Windows Update",
                "Windows Update",
                "ms-settings:windowsupdate",
            ));
            results.push((
                "Update history",
                "Windows Update > Update history",
                "ms-settings:windowsupdate-history",
            ));
        }
        if q.contains("active hours") || q.contains("restart") {
            results.push((
                "Change active hours",
                "Windows Update > Advanced options > Active hours",
                "ms-settings:windowsupdate-activehours",
            ));
        }
        if q.contains("diagnostic") || q.contains("telemetry") {
            results.push((
                "Diagnostics & feedback",
                "Privacy & security > Diagnostics & feedback",
                "ms-settings:privacy-feedback",
            ));
        }

        results
    }

    /// Renders a Fluent UI ASCII representation of the current view model.
    pub fn render_view(&self) -> String {
        let mut out = String::new();
        out.push_str("================================================================================\n");
        out.push_str("                       WINDOWS 11 COMMERCIAL SETTINGS APP                       \n");
        out.push_str("================================================================================\n");

        match &self.current_subpage {
            SecurityUpdateSubpage::SecurityHome => {
                let (headline, sub) = self.windows_security.get_security_health_status();
                out.push_str(" [ Windows Security ]\n");
                out.push_str(&format!(" Status: {}\n Details: {}\n\n", headline, sub));
                out.push_str(" Protection areas:\n");
                out.push_str(&format!(
                    "  [1] Virus & threat protection ........... [Real-time: {}]\n",
                    if self.windows_security.virus_threat_protection.real_time_protection { "ON" } else { "OFF" }
                ));
                out.push_str(&format!(
                    "  [2] Firewall & network protection ....... [Private: {}, Public: {}]\n",
                    if self.windows_security.firewall_protection.private_profile.is_active { "Active" } else { "Off" },
                    if self.windows_security.firewall_protection.public_profile.is_active { "Active" } else { "Off" }
                ));
                out.push_str(&format!(
                    "  [3] App & browser control ............... [SmartScreen: {}]\n",
                    if self.windows_security.app_browser_control.smartscreen_for_edge { "ON" } else { "OFF" }
                ));
                out.push_str(&format!(
                    "  [4] Device security ..................... [HVCI Memory Integrity: {}, TPM: {}]\n",
                    if self.windows_security.device_security.memory_integrity_hvci { "ON" } else { "OFF" },
                    self.windows_security.device_security.tpm_version
                ));
            }
            SecurityUpdateSubpage::VirusThreatProtection => {
                let v = &self.windows_security.virus_threat_protection;
                out.push_str(" [ Windows Security > Virus & threat protection ]\n");
                out.push_str(&format!("  Real-time protection: {}\n", if v.real_time_protection { "ON" } else { "OFF" }));
                out.push_str(&format!("  Cloud-delivered protection: {}\n", if v.cloud_delivered_protection { "ON" } else { "OFF" }));
                out.push_str(&format!("  Tamper protection: {}\n", if v.tamper_protection { "ON" } else { "OFF" }));
                out.push_str(&format!("  Security intelligence: v{} ({})\n", v.intelligence.version, v.intelligence.last_updated_timestamp));
                if let Some(ref res) = v.last_scan_result {
                    out.push_str(&format!(
                        "  Last scan: {:?} (Files: {}, Threats: {}, Duration: {}s, Completed: {})\n",
                        res.scan_type, res.files_scanned, res.threats_found, res.duration_seconds, res.completion_timestamp
                    ));
                }
            }
            SecurityUpdateSubpage::FirewallNetworkProtection => {
                let f = &self.windows_security.firewall_protection;
                out.push_str(" [ Windows Security > Firewall & network protection ]\n");
                out.push_str(&format!("  Domain network: {}\n", if f.domain_profile.is_active { "Active" } else { "Inactive" }));
                out.push_str(&format!("  Private network: {}\n", if f.private_profile.is_active { "Active" } else { "Inactive" }));
                out.push_str(&format!("  Public network: {}\n", if f.public_profile.is_active { "Active" } else { "Inactive" }));
            }
            SecurityUpdateSubpage::AppBrowserControl => {
                let a = &self.windows_security.app_browser_control;
                out.push_str(" [ Windows Security > App & browser control ]\n");
                out.push_str(&format!("  Check apps and files: {}\n", a.check_apps_and_files));
                out.push_str(&format!("  SmartScreen for Microsoft Edge: {}\n", a.smartscreen_for_edge));
                out.push_str(&format!("  Potentially unwanted app blocking: {}\n", a.pua_blocking));
                out.push_str(&format!("  Exploit protection (CFG: {}, DEP: {}, ASLR: {})\n", a.control_flow_guard, a.data_execution_prevention, a.mandatory_aslr));
            }
            SecurityUpdateSubpage::DeviceSecurity => {
                let d = &self.windows_security.device_security;
                out.push_str(" [ Windows Security > Device security ]\n");
                out.push_str(&format!("  Core isolation (Memory integrity HVCI): {}\n", d.memory_integrity_hvci));
                out.push_str(&format!("  Kernel DMA Protection: {}\n", d.kernel_dma_protection));
                out.push_str(&format!("  Security processor (TPM {}): Manufacturer {}\n", d.tpm_version, d.tpm_manufacturer));
                out.push_str(&format!("  Secure Boot: {}\n", if d.secure_boot_enabled { "Enabled" } else { "Disabled" }));
            }
            SecurityUpdateSubpage::PrivacyGeneral => {
                let p = &self.privacy.windows_permissions;
                out.push_str(" [ Privacy & security > General ]\n");
                out.push_str(&format!("  Personalized ads via Advertising ID: {}\n", p.let_apps_show_personalized_ads));
                out.push_str(&format!("  Let websites access language list: {}\n", p.let_websites_access_language_list));
                out.push_str(&format!("  Let Windows track app launches: {}\n", p.let_windows_track_app_launches));
            }
            SecurityUpdateSubpage::PrivacyDiagnostics => {
                let p = &self.privacy.windows_permissions;
                out.push_str(" [ Privacy & security > Diagnostics & feedback ]\n");
                out.push_str(&format!("  Diagnostic data level: {}\n", p.diagnostic_data_level));
                out.push_str(&format!("  Feedback frequency: {}\n", p.feedback_frequency));
            }
            SecurityUpdateSubpage::PrivacyActivityHistory => {
                let p = &self.privacy.windows_permissions;
                out.push_str(" [ Privacy & security > Activity history ]\n");
                out.push_str(&format!("  Store activity history on this device: {}\n", p.store_activity_history));
                out.push_str(&format!("  Send activity history to Microsoft: {}\n", p.send_activity_history_to_microsoft));
            }
            SecurityUpdateSubpage::PrivacySearch => {
                let p = &self.privacy.windows_permissions;
                out.push_str(" [ Privacy & security > Search permissions ]\n");
                out.push_str(&format!("  SafeSearch level: {}\n", p.safesearch_level));
                out.push_str(&format!("  Cloud search (Microsoft account): {}\n", p.cloud_search_microsoft_account));
                out.push_str(&format!("  Search history on this device: {}\n", p.device_search_history));
            }
            SecurityUpdateSubpage::AppCapabilityDetail(cap) => {
                out.push_str(&format!(" [ Privacy & security > App permissions > {} ]\n", cap));
                if let Some(set) = self.privacy.capabilities.get(cap) {
                    out.push_str(&format!("  Device master access: {}\n", if set.master_device_access { "ON" } else { "OFF" }));
                    out.push_str(&format!("  Let apps access {}: {}\n", cap, if set.let_apps_access { "ON" } else { "OFF" }));
                    out.push_str("  Registered apps:\n");
                    for app in set.app_permissions.values() {
                        out.push_str(&format!(
                            "   - {:<28} Allowed: {:<5} (Last accessed: {:?})\n",
                            app.display_name,
                            app.is_allowed,
                            app.last_accessed_timestamp.as_deref().unwrap_or("Never")
                        ));
                    }
                }
            }
            SecurityUpdateSubpage::UpdateHome => {
                let u = &self.windows_update;
                out.push_str(" [ Windows Update ]\n");
                match &u.state {
                    WindowsUpdateState::UpToDate => out.push_str("  Status: You're up to date\n"),
                    WindowsUpdateState::CheckingForUpdates => out.push_str("  Status: Checking for updates...\n"),
                    WindowsUpdateState::UpdatesAvailable(list) => {
                        out.push_str(&format!("  Status: {} update(s) available for download\n", list.len()));
                        for item in list {
                            out.push_str(&format!("   * {} - {} ({} MB)\n", item.kb_id, item.title, item.size_mb));
                        }
                    }
                    WindowsUpdateState::Downloading { percent, kb_id } => {
                        out.push_str(&format!("  Status: Downloading {} ({}%)\n", kb_id, percent));
                    }
                    WindowsUpdateState::Installing { percent, kb_id } => {
                        out.push_str(&format!("  Status: Installing {} ({}%)\n", kb_id, percent));
                    }
                    WindowsUpdateState::RestartPending { packages } => {
                        out.push_str(&format!("  Status: Restart required for {} update(s)\n", packages.len()));
                    }
                    WindowsUpdateState::Paused { paused_until } => {
                        out.push_str(&format!("  Status: Updates paused until {}\n", paused_until));
                    }
                    WindowsUpdateState::Error { code, message } => {
                        out.push_str(&format!("  Status: Update error {} ({})\n", code, message));
                    }
                }
                out.push_str(&format!("  Last checked: {}\n", u.last_checked_timestamp));
            }
            SecurityUpdateSubpage::UpdateHistory => {
                let u = &self.windows_update;
                out.push_str(" [ Windows Update > Update history ]\n");
                out.push_str("  Installed updates:\n");
                for r in &u.update_history {
                    out.push_str(&format!("   * [{}] {} ({}) - {}\n", r.category, r.kb_id, r.install_date, r.status));
                }
            }
            SecurityUpdateSubpage::UpdateAdvancedOptions => {
                let o = &self.windows_update.advanced_options;
                out.push_str(" [ Windows Update > Advanced options ]\n");
                out.push_str(&format!("  Receive updates for other Microsoft products: {}\n", o.receive_updates_for_other_ms_products));
                out.push_str(&format!("  Get me up to date (restart ASAP): {}\n", o.get_me_up_to_date_restart_asap));
                out.push_str(&format!("  Download updates over metered connections: {}\n", o.download_over_metered_connections));
                out.push_str(&format!("  Notify when restart is required: {}\n", o.notify_when_restart_required));
                out.push_str(&format!(
                    "  Active hours: {:?} (Range: {:02}:00 - {:02}:00, Span: {}h)\n",
                    o.active_hours.mode,
                    o.active_hours.start_hour,
                    o.active_hours.end_hour,
                    ActiveHoursSettings::calculate_span(o.active_hours.start_hour, o.active_hours.end_hour)
                ));
            }
        }

        out.push_str("================================================================================\n");
        out
    }
}

// ============================================================================
// 5. COMPREHENSIVE PRODUCTION-GRADE UNIT TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Test 1: Windows Defender antimalware scan trigger, progression, and threat history.
    #[test]
    fn test_defender_scan_trigger() {
        let mut vtp = VirusThreatProtection::new();

        // Ensure engine is initially idle
        assert_eq!(vtp.scan_state, DefenderScanState::Idle);

        // Trigger Quick Scan
        let result = vtp.trigger_scan(DefenderScanType::Quick).expect("Failed to start quick scan");
        assert_eq!(result.scan_type, DefenderScanType::Quick);
        assert_eq!(result.files_scanned, 68_450);
        assert_eq!(result.threats_found, 0);

        // Scan state should reflect completion
        match vtp.scan_state {
            DefenderScanState::Completed(ref res) => {
                assert_eq!(res.scan_type, DefenderScanType::Quick);
                assert_eq!(res.files_scanned, 68_450);
            }
            _ => panic!("Expected scan state to be Completed"),
        }

        // Test threat injection and quarantine remediation flow
        let threat_id = vtp.inject_threat_detection(
            "Ransom:Win32/WannaCrypt",
            ThreatSeverity::Severe,
            ThreatCategory::Ransomware,
            "C:\\Users\\Public\\ransom.exe",
        );

        assert!(vtp.threat_history.iter().any(|t| t.id == threat_id && t.status == ThreatStatus::Active));

        // Quarantine threat
        assert!(vtp.quarantine_threat(threat_id).is_ok());
        assert!(vtp.quarantine_vault.iter().any(|t| t.id == threat_id));
        assert!(vtp.threat_history.iter().any(|t| t.id == threat_id && t.status == ThreatStatus::Quarantined));

        // Remove threat permanently
        assert!(vtp.remove_threat(threat_id).is_ok());
        assert!(!vtp.quarantine_vault.iter().any(|t| t.id == threat_id));
        assert!(vtp.threat_history.iter().any(|t| t.id == threat_id && t.status == ThreatStatus::Removed));
    }

    /// Test 2: Firewall profile toggles, block incoming connections, and restore defaults.
    #[test]
    fn test_firewall_status_toggle() {
        let mut firewall = FirewallNetworkProtection::new();

        // Initial state: Private profile is active, Public is inactive
        assert!(firewall.get_profile(NetworkProfileType::Private).is_active);
        assert!(!firewall.get_profile(NetworkProfileType::Public).is_active);

        // Toggle Public profile ON
        firewall.set_profile_active(NetworkProfileType::Public, true);
        assert!(firewall.get_profile(NetworkProfileType::Public).is_active);

        // Toggle Private profile OFF
        firewall.set_profile_active(NetworkProfileType::Private, false);
        assert!(!firewall.get_profile(NetworkProfileType::Private).is_active);

        // Enable "Block all incoming connections" on Public profile
        firewall.set_block_all_incoming(NetworkProfileType::Public, true);
        assert!(firewall.get_profile(NetworkProfileType::Public).block_all_incoming);
        assert!(!firewall.get_profile(NetworkProfileType::Public).incoming_connections_allowed);

        // Add a custom allowed app rule
        let custom_rule = FirewallAppRule {
            display_name: "HybridOS Terminal".to_string(),
            binary_path: "C:\\Windows\\System32\\hybridos-term.exe".to_string(),
            allow_domain: true,
            allow_private: true,
            allow_public: false,
        };
        firewall.set_app_rule(custom_rule.clone());
        assert!(firewall.allowed_apps.iter().any(|r| r.display_name == "HybridOS Terminal"));

        // Restore factory defaults and verify reset
        firewall.restore_defaults();
        assert!(firewall.get_profile(NetworkProfileType::Private).is_active);
        assert!(!firewall.get_profile(NetworkProfileType::Public).is_active);
        assert!(!firewall.allowed_apps.iter().any(|r| r.display_name == "HybridOS Terminal"));
    }

    /// Test 3: App permission enforcement check (global master toggle, category switch, per-app toggle).
    #[test]
    fn test_app_permission_enforcement_check() {
        let mut privacy = PrivacySettingsManager::new();
        let maps_id = "Microsoft.WindowsMaps_8wekyb3d8bbwe";

        // Initial state: Everything allowed
        let res = privacy.check_permission(maps_id, AppCapability::Location);
        assert_eq!(res, PermissionAccessResult::Allowed);

        // 3.1: Per-app revocation
        assert!(privacy.set_app_permission(AppCapability::Location, maps_id, false).is_ok());
        let res = privacy.check_permission(maps_id, AppCapability::Location);
        assert_eq!(res, PermissionAccessResult::DeniedForApp);

        // Re-enable per-app
        assert!(privacy.set_app_permission(AppCapability::Location, maps_id, true).is_ok());
        let res = privacy.check_permission(maps_id, AppCapability::Location);
        assert_eq!(res, PermissionAccessResult::Allowed);

        // 3.2: Secondary category toggle ("Let apps access your location" -> OFF)
        privacy.set_let_apps_access(AppCapability::Location, false);
        let res = privacy.check_permission(maps_id, AppCapability::Location);
        assert_eq!(res, PermissionAccessResult::DeniedCategoryDisabled);

        // 3.3: Master device toggle ("Location for this device" -> OFF)
        privacy.set_let_apps_access(AppCapability::Location, true);
        privacy.set_master_toggle(AppCapability::Location, false);
        let res = privacy.check_permission(maps_id, AppCapability::Location);
        assert_eq!(res, PermissionAccessResult::DeniedGlobally);

        // 3.4: Unregistered app query returns AppNotFound
        let res = privacy.check_permission("Unknown.MaliciousApp_xxx", AppCapability::Camera);
        assert_eq!(res, PermissionAccessResult::AppNotFound);

        // Verify audit log tracked all access attempts
        assert!(privacy.access_log.len() >= 4);
    }

    /// Test 4: Windows Update check for updates, timestamp recording, and state transition.
    #[test]
    fn test_windows_update_check_and_timestamp() {
        let mut update_mgr = WindowsUpdateManager::new();
        assert_eq!(update_mgr.state, WindowsUpdateState::UpToDate);

        let initial_timestamp = update_mgr.last_checked_timestamp.clone();

        // Perform check for updates
        let count = update_mgr.check_for_updates().expect("Failed to check for updates");
        assert!(count > 0);

        // Verify timestamp was updated
        assert_ne!(update_mgr.last_checked_timestamp, initial_timestamp);

        // Verify state transitioned to UpdatesAvailable with pending items
        match update_mgr.state {
            WindowsUpdateState::UpdatesAvailable(ref items) => {
                assert_eq!(items.len(), count);
                assert!(items.iter().any(|u| u.kb_id == "KB5045999"));
            }
            _ => panic!("Expected state to be UpdatesAvailable"),
        }
    }

    /// Test 5: Windows Update pause updates dropdown durations and resume mechanism.
    #[test]
    fn test_update_pause_and_resume() {
        let mut update_mgr = WindowsUpdateManager::new();

        assert!(!update_mgr.is_paused());
        assert_eq!(update_mgr.get_paused_until(), None);

        // Pause for 1 week (7 days)
        let pause_date = update_mgr.pause_updates(PauseDuration::OneWeek).expect("Pause failed");
        assert!(update_mgr.is_paused());
        assert_eq!(update_mgr.get_paused_until(), Some(pause_date.as_str()));

        // While paused, check_for_updates should return an error notice
        let check_res = update_mgr.check_for_updates();
        assert!(check_res.is_err());
        assert!(check_res.unwrap_err().contains("paused"));

        // Pause for 5 weeks (35 days)
        let pause_date_5w = update_mgr.pause_updates(PauseDuration::FiveWeeks).expect("Pause failed");
        assert!(update_mgr.is_paused());
        assert_eq!(update_mgr.get_paused_until(), Some(pause_date_5w.as_str()));

        // Resume updates
        update_mgr.resume_updates();
        assert!(!update_mgr.is_paused());
        assert_eq!(update_mgr.get_paused_until(), None);

        // Check for updates works again
        assert!(matches!(update_mgr.state, WindowsUpdateState::UpdatesAvailable(_)));
    }

    /// Test 6: Active hours manual range validation (18-hour max constraint, wrap, boundary checks).
    #[test]
    fn test_active_hours_validation() {
        let mut active_hours = ActiveHoursSettings::default();

        // Valid default: 8:00 AM to 5:00 PM (9 hours span)
        assert_eq!(active_hours.start_hour, 8);
        assert_eq!(active_hours.end_hour, 17);
        assert_eq!(ActiveHoursSettings::calculate_span(8, 17), 9);
        assert!(active_hours.is_within_active_hours(8));
        assert!(active_hours.is_within_active_hours(12));
        assert!(active_hours.is_within_active_hours(16));
        assert!(!active_hours.is_within_active_hours(17)); // End hour is exclusive boundary
        assert!(!active_hours.is_within_active_hours(20));
        assert!(!active_hours.is_within_active_hours(3));

        // Valid 18-hour maximum window (e.g. 6:00 AM to 12:00 AM / 24:00 -> 0:00)
        assert!(active_hours.set_manual_range(6, 0).is_ok()); // 6 to 24 = 18 hours
        assert_eq!(ActiveHoursSettings::calculate_span(6, 0), 18);
        assert!(active_hours.is_within_active_hours(6));
        assert!(active_hours.is_within_active_hours(23));
        assert!(!active_hours.is_within_active_hours(0));

        // Invalid: Window exceeding 18 hours (e.g. 6:00 AM to 2:00 AM next day = 20 hours) -> MUST FAIL
        let overspan_res = active_hours.set_manual_range(6, 2);
        assert!(overspan_res.is_err());
        assert!(overspan_res.unwrap_err().contains("cannot exceed 18 hours"));

        // Invalid: Start == End (0 hours) -> MUST FAIL
        let zero_span = active_hours.set_manual_range(9, 9);
        assert!(zero_span.is_err());

        // Invalid: Hours >= 24 -> MUST FAIL
        assert!(active_hours.set_manual_range(24, 10).is_err());
        assert!(active_hours.set_manual_range(10, 25).is_err());

        // Valid midnight wrapping range: 22:00 to 06:00 (8 hours span)
        assert!(active_hours.set_manual_range(22, 6).is_ok());
        assert_eq!(ActiveHoursSettings::calculate_span(22, 6), 8);
        assert!(active_hours.is_within_active_hours(22));
        assert!(active_hours.is_within_active_hours(23));
        assert!(active_hours.is_within_active_hours(0));
        assert!(active_hours.is_within_active_hours(5));
        assert!(!active_hours.is_within_active_hours(6));
        assert!(!active_hours.is_within_active_hours(14));
    }

    /// Test 7: Device Security (Core Isolation HVCI, TPM 2.0 readiness, Secure Boot state).
    #[test]
    fn test_device_security_hvci_and_tpm() {
        let mut dev_sec = DeviceSecurity::new();

        assert!(dev_sec.memory_integrity_hvci);
        assert!(dev_sec.tpm_present);
        assert_eq!(dev_sec.tpm_version, "2.0");
        assert!(dev_sec.tpm_attestation_ready);
        assert!(dev_sec.secure_boot_enabled);

        // Toggle memory integrity
        assert!(dev_sec.toggle_memory_integrity(false).is_ok());
        assert!(!dev_sec.memory_integrity_hvci);
        assert!(dev_sec.toggle_memory_integrity(true).is_ok());
        assert!(dev_sec.memory_integrity_hvci);

        // Clear TPM
        assert!(dev_sec.clear_tpm().is_ok());
        assert!(dev_sec.tpm_is_cleared);
        assert!(!dev_sec.tpm_attestation_ready);
    }

    /// Test 8: URI Protocol Dispatcher and Keyword Search indexing.
    #[test]
    fn test_uri_dispatch_and_search() {
        let mut app = SecurityUpdateSettingsApp::new();

        // Dispatch Windows Security URIs
        assert!(app.dispatch_uri("ms-settings:windowsdefender"));
        assert_eq!(app.current_subpage, SecurityUpdateSubpage::SecurityHome);

        assert!(app.dispatch_uri("ms-settings:windowsdefender-virus"));
        assert_eq!(app.current_subpage, SecurityUpdateSubpage::VirusThreatProtection);

        assert!(app.dispatch_uri("ms-settings:windowsdefender-firewall"));
        assert_eq!(app.current_subpage, SecurityUpdateSubpage::FirewallNetworkProtection);

        // Dispatch Privacy URIs
        assert!(app.dispatch_uri("ms-settings:privacy-location"));
        assert_eq!(app.current_subpage, SecurityUpdateSubpage::AppCapabilityDetail(AppCapability::Location));

        assert!(app.dispatch_uri("ms-settings:privacy-webcam"));
        assert_eq!(app.current_subpage, SecurityUpdateSubpage::AppCapabilityDetail(AppCapability::Camera));

        // Dispatch Windows Update URIs
        assert!(app.dispatch_uri("ms-settings:windowsupdate"));
        assert_eq!(app.current_subpage, SecurityUpdateSubpage::UpdateHome);

        assert!(app.dispatch_uri("ms-settings:windowsupdate-history"));
        assert_eq!(app.current_subpage, SecurityUpdateSubpage::UpdateHistory);

        // Search queries
        let results = app.search("virus");
        assert!(results.iter().any(|r| r.2 == "ms-settings:windowsdefender-virus"));

        let results = app.search("active hours");
        assert!(results.iter().any(|r| r.2 == "ms-settings:windowsupdate-activehours"));

        // Render view should produce rich Fluent ASCII output without panicking
        let rendered = app.render_view();
        assert!(rendered.contains("WINDOWS 11 COMMERCIAL SETTINGS APP"));
        assert!(rendered.contains("Windows Update"));
    }

    /// Test 9: Update history categorization and update uninstallation.
    #[test]
    fn test_update_history_and_uninstall() {
        let mut update_mgr = WindowsUpdateManager::new();

        let quality_updates = update_mgr.get_history_by_category(UpdateCategory::QualityUpdate);
        assert_eq!(quality_updates.len(), 1);
        assert_eq!(quality_updates[0].kb_id, "KB5044284");

        // Attempt uninstalling an un-uninstallable feature update -> must fail
        assert!(update_mgr.uninstall_update("KB5043937").is_err());

        // Uninstall eligible quality update -> must succeed
        assert!(update_mgr.uninstall_update("KB5044284").is_ok());
        let quality_after = update_mgr.get_history_by_category(UpdateCategory::QualityUpdate);
        assert_eq!(quality_after.len(), 0);
    }
}
