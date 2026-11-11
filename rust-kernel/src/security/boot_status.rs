//! HyAV Boot Status & Last Known Good (LKG) State Machine for HybridOS.
//! Provides persistent boot attempt tracking, fault recovery, and automatic
//! rollback to Last Known Good (LKG) or Safe Mode upon consecutive boot failures.

#[allow(unused_imports)]
use alloc::string::{String, ToString};
#[allow(unused_imports)]
use alloc::vec::Vec;
#[allow(unused_imports)]
use alloc::boxed::Box;
#[allow(unused_imports)]
use crate::compat::FloatExt;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;
use super::sha256::SHA256_DIGEST_LENGTH;

pub const DEFAULT_MAX_BOOT_FAILURES: u32 = 2;

/// Operating boot execution mode decided by the state machine
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootMode {
    /// Normal operating environment with full driver loading and AV protection
    Normal,
    /// Last Known Good Configuration: roll back recent driver/system changes to known good checkpoint
    LastKnownGood,
    /// Safe Mode: minimal core kernel and essential diagnostic drivers only
    SafeMode,
}

impl BootMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            BootMode::Normal => "Normal",
            BootMode::LastKnownGood => "LastKnownGood",
            BootMode::SafeMode => "SafeMode",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "Normal" => Some(BootMode::Normal),
            "LastKnownGood" | "LKG" => Some(BootMode::LastKnownGood),
            "SafeMode" => Some(BootMode::SafeMode),
            _ => None,
        }
    }
}

/// Information about a verified driver stored in the LKG configuration
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LkgDriverRecord {
    pub name: String,
    pub hash_hex: String,
    pub is_critical: bool,
}

/// Persistent HyavBootStatus tracking system boots and fault resilience
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HyavBootStatus {
    /// Total lifetime boot attempts
    pub boot_attempts: u32,
    /// Total lifetime successful boots
    pub successful_boots: u32,
    /// Consecutive failed boots without an intervening success
    pub consecutive_failures: u32,
    /// Failure threshold that triggers fallback to LKG (default: 2)
    pub failure_threshold: u32,
    /// Currently determined boot mode
    pub current_mode: BootMode,
    /// Version / revision of the active Last Known Good checkpoint
    pub lkg_generation: u32,
    /// Verified drivers part of the Last Known Good baseline
    pub lkg_drivers: Vec<LkgDriverRecord>,
    /// Last failure message recorded
    pub last_failure_reason: Option<String>,
    /// Target path for on-disk persistence
    pub persistence_file: Option<String>,
}

impl Default for HyavBootStatus {
    fn default() -> Self {
        Self::new()
    }
}

impl HyavBootStatus {
    /// Initialize a new HyavBootStatus state machine
    pub fn new() -> Self {
        Self {
            boot_attempts: 0,
            successful_boots: 0,
            consecutive_failures: 0,
            failure_threshold: DEFAULT_MAX_BOOT_FAILURES,
            current_mode: BootMode::Normal,
            lkg_generation: 0,
            lkg_drivers: Vec::new(),
            last_failure_reason: None,
            persistence_file: None,
        }
    }

    /// Set file path for automatic state saving
    pub fn with_persistence_file(mut self, path: &str) -> Self {
        self.persistence_file = Some(path.to_string());
        self
    }

    /// Executed at early boot before drivers are initialized.
    /// Increments boot count and consecutive failure count.
    /// If consecutive failures reach or exceed threshold (2), triggers LKG / Safe Mode reversion.
    pub fn on_boot_start(&mut self) -> BootMode {
        self.boot_attempts = self.boot_attempts.saturating_add(1);
        self.consecutive_failures = self.consecutive_failures.saturating_add(1);

        // State Machine Decision Logic:
        // 1 failure  -> Normal (try again)
        // 2 failures -> Reversion to Last Known Good (LKG)
        // 3+ failures -> Reversion to Safe Mode (diagnostic recovery)
        if self.consecutive_failures == self.failure_threshold {
            self.current_mode = BootMode::LastKnownGood;
        } else if self.consecutive_failures > self.failure_threshold {
            self.current_mode = BootMode::SafeMode;
        } else {
            self.current_mode = BootMode::Normal;
        }

        self.save_if_configured();
        self.current_mode
    }

    /// Explicitly record a boot failure (e.g. BSOD, driver crash, watchdog timeout, ELAM violation).
    pub fn record_boot_failure(&mut self, reason: &str) -> BootMode {
        self.last_failure_reason = Some(reason.to_string());
        
        // Check state transitions
        if self.consecutive_failures >= self.failure_threshold + 1 {
            self.current_mode = BootMode::SafeMode;
        } else if self.consecutive_failures >= self.failure_threshold {
            self.current_mode = BootMode::LastKnownGood;
        }

        self.save_if_configured();
        self.current_mode
    }

    /// Called when boot and initialization succeed completely.
    /// Resets consecutive failures to 0 and commits current system as LKG baseline.
    pub fn record_boot_success(&mut self, active_drivers: Option<Vec<LkgDriverRecord>>) {
        self.successful_boots = self.successful_boots.saturating_add(1);
        self.consecutive_failures = 0;
        self.last_failure_reason = None;
        self.current_mode = BootMode::Normal;

        // Commit new LKG checkpoint
        self.lkg_generation = self.lkg_generation.saturating_add(1);
        if let Some(drivers) = active_drivers {
            self.lkg_drivers = drivers;
        }

        self.save_if_configured();
    }

    /// Check if a driver is permitted to load in the current boot mode
    pub fn is_driver_permitted(&self, driver_name: &str, driver_hash_hex: &str) -> bool {
        match self.current_mode {
            BootMode::Normal => true, // Standard mode allows any valid signed driver
            BootMode::LastKnownGood => {
                // In LKG mode: only drivers recorded in the LKG baseline or core critical drivers are loaded
                self.lkg_drivers.iter().any(|d| {
                    d.name.eq_ignore_ascii_case(driver_name) && d.hash_hex.eq_ignore_ascii_case(driver_hash_hex)
                })
            }
            BootMode::SafeMode => {
                // In Safe Mode: only critical system drivers
                self.lkg_drivers.iter().any(|d| {
                    d.name.eq_ignore_ascii_case(driver_name) && d.is_critical
                })
            }
        }
    }

    /// Add a driver to the Last Known Good verified list
    pub fn register_lkg_driver(&mut self, name: &str, hash: &[u8; SHA256_DIGEST_LENGTH], is_critical: bool) {
        let hash_hex = hash.iter().map(|b| format!("{:02x}", b)).collect::<String>();
        self.lkg_drivers.push(LkgDriverRecord {
            name: name.to_string(),
            hash_hex,
            is_critical,
        });
    }

    // ========================================================================
    // Serialization & Persistence (Pure Safe Rust, Zero external dependencies)
    // ========================================================================

    /// Serialize state to a key-value format
    pub fn serialize_to_string(&self) -> String {
        let mut out = String::new();
        out.push_str("[HyavBootStatus]\n");
        out.push_str(&format!("boot_attempts={}\n", self.boot_attempts));
        out.push_str(&format!("successful_boots={}\n", self.successful_boots));
        out.push_str(&format!("consecutive_failures={}\n", self.consecutive_failures));
        out.push_str(&format!("failure_threshold={}\n", self.failure_threshold));
        out.push_str(&format!("current_mode={}\n", self.current_mode.as_str()));
        out.push_str(&format!("lkg_generation={}\n", self.lkg_generation));
        if let Some(ref reason) = self.last_failure_reason {
            out.push_str(&format!("last_failure_reason={}\n", reason));
        }
        out.push_str(&format!("lkg_driver_count={}\n", self.lkg_drivers.len()));
        for (i, drv) in self.lkg_drivers.iter().enumerate() {
            out.push_str(&format!("drv_{}_name={}\n", i, drv.name));
            out.push_str(&format!("drv_{}_hash={}\n", i, drv.hash_hex));
            out.push_str(&format!("drv_{}_crit={}\n", i, drv.is_critical));
        }
        out
    }

    /// Parse serialized state from string
    pub fn deserialize_from_str(s: &str) -> Result<Self, String> {
        let mut status = Self::new();
        let mut driver_count: usize = 0;
        let mut drv_names: std::collections::HashMap<usize, String> = std::collections::HashMap::new();
        let mut drv_hashes: std::collections::HashMap<usize, String> = std::collections::HashMap::new();
        let mut drv_crits: std::collections::HashMap<usize, bool> = std::collections::HashMap::new();

        for line in s.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('[') || line.starts_with('#') {
                continue;
            }
            if let Some((k, v)) = line.split_once('=') {
                let k = k.trim();
                let v = v.trim();
                match k {
                    "boot_attempts" => status.boot_attempts = v.parse().unwrap_or(0),
                    "successful_boots" => status.successful_boots = v.parse().unwrap_or(0),
                    "consecutive_failures" => status.consecutive_failures = v.parse().unwrap_or(0),
                    "failure_threshold" => status.failure_threshold = v.parse().unwrap_or(DEFAULT_MAX_BOOT_FAILURES),
                    "current_mode" => {
                        if let Some(m) = BootMode::from_str(v) {
                            status.current_mode = m;
                        }
                    }
                    "lkg_generation" => status.lkg_generation = v.parse().unwrap_or(0),
                    "last_failure_reason" => status.last_failure_reason = Some(v.to_string()),
                    "lkg_driver_count" => driver_count = v.parse().unwrap_or(0),
                    other => {
                        if let Some(idx_str) = other.strip_prefix("drv_").and_then(|rem| rem.strip_suffix("_name")) {
                            if let Ok(idx) = idx_str.parse::<usize>() {
                                drv_names.insert(idx, v.to_string());
                            }
                        } else if let Some(idx_str) = other.strip_prefix("drv_").and_then(|rem| rem.strip_suffix("_hash")) {
                            if let Ok(idx) = idx_str.parse::<usize>() {
                                drv_hashes.insert(idx, v.to_string());
                            }
                        } else if let Some(idx_str) = other.strip_prefix("drv_").and_then(|rem| rem.strip_suffix("_crit")) {
                            if let Ok(idx) = idx_str.parse::<usize>() {
                                drv_crits.insert(idx, v.parse::<bool>().unwrap_or(false));
                            }
                        }
                    }
                }
            }
        }

        for i in 0..driver_count {
            if let (Some(name), Some(hash)) = (drv_names.get(&i), drv_hashes.get(&i)) {
                let crit = drv_crits.get(&i).copied().unwrap_or(false);
                status.lkg_drivers.push(LkgDriverRecord {
                    name: name.clone(),
                    hash_hex: hash.clone(),
                    is_critical: crit,
                });
            }
        }

        Ok(status)
    }

    /// Save state to file path
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> std::io::Result<()> {
        let content = self.serialize_to_string();
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)?;
        file.write_all(content.as_bytes())?;
        file.flush()?;
        Ok(())
    }

    /// Load state from file path
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> std::io::Result<Self> {
        let mut file = File::open(path)?;
        let mut content = String::new();
        file.read_to_string(&mut content)?;
        Self::deserialize_from_str(&content).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }

    fn save_if_configured(&self) {
        if let Some(ref path) = self.persistence_file {
            let _ = self.save_to_file(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normal_boot_success_flow() {
        let mut status = HyavBootStatus::new();
        assert_eq!(status.current_mode, BootMode::Normal);
        assert_eq!(status.consecutive_failures, 0);

        // Boot 1 starts
        let mode1 = status.on_boot_start();
        assert_eq!(mode1, BootMode::Normal);
        assert_eq!(status.boot_attempts, 1);
        assert_eq!(status.consecutive_failures, 1);

        // Boot 1 succeeds
        status.record_boot_success(None);
        assert_eq!(status.successful_boots, 1);
        assert_eq!(status.consecutive_failures, 0);
        assert_eq!(status.lkg_generation, 1);
        assert_eq!(status.current_mode, BootMode::Normal);
    }

    #[test]
    fn test_single_boot_failure_remains_normal() {
        let mut status = HyavBootStatus::new();

        // Boot 1 starts
        let mode = status.on_boot_start();
        assert_eq!(mode, BootMode::Normal);
        assert_eq!(status.consecutive_failures, 1);

        // Boot 1 fails
        status.record_boot_failure("Driver BSOD: bugcheck 0x7E");
        assert_eq!(status.consecutive_failures, 1);
        // After 1 failure, it does NOT revert yet (threshold is 2)
        assert_eq!(status.current_mode, BootMode::Normal);
    }

    #[test]
    fn test_reversion_to_lkg_on_two_failures() {
        let mut status = HyavBootStatus::new();

        // Boot 1 starts and fails
        status.on_boot_start();
        status.record_boot_failure("Fatal Page Fault in unstable_filter.sys");
        assert_eq!(status.consecutive_failures, 1);
        assert_eq!(status.current_mode, BootMode::Normal);

        // Boot 2 starts -> consecutive_failures becomes 2!
        let mode = status.on_boot_start();
        // MUST revert to LastKnownGood!
        assert_eq!(mode, BootMode::LastKnownGood);
        assert_eq!(status.current_mode, BootMode::LastKnownGood);
        assert_eq!(status.consecutive_failures, 2);
    }

    #[test]
    fn test_reversion_to_safe_mode_on_three_failures() {
        let mut status = HyavBootStatus::new();

        // Failure 1
        status.on_boot_start();
        status.record_boot_failure("Crash 1");

        // Failure 2 -> triggers LKG
        let mode2 = status.on_boot_start();
        assert_eq!(mode2, BootMode::LastKnownGood);
        status.record_boot_failure("Crash 2 under LKG");

        // Failure 3 -> triggers Safe Mode!
        let mode3 = status.on_boot_start();
        assert_eq!(mode3, BootMode::SafeMode);
        assert_eq!(status.current_mode, BootMode::SafeMode);
        assert_eq!(status.consecutive_failures, 3);
    }

    #[test]
    fn test_lkg_recovery_restores_normal() {
        let mut status = HyavBootStatus::new();

        // Two failures -> LKG active
        status.on_boot_start();
        status.record_boot_failure("Crash 1");
        status.on_boot_start();
        assert_eq!(status.current_mode, BootMode::LastKnownGood);

        // System successfully boots under LKG!
        status.record_boot_success(None);
        assert_eq!(status.consecutive_failures, 0);
        assert_eq!(status.current_mode, BootMode::Normal);

        // Next boot starts in Normal mode
        let next_mode = status.on_boot_start();
        assert_eq!(next_mode, BootMode::Normal);
    }

    #[test]
    fn test_lkg_driver_filter_policy() {
        let mut status = HyavBootStatus::new();
        let hash_good = [0x11u8; 32];
        let hash_bad = [0x22u8; 32];
        status.register_lkg_driver("kernel32.sys", &hash_good, true);

        let good_hex = "1111111111111111111111111111111111111111111111111111111111111111";
        let bad_hex = "2222222222222222222222222222222222222222222222222222222222222222";

        // Under Normal mode, all drivers permitted
        status.current_mode = BootMode::Normal;
        assert!(status.is_driver_permitted("kernel32.sys", good_hex));
        assert!(status.is_driver_permitted("random_driver.sys", bad_hex));

        // Under LKG mode, only registered LKG drivers permitted
        status.current_mode = BootMode::LastKnownGood;
        assert!(status.is_driver_permitted("kernel32.sys", good_hex));
        assert!(!status.is_driver_permitted("random_driver.sys", bad_hex));

        // Under Safe Mode, only critical LKG drivers permitted
        status.current_mode = BootMode::SafeMode;
        assert!(status.is_driver_permitted("kernel32.sys", good_hex));
    }

    #[test]
    fn test_serialization_roundtrip() {
        let mut status = HyavBootStatus::new();
        status.boot_attempts = 15;
        status.successful_boots = 13;
        status.consecutive_failures = 2;
        status.current_mode = BootMode::LastKnownGood;
        status.lkg_generation = 4;
        status.last_failure_reason = Some("Kernel Security Check Failure".to_string());
        status.register_lkg_driver("bootmgr.sys", &[0xAAu8; 32], true);

        let serialized = status.serialize_to_string();
        let deserialized = HyavBootStatus::deserialize_from_str(&serialized).expect("Deserialization failed");

        assert_eq!(status.boot_attempts, deserialized.boot_attempts);
        assert_eq!(status.successful_boots, deserialized.successful_boots);
        assert_eq!(status.consecutive_failures, deserialized.consecutive_failures);
        assert_eq!(status.current_mode, deserialized.current_mode);
        assert_eq!(status.lkg_generation, deserialized.lkg_generation);
        assert_eq!(status.last_failure_reason, deserialized.last_failure_reason);
        assert_eq!(status.lkg_drivers.len(), deserialized.lkg_drivers.len());
        assert_eq!(status.lkg_drivers[0], deserialized.lkg_drivers[0]);
    }
}
