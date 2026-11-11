//! Heuristic Threat Classifier for HyAV Worker.
//!
//! Evaluates static, structural, import-level, and signature anomalies
//! to compute a calibrated Threat Score between 0 and 100 with actionable verdicts.

use crate::entropy::SectionEntropyReport;
use crate::pattern::RuleMatch;

/// Final threat evaluation verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreatVerdict {
    /// Safe / Clean binary (Threat Score 0..=29).
    Clean,
    /// Suspicious binary requiring alert, elevation, or further sandbox analysis (30..=69).
    Suspicious,
    /// Malicious payload requiring immediate quarantine or blocking (70..=100).
    Malicious,
}

impl ThreatVerdict {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Clean => "CLEAN",
            Self::Suspicious => "SUSPICIOUS",
            Self::Malicious => "MALICIOUS",
        }
    }
}

/// Severity classification for heuristic indicators.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum IndicatorSeverity {
    Low,
    Medium,
    High,
    Critical,
}

/// An individual security heuristic indicator triggered during inspection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeuristicIndicator {
    pub code: &'static str,
    pub description: String,
    pub weight: u8,
    pub severity: IndicatorSeverity,
}

impl HeuristicIndicator {
    pub fn new(
        code: &'static str,
        description: impl Into<String>,
        weight: u8,
        severity: IndicatorSeverity,
    ) -> Self {
        Self {
            code,
            description: description.into(),
            weight,
            severity,
        }
    }
}

/// Extracted structural and behavioural features for heuristic evaluation.
#[derive(Debug, Clone, Default)]
pub struct BinaryFeatures {
    /// Evaluated sections and their entropy metrics.
    pub sections: Vec<SectionEntropyReport>,
    /// Imported API function names (e.g., `VirtualAllocEx`, `CreateRemoteThread`).
    pub imports: Vec<String>,
    /// Address of entry point (RVA / offset), if available.
    pub entry_point: Option<u64>,
    /// Base address of image, if available.
    pub image_base: Option<u64>,
    /// Size of binary on disk or in buffer.
    pub total_file_size: usize,
    /// YARA pattern matches detected during scan.
    pub yara_matches: Vec<RuleMatch>,
    /// File extension or nominal format hint.
    pub format_hint: Option<String>,
}

/// Comprehensive heuristic evaluation report.
#[derive(Debug, Clone, PartialEq)]
pub struct ThreatReport {
    /// Final aggregate threat score clamped between 0 and 100.
    pub score: u8,
    /// Overall verdict based on score thresholds.
    pub verdict: ThreatVerdict,
    /// Detailed list of triggered heuristic indicators.
    pub indicators: Vec<HeuristicIndicator>,
    /// Human-readable threat analysis summary.
    pub summary: String,
}

/// Configurable Heuristic Threat Classifier.
#[derive(Debug, Clone)]
pub struct HeuristicClassifier {
    /// Score threshold separating Clean from Suspicious (default 30).
    pub suspicious_threshold: u8,
    /// Score threshold separating Suspicious from Malicious (default 70).
    pub malicious_threshold: u8,
}

impl Default for HeuristicClassifier {
    fn default() -> Self {
        Self {
            suspicious_threshold: 30,
            malicious_threshold: 70,
        }
    }
}

impl HeuristicClassifier {
    pub fn new(suspicious_threshold: u8, malicious_threshold: u8) -> Self {
        Self {
            suspicious_threshold,
            malicious_threshold,
        }
    }

    /// Evaluates binary features and computes the threat report.
    pub fn classify(&self, features: &BinaryFeatures) -> ThreatReport {
        let mut indicators = Vec::new();

        // 1. Section & Entropy Heuristics
        self.evaluate_sections(features, &mut indicators);

        // 2. Entry Point Anomalies
        self.evaluate_entry_point(features, &mut indicators);

        // 3. Import & API Combination Heuristics
        self.evaluate_imports(features, &mut indicators);

        // 4. YARA Signature Match Ingestion
        self.evaluate_yara_matches(features, &mut indicators);

        // 5. Aggregate Threat Score (clamped at 100)
        let total_weight: u32 = indicators.iter().map(|ind| ind.weight as u32).sum();
        let score = (total_weight.min(100)) as u8;

        let verdict = if score >= self.malicious_threshold {
            ThreatVerdict::Malicious
        } else if score >= self.suspicious_threshold {
            ThreatVerdict::Suspicious
        } else {
            ThreatVerdict::Clean
        };

        let summary = format!(
            "Threat Score: {}/100 (Verdict: {}) - {} indicator(s) triggered",
            score,
            verdict.as_str(),
            indicators.len()
        );

        ThreatReport {
            score,
            verdict,
            indicators,
            summary,
        }
    }

    fn evaluate_sections(&self, features: &BinaryFeatures, indicators: &mut Vec<HeuristicIndicator>) {
        let mut wx_sections_count = 0;
        let mut packed_sections_count = 0;
        let mut suspicious_name_count = 0;

        for sec in &features.sections {
            if sec.is_wx {
                wx_sections_count += 1;
            }
            if sec.is_packed {
                packed_sections_count += 1;
            }
            for reason in &sec.anomaly_reasons {
                if reason.contains("Known packer section") {
                    suspicious_name_count += 1;
                }
            }
        }

        if wx_sections_count > 0 {
            indicators.push(HeuristicIndicator::new(
                "HEUR_SECTION_WX",
                format!("{wx_sections_count} section(s) marked both Writable and Executable (W+X)"),
                30,
                IndicatorSeverity::High,
            ));
        }

        if suspicious_name_count > 0 {
            indicators.push(HeuristicIndicator::new(
                "HEUR_PACKER_SECTION_NAME",
                format!("Detected {suspicious_name_count} section(s) matching known packer/crypter names (UPX/VMP/ASPack)"),
                25,
                IndicatorSeverity::Medium,
            ));
        }

        if packed_sections_count > 0 {
            indicators.push(HeuristicIndicator::new(
                "HEUR_HIGH_SECTION_ENTROPY",
                format!("{packed_sections_count} section(s) exhibit high entropy (> 7.2) indicating packing/encryption"),
                25,
                IndicatorSeverity::Medium,
            ));
        }

        // Section count anomaly: extremely few or unusually large count
        if !features.sections.is_empty() {
            if features.sections.len() == 1 && features.sections[0].is_executable {
                indicators.push(HeuristicIndicator::new(
                    "HEUR_SINGLE_CODE_SECTION",
                    "Monolithic single section binary; typical for shellcode droppers or obfuscated stubs",
                    15,
                    IndicatorSeverity::Low,
                ));
            } else if features.sections.len() > 14 {
                indicators.push(HeuristicIndicator::new(
                    "HEUR_EXCESSIVE_SECTIONS",
                    format!("Abnormal number of sections ({})", features.sections.len()),
                    10,
                    IndicatorSeverity::Low,
                ));
            }
        }
    }

    fn evaluate_entry_point(&self, features: &BinaryFeatures, indicators: &mut Vec<HeuristicIndicator>) {
        if let Some(ep) = features.entry_point {
            if features.sections.is_empty() {
                return;
            }

            // Check if entry point falls within any section
            let mut ep_section = None;
            for (idx, sec) in features.sections.iter().enumerate() {
                // If section virtual size is non-zero, check bounds
                if sec.virtual_size > 0 {
                    // Approximate section RVA check
                    if ep >= (idx as u64 * 0x1000) && ep < ((idx as u64 + 1) * 0x1000 + sec.virtual_size) {
                        ep_section = Some((idx, sec));
                        break;
                    }
                }
            }

            // If EP is located in the last section, typical for packer stubs
            if let Some((idx, sec)) = ep_section {
                if idx == features.sections.len() - 1 && features.sections.len() > 1 {
                    indicators.push(HeuristicIndicator::new(
                        "HEUR_EP_LAST_SECTION",
                        format!("Entry point is in the final section '{}' (packer stub behavior)", sec.name),
                        20,
                        IndicatorSeverity::Medium,
                    ));
                }

                if sec.is_writable {
                    indicators.push(HeuristicIndicator::new(
                        "HEUR_EP_IN_WRITABLE_SECTION",
                        format!("Entry point points into writable section '{}'", sec.name),
                        25,
                        IndicatorSeverity::High,
                    ));
                }
            }
        }
    }

    fn evaluate_imports(&self, features: &BinaryFeatures, indicators: &mut Vec<HeuristicIndicator>) {
        if features.imports.is_empty() {
            return;
        }

        let import_set: Vec<String> = features
            .imports
            .iter()
            .map(|s| s.to_ascii_lowercase())
            .collect();

        let contains_import = |name: &str| -> bool {
            let target = name.to_ascii_lowercase();
            import_set.iter().any(|imp| imp == &target || imp.ends_with(&format!("!{target}")))
        };

        // 1. Process Injection Triad
        let has_alloc = contains_import("virtualallex")
            || contains_import("virtualalloc")
            || contains_import("ntallocatevirtualmemory")
            || contains_import("zwallocatevirtualmemory");
        let has_write = contains_import("writeprocessmemory")
            || contains_import("ntwritevirtualmemory")
            || contains_import("zwwritevirtualmemory");
        let has_thread = contains_import("createremotethread")
            || contains_import("createremotethreadex")
            || contains_import("rtlcreateuserthread")
            || contains_import("ntcreatethreadex");

        if has_alloc && has_write && has_thread {
            indicators.push(HeuristicIndicator::new(
                "HEUR_INJECTION_TRIAD",
                "Classic Process Injection API Triad detected (Alloc + Write + Thread)",
                35,
                IndicatorSeverity::Critical,
            ));
        }

        // 2. Process Hollowing
        let has_unmap = contains_import("ntunmapviewofsection")
            || contains_import("zwunmapviewofsection");
        if has_unmap {
            indicators.push(HeuristicIndicator::new(
                "HEUR_PROCESS_HOLLOWING",
                "Process Hollowing unmap API detected (NtUnmapViewOfSection)",
                25,
                IndicatorSeverity::High,
            ));
        }

        // 3. Keystroke / Input Hooking
        let has_hook = contains_import("setwindowshookexa")
            || contains_import("setwindowshookexw")
            || contains_import("getasynckeystate")
            || contains_import("getkeystate");
        if has_hook {
            indicators.push(HeuristicIndicator::new(
                "HEUR_KEYLOGGER_HOOK",
                "User input spying or hooking APIs detected",
                20,
                IndicatorSeverity::Medium,
            ));
        }

        // 4. Anti-Debugging & Evasion
        let has_antidebug = contains_import("isdebuggerpresent")
            || contains_import("checkremotedebuggerpresent")
            || contains_import("ntqueryinformationprocess");
        if has_antidebug {
            indicators.push(HeuristicIndicator::new(
                "HEUR_ANTI_DEBUGGING",
                "Anti-debugging / environment detection APIs found",
                15,
                IndicatorSeverity::Low,
            ));
        }
    }

    fn evaluate_yara_matches(&self, features: &BinaryFeatures, indicators: &mut Vec<HeuristicIndicator>) {
        for rule_match in &features.yara_matches {
            let weight = rule_match.severity.threat_weight();
            let sev = match rule_match.severity {
                crate::pattern::RuleSeverity::Informational => IndicatorSeverity::Low,
                crate::pattern::RuleSeverity::Suspicious => IndicatorSeverity::Medium,
                crate::pattern::RuleSeverity::Malicious => IndicatorSeverity::High,
                crate::pattern::RuleSeverity::Critical => IndicatorSeverity::Critical,
            };

            indicators.push(HeuristicIndicator::new(
                "HEUR_YARA_SIGNATURE_MATCH",
                format!(
                    "YARA rule '{}' triggered ({} matches, tags: {:?})",
                    rule_match.rule_name,
                    rule_match.matches.len(),
                    rule_match.tags
                ),
                weight,
                sev,
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pattern::{PatternMatch, RuleSeverity};

    #[test]
    fn test_clean_binary_classification() {
        let classifier = HeuristicClassifier::default();
        let features = BinaryFeatures {
            sections: vec![
                SectionEntropyReport {
                    name: ".text".to_string(),
                    entropy: 5.8,
                    raw_size: 4096,
                    virtual_size: 4096,
                    is_executable: true,
                    is_writable: false,
                    is_wx: false,
                    is_packed: false,
                    is_suspicious: false,
                    anomaly_reasons: Vec::new(),
                },
                SectionEntropyReport {
                    name: ".rdata".to_string(),
                    entropy: 4.2,
                    raw_size: 2048,
                    virtual_size: 2048,
                    is_executable: false,
                    is_writable: false,
                    is_wx: false,
                    is_packed: false,
                    is_suspicious: false,
                    anomaly_reasons: Vec::new(),
                },
            ],
            imports: vec!["ExitProcess".to_string(), "MessageBoxW".to_string()],
            ..Default::default()
        };

        let report = classifier.classify(&features);
        assert_eq!(report.verdict, ThreatVerdict::Clean);
        assert_eq!(report.score, 0);
        assert!(report.indicators.is_empty());
    }

    #[test]
    fn test_suspicious_binary_classification() {
        let classifier = HeuristicClassifier::default();
        let features = BinaryFeatures {
            sections: vec![
                SectionEntropyReport {
                    name: ".text".to_string(),
                    entropy: 7.3, // High entropy
                    raw_size: 8192,
                    virtual_size: 8192,
                    is_executable: true,
                    is_writable: false,
                    is_wx: false,
                    is_packed: true,
                    is_suspicious: true,
                    anomaly_reasons: vec!["High entropy indicates packing".to_string()],
                },
                SectionEntropyReport {
                    name: ".rdata".to_string(),
                    entropy: 4.0,
                    raw_size: 2048,
                    virtual_size: 2048,
                    is_executable: false,
                    is_writable: false,
                    is_wx: false,
                    is_packed: false,
                    is_suspicious: false,
                    anomaly_reasons: Vec::new(),
                },
            ],
            imports: vec!["IsDebuggerPresent".to_string()],
            ..Default::default()
        };

        let report = classifier.classify(&features);
        assert_eq!(report.verdict, ThreatVerdict::Suspicious);
        // High entropy (25) + Anti-debug (15) = 40
        assert_eq!(report.score, 40);
        assert_eq!(report.indicators.len(), 2);
    }

    #[test]
    fn test_single_code_section_indicator() {
        let classifier = HeuristicClassifier::default();
        let features = BinaryFeatures {
            sections: vec![SectionEntropyReport {
                name: ".text".to_string(),
                entropy: 5.5,
                raw_size: 4096,
                virtual_size: 4096,
                is_executable: true,
                is_writable: false,
                is_wx: false,
                is_packed: false,
                is_suspicious: false,
                anomaly_reasons: Vec::new(),
            }],
            ..Default::default()
        };

        let report = classifier.classify(&features);
        assert_eq!(report.score, 15);
        assert_eq!(report.indicators[0].code, "HEUR_SINGLE_CODE_SECTION");
    }

    #[test]
    fn test_malicious_binary_with_injection_and_yara() {
        let classifier = HeuristicClassifier::default();
        let features = BinaryFeatures {
            sections: vec![
                SectionEntropyReport {
                    name: "UPX0".to_string(),
                    entropy: 7.6,
                    raw_size: 1024,
                    virtual_size: 65536,
                    is_executable: true,
                    is_writable: true,
                    is_wx: true,
                    is_packed: true,
                    is_suspicious: true,
                    anomaly_reasons: vec![
                        "Known packer section signature: 'UPX0'".to_string(),
                        "Section is marked both Writable and Executable (W+X)".to_string(),
                    ],
                },
            ],
            imports: vec![
                "VirtualAllocEx".to_string(),
                "WriteProcessMemory".to_string(),
                "CreateRemoteThread".to_string(),
            ],
            yara_matches: vec![RuleMatch {
                rule_name: "Ransomware_LockBit".to_string(),
                tags: vec!["ransomware".to_string()],
                severity: RuleSeverity::Critical,
                matches: vec![PatternMatch {
                    pattern_id: "$lock".to_string(),
                    offset: 100,
                    length: 16,
                }],
            }],
            ..Default::default()
        };

        let report = classifier.classify(&features);
        assert_eq!(report.verdict, ThreatVerdict::Malicious);
        // Section WX (30) + Packer name (25) + Packed entropy (25) + Injection (35) + YARA Critical (90) = 205 -> clamped to 100
        assert_eq!(report.score, 100);
        assert!(report.indicators.len() >= 4);
    }
}
