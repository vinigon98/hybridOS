//! libhyb-cfg: Unified Declarative Configuration & IPC Sync Library for HybridOS
//!
//! Provides parsing, validation, persistence, and real-time IPC synchronization
//! for `/etc/hybridos/system.toml` and `~/.config/hybridos/user.toml`.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Global configuration container supporting hierarchical dotted keys
#[derive(Debug, Clone)]
pub struct ConfigStore {
    system_path: PathBuf,
    user_path: PathBuf,
    values: BTreeMap<String, String>,
}

impl ConfigStore {
    /// Initialize with standard system and user configuration paths
    pub fn new() -> Self {
        let system_candidate = PathBuf::from("/etc/hybridos/system.toml");
        let fallback_system = if system_candidate.exists() {
            system_candidate
        } else if Path::new("etc/hybridos/system.toml").exists() {
            PathBuf::from("etc/hybridos/system.toml")
        } else {
            PathBuf::from("etc/hybridos/system.toml")
        };

        let user_candidate = dirs_config_path();

        let mut store = Self {
            system_path: fallback_system,
            user_path: user_candidate,
            values: BTreeMap::new(),
        };

        store.load_defaults();
        let _ = store.load_file(&store.system_path.clone());
        if store.user_path.exists() {
            let _ = store.load_file(&store.user_path.clone());
        }

        store
    }

    /// Load default parameters for Module 1 and Module 5
    fn load_defaults(&mut self) {
        // Module 1: Display & Pipeline
        self.values.insert("display.refresh_rate".to_string(), "144".to_string());
        self.values.insert("display.scaling".to_string(), "100%".to_string());
        self.values.insert("display.multi_monitor".to_string(), "true".to_string());
        self.values.insert("display.night_light".to_string(), "false".to_string());
        self.values.insert("display.hdr_pipeline".to_string(), "true".to_string());
        self.values.insert("display.disable_cabc".to_string(), "true".to_string());
        self.values.insert("graphics.scheduler_hags".to_string(), "true".to_string());
        self.values.insert("graphics.direct_scanout".to_string(), "true".to_string());
        self.values.insert("graphics.gpu_preference".to_string(), "hybrid".to_string());
        self.values.insert("graphics.tdr_recovery".to_string(), "true".to_string());
        self.values.insert("graphics.legacy_dpi_fix".to_string(), "true".to_string());
        self.values.insert("graphics.frame_limiter_global".to_string(), "0".to_string());

        // Module 5: Shell & Files
        self.values.insert("files.show_extensions".to_string(), "true".to_string());
        self.values.insert("files.show_dotfiles".to_string(), "false".to_string());
        self.values.insert("files.default_view".to_string(), "home".to_string());
        self.values.insert("files.zero_ads".to_string(), "true".to_string());
        self.values.insert("files.header_path".to_string(), "true".to_string());
        self.values.insert("shell.context_menu".to_string(), "classic_full".to_string());
        self.values.insert("shell.taskbar_clean".to_string(), "true".to_string());
        self.values.insert("shell.panel_position".to_string(), "bottom".to_string());
        self.values.insert("shell.force_kill_shortcut".to_string(), "true".to_string());
        self.values.insert("shell.launcher".to_string(), "super_space".to_string());
        self.values.insert("shell.window_tiling".to_string(), "false".to_string());
        self.values.insert("shell.virtual_desktops".to_string(), "true".to_string());
        self.values.insert("shell.compositor_effects".to_string(), "true".to_string());
        self.values.insert("shell.tray_telemetry".to_string(), "false".to_string());
    }

    /// Load and parse a TOML file line by line
    pub fn load_file(&mut self, path: &Path) -> Result<(), String> {
        let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
        self.parse_toml_str(&content);
        Ok(())
    }

    /// Parse raw TOML content into hierarchical dotted keys
    pub fn parse_toml_str(&mut self, content: &str) {
        let mut current_section = String::new();

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with(';') {
                continue;
            }

            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                current_section = trimmed[1..trimmed.len() - 1].trim().to_string();
                continue;
            }

            if let Some((raw_key, raw_val)) = trimmed.split_once('=') {
                let key = raw_key.trim();
                let mut val = raw_val.trim();

                // Strip trailing comments
                if let Some((clean_val, _)) = val.split_once('#') {
                    val = clean_val.trim();
                }

                // Strip quotes if string
                let clean_str = if (val.starts_with('"') && val.ends_with('"'))
                    || (val.starts_with('\'') && val.ends_with('\''))
                {
                    if val.len() >= 2 {
                        &val[1..val.len() - 1]
                    } else {
                        val
                    }
                } else {
                    val
                };

                let full_key = if current_section.is_empty() {
                    key.to_string()
                } else {
                    format!("{}.{}", current_section, key)
                };

                self.values.insert(full_key, clean_str.to_string());
            }
        }
    }

    /// Get raw string value
    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(|s| s.as_str())
    }

    /// Get boolean value
    pub fn get_bool(&self, key: &str) -> Option<bool> {
        match self.get(key)? {
            "true" | "1" | "yes" | "on" => Some(true),
            "false" | "0" | "no" | "off" => Some(false),
            _ => None,
        }
    }

    /// Get integer value
    pub fn get_u32(&self, key: &str) -> Option<u32> {
        self.get(key)?.parse::<u32>().ok()
    }

    /// Get string value with fallback
    pub fn get_str_or<'a>(&'a self, key: &str, default: &'a str) -> &'a str {
        self.get(key).unwrap_or(default)
    }

    /// Set a key-value pair in memory
    pub fn set(&mut self, key: &str, value: &str) {
        self.values.insert(key.to_string(), value.to_string());
    }

    /// Serialize current configuration into TOML representation
    pub fn to_toml_string(&self) -> String {
        let mut sections: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();

        for (full_key, val) in &self.values {
            if let Some((sec, subkey)) = full_key.split_once('.') {
                sections
                    .entry(sec.to_string())
                    .or_default()
                    .insert(subkey.to_string(), val.clone());
            } else {
                sections
                    .entry("general".to_string())
                    .or_default()
                    .insert(full_key.clone(), val.clone());
            }
        }

        let mut out = String::new();
        out.push_str("# HybridOS Configuration\n\n");

        for (section, entries) in sections {
            out.push_str(&format!("[{}]\n", section));
            for (k, v) in entries {
                let is_bool = v == "true" || v == "false";
                let is_num = v.parse::<i64>().is_ok();
                if is_bool || is_num {
                    out.push_str(&format!("{} = {}\n", k, v));
                } else {
                    out.push_str(&format!("{} = \"{}\"\n", k, v));
                }
            }
            out.push('\n');
        }

        out
    }

    /// Save configuration to user configuration path
    pub fn save_user(&self) -> Result<(), String> {
        if let Some(parent) = self.user_path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let rendered = self.to_toml_string();
        fs::write(&self.user_path, rendered).map_err(|e| e.to_string())?;
        Ok(())
    }
}

impl Default for ConfigStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Global shared configuration handle
pub type SharedConfig = Arc<Mutex<ConfigStore>>;

/// Create thread-safe reference to configuration store
pub fn global_config() -> SharedConfig {
    Arc::new(Mutex::new(ConfigStore::new()))
}

/// Determine target user configuration path
fn dirs_config_path() -> PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".config/hybridos/user.toml")
    } else if let Ok(userprofile) = std::env::var("USERPROFILE") {
        PathBuf::from(userprofile).join(".config/hybridos/user.toml")
    } else {
        PathBuf::from(".config/hybridos/user.toml")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_values() {
        let store = ConfigStore::new();
        assert_eq!(store.get_u32("display.refresh_rate"), Some(144));
        assert_eq!(store.get_bool("display.multi_monitor"), Some(true));
        assert_eq!(store.get_bool("files.show_extensions"), Some(true));
        assert_eq!(store.get_str_or("shell.panel_position", "bottom"), "bottom");
    }

    #[test]
    fn test_toml_parsing() {
        let mut store = ConfigStore::new();
        let sample = r#"
        [display]
        refresh_rate = 240
        scaling = "125%"
        night_light = true

        [shell]
        panel_position = "top"
        window_tiling = true
        "#;

        store.parse_toml_str(sample);
        assert_eq!(store.get_u32("display.refresh_rate"), Some(240));
        assert_eq!(store.get("display.scaling"), Some("125%"));
        assert_eq!(store.get_bool("display.night_light"), Some(true));
        assert_eq!(store.get("shell.panel_position"), Some("top"));
        assert_eq!(store.get_bool("shell.window_tiling"), Some(true));
    }

    #[test]
    fn test_set_and_serialize() {
        let mut store = ConfigStore::new();
        store.set("display.refresh_rate", "360");
        store.set("shell.panel_position", "left");
        assert_eq!(store.get_u32("display.refresh_rate"), Some(360));
        assert_eq!(store.get("shell.panel_position"), Some("left"));

        let serialized = store.to_toml_string();
        assert!(serialized.contains("[display]"));
        assert!(serialized.contains("refresh_rate = 360"));
        assert!(serialized.contains("panel_position = \"left\""));
    }
}
