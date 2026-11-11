//! Signature Records for HyAV:
//! - HASH_EXACT
//! - PATTERN
//! - PATH_RULE
//! - PE_HEADER
//! - ELF_HEADER

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RecordType {
    HashExact = 0x01,
    Pattern = 0x02,
    PathRule = 0x03,
    PeHeader = 0x04,
    ElfHeader = 0x05,
}

impl RecordType {
    pub fn from_u8(val: u8) -> Option<Self> {
        match val {
            0x01 => Some(RecordType::HashExact),
            0x02 => Some(RecordType::Pattern),
            0x03 => Some(RecordType::PathRule),
            0x04 => Some(RecordType::PeHeader),
            0x05 => Some(RecordType::ElfHeader),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Severity {
    Whitelist = 0,
    Low = 1,
    Medium = 2,
    High = 3,
    Critical = 4,
}

impl Severity {
    pub fn from_u8(val: u8) -> Self {
        match val {
            0 => Severity::Whitelist,
            1 => Severity::Low,
            2 => Severity::Medium,
            3 => Severity::High,
            _ => Severity::Critical,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Action {
    Alert = 0,
    Block = 1,
    Quarantine = 2,
    Kill = 3,
}

impl Action {
    pub fn from_u8(val: u8) -> Self {
        match val {
            0 => Action::Alert,
            1 => Action::Block,
            2 => Action::Quarantine,
            _ => Action::Kill,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum HashAlgo {
    Sha256 = 0,
    Blake3 = 1,
    Md5 = 2,
}

impl HashAlgo {
    pub fn from_u8(val: u8) -> Self {
        match val {
            1 => HashAlgo::Blake3,
            2 => HashAlgo::Md5,
            _ => HashAlgo::Sha256,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TargetFormat {
    Any = 0,
    Pe = 1,
    Elf = 2,
    Script = 3,
    Document = 4,
}

impl TargetFormat {
    pub fn from_u8(val: u8) -> Self {
        match val {
            1 => TargetFormat::Pe,
            2 => TargetFormat::Elf,
            3 => TargetFormat::Script,
            4 => TargetFormat::Document,
            _ => TargetFormat::Any,
        }
    }
}

// -----------------------------------------------------------------------------
// Record Payloads
// -----------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HashExactRecord {
    pub algo: HashAlgo,
    pub target_size: u64, // 0 = any size
    pub hash: [u8; 32],
}

impl HashExactRecord {
    pub fn new(algo: HashAlgo, hash: [u8; 32], target_size: u64) -> Self {
        Self { algo, target_size, hash }
    }

    pub fn matches(&self, hash: &[u8; 32], size: u64) -> bool {
        if self.target_size != 0 && self.target_size != size {
            return false;
        }
        self.hash == *hash
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatternRecord {
    pub target_format: TargetFormat,
    pub min_offset: u32,
    pub max_offset: u32, // u32::MAX for anywhere
    pub pattern: Vec<u8>,
    pub mask: Vec<u8>, // Empty = exact match. Otherwise len must match pattern.len()
}

impl PatternRecord {
    pub fn exact(target_format: TargetFormat, pattern: Vec<u8>, min_offset: u32, max_offset: u32) -> Self {
        Self {
            target_format,
            min_offset,
            max_offset,
            pattern,
            mask: Vec::new(),
        }
    }

    pub fn with_mask(
        target_format: TargetFormat,
        pattern: Vec<u8>,
        mask: Vec<u8>,
        min_offset: u32,
        max_offset: u32,
    ) -> Self {
        Self {
            target_format,
            min_offset,
            max_offset,
            pattern,
            mask,
        }
    }

    pub fn matches(&self, buffer: &[u8]) -> bool {
        if self.pattern.is_empty() || buffer.len() < self.pattern.len() {
            return false;
        }

        let start = (self.min_offset as usize).min(buffer.len());
        let end = if self.max_offset == u32::MAX {
            buffer.len()
        } else {
            (self.max_offset as usize).min(buffer.len())
        };

        if end < start || (end - start) < self.pattern.len() {
            return false;
        }

        let search_slice = &buffer[start..end];
        let pat_len = self.pattern.len();

        if self.mask.is_empty() {
            // Fast byte window search
            for window in search_slice.windows(pat_len) {
                if window == self.pattern.as_slice() {
                    return true;
                }
            }
        } else {
            // Masked search (0xFF = exact match, 0x00 = wildcard)
            for window in search_slice.windows(pat_len) {
                let mut matched = true;
                for i in 0..pat_len {
                    let m = self.mask[i];
                    if (window[i] & m) != (self.pattern[i] & m) {
                        matched = false;
                        break;
                    }
                }
                if matched {
                    return true;
                }
            }
        }

        false
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathRuleRecord {
    pub pattern: String,
    pub is_regex: bool,
    pub is_whitelist: bool,
}

impl PathRuleRecord {
    pub fn new(pattern: String, is_regex: bool, is_whitelist: bool) -> Self {
        Self {
            pattern,
            is_regex,
            is_whitelist,
        }
    }

    pub fn matches(&self, path: &str) -> bool {
        if self.pattern.is_empty() {
            return false;
        }

        let norm_path = path.replace('\\', "/").to_lowercase();
        let norm_pat = self.pattern.replace('\\', "/").to_lowercase();

        if norm_pat.starts_with('*') && norm_pat.ends_with('*') && norm_pat.len() > 2 {
            let substr = &norm_pat[1..norm_pat.len() - 1];
            norm_path.contains(substr)
        } else if norm_pat.starts_with('*') {
            let suffix = &norm_pat[1..];
            norm_path.ends_with(suffix)
        } else if norm_pat.ends_with('*') {
            let prefix = &norm_pat[..norm_pat.len() - 1];
            norm_path.starts_with(prefix)
        } else {
            norm_path == norm_pat
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PeHeaderRecord {
    pub machine: Option<u16>,
    pub characteristics: Option<u16>,
    pub subsystem: Option<u16>,
    pub min_sections: Option<u16>,
    pub max_sections: Option<u16>,
    pub suspicious_section_names: Vec<String>,
    pub check_entry_in_last_section: bool,
    pub max_entropy: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntheticPeSection {
    pub name: String,
    pub virtual_size: u32,
    pub virtual_address: u32,
    pub raw_data_size: u32,
    pub raw_data_pointer: u32,
    pub characteristics: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SyntheticPeInfo {
    pub machine: u16,
    pub characteristics: u16,
    pub subsystem: u16,
    pub entry_point: u32,
    pub sections: Vec<SyntheticPeSection>,
    pub max_section_entropy: f32,
}

impl PeHeaderRecord {
    pub fn matches(&self, pe: &SyntheticPeInfo) -> bool {
        if let Some(m) = self.machine {
            if pe.machine != m {
                return false;
            }
        }
        if let Some(c) = self.characteristics {
            if (pe.characteristics & c) != c {
                return false;
            }
        }
        if let Some(sub) = self.subsystem {
            if pe.subsystem != sub {
                return false;
            }
        }
        let sec_count = pe.sections.len() as u16;
        if let Some(min_s) = self.min_sections {
            if sec_count < min_s {
                return false;
            }
        }
        if let Some(max_s) = self.max_sections {
            if sec_count > max_s {
                return false;
            }
        }
        for susp in &self.suspicious_section_names {
            for sec in &pe.sections {
                if sec.name.eq_ignore_ascii_case(susp) {
                    return true;
                }
            }
        }
        if self.check_entry_in_last_section && !pe.sections.is_empty() {
            let last = pe.sections.last().unwrap();
            let start = last.virtual_address;
            let end = start + last.virtual_size.max(last.raw_data_size);
            if pe.entry_point >= start && pe.entry_point < end {
                return true;
            }
        }
        if let Some(max_ent) = self.max_entropy {
            if pe.max_section_entropy >= max_ent {
                return true;
            }
        }
        false
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElfHeaderRecord {
    pub machine: Option<u16>,
    pub elf_type: Option<u16>,
    pub suspicious_interpreter: Option<String>,
    pub requires_exec_stack: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntheticElfInfo {
    pub machine: u16,
    pub elf_type: u16,
    pub interpreter: Option<String>,
    pub has_executable_stack: bool,
}

impl ElfHeaderRecord {
    pub fn matches(&self, elf: &SyntheticElfInfo) -> bool {
        if let Some(m) = self.machine {
            if elf.machine != m {
                return false;
            }
        }
        if let Some(t) = self.elf_type {
            if elf.elf_type != t {
                return false;
            }
        }
        if let Some(susp_interp) = &self.suspicious_interpreter {
            if let Some(actual) = &elf.interpreter {
                if actual.contains(susp_interp) {
                    return true;
                }
            }
        }
        if self.requires_exec_stack && elf.has_executable_stack {
            return true;
        }
        false
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum RecordPayload {
    HashExact(HashExactRecord),
    Pattern(PatternRecord),
    PathRule(PathRuleRecord),
    PeHeader(PeHeaderRecord),
    ElfHeader(ElfHeaderRecord),
}

#[derive(Debug, Clone, PartialEq)]
pub struct SignatureRecord {
    pub id: u32,
    pub name: String,
    pub severity: Severity,
    pub action: Action,
    pub payload: RecordPayload,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordCodecError {
    BufferTooSmall,
    InvalidRecordType,
    InvalidUtf8,
    PayloadMismatch,
}

impl SignatureRecord {
    pub fn new(id: u32, name: impl Into<String>, severity: Severity, action: Action, payload: RecordPayload) -> Self {
        Self {
            id,
            name: name.into(),
            severity,
            action,
            payload,
        }
    }

    pub fn record_type(&self) -> RecordType {
        match &self.payload {
            RecordPayload::HashExact(_) => RecordType::HashExact,
            RecordPayload::Pattern(_) => RecordType::Pattern,
            RecordPayload::PathRule(_) => RecordType::PathRule,
            RecordPayload::PeHeader(_) => RecordType::PeHeader,
            RecordPayload::ElfHeader(_) => RecordType::ElfHeader,
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.push(self.record_type() as u8);
        out.extend_from_slice(&self.id.to_le_bytes());
        out.push(self.severity as u8);
        out.push(self.action as u8);

        let name_bytes = self.name.as_bytes();
        out.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        out.extend_from_slice(name_bytes);

        let payload_bytes = self.serialize_payload();
        out.extend_from_slice(&(payload_bytes.len() as u32).to_le_bytes());
        out.extend_from_slice(&payload_bytes);

        out
    }

    fn serialize_payload(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        match &self.payload {
            RecordPayload::HashExact(r) => {
                buf.push(r.algo as u8);
                buf.extend_from_slice(&r.target_size.to_le_bytes());
                buf.extend_from_slice(&r.hash);
            }
            RecordPayload::Pattern(r) => {
                buf.push(r.target_format as u8);
                buf.extend_from_slice(&r.min_offset.to_le_bytes());
                buf.extend_from_slice(&r.max_offset.to_le_bytes());
                buf.extend_from_slice(&(r.pattern.len() as u32).to_le_bytes());
                buf.extend_from_slice(&r.pattern);
                buf.extend_from_slice(&(r.mask.len() as u32).to_le_bytes());
                buf.extend_from_slice(&r.mask);
            }
            RecordPayload::PathRule(r) => {
                buf.push(if r.is_regex { 1 } else { 0 });
                buf.push(if r.is_whitelist { 1 } else { 0 });
                let p_bytes = r.pattern.as_bytes();
                buf.extend_from_slice(&(p_bytes.len() as u16).to_le_bytes());
                buf.extend_from_slice(p_bytes);
            }
            RecordPayload::PeHeader(r) => {
                buf.extend_from_slice(&r.machine.unwrap_or(0).to_le_bytes());
                buf.extend_from_slice(&r.characteristics.unwrap_or(0).to_le_bytes());
                buf.extend_from_slice(&r.subsystem.unwrap_or(0).to_le_bytes());
                buf.extend_from_slice(&r.min_sections.unwrap_or(0).to_le_bytes());
                buf.extend_from_slice(&r.max_sections.unwrap_or(0).to_le_bytes());
                buf.push(if r.check_entry_in_last_section { 1 } else { 0 });
                buf.extend_from_slice(&r.max_entropy.unwrap_or(0.0).to_le_bytes());

                buf.extend_from_slice(&(r.suspicious_section_names.len() as u16).to_le_bytes());
                for s in &r.suspicious_section_names {
                    let sb = s.as_bytes();
                    buf.extend_from_slice(&(sb.len() as u8).to_le_bytes());
                    buf.extend_from_slice(sb);
                }
            }
            RecordPayload::ElfHeader(r) => {
                buf.extend_from_slice(&r.machine.unwrap_or(0).to_le_bytes());
                buf.extend_from_slice(&r.elf_type.unwrap_or(0).to_le_bytes());
                buf.push(if r.requires_exec_stack { 1 } else { 0 });
                if let Some(interp) = &r.suspicious_interpreter {
                    buf.push(1);
                    let ib = interp.as_bytes();
                    buf.extend_from_slice(&(ib.len() as u16).to_le_bytes());
                    buf.extend_from_slice(ib);
                } else {
                    buf.push(0);
                }
            }
        }
        buf
    }

    pub fn deserialize(bytes: &[u8]) -> Result<(Self, usize), RecordCodecError> {
        if bytes.len() < 9 {
            return Err(RecordCodecError::BufferTooSmall);
        }

        let rtype_raw = bytes[0];
        let rtype = RecordType::from_u8(rtype_raw).ok_or(RecordCodecError::InvalidRecordType)?;
        let id = u32::from_le_bytes(bytes[1..5].try_into().unwrap());
        let severity = Severity::from_u8(bytes[5]);
        let action = Action::from_u8(bytes[6]);

        let name_len = u16::from_le_bytes(bytes[7..9].try_into().unwrap()) as usize;
        let offset = 9;
        if bytes.len() < offset + name_len + 4 {
            return Err(RecordCodecError::BufferTooSmall);
        }

        let name = core::str::from_utf8(&bytes[offset..offset + name_len])
            .map_err(|_| RecordCodecError::InvalidUtf8)?
            .to_string();
        let offset = offset + name_len;

        let payload_len = u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
        let offset = offset + 4;
        if bytes.len() < offset + payload_len {
            return Err(RecordCodecError::BufferTooSmall);
        }

        let payload_bytes = &bytes[offset..offset + payload_len];
        let payload = Self::deserialize_payload(rtype, payload_bytes)?;

        let total_consumed = offset + payload_len;
        Ok((
            Self {
                id,
                name,
                severity,
                action,
                payload,
            },
            total_consumed,
        ))
    }

    fn deserialize_payload(rtype: RecordType, data: &[u8]) -> Result<RecordPayload, RecordCodecError> {
        match rtype {
            RecordType::HashExact => {
                if data.len() < 1 + 8 + 32 {
                    return Err(RecordCodecError::BufferTooSmall);
                }
                let algo = HashAlgo::from_u8(data[0]);
                let target_size = u64::from_le_bytes(data[1..9].try_into().unwrap());
                let mut hash = [0u8; 32];
                hash.copy_from_slice(&data[9..41]);
                Ok(RecordPayload::HashExact(HashExactRecord {
                    algo,
                    target_size,
                    hash,
                }))
            }
            RecordType::Pattern => {
                if data.len() < 1 + 4 + 4 + 4 {
                    return Err(RecordCodecError::BufferTooSmall);
                }
                let target_format = TargetFormat::from_u8(data[0]);
                let min_offset = u32::from_le_bytes(data[1..5].try_into().unwrap());
                let max_offset = u32::from_le_bytes(data[5..9].try_into().unwrap());
                let pat_len = u32::from_le_bytes(data[9..13].try_into().unwrap()) as usize;

                let mut cur = 13;
                if data.len() < cur + pat_len + 4 {
                    return Err(RecordCodecError::BufferTooSmall);
                }
                let pattern = data[cur..cur + pat_len].to_vec();
                cur += pat_len;

                let mask_len = u32::from_le_bytes(data[cur..cur + 4].try_into().unwrap()) as usize;
                cur += 4;
                if data.len() < cur + mask_len {
                    return Err(RecordCodecError::BufferTooSmall);
                }
                let mask = data[cur..cur + mask_len].to_vec();

                Ok(RecordPayload::Pattern(PatternRecord {
                    target_format,
                    min_offset,
                    max_offset,
                    pattern,
                    mask,
                }))
            }
            RecordType::PathRule => {
                if data.len() < 4 {
                    return Err(RecordCodecError::BufferTooSmall);
                }
                let is_regex = data[0] != 0;
                let is_whitelist = data[1] != 0;
                let p_len = u16::from_le_bytes(data[2..4].try_into().unwrap()) as usize;
                if data.len() < 4 + p_len {
                    return Err(RecordCodecError::BufferTooSmall);
                }
                let pattern = core::str::from_utf8(&data[4..4 + p_len])
                    .map_err(|_| RecordCodecError::InvalidUtf8)?
                    .to_string();
                Ok(RecordPayload::PathRule(PathRuleRecord {
                    pattern,
                    is_regex,
                    is_whitelist,
                }))
            }
            RecordType::PeHeader => {
                if data.len() < 2 + 2 + 2 + 2 + 2 + 1 + 4 + 2 {
                    return Err(RecordCodecError::BufferTooSmall);
                }
                let machine_raw = u16::from_le_bytes(data[0..2].try_into().unwrap());
                let characteristics_raw = u16::from_le_bytes(data[2..4].try_into().unwrap());
                let subsystem_raw = u16::from_le_bytes(data[4..6].try_into().unwrap());
                let min_sections_raw = u16::from_le_bytes(data[6..8].try_into().unwrap());
                let max_sections_raw = u16::from_le_bytes(data[8..10].try_into().unwrap());
                let check_entry_in_last_section = data[10] != 0;
                let max_entropy_raw = f32::from_le_bytes(data[11..15].try_into().unwrap());
                let susp_count = u16::from_le_bytes(data[15..17].try_into().unwrap()) as usize;

                let mut cur = 17;
                let mut suspicious_section_names = Vec::with_capacity(susp_count);
                for _ in 0..susp_count {
                    if data.len() < cur + 1 {
                        return Err(RecordCodecError::BufferTooSmall);
                    }
                    let slen = data[cur] as usize;
                    cur += 1;
                    if data.len() < cur + slen {
                        return Err(RecordCodecError::BufferTooSmall);
                    }
                    let s = core::str::from_utf8(&data[cur..cur + slen])
                        .map_err(|_| RecordCodecError::InvalidUtf8)?
                        .to_string();
                    suspicious_section_names.push(s);
                    cur += slen;
                }

                Ok(RecordPayload::PeHeader(PeHeaderRecord {
                    machine: if machine_raw != 0 { Some(machine_raw) } else { None },
                    characteristics: if characteristics_raw != 0 { Some(characteristics_raw) } else { None },
                    subsystem: if subsystem_raw != 0 { Some(subsystem_raw) } else { None },
                    min_sections: if min_sections_raw != 0 { Some(min_sections_raw) } else { None },
                    max_sections: if max_sections_raw != 0 { Some(max_sections_raw) } else { None },
                    suspicious_section_names,
                    check_entry_in_last_section,
                    max_entropy: if max_entropy_raw > 0.0 { Some(max_entropy_raw) } else { None },
                }))
            }
            RecordType::ElfHeader => {
                if data.len() < 2 + 2 + 1 + 1 {
                    return Err(RecordCodecError::BufferTooSmall);
                }
                let machine_raw = u16::from_le_bytes(data[0..2].try_into().unwrap());
                let elf_type_raw = u16::from_le_bytes(data[2..4].try_into().unwrap());
                let requires_exec_stack = data[4] != 0;
                let has_interp = data[5] != 0;
                let mut cur = 6;
                let suspicious_interpreter = if has_interp {
                    if data.len() < cur + 2 {
                        return Err(RecordCodecError::BufferTooSmall);
                    }
                    let ilen = u16::from_le_bytes(data[cur..cur + 2].try_into().unwrap()) as usize;
                    cur += 2;
                    if data.len() < cur + ilen {
                        return Err(RecordCodecError::BufferTooSmall);
                    }
                    let s = core::str::from_utf8(&data[cur..cur + ilen])
                        .map_err(|_| RecordCodecError::InvalidUtf8)?
                        .to_string();
                    Some(s)
                } else {
                    None
                };

                Ok(RecordPayload::ElfHeader(ElfHeaderRecord {
                    machine: if machine_raw != 0 { Some(machine_raw) } else { None },
                    elf_type: if elf_type_raw != 0 { Some(elf_type_raw) } else { None },
                    suspicious_interpreter,
                    requires_exec_stack,
                }))
            }
        }
    }
}
