//! HybridOS Windows Registry Editor (regedit.exe)
//! 100% Pure, Safe, Robust Rust Implementation
//!
//! Complete, production-grade Registry Editor:
//! 1. Registry Tree Hierarchy:
//!    - Standard Root Hives: HKEY_CLASSES_ROOT, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, HKEY_USERS, HKEY_CURRENT_CONFIG
//!    - Deep tree navigation with expand/collapse, Computer root node
//!    - Address bar with path normalization, abbreviations (HKLM, HKCU, etc.) & breadcrumbs
//!    - Back / Forward history navigation stack
//! 2. Value Data Store & Types:
//!    - RegistryValue: Name, ValueType, RawData (Vec<u8>)
//!    - Supported types: REG_SZ, REG_EXPAND_SZ, REG_BINARY, REG_DWORD, REG_QWORD, REG_MULTI_SZ, REG_NONE
//!    - Formatted value display: String decoding, Hex viewer format, DWORD (dec & hex)
//! 3. Value Editing Dialogs:
//!    - Edit String dialog (REG_SZ / REG_EXPAND_SZ)
//!    - Edit DWORD (32-bit) & QWORD (64-bit) dialog (Hexadecimal vs Decimal base switcher)
//!    - Edit Binary Data dialog (Byte hex editor grid with ASCII column)
//!    - Edit Multi-String dialog (Multi-line text area)
//! 4. Key & Value Operations:
//!    - New Key, New String Value, New DWORD (32-bit), New QWORD (64-bit), New Binary Value, New Multi-String Value
//!    - Rename Key / Value, Delete Key / Value with confirmation dialog
//!    - Find dialog (Ctrl+F) with search across keys, values, and data, match whole string, match case, Find Next (F3)
//!    - Windows Registry Editor Version 5.00 Export and Import (.reg serializer/deserializer)
//! 5. UI Layout, Renderer & Event Dispatch:
//!    - Menu Bar, Address Bar, Split Pane (Tree View + Value List), Status Bar
//!    - Interactive event processing and view-model projection

use std::collections::BTreeMap;
use std::fmt;

// ============================================================================
// 1. Constants & Hive Definitions
// ============================================================================

pub const COMPUTER_ROOT: &str = "Computer";
pub const HKCR: &str = "HKEY_CLASSES_ROOT";
pub const HKCU: &str = "HKEY_CURRENT_USER";
pub const HKLM: &str = "HKEY_LOCAL_MACHINE";
pub const HKU: &str = "HKEY_USERS";
pub const HKCC: &str = "HKEY_CURRENT_CONFIG";

pub const ROOT_HIVES: [&str; 5] = [HKCR, HKCU, HKLM, HKU, HKCC];

// ============================================================================
// 2. Registry Value Types & Data Structures
// ============================================================================

/// Win32 Registry Value Types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RegistryValueType {
    RegNone = 0,
    RegSz = 1,
    RegExpandSz = 2,
    RegBinary = 3,
    RegDword = 4,
    RegDwordBigEndian = 5,
    RegLink = 6,
    RegMultiSz = 7,
    RegResourceList = 8,
    RegFullResourceDescriptor = 9,
    RegResourceRequirementsList = 10,
    RegQword = 11,
}

impl RegistryValueType {
    pub fn from_u32(val: u32) -> Self {
        match val {
            0 => Self::RegNone,
            1 => Self::RegSz,
            2 => Self::RegExpandSz,
            3 => Self::RegBinary,
            4 => Self::RegDword,
            5 => Self::RegDwordBigEndian,
            6 => Self::RegLink,
            7 => Self::RegMultiSz,
            8 => Self::RegResourceList,
            9 => Self::RegFullResourceDescriptor,
            10 => Self::RegResourceRequirementsList,
            11 => Self::RegQword,
            _ => Self::RegNone,
        }
    }

    pub fn to_u32(self) -> u32 {
        self as u32
    }

    pub fn type_name(self) -> &'static str {
        match self {
            Self::RegNone => "REG_NONE",
            Self::RegSz => "REG_SZ",
            Self::RegExpandSz => "REG_EXPAND_SZ",
            Self::RegBinary => "REG_BINARY",
            Self::RegDword => "REG_DWORD",
            Self::RegDwordBigEndian => "REG_DWORD_BIG_ENDIAN",
            Self::RegLink => "REG_LINK",
            Self::RegMultiSz => "REG_MULTI_SZ",
            Self::RegResourceList => "REG_RESOURCE_LIST",
            Self::RegFullResourceDescriptor => "REG_FULL_RESOURCE_DESCRIPTOR",
            Self::RegResourceRequirementsList => "REG_RESOURCE_REQUIREMENTS_LIST",
            Self::RegQword => "REG_QWORD",
        }
    }

    pub fn from_type_name(name: &str) -> Option<Self> {
        match name.to_ascii_uppercase().as_str() {
            "REG_NONE" => Some(Self::RegNone),
            "REG_SZ" => Some(Self::RegSz),
            "REG_EXPAND_SZ" => Some(Self::RegExpandSz),
            "REG_BINARY" => Some(Self::RegBinary),
            "REG_DWORD" => Some(Self::RegDword),
            "REG_DWORD_BIG_ENDIAN" => Some(Self::RegDwordBigEndian),
            "REG_LINK" => Some(Self::RegLink),
            "REG_MULTI_SZ" => Some(Self::RegMultiSz),
            "REG_RESOURCE_LIST" => Some(Self::RegResourceList),
            "REG_FULL_RESOURCE_DESCRIPTOR" => Some(Self::RegFullResourceDescriptor),
            "REG_RESOURCE_REQUIREMENTS_LIST" => Some(Self::RegResourceRequirementsList),
            "REG_QWORD" => Some(Self::RegQword),
            _ => None,
        }
    }
}

impl fmt::Display for RegistryValueType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.type_name())
    }
}

/// Registry Error Enumeration
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryError {
    KeyNotFound(String),
    KeyAlreadyExists(String),
    ValueNotFound(String),
    ValueAlreadyExists(String),
    CannotDeleteRootHive(String),
    InvalidPath(String),
    InvalidName(String),
    InvalidData(String),
    SerializationError(String),
    ParseError(String),
}

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::KeyNotFound(k) => write!(f, "Registry key not found: '{}'", k),
            Self::KeyAlreadyExists(k) => write!(f, "Registry key already exists: '{}'", k),
            Self::ValueNotFound(v) => write!(f, "Registry value not found: '{}'", v),
            Self::ValueAlreadyExists(v) => write!(f, "Registry value already exists: '{}'", v),
            Self::CannotDeleteRootHive(h) => write!(f, "Cannot delete root hive: '{}'", h),
            Self::InvalidPath(p) => write!(f, "Invalid registry path: '{}'", p),
            Self::InvalidName(n) => write!(f, "Invalid registry name: '{}'", n),
            Self::InvalidData(d) => write!(f, "Invalid registry data: '{}'", d),
            Self::SerializationError(e) => write!(f, "Registry serialization error: '{}'", e),
            Self::ParseError(p) => write!(f, "Registry parse error: '{}'", p),
        }
    }
}

impl std::error::Error for RegistryError {}

/// Represents a single registry value entry in a key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryValue {
    /// Name of the value. Empty string `""` represents `(Default)`.
    pub name: String,
    pub value_type: RegistryValueType,
    pub raw_data: Vec<u8>,
}

impl RegistryValue {
    pub fn new_default_empty() -> Self {
        Self {
            name: String::new(),
            value_type: RegistryValueType::RegSz,
            raw_data: Vec::new(),
        }
    }

    pub fn new_sz(name: impl Into<String>, text: &str) -> Self {
        Self {
            name: name.into(),
            value_type: RegistryValueType::RegSz,
            raw_data: string_to_utf16le_bytes(text, true),
        }
    }

    pub fn new_expand_sz(name: impl Into<String>, text: &str) -> Self {
        Self {
            name: name.into(),
            value_type: RegistryValueType::RegExpandSz,
            raw_data: string_to_utf16le_bytes(text, true),
        }
    }

    pub fn new_dword(name: impl Into<String>, val: u32) -> Self {
        Self {
            name: name.into(),
            value_type: RegistryValueType::RegDword,
            raw_data: val.to_le_bytes().to_vec(),
        }
    }

    pub fn new_qword(name: impl Into<String>, val: u64) -> Self {
        Self {
            name: name.into(),
            value_type: RegistryValueType::RegQword,
            raw_data: val.to_le_bytes().to_vec(),
        }
    }

    pub fn new_binary(name: impl Into<String>, bytes: &[u8]) -> Self {
        Self {
            name: name.into(),
            value_type: RegistryValueType::RegBinary,
            raw_data: bytes.to_vec(),
        }
    }

    pub fn new_multi_sz(name: impl Into<String>, strings: &[String]) -> Self {
        Self {
            name: name.into(),
            value_type: RegistryValueType::RegMultiSz,
            raw_data: encode_multi_sz(strings),
        }
    }

    pub fn new_none(name: impl Into<String>, bytes: &[u8]) -> Self {
        Self {
            name: name.into(),
            value_type: RegistryValueType::RegNone,
            raw_data: bytes.to_vec(),
        }
    }

    /// Check if this value represents the key's Default value
    pub fn is_default(&self) -> bool {
        self.name.is_empty() || self.name == "(Default)"
    }

    /// Display name formatted for the UI (shows `(Default)` if empty)
    pub fn display_name(&self) -> &str {
        if self.name.is_empty() {
            "(Default)"
        } else {
            &self.name
        }
    }

    /// Decode raw data into a readable String for strings (REG_SZ, REG_EXPAND_SZ)
    pub fn decode_string(&self) -> String {
        decode_registry_string(&self.raw_data)
    }

    /// Decode raw data as a 32-bit integer (REG_DWORD)
    pub fn decode_dword(&self) -> u32 {
        let mut b = [0u8; 4];
        let copy_len = self.raw_data.len().min(4);
        b[..copy_len].copy_from_slice(&self.raw_data[..copy_len]);
        u32::from_le_bytes(b)
    }

    /// Decode raw data as a 64-bit integer (REG_QWORD)
    pub fn decode_qword(&self) -> u64 {
        let mut b = [0u8; 8];
        let copy_len = self.raw_data.len().min(8);
        b[..copy_len].copy_from_slice(&self.raw_data[..copy_len]);
        u64::from_le_bytes(b)
    }

    /// Decode raw data as a list of strings (REG_MULTI_SZ)
    pub fn decode_multi_sz(&self) -> Vec<String> {
        decode_multi_sz(&self.raw_data)
    }

    /// Formats the value data for display in the Regedit right-hand list view.
    pub fn format_display_data(&self) -> String {
        match self.value_type {
            RegistryValueType::RegSz | RegistryValueType::RegExpandSz => {
                let s = self.decode_string();
                if s.is_empty() && self.raw_data.is_empty() {
                    "(value not set)".to_string()
                } else {
                    s
                }
            }
            RegistryValueType::RegDword => {
                let val = self.decode_dword();
                format!("0x{:08x} ({})", val, val)
            }
            RegistryValueType::RegQword => {
                let val = self.decode_qword();
                format!("0x{:016x} ({})", val, val)
            }
            RegistryValueType::RegBinary => {
                if self.raw_data.is_empty() {
                    "(zero-length binary value)".to_string()
                } else {
                    self.raw_data
                        .iter()
                        .map(|b| format!("{:02x}", b))
                        .collect::<Vec<String>>()
                        .join(" ")
                }
            }
            RegistryValueType::RegMultiSz => {
                let list = self.decode_multi_sz();
                if list.is_empty() {
                    "(zero-length multi-string)".to_string()
                } else {
                    list.join(" ")
                }
            }
            RegistryValueType::RegNone => {
                if self.raw_data.is_empty() {
                    "(zero-length binary value)".to_string()
                } else {
                    self.raw_data
                        .iter()
                        .map(|b| format!("{:02x}", b))
                        .collect::<Vec<String>>()
                        .join(" ")
                }
            }
            _ => {
                if self.raw_data.is_empty() {
                    "(empty)".to_string()
                } else {
                    self.raw_data
                        .iter()
                        .map(|b| format!("{:02x}", b))
                        .collect::<Vec<String>>()
                        .join(" ")
                }
            }
        }
    }
}

// ============================================================================
// 3. String & Byte Encoding / Decoding Helpers
// ============================================================================

pub fn string_to_utf16le_bytes(s: &str, null_terminate: bool) -> Vec<u8> {
    let mut bytes = Vec::with_capacity((s.len() + 1) * 2);
    for code_unit in s.encode_utf16() {
        bytes.extend_from_slice(&code_unit.to_le_bytes());
    }
    if null_terminate {
        bytes.push(0);
        bytes.push(0);
    }
    bytes
}

pub fn bytes_to_utf16le_string(bytes: &[u8]) -> Option<String> {
    if bytes.len() % 2 != 0 {
        return None;
    }
    let mut units = Vec::with_capacity(bytes.len() / 2);
    for chunk in bytes.chunks_exact(2) {
        let code = u16::from_le_bytes([chunk[0], chunk[1]]);
        if code == 0 {
            break;
        }
        units.push(code);
    }
    String::from_utf16(&units).ok()
}

pub fn decode_registry_string(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return String::new();
    }
    // Attempt UTF-16LE first if length is even
    if bytes.len() % 2 == 0 {
        if let Some(s) = bytes_to_utf16le_string(bytes) {
            return s;
        }
    }
    // Fallback: UTF-8 / ASCII, trimming trailing nulls
    let trimmed = if bytes.last() == Some(&0) {
        &bytes[..bytes.len() - 1]
    } else {
        bytes
    };
    String::from_utf8_lossy(trimmed).into_owned()
}

pub fn encode_multi_sz(strings: &[String]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for s in strings {
        for code in s.encode_utf16() {
            bytes.extend_from_slice(&code.to_le_bytes());
        }
        bytes.push(0);
        bytes.push(0);
    }
    // Final null terminator for double-null
    bytes.push(0);
    bytes.push(0);
    bytes
}

pub fn decode_multi_sz(bytes: &[u8]) -> Vec<String> {
    if bytes.len() < 2 {
        return Vec::new();
    }
    if bytes.len() % 2 == 0 {
        let mut result = Vec::new();
        let mut current_units = Vec::new();
        for chunk in bytes.chunks_exact(2) {
            let code = u16::from_le_bytes([chunk[0], chunk[1]]);
            if code == 0 {
                if current_units.is_empty() {
                    break;
                } else {
                    if let Ok(s) = String::from_utf16(&current_units) {
                        result.push(s);
                    }
                    current_units.clear();
                }
            } else {
                current_units.push(code);
            }
        }
        if !current_units.is_empty() {
            if let Ok(s) = String::from_utf16(&current_units) {
                result.push(s);
            }
        }
        if !result.is_empty() {
            return result;
        }
    }

    // Fallback UTF-8 null-delimited
    let text = String::from_utf8_lossy(bytes);
    text.split('\0')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

// ============================================================================
// 4. Registry Key Node & Tree Hierarchy
// ============================================================================

/// Represents a single registry node in the hierarchy.
#[derive(Debug, Clone)]
pub struct RegistryKeyNode {
    pub name: String,
    pub subkeys: BTreeMap<String, RegistryKeyNode>,
    pub values: BTreeMap<String, RegistryValue>,
    pub is_expanded: bool,
    pub last_modified: u64,
}

impl RegistryKeyNode {
    pub fn new(name: impl Into<String>) -> Self {
        let mut node = Self {
            name: name.into(),
            subkeys: BTreeMap::new(),
            values: BTreeMap::new(),
            is_expanded: false,
            last_modified: 0,
        };
        // Ensure default value entry exists
        node.values.insert(String::new(), RegistryValue::new_default_empty());
        node
    }

    /// Case-insensitive search for subkey
    pub fn get_subkey(&self, name: &str) -> Option<&RegistryKeyNode> {
        if let Some(exact) = self.subkeys.get(name) {
            return Some(exact);
        }
        self.subkeys
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v)
    }

    /// Case-insensitive mutable search for subkey
    pub fn get_subkey_mut(&mut self, name: &str) -> Option<&mut RegistryKeyNode> {
        if self.subkeys.contains_key(name) {
            return self.subkeys.get_mut(name);
        }
        let matching_key = self
            .subkeys
            .keys()
            .find(|k| k.eq_ignore_ascii_case(name))
            .cloned();

        if let Some(key) = matching_key {
            self.subkeys.get_mut(&key)
        } else {
            None
        }
    }

    /// Case-insensitive search for value
    pub fn get_value(&self, name: &str) -> Option<&RegistryValue> {
        if name.is_empty() || name == "(Default)" {
            return self.values.get("");
        }
        if let Some(val) = self.values.get(name) {
            return Some(val);
        }
        self.values
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v)
    }

    /// Case-insensitive mutable search for value
    pub fn get_value_mut(&mut self, name: &str) -> Option<&mut RegistryValue> {
        if name.is_empty() || name == "(Default)" {
            return self.values.get_mut("");
        }
        if self.values.contains_key(name) {
            return self.values.get_mut(name);
        }
        let matching_key = self
            .values
            .keys()
            .find(|k| k.eq_ignore_ascii_case(name))
            .cloned();

        if let Some(key) = matching_key {
            self.values.get_mut(&key)
        } else {
            None
        }
    }

    pub fn set_value(&mut self, val: RegistryValue) {
        let key = if val.name == "(Default)" {
            String::new()
        } else {
            val.name.clone()
        };
        self.values.insert(key, val);
    }

    pub fn remove_value(&mut self, name: &str) -> bool {
        let key_to_remove = if name.is_empty() || name == "(Default)" {
            "".to_string()
        } else {
            self.values
                .keys()
                .find(|k| k.eq_ignore_ascii_case(name))
                .cloned()
                .unwrap_or_else(|| name.to_string())
        };

        if key_to_remove.is_empty() {
            // Reset default value to empty rather than removing key
            self.values.insert(String::new(), RegistryValue::new_default_empty());
            true
        } else {
            self.values.remove(&key_to_remove).is_some()
        }
    }
}

/// The complete Registry Tree holding all 5 root hives.
#[derive(Debug, Clone)]
pub struct RegistryTree {
    pub roots: BTreeMap<String, RegistryKeyNode>,
}

impl RegistryTree {
    pub fn new() -> Self {
        let mut roots = BTreeMap::new();
        for hive in ROOT_HIVES {
            roots.insert(hive.to_string(), RegistryKeyNode::new(hive));
        }
        Self { roots }
    }

    /// Initialize with typical Windows 11 / HybridOS system registry entries
    pub fn with_default_system_keys() -> Self {
        let mut tree = Self::new();

        // 1. HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion
        let cur_ver = "Computer\\HKEY_LOCAL_MACHINE\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion";
        let _ = tree.create_key(cur_ver);
        let _ = tree.set_value(cur_ver, RegistryValue::new_sz("ProductName", "HybridOS 11 Pro Commercial"));
        let _ = tree.set_value(cur_ver, RegistryValue::new_sz("CurrentBuild", "26100"));
        let _ = tree.set_value(cur_ver, RegistryValue::new_sz("DisplayVersion", "24H2"));
        let _ = tree.set_value(cur_ver, RegistryValue::new_sz("EditionID", "Professional"));
        let _ = tree.set_value(cur_ver, RegistryValue::new_sz("InstallationType", "Client"));
        let _ = tree.set_value(cur_ver, RegistryValue::new_dword("InstallDate", 1731234567));

        // 2. HKLM\SYSTEM\CurrentControlSet\Services\HybridKernel
        let srv = "Computer\\HKEY_LOCAL_MACHINE\\SYSTEM\\CurrentControlSet\\Services\\HybridKernel";
        let _ = tree.create_key(srv);
        let _ = tree.set_value(srv, RegistryValue::new_sz("DisplayName", "HybridOS Dual-Personality Kernel Service"));
        let _ = tree.set_value(srv, RegistryValue::new_dword("Start", 2));
        let _ = tree.set_value(srv, RegistryValue::new_dword("Type", 1));
        let _ = tree.set_value(srv, RegistryValue::new_expand_sz("ImagePath", "%SystemRoot%\\System32\\drivers\\hybridos.sys"));

        // 3. HKLM\SYSTEM\CurrentControlSet\Control
        let ctrl = "Computer\\HKEY_LOCAL_MACHINE\\SYSTEM\\CurrentControlSet\\Control";
        let _ = tree.create_key(ctrl);
        let _ = tree.set_value(ctrl, RegistryValue::new_sz("SystemStartOptions", "NOEXECUTE=OPTIN HIGHRES=1"));
        let _ = tree.set_value(ctrl, RegistryValue::new_sz("CurrentUser", "Administrator"));

        // 4. HKLM\HARDWARE\DESCRIPTION\System\CentralProcessor\0
        let cpu = "Computer\\HKEY_LOCAL_MACHINE\\HARDWARE\\DESCRIPTION\\System\\CentralProcessor\\0";
        let _ = tree.create_key(cpu);
        let _ = tree.set_value(cpu, RegistryValue::new_sz("ProcessorNameString", "HybridOS Virtual High-Throughput Processor 4.20GHz"));
        let _ = tree.set_value(cpu, RegistryValue::new_dword("~MHz", 4200));
        let _ = tree.set_value(cpu, RegistryValue::new_sz("Identifier", "Intel64 Family 6 Model 158 Stepping 10"));

        // 5. HKCU\Control Panel\Desktop
        let desktop = "Computer\\HKEY_CURRENT_USER\\Control Panel\\Desktop";
        let _ = tree.create_key(desktop);
        let _ = tree.set_value(desktop, RegistryValue::new_sz("Wallpaper", "C:\\Windows\\Web\\Wallpaper\\Windows\\img0.jpg"));
        let _ = tree.set_value(desktop, RegistryValue::new_sz("FontSmoothing", "2"));
        let _ = tree.set_value(desktop, RegistryValue::new_dword("ScreenSaveActive", 1));
        let _ = tree.set_value(desktop, RegistryValue::new_dword("ScreenSaveTimeOut", 900));

        // 6. HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced
        let explorer = "Computer\\HKEY_CURRENT_USER\\Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\Advanced";
        let _ = tree.create_key(explorer);
        let _ = tree.set_value(explorer, RegistryValue::new_dword("Hidden", 1));
        let _ = tree.set_value(explorer, RegistryValue::new_dword("ShowSuperHidden", 0));
        let _ = tree.set_value(explorer, RegistryValue::new_dword("TaskbarAl", 1)); // Win11 Center Aligned Taskbar

        // 7. HKCR\.txt
        let txt = "Computer\\HKEY_CLASSES_ROOT\\.txt";
        let _ = tree.create_key(txt);
        let _ = tree.set_value(txt, RegistryValue::new_sz("", "txtfile"));
        let _ = tree.set_value(txt, RegistryValue::new_sz("Content Type", "text/plain"));
        let _ = tree.set_value(txt, RegistryValue::new_sz("PerceivedType", "text"));

        // 8. HKCC\System\CurrentControlSet\Control\Print
        let print = "Computer\\HKEY_CURRENT_CONFIG\\System\\CurrentControlSet\\Control\\Print";
        let _ = tree.create_key(print);
        let _ = tree.set_value(print, RegistryValue::new_sz("Printers", "HybridPDF Virtual Printer"));

        tree
    }

    /// Retrieve a reference to a key at the specified path
    pub fn get_key(&self, path: &str) -> Option<&RegistryKeyNode> {
        let parts = parse_registry_path(path);
        if parts.is_empty() {
            return None;
        }
        let hive_name = &parts[0];
        let mut current = self.roots.get(hive_name)?;

        for part in &parts[1..] {
            current = current.get_subkey(part)?;
        }
        Some(current)
    }

    /// Retrieve a mutable reference to a key at the specified path
    pub fn get_key_mut(&mut self, path: &str) -> Option<&mut RegistryKeyNode> {
        let parts = parse_registry_path(path);
        if parts.is_empty() {
            return None;
        }
        let hive_name = &parts[0];
        let mut current = self.roots.get_mut(hive_name)?;

        for part in &parts[1..] {
            current = current.get_subkey_mut(part)?;
        }
        Some(current)
    }

    /// Create a key (and all required intermediate parent keys)
    pub fn create_key(&mut self, path: &str) -> Result<(), RegistryError> {
        let parts = parse_registry_path(path);
        if parts.is_empty() {
            return Err(RegistryError::InvalidPath(path.to_string()));
        }
        let hive_name = &parts[0];
        let mut current = self
            .roots
            .get_mut(hive_name)
            .ok_or_else(|| RegistryError::KeyNotFound(hive_name.clone()))?;

        for part in &parts[1..] {
            if part.is_empty() {
                continue;
            }
            if !current.subkeys.contains_key(part) {
                current.subkeys.insert(part.clone(), RegistryKeyNode::new(part));
            }
            current = current.get_subkey_mut(part).unwrap();
        }
        Ok(())
    }

    /// Delete a key and all of its subtrees
    pub fn delete_key(&mut self, path: &str) -> Result<(), RegistryError> {
        let parts = parse_registry_path(path);
        if parts.is_empty() {
            return Err(RegistryError::InvalidPath(path.to_string()));
        }
        if parts.len() == 1 {
            return Err(RegistryError::CannotDeleteRootHive(parts[0].clone()));
        }

        let hive_name = &parts[0];
        let target_name = parts.last().unwrap();
        let parent_parts = &parts[..parts.len() - 1];

        let mut current = self
            .roots
            .get_mut(hive_name)
            .ok_or_else(|| RegistryError::KeyNotFound(hive_name.clone()))?;

        for part in &parent_parts[1..] {
            current = current
                .get_subkey_mut(part)
                .ok_or_else(|| RegistryError::KeyNotFound(part.clone()))?;
        }

        // Find key with case-insensitive match
        let key_to_remove = current
            .subkeys
            .keys()
            .find(|k| k.eq_ignore_ascii_case(target_name))
            .cloned()
            .ok_or_else(|| RegistryError::KeyNotFound(target_name.clone()))?;

        current.subkeys.remove(&key_to_remove);
        Ok(())
    }

    /// Rename an existing key
    pub fn rename_key(&mut self, path: &str, new_name: &str) -> Result<String, RegistryError> {
        if new_name.trim().is_empty() || new_name.contains('\\') || new_name.contains('/') {
            return Err(RegistryError::InvalidName(new_name.to_string()));
        }

        let parts = parse_registry_path(path);
        if parts.is_empty() {
            return Err(RegistryError::InvalidPath(path.to_string()));
        }
        if parts.len() == 1 {
            return Err(RegistryError::CannotDeleteRootHive(parts[0].clone()));
        }

        let hive_name = &parts[0];
        let old_name = parts.last().unwrap();
        let parent_parts = &parts[..parts.len() - 1];

        let mut current = self
            .roots
            .get_mut(hive_name)
            .ok_or_else(|| RegistryError::KeyNotFound(hive_name.clone()))?;

        for part in &parent_parts[1..] {
            current = current
                .get_subkey_mut(part)
                .ok_or_else(|| RegistryError::KeyNotFound(part.clone()))?;
        }

        if current.subkeys.contains_key(new_name) && !new_name.eq_ignore_ascii_case(old_name) {
            return Err(RegistryError::KeyAlreadyExists(new_name.to_string()));
        }

        let old_key = current
            .subkeys
            .keys()
            .find(|k| k.eq_ignore_ascii_case(old_name))
            .cloned()
            .ok_or_else(|| RegistryError::KeyNotFound(old_name.clone()))?;

        let mut node = current.subkeys.remove(&old_key).unwrap();
        node.name = new_name.to_string();
        current.subkeys.insert(new_name.to_string(), node);

        // Build new path
        let mut new_path_parts = parent_parts.to_vec();
        new_path_parts.push(new_name.to_string());
        Ok(format!("{}\\{}", COMPUTER_ROOT, new_path_parts.join("\\")))
    }

    /// Set or update a value in a key
    pub fn set_value(&mut self, key_path: &str, value: RegistryValue) -> Result<(), RegistryError> {
        let key = self
            .get_key_mut(key_path)
            .ok_or_else(|| RegistryError::KeyNotFound(key_path.to_string()))?
            ;
        key.set_value(value);
        Ok(())
    }

    /// Delete a value from a key
    pub fn delete_value(&mut self, key_path: &str, value_name: &str) -> Result<(), RegistryError> {
        let key = self
            .get_key_mut(key_path)
            .ok_or_else(|| RegistryError::KeyNotFound(key_path.to_string()))?;
        if key.remove_value(value_name) {
            Ok(())
        } else {
            Err(RegistryError::ValueNotFound(value_name.to_string()))
        }
    }

    /// Rename an existing value inside a key
    pub fn rename_value(
        &mut self,
        key_path: &str,
        old_name: &str,
        new_name: &str,
    ) -> Result<(), RegistryError> {
        if new_name.trim().is_empty() {
            return Err(RegistryError::InvalidName(new_name.to_string()));
        }
        let key = self
            .get_key_mut(key_path)
            .ok_or_else(|| RegistryError::KeyNotFound(key_path.to_string()))?;

        let old_key = key
            .values
            .keys()
            .find(|k| k.eq_ignore_ascii_case(old_name))
            .cloned()
            .ok_or_else(|| RegistryError::ValueNotFound(old_name.to_string()))?;

        if key.values.contains_key(new_name) && !new_name.eq_ignore_ascii_case(&old_key) {
            return Err(RegistryError::ValueAlreadyExists(new_name.to_string()));
        }

        let mut val = key.values.remove(&old_key).unwrap();
        val.name = new_name.to_string();
        key.values.insert(new_name.to_string(), val);
        Ok(())
    }
}

// ============================================================================
// 5. Path Parsing, Normalization & Breadcrumbs
// ============================================================================

/// Normalizes any input path into standard `Computer\HKEY_...` canonical format
pub fn normalize_registry_path(input: &str) -> String {
    let clean = input.trim().replace('/', "\\");
    let parts: Vec<&str> = clean
        .split('\\')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    if parts.is_empty() {
        return COMPUTER_ROOT.to_string();
    }

    let mut start_idx = 0;
    if parts[0].eq_ignore_ascii_case(COMPUTER_ROOT) {
        start_idx = 1;
    }

    if start_idx >= parts.len() {
        return COMPUTER_ROOT.to_string();
    }

    let root_str = parts[start_idx];
    let resolved_root = match root_str.to_ascii_uppercase().as_str() {
        "HKCR" | "HKEY_CLASSES_ROOT" => HKCR,
        "HKCU" | "HKEY_CURRENT_USER" => HKCU,
        "HKLM" | "HKEY_LOCAL_MACHINE" => HKLM,
        "HKU" | "HKEY_USERS" => HKU,
        "HKCC" | "HKEY_CURRENT_CONFIG" => HKCC,
        other => {
            // Check matching root hive
            let found = ROOT_HIVES
                .iter()
                .find(|h| h.eq_ignore_ascii_case(other))
                .copied();
            match found {
                Some(h) => h,
                None => return format!("{}\\{}", COMPUTER_ROOT, parts[start_idx..].join("\\")),
            }
        }
    };

    let mut result = vec![COMPUTER_ROOT, resolved_root];
    for &p in &parts[start_idx + 1..] {
        result.push(p);
    }
    result.join("\\")
}

/// Parses a path into hive and subkey components (ignoring "Computer" prefix)
pub fn parse_registry_path(path: &str) -> Vec<String> {
    let normalized = normalize_registry_path(path);
    let parts: Vec<&str> = normalized.split('\\').collect();
    if parts.len() <= 1 {
        return Vec::new();
    }
    // parts[0] is "Computer"
    parts[1..].iter().map(|s| s.to_string()).collect()
}

/// A breadcrumb segment representing a clickable segment in the address bar
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BreadcrumbItem {
    pub display_name: String,
    pub target_path: String,
}

pub fn generate_breadcrumbs(canonical_path: &str) -> Vec<BreadcrumbItem> {
    let normalized = normalize_registry_path(canonical_path);
    let parts: Vec<&str> = normalized.split('\\').collect();
    let mut crumbs = Vec::new();
    let mut accumulated = Vec::new();

    for part in parts {
        accumulated.push(part);
        crumbs.push(BreadcrumbItem {
            display_name: part.to_string(),
            target_path: accumulated.join("\\"),
        });
    }
    crumbs
}

// ============================================================================
// 6. Value Editing Dialogs State & Implementations
// ============================================================================

/// Numeric base switcher for DWORD / QWORD editing dialogs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DwordBase {
    Hexadecimal,
    Decimal,
}

impl DwordBase {
    pub fn name(self) -> &'static str {
        match self {
            Self::Hexadecimal => "Hexadecimal",
            Self::Decimal => "Decimal",
        }
    }
}

/// Edit String Dialog (REG_SZ / REG_EXPAND_SZ)
#[derive(Debug, Clone)]
pub struct EditStringDialog {
    pub key_path: String,
    pub value_name: String,
    pub value_data: String,
    pub value_type: RegistryValueType,
}

impl EditStringDialog {
    pub fn new(key_path: &str, val: &RegistryValue) -> Self {
        Self {
            key_path: key_path.to_string(),
            value_name: val.name.clone(),
            value_data: val.decode_string(),
            value_type: val.value_type,
        }
    }

    pub fn set_data(&mut self, text: &str) {
        self.value_data = text.to_string();
    }

    pub fn commit(&self) -> RegistryValue {
        if self.value_type == RegistryValueType::RegExpandSz {
            RegistryValue::new_expand_sz(&self.value_name, &self.value_data)
        } else {
            RegistryValue::new_sz(&self.value_name, &self.value_data)
        }
    }
}

/// Edit DWORD (32-bit) & QWORD (64-bit) Dialog
#[derive(Debug, Clone)]
pub struct EditDwordDialog {
    pub key_path: String,
    pub value_name: String,
    pub is_qword: bool,
    pub base: DwordBase,
    pub input_text: String,
    pub current_raw_val: u64,
}

impl EditDwordDialog {
    pub fn new(key_path: &str, val: &RegistryValue) -> Self {
        let is_qword = val.value_type == RegistryValueType::RegQword;
        let num = if is_qword {
            val.decode_qword()
        } else {
            val.decode_dword() as u64
        };
        let input_text = if is_qword {
            format!("{:x}", num)
        } else {
            format!("{:x}", num as u32)
        };

        Self {
            key_path: key_path.to_string(),
            value_name: val.name.clone(),
            is_qword,
            base: DwordBase::Hexadecimal,
            input_text,
            current_raw_val: num,
        }
    }

    /// Switching base switches between hexadecimal and decimal representation seamlessly
    pub fn set_base(&mut self, new_base: DwordBase) {
        if self.base == new_base {
            return;
        }
        // First sync current input to raw val if valid
        let _ = self.sync_input_to_raw();
        self.base = new_base;
        self.input_text = match new_base {
            DwordBase::Hexadecimal => {
                if self.is_qword {
                    format!("{:x}", self.current_raw_val)
                } else {
                    format!("{:x}", self.current_raw_val as u32)
                }
            }
            DwordBase::Decimal => {
                if self.is_qword {
                    format!("{}", self.current_raw_val)
                } else {
                    format!("{}", self.current_raw_val as u32)
                }
            }
        };
    }

    pub fn set_input_text(&mut self, text: &str) -> Result<(), String> {
        self.input_text = text.trim().to_string();
        self.sync_input_to_raw()
    }

    fn sync_input_to_raw(&mut self) -> Result<(), String> {
        let text = self.input_text.trim();
        if text.is_empty() {
            self.current_raw_val = 0;
            return Ok(());
        }
        let parsed = match self.base {
            DwordBase::Hexadecimal => {
                let clean = text.strip_prefix("0x").unwrap_or(text);
                u64::from_str_radix(clean, 16)
                    .map_err(|e| format!("Invalid hexadecimal value: {}", e))?
            }
            DwordBase::Decimal => text
                .parse::<u64>()
                .map_err(|e| format!("Invalid decimal value: {}", e))?,
        };

        if !self.is_qword && parsed > u32::MAX as u64 {
            return Err("Value exceeds 32-bit DWORD range (0 to 4294967295)".to_string());
        }
        self.current_raw_val = parsed;
        Ok(())
    }

    pub fn commit(&self) -> Result<RegistryValue, String> {
        let mut cloned = self.clone();
        cloned.sync_input_to_raw()?;
        if self.is_qword {
            Ok(RegistryValue::new_qword(
                &self.value_name,
                cloned.current_raw_val,
            ))
        } else {
            Ok(RegistryValue::new_dword(
                &self.value_name,
                cloned.current_raw_val as u32,
            ))
        }
    }
}

/// A formatted hex dump line for the Binary Editor
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HexDumpRow {
    pub offset: usize,
    pub offset_label: String,
    pub hex_bytes: Vec<String>,
    pub ascii_chars: String,
}

/// Edit Binary Data Dialog (Byte hex editor grid)
#[derive(Debug, Clone)]
pub struct EditBinaryDialog {
    pub key_path: String,
    pub value_name: String,
    pub data: Vec<u8>,
    pub cursor_byte_idx: usize,
    pub is_hex_column: bool,
}

impl EditBinaryDialog {
    pub fn new(key_path: &str, val: &RegistryValue) -> Self {
        Self {
            key_path: key_path.to_string(),
            value_name: val.name.clone(),
            data: val.raw_data.clone(),
            cursor_byte_idx: 0,
            is_hex_column: true,
        }
    }

    pub fn rows(&self) -> Vec<HexDumpRow> {
        let mut rows = Vec::new();
        if self.data.is_empty() {
            rows.push(HexDumpRow {
                offset: 0,
                offset_label: "00000000".to_string(),
                hex_bytes: Vec::new(),
                ascii_chars: String::new(),
            });
            return rows;
        }

        for (chunk_idx, chunk) in self.data.chunks(8).enumerate() {
            let offset = chunk_idx * 8;
            let offset_label = format!("{:08x}", offset);
            let hex_bytes = chunk.iter().map(|b| format!("{:02x}", b)).collect();
            let ascii_chars: String = chunk
                .iter()
                .map(|&b| if (0x20..=0x7e).contains(&b) { b as char } else { '.' })
                .collect();

            rows.push(HexDumpRow {
                offset,
                offset_label,
                hex_bytes,
                ascii_chars,
            });
        }
        rows
    }

    pub fn insert_byte(&mut self, byte: u8) {
        if self.cursor_byte_idx <= self.data.len() {
            self.data.insert(self.cursor_byte_idx, byte);
            self.cursor_byte_idx += 1;
        }
    }

    pub fn delete_byte(&mut self) {
        if self.cursor_byte_idx < self.data.len() {
            self.data.remove(self.cursor_byte_idx);
        } else if self.cursor_byte_idx > 0 && self.cursor_byte_idx == self.data.len() {
            self.cursor_byte_idx -= 1;
            self.data.remove(self.cursor_byte_idx);
        }
    }

    pub fn update_byte_at_cursor(&mut self, byte: u8) {
        if self.cursor_byte_idx < self.data.len() {
            self.data[self.cursor_byte_idx] = byte;
        } else {
            self.data.push(byte);
        }
    }

    pub fn set_data_from_hex_string(&mut self, hex: &str) -> Result<(), String> {
        let clean: String = hex.chars().filter(|c| !c.is_whitespace() && *c != ',').collect();
        if clean.len() % 2 != 0 {
            return Err("Hex string must have an even number of characters".to_string());
        }
        let mut new_data = Vec::new();
        for i in (0..clean.len()).step_by(2) {
            let byte_str = &clean[i..i + 2];
            let byte = u8::from_str_radix(byte_str, 16)
                .map_err(|e| format!("Invalid hex byte '{}': {}", byte_str, e))?;
            new_data.push(byte);
        }
        self.data = new_data;
        if self.cursor_byte_idx > self.data.len() {
            self.cursor_byte_idx = self.data.len();
        }
        Ok(())
    }

    pub fn commit(&self) -> RegistryValue {
        RegistryValue::new_binary(&self.value_name, &self.data)
    }
}

/// Edit Multi-String Dialog (Multi-line text area)
#[derive(Debug, Clone)]
pub struct EditMultiStringDialog {
    pub key_path: String,
    pub value_name: String,
    pub lines: Vec<String>,
}

impl EditMultiStringDialog {
    pub fn new(key_path: &str, val: &RegistryValue) -> Self {
        Self {
            key_path: key_path.to_string(),
            value_name: val.name.clone(),
            lines: val.decode_multi_sz(),
        }
    }

    pub fn set_text(&mut self, full_text: &str) {
        self.lines = full_text
            .lines()
            .map(|s| s.trim_end().to_string())
            .filter(|s| !s.is_empty())
            .collect();
    }

    pub fn get_text(&self) -> String {
        self.lines.join("\n")
    }

    pub fn commit(&self) -> RegistryValue {
        RegistryValue::new_multi_sz(&self.value_name, &self.lines)
    }
}

/// Target type for Delete confirmation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeleteTargetType {
    Key,
    Value,
}

/// Confirmation dialog for Key or Value deletion
#[derive(Debug, Clone)]
pub struct ConfirmDeleteDialog {
    pub target_type: DeleteTargetType,
    pub path: String,
    pub item_name: String,
    pub message: String,
}

impl ConfirmDeleteDialog {
    pub fn for_key(path: &str) -> Self {
        let key_name = path.split('\\').last().unwrap_or(path);
        Self {
            target_type: DeleteTargetType::Key,
            path: path.to_string(),
            item_name: key_name.to_string(),
            message: format!(
                "Are you sure you want to permanently delete this key and all of its subkeys?\nKey: {}",
                path
            ),
        }
    }

    pub fn for_value(key_path: &str, value_name: &str) -> Self {
        let display = if value_name.is_empty() {
            "(Default)"
        } else {
            value_name
        };
        Self {
            target_type: DeleteTargetType::Value,
            path: key_path.to_string(),
            item_name: value_name.to_string(),
            message: format!(
                "Deleting certain registry values could cause system instability. Are you sure you want to permanently delete this value?\nValue: {}",
                display
            ),
        }
    }
}

/// Rename key or value dialog
#[derive(Debug, Clone)]
pub struct RenameDialog {
    pub is_key: bool,
    pub parent_path: String,
    pub old_name: String,
    pub new_name: String,
}

// ============================================================================
// 7. Search / Find Engine (Ctrl+F, F3)
// ============================================================================

/// Search criteria configuration
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FindOptions {
    pub query: String,
    pub look_at_keys: bool,
    pub look_at_values: bool,
    pub look_at_data: bool,
    pub match_whole_string: bool,
    pub match_case: bool,
}

impl Default for FindOptions {
    fn default() -> Self {
        Self {
            query: String::new(),
            look_at_keys: true,
            look_at_values: true,
            look_at_data: true,
            match_whole_string: false,
            match_case: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchedField {
    KeyName,
    ValueName,
    ValueData,
}

/// Result of a search match
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchResult {
    pub key_path: String,
    pub value_name: Option<String>,
    pub matched_field: MatchedField,
    pub matched_text: String,
}

/// Internal flat item used for sequential navigation
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlattenedItem {
    pub key_path: String,
    pub value_name: Option<String>,
}

impl RegistryTree {
    /// Collect all keys and values in depth-first traversal order
    pub fn collect_all_flattened_items(&self) -> Vec<FlattenedItem> {
        let mut items = Vec::new();
        for (hive_name, hive_node) in &self.roots {
            let path = format!("{}\\{}", COMPUTER_ROOT, hive_name);
            Self::recurse_flatten(&path, hive_node, &mut items);
        }
        items
    }

    fn recurse_flatten(
        current_path: &str,
        node: &RegistryKeyNode,
        out: &mut Vec<FlattenedItem>,
    ) {
        out.push(FlattenedItem {
            key_path: current_path.to_string(),
            value_name: None,
        });

        for val_name in node.values.keys() {
            out.push(FlattenedItem {
                key_path: current_path.to_string(),
                value_name: Some(val_name.clone()),
            });
        }

        for (sub_name, sub_node) in &node.subkeys {
            let child_path = format!("{}\\{}", current_path, sub_name);
            Self::recurse_flatten(&child_path, sub_node, out);
        }
    }

    /// Search for matches given search options
    pub fn find_matches(&self, options: &FindOptions) -> Vec<SearchResult> {
        if options.query.trim().is_empty() {
            return Vec::new();
        }
        let items = self.collect_all_flattened_items();
        let mut results = Vec::new();

        for item in items {
            if let Some(val_name) = &item.value_name {
                if let Some(key_node) = self.get_key(&item.key_path) {
                    if let Some(val) = key_node.get_value(val_name) {
                        // Check value name
                        if options.look_at_values {
                            let test_name = val.display_name();
                            if check_match(test_name, &options.query, options.match_whole_string, options.match_case) {
                                results.push(SearchResult {
                                    key_path: item.key_path.clone(),
                                    value_name: Some(val.name.clone()),
                                    matched_field: MatchedField::ValueName,
                                    matched_text: test_name.to_string(),
                                });
                                continue;
                            }
                        }
                        // Check value data
                        if options.look_at_data {
                            let data_str = val.format_display_data();
                            if check_match(&data_str, &options.query, options.match_whole_string, options.match_case) {
                                results.push(SearchResult {
                                    key_path: item.key_path.clone(),
                                    value_name: Some(val.name.clone()),
                                    matched_field: MatchedField::ValueData,
                                    matched_text: data_str,
                                });
                            }
                        }
                    }
                }
            } else {
                // Key node match
                if options.look_at_keys {
                    let key_name = item.key_path.split('\\').last().unwrap_or(&item.key_path);
                    if check_match(key_name, &options.query, options.match_whole_string, options.match_case) {
                        results.push(SearchResult {
                            key_path: item.key_path.clone(),
                            value_name: None,
                            matched_field: MatchedField::KeyName,
                            matched_text: key_name.to_string(),
                        });
                    }
                }
            }
        }
        results
    }
}

fn check_match(target: &str, query: &str, match_whole: bool, match_case: bool) -> bool {
    if match_whole {
        if match_case {
            target == query
        } else {
            target.eq_ignore_ascii_case(query)
        }
    } else {
        if match_case {
            target.contains(query)
        } else {
            target
                .to_ascii_lowercase()
                .contains(&query.to_ascii_lowercase())
        }
    }
}

// ============================================================================
// 8. Windows Registry Editor Version 5.00 Export & Import (.reg)
// ============================================================================

pub const REG_HEADER_V5: &str = "Windows Registry Editor Version 5.00";

impl RegistryTree {
    /// Export an entire subtree or branch to standard .reg v5 file format
    pub fn export_subtree(&self, root_path: &str) -> Result<String, RegistryError> {
        let mut out = String::new();
        out.push_str(REG_HEADER_V5);
        out.push_str("\r\n\r\n");

        if root_path.eq_ignore_ascii_case(COMPUTER_ROOT) || root_path.is_empty() {
            // Export all hives
            for (hive_name, hive_node) in &self.roots {
                Self::export_node_recursive(hive_name, hive_node, &mut out)?;
            }
        } else {
            let key = self
                .get_key(root_path)
                .ok_or_else(|| RegistryError::KeyNotFound(root_path.to_string()))?;

            let export_header_path = root_path
                .strip_prefix(COMPUTER_ROOT)
                .unwrap_or(root_path)
                .trim_start_matches('\\');

            Self::export_node_recursive(export_header_path, key, &mut out)?;
        }

        Ok(out)
    }

    fn export_node_recursive(
        path: &str,
        node: &RegistryKeyNode,
        out: &mut String,
    ) -> Result<(), RegistryError> {
        out.push('[');
        out.push_str(path);
        out.push_str("]\r\n");

        // 1. Export (Default) value first if set
        if let Some(def_val) = node.values.get("") {
            if !def_val.raw_data.is_empty() {
                Self::export_single_value(out, "@", def_val);
            }
        }

        // 2. Export named values in alphabetical order
        for (name, val) in &node.values {
            if name.is_empty() {
                continue;
            }
            let escaped_name = format!("\"{}\"", escape_reg_string(name));
            Self::export_single_value(out, &escaped_name, val);
        }

        out.push_str("\r\n");

        // 3. Recurse into subkeys
        for (sub_name, sub_node) in &node.subkeys {
            let sub_path = format!("{}\\{}", path, sub_name);
            Self::export_node_recursive(&sub_path, sub_node, out)?;
        }

        Ok(())
    }

    fn export_single_value(out: &mut String, prefix: &str, val: &RegistryValue) {
        match val.value_type {
            RegistryValueType::RegSz => {
                let text = val.decode_string();
                let escaped = escape_reg_string(&text);
                out.push_str(&format!("{}=\"{}\"\r\n", prefix, escaped));
            }
            RegistryValueType::RegDword => {
                let d = val.decode_dword();
                out.push_str(&format!("{}=dword:{:08x}\r\n", prefix, d));
            }
            RegistryValueType::RegBinary => {
                let formatted = format_hex_byte_stream(&val.raw_data, prefix, "hex:");
                out.push_str(&formatted);
            }
            RegistryValueType::RegExpandSz => {
                let formatted = format_hex_byte_stream(&val.raw_data, prefix, "hex(2):");
                out.push_str(&formatted);
            }
            RegistryValueType::RegMultiSz => {
                let formatted = format_hex_byte_stream(&val.raw_data, prefix, "hex(7):");
                out.push_str(&formatted);
            }
            RegistryValueType::RegQword => {
                let formatted = format_hex_byte_stream(&val.raw_data, prefix, "hex(b):");
                out.push_str(&formatted);
            }
            RegistryValueType::RegNone => {
                let formatted = format_hex_byte_stream(&val.raw_data, prefix, "hex(0):");
                out.push_str(&formatted);
            }
            other => {
                let type_code = format!("hex({:x}):", other.to_u32());
                let formatted = format_hex_byte_stream(&val.raw_data, prefix, &type_code);
                out.push_str(&formatted);
            }
        }
    }

    /// Import a .reg file text stream
    pub fn import_reg_file(&mut self, content: &str) -> Result<usize, RegistryError> {
        let lines = preprocess_reg_lines(content)?;
        let mut count = 0;
        let mut current_key_path: Option<String> = None;

        for line in lines {
            let line = line.trim();
            if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
                continue;
            }

            // Key Header: [HKEY_...] or [-HKEY_...]
            if line.starts_with('[') && line.ends_with(']') {
                let raw_key = &line[1..line.len() - 1].trim();
                if raw_key.starts_with('-') {
                    // Delete key command: [-HKEY_...]
                    let delete_target = &raw_key[1..];
                    let canonical = normalize_registry_path(delete_target);
                    let _ = self.delete_key(&canonical);
                    current_key_path = None;
                } else {
                    let canonical = normalize_registry_path(raw_key);
                    self.create_key(&canonical)?;
                    current_key_path = Some(canonical);
                    count += 1;
                }
                continue;
            }

            // Must be inside a key to set values
            let key_path = match &current_key_path {
                Some(kp) => kp.clone(),
                None => continue,
            };

            // Value entry: "Name"=... or @=...
            if let Some(equals_pos) = find_unquoted_equals(line) {
                let name_part = line[..equals_pos].trim();
                let val_part = line[equals_pos + 1..].trim();

                let val_name = if name_part == "@" {
                    String::new()
                } else if name_part.starts_with('"') && name_part.ends_with('"') && name_part.len() >= 2 {
                    unescape_reg_string(&name_part[1..name_part.len() - 1])
                } else {
                    name_part.to_string()
                };

                // Check for value deletion: "Name"=-
                if val_part == "-" {
                    let _ = self.delete_value(&key_path, &val_name);
                    continue;
                }

                let value = parse_reg_value(&val_name, val_part)?;
                self.set_value(&key_path, value)?;
                count += 1;
            }
        }

        Ok(count)
    }
}

fn escape_reg_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
    }
    out
}

fn unescape_reg_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(&next) = chars.peek() {
                chars.next();
                match next {
                    '\\' => out.push('\\'),
                    '"' => out.push('"'),
                    'n' => out.push('\n'),
                    'r' => out.push('\r'),
                    't' => out.push('\t'),
                    other => {
                        out.push('\\');
                        out.push(other);
                    }
                }
            } else {
                out.push('\\');
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn format_hex_byte_stream(bytes: &[u8], prefix: &str, type_prefix: &str) -> String {
    if bytes.is_empty() {
        return format!("{}={}\r\n", prefix, type_prefix);
    }
    let mut out = format!("{}={}", prefix, type_prefix);
    let mut current_line_len = out.len();

    for (i, &b) in bytes.iter().enumerate() {
        let byte_str = format!("{:02x}", b);
        let separator = if i + 1 == bytes.len() { "" } else { "," };
        let piece = format!("{}{}", byte_str, separator);

        if current_line_len + piece.len() > 76 && i + 1 < bytes.len() {
            out.push_str("\\\r\n  ");
            current_line_len = 2;
        }

        out.push_str(&piece);
        current_line_len += piece.len();
    }
    out.push_str("\r\n");
    out
}

fn preprocess_reg_lines(content: &str) -> Result<Vec<String>, RegistryError> {
    let clean = content.strip_prefix('\u{feff}').unwrap_or(content);
    let mut raw_lines = clean.lines();

    // Verify header
    let first = raw_lines
        .find(|l| !l.trim().is_empty())
        .ok_or_else(|| RegistryError::ParseError("Empty .reg file".to_string()))?;

    if !first.contains("Windows Registry Editor Version 5.00") && !first.contains("REGEDIT4") {
        return Err(RegistryError::ParseError(format!(
            "Invalid .reg header: '{}'",
            first
        )));
    }

    let mut result = Vec::new();
    let mut current_combined = String::new();

    for line in raw_lines {
        let trimmed = line.trim_end();
        if trimmed.ends_with('\\') {
            let slice = &trimmed[..trimmed.len() - 1].trim_end();
            if current_combined.is_empty() {
                current_combined.push_str(slice);
            } else {
                current_combined.push_str(slice.trim_start());
            }
        } else {
            if current_combined.is_empty() {
                result.push(trimmed.to_string());
            } else {
                current_combined.push_str(trimmed.trim_start());
                result.push(current_combined.clone());
                current_combined.clear();
            }
        }
    }
    if !current_combined.is_empty() {
        result.push(current_combined);
    }
    Ok(result)
}

fn find_unquoted_equals(s: &str) -> Option<usize> {
    let mut in_quotes = false;
    let mut escaped = false;
    for (idx, c) in s.char_indices() {
        if c == '\\' && in_quotes {
            escaped = !escaped;
            continue;
        }
        if c == '"' && !escaped {
            in_quotes = !in_quotes;
        }
        if c == '=' && !in_quotes {
            return Some(idx);
        }
        escaped = false;
    }
    None
}

fn parse_reg_value(name: &str, raw_val: &str) -> Result<RegistryValue, RegistryError> {
    if raw_val.starts_with('"') && raw_val.ends_with('"') && raw_val.len() >= 2 {
        let unescaped = unescape_reg_string(&raw_val[1..raw_val.len() - 1]);
        return Ok(RegistryValue::new_sz(name, &unescaped));
    }

    if let Some(rest) = raw_val.strip_prefix("dword:") {
        let clean = rest.trim();
        let val = u32::from_str_radix(clean, 16).map_err(|e| {
            RegistryError::ParseError(format!("Invalid dword '{}': {}", clean, e))
        })?;
        return Ok(RegistryValue::new_dword(name, val));
    }

    if let Some(rest) = raw_val.strip_prefix("hex:") {
        let bytes = parse_hex_byte_list(rest)?;
        return Ok(RegistryValue::new_binary(name, &bytes));
    }

    if raw_val.starts_with("hex(") {
        let close_paren = raw_val.find(')').ok_or_else(|| {
            RegistryError::ParseError(format!("Malformed hex type specifier: '{}'", raw_val))
        })?;
        let type_hex = &raw_val[4..close_paren].trim();
        let type_code = u32::from_str_radix(type_hex, 16).map_err(|e| {
            RegistryError::ParseError(format!("Invalid hex type code '{}': {}", type_hex, e))
        })?;
        let colon_pos = raw_val[close_paren..].find(':').ok_or_else(|| {
            RegistryError::ParseError(format!("Missing colon in hex value: '{}'", raw_val))
        })? + close_paren;

        let data_part = &raw_val[colon_pos + 1..];
        let bytes = parse_hex_byte_list(data_part)?;

        let val_type = RegistryValueType::from_u32(type_code);
        return Ok(RegistryValue {
            name: name.to_string(),
            value_type: val_type,
            raw_data: bytes,
        });
    }

    Err(RegistryError::ParseError(format!(
        "Unrecognized value format: '{}'",
        raw_val
    )))
}

fn parse_hex_byte_list(s: &str) -> Result<Vec<u8>, RegistryError> {
    let mut bytes = Vec::new();
    for token in s.split(',') {
        let token = token.trim();
        if token.is_empty() {
            continue;
        }
        let b = u8::from_str_radix(token, 16).map_err(|e| {
            RegistryError::ParseError(format!("Invalid hex byte '{}': {}", token, e))
        })?;
        bytes.push(b);
    }
    Ok(bytes)
}

// ============================================================================
// 9. UI View Models & Regedit Application Component
// ============================================================================

/// Keyboard keys recognized by the Registry Editor
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegeditKey {
    F2,
    F3,
    F5,
    Delete,
    Enter,
    Escape,
    Backspace,
    Up,
    Down,
    Left,
    Right,
    Char(char),
}

/// Tree view row model for UI rendering
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeViewRow {
    pub path: String,
    pub display_name: String,
    pub depth: usize,
    pub has_children: bool,
    pub is_expanded: bool,
    pub is_selected: bool,
}

/// List view row model for registry values
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValueListRow {
    pub name: String,
    pub is_default: bool,
    pub type_name: String,
    pub data_display: String,
    pub is_selected: bool,
}

/// Active dialog in Regedit
#[derive(Debug, Clone)]
pub enum RegeditDialog {
    EditString(EditStringDialog),
    EditDword(EditDwordDialog),
    EditBinary(EditBinaryDialog),
    EditMultiString(EditMultiStringDialog),
    ConfirmDelete(ConfirmDeleteDialog),
    Rename(RenameDialog),
    Find(FindOptions),
}

/// The main Registry Editor state controller
#[derive(Debug, Clone)]
pub struct RegeditApp {
    pub tree: RegistryTree,
    pub selected_path: String,
    pub selected_value_name: Option<String>,
    pub address_bar_input: String,
    pub is_editing_address_bar: bool,
    pub history: Vec<String>,
    pub history_idx: usize,
    pub active_dialog: Option<RegeditDialog>,
    pub status_text: String,
    pub search_options: FindOptions,
    pub last_search_results: Vec<SearchResult>,
    pub search_result_idx: usize,
}

impl RegeditApp {
    pub fn new() -> Self {
        let tree = RegistryTree::with_default_system_keys();
        let initial_path = format!("{}\\{}", COMPUTER_ROOT, HKLM);
        let mut app = Self {
            tree,
            selected_path: initial_path.clone(),
            selected_value_name: None,
            address_bar_input: initial_path.clone(),
            is_editing_address_bar: false,
            history: vec![initial_path.clone()],
            history_idx: 0,
            active_dialog: None,
            status_text: initial_path.clone(),
            search_options: FindOptions::default(),
            last_search_results: Vec::new(),
            search_result_idx: 0,
        };
        // Auto-expand Computer and HKLM
        if let Some(hklm_node) = app.tree.roots.get_mut(HKLM) {
            hklm_node.is_expanded = true;
        }
        app
    }

    // --- Navigation & History ---

    pub fn navigate_to(&mut self, path: &str) {
        let canonical = normalize_registry_path(path);
        // Truncate any forward history
        if self.history_idx + 1 < self.history.len() {
            self.history.truncate(self.history_idx + 1);
        }
        if self.history.last() != Some(&canonical) {
            self.history.push(canonical.clone());
            self.history_idx = self.history.len() - 1;
        }
        self.selected_path = canonical.clone();
        self.address_bar_input = canonical.clone();
        self.selected_value_name = None;
        self.status_text = canonical.clone();
        self.expand_ancestors(&canonical);
    }

    pub fn can_go_back(&self) -> bool {
        self.history_idx > 0
    }

    pub fn can_go_forward(&self) -> bool {
        self.history_idx + 1 < self.history.len()
    }

    pub fn navigate_back(&mut self) {
        if self.can_go_back() {
            self.history_idx -= 1;
            let path = self.history[self.history_idx].clone();
            self.selected_path = path.clone();
            self.address_bar_input = path.clone();
            self.selected_value_name = None;
            self.status_text = path.clone();
            self.expand_ancestors(&path);
        }
    }

    pub fn navigate_forward(&mut self) {
        if self.can_go_forward() {
            self.history_idx += 1;
            let path = self.history[self.history_idx].clone();
            self.selected_path = path.clone();
            self.address_bar_input = path.clone();
            self.selected_value_name = None;
            self.status_text = path.clone();
            self.expand_ancestors(&path);
        }
    }

    pub fn navigate_up(&mut self) {
        let parts: Vec<&str> = self.selected_path.split('\\').collect();
        if parts.len() > 1 {
            let parent = parts[..parts.len() - 1].join("\\");
            self.navigate_to(&parent);
        }
    }

    pub fn submit_address_bar(&mut self) {
        let target = self.address_bar_input.clone();
        self.navigate_to(&target);
        self.is_editing_address_bar = false;
    }

    pub fn breadcrumbs(&self) -> Vec<BreadcrumbItem> {
        generate_breadcrumbs(&self.selected_path)
    }

    fn expand_ancestors(&mut self, path: &str) {
        let parts = parse_registry_path(path);
        if parts.is_empty() {
            return;
        }
        let hive_name = &parts[0];
        if let Some(hive_node) = self.tree.roots.get_mut(hive_name) {
            hive_node.is_expanded = true;
            let mut current = hive_node;
            for part in &parts[1..] {
                if let Some(sub) = current.get_subkey_mut(part) {
                    sub.is_expanded = true;
                    current = sub;
                } else {
                    break;
                }
            }
        }
    }

    // --- Tree & Value View Projection ---

    pub fn generate_tree_rows(&self) -> Vec<TreeViewRow> {
        let mut rows = Vec::new();
        // Virtual root: Computer
        rows.push(TreeViewRow {
            path: COMPUTER_ROOT.to_string(),
            display_name: COMPUTER_ROOT.to_string(),
            depth: 0,
            has_children: true,
            is_expanded: true,
            is_selected: self.selected_path == COMPUTER_ROOT,
        });

        for (hive_name, hive_node) in &self.tree.roots {
            let hive_path = format!("{}\\{}", COMPUTER_ROOT, hive_name);
            self.recurse_tree_rows(&hive_path, hive_node, 1, &mut rows);
        }
        rows
    }

    fn recurse_tree_rows(
        &self,
        current_path: &str,
        node: &RegistryKeyNode,
        depth: usize,
        out: &mut Vec<TreeViewRow>,
    ) {
        let has_children = !node.subkeys.is_empty();
        let is_selected = self.selected_path.eq_ignore_ascii_case(current_path);

        out.push(TreeViewRow {
            path: current_path.to_string(),
            display_name: node.name.clone(),
            depth,
            has_children,
            is_expanded: node.is_expanded,
            is_selected,
        });

        if node.is_expanded {
            for (sub_name, sub_node) in &node.subkeys {
                let child_path = format!("{}\\{}", current_path, sub_name);
                self.recurse_tree_rows(&child_path, sub_node, depth + 1, out);
            }
        }
    }

    pub fn toggle_key_expansion(&mut self, path: &str) {
        let parts = parse_registry_path(path);
        if parts.is_empty() {
            return;
        }
        let hive_name = &parts[0];
        if let Some(hive_node) = self.tree.roots.get_mut(hive_name) {
            if parts.len() == 1 {
                hive_node.is_expanded = !hive_node.is_expanded;
            } else {
                let mut current = hive_node;
                for part in &parts[1..] {
                    if let Some(sub) = current.get_subkey_mut(part) {
                        current = sub;
                    } else {
                        return;
                    }
                }
                current.is_expanded = !current.is_expanded;
            }
        }
    }

    pub fn generate_value_rows(&self) -> Vec<ValueListRow> {
        let key = match self.tree.get_key(&self.selected_path) {
            Some(k) => k,
            None => return Vec::new(),
        };

        let mut rows = Vec::new();
        // 1. Default value first
        if let Some(def) = key.values.get("") {
            let is_sel = self.selected_value_name.as_deref() == Some("")
                || self.selected_value_name.as_deref() == Some("(Default)");
            rows.push(ValueListRow {
                name: "(Default)".to_string(),
                is_default: true,
                type_name: def.value_type.type_name().to_string(),
                data_display: def.format_display_data(),
                is_selected: is_sel,
            });
        }

        // 2. Named values
        for (name, val) in &key.values {
            if name.is_empty() {
                continue;
            }
            let is_sel = self.selected_value_name.as_deref() == Some(name);
            rows.push(ValueListRow {
                name: name.clone(),
                is_default: false,
                type_name: val.value_type.type_name().to_string(),
                data_display: val.format_display_data(),
                is_selected: is_sel,
            });
        }

        rows
    }

    pub fn select_value(&mut self, name: Option<&str>) {
        self.selected_value_name = name.map(|s| s.to_string());
        if let Some(val_name) = name {
            if let Some(key) = self.tree.get_key(&self.selected_path) {
                if let Some(val) = key.get_value(val_name) {
                    self.status_text = format!(
                        "{} | {} : {}",
                        self.selected_path,
                        val.display_name(),
                        val.format_display_data()
                    );
                }
            }
        } else {
            self.status_text = self.selected_path.clone();
        }
    }

    // --- Key & Value Operations ---

    pub fn new_key(&mut self) -> Result<String, RegistryError> {
        let key = self
            .tree
            .get_key(&self.selected_path)
            .ok_or_else(|| RegistryError::KeyNotFound(self.selected_path.clone()))?;

        // Generate next available "New Key #X" name
        let mut idx = 1;
        let mut new_name = format!("New Key #{}", idx);
        while key.subkeys.contains_key(&new_name) {
            idx += 1;
            new_name = format!("New Key #{}", idx);
        }

        let new_path = format!("{}\\{}", self.selected_path, new_name);
        self.tree.create_key(&new_path)?;
        self.navigate_to(&new_path);
        Ok(new_name)
    }

    pub fn new_string_value(&mut self) -> Result<String, RegistryError> {
        self.create_typed_new_value(RegistryValueType::RegSz)
    }

    pub fn new_dword_value(&mut self) -> Result<String, RegistryError> {
        self.create_typed_new_value(RegistryValueType::RegDword)
    }

    pub fn new_qword_value(&mut self) -> Result<String, RegistryError> {
        self.create_typed_new_value(RegistryValueType::RegQword)
    }

    pub fn new_binary_value(&mut self) -> Result<String, RegistryError> {
        self.create_typed_new_value(RegistryValueType::RegBinary)
    }

    pub fn new_multi_string_value(&mut self) -> Result<String, RegistryError> {
        self.create_typed_new_value(RegistryValueType::RegMultiSz)
    }

    fn create_typed_new_value(&mut self, val_type: RegistryValueType) -> Result<String, RegistryError> {
        let key = self
            .tree
            .get_key(&self.selected_path)
            .ok_or_else(|| RegistryError::KeyNotFound(self.selected_path.clone()))?;

        let mut idx = 1;
        let mut new_name = format!("New Value #{}", idx);
        while key.values.contains_key(&new_name) {
            idx += 1;
            new_name = format!("New Value #{}", idx);
        }

        let val = match val_type {
            RegistryValueType::RegSz => RegistryValue::new_sz(&new_name, ""),
            RegistryValueType::RegDword => RegistryValue::new_dword(&new_name, 0),
            RegistryValueType::RegQword => RegistryValue::new_qword(&new_name, 0),
            RegistryValueType::RegBinary => RegistryValue::new_binary(&new_name, &[]),
            RegistryValueType::RegMultiSz => RegistryValue::new_multi_sz(&new_name, &[]),
            _ => RegistryValue::new_sz(&new_name, ""),
        };

        self.tree.set_value(&self.selected_path, val)?;
        self.selected_value_name = Some(new_name.clone());
        Ok(new_name)
    }

    pub fn prompt_delete_selected(&mut self) {
        if let Some(val_name) = &self.selected_value_name {
            self.active_dialog = Some(RegeditDialog::ConfirmDelete(ConfirmDeleteDialog::for_value(
                &self.selected_path,
                val_name,
            )));
        } else {
            // Delete Key prompt
            if self.selected_path != COMPUTER_ROOT && parse_registry_path(&self.selected_path).len() > 1 {
                self.active_dialog = Some(RegeditDialog::ConfirmDelete(ConfirmDeleteDialog::for_key(
                    &self.selected_path,
                )));
            }
        }
    }

    pub fn confirm_delete_dialog(&mut self) -> Result<(), RegistryError> {
        if let Some(RegeditDialog::ConfirmDelete(dlg)) = self.active_dialog.take() {
            match dlg.target_type {
                DeleteTargetType::Value => {
                    self.tree.delete_value(&dlg.path, &dlg.item_name)?;
                    self.selected_value_name = None;
                }
                DeleteTargetType::Key => {
                    let parent = dlg.path.rsplit_once('\\').map(|(p, _)| p.to_string()).unwrap_or_else(|| COMPUTER_ROOT.to_string());
                    self.tree.delete_key(&dlg.path)?;
                    self.navigate_to(&parent);
                }
            }
        }
        Ok(())
    }

    pub fn prompt_rename_selected(&mut self) {
        if let Some(val_name) = &self.selected_value_name {
            if !val_name.is_empty() && val_name != "(Default)" {
                self.active_dialog = Some(RegeditDialog::Rename(RenameDialog {
                    is_key: false,
                    parent_path: self.selected_path.clone(),
                    old_name: val_name.clone(),
                    new_name: val_name.clone(),
                }));
            }
        } else {
            if parse_registry_path(&self.selected_path).len() > 1 {
                let key_name = self.selected_path.split('\\').last().unwrap_or("").to_string();
                self.active_dialog = Some(RegeditDialog::Rename(RenameDialog {
                    is_key: true,
                    parent_path: self.selected_path.clone(),
                    old_name: key_name.clone(),
                    new_name: key_name,
                }));
            }
        }
    }

    pub fn confirm_rename_dialog(&mut self, new_name: &str) -> Result<(), RegistryError> {
        if let Some(RegeditDialog::Rename(dlg)) = self.active_dialog.take() {
            if dlg.is_key {
                let updated_path = self.tree.rename_key(&dlg.parent_path, new_name)?;
                self.navigate_to(&updated_path);
            } else {
                self.tree.rename_value(&dlg.parent_path, &dlg.old_name, new_name)?;
                self.selected_value_name = Some(new_name.to_string());
            }
        }
        Ok(())
    }

    // --- Dialogs Activation ---

    pub fn open_edit_dialog_for_selected(&mut self) {
        let key = match self.tree.get_key(&self.selected_path) {
            Some(k) => k,
            None => return,
        };

        let val_name = self.selected_value_name.as_deref().unwrap_or("");
        let val = match key.get_value(val_name) {
            Some(v) => v,
            None => return,
        };

        let dialog = match val.value_type {
            RegistryValueType::RegSz | RegistryValueType::RegExpandSz => {
                RegeditDialog::EditString(EditStringDialog::new(&self.selected_path, val))
            }
            RegistryValueType::RegDword | RegistryValueType::RegQword => {
                RegeditDialog::EditDword(EditDwordDialog::new(&self.selected_path, val))
            }
            RegistryValueType::RegBinary | RegistryValueType::RegNone => {
                RegeditDialog::EditBinary(EditBinaryDialog::new(&self.selected_path, val))
            }
            RegistryValueType::RegMultiSz => {
                RegeditDialog::EditMultiString(EditMultiStringDialog::new(&self.selected_path, val))
            }
            _ => RegeditDialog::EditBinary(EditBinaryDialog::new(&self.selected_path, val)),
        };

        self.active_dialog = Some(dialog);
    }

    pub fn save_active_edit_dialog(&mut self) -> Result<(), String> {
        let dialog = match self.active_dialog.take() {
            Some(d) => d,
            None => return Ok(()),
        };

        match dialog {
            RegeditDialog::EditString(d) => {
                let val = d.commit();
                let _ = self.tree.set_value(&d.key_path, val);
            }
            RegeditDialog::EditDword(d) => {
                let val = d.commit()?;
                let _ = self.tree.set_value(&d.key_path, val);
            }
            RegeditDialog::EditBinary(d) => {
                let val = d.commit();
                let _ = self.tree.set_value(&d.key_path, val);
            }
            RegeditDialog::EditMultiString(d) => {
                let val = d.commit();
                let _ = self.tree.set_value(&d.key_path, val);
            }
            other => {
                self.active_dialog = Some(other);
            }
        }
        Ok(())
    }

    pub fn cancel_active_dialog(&mut self) {
        self.active_dialog = None;
    }

    // --- Search Engine Integration (Ctrl+F, F3) ---

    pub fn open_find_dialog(&mut self) {
        self.active_dialog = Some(RegeditDialog::Find(self.search_options.clone()));
    }

    pub fn execute_find(&mut self, options: FindOptions) -> bool {
        self.search_options = options.clone();
        self.last_search_results = self.tree.find_matches(&options);
        self.search_result_idx = 0;
        self.active_dialog = None;

        if !self.last_search_results.is_empty() {
            self.jump_to_current_search_result();
            true
        } else {
            self.status_text = format!("Finished searching through the registry. No matches for '{}'.", options.query);
            false
        }
    }

    pub fn find_next(&mut self) -> bool {
        if self.last_search_results.is_empty() {
            if !self.search_options.query.is_empty() {
                return self.execute_find(self.search_options.clone());
            }
            return false;
        }

        self.search_result_idx = (self.search_result_idx + 1) % self.last_search_results.len();
        self.jump_to_current_search_result();
        true
    }

    fn jump_to_current_search_result(&mut self) {
        if let Some(res) = self.last_search_results.get(self.search_result_idx) {
            let key_path = res.key_path.clone();
            let value_name = res.value_name.clone();
            let matched_text = res.matched_text.clone();
            let matched_field = res.matched_field;
            let total = self.last_search_results.len();
            let current = self.search_result_idx + 1;

            self.navigate_to(&key_path);
            self.selected_value_name = value_name;
            self.status_text = format!(
                "Match {} of {}: Found '{}' in {:?} ({})",
                current,
                total,
                matched_text,
                matched_field,
                key_path
            );
        }
    }

    // --- Keyboard Event Dispatching ---

    pub fn handle_key_event(&mut self, key: RegeditKey, ctrl: bool, _shift: bool, alt: bool) {
        // If a dialog is open, handle dialog escape / enter
        if self.active_dialog.is_some() {
            match key {
                RegeditKey::Escape => self.cancel_active_dialog(),
                RegeditKey::Enter => {
                    let _ = self.save_active_edit_dialog();
                }
                _ => {}
            }
            return;
        }

        match key {
            RegeditKey::F2 => self.prompt_rename_selected(),
            RegeditKey::F3 => {
                let _ = self.find_next();
            }
            RegeditKey::F5 => {
                // Refresh
                let cur = self.selected_path.clone();
                self.navigate_to(&cur);
            }
            RegeditKey::Delete => self.prompt_delete_selected(),
            RegeditKey::Enter => {
                if self.is_editing_address_bar {
                    self.submit_address_bar();
                } else {
                    self.open_edit_dialog_for_selected();
                }
            }
            RegeditKey::Left if alt => self.navigate_back(),
            RegeditKey::Right if alt => self.navigate_forward(),
            RegeditKey::Backspace => self.navigate_up(),
            RegeditKey::Char('f') if ctrl => self.open_find_dialog(),
            _ => {}
        }
    }

    // --- ASCII View Visualizer (for Console / Testing / DWM) ---

    pub fn render_ascii_view(&self) -> String {
        let mut out = String::new();
        out.push_str("┌─ Registry Editor ─────────────────────────────────────────────────────────────┐\n");
        out.push_str("│ File  Edit  View  Favorites  Help                                            │\n");
        out.push_str("├──────────────────────────────────────────────────────────────────────────────┤\n");
        let addr_line = format!("│ Address: {:<68} │\n", self.selected_path);
        out.push_str(&addr_line);
        out.push_str("├─────────────────────────────┬────────────────────────────────────────────────┤\n");

        let tree_rows = self.generate_tree_rows();
        let val_rows = self.generate_value_rows();
        let max_lines = tree_rows.len().max(val_rows.len()).max(6);

        for i in 0..max_lines {
            let left_col = if i < tree_rows.len() {
                let r = &tree_rows[i];
                let prefix = "  ".repeat(r.depth);
                let expand_icon = if r.has_children {
                    if r.is_expanded { "[-] " } else { "[+] " }
                } else {
                    "    "
                };
                let sel_marker = if r.is_selected { ">" } else { " " };
                let text = format!("{}{}{}{}", sel_marker, prefix, expand_icon, r.display_name);
                format!("{:<27}", text.chars().take(27).collect::<String>())
            } else {
                format!("{:<27}", "")
            };

            let right_col = if i < val_rows.len() {
                let v = &val_rows[i];
                let sel_marker = if v.is_selected { ">" } else { " " };
                let line = format!("{} {:<12} {:<10} {}", sel_marker, v.name, v.type_name, v.data_display);
                format!("{:<46}", line.chars().take(46).collect::<String>())
            } else if i == 0 && val_rows.is_empty() {
                format!("{:<46}", " (no values)")
            } else {
                format!("{:<46}", "")
            };

            out.push_str(&format!("│ {} │ {} │\n", left_col, right_col));
        }

        out.push_str("├─────────────────────────────┴────────────────────────────────────────────────┤\n");
        let status = format!("│ {:<76} │\n", self.status_text.chars().take(76).collect::<String>());
        out.push_str(&status);
        out.push_str("└──────────────────────────────────────────────────────────────────────────────┘\n");

        if let Some(dialog_str) = self.render_active_dialog() {
            out.push('\n');
            out.push_str(&dialog_str);
        }

        out
    }

    /// Render active modal dialog in ASCII
    pub fn render_active_dialog(&self) -> Option<String> {
        let dialog = self.active_dialog.as_ref()?;
        let mut out = String::new();

        match dialog {
            RegeditDialog::EditString(d) => {
                let title = if d.value_type == RegistryValueType::RegExpandSz {
                    "Edit Expandable String"
                } else {
                    "Edit String"
                };
                out.push_str(&format!("┌─ {:<44} ─┐\n", title));
                out.push_str("│ Value name:                                  │\n");
                out.push_str(&format!("│ {:<44} │\n", d.value_name));
                out.push_str("│                                              │\n");
                out.push_str("│ Value data:                                  │\n");
                let data_disp = format!("[{:<42}]", d.value_data.chars().take(42).collect::<String>());
                out.push_str(&format!("│ {:<44} │\n", data_disp));
                out.push_str("│                                              │\n");
                out.push_str("│                 [  OK  ]      [ Cancel ]     │\n");
                out.push_str("└──────────────────────────────────────────────┘\n");
            }
            RegeditDialog::EditDword(d) => {
                let title = if d.is_qword {
                    "Edit QWORD (64-bit) Value"
                } else {
                    "Edit DWORD (32-bit) Value"
                };
                let hex_mark = if d.base == DwordBase::Hexadecimal { "(o)" } else { "( )" };
                let dec_mark = if d.base == DwordBase::Decimal { "(o)" } else { "( )" };
                out.push_str(&format!("┌─ {:<44} ─┐\n", title));
                out.push_str("│ Value name:                                  │\n");
                out.push_str(&format!("│ {:<44} │\n", d.value_name));
                out.push_str("│                                              │\n");
                out.push_str("│ Value data:               Base               │\n");
                let val_disp = format!("[{:<20}]", d.input_text);
                out.push_str(&format!("│ {:<22}  {} Hexadecimal      │\n", val_disp, hex_mark));
                out.push_str(&format!("│ {:<22}  {} Decimal          │\n", "", dec_mark));
                out.push_str("│                                              │\n");
                out.push_str("│                 [  OK  ]      [ Cancel ]     │\n");
                out.push_str("└──────────────────────────────────────────────┘\n");
            }
            RegeditDialog::EditBinary(d) => {
                out.push_str("┌─ Edit Binary Value ──────────────────────────┐\n");
                out.push_str("│ Value name:                                  │\n");
                out.push_str(&format!("│ {:<44} │\n", d.value_name));
                out.push_str("│                                              │\n");
                out.push_str("│ Value data:                                  │\n");
                for row in d.rows().iter().take(4) {
                    let hex_part = row.hex_bytes.join(" ");
                    let line = format!("{}  {:<24}  {}", row.offset_label, hex_part, row.ascii_chars);
                    out.push_str(&format!("│ {:<44} │\n", line));
                }
                out.push_str("│                                              │\n");
                out.push_str("│                 [  OK  ]      [ Cancel ]     │\n");
                out.push_str("└──────────────────────────────────────────────┘\n");
            }
            RegeditDialog::EditMultiString(d) => {
                out.push_str("┌─ Edit Multi-String ──────────────────────────┐\n");
                out.push_str("│ Value name:                                  │\n");
                out.push_str(&format!("│ {:<44} │\n", d.value_name));
                out.push_str("│                                              │\n");
                out.push_str("│ Value data:                                  │\n");
                for line in d.lines.iter().take(3) {
                    out.push_str(&format!("│ │ {:<42} │\n", line.chars().take(42).collect::<String>()));
                }
                out.push_str("│                                              │\n");
                out.push_str("│                 [  OK  ]      [ Cancel ]     │\n");
                out.push_str("└──────────────────────────────────────────────┘\n");
            }
            RegeditDialog::ConfirmDelete(d) => {
                let title = match d.target_type {
                    DeleteTargetType::Key => "Confirm Key Delete",
                    DeleteTargetType::Value => "Confirm Value Delete",
                };
                out.push_str(&format!("┌─ {:<44} ─┐\n", title));
                for line in d.message.lines() {
                    out.push_str(&format!("│ {:<44} │\n", line.chars().take(44).collect::<String>()));
                }
                out.push_str("│                                              │\n");
                out.push_str("│                 [  Yes ]      [  No  ]       │\n");
                out.push_str("└──────────────────────────────────────────────┘\n");
            }
            RegeditDialog::Rename(d) => {
                let title = if d.is_key { "Rename Key" } else { "Rename Value" };
                out.push_str(&format!("┌─ {:<44} ─┐\n", title));
                out.push_str("│ New name:                                    │\n");
                let input = format!("[{:<42}]", d.new_name);
                out.push_str(&format!("│ {:<44} │\n", input));
                out.push_str("│                                              │\n");
                out.push_str("│                 [  OK  ]      [ Cancel ]     │\n");
                out.push_str("└──────────────────────────────────────────────┘\n");
            }
            RegeditDialog::Find(f) => {
                out.push_str("┌─ Find ───────────────────────────────────────┐\n");
                let what = format!("[{:<40}]", f.query);
                out.push_str(&format!("│ Find what: {:<33} │\n", what));
                out.push_str("│                                              │\n");
                let k_mark = if f.look_at_keys { "[X]" } else { "[ ]" };
                let v_mark = if f.look_at_values { "[X]" } else { "[ ]" };
                let d_mark = if f.look_at_data { "[X]" } else { "[ ]" };
                let w_mark = if f.match_whole_string { "[X]" } else { "[ ]" };
                let c_mark = if f.match_case { "[X]" } else { "[ ]" };
                out.push_str(&format!("│ Look at                 {} Match whole string│\n", w_mark));
                out.push_str(&format!("│ {} Keys                 {} Match case        │\n", k_mark, c_mark));
                out.push_str(&format!("│ {} Values                                    │\n", v_mark));
                out.push_str(&format!("│ {} Data               [ Find Next ] [Cancel] │\n", d_mark));
                out.push_str("└──────────────────────────────────────────────┘\n");
            }
        }

        Some(out)
    }
}

// ============================================================================
// 10. Comprehensive Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hive_hierarchy_traversal() {
        let app = RegeditApp::new();
        // 1. Verify 5 standard root hives exist
        assert_eq!(app.tree.roots.len(), 5);
        for hive in ROOT_HIVES {
            assert!(app.tree.roots.contains_key(hive), "Missing root hive {}", hive);
        }

        // 2. Traversal to child keys
        let cur_ver_path = "Computer\\HKEY_LOCAL_MACHINE\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion";
        let cur_ver = app.tree.get_key(cur_ver_path);
        assert!(cur_ver.is_some(), "CurrentVersion key must exist");
        let cur_ver_key = cur_ver.unwrap();
        assert_eq!(cur_ver_key.name, "CurrentVersion");

        // 3. Tree view rows generation includes Computer and expanded roots
        let rows = app.generate_tree_rows();
        assert!(rows.iter().any(|r| r.display_name == COMPUTER_ROOT));
        assert!(rows.iter().any(|r| r.display_name == HKLM));
    }

    #[test]
    fn test_value_type_formatting() {
        // REG_SZ
        let sz = RegistryValue::new_sz("Name", "Hello, HybridOS!");
        assert_eq!(sz.value_type, RegistryValueType::RegSz);
        assert_eq!(sz.decode_string(), "Hello, HybridOS!");
        assert_eq!(sz.format_display_data(), "Hello, HybridOS!");

        // REG_DWORD
        let dw = RegistryValue::new_dword("Count", 42);
        assert_eq!(dw.decode_dword(), 42);
        assert_eq!(dw.format_display_data(), "0x0000002a (42)");

        // REG_QWORD
        let qw = RegistryValue::new_qword("LargeCount", 0x100000002a);
        assert_eq!(qw.decode_qword(), 0x100000002a);
        assert_eq!(qw.format_display_data(), "0x000000100000002a (68719476778)");

        // REG_BINARY
        let bin = RegistryValue::new_binary("Data", &[0xde, 0xad, 0xbe, 0xef]);
        assert_eq!(bin.format_display_data(), "de ad be ef");

        // REG_MULTI_SZ
        let multi = RegistryValue::new_multi_sz("List", &[
            "Alpha".to_string(),
            "Beta".to_string(),
            "Gamma".to_string(),
        ]);
        let decoded_multi = multi.decode_multi_sz();
        assert_eq!(decoded_multi, vec!["Alpha", "Beta", "Gamma"]);
        assert_eq!(multi.format_display_data(), "Alpha Beta Gamma");

        // REG_NONE
        let none = RegistryValue::new_none("Empty", &[]);
        assert_eq!(none.format_display_data(), "(zero-length binary value)");
    }

    #[test]
    fn test_string_edit_dialog() {
        let val = RegistryValue::new_sz("TestVal", "Initial Content");
        let mut dlg = EditStringDialog::new("Computer\\HKEY_CURRENT_USER\\Software", &val);
        assert_eq!(dlg.value_data, "Initial Content");

        dlg.set_data("Updated Content for HybridOS");
        let updated_val = dlg.commit();
        assert_eq!(updated_val.name, "TestVal");
        assert_eq!(updated_val.decode_string(), "Updated Content for HybridOS");
    }

    #[test]
    fn test_dword_edit_dialog_hex_and_dec() {
        let val = RegistryValue::new_dword("ConfigOption", 255);
        let mut dlg = EditDwordDialog::new("Computer\\HKEY_LOCAL_MACHINE\\SYSTEM", &val);
        assert_eq!(dlg.input_text, "ff");
        assert_eq!(dlg.base, DwordBase::Hexadecimal);

        // Switch to Decimal
        dlg.set_base(DwordBase::Decimal);
        assert_eq!(dlg.input_text, "255");
        assert_eq!(dlg.base, DwordBase::Decimal);

        // Edit Decimal input
        assert!(dlg.set_input_text("1024").is_ok());

        // Switch back to Hexadecimal
        dlg.set_base(DwordBase::Hexadecimal);
        assert_eq!(dlg.input_text, "400"); // 1024 in hex is 0x400

        // Commit dialog and verify little-endian encoding
        let committed = dlg.commit().expect("Commit should succeed");
        assert_eq!(committed.decode_dword(), 1024);
        assert_eq!(committed.raw_data, 1024u32.to_le_bytes().to_vec());
    }

    #[test]
    fn test_binary_edit_dialog() {
        let val = RegistryValue::new_binary("RawBytes", &[0x01, 0x02, 0x03, 0x04]);
        let mut dlg = EditBinaryDialog::new("Computer\\HKLM\\Software", &val);
        let rows = dlg.rows();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].hex_bytes, vec!["01", "02", "03", "04"]);

        // Insert byte
        dlg.cursor_byte_idx = 4;
        dlg.insert_byte(0x05);
        assert_eq!(dlg.data, vec![0x01, 0x02, 0x03, 0x04, 0x05]);

        // Delete byte
        dlg.cursor_byte_idx = 1;
        dlg.delete_byte();
        assert_eq!(dlg.data, vec![0x01, 0x03, 0x04, 0x05]);

        // Set from hex string
        assert!(dlg.set_data_from_hex_string("de ad be ef 00 11 22").is_ok());
        assert_eq!(dlg.data, vec![0xde, 0xad, 0xbe, 0xef, 0x00, 0x11, 0x22]);

        let committed = dlg.commit();
        assert_eq!(committed.raw_data, vec![0xde, 0xad, 0xbe, 0xef, 0x00, 0x11, 0x22]);
    }

    #[test]
    fn test_multi_string_edit_dialog() {
        let original_lines = vec!["Server1".to_string(), "Server2".to_string(), "Server3".to_string()];
        let val = RegistryValue::new_multi_sz("HostList", &original_lines);
        let mut dlg = EditMultiStringDialog::new("Computer\\HKLM\\System", &val);
        assert_eq!(dlg.lines, original_lines);
        assert_eq!(dlg.get_text(), "Server1\nServer2\nServer3");

        dlg.set_text("ClusterA\nClusterB\nClusterC\n");
        let committed = dlg.commit();
        assert_eq!(committed.decode_multi_sz(), vec!["ClusterA", "ClusterB", "ClusterC"]);
    }

    #[test]
    fn test_create_and_delete_key() {
        let mut tree = RegistryTree::new();
        let target = "Computer\\HKEY_CURRENT_USER\\Software\\TestCompany\\Product";
        assert!(tree.create_key(target).is_ok());

        assert!(tree.get_key(target).is_some());
        assert!(tree.get_key("Computer\\HKEY_CURRENT_USER\\Software\\TestCompany").is_some());

        // Deleting root hive must fail
        assert!(tree.delete_key("Computer\\HKEY_CURRENT_USER").is_err());

        // Deleting subkey succeeds
        assert!(tree.delete_key("Computer\\HKEY_CURRENT_USER\\Software\\TestCompany").is_ok());
        assert!(tree.get_key(target).is_none());
        assert!(tree.get_key("Computer\\HKEY_CURRENT_USER\\Software\\TestCompany").is_none());
    }

    #[test]
    fn test_rename_key_and_value() {
        let mut tree = RegistryTree::new();
        let path = "Computer\\HKEY_CURRENT_USER\\Software\\AppOriginal";
        assert!(tree.create_key(path).is_ok());
        assert!(tree.set_value(path, RegistryValue::new_sz("SettingA", "123")).is_ok());

        // Rename Value
        assert!(tree.rename_value(path, "SettingA", "SettingRenamed").is_ok());
        let key = tree.get_key(path).unwrap();
        assert!(key.get_value("SettingA").is_none());
        assert_eq!(key.get_value("SettingRenamed").unwrap().decode_string(), "123");

        // Rename Key
        let new_path = tree.rename_key(path, "AppRenamed").expect("Rename key should succeed");
        assert_eq!(new_path, "Computer\\HKEY_CURRENT_USER\\Software\\AppRenamed");
        assert!(tree.get_key(path).is_none());
        assert!(tree.get_key(&new_path).is_some());
        assert!(tree.get_key(&new_path).unwrap().get_value("SettingRenamed").is_some());
    }

    #[test]
    fn test_search_find_and_find_next() {
        let mut tree = RegistryTree::new();
        let key_path = "Computer\\HKEY_LOCAL_MACHINE\\SOFTWARE\\HybridOS\\Core";
        tree.create_key(key_path).unwrap();
        tree.set_value(key_path, RegistryValue::new_sz("KernelArchitecture", "AArch64-x86_64")).unwrap();
        tree.set_value(key_path, RegistryValue::new_dword("MaxCPUs", 256)).unwrap();

        // Search for "KernelArchitecture" (Value name)
        let opts1 = FindOptions {
            query: "KernelArchitecture".to_string(),
            look_at_keys: true,
            look_at_values: true,
            look_at_data: true,
            match_whole_string: true,
            match_case: false,
        };
        let matches1 = tree.find_matches(&opts1);
        assert_eq!(matches1.len(), 1);
        assert_eq!(matches1[0].matched_field, MatchedField::ValueName);

        // Search for "AArch64" (Value data)
        let opts2 = FindOptions {
            query: "AArch64".to_string(),
            look_at_keys: false,
            look_at_values: false,
            look_at_data: true,
            match_whole_string: false,
            match_case: true,
        };
        let matches2 = tree.find_matches(&opts2);
        assert_eq!(matches2.len(), 1);
        assert_eq!(matches2[0].matched_field, MatchedField::ValueData);

        // Search for "HybridOS" (Key name)
        let opts3 = FindOptions {
            query: "HybridOS".to_string(),
            look_at_keys: true,
            look_at_values: false,
            look_at_data: false,
            match_whole_string: true,
            match_case: false,
        };
        let matches3 = tree.find_matches(&opts3);
        assert_eq!(matches3.len(), 1);
        assert_eq!(matches3[0].matched_field, MatchedField::KeyName);
    }

    #[test]
    fn test_reg_file_export_import() {
        let mut original_tree = RegistryTree::new();
        let branch = "Computer\\HKEY_CURRENT_USER\\Software\\HybridTest";
        original_tree.create_key(branch).unwrap();
        original_tree.set_value(branch, RegistryValue::new_sz("", "DefaultAppVal")).unwrap();
        original_tree.set_value(branch, RegistryValue::new_sz("AppTitle", "HybridOS Suite")).unwrap();
        original_tree.set_value(branch, RegistryValue::new_dword("VersionCode", 100)).unwrap();
        original_tree.set_value(branch, RegistryValue::new_binary("Signature", &[0xca, 0xfe, 0xba, 0xbe])).unwrap();

        // Export
        let reg_text = original_tree.export_subtree(branch).expect("Export must succeed");
        assert!(reg_text.contains("Windows Registry Editor Version 5.00"));
        assert!(reg_text.contains("[HKEY_CURRENT_USER\\Software\\HybridTest]"));
        assert!(reg_text.contains("@=\"DefaultAppVal\""));
        assert!(reg_text.contains("\"AppTitle\"=\"HybridOS Suite\""));
        assert!(reg_text.contains("\"VersionCode\"=dword:00000064"));
        assert!(reg_text.contains("\"Signature\"=hex:ca,fe,ba,be"));

        // Import into fresh tree
        let mut new_tree = RegistryTree::new();
        let imported_count = new_tree.import_reg_file(&reg_text).expect("Import must succeed");
        assert!(imported_count >= 4);

        let key = new_tree.get_key(branch).expect("Imported key must exist");
        assert_eq!(key.get_value("").unwrap().decode_string(), "DefaultAppVal");
        assert_eq!(key.get_value("AppTitle").unwrap().decode_string(), "HybridOS Suite");
        assert_eq!(key.get_value("VersionCode").unwrap().decode_dword(), 100);
        assert_eq!(key.get_value("Signature").unwrap().raw_data, vec![0xca, 0xfe, 0xba, 0xbe]);
    }

    #[test]
    fn test_address_bar_and_history() {
        let mut app = RegeditApp::new();
        assert_eq!(app.selected_path, "Computer\\HKEY_LOCAL_MACHINE");

        // Navigate with abbreviation
        app.navigate_to("HKCU\\Software\\Microsoft");
        assert_eq!(app.selected_path, "Computer\\HKEY_CURRENT_USER\\Software\\Microsoft");

        app.navigate_to("HKLM\\SYSTEM\\CurrentControlSet\\Services");
        assert_eq!(app.selected_path, "Computer\\HKEY_LOCAL_MACHINE\\SYSTEM\\CurrentControlSet\\Services");

        // Check history back / forward
        assert!(app.can_go_back());
        app.navigate_back();
        assert_eq!(app.selected_path, "Computer\\HKEY_CURRENT_USER\\Software\\Microsoft");

        app.navigate_back();
        assert_eq!(app.selected_path, "Computer\\HKEY_LOCAL_MACHINE");

        assert!(app.can_go_forward());
        app.navigate_forward();
        assert_eq!(app.selected_path, "Computer\\HKEY_CURRENT_USER\\Software\\Microsoft");

        // Check breadcrumbs
        let crumbs = app.breadcrumbs();
        assert_eq!(crumbs.len(), 4);
        assert_eq!(crumbs[0].display_name, "Computer");
        assert_eq!(crumbs[1].display_name, "HKEY_CURRENT_USER");
        assert_eq!(crumbs[2].display_name, "Software");
        assert_eq!(crumbs[3].display_name, "Microsoft");
    }

    #[test]
    fn test_ascii_view_and_keyboard_shortcuts() {
        let mut app = RegeditApp::new();
        let view = app.render_ascii_view();
        assert!(view.contains("Registry Editor"));
        assert!(view.contains("Computer\\HKEY_LOCAL_MACHINE"));
        assert!(view.contains("HKEY_LOCAL_MACHINE"));

        // Test Alt+Left and Alt+Right keyboard shortcuts
        app.navigate_to("Computer\\HKEY_CURRENT_CONFIG");
        assert_eq!(app.selected_path, "Computer\\HKEY_CURRENT_CONFIG");

        app.handle_key_event(RegeditKey::Left, false, false, true); // Alt+Left
        assert_eq!(app.selected_path, "Computer\\HKEY_LOCAL_MACHINE");

        app.handle_key_event(RegeditKey::Right, false, false, true); // Alt+Right
        assert_eq!(app.selected_path, "Computer\\HKEY_CURRENT_CONFIG");

        // Test Ctrl+F opens Find dialog
        app.handle_key_event(RegeditKey::Char('f'), true, false, false);
        assert!(matches!(app.active_dialog, Some(RegeditDialog::Find(_))));

        let dlg_view = app.render_active_dialog().expect("Dialog view should render");
        assert!(dlg_view.contains("Find"));
        assert!(dlg_view.contains("Find what:"));
        assert!(dlg_view.contains("Match whole string"));

        app.handle_key_event(RegeditKey::Escape, false, false, false);
        assert!(app.active_dialog.is_none());
    }
}
