//! Section Entropy Analyzer using Shannon Entropy for HyAV Worker.
//!
//! Evaluates information density across binary sections and sliding memory windows
//! to detect packers, crypters, shellcode, and compressed/encrypted payloads.

/// Standard PE section characteristics flags.
pub const IMAGE_SCN_CNT_CODE: u32 = 0x0000_0020;
pub const IMAGE_SCN_CNT_INITIALIZED_DATA: u32 = 0x0000_0040;
pub const IMAGE_SCN_CNT_UNINITIALIZED_DATA: u32 = 0x0000_0080;
pub const IMAGE_SCN_MEM_EXECUTE: u32 = 0x2000_0000;
pub const IMAGE_SCN_MEM_READ: u32 = 0x4000_0000;
pub const IMAGE_SCN_MEM_WRITE: u32 = 0x8000_0000;

/// Default entropy thresholds.
pub const DEFAULT_PACKED_ENTROPY_THRESHOLD: f64 = 7.20;
pub const DEFAULT_SUSPICIOUS_ENTROPY_THRESHOLD: f64 = 6.80;
pub const DEFAULT_MIN_CODE_ENTROPY: f64 = 4.00;

/// Known packer section name prefixes.
pub const KNOWN_PACKER_SECTIONS: &[&str] = &[
    "UPX0", "UPX1", "UPX2", ".aspack", ".adata", ".vmp0", ".vmp1",
    ".themida", ".pecompact", "MPRESS1", "MPRESS2", ".nsp0", ".nsp1",
    ".packed", "PEC2TO", "PECompact2",
];

/// Computes the Shannon entropy of a byte buffer.
///
/// Returns a value between `0.0` (all bytes identical or empty buffer)
/// and `8.0` (maximum randomness / uniformly distributed bytes).
///
/// Formula:
/// H = - sum_{i=0}^{255} (P(x_i) * log2(P(x_i)))
pub fn calculate_entropy(data: &[u8]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }

    let mut counts = [0usize; 256];
    for &byte in data {
        counts[byte as usize] += 1;
    }

    let len_f = data.len() as f64;
    let mut entropy = 0.0;

    for &count in &counts {
        if count > 0 {
            let p = (count as f64) / len_f;
            entropy -= p * p.log2();
        }
    }

    // Clamp small floating precision deviations
    if entropy < 0.0 {
        0.0
    } else if entropy > 8.0 {
        8.0
    } else {
        entropy
    }
}

/// Description of a section within an executable or container.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionDescriptor {
    pub name: String,
    pub virtual_address: u64,
    pub virtual_size: u64,
    pub raw_offset: usize,
    pub raw_size: usize,
    pub characteristics: u32,
}

impl SectionDescriptor {
    pub fn new<S: Into<String>>(
        name: S,
        virtual_address: u64,
        virtual_size: u64,
        raw_offset: usize,
        raw_size: usize,
        characteristics: u32,
    ) -> Self {
        Self {
            name: name.into(),
            virtual_address,
            virtual_size,
            raw_offset,
            raw_size,
            characteristics,
        }
    }

    #[inline]
    pub fn is_executable(&self) -> bool {
        (self.characteristics & IMAGE_SCN_MEM_EXECUTE) != 0
            || (self.characteristics & IMAGE_SCN_CNT_CODE) != 0
    }

    #[inline]
    pub fn is_writable(&self) -> bool {
        (self.characteristics & IMAGE_SCN_MEM_WRITE) != 0
    }

    #[inline]
    pub fn is_readable(&self) -> bool {
        (self.characteristics & IMAGE_SCN_MEM_READ) != 0
    }

    #[inline]
    pub fn is_wx(&self) -> bool {
        self.is_executable() && self.is_writable()
    }
}

/// Detailed entropy and structural report for a binary section.
#[derive(Debug, Clone, PartialEq)]
pub struct SectionEntropyReport {
    pub name: String,
    pub entropy: f64,
    pub raw_size: usize,
    pub virtual_size: u64,
    pub is_executable: bool,
    pub is_writable: bool,
    pub is_wx: bool,
    pub is_packed: bool,
    pub is_suspicious: bool,
    pub anomaly_reasons: Vec<String>,
}

/// Configurable section entropy analyzer.
#[derive(Debug, Clone)]
pub struct SectionAnalyzer {
    pub packed_threshold: f64,
    pub suspicious_threshold: f64,
}

impl Default for SectionAnalyzer {
    fn default() -> Self {
        Self {
            packed_threshold: DEFAULT_PACKED_ENTROPY_THRESHOLD,
            suspicious_threshold: DEFAULT_SUSPICIOUS_ENTROPY_THRESHOLD,
        }
    }
}

impl SectionAnalyzer {
    pub fn new(packed_threshold: f64, suspicious_threshold: f64) -> Self {
        Self {
            packed_threshold,
            suspicious_threshold,
        }
    }

    /// Analyze an isolated section byte slice with its name and characteristics.
    pub fn analyze_slice(
        &self,
        name: &str,
        data: &[u8],
        virtual_size: u64,
        characteristics: u32,
    ) -> SectionEntropyReport {
        let entropy = calculate_entropy(data);
        let is_exec = (characteristics & IMAGE_SCN_MEM_EXECUTE) != 0
            || (characteristics & IMAGE_SCN_CNT_CODE) != 0;
        let is_write = (characteristics & IMAGE_SCN_MEM_WRITE) != 0;
        let is_wx = is_exec && is_write;

        let mut anomaly_reasons = Vec::new();

        let is_packed = entropy >= self.packed_threshold;
        let mut is_suspicious = entropy >= self.suspicious_threshold;

        if is_packed {
            anomaly_reasons.push(format!(
                "High entropy ({:.2}) indicates packing or encryption",
                entropy
            ));
        }

        if is_wx {
            anomaly_reasons.push("Section is marked both Writable and Executable (W+X)".to_string());
            is_suspicious = true;
        }

        let clean_name = name.trim_matches('\0').trim();
        for &packer in KNOWN_PACKER_SECTIONS {
            if clean_name.eq_ignore_ascii_case(packer)
                || clean_name.to_ascii_uppercase().starts_with(packer)
            {
                anomaly_reasons.push(format!("Known packer section signature: '{clean_name}'"));
                is_suspicious = true;
                break;
            }
        }

        if is_exec && data.len() < 512 && virtual_size > 4096 {
            anomaly_reasons.push(format!(
                "Deceptive size: raw size ({}) is disproportionately smaller than virtual size ({})",
                data.len(),
                virtual_size
            ));
            is_suspicious = true;
        }

        SectionEntropyReport {
            name: clean_name.to_string(),
            entropy,
            raw_size: data.len(),
            virtual_size,
            is_executable: is_exec,
            is_writable: is_write,
            is_wx,
            is_packed,
            is_suspicious,
            anomaly_reasons,
        }
    }

    /// Analyze a list of section descriptors against the full binary image.
    pub fn analyze_sections(
        &self,
        full_image: &[u8],
        sections: &[SectionDescriptor],
    ) -> Vec<SectionEntropyReport> {
        let mut reports = Vec::with_capacity(sections.len());

        for sec in sections {
            let data = if sec.raw_offset < full_image.len() {
                let end = (sec.raw_offset + sec.raw_size).min(full_image.len());
                &full_image[sec.raw_offset..end]
            } else {
                &[]
            };

            reports.push(self.analyze_slice(
                &sec.name,
                data,
                sec.virtual_size,
                sec.characteristics,
            ));
        }

        reports
    }

    /// Computes sliding-window entropy across a buffer.
    /// Returns vector of `(offset, entropy)` pairs.
    pub fn sliding_window_entropy(
        data: &[u8],
        window_size: usize,
        step: usize,
    ) -> Vec<(usize, f64)> {
        let mut results = Vec::new();
        if window_size == 0 || step == 0 || data.len() < window_size {
            return results;
        }

        let mut offset = 0;
        while offset + window_size <= data.len() {
            let slice = &data[offset..offset + window_size];
            let ent = calculate_entropy(slice);
            results.push((offset, ent));
            offset += step;
        }

        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entropy_empty_and_uniform() {
        assert_eq!(calculate_entropy(&[]), 0.0);

        let zeros = vec![0u8; 1024];
        assert_eq!(calculate_entropy(&zeros), 0.0);

        let ones = vec![0xFFu8; 500];
        assert_eq!(calculate_entropy(&ones), 0.0);
    }

    #[test]
    fn test_entropy_maximum_256_bytes() {
        let mut all_bytes = Vec::with_capacity(256 * 10);
        for _ in 0..10 {
            for b in 0u8..=255 {
                all_bytes.push(b);
            }
        }
        let ent = calculate_entropy(&all_bytes);
        assert!((ent - 8.0).abs() < 1e-6, "Expected 8.0, got {}", ent);
    }

    #[test]
    fn test_entropy_text_and_random() {
        let text = b"The quick brown fox jumps over the lazy dog. HybridOS microkernel architecture!";
        let text_ent = calculate_entropy(text);
        assert!(text_ent > 3.0 && text_ent < 5.5, "Text entropy should be medium: {}", text_ent);

        // Artificial pseudo-random high entropy payload (simulating AES or packed stub)
        let mut lcg: u32 = 123456789;
        let mut random_bytes = vec![0u8; 2048];
        for b in &mut random_bytes {
            lcg = lcg.wrapping_mul(1664525).wrapping_add(1013904223);
            *b = (lcg >> 16) as u8;
        }
        let rand_ent = calculate_entropy(&random_bytes);
        assert!(rand_ent > 7.8, "Random bytes should have high entropy: {}", rand_ent);
    }

    #[test]
    fn test_section_analyzer_wx_and_packed() {
        let analyzer = SectionAnalyzer::default();

        // 1. UPX section with high entropy and W+X flags
        let mut lcg: u32 = 987654321;
        let mut packed_payload = vec![0u8; 1024];
        for b in &mut packed_payload {
            lcg = lcg.wrapping_mul(1664525).wrapping_add(1013904223);
            *b = (lcg >> 16) as u8;
        }

        let report_upx = analyzer.analyze_slice(
            "UPX0\0\0\0\0",
            &packed_payload,
            65536,
            IMAGE_SCN_MEM_EXECUTE | IMAGE_SCN_MEM_WRITE | IMAGE_SCN_MEM_READ,
        );

        assert!(report_upx.is_packed);
        assert!(report_upx.is_wx);
        assert!(report_upx.is_suspicious);
        assert_eq!(report_upx.name, "UPX0");
        assert!(report_upx.anomaly_reasons.len() >= 2);

        // 2. Normal .text section
        let normal_code = b"\x55\x8B\xEC\x83\xEC\x10\x53\x56\x57\x8B\x7D\x08\x85\xFF\x74\x20";
        let report_text = analyzer.analyze_slice(
            ".text",
            normal_code,
            4096,
            IMAGE_SCN_MEM_EXECUTE | IMAGE_SCN_MEM_READ,
        );

        assert!(!report_text.is_packed);
        assert!(!report_text.is_wx);
        assert!(!report_text.is_suspicious);
    }

    #[test]
    fn test_sliding_window_entropy() {
        let mut buffer = vec![0u8; 500]; // 0..500 zero entropy
        // Append 500 bytes of alternating pattern
        for i in 0..500 {
            buffer.push((i % 256) as u8);
        }

        let windows = SectionAnalyzer::sliding_window_entropy(&buffer, 100, 50);
        assert!(!windows.is_empty());
        assert_eq!(windows[0].1, 0.0); // First window has zero entropy
        let last_window = windows.last().unwrap();
        assert!(last_window.1 > 5.0); // Window over diverse bytes has high entropy
    }
}
