//! Virtual File System - Unified Path Resolver
//!
//! Provides bidirectional path resolution for HybridOS:
//! - POSIX Unix paths (`/bin/sh`, `/dev/null`, `/usr/lib/libc.so`)
//! - Windows NT paths (`C:\Windows\System32\cmd.exe`, `\Device\HarddiskVolume1\Windows`, `\DosDevices\C:\...`)
//! - NT Object Manager namespaces (`\??\C:\...`, `\SystemRoot\System32`, `\\.\pipe`)

use std::collections::HashMap;
use std::fmt;
use std::sync::RwLock;

use super::VfsError;

/// Syntactic origin / style of the path string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathStyle {
    /// Unix/Linux standard path (`/bin/bash`, `../local`)
    Unix,
    /// Win32 DOS drive letter path (`C:\Windows\System32`, `D:/data`)
    WindowsDos,
    /// Windows NT Object Manager path (`\Device\HarddiskVolume1`, `\DosDevices\C:`)
    WindowsNt,
    /// Universal Naming Convention network path (`\\server\share\file`)
    WindowsUnc,
}

/// A fully normalized, canonical VFS path representation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalPath {
    pub is_absolute: bool,
    pub components: Vec<String>,
    pub original_style: PathStyle,
}

impl CanonicalPath {
    /// Root path `/`.
    pub fn root() -> Self {
        Self {
            is_absolute: true,
            components: Vec::new(),
            original_style: PathStyle::Unix,
        }
    }

    /// Check if this path represents the filesystem root.
    pub fn is_root(&self) -> bool {
        self.is_absolute && self.components.is_empty()
    }

    /// Retrieve the final path component (the filename or directory name).
    pub fn file_name(&self) -> Option<&str> {
        self.components.last().map(|s| s.as_str())
    }

    /// Retrieve the file extension, if present.
    pub fn extension(&self) -> Option<&str> {
        self.file_name().and_then(|name| {
            let dot_idx = name.rfind('.')?;
            if dot_idx > 0 && dot_idx < name.len() - 1 {
                Some(&name[dot_idx + 1..])
            } else {
                None
            }
        })
    }

    /// Return the parent directory's canonical path.
    pub fn parent(&self) -> Option<CanonicalPath> {
        if self.components.is_empty() {
            return None;
        }
        let mut parent_comps = self.components.clone();
        parent_comps.pop();
        Some(CanonicalPath {
            is_absolute: self.is_absolute,
            components: parent_comps,
            original_style: self.original_style,
        })
    }

    /// Join a child subpath to this canonical path.
    pub fn join(&self, child_name: &str) -> CanonicalPath {
        let mut new_comps = self.components.clone();
        for seg in child_name.split(&['/', '\\'][..]) {
            let seg = seg.trim();
            if seg.is_empty() || seg == "." {
                continue;
            } else if seg == ".." {
                new_comps.pop();
            } else {
                new_comps.push(seg.to_string());
            }
        }
        CanonicalPath {
            is_absolute: self.is_absolute,
            components: new_comps,
            original_style: self.original_style,
        }
    }

    /// Format as standard POSIX Unix path: `/c/Windows/System32/cmd.exe` or `/bin/sh`.
    pub fn as_unix_path(&self) -> String {
        if self.components.is_empty() {
            return "/".to_string();
        }
        let mut out = String::with_capacity(32);
        for comp in &self.components {
            out.push('/');
            out.push_str(comp);
        }
        out
    }

    /// Format as standard Win32 DOS path: `C:\Windows\System32\cmd.exe` or `C:\bin\sh`.
    pub fn as_windows_path(&self) -> String {
        if self.components.is_empty() {
            return "C:\\".to_string();
        }

        let first = &self.components[0];
        // If first component is a single drive letter (e.g. 'c' or 'd')
        if first.len() == 1 && first.chars().next().unwrap().is_ascii_alphabetic() {
            let drive_char = first.to_ascii_uppercase();
            if self.components.len() == 1 {
                return format!("{}:\\", drive_char);
            }
            let mut out = format!("{}:\\", drive_char);
            for (i, comp) in self.components[1..].iter().enumerate() {
                if i > 0 {
                    out.push('\\');
                }
                out.push_str(comp);
            }
            out
        } else {
            // Non-drive rooted Unix paths mapped into C:\root...
            let mut out = String::from("C:\\");
            for (i, comp) in self.components.iter().enumerate() {
                if i > 0 {
                    out.push('\\');
                }
                out.push_str(comp);
            }
            out
        }
    }

    /// Format as Windows NT Object Manager kernel path:
    /// `\Device\HarddiskVolume1\Windows\System32` or `\Device\Null`.
    pub fn as_nt_path(&self) -> String {
        if self.components.is_empty() {
            return "\\Device\\HarddiskVolume1".to_string();
        }

        // Check for device shortcuts
        if self.components.len() == 2 && self.components[0].eq_ignore_ascii_case("dev") {
            let dev_name = &self.components[1];
            if dev_name.eq_ignore_ascii_case("null") {
                return "\\Device\\Null".to_string();
            } else if dev_name.eq_ignore_ascii_case("zero") {
                return "\\Device\\Zero".to_string();
            } else if dev_name.eq_ignore_ascii_case("console") {
                return "\\Device\\Console".to_string();
            }
        }

        let first = &self.components[0];
        if first.len() == 1 && first.chars().next().unwrap().is_ascii_alphabetic() {
            let drive_char = first.chars().next().unwrap().to_ascii_uppercase();
            let volume_num = match drive_char {
                'C' => 1,
                'D' => 2,
                'E' => 3,
                _ => 1,
            };

            let mut out = format!("\\Device\\HarddiskVolume{}", volume_num);
            for comp in &self.components[1..] {
                out.push('\\');
                out.push_str(comp);
            }
            out
        } else {
            let mut out = String::from("\\Device\\HarddiskVolume1");
            for comp in &self.components {
                out.push('\\');
                out.push_str(comp);
            }
            out
        }
    }
}

impl fmt::Display for CanonicalPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_unix_path())
    }
}

/// Unified Path Resolver capable of normalizing and mapping any Unix or Windows NT path.
pub struct UnifiedPathResolver {
    /// NT namespace / drive mappings (e.g. `C:` -> `/c`, `\SystemRoot` -> `/c/Windows`)
    mount_mappings: RwLock<HashMap<String, String>>,
}

impl UnifiedPathResolver {
    /// Create a resolver pre-configured with default HybridOS kernel mappings.
    pub fn new() -> Self {
        let mut mappings = HashMap::new();

        // Win32 Drive Letter Mappings
        mappings.insert("c:".to_string(), "/c".to_string());
        mappings.insert("d:".to_string(), "/d".to_string());
        mappings.insert("e:".to_string(), "/e".to_string());
        mappings.insert("z:".to_string(), "/z".to_string());

        // NT Object Manager Prefixes
        mappings.insert("\\systemroot".to_string(), "/c/Windows".to_string());
        mappings.insert("\\systemroot\\".to_string(), "/c/Windows/".to_string());
        mappings.insert("\\dosdevices\\c:".to_string(), "/c".to_string());
        mappings.insert("\\??\\c:".to_string(), "/c".to_string());
        mappings.insert("\\\\?\\c:".to_string(), "/c".to_string());

        // NT Devices
        mappings.insert("\\device\\harddiskvolume1".to_string(), "/c".to_string());
        mappings.insert("\\device\\harddiskvolume2".to_string(), "/d".to_string());
        mappings.insert("\\device\\null".to_string(), "/dev/null".to_string());
        mappings.insert("\\device\\zero".to_string(), "/dev/zero".to_string());
        mappings.insert("\\device\\console".to_string(), "/dev/console".to_string());

        // DOS Reserved Names
        mappings.insert("nul".to_string(), "/dev/null".to_string());
        mappings.insert("con".to_string(), "/dev/console".to_string());

        Self {
            mount_mappings: RwLock::new(mappings),
        }
    }

    /// Register a custom prefix mapping (e.g., `X:` -> `/mnt/external`).
    pub fn add_mapping(&self, prefix: &str, target: &str) {
        let mut map = self.mount_mappings.write().unwrap();
        map.insert(prefix.to_ascii_lowercase(), target.to_string());
    }

    /// Resolve an arbitrary Unix or Windows NT path into a canonical VFS path.
    pub fn resolve(&self, raw_path: &str, cwd: &str) -> Result<CanonicalPath, VfsError> {
        let trimmed = raw_path.trim();
        if trimmed.is_empty() {
            return Err(VfsError::InvalidPath);
        }

        let (style, normalized_str) = self.preprocess_path(trimmed, cwd)?;

        // Split normalized string into canonical path components
        let mut components: Vec<String> = Vec::new();

        for part in normalized_str.split('/') {
            let seg = part.trim();
            if seg.is_empty() || seg == "." {
                continue;
            } else if seg == ".." {
                components.pop();
            } else {
                components.push(seg.to_string());
            }
        }

        Ok(CanonicalPath {
            is_absolute: true,
            components,
            original_style: style,
        })
    }

    /// Pre-process and normalize prefixes (e.g. drive letters, NT device prefixes, cwd resolution).
    fn preprocess_path(&self, path: &str, cwd: &str) -> Result<(PathStyle, String), VfsError> {
        let lower = path.to_ascii_lowercase();

        // 1. Check exact match in configured prefix mappings (e.g. NUL, CON)
        {
            let map = self.mount_mappings.read().unwrap();
            if let Some(target) = map.get(&lower) {
                return Ok((PathStyle::WindowsNt, target.clone()));
            }
        }

        // 2. Windows UNC paths (`\\server\share\path` or `//server/share/path`)
        if path.starts_with("\\\\") || path.starts_with("//") {
            let unc_trimmed = &path[2..];
            let converted = unc_trimmed.replace('\\', "/");
            let full = format!("/network/{}", converted.trim_matches('/'));
            return Ok((PathStyle::WindowsUnc, full));
        }

        // 3. NT Object Manager Prefix handling (`\??\`, `\DosDevices\`, `\SystemRoot\`, `\Device\`)
        if path.starts_with('\\') {
            let map = self.mount_mappings.read().unwrap();

            // Match longest known NT prefix first
            let mut matches: Vec<(&String, &String)> = map.iter()
                .filter(|(prefix, _)| lower.starts_with(*prefix))
                .collect();
            matches.sort_by_key(|(prefix, _)| std::cmp::Reverse(prefix.len()));

            if let Some((prefix, target)) = matches.first() {
                let suffix = &path[prefix.len()..];
                let clean_suffix = suffix.trim_start_matches('\\').replace('\\', "/");
                let result = if clean_suffix.is_empty() {
                    target.to_string()
                } else if target.ends_with('/') {
                    format!("{}{}", target, clean_suffix)
                } else {
                    format!("{}/{}", target, clean_suffix)
                };
                return Ok((PathStyle::WindowsNt, result));
            }

            // General NT path fallback (e.g. `\Windows\System32`)
            let converted = path.replace('\\', "/");
            return Ok((PathStyle::WindowsNt, format!("/c{}", converted)));
        }

        // 4. Windows DOS Drive Letter Paths (`C:\foo\bar`, `d:/data`)
        if path.len() >= 2 {
            let bytes = path.as_bytes();
            if bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
                let drive_char = (bytes[0] as char).to_ascii_lowercase();
                let remainder = &path[2..];
                let converted_remainder = remainder.trim_start_matches(|c| c == '/' || c == '\\').replace('\\', "/");
                let full = if converted_remainder.is_empty() {
                    format!("/{}", drive_char)
                } else {
                    format!("/{}/{}", drive_char, converted_remainder)
                };
                return Ok((PathStyle::WindowsDos, full));
            }
        }

        // 5. Unix Absolute Paths (`/bin/sh`, `/dev/null`)
        if path.starts_with('/') {
            let converted = path.replace('\\', "/");
            return Ok((PathStyle::Unix, converted));
        }

        // 6. Relative Paths (`foo/bar`, `..\file.txt`, `./dir`)
        let clean_cwd = if cwd.trim().is_empty() { "/" } else { cwd.trim() };
        let converted_rel = path.replace('\\', "/");
        let combined = if clean_cwd.ends_with('/') {
            format!("{}{}", clean_cwd, converted_rel)
        } else {
            format!("{}/{}", clean_cwd, converted_rel)
        };

        let style = if path.contains('\\') {
            PathStyle::WindowsDos
        } else {
            PathStyle::Unix
        };

        Ok((style, combined))
    }

    /// Convert any input path directly to its canonical Unix string.
    pub fn to_unix_path(&self, raw_path: &str) -> Result<String, VfsError> {
        let canon = self.resolve(raw_path, "/")?;
        Ok(canon.as_unix_path())
    }

    /// Convert any input path directly to its Win32 DOS string.
    pub fn to_windows_path(&self, raw_path: &str) -> Result<String, VfsError> {
        let canon = self.resolve(raw_path, "/c")?;
        Ok(canon.as_windows_path())
    }
}

impl Default for UnifiedPathResolver {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unix_path_resolution() {
        let resolver = UnifiedPathResolver::new();

        let p1 = resolver.resolve("/bin/sh", "/").unwrap();
        assert_eq!(p1.as_unix_path(), "/bin/sh");
        assert_eq!(p1.components, vec!["bin", "sh"]);

        let p2 = resolver.resolve("/dev/null", "/").unwrap();
        assert_eq!(p2.as_unix_path(), "/dev/null");

        let p3 = resolver.resolve("/usr/local/../bin/sh", "/").unwrap();
        assert_eq!(p3.as_unix_path(), "/usr/bin/sh");
    }

    #[test]
    fn test_windows_dos_path_resolution() {
        let resolver = UnifiedPathResolver::new();

        let p1 = resolver.resolve(r"C:\Windows\System32\cmd.exe", "/").unwrap();
        assert_eq!(p1.as_unix_path(), "/c/Windows/System32/cmd.exe");
        assert_eq!(p1.as_windows_path(), r"C:\Windows\System32\cmd.exe");

        let p2 = resolver.resolve("c:/windows/system32/cmd.exe", "/").unwrap();
        assert_eq!(p2.as_unix_path(), "/c/windows/system32/cmd.exe");

        let p3 = resolver.resolve(r"D:\data\file.txt", "/").unwrap();
        assert_eq!(p3.as_unix_path(), "/d/data/file.txt");
        assert_eq!(p3.as_windows_path(), r"D:\data\file.txt");
    }

    #[test]
    fn test_windows_nt_object_namespace() {
        let resolver = UnifiedPathResolver::new();

        let p1 = resolver.resolve(r"\Device\HarddiskVolume1\Windows\System32", "/").unwrap();
        assert_eq!(p1.as_unix_path(), "/c/Windows/System32");

        let p2 = resolver.resolve(r"\SystemRoot\System32\cmd.exe", "/").unwrap();
        assert_eq!(p2.as_unix_path(), "/c/Windows/System32/cmd.exe");

        let p3 = resolver.resolve(r"\DosDevices\C:\Windows", "/").unwrap();
        assert_eq!(p3.as_unix_path(), "/c/Windows");

        let p4 = resolver.resolve(r"\??\C:\boot.ini", "/").unwrap();
        assert_eq!(p4.as_unix_path(), "/c/boot.ini");

        let p5 = resolver.resolve(r"\Device\Null", "/").unwrap();
        assert_eq!(p5.as_unix_path(), "/dev/null");

        let p6 = resolver.resolve("NUL", "/").unwrap();
        assert_eq!(p6.as_unix_path(), "/dev/null");
    }

    #[test]
    fn test_canonical_path_helpers() {
        let resolver = UnifiedPathResolver::new();
        let p = resolver.resolve(r"C:\Windows\System32\cmd.exe", "/").unwrap();

        assert_eq!(p.file_name(), Some("cmd.exe"));
        assert_eq!(p.extension(), Some("exe"));
        let parent = p.parent().unwrap();
        assert_eq!(parent.as_unix_path(), "/c/Windows/System32");
    }
}
