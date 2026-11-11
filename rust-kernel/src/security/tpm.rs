//! Trusted Platform Module (TPM 2.0) & Measured Boot Implementation for HybridOS.
//! Provides hardware-grade measurement primitives (PCR extend) and TCG event logging.

#[allow(unused_imports)]
use alloc::string::{String, ToString};
#[allow(unused_imports)]
use alloc::vec::Vec;
#[allow(unused_imports)]
use alloc::boxed::Box;
#[allow(unused_imports)]
use crate::compat::FloatExt;
use std::sync::{Arc, Mutex, OnceLock};
use super::sha256::{sha256_hash, SHA256_DIGEST_LENGTH};

pub const TPM_PCR_COUNT: usize = 24;
pub const PCR_BASE_SYSTEM: usize = 9;   // PCR 9: System Base / Signature Database
pub const PCR_DRIVERS: usize = 10;      // PCR 10: Boot-time Drivers (ELAM / Kernel)

/// Standard TCG Event Types
pub const EV_PREBOOT_CERT: u32    = 0x00000000;
pub const EV_POST_CODE: u32       = 0x00000001;
pub const EV_SEPARATOR: u32       = 0x00000004;
pub const EV_ACTION: u32          = 0x00000005;
pub const EV_EVENT_TAG: u32       = 0x00000006;
pub const EV_HYAV_BASE: u32       = 0x00000009; // HyAV Base measurement
pub const EV_HYAV_DRIVER: u32     = 0x0000000A; // Early launch / kernel driver measurement

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TpmError {
    InvalidPcrIndex(usize),
    DeviceLocked,
    IntegrityCheckFailed,
    IoError(String),
}

impl std::fmt::Display for TpmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TpmError::InvalidPcrIndex(idx) => write!(f, "Invalid PCR index: {} (valid: 0..{})", idx, TPM_PCR_COUNT - 1),
            TpmError::DeviceLocked => write!(f, "TPM device is locked or disabled"),
            TpmError::IntegrityCheckFailed => write!(f, "Measurement integrity verification failed"),
            TpmError::IoError(msg) => write!(f, "TPM I/O error: {}", msg),
        }
    }
}

impl std::error::Error for TpmError {}

/// TCG Measured Boot Event Log Entry
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TpmEventLogEntry {
    pub pcr_index: usize,
    pub event_type: u32,
    pub digest: [u8; SHA256_DIGEST_LENGTH],
    pub description: String,
    pub pcr_before: [u8; SHA256_DIGEST_LENGTH],
    pub pcr_after: [u8; SHA256_DIGEST_LENGTH],
    pub timestamp_ms: u64,
}

/// Simulated TPM 2.0 Hardware Device
#[derive(Debug, Clone)]
pub struct TpmDevice {
    /// 24 SHA-256 Platform Configuration Registers (PCR 0..23)
    pcrs: [[u8; SHA256_DIGEST_LENGTH]; TPM_PCR_COUNT],
    /// TCG Measured Boot Event Log
    event_log: Vec<TpmEventLogEntry>,
    /// Clock counter in milliseconds
    tick_counter: u64,
}

impl Default for TpmDevice {
    fn default() -> Self {
        Self::new()
    }
}

impl TpmDevice {
    /// Initialize a new TPM 2.0 device with all PCRs reset to 0.
    pub fn new() -> Self {
        Self {
            pcrs: [[0u8; SHA256_DIGEST_LENGTH]; TPM_PCR_COUNT],
            event_log: Vec::new(),
            tick_counter: 100,
        }
    }

    /// Read the current 32-byte SHA-256 value of a PCR register.
    pub fn read_pcr(&self, pcr: usize) -> Result<[u8; SHA256_DIGEST_LENGTH], TpmError> {
        if pcr >= TPM_PCR_COUNT {
            return Err(TpmError::InvalidPcrIndex(pcr));
        }
        Ok(self.pcrs[pcr])
    }

    /// Extend a Platform Configuration Register (PCR) using SHA-256.
    ///
    /// According to TCG TPM 2.0 specifications:
    /// `PCR_new = SHA-256(PCR_old || measurement_digest)`
    ///
    /// If `measurement` is 32 bytes, it is treated as a pre-computed digest.
    /// Otherwise, SHA-256 is computed over the payload.
    pub fn tpm_extend_pcr(&mut self, pcr: usize, measurement: &[u8]) -> Result<[u8; SHA256_DIGEST_LENGTH], TpmError> {
        self.tpm_extend_pcr_with_desc(pcr, measurement, EV_ACTION, "TPM PCR Extend")
    }

    /// Extend PCR with explicit event type and descriptive tag for the TCG Event Log.
    pub fn tpm_extend_pcr_with_desc(
        &mut self,
        pcr: usize,
        measurement: &[u8],
        event_type: u32,
        desc: &str,
    ) -> Result<[u8; SHA256_DIGEST_LENGTH], TpmError> {
        if pcr >= TPM_PCR_COUNT {
            return Err(TpmError::InvalidPcrIndex(pcr));
        }

        // 1. Resolve 32-byte measurement digest
        let digest = if measurement.len() == SHA256_DIGEST_LENGTH {
            let mut d = [0u8; SHA256_DIGEST_LENGTH];
            d.copy_from_slice(measurement);
            d
        } else {
            sha256_hash(measurement)
        };

        let pcr_before = self.pcrs[pcr];

        // 2. Concatenate: buffer = PCR_old || digest (64 bytes)
        let mut concat_buf = [0u8; SHA256_DIGEST_LENGTH * 2];
        concat_buf[..SHA256_DIGEST_LENGTH].copy_from_slice(&pcr_before);
        concat_buf[SHA256_DIGEST_LENGTH..].copy_from_slice(&digest);

        // 3. PCR_new = SHA-256(concat_buf)
        let pcr_after = sha256_hash(&concat_buf);
        self.pcrs[pcr] = pcr_after;

        // 4. Record entry in TCG Event Log
        self.tick_counter += 10;
        self.event_log.push(TpmEventLogEntry {
            pcr_index: pcr,
            event_type,
            digest,
            description: desc.to_string(),
            pcr_before,
            pcr_after,
            timestamp_ms: self.tick_counter,
        });

        Ok(pcr_after)
    }

    /// Helper for PCR 9: System Base / Signature Database measurement
    pub fn extend_pcr9_base(&mut self, hash_da_base: &[u8]) -> Result<[u8; SHA256_DIGEST_LENGTH], TpmError> {
        self.tpm_extend_pcr_with_desc(
            PCR_BASE_SYSTEM,
            hash_da_base,
            EV_HYAV_BASE,
            "HyAV System Base / Signature DB Measurement",
        )
    }

    /// Helper for PCR 10: Boot-time Driver Measurement
    /// Computes digest of driver image/spec and extends PCR 10.
    pub fn measure_driver(&mut self, driver_name: &str, driver_image: &[u8]) -> Result<[u8; SHA256_DIGEST_LENGTH], TpmError> {
        let digest = sha256_hash(driver_image);
        let desc = format!("Driver Measurement: {}", driver_name);
        self.tpm_extend_pcr_with_desc(
            PCR_DRIVERS,
            &digest,
            EV_HYAV_DRIVER,
            &desc,
        )
    }

    /// Measure driver given a pre-computed SHA-256 hash
    pub fn measure_driver_hash(&mut self, driver_name: &str, driver_hash: &[u8; SHA256_DIGEST_LENGTH]) -> Result<[u8; SHA256_DIGEST_LENGTH], TpmError> {
        let desc = format!("Driver Measurement (Pre-hashed): {}", driver_name);
        self.tpm_extend_pcr_with_desc(
            PCR_DRIVERS,
            driver_hash,
            EV_HYAV_DRIVER,
            &desc,
        )
    }

    /// Get reference to full TCG Event Log
    pub fn event_log(&self) -> &[TpmEventLogEntry] {
        &self.event_log
    }

    /// Replay and verify all events in the event log against final PCR values
    pub fn verify_event_log(&self) -> bool {
        let mut sim_pcrs = [[0u8; SHA256_DIGEST_LENGTH]; TPM_PCR_COUNT];
        for entry in &self.event_log {
            if entry.pcr_index >= TPM_PCR_COUNT {
                return false;
            }
            let mut concat_buf = [0u8; SHA256_DIGEST_LENGTH * 2];
            concat_buf[..SHA256_DIGEST_LENGTH].copy_from_slice(&sim_pcrs[entry.pcr_index]);
            concat_buf[SHA256_DIGEST_LENGTH..].copy_from_slice(&entry.digest);
            let expected_after = sha256_hash(&concat_buf);
            if expected_after != entry.pcr_after {
                return false;
            }
            sim_pcrs[entry.pcr_index] = expected_after;
        }

        // Compare simulated PCRs with current PCR bank
        sim_pcrs == self.pcrs
    }
}

// ============================================================================
// Global Singleton TPM Device & Convenience Functions
// ============================================================================

static GLOBAL_TPM: OnceLock<Arc<Mutex<TpmDevice>>> = OnceLock::new();

/// Retrieve or initialize the global kernel TPM device.
pub fn get_global_tpm() -> Arc<Mutex<TpmDevice>> {
    GLOBAL_TPM.get_or_init(|| Arc::new(Mutex::new(TpmDevice::new()))).clone()
}

/// Global convenience function to extend a PCR (e.g. pcr=9, hash_da_base)
pub fn tpm_extend_pcr(pcr: usize, hash_da_base: &[u8]) -> Result<[u8; SHA256_DIGEST_LENGTH], TpmError> {
    let tpm = get_global_tpm();
    let mut dev = tpm.lock().unwrap();
    dev.tpm_extend_pcr(pcr, hash_da_base)
}

/// Global convenience function to measure a driver in PCR 10
pub fn tpm_measure_driver(driver_name: &str, driver_bytes: &[u8]) -> Result<[u8; SHA256_DIGEST_LENGTH], TpmError> {
    let tpm = get_global_tpm();
    let mut dev = tpm.lock().unwrap();
    dev.measure_driver(driver_name, driver_bytes)
}

/// Global convenience function to read PCR value
pub fn tpm_read_pcr(pcr: usize) -> Result<[u8; SHA256_DIGEST_LENGTH], TpmError> {
    let tpm = get_global_tpm();
    let dev = tpm.lock().unwrap();
    dev.read_pcr(pcr)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pcr_initialization() {
        let tpm = TpmDevice::new();
        for i in 0..TPM_PCR_COUNT {
            assert_eq!(tpm.read_pcr(i).unwrap(), [0u8; 32]);
        }
        assert_eq!(tpm.read_pcr(24), Err(TpmError::InvalidPcrIndex(24)));
    }

    #[test]
    fn test_pcr9_base_extension() {
        let mut tpm = TpmDevice::new();
        let base_hash = sha256_hash(b"HYAV_SIGNATURE_BASE_V1.0_2026");

        // Extend PCR 9 with base hash
        let pcr9_val = tpm.extend_pcr9_base(&base_hash).expect("PCR 9 extension failed");

        // Verify mathematical formula: SHA256(PCR_old || measurement)
        let mut expected_buf = [0u8; 64];
        expected_buf[0..32].copy_from_slice(&[0u8; 32]); // PCR_old was zero
        expected_buf[32..64].copy_from_slice(&base_hash);
        let expected_hash = sha256_hash(&expected_buf);

        assert_eq!(pcr9_val, expected_hash);
        assert_eq!(tpm.read_pcr(9).unwrap(), expected_hash);

        // Verify event log
        assert_eq!(tpm.event_log().len(), 1);
        let entry = &tpm.event_log()[0];
        assert_eq!(entry.pcr_index, 9);
        assert_eq!(entry.event_type, EV_HYAV_BASE);
        assert_eq!(entry.pcr_after, expected_hash);
    }

    #[test]
    fn test_pcr10_driver_measurements() {
        let mut tpm = TpmDevice::new();

        // 1. Measure Driver A (e.g. nvme.sys)
        let driver_a_payload = b"MOCK_NVME_STORAGE_DRIVER_PAYLOAD";
        let pcr10_after_a = tpm.measure_driver("nvme.sys", driver_a_payload).expect("Driver A measurement failed");

        let driver_a_hash = sha256_hash(driver_a_payload);
        let mut expected_a_buf = [0u8; 64];
        expected_a_buf[0..32].copy_from_slice(&[0u8; 32]);
        expected_a_buf[32..64].copy_from_slice(&driver_a_hash);
        let expected_a = sha256_hash(&expected_a_buf);
        assert_eq!(pcr10_after_a, expected_a);

        // 2. Measure Driver B (e.g. virtio_net.ko)
        let driver_b_payload = b"MOCK_VIRTIO_NET_DRIVER_BINARY";
        let pcr10_after_b = tpm.measure_driver("virtio_net.ko", driver_b_payload).expect("Driver B measurement failed");

        let driver_b_hash = sha256_hash(driver_b_payload);
        let mut expected_b_buf = [0u8; 64];
        expected_b_buf[0..32].copy_from_slice(&expected_a);
        expected_b_buf[32..64].copy_from_slice(&driver_b_hash);
        let expected_b = sha256_hash(&expected_b_buf);
        assert_eq!(pcr10_after_b, expected_b);
        assert_eq!(tpm.read_pcr(10).unwrap(), expected_b);

        // 3. Verify event log replay
        assert_eq!(tpm.event_log().len(), 2);
        assert!(tpm.verify_event_log());
    }

    #[test]
    fn test_sequential_extends_deterministic() {
        let mut tpm1 = TpmDevice::new();
        let mut tpm2 = TpmDevice::new();

        let base = sha256_hash(b"system_base");
        let drv1 = b"driver1";
        let drv2 = b"driver2";

        tpm1.tpm_extend_pcr(9, &base).unwrap();
        tpm1.measure_driver("drv1", drv1).unwrap();
        tpm1.measure_driver("drv2", drv2).unwrap();

        tpm2.tpm_extend_pcr(9, &base).unwrap();
        tpm2.measure_driver("drv1", drv1).unwrap();
        tpm2.measure_driver("drv2", drv2).unwrap();

        assert_eq!(tpm1.read_pcr(9).unwrap(), tpm2.read_pcr(9).unwrap());
        assert_eq!(tpm1.read_pcr(10).unwrap(), tpm2.read_pcr(10).unwrap());
        assert!(tpm1.verify_event_log());
        assert!(tpm2.verify_event_log());
    }
}
