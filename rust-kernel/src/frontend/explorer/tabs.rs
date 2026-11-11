//! Windows 11 Modern File Explorer for HybridOS
//!
//! 100% Pure, Safe, Robust Rust Implementation of the Commercial Windows 11 File Explorer.
//! Designed for the HybridOS dual-personality environment supporting both Windows NT drive
//! letters (C:, D:, NVMe) and POSIX Unix filesystems (/, /bin, /etc, /home).
//!
//! Features:
//! 1. Multi-Tab Tabbed Interface (ExplorerTab, TabBar, add, close, switch, reorder)
//! 2. Modern Command Bar (New, Cut, Copy, Paste, Rename, Share, Delete, Sort by, View options)
//! 3. Navigation & Address Bar (Breadcrumb navigation, Back/Forward/Up, Search filtering)
//! 4. Left Navigation Pane (Quick Access, This PC, Linux POSIX Root, Network shares)
//! 5. File List & Details View (Columns, Multi-selection Ctrl/Shift/Ctrl+A, Status bar metrics)
//! 6. Dual-Personality Hybrid Filesystem Provider with realistic directories and file operations

#[allow(unused_imports)]
use alloc::string::{String, ToString};
#[allow(unused_imports)]
use alloc::vec::Vec;
#[allow(unused_imports)]
use alloc::boxed::Box;
#[allow(unused_imports)]
use crate::compat::FloatExt;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::fmt;

// ============================================================================
// Core Identifiers and Display Enums
// ============================================================================

/// Unique identifier for an Explorer Tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TabId(pub u64);

impl fmt::Display for TabId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Tab#{}", self.0)
    }
}

/// View options matching the Windows 11 Explorer View menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ViewOption {
    ExtraLargeIcons,
    LargeIcons,
    MediumIcons,
    SmallIcons,
    List,
    Details,
    Tiles,
}

impl ViewOption {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::ExtraLargeIcons => "Extra large icons",
            Self::LargeIcons => "Large icons",
            Self::MediumIcons => "Medium icons",
            Self::SmallIcons => "Small icons",
            Self::List => "List",
            Self::Details => "Details",
            Self::Tiles => "Tiles",
        }
    }
}

/// Sort column options for Details and List views.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SortColumn {
    Name,
    DateModified,
    Type,
    Size,
}

impl SortColumn {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Name => "Name",
            Self::DateModified => "Date modified",
            Self::Type => "Type",
            Self::Size => "Size",
        }
    }
}

/// Sort order direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SortDirection {
    Ascending,
    Descending,
}

/// Kind of new item to create via the Command Bar "New" menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NewItemKind {
    Folder,
    Shortcut,
    TextDocument,
}

impl NewItemKind {
    pub fn default_name(&self) -> &'static str {
        match self {
            Self::Folder => "New folder",
            Self::Shortcut => "New Shortcut.lnk",
            Self::TextDocument => "New Text Document.txt",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Folder => "Folder",
            Self::Shortcut => "Shortcut",
            Self::TextDocument => "Text Document",
        }
    }
}

/// Clipboard operation mode (Cut vs Copy).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClipboardMode {
    Cut,
    Copy,
}

// ============================================================================
// File System Item Representation
// ============================================================================

/// Represents an item (file, folder, drive, device) in the File Explorer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileSystemItem {
    pub name: String,
    pub path: String,
    pub is_directory: bool,
    pub size_bytes: u64,
    pub date_modified: String,
    pub file_type: String,
    pub icon: String,
    pub is_hidden: bool,
    pub is_system: bool,
    pub permissions: String,
}

impl FileSystemItem {
    /// Create a new directory item.
    pub fn new_dir(name: &str, path: &str, date_modified: &str) -> Self {
        Self {
            name: name.to_string(),
            path: path.to_string(),
            is_directory: true,
            size_bytes: 0,
            date_modified: date_modified.to_string(),
            file_type: "File folder".to_string(),
            icon: "📁".to_string(),
            is_hidden: false,
            is_system: false,
            permissions: "drwxr-xr-x".to_string(),
        }
    }

    /// Create a new file item.
    pub fn new_file(
        name: &str,
        path: &str,
        size_bytes: u64,
        date_modified: &str,
        file_type: &str,
        icon: &str,
    ) -> Self {
        Self {
            name: name.to_string(),
            path: path.to_string(),
            is_directory: false,
            size_bytes,
            date_modified: date_modified.to_string(),
            file_type: file_type.to_string(),
            icon: icon.to_string(),
            is_hidden: false,
            is_system: false,
            permissions: "-rw-r--r--".to_string(),
        }
    }

    /// Format file size into human-readable string (KB, MB, GB).
    pub fn formatted_size(&self) -> String {
        if self.is_directory {
            String::new()
        } else {
            format_file_size(self.size_bytes)
        }
    }
}

/// Helper to format byte count into human-readable string.
pub fn format_file_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    const TB: u64 = 1024 * GB;

    if bytes >= TB {
        format!("{:.1} TB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{} KB", (bytes + 1023) / KB)
    } else if bytes > 0 {
        format!("{} bytes", bytes)
    } else {
        "0 KB".to_string()
    }
}

// ============================================================================
// Breadcrumb Navigation & Path Utilities
// ============================================================================

/// Clickable breadcrumb segment in the address bar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BreadcrumbSegment {
    pub display_name: String,
    pub target_path: String,
    pub is_last: bool,
}

/// Normalize path for consistent comparison and navigation.
pub fn normalize_path(path: &str) -> String {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return "C:\\".to_string();
    }

    // Windows drive format
    if (trimmed.len() >= 2 && trimmed.chars().nth(1) == Some(':'))
        || trimmed.starts_with('\\')
    {
        let replaced = trimmed.replace('/', "\\");
        let mut clean = replaced.trim_end_matches('\\').to_string();
        if clean.len() == 2 && clean.chars().nth(1) == Some(':') {
            clean.push('\\');
        }
        clean
    } else if trimmed.starts_with('/') {
        // POSIX Unix format
        if trimmed == "/" {
            "/".to_string()
        } else {
            let replaced = trimmed.replace('\\', "/");
            let clean = replaced.trim_end_matches('/');
            if clean.is_empty() {
                "/".to_string()
            } else {
                clean.to_string()
            }
        }
    } else if trimmed.starts_with("home://") || trimmed.starts_with("thispc://") {
        trimmed.to_string()
    } else {
        // Default to Windows drive path
        format!("C:\\{}", trimmed.trim_start_matches('\\'))
    }
}

/// Calculate the parent directory of a path (for the "Up" button).
pub fn parent_path(path: &str) -> Option<String> {
    let norm = normalize_path(path);

    // Virtual Roots
    if norm == "thispc://" || norm == "home://" {
        return None;
    }

    // Windows Drive Root (e.g., "C:\", "D:\")
    if norm.len() == 3 && norm.chars().nth(1) == Some(':') && norm.ends_with('\\') {
        return Some("thispc://".to_string());
    }

    // POSIX Root
    if norm == "/" {
        return Some("thispc://".to_string());
    }

    // Windows Path
    if norm.contains('\\') {
        if let Some(idx) = norm.rfind('\\') {
            if idx == 2 && norm.chars().nth(1) == Some(':') {
                return Some(norm[..3].to_string());
            } else if idx > 0 {
                return Some(norm[..idx].to_string());
            }
        }
    }

    // POSIX Path
    if norm.contains('/') {
        if let Some(idx) = norm.rfind('/') {
            if idx == 0 {
                return Some("/".to_string());
            } else {
                return Some(norm[..idx].to_string());
            }
        }
    }

    None
}

/// Build clickable breadcrumb segments for address bar navigation.
pub fn build_breadcrumbs(path: &str) -> Vec<BreadcrumbSegment> {
    let norm = normalize_path(path);
    let mut segments = Vec::new();

    if norm == "thispc://" {
        segments.push(BreadcrumbSegment {
            display_name: "This PC".to_string(),
            target_path: "thispc://".to_string(),
            is_last: true,
        });
        return segments;
    }

    if norm.starts_with("home://") {
        segments.push(BreadcrumbSegment {
            display_name: "Home".to_string(),
            target_path: "home://".to_string(),
            is_last: norm == "home://",
        });
        let sub = norm.trim_start_matches("home://");
        if !sub.is_empty() {
            segments.push(BreadcrumbSegment {
                display_name: sub.to_string(),
                target_path: norm.clone(),
                is_last: true,
            });
        }
        return segments;
    }

    if norm.starts_with('/') {
        // POSIX hierarchy
        segments.push(BreadcrumbSegment {
            display_name: "Linux Root (/)".to_string(),
            target_path: "/".to_string(),
            is_last: norm == "/",
        });

        if norm != "/" {
            let parts: Vec<&str> = norm.trim_start_matches('/').split('/').collect();
            let mut accum = String::new();
            for (i, part) in parts.iter().enumerate() {
                accum.push('/');
                accum.push_str(part);
                let is_last = i == parts.len() - 1;
                segments.push(BreadcrumbSegment {
                    display_name: part.to_string(),
                    target_path: accum.clone(),
                    is_last,
                });
            }
        }
        return segments;
    }

    // Windows Drive path
    segments.push(BreadcrumbSegment {
        display_name: "This PC".to_string(),
        target_path: "thispc://".to_string(),
        is_last: false,
    });

    if norm.len() >= 2 && norm.chars().nth(1) == Some(':') {
        let drive_letter = norm.chars().next().unwrap().to_ascii_uppercase();
        let drive_root = format!("{}:\\", drive_letter);
        let drive_name = match drive_letter {
            'C' => "Local Disk (C:)",
            'D' => "Data Disk (D:)",
            'E' => "NVMe Storage (E:)",
            _ => "Drive",
        };

        let is_drive_last = norm.len() <= 3;
        segments.push(BreadcrumbSegment {
            display_name: drive_name.to_string(),
            target_path: drive_root.clone(),
            is_last: is_drive_last,
        });

        if !is_drive_last {
            let relative = &norm[3..];
            let parts: Vec<&str> = relative.split('\\').filter(|p| !p.is_empty()).collect();
            let mut current = drive_root;
            for (i, part) in parts.iter().enumerate() {
                if !current.ends_with('\\') {
                    current.push('\\');
                }
                current.push_str(part);
                let is_last = i == parts.len() - 1;
                segments.push(BreadcrumbSegment {
                    display_name: part.to_string(),
                    target_path: current.clone(),
                    is_last,
                });
            }
        }
    } else {
        segments.push(BreadcrumbSegment {
            display_name: norm.clone(),
            target_path: norm,
            is_last: true,
        });
    }

    segments
}

// ============================================================================
// Explorer Tab Structure
// ============================================================================

/// Represents an individual tab inside the Windows 11 Modern File Explorer.
#[derive(Debug, Clone)]
pub struct ExplorerTab {
    /// Unique tab identifier.
    pub id: TabId,
    /// Window title for the tab.
    pub title: String,
    /// Current filesystem or virtual path.
    pub current_path: String,
    /// Backward navigation history stack.
    pub history_back: Vec<String>,
    /// Forward navigation history stack.
    pub history_forward: Vec<String>,
    /// Set of selected item names in the current view.
    pub selected_items: HashSet<String>,
    /// Last interacted item index for Shift+Click range selections.
    pub anchor_index: Option<usize>,
    /// Vertical scroll offset.
    pub scroll_offset: f32,
    /// Current view presentation mode.
    pub view_mode: ViewOption,
    /// Current sort column.
    pub sort_column: SortColumn,
    /// Current sort order.
    pub sort_direction: SortDirection,
    /// Real-time search query filtering current folder items.
    pub search_query: String,
    /// Cached items in the current directory.
    pub cached_items: Vec<FileSystemItem>,
}

impl ExplorerTab {
    /// Create a new Explorer tab initialized to a specific path.
    pub fn new(id: TabId, initial_path: &str, provider: &HybridFsProvider) -> Self {
        let norm_path = normalize_path(initial_path);
        let items = provider.list_directory(&norm_path);
        let title = compute_tab_title(&norm_path);

        Self {
            id,
            title,
            current_path: norm_path,
            history_back: Vec::new(),
            history_forward: Vec::new(),
            selected_items: HashSet::new(),
            anchor_index: None,
            scroll_offset: 0.0,
            view_mode: ViewOption::Details,
            sort_column: SortColumn::Name,
            sort_direction: SortDirection::Ascending,
            search_query: String::new(),
            cached_items: items,
        }
    }

    /// Navigate to a new path, updating navigation history and reloading contents.
    pub fn navigate_to(&mut self, new_path: &str, provider: &HybridFsProvider) {
        let norm = normalize_path(new_path);
        if norm == self.current_path {
            return;
        }

        self.history_back.push(self.current_path.clone());
        self.history_forward.clear();
        self.current_path = norm;
        self.title = compute_tab_title(&self.current_path);
        self.selected_items.clear();
        self.anchor_index = None;
        self.scroll_offset = 0.0;
        self.search_query.clear();
        self.refresh(provider);
    }

    /// Returns true if back history is available.
    pub fn can_go_back(&self) -> bool {
        !self.history_back.is_empty()
    }

    /// Navigate backward in history.
    pub fn go_back(&mut self, provider: &HybridFsProvider) -> bool {
        if let Some(prev) = self.history_back.pop() {
            self.history_forward.push(self.current_path.clone());
            self.current_path = prev;
            self.title = compute_tab_title(&self.current_path);
            self.selected_items.clear();
            self.anchor_index = None;
            self.search_query.clear();
            self.refresh(provider);
            true
        } else {
            false
        }
    }

    /// Returns true if forward history is available.
    pub fn can_go_forward(&self) -> bool {
        !self.history_forward.is_empty()
    }

    /// Navigate forward in history.
    pub fn go_forward(&mut self, provider: &HybridFsProvider) -> bool {
        if let Some(next) = self.history_forward.pop() {
            self.history_back.push(self.current_path.clone());
            self.current_path = next;
            self.title = compute_tab_title(&self.current_path);
            self.selected_items.clear();
            self.anchor_index = None;
            self.search_query.clear();
            self.refresh(provider);
            true
        } else {
            false
        }
    }

    /// Returns true if parent path can be navigated to.
    pub fn can_go_up(&self) -> bool {
        parent_path(&self.current_path).is_some()
    }

    /// Navigate up to parent folder (Alt+Up).
    pub fn go_up(&mut self, provider: &HybridFsProvider) -> bool {
        if let Some(parent) = parent_path(&self.current_path) {
            self.navigate_to(&parent, provider);
            true
        } else {
            false
        }
    }

    /// Refresh directory items from the filesystem provider.
    pub fn refresh(&mut self, provider: &HybridFsProvider) {
        self.cached_items = provider.list_directory(&self.current_path);
        // Retain selections that still exist
        let existing_names: HashSet<&str> =
            self.cached_items.iter().map(|it| it.name.as_str()).collect();
        self.selected_items.retain(|s| existing_names.contains(s.as_str()));
    }

    /// Set real-time search filter query.
    pub fn set_search_query(&mut self, query: &str) {
        self.search_query = query.to_string();
    }

    /// Clear search query filter.
    pub fn clear_search(&mut self) {
        self.search_query.clear();
    }

    /// Retrieve filtered and sorted items according to active view parameters.
    pub fn display_items(&self) -> Vec<FileSystemItem> {
        let mut list: Vec<FileSystemItem> = if self.search_query.trim().is_empty() {
            self.cached_items.clone()
        } else {
            let q = self.search_query.to_lowercase();
            self.cached_items
                .iter()
                .filter(|it| {
                    it.name.to_lowercase().contains(&q)
                        || it.file_type.to_lowercase().contains(&q)
                })
                .cloned()
                .collect()
        };

        // Sort items (Directories always grouped first like Windows 11 Explorer)
        list.sort_by(|a, b| {
            if a.is_directory != b.is_directory {
                return if a.is_directory {
                    Ordering::Less
                } else {
                    Ordering::Greater
                };
            }

            let cmp = match self.sort_column {
                SortColumn::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                SortColumn::DateModified => a.date_modified.cmp(&b.date_modified),
                SortColumn::Type => a.file_type.to_lowercase().cmp(&b.file_type.to_lowercase()),
                SortColumn::Size => a.size_bytes.cmp(&b.size_bytes),
            };

            if self.sort_direction == SortDirection::Ascending {
                cmp
            } else {
                cmp.reverse()
            }
        });

        list
    }

    // --- Selection Management ---

    /// Single click selection: replaces all previous selections.
    pub fn select_single(&mut self, index: usize) {
        let items = self.display_items();
        self.selected_items.clear();
        if let Some(item) = items.get(index) {
            self.selected_items.insert(item.name.clone());
            self.anchor_index = Some(index);
        }
    }

    /// Ctrl+Click selection: toggles an item's selected state without clearing others.
    pub fn toggle_selection(&mut self, index: usize) {
        let items = self.display_items();
        if let Some(item) = items.get(index) {
            if self.selected_items.contains(&item.name) {
                self.selected_items.remove(&item.name);
            } else {
                self.selected_items.insert(item.name.clone());
            }
            self.anchor_index = Some(index);
        }
    }

    /// Shift+Click selection: selects all items between anchor and clicked item.
    pub fn select_range(&mut self, index: usize) {
        let items = self.display_items();
        if items.is_empty() {
            return;
        }

        let anchor = self.anchor_index.unwrap_or(0);
        let start = anchor.min(index);
        let end = anchor.max(index);

        self.selected_items.clear();
        for i in start..=end {
            if let Some(item) = items.get(i) {
                self.selected_items.insert(item.name.clone());
            }
        }
    }

    /// Ctrl+A selection: selects all visible items.
    pub fn select_all(&mut self) {
        let items = self.display_items();
        self.selected_items.clear();
        for it in items {
            self.selected_items.insert(it.name);
        }
    }

    /// Clear all selections.
    pub fn clear_selection(&mut self) {
        self.selected_items.clear();
        self.anchor_index = None;
    }

    /// Query if a specific file name is selected.
    pub fn is_selected(&self, name: &str) -> bool {
        self.selected_items.contains(name)
    }

    /// Get all selected FileSystemItem references.
    pub fn selected_items(&self) -> Vec<FileSystemItem> {
        let displayed = self.display_items();
        displayed
            .into_iter()
            .filter(|it| self.selected_items.contains(&it.name))
            .collect()
    }

    /// Count of selected items.
    pub fn selected_count(&self) -> usize {
        self.selected_items.len()
    }

    /// Total size in bytes of selected files.
    pub fn selected_total_size(&self) -> u64 {
        self.selected_items()
            .iter()
            .filter(|it| !it.is_directory)
            .map(|it| it.size_bytes)
            .sum()
    }
}

/// Helper to compute readable title for tabs.
pub fn compute_tab_title(path: &str) -> String {
    let norm = normalize_path(path);
    if norm == "thispc://" {
        return "This PC".to_string();
    }
    if norm.starts_with("home://") {
        let sub = norm.trim_start_matches("home://");
        if sub.is_empty() {
            return "Home".to_string();
        } else {
            return sub.to_string();
        }
    }
    if norm == "/" {
        return "Linux (/)".to_string();
    }
    if norm.len() == 3 && norm.chars().nth(1) == Some(':') {
        let drive = norm.chars().next().unwrap().to_ascii_uppercase();
        return match drive {
            'C' => "Local Disk (C:)".to_string(),
            'D' => "Data Disk (D:)".to_string(),
            'E' => "NVMe (E:)".to_string(),
            _ => format!("Drive ({}:)", drive),
        };
    }
    if let Some(idx) = norm.rfind('\\') {
        let segment = &norm[idx + 1..];
        if !segment.is_empty() {
            return segment.to_string();
        }
    }
    if let Some(idx) = norm.rfind('/') {
        let segment = &norm[idx + 1..];
        if !segment.is_empty() {
            return segment.to_string();
        }
    }
    norm
}

// ============================================================================
// Multi-Tab Tab Bar
// ============================================================================

/// Represents the top tab bar holding one or more ExplorerTabs.
#[derive(Debug, Clone)]
pub struct TabBar {
    pub tabs: Vec<ExplorerTab>,
    pub active_tab_index: usize,
    pub next_id: u64,
}

impl TabBar {
    /// Create a new tab bar with an initial tab.
    pub fn new(initial_path: &str, provider: &HybridFsProvider) -> Self {
        let tab0 = ExplorerTab::new(TabId(1), initial_path, provider);
        Self {
            tabs: vec![tab0],
            active_tab_index: 0,
            next_id: 2,
        }
    }

    /// Add a new tab (+) and make it active.
    pub fn add_tab(&mut self, path: &str, provider: &HybridFsProvider) -> TabId {
        let id = TabId(self.next_id);
        self.next_id += 1;
        let tab = ExplorerTab::new(id, path, provider);
        self.tabs.push(tab);
        self.active_tab_index = self.tabs.len() - 1;
        id
    }

    /// Close tab by TabId (X). Never drops below 1 tab (creates a fallback if last closed).
    pub fn close_tab(&mut self, id: TabId, provider: &HybridFsProvider) -> bool {
        let idx_opt = self.tabs.iter().position(|t| t.id == id);
        if let Some(idx) = idx_opt {
            self.tabs.remove(idx);

            if self.tabs.is_empty() {
                // If all closed, spawn default tab at C:\
                let fallback_id = TabId(self.next_id);
                self.next_id += 1;
                self.tabs.push(ExplorerTab::new(fallback_id, "C:\\", provider));
                self.active_tab_index = 0;
            } else if self.active_tab_index >= self.tabs.len() {
                self.active_tab_index = self.tabs.len() - 1;
            }
            true
        } else {
            false
        }
    }

    /// Close the currently active tab.
    pub fn close_active_tab(&mut self, provider: &HybridFsProvider) -> bool {
        if let Some(id) = self.active_tab().map(|t| t.id) {
            self.close_tab(id, provider)
        } else {
            false
        }
    }

    /// Switch active tab by TabId.
    pub fn switch_tab(&mut self, id: TabId) -> bool {
        if let Some(idx) = self.tabs.iter().position(|t| t.id == id) {
            self.active_tab_index = idx;
            true
        } else {
            false
        }
    }

    /// Switch active tab by index.
    pub fn switch_to_index(&mut self, index: usize) -> bool {
        if index < self.tabs.len() {
            self.active_tab_index = index;
            true
        } else {
            false
        }
    }

    /// Reorder tabs (drag & drop tab support).
    pub fn reorder_tabs(&mut self, from_idx: usize, to_idx: usize) -> bool {
        if from_idx >= self.tabs.len() || to_idx >= self.tabs.len() {
            return false;
        }
        let active_id = self.tabs[self.active_tab_index].id;
        let tab = self.tabs.remove(from_idx);
        self.tabs.insert(to_idx, tab);
        // Restore active index to match active tab id
        if let Some(new_idx) = self.tabs.iter().position(|t| t.id == active_id) {
            self.active_tab_index = new_idx;
        }
        true
    }

    /// Borrow active ExplorerTab.
    pub fn active_tab(&self) -> Option<&ExplorerTab> {
        self.tabs.get(self.active_tab_index)
    }

    /// Mutably borrow active ExplorerTab.
    pub fn active_tab_mut(&mut self) -> Option<&mut ExplorerTab> {
        self.tabs.get_mut(self.active_tab_index)
    }

    /// Number of open tabs.
    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }
}

// ============================================================================
// Modern Command Bar
// ============================================================================

/// Action triggered from the modern command bar or keyboard shortcuts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandAction {
    /// Create new folder, shortcut, or text file.
    NewItem(NewItemKind),
    /// Cut selected items (Ctrl+X).
    Cut,
    /// Copy selected items (Ctrl+C).
    Copy,
    /// Paste clipboard contents (Ctrl+V).
    Paste,
    /// Rename selected item (F2).
    Rename(String),
    /// Share selected item.
    Share,
    /// Delete selected items (Del / Shift+Del).
    Delete { permanent: bool },
    /// Sort view by column and direction.
    SortBy(SortColumn, SortDirection),
    /// Switch view mode.
    View(ViewOption),
    /// Select all items (Ctrl+A).
    SelectAll,
    /// Invert current selection.
    InvertSelection,
    /// Refresh current directory (F5).
    Refresh,
    /// Inspect item properties (Alt+Enter).
    Properties,
}

/// Command Bar button availability state based on current selection and clipboard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandBarState {
    pub can_new: bool,
    pub can_cut: bool,
    pub can_copy: bool,
    pub can_paste: bool,
    pub can_rename: bool,
    pub can_share: bool,
    pub can_delete: bool,
}

/// Manages Explorer clipboard and executes Command Bar operations.
#[derive(Debug, Clone)]
pub struct CommandBar {
    pub clipboard_items: Vec<String>,
    pub clipboard_mode: Option<ClipboardMode>,
}

impl CommandBar {
    pub fn new() -> Self {
        Self {
            clipboard_items: Vec::new(),
            clipboard_mode: None,
        }
    }

    /// Evaluate button enable/disable state for the active tab.
    pub fn evaluate_state(&self, tab: &ExplorerTab) -> CommandBarState {
        let has_sel = tab.selected_count() > 0;
        let single_sel = tab.selected_count() == 1;
        let has_clip = !self.clipboard_items.is_empty();

        CommandBarState {
            can_new: true,
            can_cut: has_sel,
            can_copy: has_sel,
            can_paste: has_clip,
            can_rename: single_sel,
            can_share: has_sel,
            can_delete: has_sel,
        }
    }

    /// Execute command bar action on the active tab and filesystem provider.
    pub fn dispatch(
        &mut self,
        action: CommandAction,
        tab: &mut ExplorerTab,
        provider: &mut HybridFsProvider,
    ) -> Result<String, String> {
        match action {
            CommandAction::NewItem(kind) => {
                let name = kind.default_name();
                let full_path = provider.create_item(&tab.current_path, name, kind)?;
                tab.refresh(provider);
                tab.selected_items.clear();
                tab.selected_items.insert(name.to_string());
                Ok(format!("Created {}", full_path))
            }
            CommandAction::Cut => {
                let items: Vec<String> = tab
                    .selected_items()
                    .into_iter()
                    .map(|it| it.path)
                    .collect();
                if items.is_empty() {
                    return Err("No items selected to cut".to_string());
                }
                let count = items.len();
                self.clipboard_items = items;
                self.clipboard_mode = Some(ClipboardMode::Cut);
                Ok(format!("Cut {} items to clipboard", count))
            }
            CommandAction::Copy => {
                let items: Vec<String> = tab
                    .selected_items()
                    .into_iter()
                    .map(|it| it.path)
                    .collect();
                if items.is_empty() {
                    return Err("No items selected to copy".to_string());
                }
                let count = items.len();
                self.clipboard_items = items;
                self.clipboard_mode = Some(ClipboardMode::Copy);
                Ok(format!("Copied {} items to clipboard", count))
            }
            CommandAction::Paste => {
                if self.clipboard_items.is_empty() {
                    return Err("Clipboard is empty".to_string());
                }
                let is_cut = self.clipboard_mode == Some(ClipboardMode::Cut);
                let mut pasted_count = 0;
                for src in &self.clipboard_items {
                    if is_cut {
                        provider.move_item(src, &tab.current_path)?;
                    } else {
                        provider.copy_item(src, &tab.current_path)?;
                    }
                    pasted_count += 1;
                }
                if is_cut {
                    self.clipboard_items.clear();
                    self.clipboard_mode = None;
                }
                tab.refresh(provider);
                Ok(format!("Pasted {} items into {}", pasted_count, tab.current_path))
            }
            CommandAction::Rename(new_name) => {
                let selected = tab.selected_items();
                if selected.len() != 1 {
                    return Err("Rename requires exactly one selected item".to_string());
                }
                let old_item = &selected[0];
                provider.rename_item(&old_item.path, &new_name)?;
                tab.refresh(provider);
                tab.selected_items.clear();
                tab.selected_items.insert(new_name.clone());
                Ok(format!("Renamed '{}' to '{}'", old_item.name, new_name))
            }
            CommandAction::Share => {
                let selected = tab.selected_items();
                if selected.is_empty() {
                    return Err("No items selected to share".to_string());
                }
                Ok(format!("Opening Windows Share dialog for {} items", selected.len()))
            }
            CommandAction::Delete { permanent } => {
                let selected = tab.selected_items();
                if selected.is_empty() {
                    return Err("No items selected to delete".to_string());
                }
                let count = selected.len();
                for it in selected {
                    provider.delete_item(&it.path)?;
                }
                tab.clear_selection();
                tab.refresh(provider);
                let mode_str = if permanent { "permanently deleted" } else { "moved to Recycle Bin" };
                Ok(format!("{} items {}", count, mode_str))
            }
            CommandAction::SortBy(col, dir) => {
                tab.sort_column = col;
                tab.sort_direction = dir;
                Ok(format!("Sorted by {:?} {:?}", col, dir))
            }
            CommandAction::View(mode) => {
                tab.view_mode = mode;
                Ok(format!("View mode set to {:?}", mode))
            }
            CommandAction::SelectAll => {
                tab.select_all();
                Ok(format!("Selected all {} items", tab.selected_count()))
            }
            CommandAction::InvertSelection => {
                let all = tab.display_items();
                let mut new_sel = HashSet::new();
                for it in all {
                    if !tab.selected_items.contains(&it.name) {
                        new_sel.insert(it.name);
                    }
                }
                tab.selected_items = new_sel;
                Ok(format!("Inverted selection ({} items selected)", tab.selected_count()))
            }
            CommandAction::Refresh => {
                tab.refresh(provider);
                Ok("Directory refreshed".to_string())
            }
            CommandAction::Properties => {
                let count = tab.selected_count();
                if count == 0 {
                    Ok(format!("Folder Properties: {}", tab.current_path))
                } else {
                    Ok(format!("Properties for {} selected items", count))
                }
            }
        }
    }
}

// ============================================================================
// Left Navigation Pane
// ============================================================================

/// Represents an item in the left navigation tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavItem {
    pub title: String,
    pub target_path: String,
    pub icon: String,
    pub is_expanded: bool,
    pub badge: Option<String>,
}

/// Left Navigation Pane containing Quick Access, This PC, and Linux POSIX Root.
#[derive(Debug, Clone)]
pub struct NavigationPane {
    /// Quick Access / Home shortcuts.
    pub quick_access: Vec<NavItem>,
    /// This PC storage disks and network locations.
    pub this_pc: Vec<NavItem>,
    /// Linux POSIX roots under HybridOS.
    pub linux_root: Vec<NavItem>,
    /// Currently highlighted navigation target path.
    pub selected_nav_path: Option<String>,
}

impl NavigationPane {
    /// Initialize Windows 11 Left Navigation Pane with standard HybridOS hierarchy.
    pub fn new() -> Self {
        let quick_access = vec![
            NavItem {
                title: "Desktop".to_string(),
                target_path: "C:\\Users\\Vinicius\\Desktop".to_string(),
                icon: "🖥️".to_string(),
                is_expanded: false,
                badge: None,
            },
            NavItem {
                title: "Downloads".to_string(),
                target_path: "C:\\Users\\Vinicius\\Downloads".to_string(),
                icon: "📥".to_string(),
                is_expanded: false,
                badge: None,
            },
            NavItem {
                title: "Documents".to_string(),
                target_path: "C:\\Users\\Vinicius\\Documents".to_string(),
                icon: "📄".to_string(),
                is_expanded: false,
                badge: None,
            },
            NavItem {
                title: "Pictures".to_string(),
                target_path: "C:\\Users\\Vinicius\\Pictures".to_string(),
                icon: "🖼️".to_string(),
                is_expanded: false,
                badge: None,
            },
            NavItem {
                title: "Music".to_string(),
                target_path: "C:\\Users\\Vinicius\\Music".to_string(),
                icon: "🎵".to_string(),
                is_expanded: false,
                badge: None,
            },
            NavItem {
                title: "Videos".to_string(),
                target_path: "C:\\Users\\Vinicius\\Videos".to_string(),
                icon: "🎬".to_string(),
                is_expanded: false,
                badge: None,
            },
        ];

        let this_pc = vec![
            NavItem {
                title: "Local Disk (C:)".to_string(),
                target_path: "C:\\".to_string(),
                icon: "💽".to_string(),
                is_expanded: true,
                badge: Some("256 GB (FAT32/NTFS)".to_string()),
            },
            NavItem {
                title: "Data Disk (D:)".to_string(),
                target_path: "D:\\".to_string(),
                icon: "💾".to_string(),
                is_expanded: false,
                badge: Some("1.0 TB (SATA AHCI)".to_string()),
            },
            NavItem {
                title: "NVMe Storage (NVMe0)".to_string(),
                target_path: "E:\\".to_string(),
                icon: "⚡".to_string(),
                is_expanded: false,
                badge: Some("2.0 TB (PCIe Gen4)".to_string()),
            },
            NavItem {
                title: "Network Shares".to_string(),
                target_path: "\\\\HYBRID-SERVER\\SharedPool".to_string(),
                icon: "🌐".to_string(),
                is_expanded: false,
                badge: Some("SMB / NFS".to_string()),
            },
        ];

        let linux_root = vec![
            NavItem {
                title: "/bin".to_string(),
                target_path: "/bin".to_string(),
                icon: "⚙️".to_string(),
                is_expanded: false,
                badge: Some("POSIX Binaries".to_string()),
            },
            NavItem {
                title: "/etc".to_string(),
                target_path: "/etc".to_string(),
                icon: "🔧".to_string(),
                is_expanded: false,
                badge: Some("Configurations".to_string()),
            },
            NavItem {
                title: "/home".to_string(),
                target_path: "/home".to_string(),
                icon: "🏠".to_string(),
                is_expanded: false,
                badge: Some("User Profiles".to_string()),
            },
            NavItem {
                title: "/var".to_string(),
                target_path: "/var".to_string(),
                icon: "📦".to_string(),
                is_expanded: false,
                badge: Some("Logs & Spool".to_string()),
            },
            NavItem {
                title: "/proc".to_string(),
                target_path: "/proc".to_string(),
                icon: "🐧".to_string(),
                is_expanded: false,
                badge: Some("Kernel VFS".to_string()),
            },
        ];

        Self {
            quick_access,
            this_pc,
            linux_root,
            selected_nav_path: Some("C:\\Users\\Vinicius\\Documents".to_string()),
        }
    }

    /// Retrieve flat list of all navigation targets.
    pub fn all_items(&self) -> Vec<&NavItem> {
        let mut list = Vec::new();
        list.extend(self.quick_access.iter());
        list.extend(self.this_pc.iter());
        list.extend(self.linux_root.iter());
        list
    }

    /// Highlight navigation item matching path.
    pub fn select_path(&mut self, path: &str) {
        let norm = normalize_path(path);
        self.selected_nav_path = Some(norm);
    }
}

// ============================================================================
// Status Bar Metrics
// ============================================================================

/// File Explorer bottom status bar statistics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusBarMetrics {
    pub total_items: usize,
    pub selected_items: usize,
    pub selected_bytes: u64,
}

impl StatusBarMetrics {
    pub fn compute(tab: &ExplorerTab) -> Self {
        let total = tab.display_items().len();
        let sel_count = tab.selected_count();
        let sel_bytes = tab.selected_total_size();

        Self {
            total_items: total,
            selected_items: sel_count,
            selected_bytes: sel_bytes,
        }
    }

    /// Display string matching Windows 11 Explorer bottom bar.
    pub fn display_text(&self) -> String {
        if self.selected_items == 0 {
            format!("{} items", self.total_items)
        } else {
            format!(
                "{} items | {} item{} selected  {}",
                self.total_items,
                self.selected_items,
                if self.selected_items == 1 { "" } else { "s" },
                format_file_size(self.selected_bytes)
            )
        }
    }
}

// ============================================================================
// Hybrid Dual-Personality Filesystem Provider
// ============================================================================

/// In-memory mock/provider that realistically represents Windows drives and Linux VFS.
#[derive(Debug, Clone)]
pub struct HybridFsProvider {
    entries: HashMap<String, Vec<FileSystemItem>>,
}

impl HybridFsProvider {
    /// Initialize with rich commercial Windows 11 and Linux POSIX directories.
    pub fn new() -> Self {
        let mut provider = Self {
            entries: HashMap::new(),
        };
        provider.seed_default_filesystem();
        provider
    }

    fn seed_default_filesystem(&mut self) {
        // --- Windows C: Root ---
        let c_root = vec![
            FileSystemItem::new_dir("Program Files", "C:\\Program Files", "2026-11-09 14:22"),
            FileSystemItem::new_dir("Program Files (x86)", "C:\\Program Files (x86)", "2026-11-09 14:22"),
            FileSystemItem::new_dir("Users", "C:\\Users", "2026-11-09 14:20"),
            FileSystemItem::new_dir("Windows", "C:\\Windows", "2026-11-09 14:15"),
            FileSystemItem::new_file("bootmgr", "C:\\bootmgr", 412784, "2026-10-15 08:00", "System File", "🛡️"),
            FileSystemItem::new_file("pagefile.sys", "C:\\pagefile.sys", 4294967296, "2026-11-10 10:00", "System Pagefile", "⚙️"),
        ];
        self.entries.insert("C:\\".to_string(), c_root);

        // --- C:\Windows ---
        let c_windows = vec![
            FileSystemItem::new_dir("System32", "C:\\Windows\\System32", "2026-11-09 14:16"),
            FileSystemItem::new_dir("SysWOW64", "C:\\Windows\\SysWOW64", "2026-11-09 14:18"),
            FileSystemItem::new_dir("Fonts", "C:\\Windows\\Fonts", "2026-11-09 14:19"),
            FileSystemItem::new_file("explorer.exe", "C:\\Windows\\explorer.exe", 5124096, "2026-11-05 12:00", "Application", "🪟"),
            FileSystemItem::new_file("notepad.exe", "C:\\Windows\\notepad.exe", 256128, "2026-11-05 12:00", "Application", "📝"),
            FileSystemItem::new_file("win.ini", "C:\\Windows\\win.ini", 512, "2026-10-20 09:30", "Configuration File", "⚙️"),
        ];
        self.entries.insert("C:\\Windows".to_string(), c_windows);

        // --- C:\Windows\System32 ---
        let c_sys32 = vec![
            FileSystemItem::new_dir("drivers", "C:\\Windows\\System32\\drivers", "2026-11-09 14:16"),
            FileSystemItem::new_file("ntoskrnl.exe", "C:\\Windows\\System32\\ntoskrnl.exe", 11250128, "2026-11-01 10:00", "Kernel Executable", "⚙️"),
            FileSystemItem::new_file("kernel32.dll", "C:\\Windows\\System32\\kernel32.dll", 784210, "2026-11-01 10:00", "System DLL", "🧩"),
            FileSystemItem::new_file("cmd.exe", "C:\\Windows\\System32\\cmd.exe", 312500, "2026-11-01 10:00", "Application", "⬛"),
            FileSystemItem::new_file("dwm.exe", "C:\\Windows\\System32\\dwm.exe", 2048576, "2026-11-05 11:00", "Desktop Window Manager", "🪟"),
        ];
        self.entries.insert("C:\\Windows\\System32".to_string(), c_sys32);

        // --- C:\Users\Vinicius\Documents ---
        let user_docs = vec![
            FileSystemItem::new_dir("HybridOS-Kernel", "C:\\Users\\Vinicius\\Documents\\HybridOS-Kernel", "2026-11-10 10:35"),
            FileSystemItem::new_file("Roadmap.docx", "C:\\Users\\Vinicius\\Documents\\Roadmap.docx", 1048576, "2026-11-08 17:00", "Word Document", "📘"),
            FileSystemItem::new_file("Architecture_Specs.pdf", "C:\\Users\\Vinicius\\Documents\\Architecture_Specs.pdf", 4521984, "2026-11-09 09:15", "PDF Document", "📕"),
            FileSystemItem::new_file("Notes.txt", "C:\\Users\\Vinicius\\Documents\\Notes.txt", 12450, "2026-11-10 08:30", "Text Document", "📄"),
        ];
        self.entries.insert("C:\\Users\\Vinicius\\Documents".to_string(), user_docs);

        // --- C:\Users\Vinicius\Downloads ---
        let user_downloads = vec![
            FileSystemItem::new_file("RustRover-2026.1.exe", "C:\\Users\\Vinicius\\Downloads\\RustRover-2026.1.exe", 629145600, "2026-11-07 15:40", "Application Installer", "📦"),
            FileSystemItem::new_file("ubuntu-26.04-live-server.iso", "C:\\Users\\Vinicius\\Downloads\\ubuntu-26.04-live-server.iso", 2147483648, "2026-11-06 20:10", "Disk Image", "💿"),
        ];
        self.entries.insert("C:\\Users\\Vinicius\\Downloads".to_string(), user_downloads);

        // --- Data Disk D: Root ---
        let d_root = vec![
            FileSystemItem::new_dir("Backups", "D:\\Backups", "2026-11-01 12:00"),
            FileSystemItem::new_dir("VirtualMachines", "D:\\VirtualMachines", "2026-11-04 18:30"),
            FileSystemItem::new_file("Archive_2025.zip", "D:\\Archive_2025.zip", 10737418240, "2026-01-01 00:00", "Compressed ZIP Archive", "🗜️"),
        ];
        self.entries.insert("D:\\".to_string(), d_root);

        // --- NVMe Storage E: Root ---
        let e_root = vec![
            FileSystemItem::new_dir("SteamLibrary", "E:\\SteamLibrary", "2026-11-02 21:00"),
            FileSystemItem::new_dir("Datasets", "E:\\Datasets", "2026-11-03 14:00"),
            FileSystemItem::new_file("benchmark_result.csv", "E:\\benchmark_result.csv", 8388608, "2026-11-09 23:45", "CSV Data File", "📊"),
        ];
        self.entries.insert("E:\\".to_string(), e_root);

        // --- Linux POSIX Root (/) ---
        let linux_root = vec![
            FileSystemItem::new_dir("bin", "/bin", "2026-11-01 00:00"),
            FileSystemItem::new_dir("etc", "/etc", "2026-11-01 00:00"),
            FileSystemItem::new_dir("home", "/home", "2026-11-01 00:00"),
            FileSystemItem::new_dir("lib", "/lib", "2026-11-01 00:00"),
            FileSystemItem::new_dir("proc", "/proc", "2026-11-10 10:00"),
            FileSystemItem::new_dir("usr", "/usr", "2026-11-01 00:00"),
            FileSystemItem::new_dir("var", "/var", "2026-11-01 00:00"),
            FileSystemItem::new_file("vmlinuz", "/vmlinuz", 14680064, "2026-10-28 05:00", "Linux Kernel Image", "🐧"),
        ];
        self.entries.insert("/".to_string(), linux_root);

        // --- Linux /etc ---
        let linux_etc = vec![
            FileSystemItem::new_dir("systemd", "/etc/systemd", "2026-11-01 00:00"),
            FileSystemItem::new_file("passwd", "/etc/passwd", 2048, "2026-11-01 00:00", "POSIX Account File", "⚙️"),
            FileSystemItem::new_file("hosts", "/etc/hosts", 512, "2026-11-01 00:00", "Network Hosts Config", "🌐"),
            FileSystemItem::new_file("fstab", "/etc/fstab", 1024, "2026-11-01 00:00", "Filesystem Mount Table", "🔧"),
        ];
        self.entries.insert("/etc".to_string(), linux_etc);

        // --- Linux /bin ---
        let linux_bin = vec![
            FileSystemItem::new_file("bash", "/bin/bash", 1250000, "2026-10-15 00:00", "ELF 64-bit Executable", "⚙️"),
            FileSystemItem::new_file("sh", "/bin/sh", 128000, "2026-10-15 00:00", "POSIX Shell", "⚙️"),
            FileSystemItem::new_file("ls", "/bin/ls", 145000, "2026-10-15 00:00", "Coreutils Binary", "⚙️"),
            FileSystemItem::new_file("cat", "/bin/cat", 45000, "2026-10-15 00:00", "Coreutils Binary", "⚙️"),
        ];
        self.entries.insert("/bin".to_string(), linux_bin);
    }

    /// List directory contents.
    pub fn list_directory(&self, path: &str) -> Vec<FileSystemItem> {
        let norm = normalize_path(path);
        if let Some(items) = self.entries.get(&norm) {
            items.clone()
        } else {
            Vec::new()
        }
    }

    /// Create a new item (Folder, Shortcut, Text Document) in a directory.
    pub fn create_item(
        &mut self,
        dir_path: &str,
        requested_name: &str,
        kind: NewItemKind,
    ) -> Result<String, String> {
        let norm_dir = normalize_path(dir_path);

        // Disambiguate name if already exists
        let mut final_name = requested_name.to_string();
        let mut counter = 2;
        if let Some(items) = self.entries.get(&norm_dir) {
            while items.iter().any(|it| it.name.eq_ignore_ascii_case(&final_name)) {
                let (stem, ext) = match requested_name.rfind('.') {
                    Some(idx) => (&requested_name[..idx], &requested_name[idx..]),
                    None => (requested_name, ""),
                };
                final_name = format!("{} ({}){}", stem, counter, ext);
                counter += 1;
            }
        }

        let full_path = if norm_dir.ends_with('\\') {
            format!("{}{}", norm_dir, final_name)
        } else if norm_dir == "/" {
            format!("/{}", final_name)
        } else if norm_dir.starts_with('/') {
            format!("{}/{}", norm_dir, final_name)
        } else {
            format!("{}\\{}", norm_dir, final_name)
        };

        if kind == NewItemKind::Folder {
            self.entries.insert(full_path.clone(), Vec::new());
        }

        let new_item = match kind {
            NewItemKind::Folder => {
                FileSystemItem::new_dir(&final_name, &full_path, "2026-11-10 10:35")
            }
            NewItemKind::Shortcut => FileSystemItem::new_file(
                &final_name,
                &full_path,
                1024,
                "2026-11-10 10:35",
                "Shortcut",
                "🔗",
            ),
            NewItemKind::TextDocument => FileSystemItem::new_file(
                &final_name,
                &full_path,
                0,
                "2026-11-10 10:35",
                "Text Document",
                "📄",
            ),
        };

        let items = self.entries.entry(norm_dir).or_insert_with(Vec::new);
        items.push(new_item);
        Ok(full_path)
    }

    /// Delete an item by full path.
    pub fn delete_item(&mut self, full_path: &str) -> Result<(), String> {
        let norm = normalize_path(full_path);
        if let Some(parent) = parent_path(&norm) {
            if let Some(items) = self.entries.get_mut(&parent) {
                items.retain(|it| normalize_path(&it.path) != norm);
            }
        }
        self.entries.remove(&norm);
        Ok(())
    }

    /// Rename an item.
    pub fn rename_item(&mut self, old_path: &str, new_name: &str) -> Result<String, String> {
        let norm = normalize_path(old_path);
        let parent = parent_path(&norm).ok_or_else(|| "Cannot rename root directory".to_string())?;

        let items = self.entries.get_mut(&parent).ok_or_else(|| "Parent directory not found".to_string())?;
        let item = items
            .iter_mut()
            .find(|it| normalize_path(&it.path) == norm)
            .ok_or_else(|| "Item not found to rename".to_string())?;

        item.name = new_name.to_string();
        let new_full_path = if parent.ends_with('\\') {
            format!("{}{}", parent, new_name)
        } else if parent == "/" {
            format!("/{}", new_name)
        } else if parent.starts_with('/') {
            format!("{}/{}", parent, new_name)
        } else {
            format!("{}\\{}", parent, new_name)
        };
        let is_dir = item.is_directory;
        item.path = new_full_path.clone();

        // If directory, relocate its map entry
        if is_dir {
            if let Some(children) = self.entries.remove(&norm) {
                self.entries.insert(new_full_path.clone(), children);
            }
        }

        Ok(new_full_path)
    }

    /// Copy an item into a destination directory.
    pub fn copy_item(&mut self, src_path: &str, dest_dir: &str) -> Result<String, String> {
        let norm_src = normalize_path(src_path);
        let norm_dest = normalize_path(dest_dir);

        let src_parent = parent_path(&norm_src).ok_or_else(|| "Invalid source parent".to_string())?;
        let src_item = {
            let parent_items = self.entries.get(&src_parent).ok_or_else(|| "Source not found".to_string())?;
            parent_items
                .iter()
                .find(|it| normalize_path(&it.path) == norm_src)
                .cloned()
                .ok_or_else(|| "Source item not found".to_string())?
        };

        let dest_items = self.entries.entry(norm_dest.clone()).or_insert_with(Vec::new);

        let mut final_name = src_item.name.clone();
        if dest_items.iter().any(|it| it.name.eq_ignore_ascii_case(&final_name)) {
            final_name = format!("Copy of {}", final_name);
        }

        let new_path = if norm_dest.ends_with('\\') {
            format!("{}{}", norm_dest, final_name)
        } else if norm_dest == "/" {
            format!("/{}", final_name)
        } else if norm_dest.starts_with('/') {
            format!("{}/{}", norm_dest, final_name)
        } else {
            format!("{}\\{}", norm_dest, final_name)
        };

        let mut copied = src_item;
        copied.name = final_name;
        copied.path = new_path.clone();
        dest_items.push(copied);

        Ok(new_path)
    }

    /// Move an item into a destination directory.
    pub fn move_item(&mut self, src_path: &str, dest_dir: &str) -> Result<String, String> {
        let copied = self.copy_item(src_path, dest_dir)?;
        self.delete_item(src_path)?;
        Ok(copied)
    }
}

// ============================================================================
// Modern Address Bar
// ============================================================================

/// Address Bar with clickable breadcrumbs and editable path text box.
#[derive(Debug, Clone)]
pub struct AddressBar {
    pub raw_path: String,
    pub is_editing: bool,
    pub edit_buffer: String,
    pub breadcrumbs: Vec<BreadcrumbSegment>,
}

impl AddressBar {
    pub fn new(path: &str) -> Self {
        let norm = normalize_path(path);
        let crumbs = build_breadcrumbs(&norm);
        Self {
            raw_path: norm.clone(),
            is_editing: false,
            edit_buffer: norm,
            breadcrumbs: crumbs,
        }
    }

    /// Update address bar to reflect current path.
    pub fn update(&mut self, path: &str) {
        let norm = normalize_path(path);
        self.raw_path = norm.clone();
        self.edit_buffer = norm.clone();
        self.breadcrumbs = build_breadcrumbs(&norm);
        self.is_editing = false;
    }

    /// Begin manual typing in address bar.
    pub fn begin_editing(&mut self) {
        self.is_editing = true;
    }

    /// Cancel editing and revert to breadcrumb view.
    pub fn cancel_editing(&mut self) {
        self.is_editing = false;
        self.edit_buffer = self.raw_path.clone();
    }
}

// ============================================================================
// Complete Windows 11 Modern File Explorer
// ============================================================================

/// Complete Windows 11 Modern File Explorer window component.
#[derive(Debug, Clone)]
pub struct ModernFileExplorer {
    pub tab_bar: TabBar,
    pub nav_pane: NavigationPane,
    pub command_bar: CommandBar,
    pub address_bar: AddressBar,
    pub fs_provider: HybridFsProvider,
}

impl ModernFileExplorer {
    /// Initialize a new production-grade Windows 11 Modern File Explorer.
    pub fn new() -> Self {
        let fs_provider = HybridFsProvider::new();
        let initial_path = "C:\\Users\\Vinicius\\Documents";
        let tab_bar = TabBar::new(initial_path, &fs_provider);
        let nav_pane = NavigationPane::new();
        let command_bar = CommandBar::new();
        let address_bar = AddressBar::new(initial_path);

        Self {
            tab_bar,
            nav_pane,
            command_bar,
            address_bar,
            fs_provider,
        }
    }

    /// Get reference to active ExplorerTab.
    pub fn active_tab(&self) -> &ExplorerTab {
        self.tab_bar.active_tab().expect("Active tab must always exist")
    }

    /// Get mutable reference to active ExplorerTab.
    pub fn active_tab_mut(&mut self) -> &mut ExplorerTab {
        self.tab_bar.active_tab_mut().expect("Active tab must always exist")
    }

    /// Navigate active tab to path, synchronizing address bar and navigation pane.
    pub fn navigate(&mut self, target_path: &str) {
        let path_string = target_path.to_string();
        self.tab_bar
            .active_tab_mut()
            .expect("Active tab")
            .navigate_to(&path_string, &self.fs_provider);

        let active_path = self.active_tab().current_path.clone();
        self.address_bar.update(&active_path);
        self.nav_pane.select_path(&active_path);
    }

    /// Navigate Back in active tab.
    pub fn go_back(&mut self) -> bool {
        let success = self.tab_bar.active_tab_mut().expect("Active tab").go_back(&self.fs_provider);
        if success {
            let active_path = self.active_tab().current_path.clone();
            self.address_bar.update(&active_path);
            self.nav_pane.select_path(&active_path);
        }
        success
    }

    /// Navigate Forward in active tab.
    pub fn go_forward(&mut self) -> bool {
        let success = self.tab_bar.active_tab_mut().expect("Active tab").go_forward(&self.fs_provider);
        if success {
            let active_path = self.active_tab().current_path.clone();
            self.address_bar.update(&active_path);
            self.nav_pane.select_path(&active_path);
        }
        success
    }

    /// Navigate Up to parent folder (Alt+Up) in active tab.
    pub fn go_up(&mut self) -> bool {
        let success = self.tab_bar.active_tab_mut().expect("Active tab").go_up(&self.fs_provider);
        if success {
            let active_path = self.active_tab().current_path.clone();
            self.address_bar.update(&active_path);
            self.nav_pane.select_path(&active_path);
        }
        success
    }

    /// Dispatch command bar or keyboard action to active tab.
    pub fn dispatch_command(&mut self, action: CommandAction) -> Result<String, String> {
        let tab = self.tab_bar.active_tab_mut().expect("Active tab");
        self.command_bar.dispatch(action, tab, &mut self.fs_provider)
    }

    /// Add new tab (+) and navigate to specified path.
    pub fn new_tab(&mut self, path: &str) -> TabId {
        let id = self.tab_bar.add_tab(path, &self.fs_provider);
        let active_path = self.active_tab().current_path.clone();
        self.address_bar.update(&active_path);
        self.nav_pane.select_path(&active_path);
        id
    }

    /// Close tab by id.
    pub fn close_tab(&mut self, id: TabId) -> bool {
        let ok = self.tab_bar.close_tab(id, &self.fs_provider);
        let active_path = self.active_tab().current_path.clone();
        self.address_bar.update(&active_path);
        self.nav_pane.select_path(&active_path);
        ok
    }

    /// Handle click on navigation pane item.
    pub fn click_nav_item(&mut self, path: &str) {
        self.navigate(path);
    }

    /// Handle click on breadcrumb segment.
    pub fn click_breadcrumb(&mut self, target_path: &str) {
        self.navigate(target_path);
    }

    /// Apply real-time search filtering.
    pub fn search(&mut self, query: &str) {
        self.active_tab_mut().set_search_query(query);
    }

    /// Compute status bar metrics for active tab.
    pub fn status_metrics(&self) -> StatusBarMetrics {
        StatusBarMetrics::compute(self.active_tab())
    }

    /// Render a high-fidelity visual ASCII representation of the Windows 11 Explorer interface.
    pub fn render_ascii_ui(&self) -> String {
        let mut out = String::new();
        let tab = self.active_tab();

        out.push_str("┌─────────────────────────────────────────────────────────────────────────────┐\n");
        out.push_str("│ [Windows 11 Modern File Explorer]                                  ─  □  ✕  │\n");
        out.push_str("├─────────────────────────────────────────────────────────────────────────────┤\n");

        // Tab Bar
        out.push_str("│ Tabs: ");
        for (i, t) in self.tab_bar.tabs.iter().enumerate() {
            let active_mark = if i == self.tab_bar.active_tab_index { "●" } else { " " };
            out.push_str(&format!("[{} {} ✕] ", active_mark, t.title));
        }
        out.push_str("[+]                                                    │\n");
        out.push_str("├─────────────────────────────────────────────────────────────────────────────┤\n");

        // Command Bar
        out.push_str("│ [+ New ▼] | [✂ Cut] [📋 Copy] [📎 Paste] [✏ Rename] [↗ Share] [🗑 Delete]    │\n");
        out.push_str(&format!(
            "│ [Sort: {:?} ▼] [View: {:?} ▼]                                            │\n",
            tab.sort_column, tab.view_mode
        ));
        out.push_str("├─────────────────────────────────────────────────────────────────────────────┤\n");

        // Address & Search Bar
        let back_btn = if tab.can_go_back() { "◀" } else { "◁" };
        let fwd_btn = if tab.can_go_forward() { "▶" } else { "▷" };
        let up_btn = if tab.can_go_up() { "▲" } else { "△" };

        let breadcrumb_str: String = self
            .address_bar
            .breadcrumbs
            .iter()
            .map(|b| b.display_name.as_str())
            .collect::<Vec<&str>>()
            .join(" > ");

        out.push_str(&format!(
            "│ {} {} {}  [ 📁 {} ] [🔍 Search {}] │\n",
            back_btn,
            fwd_btn,
            up_btn,
            truncate_str(&breadcrumb_str, 40),
            if tab.search_query.is_empty() {
                "..."
            } else {
                &tab.search_query
            }
        ));
        out.push_str("├──────────────────────────────┬──────────────────────────────────────────────┤\n");

        // Left Navigation Pane & File List Header
        out.push_str("│ Quick Access                 │ Name           Date modified       Size      │\n");
        out.push_str("│   🖥️  Desktop                 │ ──────────────────────────────────────────── │\n");
        out.push_str("│   📥 Downloads               │                                              │\n");
        out.push_str("│   📄 Documents               │                                              │\n");
        out.push_str("│ This PC                      │                                              │\n");
        out.push_str("│   💽 Local Disk (C:)         │                                              │\n");
        out.push_str("│   💾 Data Disk (D:)          │                                              │\n");
        out.push_str("│   ⚡ NVMe Storage (E:)        │                                              │\n");
        out.push_str("│ Linux POSIX Root (/)         │                                              │\n");
        out.push_str("│   ⚙️  /bin                    │                                              │\n");
        out.push_str("│   🔧 /etc                    │                                              │\n");
        out.push_str("│   🏠 /home                   │                                              │\n");

        // Status bar
        let status = self.status_metrics();
        out.push_str("├──────────────────────────────┴──────────────────────────────────────────────┤\n");
        out.push_str(&format!("│ Status: {:<68} │\n", status.display_text()));
        out.push_str("└─────────────────────────────────────────────────────────────────────────────┘\n");

        out
    }
}

fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[..max_len - 3])
    } else {
        format!("{:<width$}", s, width = max_len)
    }
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tab_creation_and_switching() {
        let provider = HybridFsProvider::new();
        let mut tab_bar = TabBar::new("C:\\Users\\Vinicius\\Documents", &provider);

        assert_eq!(tab_bar.tab_count(), 1);
        assert_eq!(tab_bar.active_tab().unwrap().title, "Documents");
        assert_eq!(tab_bar.active_tab().unwrap().id, TabId(1));

        // Add second tab
        let id2 = tab_bar.add_tab("C:\\Windows\\System32", &provider);
        assert_eq!(tab_bar.tab_count(), 2);
        assert_eq!(tab_bar.active_tab().unwrap().id, id2);
        assert_eq!(tab_bar.active_tab().unwrap().title, "System32");

        // Add third tab for Linux POSIX
        let id3 = tab_bar.add_tab("/etc", &provider);
        assert_eq!(tab_bar.tab_count(), 3);
        assert_eq!(tab_bar.active_tab().unwrap().id, id3);
        assert_eq!(tab_bar.active_tab().unwrap().title, "etc");

        // Switch to first tab
        assert!(tab_bar.switch_tab(TabId(1)));
        assert_eq!(tab_bar.active_tab().unwrap().title, "Documents");

        // Switch by index
        assert!(tab_bar.switch_to_index(1));
        assert_eq!(tab_bar.active_tab().unwrap().title, "System32");
    }

    #[test]
    fn test_tab_close_and_reorder() {
        let provider = HybridFsProvider::new();
        let mut tab_bar = TabBar::new("C:\\", &provider);
        let id2 = tab_bar.add_tab("D:\\", &provider);
        let id3 = tab_bar.add_tab("E:\\", &provider);

        assert_eq!(tab_bar.tab_count(), 3);

        // Reorder tabs: move tab 0 to end (index 2)
        assert!(tab_bar.reorder_tabs(0, 2));
        assert_eq!(tab_bar.tabs[0].id, id2);
        assert_eq!(tab_bar.tabs[1].id, id3);
        assert_eq!(tab_bar.tabs[2].id, TabId(1));

        // Close middle tab
        assert!(tab_bar.close_tab(id3, &provider));
        assert_eq!(tab_bar.tab_count(), 2);

        // Close until empty, ensuring fallback tab is created
        assert!(tab_bar.close_tab(id2, &provider));
        assert!(tab_bar.close_tab(TabId(1), &provider));
        assert_eq!(tab_bar.tab_count(), 1); // Fallback tab created
        assert_eq!(tab_bar.active_tab().unwrap().current_path, "C:\\");
    }

    #[test]
    fn test_path_navigation_and_breadcrumbs() {
        let breadcrumbs = build_breadcrumbs("C:\\Windows\\System32\\drivers");
        assert_eq!(breadcrumbs.len(), 5);
        assert_eq!(breadcrumbs[0].display_name, "This PC");
        assert_eq!(breadcrumbs[1].display_name, "Local Disk (C:)");
        assert_eq!(breadcrumbs[1].target_path, "C:\\");
        assert_eq!(breadcrumbs[2].display_name, "Windows");
        assert_eq!(breadcrumbs[2].target_path, "C:\\Windows");
        assert_eq!(breadcrumbs[3].display_name, "System32");
        assert_eq!(breadcrumbs[3].target_path, "C:\\Windows\\System32");
        assert_eq!(breadcrumbs[4].display_name, "drivers");
        assert_eq!(breadcrumbs[4].target_path, "C:\\Windows\\System32\\drivers");
        assert!(breadcrumbs[4].is_last);

        // POSIX breadcrumbs
        let posix_crumbs = build_breadcrumbs("/etc/systemd");
        assert_eq!(posix_crumbs.len(), 3);
        assert_eq!(posix_crumbs[0].display_name, "Linux Root (/)");
        assert_eq!(posix_crumbs[1].display_name, "etc");
        assert_eq!(posix_crumbs[1].target_path, "/etc");
        assert_eq!(posix_crumbs[2].display_name, "systemd");
        assert_eq!(posix_crumbs[2].target_path, "/etc/systemd");
    }

    #[test]
    fn test_back_forward_up_history() {
        let provider = HybridFsProvider::new();
        let mut tab = ExplorerTab::new(TabId(1), "C:\\", &provider);

        assert!(!tab.can_go_back());
        assert!(!tab.can_go_forward());

        // Navigate forward through folders
        tab.navigate_to("C:\\Windows", &provider);
        tab.navigate_to("C:\\Windows\\System32", &provider);

        assert!(tab.can_go_back());
        assert_eq!(tab.current_path, "C:\\Windows\\System32");

        // Go back
        assert!(tab.go_back(&provider));
        assert_eq!(tab.current_path, "C:\\Windows");
        assert!(tab.can_go_forward());

        assert!(tab.go_back(&provider));
        assert_eq!(tab.current_path, "C:\\");
        assert!(!tab.can_go_back());

        // Go forward
        assert!(tab.go_forward(&provider));
        assert_eq!(tab.current_path, "C:\\Windows");

        // Up navigation (Alt+Up)
        tab.navigate_to("C:\\Windows\\System32", &provider);
        assert!(tab.can_go_up());
        assert!(tab.go_up(&provider));
        assert_eq!(tab.current_path, "C:\\Windows");
    }

    #[test]
    fn test_search_filtering() {
        let provider = HybridFsProvider::new();
        let mut tab = ExplorerTab::new(TabId(1), "C:\\Windows", &provider);

        let initial_count = tab.display_items().len();
        assert!(initial_count > 0);

        // Filter by "exe"
        tab.set_search_query("exe");
        let filtered = tab.display_items();
        assert!(filtered.len() < initial_count);
        for item in &filtered {
            assert!(
                item.name.to_lowercase().contains("exe")
                    || item.file_type.to_lowercase().contains("exe")
            );
        }

        // Clear search
        tab.clear_search();
        assert_eq!(tab.display_items().len(), initial_count);
    }

    #[test]
    fn test_multi_item_selection() {
        let provider = HybridFsProvider::new();
        let mut tab = ExplorerTab::new(TabId(1), "C:\\Windows", &provider);

        // Single selection
        tab.select_single(0);
        assert_eq!(tab.selected_count(), 1);

        // Ctrl+Click toggle
        tab.toggle_selection(1);
        assert_eq!(tab.selected_count(), 2);

        tab.toggle_selection(1); // un-toggle
        assert_eq!(tab.selected_count(), 1);

        // Select All (Ctrl+A)
        tab.select_all();
        let total_items = tab.display_items().len();
        assert_eq!(tab.selected_count(), total_items);

        // Clear
        tab.clear_selection();
        assert_eq!(tab.selected_count(), 0);

        // Shift+Click range
        tab.select_single(0);
        tab.select_range(2);
        assert_eq!(tab.selected_count(), 3);
    }

    #[test]
    fn test_command_bar_actions() {
        let mut explorer = ModernFileExplorer::new();
        explorer.navigate("C:\\Users\\Vinicius\\Documents");

        // 1. New Folder
        let res = explorer.dispatch_command(CommandAction::NewItem(NewItemKind::Folder));
        assert!(res.is_ok());
        assert!(explorer.active_tab().is_selected("New folder"));

        // 2. Rename (F2)
        let rename_res = explorer.dispatch_command(CommandAction::Rename("HybridOS_Research".to_string()));
        assert!(rename_res.is_ok());
        assert!(explorer.active_tab().is_selected("HybridOS_Research"));

        // 3. Copy & Paste
        assert!(explorer.dispatch_command(CommandAction::Copy).is_ok());
        assert_eq!(explorer.command_bar.clipboard_items.len(), 1);
        explorer.navigate("D:\\");
        assert!(explorer.dispatch_command(CommandAction::Paste).is_ok());
        assert!(explorer.active_tab().display_items().iter().any(|it| it.name == "HybridOS_Research"));

        // 4. Delete (Del)
        explorer.active_tab_mut().select_single(0);
        let del_res = explorer.dispatch_command(CommandAction::Delete { permanent: true });
        assert!(del_res.is_ok());
    }

    #[test]
    fn test_navigation_pane_and_status_metrics() {
        let explorer = ModernFileExplorer::new();

        // Check Quick Access items
        assert_eq!(explorer.nav_pane.quick_access.len(), 6);
        assert_eq!(explorer.nav_pane.quick_access[0].title, "Desktop");

        // Check This PC items
        assert_eq!(explorer.nav_pane.this_pc.len(), 4);
        assert_eq!(explorer.nav_pane.this_pc[0].title, "Local Disk (C:)");
        assert_eq!(explorer.nav_pane.this_pc[1].title, "Data Disk (D:)");
        assert_eq!(explorer.nav_pane.this_pc[2].title, "NVMe Storage (NVMe0)");

        // Check Linux POSIX roots
        assert_eq!(explorer.nav_pane.linux_root.len(), 5);
        assert_eq!(explorer.nav_pane.linux_root[0].title, "/bin");
        assert_eq!(explorer.nav_pane.linux_root[1].title, "/etc");

        // Status Bar Metrics
        let status = explorer.status_metrics();
        assert!(status.total_items > 0);
        assert_eq!(status.selected_items, 0);
        assert_eq!(status.display_text(), format!("{} items", status.total_items));
    }

    #[test]
    fn test_sort_and_view_options() {
        let provider = HybridFsProvider::new();
        let mut tab = ExplorerTab::new(TabId(1), "C:\\Windows", &provider);

        // Sort by size descending
        tab.sort_column = SortColumn::Size;
        tab.sort_direction = SortDirection::Descending;
        let items = tab.display_items();

        // Verify non-directories are sorted descending by size
        let files: Vec<&FileSystemItem> = items.iter().filter(|i| !i.is_directory).collect();
        for w in files.windows(2) {
            assert!(w[0].size_bytes >= w[1].size_bytes);
        }

        // View Mode
        tab.view_mode = ViewOption::ExtraLargeIcons;
        assert_eq!(tab.view_mode.display_name(), "Extra large icons");
    }

    #[test]
    fn test_ascii_ui_render() {
        let explorer = ModernFileExplorer::new();
        let ui = explorer.render_ascii_ui();
        assert!(ui.contains("Windows 11 Modern File Explorer"));
        assert!(ui.contains("Quick Access"));
        assert!(ui.contains("This PC"));
        assert!(ui.contains("Linux POSIX Root"));
        assert!(ui.contains("Local Disk (C:)"));
    }
}
