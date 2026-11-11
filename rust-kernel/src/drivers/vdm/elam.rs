//! Early Launch Anti-Malware (ELAM) Driver Subsystem for Vortex Driver Model (VDM).
//!
//! Evaluates boot-start drivers in Phase 1 before their DriverEntry routine executes.
//! Enforces driver classification into:
//! - KnownGood: Verified, WHQL-signed, or cryptographic whitelist match.
//! - KnownBad: Malware, known vulnerable drivers (BYOVD), or blacklist match.
//! - UnknownGood: Unsigned or third-party drivers passing heuristic checks.
//! - UnknownBad: Drivers failing heuristic checks or displaying suspicious anomalies.
//!
//! Malicious drivers (KnownBad) are mandatorily blocked before DriverEntry execution.

#[allow(unused_imports)]
use alloc::string::{String, ToString};
#[allow(unused_imports)]
use alloc::vec::Vec;
#[allow(unused_imports)]
use alloc::boxed::Box;
#[allow(unused_imports)]
use crate::compat::FloatExt;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use crate::drivers::vdm::{Irp, WdmDriver, VortexDriverManager};
use crate::types::NtStatus;

/// ELAM Driver Classification categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DriverClassification {
    /// Cryptographically verified, WHQL-certified, or known good signature.
    KnownGood,
    /// Known malware, rootkit, or vulnerable driver (BYOVD exploit list).
    KnownBad,
    /// Third-party or unsigned driver with clean heuristic profile.
    UnknownGood,
    /// Driver exhibiting anomalies, invalid headers, or suspicious characteristics.
    UnknownBad,
}

impl DriverClassification {
    pub fn as_str(&self) -> &'static str {
        match self {
            DriverClassification::KnownGood => "KnownGood",
            DriverClassification::KnownBad => "KnownBad",
            DriverClassification::UnknownGood => "UnknownGood",
            DriverClassification::UnknownBad => "UnknownBad",
        }
    }

    pub fn is_malicious(&self) -> bool {
        matches!(self, DriverClassification::KnownBad)
    }
}

/// Early Launch Anti-Malware Policy for boot driver initialization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElamPolicy {
    /// Only KnownGood drivers are permitted; all others are blocked.
    GoodOnly,
    /// KnownGood and UnknownGood drivers are permitted; KnownBad and UnknownBad are blocked.
    GoodAndUnknownGood,
    /// Mandatory blocking of KnownBad drivers unconditionally before DriverEntry.
    /// UnknownBad drivers are also blocked under standard security policy.
    MandatoryBlockMalicious,
}

/// Boot phase tracking for driver loading.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BootPhase {
    /// Phase 0: Core kernel and early HAL initialization.
    Phase0Core,
    /// Phase 1: ELAM Driver initialization.
    Phase1Elam,
    /// Phase 1: Protected boot-start driver loading (mediated by ELAM).
    Phase1BootDrivers,
    /// Phase 2: Runtime and on-demand device driver loading.
    Phase2RuntimeDrivers,
}

/// Digital signature metadata for driver candidate evaluation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriverSignatureInfo {
    pub is_signed: bool,
    pub signer_name: Option<String>,
    pub root_authority: Option<String>,
    pub is_whql: bool,
}

impl Default for DriverSignatureInfo {
    fn default() -> Self {
        Self {
            is_signed: false,
            signer_name: None,
            root_authority: None,
            is_whql: false,
        }
    }
}

/// Evaluation verdict and audit record emitted by ELAM.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElamDecision {
    pub driver_name: String,
    pub classification: DriverClassification,
    pub permitted: bool,
    pub reason: String,
    pub timestamp_ns: u64,
}

/// Error conditions during ELAM driver evaluation and loading.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ElamError {
    /// Mandatory block: Driver classified as malicious (KnownBad) or blocked before DriverEntry.
    DriverBlockedMalicious {
        name: String,
        classification: DriverClassification,
        reason: String,
    },
    /// Driver blocked due to active ELAM policy restriction.
    DriverBlockedByPolicy {
        name: String,
        classification: DriverClassification,
        policy: ElamPolicy,
    },
    /// ELAM subsystem has not been initialized in Phase 1.
    ElamNotInitialized,
    /// Driver entry routine failed with NTSTATUS error.
    DriverEntryFailed(NtStatus),
}

/// A boot driver candidate attempting to load and execute DriverEntry in Phase 1.
pub struct BootDriverCandidate {
    pub name: String,
    pub image_path: String,
    pub sha256_hash: [u8; 32],
    pub signature: DriverSignatureInfo,
    pub is_critical: bool,
    pub has_suspicious_heuristics: bool,
    /// DriverEntry routine. Invoked ONLY if ELAM grants permission!
    pub driver_entry: Box<dyn Fn(&mut VortexDriverManager) -> NtStatus + Send + Sync>,
}

impl BootDriverCandidate {
    pub fn new<F>(
        name: &str,
        image_path: &str,
        sha256_hash: [u8; 32],
        signature: DriverSignatureInfo,
        driver_entry: F,
    ) -> Self
    where
        F: Fn(&mut VortexDriverManager) -> NtStatus + Send + Sync + 'static,
    {
        Self {
            name: name.to_string(),
            image_path: image_path.to_string(),
            sha256_hash,
            signature,
            is_critical: false,
            has_suspicious_heuristics: false,
            driver_entry: Box::new(driver_entry),
        }
    }

    pub fn with_suspicious_heuristics(mut self, flag: bool) -> Self {
        self.has_suspicious_heuristics = flag;
        self
    }

    pub fn with_critical(mut self, flag: bool) -> Self {
        self.is_critical = flag;
        self
    }
}

pub type ElamCallback = Box<dyn Fn(&ElamDecision) + Send + Sync>;

/// Early Launch Anti-Malware (ELAM) Driver.
///
/// Registered in Phase 1 before any other boot-start drivers.
pub struct ElamDriver {
    name: String,
    policy: Mutex<ElamPolicy>,
    known_good_hashes: Mutex<HashSet<[u8; 32]>>,
    known_bad_hashes: Mutex<HashSet<[u8; 32]>>,
    trusted_signers: Mutex<HashSet<String>>,
    blacklisted_signers: Mutex<HashSet<String>>,
    audit_log: Mutex<Vec<ElamDecision>>,
    blocked_count: Mutex<usize>,
    callbacks: Mutex<Vec<ElamCallback>>,
}

impl ElamDriver {
    /// Creates a new ELAM Driver with the specified policy.
    pub fn new(name: &str, policy: ElamPolicy) -> Self {
        Self {
            name: name.to_string(),
            policy: Mutex::new(policy),
            known_good_hashes: Mutex::new(HashSet::new()),
            known_bad_hashes: Mutex::new(HashSet::new()),
            trusted_signers: Mutex::new(HashSet::new()),
            blacklisted_signers: Mutex::new(HashSet::new()),
            audit_log: Mutex::new(Vec::new()),
            blocked_count: Mutex::new(0),
            callbacks: Mutex::new(Vec::new()),
        }
    }

    /// Creates an ELAM Driver populated with default security rules and root-of-trust signatures.
    pub fn with_default_rules() -> Self {
        let elam = Self::new("hyav_elam", ElamPolicy::MandatoryBlockMalicious);

        // Prepopulate trusted signers
        elam.register_trusted_signer("Microsoft Windows Hardware Compatibility Publisher");
        elam.register_trusted_signer("HybridOS Foundation Secure Boot Authority");
        elam.register_trusted_signer("Red Hat VirtIO Certified Publisher");

        // Prepopulate known malicious signer blacklist (revoked certificates / malware authors)
        elam.register_blacklisted_signer("Stolen Certificates Inc");
        elam.register_blacklisted_signer("Rootkit Author Syndicate");
        elam.register_blacklisted_signer("Revoked Rogue CA");

        // Prepopulate known bad hashes (e.g. vulnerable rootkits / BYOVD drivers like RTCore64, gdrv, etc.)
        let sample_bad_hash: [u8; 32] = [
            0xDE, 0xAD, 0xBE, 0xEF, 0x01, 0x23, 0x45, 0x67,
            0x89, 0xAB, 0xCD, 0xEF, 0xFE, 0xDC, 0xBA, 0x98,
            0x76, 0x54, 0x32, 0x10, 0x00, 0x11, 0x22, 0x33,
            0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB,
        ];
        elam.register_known_bad_hash(sample_bad_hash);

        // Prepopulate known good hash (e.g. verified kernel storage driver)
        let sample_good_hash: [u8; 32] = [
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
            0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10,
            0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18,
            0x19, 0x1A, 0x1B, 0x1C, 0x1D, 0x1E, 0x1F, 0x20,
        ];
        elam.register_known_good_hash(sample_good_hash);

        elam
    }

    pub fn register_known_good_hash(&self, hash: [u8; 32]) {
        self.known_good_hashes.lock().unwrap().insert(hash);
    }

    pub fn register_known_bad_hash(&self, hash: [u8; 32]) {
        self.known_bad_hashes.lock().unwrap().insert(hash);
    }

    pub fn register_trusted_signer(&self, signer: &str) {
        self.trusted_signers.lock().unwrap().insert(signer.to_string());
    }

    pub fn register_blacklisted_signer(&self, signer: &str) {
        self.blacklisted_signers.lock().unwrap().insert(signer.to_string());
    }

    pub fn set_policy(&self, policy: ElamPolicy) {
        *self.policy.lock().unwrap() = policy;
    }

    pub fn policy(&self) -> ElamPolicy {
        *self.policy.lock().unwrap()
    }

    pub fn register_callback<F: Fn(&ElamDecision) + Send + Sync + 'static>(&self, callback: F) {
        self.callbacks.lock().unwrap().push(Box::new(callback));
    }

    pub fn blocked_count(&self) -> usize {
        *self.blocked_count.lock().unwrap()
    }

    pub fn audit_log(&self) -> Vec<ElamDecision> {
        self.audit_log.lock().unwrap().clone()
    }

    /// Evaluates and classifies a driver candidate into one of the four categories:
    /// - KnownGood
    /// - KnownBad
    /// - UnknownGood
    /// - UnknownBad
    pub fn classify(&self, candidate: &BootDriverCandidate) -> DriverClassification {
        let bad_hashes = self.known_bad_hashes.lock().unwrap();
        let blacklisted_signers = self.blacklisted_signers.lock().unwrap();

        // 1. Check known malicious signatures & blacklisted signers -> KnownBad
        if bad_hashes.contains(&candidate.sha256_hash) {
            return DriverClassification::KnownBad;
        }

        if let Some(ref signer) = candidate.signature.signer_name {
            if blacklisted_signers.contains(signer) {
                return DriverClassification::KnownBad;
            }
        }

        // 2. Check known good hashes & trusted WHQL signers -> KnownGood
        let good_hashes = self.known_good_hashes.lock().unwrap();
        let trusted_signers = self.trusted_signers.lock().unwrap();

        if good_hashes.contains(&candidate.sha256_hash) {
            return DriverClassification::KnownGood;
        }

        if candidate.signature.is_signed {
            if candidate.signature.is_whql {
                return DriverClassification::KnownGood;
            }
            if let Some(ref signer) = candidate.signature.signer_name {
                if trusted_signers.contains(signer) {
                    return DriverClassification::KnownGood;
                }
            }
        }

        // 3. Heuristic / anomaly evaluation
        if candidate.has_suspicious_heuristics
            || candidate.image_path.to_lowercase().contains("\\temp\\")
            || candidate.image_path.to_lowercase().contains("/tmp/")
        {
            return DriverClassification::UnknownBad;
        }

        // 4. Default clean third-party driver
        DriverClassification::UnknownGood
    }

    /// Evaluates a driver candidate against the active ELAM policy.
    ///
    /// Malicious drivers (KnownBad) are MANDATORILY blocked.
    /// Returns Ok(ElamDecision) if allowed, or Err(ElamError) if blocked.
    pub fn evaluate_candidate(&self, candidate: &BootDriverCandidate) -> Result<ElamDecision, ElamError> {
        let classification = self.classify(candidate);
        let policy = self.policy();

        let now_ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);

        let (permitted, reason) = match classification {
            DriverClassification::KnownBad => {
                // MANDATORY BLOCK: KnownBad drivers can NEVER execute DriverEntry under any policy!
                (false, "MANDATORY BLOCK: Driver matched malicious signature or blacklisted signer".to_string())
            }
            DriverClassification::UnknownBad => {
                // UnknownBad is blocked under all standard policies
                match policy {
                    ElamPolicy::MandatoryBlockMalicious
                    | ElamPolicy::GoodAndUnknownGood
                    | ElamPolicy::GoodOnly => {
                        (false, "Driver blocked by policy: Suspicious heuristics or anomalous attributes (UnknownBad)".to_string())
                    }
                }
            }
            DriverClassification::UnknownGood => {
                match policy {
                    ElamPolicy::GoodOnly => {
                        (false, "Driver blocked by policy: System policy strictly permits KnownGood drivers only".to_string())
                    }
                    ElamPolicy::GoodAndUnknownGood | ElamPolicy::MandatoryBlockMalicious => {
                        (true, "Driver permitted: Clean heuristic profile (UnknownGood)".to_string())
                    }
                }
            }
            DriverClassification::KnownGood => {
                (true, "Driver permitted: Cryptographically verified trusted driver (KnownGood)".to_string())
            }
        };

        let decision = ElamDecision {
            driver_name: candidate.name.clone(),
            classification,
            permitted,
            reason: reason.clone(),
            timestamp_ns: now_ns,
        };

        // Record decision in audit trail
        self.audit_log.lock().unwrap().push(decision.clone());

        // Notify registered ELAM boot callbacks
        let cbs = self.callbacks.lock().unwrap();
        for cb in cbs.iter() {
            cb(&decision);
        }

        if !permitted {
            let mut count = self.blocked_count.lock().unwrap();
            *count += 1;

            if classification == DriverClassification::KnownBad {
                Err(ElamError::DriverBlockedMalicious {
                    name: candidate.name.clone(),
                    classification,
                    reason,
                })
            } else {
                Err(ElamError::DriverBlockedByPolicy {
                    name: candidate.name.clone(),
                    classification,
                    policy,
                })
            }
        } else {
            Ok(decision)
        }
    }
}

impl WdmDriver for ElamDriver {
    fn name(&self) -> &str {
        &self.name
    }

    fn dispatch(&self, irp: &mut Irp) -> NtStatus {
        if irp.major_function == Irp::IRP_MJ_DEVICE_CONTROL {
            // IOCTL to query ELAM blocked drivers count
            let count = self.blocked_count() as u64;
            let bytes = count.to_le_bytes();
            let len = bytes.len().min(irp.user_buffer.len());
            irp.user_buffer[..len].copy_from_slice(&bytes[..len]);
            irp.bytes_transferred = len;
            NtStatus::SUCCESS
        } else {
            NtStatus::SUCCESS
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[test]
    fn test_elam_driver_initialization_phase1() {
        let elam = Arc::new(ElamDriver::with_default_rules());
        assert_eq!(elam.name(), "hyav_elam");
        assert_eq!(elam.policy(), ElamPolicy::MandatoryBlockMalicious);
        assert_eq!(elam.blocked_count(), 0);

        let mut vdm = VortexDriverManager::new();
        assert_eq!(vdm.boot_phase(), BootPhase::Phase0Core);

        let res = vdm.initialize_phase1_elam(elam.clone());
        assert!(res.is_ok());
        assert_eq!(vdm.boot_phase(), BootPhase::Phase1BootDrivers);
        assert!(vdm.has_driver("hyav_elam"));
    }

    #[test]
    fn test_driver_classification_all_four_categories() {
        let elam = ElamDriver::with_default_rules();

        // 1. KnownGood: Registered good hash
        let good_hash = [
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
            0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10,
            0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18,
            0x19, 0x1A, 0x1B, 0x1C, 0x1D, 0x1E, 0x1F, 0x20,
        ];
        let drv_good = BootDriverCandidate::new(
            "good_driver",
            "\\SystemRoot\\System32\\drivers\\good.sys",
            good_hash,
            DriverSignatureInfo::default(),
            |_| NtStatus::SUCCESS,
        );
        assert_eq!(elam.classify(&drv_good), DriverClassification::KnownGood);

        // 2. KnownBad: Registered malicious hash
        let bad_hash = [
            0xDE, 0xAD, 0xBE, 0xEF, 0x01, 0x23, 0x45, 0x67,
            0x89, 0xAB, 0xCD, 0xEF, 0xFE, 0xDC, 0xBA, 0x98,
            0x76, 0x54, 0x32, 0x10, 0x00, 0x11, 0x22, 0x33,
            0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB,
        ];
        let drv_bad = BootDriverCandidate::new(
            "bad_rootkit",
            "\\SystemRoot\\System32\\drivers\\bad.sys",
            bad_hash,
            DriverSignatureInfo::default(),
            |_| NtStatus::SUCCESS,
        );
        assert_eq!(elam.classify(&drv_bad), DriverClassification::KnownBad);

        // 3. UnknownGood: Clean unsigned driver
        let random_hash = [0x55; 32];
        let drv_clean = BootDriverCandidate::new(
            "custom_clean",
            "\\SystemRoot\\System32\\drivers\\custom.sys",
            random_hash,
            DriverSignatureInfo::default(),
            |_| NtStatus::SUCCESS,
        );
        assert_eq!(elam.classify(&drv_clean), DriverClassification::UnknownGood);

        // 4. UnknownBad: Suspicious heuristics / temp path
        let drv_suspicious = BootDriverCandidate::new(
            "temp_anomaly",
            "C:\\Temp\\suspicious_loader.sys",
            [0x77; 32],
            DriverSignatureInfo::default(),
            |_| NtStatus::SUCCESS,
        ).with_suspicious_heuristics(true);
        assert_eq!(elam.classify(&drv_suspicious), DriverClassification::UnknownBad);
    }

    #[test]
    fn test_mandatory_blocking_malicious_known_bad_before_driver_entry() {
        let elam = Arc::new(ElamDriver::with_default_rules());
        let mut vdm = VortexDriverManager::new();
        vdm.initialize_phase1_elam(elam.clone()).unwrap();

        let entry_executed = Arc::new(AtomicBool::new(false));
        let entry_executed_clone = entry_executed.clone();

        let malicious_hash = [
            0xDE, 0xAD, 0xBE, 0xEF, 0x01, 0x23, 0x45, 0x67,
            0x89, 0xAB, 0xCD, 0xEF, 0xFE, 0xDC, 0xBA, 0x98,
            0x76, 0x54, 0x32, 0x10, 0x00, 0x11, 0x22, 0x33,
            0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB,
        ];

        let malicious_driver = BootDriverCandidate::new(
            "rootkit_rtcore",
            "\\SystemRoot\\System32\\drivers\\rtcore64.sys",
            malicious_hash,
            DriverSignatureInfo::default(),
            move |_| {
                // If this is ever called, security is breached!
                entry_executed_clone.store(true, Ordering::SeqCst);
                NtStatus::SUCCESS
            },
        );

        // Attempt to load driver in Phase 1
        let result = vdm.load_boot_driver(&malicious_driver);

        // Verification:
        // 1. Result must be an error with DriverBlockedMalicious
        match result {
            Err(ElamError::DriverBlockedMalicious { name, classification, .. }) => {
                assert_eq!(name, "rootkit_rtcore");
                assert_eq!(classification, DriverClassification::KnownBad);
            }
            other => panic!("Expected DriverBlockedMalicious, got {:?}", other),
        }

        // 2. CRITICAL: DriverEntry MUST NEVER have been executed!
        assert!(!entry_executed.load(Ordering::SeqCst), "CRITICAL: DriverEntry was called on KnownBad driver!");

        // 3. Driver must be recorded as blocked in VDM and ELAM telemetry
        assert!(vdm.is_driver_blocked("rootkit_rtcore"));
        assert!(!vdm.has_driver("rootkit_rtcore"));
        assert_eq!(elam.blocked_count(), 1);
    }

    #[test]
    fn test_mandatory_blocking_by_blacklisted_signer() {
        let elam = Arc::new(ElamDriver::with_default_rules());
        let mut vdm = VortexDriverManager::new();
        vdm.initialize_phase1_elam(elam.clone()).unwrap();

        let entry_executed = Arc::new(AtomicBool::new(false));
        let entry_clone = entry_executed.clone();

        let rogue_sig = DriverSignatureInfo {
            is_signed: true,
            signer_name: Some("Rootkit Author Syndicate".to_string()),
            root_authority: None,
            is_whql: false,
        };

        let candidate = BootDriverCandidate::new(
            "rogue_hook_drv",
            "\\SystemRoot\\System32\\drivers\\rogue.sys",
            [0x88; 32],
            rogue_sig,
            move |_| {
                entry_clone.store(true, Ordering::SeqCst);
                NtStatus::SUCCESS
            },
        );

        let result = vdm.load_boot_driver(&candidate);
        assert!(matches!(result, Err(ElamError::DriverBlockedMalicious { .. })));
        assert!(!entry_executed.load(Ordering::SeqCst), "DriverEntry executed despite blacklisted signer!");
        assert!(vdm.is_driver_blocked("rogue_hook_drv"));
    }

    #[test]
    fn test_known_good_driver_entry_executed_and_registered() {
        let elam = Arc::new(ElamDriver::with_default_rules());
        let mut vdm = VortexDriverManager::new();
        vdm.initialize_phase1_elam(elam.clone()).unwrap();

        let entry_executed = Arc::new(AtomicBool::new(false));
        let entry_clone = entry_executed.clone();

        let whql_sig = DriverSignatureInfo {
            is_signed: true,
            signer_name: Some("Microsoft Windows Hardware Compatibility Publisher".to_string()),
            root_authority: Some("Microsoft Root CA".to_string()),
            is_whql: true,
        };

        let candidate = BootDriverCandidate::new(
            "trusted_storage",
            "\\SystemRoot\\System32\\drivers\\storport.sys",
            [0x12; 32],
            whql_sig,
            move |_mgr| {
                entry_clone.store(true, Ordering::SeqCst);
                NtStatus::SUCCESS
            },
        );

        let result = vdm.load_boot_driver(&candidate);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), NtStatus::SUCCESS);
        assert!(entry_executed.load(Ordering::SeqCst), "DriverEntry was not executed for KnownGood driver!");
        assert!(!vdm.is_driver_blocked("trusted_storage"));
    }

    #[test]
    fn test_unknown_bad_blocking_under_mandatory_policy() {
        let elam = Arc::new(ElamDriver::with_default_rules());
        let mut vdm = VortexDriverManager::new();
        vdm.initialize_phase1_elam(elam.clone()).unwrap();

        let entry_executed = Arc::new(AtomicBool::new(false));
        let entry_clone = entry_executed.clone();

        let suspicious_candidate = BootDriverCandidate::new(
            "suspicious_packer",
            "\\SystemRoot\\System32\\drivers\\packed.sys",
            [0x99; 32],
            DriverSignatureInfo::default(),
            move |_| {
                entry_clone.store(true, Ordering::SeqCst);
                NtStatus::SUCCESS
            },
        ).with_suspicious_heuristics(true);

        let result = vdm.load_boot_driver(&suspicious_candidate);
        assert!(matches!(result, Err(ElamError::DriverBlockedByPolicy {
            classification: DriverClassification::UnknownBad,
            ..
        })));
        assert!(!entry_executed.load(Ordering::SeqCst), "DriverEntry executed on UnknownBad driver!");
        assert!(vdm.is_driver_blocked("suspicious_packer"));
    }

    #[test]
    fn test_unknown_good_allowed_under_mandatory_policy() {
        let elam = Arc::new(ElamDriver::with_default_rules());
        let mut vdm = VortexDriverManager::new();
        vdm.initialize_phase1_elam(elam.clone()).unwrap();

        let entry_executed = Arc::new(AtomicBool::new(false));
        let entry_clone = entry_executed.clone();

        let candidate = BootDriverCandidate::new(
            "custom_clean_vdm",
            "\\SystemRoot\\System32\\drivers\\custom_sensor.sys",
            [0x44; 32],
            DriverSignatureInfo::default(),
            move |_| {
                entry_clone.store(true, Ordering::SeqCst);
                NtStatus::SUCCESS
            },
        );

        let result = vdm.load_boot_driver(&candidate);
        assert!(result.is_ok());
        assert!(entry_executed.load(Ordering::SeqCst));
    }

    #[test]
    fn test_elam_policy_good_only_blocks_unknown_good() {
        let elam = Arc::new(ElamDriver::new("hyav_elam", ElamPolicy::GoodOnly));
        let mut vdm = VortexDriverManager::new();
        vdm.initialize_phase1_elam(elam.clone()).unwrap();

        let entry_executed = Arc::new(AtomicBool::new(false));
        let entry_clone = entry_executed.clone();

        let candidate = BootDriverCandidate::new(
            "unsigned_vendor_driver",
            "\\SystemRoot\\System32\\drivers\\vendor.sys",
            [0x33; 32],
            DriverSignatureInfo::default(),
            move |_| {
                entry_clone.store(true, Ordering::SeqCst);
                NtStatus::SUCCESS
            },
        );

        let result = vdm.load_boot_driver(&candidate);
        match result {
            Err(ElamError::DriverBlockedByPolicy { classification, policy, .. }) => {
                assert_eq!(classification, DriverClassification::UnknownGood);
                assert_eq!(policy, ElamPolicy::GoodOnly);
            }
            other => panic!("Expected policy block for UnknownGood under GoodOnly, got {:?}", other),
        }
        assert!(!entry_executed.load(Ordering::SeqCst));
    }

    #[test]
    fn test_elam_boot_driver_callback_notification() {
        let elam = Arc::new(ElamDriver::with_default_rules());
        let events = Arc::new(Mutex::new(Vec::new()));
        let events_clone = events.clone();

        elam.register_callback(move |decision| {
            events_clone.lock().unwrap().push(decision.clone());
        });

        let mut vdm = VortexDriverManager::new();
        vdm.initialize_phase1_elam(elam.clone()).unwrap();

        let bad_hash = [
            0xDE, 0xAD, 0xBE, 0xEF, 0x01, 0x23, 0x45, 0x67,
            0x89, 0xAB, 0xCD, 0xEF, 0xFE, 0xDC, 0xBA, 0x98,
            0x76, 0x54, 0x32, 0x10, 0x00, 0x11, 0x22, 0x33,
            0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB,
        ];
        let drv = BootDriverCandidate::new("cb_bad", "path", bad_hash, DriverSignatureInfo::default(), |_| NtStatus::SUCCESS);
        let _ = vdm.load_boot_driver(&drv);

        let captured = events.lock().unwrap();
        assert_eq!(captured.len(), 1);
        assert_eq!(captured[0].driver_name, "cb_bad");
        assert_eq!(captured[0].classification, DriverClassification::KnownBad);
        assert!(!captured[0].permitted);
    }

    #[test]
    fn test_elam_wdm_dispatch_ioctl() {
        let elam = Arc::new(ElamDriver::with_default_rules());
        let mut vdm = VortexDriverManager::new();
        vdm.initialize_phase1_elam(elam.clone()).unwrap();

        // Block one driver
        let bad_hash = [
            0xDE, 0xAD, 0xBE, 0xEF, 0x01, 0x23, 0x45, 0x67,
            0x89, 0xAB, 0xCD, 0xEF, 0xFE, 0xDC, 0xBA, 0x98,
            0x76, 0x54, 0x32, 0x10, 0x00, 0x11, 0x22, 0x33,
            0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB,
        ];
        let drv = BootDriverCandidate::new("ioctl_bad", "path", bad_hash, DriverSignatureInfo::default(), |_| NtStatus::SUCCESS);
        let _ = vdm.load_boot_driver(&drv);

        // Query ELAM driver via IRP_MJ_DEVICE_CONTROL
        let mut irp = Irp {
            major_function: Irp::IRP_MJ_DEVICE_CONTROL,
            user_buffer: vec![0u8; 8],
            buffer_length: 8,
            file_offset: 0,
            ioctl_code: 0x0022E000,
            status: NtStatus::SUCCESS,
            bytes_transferred: 0,
        };

        let status = vdm.windows_dispatch_irp("hyav_elam", &mut irp);
        assert_eq!(status, NtStatus::SUCCESS);
        assert_eq!(irp.bytes_transferred, 8);
        let blocked_count = u64::from_le_bytes(irp.user_buffer.try_into().unwrap());
        assert_eq!(blocked_count, 1);
    }
}
