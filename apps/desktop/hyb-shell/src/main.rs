//! hyb-shell: HybridOS Modern Fluent Shell, Taskbar & Application Launcher

use libhyb_cfg::ConfigStore;
use libhyb_ui::colors::*;
use libhyb_ui::icons::{draw_icon, IconType};
use libhyb_ui::surface::Surface;
use libhyb_ui::widgets::{Button, TextInput};
use std::env;
use std::path::Path;

/// Application Launcher State
#[derive(Debug, Clone)]
pub struct LauncherState {
    pub is_open: bool,
    pub search_input: TextInput,
    pub results: Vec<(String, String)>, // (Title, Description/Category)
    pub calc_result: Option<String>,
}

impl LauncherState {
    pub fn new(screen_w: u32, screen_h: u32) -> Self {
        let lw = 500;
        let lh = 340;
        let lx = ((screen_w.saturating_sub(lw)) / 2) as i32;
        let ly = ((screen_h.saturating_sub(lh)) / 2) as i32;

        let search_input = TextInput::new(lx + 20, ly + 20, lw - 40, 36, "Search apps, files, or calculate...");

        Self {
            is_open: false,
            search_input,
            results: Vec::new(),
            calc_result: None,
        }
    }

    pub fn handle_query(&mut self, query: &str) {
        self.search_input.text = query.to_string();
        self.results.clear();
        self.calc_result = None;

        let trimmed = query.trim();
        if trimmed.is_empty() {
            // Default pinned recommendations
            self.results.push(("hyb-files".to_string(), "Async File Manager & Dual-Pane VFS".to_string()));
            self.results.push(("hyb-edit".to_string(), "Native High-Speed Code & Text Editor".to_string()));
            self.results.push(("hyb-calc".to_string(), "Programmer & Scientific Bitwise Calculator".to_string()));
            self.results.push(("hyb-term".to_string(), "HybridOS Terminal (Bash + PowerShell)".to_string()));
            return;
        }

        // Inline Calculator evaluation: e.g. "1920*1080"
        if let Some(res) = evaluate_math_expr(trimmed) {
            self.calc_result = Some(format!("= {}", res));
            self.results.push((format!("Result: {}", res), "Inline Math Calculator".to_string()));
        }

        // App matching
        let apps = [
            ("hyb-files", "File Explorer with dual-pane and tabs"),
            ("hyb-edit", "High-performance text editor"),
            ("hyb-calc", "Programmer calculator"),
            ("hyb-capture", "Screen capture and OCR engine"),
            ("hyb-clip", "Encrypted clipboard manager"),
            ("hyb-chars", "Unicode character inspector"),
            ("hyb-fonts", "Font manager & HarfBuzz preview"),
            ("hyb-paint", "Image annotator & paint editor"),
            ("hyb-rdp", "Remote desktop client (RDP/VNC/SPICE)"),
            ("hyb-vault", "Credential vault & Secret Service"),
            ("hyb-cert-user", "User certificates & PKCS#11"),
            ("hyb-cert-sys", "System root CA manager"),
            ("hyb-focus", "Pomodoro timer & distraction blocker"),
            ("hyb-pdf", "PDF/EPUB reader & virtual printer"),
            ("hyb-zoom", "Compositor magnifier & color picker"),
        ];

        let q_lower = trimmed.to_lowercase();
        for (name, desc) in apps {
            if name.contains(&q_lower) || desc.to_lowercase().contains(&q_lower) {
                self.results.push((name.to_string(), desc.to_string()));
            }
        }
    }

    pub fn render(&self, surface: &mut Surface) {
        if !self.is_open {
            return;
        }

        let lx = self.search_input.rect.x - 20;
        let ly = self.search_input.rect.y - 20;
        let lw = self.search_input.rect.width + 40;
        let lh = 360;

        // Shadow and background
        surface.draw_drop_shadow(lx, ly, lw, lh, 8);
        surface.fill_rect(lx, ly, lw, lh, COLOR_SURFACE);
        surface.draw_rect(lx, ly, lw, lh, COLOR_BORDER);

        self.search_input.render(surface);

        // Results listing
        let mut row_y = ly + 72;
        for (title, desc) in self.results.iter().take(5) {
            surface.fill_rect(lx + 20, row_y, lw - 40, 48, COLOR_CARD);
            surface.draw_rect(lx + 20, row_y, lw - 40, 48, COLOR_BORDER);

            draw_icon(surface, (lx + 28) as usize, (row_y + 16) as usize, IconType::Terminal, COLOR_ACCENT);
            surface.draw_string((lx + 54) as usize, (row_y + 10) as usize, title, COLOR_TEXT);
            surface.draw_string((lx + 54) as usize, (row_y + 26) as usize, desc, COLOR_TEXT_MUTED);

            row_y += 54;
        }
    }
}

/// Simple inline math evaluator for launcher
fn evaluate_math_expr(expr: &str) -> Option<i64> {
    if let Some((l, r)) = expr.split_once('*') {
        let a = l.trim().parse::<i64>().ok()?;
        let b = r.trim().parse::<i64>().ok()?;
        return Some(a * b);
    }
    if let Some((l, r)) = expr.split_once('+') {
        let a = l.trim().parse::<i64>().ok()?;
        let b = r.trim().parse::<i64>().ok()?;
        return Some(a + b);
    }
    if let Some((l, r)) = expr.split_once('-') {
        let a = l.trim().parse::<i64>().ok()?;
        let b = r.trim().parse::<i64>().ok()?;
        return Some(a - b);
    }
    if let Some((l, r)) = expr.split_once('/') {
        let a = l.trim().parse::<i64>().ok()?;
        let b = r.trim().parse::<i64>().ok()?;
        if b != 0 {
            return Some(a / b);
        }
    }
    None
}

/// Shell State Container
pub struct ShellState {
    pub screen_w: u32,
    pub screen_h: u32,
    pub taskbar_height: u32,
    pub position: String,
    pub telemetry_enabled: bool,
    pub launcher: LauncherState,
    pub start_button: Button,
}

impl ShellState {
    pub fn new(screen_w: u32, screen_h: u32, config: &ConfigStore) -> Self {
        let position = config.get_str_or("shell.panel_position", "bottom").to_string();
        let telemetry_enabled = config.get_bool("shell.tray_telemetry").unwrap_or(false);
        let taskbar_height = 48;

        let tb_y = (screen_h - taskbar_height) as i32;
        let start_btn = Button::new(12, tb_y + 8, 80, 32, "Start").with_accent();

        let mut launcher = LauncherState::new(screen_w, screen_h);
        launcher.handle_query("");

        Self {
            screen_w,
            screen_h,
            taskbar_height,
            position,
            telemetry_enabled,
            launcher,
            start_button: start_btn,
        }
    }

    pub fn render(&self, surface: &mut Surface) {
        let tb_y = (self.screen_h - self.taskbar_height) as i32;

        // Background
        surface.fill_rect(0, tb_y, self.screen_w, self.taskbar_height, COLOR_SURFACE);
        surface.fill_rect(0, tb_y, self.screen_w, 1, COLOR_BORDER);

        // Start Button
        self.start_button.render(surface);

        // Pinned App Icons
        let center_x = (self.screen_w / 2).saturating_sub(120) as i32;
        draw_icon(surface, (center_x) as usize, (tb_y + 16) as usize, IconType::Folder, COLOR_ACCENT);
        surface.draw_string((center_x + 22) as usize, (tb_y + 16) as usize, "Files", COLOR_TEXT);

        draw_icon(surface, (center_x + 90) as usize, (tb_y + 16) as usize, IconType::Terminal, COLOR_SUCCESS);
        surface.draw_string((center_x + 112) as usize, (tb_y + 16) as usize, "Terminal", COLOR_TEXT);

        draw_icon(surface, (center_x + 200) as usize, (tb_y + 16) as usize, IconType::Gear, COLOR_TEXT_MUTED);
        surface.draw_string((center_x + 222) as usize, (tb_y + 16) as usize, "Settings", COLOR_TEXT);

        // Telemetry & Clock
        let right_x = (self.screen_w as usize).saturating_sub(220);
        let telemetry_str = if self.telemetry_enabled {
            "CPU: 3% | RAM: 1.8GB | 18:30"
        } else {
            "HybridOS | 18:30"
        };
        surface.draw_string(right_x, (tb_y + 16) as usize, telemetry_str, COLOR_TEXT);

        // Launcher Flyout if open
        self.launcher.render(surface);
    }
}

fn print_usage() {
    println!("hyb-shell: HybridOS Modern Fluent Desktop Shell");
    println!("Usage: hyb-shell [OPTIONS]");
    println!("Options:");
    println!("  --screenshot <PATH>  Render shell UI to a PNG screenshot and exit");
    println!("  --version            Display version information");
    println!("  --help               Display this help text");
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let config = ConfigStore::new();

    if args.contains(&"--version".to_string()) {
        println!("hyb-shell 0.1.0 (HybridOS Modern Shell)");
        return;
    }

    if args.contains(&"--help".to_string()) {
        print_usage();
        return;
    }

    println!("[hyb-shell] Initializing HybridOS Shell Environment...");
    let mut shell = ShellState::new(1920, 1080, &config);

    // Open launcher for demonstration
    shell.launcher.is_open = true;
    shell.launcher.handle_query("1920*1080");

    let mut surface = Surface::new(1920, 1080, COLOR_BG);
    shell.render(&mut surface);

    if let Some(pos) = args.iter().position(|a| a == "--screenshot") {
        if let Some(path_str) = args.get(pos + 1) {
            let p = Path::new(path_str);
            match surface.export_png(p) {
                Ok(_) => println!("[hyb-shell] Screenshot written successfully: {}", path_str),
                Err(e) => eprintln!("[hyb-shell] Error exporting screenshot: {}", e),
            }
            return;
        }
    }

    println!("[hyb-shell] Shell active. Taskbar positioned at: {}", shell.position);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_launcher_math_evaluation() {
        assert_eq!(evaluate_math_expr("1920*1080"), Some(2073600));
        assert_eq!(evaluate_math_expr("100 + 50"), Some(150));
        assert_eq!(evaluate_math_expr("500 - 200"), Some(300));
        assert_eq!(evaluate_math_expr("100 / 4"), Some(25));
        assert_eq!(evaluate_math_expr("100 / 0"), None);
    }

    #[test]
    fn test_launcher_query_filtering() {
        let mut launcher = LauncherState::new(1024, 768);
        launcher.handle_query("files");
        assert!(!launcher.results.is_empty());
        assert_eq!(launcher.results[0].0, "hyb-files");
    }
}
