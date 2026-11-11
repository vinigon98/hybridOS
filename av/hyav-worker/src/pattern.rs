//! YARA-compatible pattern matching engine for HyAV Worker.
//!
//! Provides high-performance text pattern and hex wildcard matching
//! with rule conditions and multi-pattern evaluations.

use std::fmt;

/// Represents an individual byte match criterion with an optional bitmask.
///
/// Exact match: `mask == 0xFF`
/// Full wildcard (`??`): `mask == 0x00`
/// Nibble wildcard (`?A`, `A?`): `mask == 0x0F` or `mask == 0xF0`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MaskedByte {
    pub value: u8,
    pub mask: u8,
}

impl MaskedByte {
    /// Create an exact byte match.
    #[inline]
    pub const fn exact(byte: u8) -> Self {
        Self { value: byte, mask: 0xFF }
    }

    /// Create a full wildcard byte (matches any byte).
    #[inline]
    pub const fn wildcard() -> Self {
        Self { value: 0x00, mask: 0x00 }
    }

    /// Create a masked byte with an arbitrary mask.
    #[inline]
    pub const fn with_mask(value: u8, mask: u8) -> Self {
        Self {
            value: value & mask,
            mask,
        }
    }

    /// Returns `true` if this masked byte matches the given target byte.
    #[inline]
    pub const fn matches(&self, target: u8) -> bool {
        (target & self.mask) == self.value
    }
}

/// Errors occurring during pattern parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatternError {
    EmptyPattern,
    InvalidHexCharacter(char),
    OddLengthHexString,
    InvalidToken(String),
}

impl fmt::Display for PatternError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyPattern => write!(f, "pattern string is empty"),
            Self::InvalidHexCharacter(c) => write!(f, "invalid hex character: '{c}'"),
            Self::OddLengthHexString => write!(f, "odd length hex string token"),
            Self::InvalidToken(tok) => write!(f, "invalid hex pattern token: '{tok}'"),
        }
    }
}

impl std::error::Error for PatternError {}

/// Type of pattern to search for in targets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatternType {
    /// Exact byte sequence.
    Exact(Vec<u8>),
    /// Case-insensitive ASCII string.
    CaseInsensitive(Vec<u8>),
    /// Wide string (UTF-16LE representation of ASCII text).
    Wide(Vec<u8>),
    /// Hex sequence with wildcards/masks.
    Masked(Vec<MaskedByte>),
}

impl PatternType {
    /// Parse a YARA-style hex pattern string, e.g.:
    /// `{ 4D 5A 90 00 ?? ?? ?? ?? 50 54 }` or `4d 5a ?A B? ??`
    pub fn parse_hex(pattern_str: &str) -> Result<Self, PatternError> {
        let trimmed = pattern_str.trim();
        let cleaned = trimmed
            .strip_prefix('{')
            .unwrap_or(trimmed)
            .strip_suffix('}')
            .unwrap_or(trimmed)
            .trim();

        if cleaned.is_empty() {
            return Err(PatternError::EmptyPattern);
        }

        let mut masked_bytes = Vec::new();
        for token in cleaned.split_whitespace() {
            let tok = token.trim();
            if tok.is_empty() {
                continue;
            }

            if tok == "??" || tok == "?" {
                masked_bytes.push(MaskedByte::wildcard());
                continue;
            }

            let chars: Vec<char> = tok.chars().collect();
            if chars.len() != 2 {
                return Err(PatternError::InvalidToken(tok.to_string()));
            }

            let (high_char, low_char) = (chars[0], chars[1]);
            match (high_char, low_char) {
                ('?', '?') => {
                    masked_bytes.push(MaskedByte::wildcard());
                }
                ('?', l) => {
                    let low_val = hex_digit_to_val(l)
                        .ok_or(PatternError::InvalidHexCharacter(l))?;
                    masked_bytes.push(MaskedByte::with_mask(low_val, 0x0F));
                }
                (h, '?') => {
                    let high_val = hex_digit_to_val(h)
                        .ok_or(PatternError::InvalidHexCharacter(h))?;
                    masked_bytes.push(MaskedByte::with_mask(high_val << 4, 0xF0));
                }
                (h, l) => {
                    let high_val = hex_digit_to_val(h)
                        .ok_or(PatternError::InvalidHexCharacter(h))?;
                    let low_val = hex_digit_to_val(l)
                        .ok_or(PatternError::InvalidHexCharacter(l))?;
                    masked_bytes.push(MaskedByte::exact((high_val << 4) | low_val));
                }
            }
        }

        if masked_bytes.is_empty() {
            return Err(PatternError::EmptyPattern);
        }

        Ok(PatternType::Masked(masked_bytes))
    }

    /// Construct a wide (UTF-16LE) pattern from an ASCII string.
    pub fn wide_ascii(ascii_str: &str) -> Self {
        let mut wide = Vec::with_capacity(ascii_str.len() * 2);
        for b in ascii_str.as_bytes() {
            wide.push(*b);
            wide.push(0x00);
        }
        PatternType::Wide(wide)
    }

    /// Length in bytes that this pattern occupies when matched.
    pub fn pattern_len(&self) -> usize {
        match self {
            Self::Exact(b) | Self::CaseInsensitive(b) | Self::Wide(b) => b.len(),
            Self::Masked(m) => m.len(),
        }
    }

    /// Search for all occurrences of this pattern in the target buffer.
    pub fn find_matches(&self, buffer: &[u8]) -> Vec<MatchLocation> {
        let mut results = Vec::new();
        let pat_len = self.pattern_len();
        if pat_len == 0 || buffer.len() < pat_len {
            return results;
        }

        let max_start = buffer.len() - pat_len;

        match self {
            Self::Exact(target) => {
                for i in 0..=max_start {
                    if &buffer[i..i + pat_len] == target.as_slice() {
                        results.push(MatchLocation {
                            offset: i,
                            length: pat_len,
                        });
                    }
                }
            }
            Self::CaseInsensitive(target) => {
                for i in 0..=max_start {
                    let slice = &buffer[i..i + pat_len];
                    if slice.iter().zip(target.iter()).all(|(a, b)| {
                        a.to_ascii_lowercase() == b.to_ascii_lowercase()
                    }) {
                        results.push(MatchLocation {
                            offset: i,
                            length: pat_len,
                        });
                    }
                }
            }
            Self::Wide(target) => {
                for i in 0..=max_start {
                    if &buffer[i..i + pat_len] == target.as_slice() {
                        results.push(MatchLocation {
                            offset: i,
                            length: pat_len,
                        });
                    }
                }
            }
            Self::Masked(masks) => {
                for i in 0..=max_start {
                    let slice = &buffer[i..i + pat_len];
                    let mut matched = true;
                    for (b, mask) in slice.iter().zip(masks.iter()) {
                        if !mask.matches(*b) {
                            matched = false;
                            break;
                        }
                    }
                    if matched {
                        results.push(MatchLocation {
                            offset: i,
                            length: pat_len,
                        });
                    }
                }
            }
        }

        results
    }
}

fn hex_digit_to_val(c: char) -> Option<u8> {
    match c {
        '0'..='9' => Some((c as u8) - b'0'),
        'a'..='f' => Some((c as u8) - b'a' + 10),
        'A'..='F' => Some((c as u8) - b'A' + 10),
        _ => None,
    }
}

/// Location of a match within scanned buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MatchLocation {
    pub offset: usize,
    pub length: usize,
}

/// A named pattern within a YARA rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YaraPattern {
    pub id: String,
    pub pattern_type: PatternType,
}

impl YaraPattern {
    pub fn exact<S: Into<String>>(id: S, bytes: &[u8]) -> Self {
        Self {
            id: id.into(),
            pattern_type: PatternType::Exact(bytes.to_vec()),
        }
    }

    pub fn text<S: Into<String>>(id: S, text: &str) -> Self {
        Self {
            id: id.into(),
            pattern_type: PatternType::Exact(text.as_bytes().to_vec()),
        }
    }

    pub fn nocase<S: Into<String>>(id: S, text: &str) -> Self {
        Self {
            id: id.into(),
            pattern_type: PatternType::CaseInsensitive(text.as_bytes().to_vec()),
        }
    }

    pub fn wide<S: Into<String>>(id: S, text: &str) -> Self {
        Self {
            id: id.into(),
            pattern_type: PatternType::wide_ascii(text),
        }
    }

    pub fn hex<S: Into<String>>(id: S, hex_str: &str) -> Result<Self, PatternError> {
        let pattern_type = PatternType::parse_hex(hex_str)?;
        Ok(Self {
            id: id.into(),
            pattern_type,
        })
    }
}

/// Rule evaluation condition logic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleCondition {
    /// All patterns defined in the rule must match at least once.
    All,
    /// At least one pattern in the rule must match.
    Any,
    /// At least N distinct patterns must match.
    AtLeast(usize),
    /// Exactly N patterns must match.
    ExactCount(usize),
}

/// Threat severity level assigned to a matched rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RuleSeverity {
    Informational = 1,
    Suspicious = 2,
    Malicious = 3,
    Critical = 4,
}

impl RuleSeverity {
    /// Base heuristic threat points contribution (0..100).
    pub fn threat_weight(&self) -> u8 {
        match self {
            Self::Informational => 10,
            Self::Suspicious => 30,
            Self::Malicious => 60,
            Self::Critical => 90,
        }
    }
}

/// Complete YARA-compatible signature rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YaraRule {
    pub name: String,
    pub tags: Vec<String>,
    pub severity: RuleSeverity,
    pub patterns: Vec<YaraPattern>,
    pub condition: RuleCondition,
}

impl YaraRule {
    pub fn new<S: Into<String>>(
        name: S,
        severity: RuleSeverity,
        patterns: Vec<YaraPattern>,
        condition: RuleCondition,
    ) -> Self {
        Self {
            name: name.into(),
            tags: Vec::new(),
            severity,
            patterns,
            condition,
        }
    }

    pub fn with_tags(mut self, tags: Vec<&str>) -> Self {
        self.tags = tags.into_iter().map(String::from).collect();
        self
    }
}

/// Match details for an individual pattern within a rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatternMatch {
    pub pattern_id: String,
    pub offset: usize,
    pub length: usize,
}

/// Full rule match result returned after scanning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleMatch {
    pub rule_name: String,
    pub tags: Vec<String>,
    pub severity: RuleSeverity,
    pub matches: Vec<PatternMatch>,
}

/// YARA-compatible pattern matching engine.
#[derive(Debug, Default, Clone)]
pub struct YaraEngine {
    rules: Vec<YaraRule>,
}

impl YaraEngine {
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    pub fn with_rules(rules: Vec<YaraRule>) -> Self {
        Self { rules }
    }

    pub fn add_rule(&mut self, rule: YaraRule) {
        self.rules.push(rule);
    }

    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }

    /// Scan a byte buffer against all registered rules.
    pub fn scan(&self, buffer: &[u8]) -> Vec<RuleMatch> {
        let mut results = Vec::new();

        for rule in &self.rules {
            let mut pattern_matches = Vec::new();
            let mut matched_pattern_indices = Vec::new();

            for (idx, pattern) in rule.patterns.iter().enumerate() {
                let locations = pattern.pattern_type.find_matches(buffer);
                if !locations.is_empty() {
                    matched_pattern_indices.push(idx);
                    for loc in locations {
                        pattern_matches.push(PatternMatch {
                            pattern_id: pattern.id.clone(),
                            offset: loc.offset,
                            length: loc.length,
                        });
                    }
                }
            }

            let distinct_matched_count = matched_pattern_indices.len();
            let total_patterns = rule.patterns.len();

            let condition_met = match rule.condition {
                RuleCondition::All => {
                    total_patterns > 0 && distinct_matched_count == total_patterns
                }
                RuleCondition::Any => distinct_matched_count > 0,
                RuleCondition::AtLeast(n) => distinct_matched_count >= n,
                RuleCondition::ExactCount(n) => distinct_matched_count == n,
            };

            if condition_met {
                results.push(RuleMatch {
                    rule_name: rule.name.clone(),
                    tags: rule.tags.clone(),
                    severity: rule.severity,
                    matches: pattern_matches,
                });
            }
        }

        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_masked_byte_exact_and_wildcard() {
        let exact = MaskedByte::exact(0x4D);
        assert!(exact.matches(0x4D));
        assert!(!exact.matches(0x5A));

        let wildcard = MaskedByte::wildcard();
        assert!(wildcard.matches(0x00));
        assert!(wildcard.matches(0xFF));
        assert!(wildcard.matches(0x4D));

        let nibble_low = MaskedByte::with_mask(0x0A, 0x0F);
        assert!(nibble_low.matches(0x0A));
        assert!(nibble_low.matches(0xFA));
        assert!(!nibble_low.matches(0x0B));
    }

    #[test]
    fn test_hex_pattern_parsing() {
        let pat = PatternType::parse_hex("{ 4D 5A 90 00 ?? ?A F? }").unwrap();
        if let PatternType::Masked(bytes) = pat {
            assert_eq!(bytes.len(), 7);
            assert_eq!(bytes[0], MaskedByte::exact(0x4D));
            assert_eq!(bytes[1], MaskedByte::exact(0x5A));
            assert_eq!(bytes[2], MaskedByte::exact(0x90));
            assert_eq!(bytes[3], MaskedByte::exact(0x00));
            assert_eq!(bytes[4], MaskedByte::wildcard());
            assert_eq!(bytes[5], MaskedByte::with_mask(0x0A, 0x0F));
            assert_eq!(bytes[6], MaskedByte::with_mask(0xF0, 0xF0));
        } else {
            panic!("Expected Masked pattern");
        }
    }

    #[test]
    fn test_text_and_case_insensitive_matching() {
        let buffer = b"Hello, World! HELLO HYBRIDOS";
        let pat_exact = PatternType::Exact(b"HELLO".to_vec());
        let matches = pat_exact.find_matches(buffer);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].offset, 14);

        let pat_nocase = PatternType::CaseInsensitive(b"hello".to_vec());
        let matches_nocase = pat_nocase.find_matches(buffer);
        assert_eq!(matches_nocase.len(), 2);
        assert_eq!(matches_nocase[0].offset, 0);
        assert_eq!(matches_nocase[1].offset, 14);
    }

    #[test]
    fn test_wide_ascii_matching() {
        let buffer = [
            b'c', 0x00, b'm', 0x00, b'd', 0x00, b'.', 0x00, b'e', 0x00, b'x', 0x00, b'e', 0x00,
        ];
        let pat_wide = PatternType::wide_ascii("cmd.exe");
        let matches = pat_wide.find_matches(&buffer);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].offset, 0);
        assert_eq!(matches[0].length, 14);
    }

    #[test]
    fn test_yara_rule_condition_all_and_any() {
        let mut engine = YaraEngine::new();

        let rule_all = YaraRule::new(
            "Test_LockBit_Header",
            RuleSeverity::Malicious,
            vec![
                YaraPattern::text("$s1", "LOCKBIT"),
                YaraPattern::hex("$h1", "4D 5A ?? 00").unwrap(),
            ],
            RuleCondition::All,
        )
        .with_tags(vec!["ransomware", "critical"]);

        let rule_any = YaraRule::new(
            "Test_Suspicious_Script",
            RuleSeverity::Suspicious,
            vec![
                YaraPattern::nocase("$cmd", "powershell.exe"),
                YaraPattern::nocase("$enc", "-encodedcommand"),
            ],
            RuleCondition::Any,
        );

        engine.add_rule(rule_all);
        engine.add_rule(rule_any);

        // Test buffer 1: matches only powershell (Any matches, All does not)
        let buf1 = b"invoking powershell.exe with params";
        let res1 = engine.scan(buf1);
        assert_eq!(res1.len(), 1);
        assert_eq!(res1[0].rule_name, "Test_Suspicious_Script");

        // Test buffer 2: matches both LOCKBIT and 4D 5A FF 00
        let buf2 = b"prefix \x4D\x5A\xAA\x00 middle LOCKBIT suffix";
        let res2 = engine.scan(buf2);
        assert_eq!(res2.len(), 1);
        assert_eq!(res2[0].rule_name, "Test_LockBit_Header");
        assert_eq!(res2[0].tags, vec!["ransomware", "critical"]);
        assert_eq!(res2[0].matches.len(), 2);
    }
}
