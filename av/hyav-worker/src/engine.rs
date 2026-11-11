//! Unified Worker Inspection Engine for HyAV.
//!
//! Orchestrates the YARA pattern scanner, Shannon section entropy analyzer,
//! and heuristic threat classifier into a unified, zero-unsafe inspection pipeline.

use crate::entropy::{
    calculate_entropy, SectionAnalyzer, SectionDescriptor, SectionEntropyReport,
};
use crate::heuristics::{
    BinaryFeatures, HeuristicClassifier, HeuristicIndicator, ThreatReport, ThreatVerdict,
};
use crate::pattern::{RuleCondition, RuleMatch, RuleSeverity, YaraEngine, YaraPattern, YaraRule};

/// Comprehensive inspection report returned by the worker engine.
#[derive(Debug, Clone, PartialEq)]
pub struct ScanReport {
    /// Aggregate threat score between 0 and 100.
    pub threat_score: u8,
    /// Final security action verdict.
    pub verdict: ThreatVerdict,
    /// Overall file/buffer Shannon entropy (0.0 .. 8.0).
    pub overall_entropy: f64,
    /// Detailed reports per binary section (if format is PE or segmented).
    pub section_reports: Vec<SectionEntropyReport>,
    /// Matched YARA rules.
    pub yara_matches: Vec<RuleMatch>,
    /// Triggered heuristic indicators with score weights.
    pub indicators: Vec<HeuristicIndicator>,
    /// Human-readable report summary.
    pub summary: String,
}

/// Primary Worker Engine coordinating all inspection phases.
#[derive(Debug, Clone)]
pub struct WorkerEngine {
    pub yara: YaraEngine,
    pub section_analyzer: SectionAnalyzer,
    pub classifier: HeuristicClassifier,
}

impl Default for WorkerEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkerEngine {
    /// Initialize worker engine with default baseline signatures and heuristic thresholds.
    pub fn new() -> Self {
        let mut yara = YaraEngine::new();
        Self::load_baseline_signatures(&mut yara);

        Self {
            yara,
            section_analyzer: SectionAnalyzer::default(),
            classifier: HeuristicClassifier::default(),
        }
    }

    /// Initialize with custom YARA engine and thresholds.
    pub fn with_components(
        yara: YaraEngine,
        section_analyzer: SectionAnalyzer,
        classifier: HeuristicClassifier,
    ) -> Self {
        Self {
            yara,
            section_analyzer,
            classifier,
        }
    }

    /// Add a YARA rule to the inspection engine.
    pub fn add_rule(&mut self, rule: YaraRule) {
        self.yara.add_rule(rule);
    }

    /// Inspect a raw byte buffer, optionally accompanied by pre-extracted features.
    ///
    /// If no features are supplied, the engine automatically attempts safe PE parsing
    /// to extract sections and entry point information directly from the buffer.
    pub fn inspect(&self, buffer: &[u8], features: Option<BinaryFeatures>) -> ScanReport {
        let overall_entropy = calculate_entropy(buffer);

        // 1. If features not provided, attempt safe PE section extraction
        let mut feat = features.unwrap_or_else(|| {
            let mut extracted = BinaryFeatures::default();
            if let Some(parsed_sections) = parse_pe_sections_safe(buffer) {
                extracted.sections = self
                    .section_analyzer
                    .analyze_sections(buffer, &parsed_sections);
            }
            if let Some(ep) = parse_pe_entry_point_safe(buffer) {
                extracted.entry_point = Some(ep);
            }
            extracted
        });

        // 2. Run YARA pattern matching
        let matches = self.yara.scan(buffer);
        feat.yara_matches = matches.clone();
        feat.total_file_size = buffer.len();

        // 3. Run Heuristic Threat Classifier
        let ThreatReport {
            score,
            verdict,
            indicators,
            summary,
        } = self.classifier.classify(&feat);

        ScanReport {
            threat_score: score,
            verdict,
            overall_entropy,
            section_reports: feat.sections,
            yara_matches: matches,
            indicators,
            summary,
        }
    }

    fn load_baseline_signatures(yara: &mut YaraEngine) {
        // 1. Standard EICAR Antivirus Test File signature
        let eicar_rule = YaraRule::new(
            "Test_EICAR_Standard_AV",
            RuleSeverity::Malicious,
            vec![YaraPattern::text(
                "$eicar",
                "X5O!P%@AP[4\\PZX54(P^)7CC)7}$EICAR-STANDARD-ANTIVIRUS-TEST-FILE!$H+H*",
            )],
            RuleCondition::All,
        )
        .with_tags(vec!["test", "eicar", "standard"]);

        // 2. Generic Shellcode NOP Sled pattern with wildcards
        let nop_sled = YaraRule::new(
            "Generic_Shellcode_NOP_Sled",
            RuleSeverity::Suspicious,
            vec![
                YaraPattern::hex(
                    "$sled",
                    "90 90 90 90 90 90 90 90 ?? ?? ?? ?? 31 C0",
                )
                .expect("Valid hex pattern"),
            ],
            RuleCondition::All,
        )
        .with_tags(vec!["shellcode", "exploit"]);

        // 3. Ransomware Extension / Note Stub
        let ransom_note = YaraRule::new(
            "Generic_Ransomware_Note_Marker",
            RuleSeverity::Critical,
            vec![
                YaraPattern::nocase("$msg1", "all your files have been encrypted"),
                YaraPattern::nocase("$msg2", "send bitcoin to the following address"),
            ],
            RuleCondition::Any,
        )
        .with_tags(vec!["ransomware", "extortion"]);

        yara.add_rule(eicar_rule);
        yara.add_rule(nop_sled);
        yara.add_rule(ransom_note);
    }
}

/// Safely parse PE header sections without any unsafe pointers or unchecked slice accesses.
pub fn parse_pe_sections_safe(buffer: &[u8]) -> Option<Vec<SectionDescriptor>> {
    // MZ header check
    if buffer.len() < 0x40 || buffer[0] != b'M' || buffer[1] != b'Z' {
        return None;
    }

    // e_lfanew at offset 0x3C (little-endian u32)
    let e_lfanew = u32::from_le_bytes([buffer[0x3C], buffer[0x3D], buffer[0x3E], buffer[0x3F]]) as usize;
    if e_lfanew + 24 > buffer.len() {
        return None;
    }

    // PE signature check: "PE\0\0"
    if &buffer[e_lfanew..e_lfanew + 4] != b"PE\0\0" {
        return None;
    }

    // File header is at e_lfanew + 4
    let file_header_offset = e_lfanew + 4;
    let num_sections = u16::from_le_bytes([
        buffer[file_header_offset + 2],
        buffer[file_header_offset + 3],
    ]) as usize;

    let size_of_optional_header = u16::from_le_bytes([
        buffer[file_header_offset + 16],
        buffer[file_header_offset + 17],
    ]) as usize;

    let sections_start = file_header_offset + 20 + size_of_optional_header;
    if sections_start + (num_sections * 40) > buffer.len() {
        return None;
    }

    let mut sections = Vec::with_capacity(num_sections);
    for i in 0..num_sections {
        let sec_offset = sections_start + (i * 40);
        let name_bytes = &buffer[sec_offset..sec_offset + 8];
        let name = String::from_utf8_lossy(name_bytes)
            .trim_matches('\0')
            .trim()
            .to_string();

        let virtual_size = u32::from_le_bytes([
            buffer[sec_offset + 8],
            buffer[sec_offset + 9],
            buffer[sec_offset + 10],
            buffer[sec_offset + 11],
        ]) as u64;

        let virtual_address = u32::from_le_bytes([
            buffer[sec_offset + 12],
            buffer[sec_offset + 13],
            buffer[sec_offset + 14],
            buffer[sec_offset + 15],
        ]) as u64;

        let raw_size = u32::from_le_bytes([
            buffer[sec_offset + 16],
            buffer[sec_offset + 17],
            buffer[sec_offset + 18],
            buffer[sec_offset + 19],
        ]) as usize;

        let raw_offset = u32::from_le_bytes([
            buffer[sec_offset + 20],
            buffer[sec_offset + 21],
            buffer[sec_offset + 22],
            buffer[sec_offset + 23],
        ]) as usize;

        let characteristics = u32::from_le_bytes([
            buffer[sec_offset + 36],
            buffer[sec_offset + 37],
            buffer[sec_offset + 38],
            buffer[sec_offset + 39],
        ]);

        sections.push(SectionDescriptor::new(
            name,
            virtual_address,
            virtual_size,
            raw_offset,
            raw_size,
            characteristics,
        ));
    }

    Some(sections)
}

/// Safely parse PE AddressOfEntryPoint from Optional Header.
pub fn parse_pe_entry_point_safe(buffer: &[u8]) -> Option<u64> {
    if buffer.len() < 0x40 || buffer[0] != b'M' || buffer[1] != b'Z' {
        return None;
    }

    let e_lfanew = u32::from_le_bytes([buffer[0x3C], buffer[0x3D], buffer[0x3E], buffer[0x3F]]) as usize;
    if e_lfanew + 40 > buffer.len() {
        return None;
    }

    if &buffer[e_lfanew..e_lfanew + 4] != b"PE\0\0" {
        return None;
    }

    let file_header_offset = e_lfanew + 4;
    let size_of_opt = u16::from_le_bytes([
        buffer[file_header_offset + 16],
        buffer[file_header_offset + 17],
    ]) as usize;

    if size_of_opt < 20 {
        return None;
    }

    let opt_offset = file_header_offset + 20;
    if opt_offset + 20 > buffer.len() {
        return None;
    }

    // AddressOfEntryPoint is at offset 16 in Optional Header
    let ep = u32::from_le_bytes([
        buffer[opt_offset + 16],
        buffer[opt_offset + 17],
        buffer[opt_offset + 18],
        buffer[opt_offset + 19],
    ]) as u64;

    Some(ep)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_worker_engine_eicar_detection() {
        let engine = WorkerEngine::new();
        let eicar_payload = b"prefix... X5O!P%@AP[4\\PZX54(P^)7CC)7}$EICAR-STANDARD-ANTIVIRUS-TEST-FILE!$H+H* ...suffix";

        let report = engine.inspect(eicar_payload, None);
        assert_eq!(report.verdict, ThreatVerdict::Suspicious.max_verdict_or_score(report.threat_score));
        assert!(report.threat_score >= 60);
        assert_eq!(report.yara_matches.len(), 1);
        assert_eq!(report.yara_matches[0].rule_name, "Test_EICAR_Standard_AV");
    }

    #[test]
    fn test_worker_engine_ransom_note_malicious() {
        let engine = WorkerEngine::new();
        let ransom_file = b"Attention! ALL YOUR FILES HAVE BEEN ENCRYPTED! Please contact recovery support.";

        let report = engine.inspect(ransom_file, None);
        assert!(report.threat_score >= 70);
        assert_eq!(report.verdict, ThreatVerdict::Malicious);
    }

    #[test]
    fn test_worker_engine_clean_file() {
        let engine = WorkerEngine::new();
        let clean_content = b"HybridOS system configuration: host=localhost, port=8080, mode=production";

        let report = engine.inspect(clean_content, None);
        assert_eq!(report.verdict, ThreatVerdict::Clean);
        assert_eq!(report.threat_score, 0);
        assert!(report.yara_matches.is_empty());
    }

    trait VerdictHelper {
        fn max_verdict_or_score(&self, score: u8) -> ThreatVerdict;
    }

    impl VerdictHelper for ThreatVerdict {
        fn max_verdict_or_score(&self, score: u8) -> ThreatVerdict {
            if score >= 70 {
                ThreatVerdict::Malicious
            } else if score >= 30 {
                ThreatVerdict::Suspicious
            } else {
                ThreatVerdict::Clean
            }
        }
    }
}
