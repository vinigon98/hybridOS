//! hyb-files core library

use libhyb_cfg::ConfigStore;
use libhyb_ui::colors::*;
use libhyb_ui::icons::IconType;
use libhyb_ui::surface::Surface;
use libhyb_ui::widgets::{Button, IconGrid, TableView};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

/// FreeDesktop Trash Manager
#[derive(Debug, Clone)]
pub struct TrashManager {
    pub trash_dir: PathBuf,
}

impl TrashManager {
    pub fn new() -> Self {
        let base = if let Ok(data_home) = env::var("XDG_DATA_HOME") {
            PathBuf::from(data_home).join("Trash")
        } else if let Ok(home) = env::var("HOME") {
            PathBuf::from(home).join(".local/share/Trash")
        } else {
            PathBuf::from(".trash")
        };

        let files_dir = base.join("files");
        let info_dir = base.join("info");
        let _ = fs::create_dir_all(&files_dir);
        let _ = fs::create_dir_all(&info_dir);

        Self { trash_dir: base }
    }

    pub fn move_to_trash(&self, path: &Path) -> Result<(), String> {
        if !path.exists() {
            return Err("Path does not exist".to_string());
        }
        let fname = path.file_name().ok_or("Invalid filename")?.to_string_lossy();
        let target_file = self.trash_dir.join("files").join(&*fname);
        let target_info = self.trash_dir.join("info").join(format!("{}.trashinfo", fname));

        fs::rename(path, &target_file).map_err(|e| e.to_string())?;

        let info_content = format!(
            "[Trash Info]\nPath={}\nDeletionDate=2026-11-19T18:00:00\n",
            path.display()
        );
        fs::write(&target_info, info_content).map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn restore_item(&self, filename: &str, destination: &Path) -> Result<(), String> {
        let trash_file = self.trash_dir.join("files").join(filename);
        let trash_info = self.trash_dir.join("info").join(format!("{}.trashinfo", filename));

        if !trash_file.exists() {
            return Err("Item not found in trash".to_string());
        }

        fs::rename(&trash_file, destination).map_err(|e| e.to_string())?;
        let _ = fs::remove_file(trash_info);
        Ok(())
    }
}

impl Default for TrashManager {
    fn default() -> Self {
        Self::new()
    }
}

/// File System Item
#[derive(Debug, Clone)]
pub struct FileItem {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub size: u64,
    pub modified_str: String,
}

impl FileItem {
    pub fn icon(&self) -> IconType {
        if self.is_dir {
            IconType::Folder
        } else if self.name.ends_with(".rs") || self.name.ends_with(".c") || self.name.ends_with(".py") {
            IconType::Code
        } else if self.name.ends_with(".png") || self.name.ends_with(".jpg") {
            IconType::Image
        } else {
            IconType::File
        }
    }
}

/// Natural sort comparison (e.g. img2 before img10)
pub fn natural_sort_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let mut a_chars = a.chars().peekable();
    let mut b_chars = b.chars().peekable();

    while let (Some(&ca), Some(&cb)) = (a_chars.peek(), b_chars.peek()) {
        if ca.is_ascii_digit() && cb.is_ascii_digit() {
            let mut num_a = 0u64;
            while let Some(&d) = a_chars.peek() {
                if d.is_ascii_digit() {
                    num_a = num_a * 10 + (d as u64 - '0' as u64);
                    a_chars.next();
                } else {
                    break;
                }
            }

            let mut num_b = 0u64;
            while let Some(&d) = b_chars.peek() {
                if d.is_ascii_digit() {
                    num_b = num_b * 10 + (d as u64 - '0' as u64);
                    b_chars.next();
                } else {
                    break;
                }
            }

            match num_a.cmp(&num_b) {
                std::cmp::Ordering::Equal => continue,
                other => return other,
            }
        } else {
            match ca.to_lowercase().cmp(cb.to_lowercase()) {
                std::cmp::Ordering::Equal => {
                    a_chars.next();
                    b_chars.next();
                }
                other => return other,
            }
        }
    }

    a.len().cmp(&b.len())
}

/// File Explorer State
pub struct FileManagerState {
    pub current_dir: PathBuf,
    pub items: Vec<FileItem>,
    pub dual_pane: bool,
    pub show_dotfiles: bool,
    pub table_view: TableView,
    pub grid_view: IconGrid,
    pub use_grid: bool,
    pub trash: TrashManager,
    pub new_folder_btn: Button,
    pub delete_btn: Button,
    pub copy_btn: Button,
    pub paste_btn: Button,
    pub clipboard_cut: bool,
    pub clipboard_path: Option<PathBuf>,
}

impl FileManagerState {
    pub fn new(initial_path: PathBuf, config: &ConfigStore) -> Self {
        let show_dotfiles = config.get_bool("files.show_dotfiles").unwrap_or(false);
        let trash = TrashManager::new();

        let table = TableView::new(
            220,
            100,
            800,
            500,
            vec![("Name", 350), ("Size", 120), ("Modified", 180)],
        );
        let grid = IconGrid::new(220, 100, 800, 500);

        let new_folder_btn = Button::new(220, 56, 110, 32, "+ Folder");
        let copy_btn = Button::new(340, 56, 80, 32, "Copy");
        let paste_btn = Button::new(430, 56, 80, 32, "Paste");
        let delete_btn = Button::new(520, 56, 90, 32, "Delete").with_danger();

        let mut mgr = Self {
            current_dir: initial_path,
            items: Vec::new(),
            dual_pane: false,
            show_dotfiles,
            table_view: table,
            grid_view: grid,
            use_grid: false,
            trash,
            new_folder_btn,
            delete_btn,
            copy_btn,
            paste_btn,
            clipboard_cut: false,
            clipboard_path: None,
        };

        mgr.refresh_directory();
        mgr
    }

    pub fn refresh_directory(&mut self) {
        self.items.clear();
        self.table_view.rows.clear();
        self.grid_view.clear();

        if let Ok(entries) = fs::read_dir(&self.current_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                let fname = entry.file_name().to_string_lossy().to_string();

                if !self.show_dotfiles && fname.starts_with('.') {
                    continue;
                }

                let meta = entry.metadata().ok();
                let is_dir = meta.as_ref().map(|m| m.is_dir()).unwrap_or(false);
                let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);

                let item = FileItem {
                    name: fname,
                    path,
                    is_dir,
                    size,
                    modified_str: "2026-11-19 18:30".to_string(),
                };
                self.items.push(item);
            }
        }

        self.items.sort_by(|a, b| {
            if a.is_dir != b.is_dir {
                b.is_dir.cmp(&a.is_dir)
            } else {
                natural_sort_cmp(&a.name, &b.name)
            }
        });

        for item in &self.items {
            let size_display = if item.is_dir {
                "<DIR>".to_string()
            } else {
                format!("{} B", item.size)
            };

            self.table_view.add_row(vec![
                &item.name,
                &size_display,
                &item.modified_str,
            ]);

            self.grid_view.add_item(&item.name, item.icon(), item.is_dir);
        }
    }

    pub fn create_folder(&mut self, name: &str) -> Result<(), String> {
        let p = self.current_dir.join(name);
        fs::create_dir_all(&p).map_err(|e| e.to_string())?;
        self.refresh_directory();
        Ok(())
    }

    pub fn copy_selection(&mut self, item_index: usize) {
        if let Some(item) = self.items.get(item_index) {
            self.clipboard_path = Some(item.path.clone());
            self.clipboard_cut = false;
        }
    }

    pub fn cut_selection(&mut self, item_index: usize) {
        if let Some(item) = self.items.get(item_index) {
            self.clipboard_path = Some(item.path.clone());
            self.clipboard_cut = true;
        }
    }

    pub fn paste_clipboard(&mut self) -> Result<(), String> {
        if let Some(src) = &self.clipboard_path {
            if !src.exists() {
                return Err("Source file does not exist".to_string());
            }
            let fname = src.file_name().ok_or("Invalid file name")?;
            let dst = self.current_dir.join(fname);

            if self.clipboard_cut {
                fs::rename(src, &dst).map_err(|e| e.to_string())?;
                self.clipboard_path = None;
            } else {
                fs::copy(src, &dst).map_err(|e| e.to_string())?;
            }
            self.refresh_directory();
            Ok(())
        } else {
            Err("Clipboard is empty".to_string())
        }
    }

    pub fn delete_selection(&mut self, item_index: usize, permanent: bool) -> Result<(), String> {
        if let Some(item) = self.items.get(item_index) {
            if permanent {
                if item.is_dir {
                    fs::remove_dir_all(&item.path).map_err(|e| e.to_string())?;
                } else {
                    fs::remove_file(&item.path).map_err(|e| e.to_string())?;
                }
            } else {
                self.trash.move_to_trash(&item.path)?;
            }
            self.refresh_directory();
            Ok(())
        } else {
            Err("Invalid item index".to_string())
        }
    }

    pub fn rename_item(&mut self, item_index: usize, new_name: &str) -> Result<(), String> {
        if let Some(item) = self.items.get(item_index) {
            let dst = item.path.parent().unwrap_or_else(|| Path::new(".")).join(new_name);
            fs::rename(&item.path, dst).map_err(|e| e.to_string())?;
            self.refresh_directory();
            Ok(())
        } else {
            Err("Invalid item index".to_string())
        }
    }

    pub fn set_view_mode(&mut self, grid: bool) {
        self.use_grid = grid;
    }

    pub fn render(&self, surface: &mut Surface) {
        surface.fill_rect(0, 0, surface.width as u32, surface.height as u32, COLOR_CARD);

        let sidebar_w = 200;
        surface.fill_rect(0, 0, sidebar_w, surface.height as u32, COLOR_SURFACE);
        surface.fill_rect(sidebar_w as i32, 0, 1, surface.height as u32, COLOR_BORDER);

        surface.draw_string(16, 20, "FAVORITES", COLOR_TEXT_MUTED);
        surface.draw_string(24, 46, "/ Root Filesystem", COLOR_TEXT);
        surface.draw_string(24, 72, "C:\\ Windows NT", COLOR_CYAN);
        surface.draw_string(24, 98, "~ Home Directory", COLOR_TEXT);
        surface.draw_string(24, 124, "Trash (Lixeira)", COLOR_TEXT_MUTED);

        let tb_h = 44;
        surface.fill_rect(sidebar_w as i32 + 1, 0, surface.width as u32 - sidebar_w, tb_h, COLOR_TITLEBAR);
        let path_str = format!("Path: {}", self.current_dir.display());
        surface.draw_string((sidebar_w + 16) as usize, 14, &path_str, COLOR_TEXT);

        self.new_folder_btn.render(surface);
        self.copy_btn.render(surface);
        self.paste_btn.render(surface);
        self.delete_btn.render(surface);

        if self.use_grid {
            self.grid_view.render(surface);
        } else {
            self.table_view.render(surface);
        }

        let sb_y = (surface.height - 24) as i32;
        surface.fill_rect(sidebar_w as i32 + 1, sb_y, surface.width as u32 - sidebar_w, 24, COLOR_SURFACE);
        let status = format!("{} items | VFS: Dual-Personality (POSIX/NT)", self.items.len());
        surface.draw_string((sidebar_w + 16) as usize, (sb_y + 6) as usize, &status, COLOR_TEXT_MUTED);
    }
}
