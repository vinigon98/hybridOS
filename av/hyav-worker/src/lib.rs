//! hyav-worker: HybridOS Anti-Virus Worker Inspection Engine
//!
//! Provides YARA-compatible pattern matching, Shannon entropy analysis per section,
//! and heuristic threat scoring (0..100) with strictly safe Rust (#![forbid(unsafe_code)]).

#![forbid(unsafe_code)]

pub mod amsi;
pub mod engine;
pub mod entropy;
pub mod heuristics;
pub mod pattern;

pub use amsi::{
    AmsiEngine, AmsiError, AmsiVerdict, Deobfuscator, IAntimalwareProvider, ScriptOriginInfo,
    AMSI_RESULT_BLOCKED_BY_ADMIN_END, AMSI_RESULT_BLOCKED_BY_ADMIN_START, AMSI_RESULT_CLEAN,
    AMSI_RESULT_DETECTED, AMSI_RESULT_NOT_DETECTED, DEFAULT_AMSI_TIMEOUT,
    DEFAULT_MAX_DEOBFUSCATED_BYTES, DEFAULT_MAX_DEOBFUSCATION_DEPTH,
};
pub use engine::{parse_pe_entry_point_safe, parse_pe_sections_safe, ScanReport, WorkerEngine};
pub use entropy::{
    calculate_entropy, SectionAnalyzer, SectionDescriptor, SectionEntropyReport,
    DEFAULT_MIN_CODE_ENTROPY, DEFAULT_PACKED_ENTROPY_THRESHOLD,
    DEFAULT_SUSPICIOUS_ENTROPY_THRESHOLD, IMAGE_SCN_CNT_CODE, IMAGE_SCN_CNT_INITIALIZED_DATA,
    IMAGE_SCN_CNT_UNINITIALIZED_DATA, IMAGE_SCN_MEM_EXECUTE, IMAGE_SCN_MEM_READ,
    IMAGE_SCN_MEM_WRITE, KNOWN_PACKER_SECTIONS,
};
pub use heuristics::{
    BinaryFeatures, HeuristicClassifier, HeuristicIndicator, IndicatorSeverity, ThreatReport,
    ThreatVerdict,
};
pub use pattern::{
    MaskedByte, MatchLocation, PatternError, PatternMatch, PatternType, RuleCondition, RuleMatch,
    RuleSeverity, YaraEngine, YaraPattern, YaraRule,
};
