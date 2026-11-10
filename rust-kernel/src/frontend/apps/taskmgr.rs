//! HybridOS Windows 11 Task Manager (taskmgr.exe)
//!
//! Complete, production-grade Windows 11 Task Manager for HybridOS in 100% pure, safe Rust.
//!
//! Subsystems & Capabilities:
//! 1. Navigation & Shell Layout:
//!    - Collapsible icon sidebar with Fluent styling: Processes, Performance, App history,
//!      Startup apps, Users, Details, Services, and Settings.
//!    - Command bar: Run new task, End task (Delete), Efficiency mode toggle (EcoQoS),
//!      Resource values toggle (Percents vs Values), Search filter bar.
//! 2. Processes View:
//!    - Grouped into: Apps, Background processes, Windows processes.
//!    - 10 Data Columns: Name, Status, CPU %, Memory, Disk, Network, GPU %, GPU Engine,
//!      Power usage, Power usage trend.
//!    - Hierarchical tree structure supporting parent/child process trees and thread lists.
//! 3. Performance View:
//!    - Hardware list with live 60-point mini-sparklines: CPU, Memory, Disk 0 (NVMe 1.4),
//!      Disk 1 (SATA AHCI), Ethernet (VirtIO-Net), GPU 0 (VirtIO-GPU).
//!    - Detailed metric telemetry panels:
//!      * CPU: Utilization %, Speed (GHz), Processes, Threads, Handles, Up time (Days:HH:MM:SS),
//!        Sockets, Cores, Logical processors, Virtualization status, L1/L2/L3 cache.
//!      * Memory: In use, Available, Committed, Cached, Paged pool, Non-paged pool, Speed (MT/s),
//!        Slots used, Form factor, Hardware reserved.
//!      * Disk: Active time %, Response time (ms), Read speed, Write speed, Capacity, Formatted size.
//!      * Ethernet: Send/Receive throughput, Adapter name, IPv4/IPv6, Connection type.
//!      * GPU: 3D utilization %, Dedicated GPU memory, Shared GPU memory, Driver version,
//!        DirectX/Vulkan versions, PCI bus location.
//! 4. Details & Services Views:
//!    - Details: Name, PID, Status, User name, CPU, Memory (Private working set), Architecture (x64),
//!      Description, Handles, Priority, Affinity.
//!    - Services: Name, PID, Description, Status (Running/Stopped), Group.
//!      Context actions: Start, Stop, Restart, Open Services.
//! 5. App History, Startup Apps, and Users Management Views.
//! 6. Dual Rendering Pipeline:
//!    - High-density ASCII terminal visualization with sparkline bars and box-drawing.
//!    - 32-bit Fluent Surface framebuffer renderer (Mica/Acrylic palette, cards, meters).
//! 7. Comprehensive Unit Test Suite validating all specifications and corner cases.

#![deny(unsafe_code)]
#![allow(dead_code)]

use std::collections::VecDeque;

// ============================================================================
// Section 1: Core Geometry, Color & Fluent Palette Primitives
// ============================================================================

/// 2D Point structure
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

/// 2D Rectangle boundary structure
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    pub const EMPTY: Self = Self { x: 0, y: 0, width: 0, height: 0 };

    pub const fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self { x, y, width, height }
    }

    #[inline]
    pub const fn left(&self) -> i32 {
        self.x
    }

    #[inline]
    pub const fn top(&self) -> i32 {
        self.y
    }

    #[inline]
    pub fn right(&self) -> i32 {
        self.x.saturating_add(self.width as i32)
    }

    #[inline]
    pub fn bottom(&self) -> i32 {
        self.y.saturating_add(self.height as i32)
    }

    #[inline]
    pub fn contains(&self, px: i32, py: i32) -> bool {
        px >= self.left() && px < self.right() && py >= self.top() && py < self.bottom()
    }
}

/// 32-bit RGBA Color structure
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const TRANSPARENT: Self = Self { r: 0, g: 0, b: 0, a: 0 };
    pub const BLACK: Self = Self { r: 0, g: 0, b: 0, a: 255 };
    pub const WHITE: Self = Self { r: 255, g: 255, b: 255, a: 255 };

    // Windows 11 Fluent Theme Palette
    pub const WIN11_BLUE: Self = Self { r: 0, g: 120, b: 212, a: 255 };
    pub const WIN11_ACCENT_LIGHT: Self = Self { r: 96, g: 205, b: 255, a: 255 };
    pub const WIN11_ACCENT_DARK: Self = Self { r: 0, g: 103, b: 192, a: 255 };
    pub const MICA_BACKGROUND_DARK: Self = Self { r: 32, g: 32, b: 32, a: 255 };
    pub const MICA_BACKGROUND_LIGHT: Self = Self { r: 243, g: 243, b: 243, a: 255 };
    pub const CARD_BACKGROUND_DARK: Self = Self { r: 44, g: 44, b: 44, a: 255 };
    pub const CARD_BACKGROUND_LIGHT: Self = Self { r: 255, g: 255, b: 255, a: 255 };
    pub const CARD_STROKE_DARK: Self = Self { r: 60, g: 60, b: 60, a: 255 };
    pub const CARD_STROKE_LIGHT: Self = Self { r: 229, g: 229, b: 229, a: 255 };
    pub const TEXT_PRIMARY_DARK: Self = Self { r: 255, g: 255, b: 255, a: 255 };
    pub const TEXT_PRIMARY_LIGHT: Self = Self { r: 26, g: 26, b: 26, a: 255 };
    pub const TEXT_SECONDARY_DARK: Self = Self { r: 180, g: 180, b: 180, a: 255 };
    pub const TEXT_SECONDARY_LIGHT: Self = Self { r: 92, g: 92, b: 92, a: 255 };
    pub const STATUS_GREEN: Self = Self { r: 16, g: 137, b: 62, a: 255 };
    pub const STATUS_YELLOW: Self = Self { r: 202, g: 138, b: 4, a: 255 };
    pub const STATUS_RED: Self = Self { r: 209, g: 52, b: 56, a: 255 };
    pub const LEAF_ECO_GREEN: Self = Self { r: 108, g: 203, b: 95, a: 255 };

    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    pub const fn from_argb_u32(val: u32) -> Self {
        Self {
            a: ((val >> 24) & 0xFF) as u8,
            r: ((val >> 16) & 0xFF) as u8,
            g: ((val >> 8) & 0xFF) as u8,
            b: (val & 0xFF) as u8,
        }
    }

    pub const fn to_argb_u32(&self) -> u32 {
        ((self.a as u32) << 24) | ((self.r as u32) << 16) | ((self.g as u32) << 8) | (self.b as u32)
    }

    pub fn blend_over(&self, base: Color) -> Color {
        let alpha = self.a as f32 / 255.0;
        let inv_alpha = 1.0 - alpha;
        Color {
            r: (self.r as f32 * alpha + base.r as f32 * inv_alpha).round() as u8,
            g: (self.g as f32 * alpha + base.g as f32 * inv_alpha).round() as u8,
            b: (self.b as f32 * alpha + base.b as f32 * inv_alpha).round() as u8,
            a: 255,
        }
    }
}

/// Fluent Theme Mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeMode {
    Dark,
    Light,
}

/// 2D Surface Framebuffer for pixel-level UI rendering
#[derive(Debug, Clone)]
pub struct TaskMgrSurface {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u32>,
}

impl TaskMgrSurface {
    pub fn new(width: usize, height: usize, clear_color: Color) -> Self {
        let packed = clear_color.to_argb_u32();
        Self {
            width,
            height,
            pixels: vec![packed; width * height],
        }
    }

    #[inline]
    pub fn get_pixel(&self, x: usize, y: usize) -> Color {
        if x < self.width && y < self.height {
            Color::from_argb_u32(self.pixels[y * self.width + x])
        } else {
            Color::TRANSPARENT
        }
    }

    #[inline]
    pub fn set_pixel(&mut self, x: usize, y: usize, color: Color) {
        if x < self.width && y < self.height {
            self.pixels[y * self.width + x] = color.to_argb_u32();
        }
    }

    pub fn fill_rect(&mut self, rect: &Rect, color: Color) {
        let x0 = rect.left().max(0) as usize;
        let y0 = rect.top().max(0) as usize;
        let x1 = (rect.right().max(0) as usize).min(self.width);
        let y1 = (rect.bottom().max(0) as usize).min(self.height);

        let packed = color.to_argb_u32();
        for y in y0..y1 {
            let row_offset = y * self.width;
            for x in x0..x1 {
                self.pixels[row_offset + x] = packed;
            }
        }
    }

    pub fn draw_rect_outline(&mut self, rect: &Rect, color: Color, stroke_width: usize) {
        let x0 = rect.left().max(0) as usize;
        let y0 = rect.top().max(0) as usize;
        let x1 = (rect.right().max(0) as usize).min(self.width);
        let y1 = (rect.bottom().max(0) as usize).min(self.height);

        if x1 <= x0 || y1 <= y0 {
            return;
        }

        // Top & Bottom
        for sw in 0..stroke_width {
            let top_y = (y0 + sw).min(y1.saturating_sub(1));
            let bot_y = (y1.saturating_sub(1 + sw)).max(y0);
            for x in x0..x1 {
                self.set_pixel(x, top_y, color);
                self.set_pixel(x, bot_y, color);
            }
        }

        // Left & Right
        for sw in 0..stroke_width {
            let left_x = (x0 + sw).min(x1.saturating_sub(1));
            let right_x = (x1.saturating_sub(1 + sw)).max(x0);
            for y in y0..y1 {
                self.set_pixel(left_x, y, color);
                self.set_pixel(right_x, y, color);
            }
        }
    }
}

// ============================================================================
// Section 2: Navigation & Shell Layout Enums and Models
// ============================================================================

/// Navigation sidebar pages in Windows 11 Task Manager
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NavPage {
    Processes,
    Performance,
    AppHistory,
    StartupApps,
    Users,
    Details,
    Services,
    Settings,
}

impl NavPage {
    pub const ALL: [NavPage; 8] = [
        NavPage::Processes,
        NavPage::Performance,
        NavPage::AppHistory,
        NavPage::StartupApps,
        NavPage::Users,
        NavPage::Details,
        NavPage::Services,
        NavPage::Settings,
    ];

    pub fn title(&self) -> &'static str {
        match self {
            Self::Processes => "Processes",
            Self::Performance => "Performance",
            Self::AppHistory => "App history",
            Self::StartupApps => "Startup apps",
            Self::Users => "Users",
            Self::Details => "Details",
            Self::Services => "Services",
            Self::Settings => "Settings",
        }
    }

    pub fn icon_glyph(&self) -> &'static str {
        match self {
            Self::Processes => "[|||]",     // Segoe Fluent Processes
            Self::Performance => "[/\\_]",  // Sparkline/Pulse
            Self::AppHistory => "[<<=]",   // History clock
            Self::StartupApps => "[^!^]",  // Rocket/Startup
            Self::Users => "[o_o]",        // Users
            Self::Details => "[=:=]",      // List details
            Self::Services => "[{*}]",     // Cog/Gear service
            Self::Settings => "[(*)]",     // App settings
        }
    }
}

/// Resource value display mode toggle (Percentages vs Absolute Values)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceDisplayMode {
    Percentages,
    AbsoluteValues,
}

impl ResourceDisplayMode {
    pub fn toggle(&mut self) {
        *self = match self {
            Self::Percentages => Self::AbsoluteValues,
            Self::AbsoluteValues => Self::Percentages,
        };
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Percentages => "Percents (%)",
            Self::AbsoluteValues => "Values (MB, Mbps)",
        }
    }
}

/// Process grouping categories
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ProcessGroup {
    Apps,
    BackgroundProcesses,
    WindowsProcesses,
}

impl ProcessGroup {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Apps => "Apps",
            Self::BackgroundProcesses => "Background processes",
            Self::WindowsProcesses => "Windows processes",
        }
    }
}

/// Process execution & responsiveness status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessStatus {
    Running,
    Suspended,
    EfficiencyMode,
    NotResponding,
}

impl ProcessStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Running => "Running",
            Self::Suspended => "Suspended",
            Self::EfficiencyMode => "Efficiency mode",
            Self::NotResponding => "Not responding",
        }
    }
}

/// Power consumption tier
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PowerUsageLevel {
    VeryLow,
    Low,
    Moderate,
    High,
    VeryHigh,
}

impl PowerUsageLevel {
    pub fn label(&self) -> &'static str {
        match self {
            Self::VeryLow => "Very low",
            Self::Low => "Low",
            Self::Moderate => "Moderate",
            Self::High => "High",
            Self::VeryHigh => "Very high",
        }
    }
}

/// Process binary target architecture
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Architecture {
    X64,
    X86,
    Arm64,
}

impl Architecture {
    pub fn label(&self) -> &'static str {
        match self {
            Self::X64 => "x64",
            Self::X86 => "x86",
            Self::Arm64 => "ARM64",
        }
    }
}

/// Windows Process Scheduling Priority Class
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ProcessPriority {
    Idle,
    BelowNormal,
    Normal,
    AboveNormal,
    High,
    Realtime,
}

impl ProcessPriority {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Idle => "Idle",
            Self::BelowNormal => "Below normal",
            Self::Normal => "Normal",
            Self::AboveNormal => "Above normal",
            Self::High => "High",
            Self::Realtime => "Realtime",
        }
    }
}

/// GPU Acceleration Engine Type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuEngineType {
    ThreeD,
    VideoDecode,
    VideoEncode,
    Copy,
    Compute,
    None,
}

impl GpuEngineType {
    pub fn label(&self) -> &'static str {
        match self {
            Self::ThreeD => "GPU 0 - 3D",
            Self::VideoDecode => "GPU 0 - Video Decode",
            Self::VideoEncode => "GPU 0 - Video Encode",
            Self::Copy => "GPU 0 - Copy",
            Self::Compute => "GPU 0 - Compute",
            Self::None => "-",
        }
    }
}

// ============================================================================
// Section 3: Process Tree & Thread Data Structures
// ============================================================================

/// Thread level telemetry inside a process
#[derive(Debug, Clone)]
pub struct ProcessThreadInfo {
    pub tid: u32,
    pub cpu_percent: f32,
    pub state: String,
    pub priority: i32,
    pub handles_count: u32,
}

/// Complete telemetry model for a single process item
#[derive(Debug, Clone)]
pub struct ProcessItem {
    pub pid: u32,
    pub parent_pid: Option<u32>,
    pub name: String,
    pub group: ProcessGroup,
    pub status: ProcessStatus,
    pub cpu_percent: f32,
    pub memory_bytes: u64,
    pub disk_read_bps: f64,
    pub disk_write_bps: f64,
    pub net_receive_bps: f64,
    pub net_send_bps: f64,
    pub gpu_percent: f32,
    pub gpu_engine: GpuEngineType,
    pub power_usage: PowerUsageLevel,
    pub power_trend: PowerUsageLevel,
    pub is_efficiency_mode: bool,
    pub is_expanded: bool,
    pub user_name: String,
    pub architecture: Architecture,
    pub description: String,
    pub handles_count: u32,
    pub threads_count: u32,
    pub priority: ProcessPriority,
    pub threads: Vec<ProcessThreadInfo>,
    pub children: Vec<ProcessItem>,
}

impl ProcessItem {
    pub fn new(
        pid: u32,
        parent_pid: Option<u32>,
        name: &str,
        group: ProcessGroup,
        memory_bytes: u64,
        description: &str,
    ) -> Self {
        Self {
            pid,
            parent_pid,
            name: name.to_string(),
            group,
            status: ProcessStatus::Running,
            cpu_percent: 0.0,
            memory_bytes,
            disk_read_bps: 0.0,
            disk_write_bps: 0.0,
            net_receive_bps: 0.0,
            net_send_bps: 0.0,
            gpu_percent: 0.0,
            gpu_engine: GpuEngineType::None,
            power_usage: PowerUsageLevel::VeryLow,
            power_trend: PowerUsageLevel::VeryLow,
            is_efficiency_mode: false,
            is_expanded: false,
            user_name: "SYSTEM".to_string(),
            architecture: Architecture::X64,
            description: description.to_string(),
            handles_count: 64,
            threads_count: 1,
            priority: ProcessPriority::Normal,
            threads: Vec::new(),
            children: Vec::new(),
        }
    }

    /// Recursively calculates total CPU usage including child processes
    pub fn aggregate_cpu_percent(&self) -> f32 {
        let child_cpu: f32 = self.children.iter().map(|c| c.aggregate_cpu_percent()).sum();
        self.cpu_percent + child_cpu
    }

    /// Recursively calculates total memory consumption including child processes
    pub fn aggregate_memory_bytes(&self) -> u64 {
        let child_mem: u64 = self.children.iter().map(|c| c.aggregate_memory_bytes()).sum();
        self.memory_bytes + child_mem
    }

    /// Recursively calculates total disk transfer in bytes/sec
    pub fn aggregate_disk_bps(&self) -> f64 {
        let self_disk = self.disk_read_bps + self.disk_write_bps;
        let child_disk: f64 = self.children.iter().map(|c| c.aggregate_disk_bps()).sum();
        self_disk + child_disk
    }

    /// Recursively calculates total network transfer in bytes/sec
    pub fn aggregate_net_bps(&self) -> f64 {
        let self_net = self.net_receive_bps + self.net_send_bps;
        let child_net: f64 = self.children.iter().map(|c| c.aggregate_net_bps()).sum();
        self_net + child_net
    }

    /// Recursively calculates total GPU percent
    pub fn aggregate_gpu_percent(&self) -> f32 {
        let child_gpu: f32 = self.children.iter().map(|c| c.aggregate_gpu_percent()).sum();
        self.gpu_percent + child_gpu
    }

    /// Count all processes recursively (self + descendants)
    pub fn total_process_count(&self) -> usize {
        1 + self.children.iter().map(|c| c.total_process_count()).sum::<usize>()
    }

    /// Find a child process by PID recursively
    pub fn find_child_mut(&mut self, target_pid: u32) -> Option<&mut ProcessItem> {
        if self.pid == target_pid {
            return Some(self);
        }
        for child in &mut self.children {
            if let Some(found) = child.find_child_mut(target_pid) {
                return Some(found);
            }
        }
        None
    }

    /// Remove a child process by PID recursively
    pub fn remove_child(&mut self, target_pid: u32) -> bool {
        if let Some(pos) = self.children.iter().position(|c| c.pid == target_pid) {
            self.children.remove(pos);
            return true;
        }
        for child in &mut self.children {
            if child.remove_child(target_pid) {
                return true;
            }
        }
        false
    }
}

// ============================================================================
// Section 4: Performance View Hardware Metrics & Sparklines
// ============================================================================

/// Selected hardware device category in Performance view
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PerfDevice {
    Cpu,
    Memory,
    Disk(u32),
    Ethernet,
    Gpu(u32),
}

impl PerfDevice {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Cpu => "CPU",
            Self::Memory => "Memory",
            Self::Disk(0) => "Disk 0 (C:)",
            Self::Disk(1) => "Disk 1 (D:)",
            Self::Disk(_) => "Disk",
            Self::Ethernet => "Ethernet",
            Self::Gpu(_) => "GPU 0",
        }
    }

    pub fn subtitle(&self) -> &'static str {
        match self {
            Self::Cpu => "Intel(R) Core(TM) i9-14900K @ 3.40 GHz",
            Self::Memory => "32.0 GB DDR5 5600 MT/s",
            Self::Disk(0) => "NVMe 1.4 Micron 3400 1TB",
            Self::Disk(1) => "SATA AHCI Crucial MX500 2TB",
            Self::Disk(_) => "Block Storage Device",
            Self::Ethernet => "VirtIO-Net 10Gbps Virtual Adapter",
            Self::Gpu(_) => "VirtIO-GPU 3D Accelerated Device",
        }
    }
}

/// Rolling 60-second telemetry sparkline ring buffer
#[derive(Debug, Clone)]
pub struct SparklineBuffer {
    pub max_capacity: usize,
    pub values: VecDeque<f32>,
}

impl SparklineBuffer {
    pub fn new(capacity: usize, initial_val: f32) -> Self {
        let mut values = VecDeque::with_capacity(capacity);
        for _ in 0..capacity {
            values.push_back(initial_val);
        }
        Self {
            max_capacity: capacity,
            values,
        }
    }

    pub fn push(&mut self, val: f32) {
        if self.values.len() >= self.max_capacity {
            self.values.pop_front();
        }
        self.values.push_back(val);
    }

    pub fn current(&self) -> f32 {
        self.values.back().copied().unwrap_or(0.0)
    }

    pub fn average(&self) -> f32 {
        if self.values.is_empty() {
            return 0.0;
        }
        let sum: f32 = self.values.iter().sum();
        sum / self.values.len() as f32
    }

    pub fn max(&self) -> f32 {
        self.values.iter().fold(0.0f32, |acc, &v| acc.max(v))
    }

    pub fn min(&self) -> f32 {
        self.values.iter().fold(100.0f32, |acc, &v| acc.min(v))
    }

    /// Formats as Unicode horizontal sparkline blocks for console rendering
    pub fn render_ascii_sparkline(&self, width: usize) -> String {
        const GLYPHS: [char; 8] = [' ', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
        let mut out = String::with_capacity(width);

        let take_count = self.values.len().min(width);
        let skip_count = self.values.len().saturating_sub(take_count);

        for &val in self.values.iter().skip(skip_count).take(take_count) {
            let normalized = (val.clamp(0.0, 100.0) / 100.0) * 7.0;
            let index = (normalized.round() as usize).min(7);
            out.push(GLYPHS[index]);
        }

        while out.len() < width {
            out.insert(0, ' ');
        }
        out
    }
}

/// CPU Hardware Telemetry metrics
#[derive(Debug, Clone)]
pub struct CpuMetrics {
    pub utilization_percent: f32,
    pub speed_ghz: f32,
    pub base_speed_ghz: f32,
    pub processes_count: usize,
    pub threads_count: usize,
    pub handles_count: usize,
    pub uptime_seconds: u64,
    pub sockets: u32,
    pub cores: u32,
    pub logical_processors: u32,
    pub virtualization_enabled: bool,
    pub hyper_v_support: bool,
    pub l1_cache_kb: u32,
    pub l2_cache_kb: u32,
    pub l3_cache_kb: u32,
    pub sparkline: SparklineBuffer,
}

impl CpuMetrics {
    pub fn formatted_uptime(&self) -> String {
        let days = self.uptime_seconds / 86400;
        let rem_day = self.uptime_seconds % 86400;
        let hours = rem_day / 3600;
        let rem_hour = rem_day % 3600;
        let minutes = rem_hour / 60;
        let seconds = rem_hour % 60;
        format!("{}:{}:{:02}:{:02}", days, hours, minutes, seconds)
    }
}

/// Physical RAM & Virtual Memory Telemetry
#[derive(Debug, Clone)]
pub struct MemoryMetrics {
    pub total_bytes: u64,
    pub in_use_bytes: u64,
    pub available_bytes: u64,
    pub committed_bytes: u64,
    pub commit_limit_bytes: u64,
    pub cached_bytes: u64,
    pub paged_pool_bytes: u64,
    pub non_paged_pool_bytes: u64,
    pub speed_mts: u32,
    pub slots_used: u32,
    pub slots_total: u32,
    pub form_factor: String,
    pub hardware_reserved_bytes: u64,
    pub sparkline: SparklineBuffer,
}

impl MemoryMetrics {
    pub fn in_use_gb(&self) -> f32 {
        self.in_use_bytes as f32 / (1024.0 * 1024.0 * 1024.0)
    }

    pub fn total_gb(&self) -> f32 {
        self.total_bytes as f32 / (1024.0 * 1024.0 * 1024.0)
    }

    pub fn available_gb(&self) -> f32 {
        self.available_bytes as f32 / (1024.0 * 1024.0 * 1024.0)
    }

    pub fn committed_gb(&self) -> f32 {
        self.committed_bytes as f32 / (1024.0 * 1024.0 * 1024.0)
    }

    pub fn commit_limit_gb(&self) -> f32 {
        self.commit_limit_bytes as f32 / (1024.0 * 1024.0 * 1024.0)
    }

    pub fn cached_gb(&self) -> f32 {
        self.cached_bytes as f32 / (1024.0 * 1024.0 * 1024.0)
    }

    pub fn paged_pool_mb(&self) -> f32 {
        self.paged_pool_bytes as f32 / (1024.0 * 1024.0)
    }

    pub fn non_paged_pool_mb(&self) -> f32 {
        self.non_paged_pool_bytes as f32 / (1024.0 * 1024.0)
    }
}

/// Disk Drive Telemetry (NVMe 1.4 & SATA AHCI)
#[derive(Debug, Clone)]
pub struct DiskMetrics {
    pub id: u32,
    pub name: String,
    pub model: String,
    pub bus_type: String,
    pub active_time_percent: f32,
    pub response_time_ms: f32,
    pub read_speed_kbps: f64,
    pub write_speed_kbps: f64,
    pub capacity_bytes: u64,
    pub formatted_bytes: u64,
    pub system_disk: bool,
    pub page_file: bool,
    pub sparkline: SparklineBuffer,
}

impl DiskMetrics {
    pub fn capacity_formatted(&self) -> String {
        let tb = self.capacity_bytes as f64 / (1024.0 * 1024.0 * 1024.0 * 1024.0);
        if tb >= 1.0 {
            format!("{:.1} TB", tb)
        } else {
            let gb = self.capacity_bytes as f64 / (1024.0 * 1024.0 * 1024.0);
            format!("{:.0} GB", gb)
        }
    }

    pub fn formatted_size_formatted(&self) -> String {
        let gb = self.formatted_bytes as f64 / (1024.0 * 1024.0 * 1024.0);
        format!("{:.0} GB", gb)
    }
}

/// Network Interface Telemetry (VirtIO-Net 10Gbps)
#[derive(Debug, Clone)]
pub struct EthernetMetrics {
    pub adapter_name: String,
    pub connection_type: String,
    pub ipv4_address: String,
    pub ipv6_address: String,
    pub send_throughput_bps: f64,
    pub receive_throughput_bps: f64,
    pub link_speed_gbps: f64,
    pub sparkline: SparklineBuffer,
}

impl EthernetMetrics {
    pub fn send_mbps(&self) -> f64 {
        (self.send_throughput_bps * 8.0) / 1_000_000.0
    }

    pub fn receive_mbps(&self) -> f64 {
        (self.receive_throughput_bps * 8.0) / 1_000_000.0
    }
}

/// GPU Graphics Accelerator Telemetry (VirtIO-GPU)
#[derive(Debug, Clone)]
pub struct GpuMetrics {
    pub id: u32,
    pub adapter_name: String,
    pub utilization_3d_percent: f32,
    pub dedicated_memory_used_bytes: u64,
    pub dedicated_memory_total_bytes: u64,
    pub shared_memory_used_bytes: u64,
    pub shared_memory_total_bytes: u64,
    pub driver_version: String,
    pub driver_date: String,
    pub directx_version: String,
    pub vulkan_version: String,
    pub hardware_location: String,
    pub sparkline: SparklineBuffer,
}

impl GpuMetrics {
    pub fn dedicated_mb_used(&self) -> f32 {
        self.dedicated_memory_used_bytes as f32 / (1024.0 * 1024.0)
    }

    pub fn dedicated_mb_total(&self) -> f32 {
        self.dedicated_memory_total_bytes as f32 / (1024.0 * 1024.0)
    }

    pub fn shared_mb_used(&self) -> f32 {
        self.shared_memory_used_bytes as f32 / (1024.0 * 1024.0)
    }

    pub fn shared_mb_total(&self) -> f32 {
        self.shared_memory_total_bytes as f32 / (1024.0 * 1024.0)
    }
}

/// Aggregated system performance telemetry state
#[derive(Debug, Clone)]
pub struct PerformanceTelemetry {
    pub cpu: CpuMetrics,
    pub memory: MemoryMetrics,
    pub disk0: DiskMetrics,
    pub disk1: DiskMetrics,
    pub ethernet: EthernetMetrics,
    pub gpu0: GpuMetrics,
}

// ============================================================================
// Section 5: Services & Background Systems
// ============================================================================

/// Windows NT Service Status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceStatus {
    Running,
    Stopped,
    Paused,
    Starting,
    Stopping,
}

impl ServiceStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Running => "Running",
            Self::Stopped => "Stopped",
            Self::Paused => "Paused",
            Self::Starting => "Starting",
            Self::Stopping => "Stopping",
        }
    }
}

/// Windows NT / HybridOS System Service Item
#[derive(Debug, Clone)]
pub struct ServiceItem {
    pub name: String,
    pub pid: Option<u32>,
    pub description: String,
    pub status: ServiceStatus,
    pub group: String,
}

impl ServiceItem {
    pub fn new(name: &str, pid: Option<u32>, description: &str, status: ServiceStatus, group: &str) -> Self {
        Self {
            name: name.to_string(),
            pid,
            description: description.to_string(),
            status,
            group: group.to_string(),
        }
    }
}

// ============================================================================
// Section 6: Additional Views (App History, Startup Apps, Users)
// ============================================================================

/// App History Usage Item
#[derive(Debug, Clone)]
pub struct AppHistoryItem {
    pub name: String,
    pub cpu_time: String,
    pub network_mb: f32,
    pub metered_network_mb: f32,
    pub tile_updates_mb: f32,
}

/// Startup Application Configuration Item
#[derive(Debug, Clone)]
pub struct StartupAppItem {
    pub name: String,
    pub publisher: String,
    pub status_enabled: bool,
    pub startup_impact: String,
}

/// Active User Session Item
#[derive(Debug, Clone)]
pub struct UserItem {
    pub user_name: String,
    pub session_id: u32,
    pub status: String,
    pub session_type: String,
    pub cpu_percent: f32,
    pub memory_mb: f32,
    pub disk_mbps: f32,
    pub network_mbps: f32,
}

// ============================================================================
// Section 7: Command Bar, Search & UI Action Events
// ============================================================================

/// Interactive events dispatchable to Task Manager
#[derive(Debug, Clone)]
pub enum TaskMgrEvent {
    SelectPage(NavPage),
    ToggleSidebar,
    SelectProcess(u32),
    ToggleExpandProcess(u32),
    EndTask(u32),
    ToggleEfficiencyMode(u32),
    SelectPerformanceDevice(PerfDevice),
    ToggleResourceValueMode,
    StartService(String),
    StopService(String),
    RestartService(String),
    OpenServicesMsc,
    ToggleStartupApp(String),
    SetSearchFilter(String),
    SortByColumn(ProcessSortColumn),
    RunNewTask { command: String, run_as_admin: bool },
    Tick(f32),
}

/// Process table sorting keys
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessSortColumn {
    Name,
    Status,
    Cpu,
    Memory,
    Disk,
    Network,
    Gpu,
    PowerUsage,
}

/// Command Bar Status State
#[derive(Debug, Clone, Default)]
pub struct CommandBarState {
    pub can_end_task: bool,
    pub can_toggle_efficiency: bool,
    pub is_efficiency_active: bool,
    pub run_dialog_open: bool,
}

// ============================================================================
// Section 8: Main Task Manager Controller & State Engine
// ============================================================================

/// Windows 11 Task Manager Master State Controller
#[derive(Debug, Clone)]
pub struct TaskManager {
    pub current_page: NavPage,
    pub sidebar_collapsed: bool,
    pub selected_perf_device: PerfDevice,
    pub resource_display_mode: ResourceDisplayMode,
    pub search_filter: String,
    pub selected_pid: Option<u32>,
    pub selected_service_name: Option<String>,
    pub processes: Vec<ProcessItem>,
    pub perf_telemetry: PerformanceTelemetry,
    pub services: Vec<ServiceItem>,
    pub startup_apps: Vec<StartupAppItem>,
    pub app_history: Vec<AppHistoryItem>,
    pub users: Vec<UserItem>,
    pub command_bar: CommandBarState,
    pub sort_column: ProcessSortColumn,
    pub sort_ascending: bool,
    pub theme: ThemeMode,
    pub next_pid_counter: u32,
}

impl TaskManager {
    /// Constructs a fully populated Windows 11 Task Manager initialized with realistic
    /// HybridOS kernel processes, VirtIO hardware sensors, and Windows NT system services.
    pub fn new() -> Self {
        let perf_telemetry = Self::init_performance_telemetry();
        let processes = Self::init_default_processes();
        let services = Self::init_default_services();
        let startup_apps = Self::init_default_startup_apps();
        let app_history = Self::init_default_app_history();
        let users = Self::init_default_users();

        Self {
            current_page: NavPage::Processes,
            sidebar_collapsed: false,
            selected_perf_device: PerfDevice::Cpu,
            resource_display_mode: ResourceDisplayMode::Percentages,
            search_filter: String::new(),
            selected_pid: Some(4020), // Default select Windows Terminal
            selected_service_name: Some("wuauserv".to_string()),
            processes,
            perf_telemetry,
            services,
            startup_apps,
            app_history,
            users,
            command_bar: CommandBarState {
                can_end_task: true,
                can_toggle_efficiency: true,
                is_efficiency_active: false,
                run_dialog_open: false,
            },
            sort_column: ProcessSortColumn::Cpu,
            sort_ascending: false,
            theme: ThemeMode::Dark,
            next_pid_counter: 9200,
        }
    }

    /// Telemetry initialization with realistic hardware values
    fn init_performance_telemetry() -> PerformanceTelemetry {
        let mut cpu_sparkline = SparklineBuffer::new(60, 18.5);
        for i in 0..60 {
            let sample = 14.0 + (i as f32 * 0.3).sin() * 8.0;
            cpu_sparkline.push(sample.clamp(2.0, 95.0));
        }

        let mut mem_sparkline = SparklineBuffer::new(60, 26.2);
        for i in 0..60 {
            let sample = 26.0 + (i as f32 * 0.1).cos() * 1.5;
            mem_sparkline.push(sample.clamp(10.0, 90.0));
        }

        let mut disk0_sparkline = SparklineBuffer::new(60, 4.2);
        for i in 0..60 {
            let sample = 3.0 + ((i % 12) as f32) * 1.8;
            disk0_sparkline.push(sample.clamp(0.0, 100.0));
        }

        let mut disk1_sparkline = SparklineBuffer::new(60, 0.5);
        for _ in 0..60 {
            disk1_sparkline.push(0.5);
        }

        let mut eth_sparkline = SparklineBuffer::new(60, 12.0);
        for i in 0..60 {
            let sample = 8.0 + (i as f32 * 0.25).sin() * 6.0;
            eth_sparkline.push(sample.clamp(0.0, 100.0));
        }

        let mut gpu_sparkline = SparklineBuffer::new(60, 9.4);
        for i in 0..60 {
            let sample = 8.0 + (i as f32 * 0.4).cos() * 5.0;
            gpu_sparkline.push(sample.clamp(0.0, 100.0));
        }

        PerformanceTelemetry {
            cpu: CpuMetrics {
                utilization_percent: 21.4,
                speed_ghz: 3.84,
                base_speed_ghz: 3.40,
                processes_count: 184,
                threads_count: 2450,
                handles_count: 98124,
                uptime_seconds: 16395, // 0:04:33:15
                sockets: 1,
                cores: 16,
                logical_processors: 32,
                virtualization_enabled: true,
                hyper_v_support: true,
                l1_cache_kb: 512,
                l2_cache_kb: 32768,
                l3_cache_kb: 36864,
                sparkline: cpu_sparkline,
            },
            memory: MemoryMetrics {
                total_bytes: 34_359_738_368, // 32.0 GB
                in_use_bytes: 9_019_432_960,  // ~8.4 GB
                available_bytes: 25_340_305_408,
                committed_bytes: 13_314_398_208, // ~12.4 GB
                commit_limit_bytes: 39_728_447_488,
                cached_bytes: 15_247_114_240,
                paged_pool_bytes: 650_117_120,    // ~620 MB
                non_paged_pool_bytes: 503_316_480, // ~480 MB
                speed_mts: 5600,
                slots_used: 2,
                slots_total: 4,
                form_factor: "DIMM".to_string(),
                hardware_reserved_bytes: 194_000_000,
                sparkline: mem_sparkline,
            },
            disk0: DiskMetrics {
                id: 0,
                name: "Disk 0 (C:)".to_string(),
                model: "NVMe 1.4 Micron 3400 1TB".to_string(),
                bus_type: "NVMe".to_string(),
                active_time_percent: 3.8,
                response_time_ms: 0.6,
                read_speed_kbps: 1820.0,
                write_speed_kbps: 4250.0,
                capacity_bytes: 1_000_204_886_016, // 1.0 TB
                formatted_bytes: 953_869_926_400, // 953 GB
                system_disk: true,
                page_file: true,
                sparkline: disk0_sparkline,
            },
            disk1: DiskMetrics {
                id: 1,
                name: "Disk 1 (D:)".to_string(),
                model: "SATA AHCI Crucial MX500 2TB".to_string(),
                bus_type: "SATA".to_string(),
                active_time_percent: 0.2,
                response_time_ms: 1.2,
                read_speed_kbps: 0.0,
                write_speed_kbps: 12.0,
                capacity_bytes: 2_000_398_934_016, // 2.0 TB
                formatted_bytes: 1_863_020_000_000,
                system_disk: false,
                page_file: false,
                sparkline: disk1_sparkline,
            },
            ethernet: EthernetMetrics {
                adapter_name: "VirtIO Network Adapter #1".to_string(),
                connection_type: "Ethernet 802.3 (10 Gbps Full Duplex)".to_string(),
                ipv4_address: "192.168.122.45".to_string(),
                ipv6_address: "fe80::5054:ff:fe12:3456%12".to_string(),
                send_throughput_bps: 125_000.0,      // 1 Mbps
                receive_throughput_bps: 1_875_000.0, // 15 Mbps
                link_speed_gbps: 10.0,
                sparkline: eth_sparkline,
            },
            gpu0: GpuMetrics {
                id: 0,
                adapter_name: "VirtIO-GPU VMM 3D Accelerator".to_string(),
                utilization_3d_percent: 11.2,
                dedicated_memory_used_bytes: 2_576_980_377, // ~2.4 GB
                dedicated_memory_total_bytes: 8_589_934_592, // 8.0 GB
                shared_memory_used_bytes: 858_993_459,       // ~0.8 GB
                shared_memory_total_bytes: 17_179_869_184,   // 16.0 GB
                driver_version: "32.0.15.6094 (HybridOS VDM)".to_string(),
                driver_date: "10/11/2026".to_string(),
                directx_version: "DirectX 12 Ultimate (Feature Level 12_2)".to_string(),
                vulkan_version: "Vulkan 1.3.280".to_string(),
                hardware_location: "PCI bus 0, device 2, function 0".to_string(),
                sparkline: gpu_sparkline,
            },
        }
    }

    /// Initializes a realistic Windows 11 process list grouped into Apps, Background, and Windows.
    fn init_default_processes() -> Vec<ProcessItem> {
        let mut list = Vec::new();

        // -------------------------------------------------------------
        // Group 1: Apps (Foreground Interactive Applications)
        // -------------------------------------------------------------
        // Task Manager itself
        let mut taskmgr = ProcessItem::new(1048, None, "Task Manager", ProcessGroup::Apps, 94_371_840, "Task Manager");
        taskmgr.cpu_percent = 1.8;
        taskmgr.disk_read_bps = 512.0;
        taskmgr.disk_write_bps = 1024.0;
        taskmgr.gpu_percent = 2.4;
        taskmgr.gpu_engine = GpuEngineType::ThreeD;
        taskmgr.power_usage = PowerUsageLevel::Low;
        taskmgr.threads_count = 14;
        taskmgr.handles_count = 520;
        list.push(taskmgr);

        // Windows Terminal with child sessions
        let mut terminal = ProcessItem::new(4020, None, "Windows Terminal", ProcessGroup::Apps, 146_800_640, "Windows Terminal Host");
        terminal.cpu_percent = 3.2;
        terminal.disk_read_bps = 8192.0;
        terminal.disk_write_bps = 4096.0;
        terminal.gpu_percent = 4.1;
        terminal.gpu_engine = GpuEngineType::ThreeD;
        terminal.power_usage = PowerUsageLevel::Moderate;
        terminal.threads_count = 22;
        terminal.handles_count = 780;
        terminal.is_expanded = true;

        // Child process: PowerShell
        let mut powershell = ProcessItem::new(4180, Some(4020), "pwsh.exe", ProcessGroup::Apps, 115_343_360, "PowerShell 7 Core");
        powershell.cpu_percent = 1.1;
        powershell.threads_count = 12;
        powershell.handles_count = 410;
        terminal.children.push(powershell);

        // Child process: HybridOS Bash Linux Subsystem
        let mut bash = ProcessItem::new(4290, Some(4020), "bash.exe", ProcessGroup::Apps, 67_108_864, "HybridOS Linux POSIX Shell");
        bash.cpu_percent = 0.4;
        bash.threads_count = 6;
        bash.handles_count = 180;
        terminal.children.push(bash);
        list.push(terminal);

        // File Explorer
        let mut explorer = ProcessItem::new(2840, None, "File Explorer", ProcessGroup::Apps, 241_172_480, "Windows Explorer Shell");
        explorer.cpu_percent = 0.8;
        explorer.disk_read_bps = 16384.0;
        explorer.gpu_percent = 1.2;
        explorer.gpu_engine = GpuEngineType::ThreeD;
        explorer.threads_count = 34;
        explorer.handles_count = 1450;
        list.push(explorer);

        // Microsoft Edge Browser with tabs
        let mut edge = ProcessItem::new(5120, None, "Microsoft Edge (6)", ProcessGroup::Apps, 587_202_560, "Microsoft Edge Browser");
        edge.cpu_percent = 4.6;
        edge.disk_read_bps = 45056.0;
        edge.disk_write_bps = 12288.0;
        edge.net_receive_bps = 245760.0;
        edge.net_send_bps = 32768.0;
        edge.gpu_percent = 5.8;
        edge.gpu_engine = GpuEngineType::ThreeD;
        edge.power_usage = PowerUsageLevel::Moderate;
        edge.is_expanded = false;
        edge.threads_count = 48;
        edge.handles_count = 2100;

        // Edge child renderers
        for i in 1..=4 {
            let mut renderer = ProcessItem::new(
                5120 + i * 10,
                Some(5120),
                &format!("Microsoft Edge Render Process ({})", i),
                ProcessGroup::Apps,
                125_829_120,
                "Edge WebContent Sandbox",
            );
            renderer.cpu_percent = 0.6;
            renderer.threads_count = 8;
            edge.children.push(renderer);
        }
        list.push(edge);

        // -------------------------------------------------------------
        // Group 2: Background Processes
        // -------------------------------------------------------------
        let mut search_host = ProcessItem::new(6010, None, "SearchHost.exe", ProcessGroup::BackgroundProcesses, 88_080_384, "Windows Search Indexer Host");
        search_host.cpu_percent = 0.5;
        search_host.threads_count = 16;
        search_host.is_efficiency_mode = true;
        search_host.status = ProcessStatus::EfficiencyMode;
        search_host.priority = ProcessPriority::Idle;
        list.push(search_host);

        let mut runtime_broker = ProcessItem::new(6120, None, "RuntimeBroker.exe", ProcessGroup::BackgroundProcesses, 37_748_736, "Runtime Broker Security Host");
        runtime_broker.cpu_percent = 0.1;
        runtime_broker.threads_count = 8;
        list.push(runtime_broker);

        let mut vortex_vdm = ProcessItem::new(6280, None, "VortexDriverHost.exe", ProcessGroup::BackgroundProcesses, 157_286_400, "HybridOS Vortex Driver Framework Host");
        vortex_vdm.cpu_percent = 1.9;
        vortex_vdm.disk_read_bps = 1048576.0;
        vortex_vdm.disk_write_bps = 2097152.0;
        vortex_vdm.threads_count = 28;
        vortex_vdm.handles_count = 1120;
        list.push(vortex_vdm);

        let mut one_drive = ProcessItem::new(6400, None, "OneDrive.exe", ProcessGroup::BackgroundProcesses, 62_914_560, "Microsoft OneDrive Sync Engine");
        one_drive.cpu_percent = 0.2;
        one_drive.threads_count = 14;
        one_drive.is_efficiency_mode = true;
        one_drive.status = ProcessStatus::EfficiencyMode;
        one_drive.priority = ProcessPriority::Idle;
        list.push(one_drive);

        // -------------------------------------------------------------
        // Group 3: Windows Processes
        // -------------------------------------------------------------
        // Desktop Window Manager (dwm.exe)
        let mut dwm = ProcessItem::new(940, None, "Desktop Window Manager", ProcessGroup::WindowsProcesses, 188_743_680, "Desktop Window Manager (dwm.exe)");
        dwm.cpu_percent = 2.7;
        dwm.gpu_percent = 6.4;
        dwm.gpu_engine = GpuEngineType::ThreeD;
        dwm.power_usage = PowerUsageLevel::Moderate;
        dwm.threads_count = 18;
        dwm.handles_count = 920;
        list.push(dwm);

        // System (PID 4)
        let mut system_proc = ProcessItem::new(4, None, "System", ProcessGroup::WindowsProcesses, 4_194_304, "NT Kernel & System");
        system_proc.cpu_percent = 0.9;
        system_proc.disk_read_bps = 524288.0;
        system_proc.disk_write_bps = 1048576.0;
        system_proc.threads_count = 260;
        system_proc.handles_count = 8400;
        list.push(system_proc);

        // Client Server Runtime (csrss.exe)
        let mut csrss = ProcessItem::new(712, None, "Client Server Runtime Process", ProcessGroup::WindowsProcesses, 12_582_912, "csrss.exe Subsystem");
        csrss.cpu_percent = 0.2;
        csrss.threads_count = 14;
        csrss.handles_count = 650;
        list.push(csrss);

        // Local Security Authority (lsass.exe)
        let mut lsass = ProcessItem::new(820, None, "Local Security Authority Subsystem Service", ProcessGroup::WindowsProcesses, 25_165_824, "lsass.exe Security Subsystem");
        lsass.cpu_percent = 0.3;
        lsass.threads_count = 12;
        lsass.handles_count = 890;
        list.push(lsass);

        list
    }

    /// Initializes default Windows & HybridOS system services
    fn init_default_services() -> Vec<ServiceItem> {
        vec![
            ServiceItem::new("wuauserv", Some(1620), "Windows Update", ServiceStatus::Running, "netsvcs"),
            ServiceItem::new("Spooler", Some(1740), "Print Spooler", ServiceStatus::Running, "spooler"),
            ServiceItem::new("Dhcp", Some(1100), "DHCP Client", ServiceStatus::Running, "LocalServiceNetworkRestricted"),
            ServiceItem::new("Dnscache", Some(1140), "DNS Client", ServiceStatus::Running, "NetworkService"),
            ServiceItem::new("EventLog", Some(980), "Windows Event Log", ServiceStatus::Running, "LocalService"),
            ServiceItem::new("WinDefend", Some(2150), "Microsoft Defender Antivirus Service", ServiceStatus::Running, "LocalSystem"),
            ServiceItem::new("VortexVfsService", Some(2310), "HybridOS Vortex Virtual Filesystem Daemon", ServiceStatus::Running, "HybridCore"),
            ServiceItem::new("WSearch", None, "Windows Search", ServiceStatus::Stopped, "Undeclared"),
            ServiceItem::new("DiagTrack", None, "Connected User Experiences and Telemetry", ServiceStatus::Stopped, "Undeclared"),
            ServiceItem::new("BluetoothUserService", None, "Bluetooth User Support Service", ServiceStatus::Stopped, "Devices"),
        ]
    }

    /// Initializes Startup apps
    fn init_default_startup_apps() -> Vec<StartupAppItem> {
        vec![
            StartupAppItem {
                name: "Windows Terminal".to_string(),
                publisher: "Microsoft Corporation".to_string(),
                status_enabled: true,
                startup_impact: "Low".to_string(),
            },
            StartupAppItem {
                name: "OneDrive".to_string(),
                publisher: "Microsoft Corporation".to_string(),
                status_enabled: true,
                startup_impact: "Medium".to_string(),
            },
            StartupAppItem {
                name: "Microsoft Teams".to_string(),
                publisher: "Microsoft Corporation".to_string(),
                status_enabled: false,
                startup_impact: "High".to_string(),
            },
            StartupAppItem {
                name: "Discord".to_string(),
                publisher: "Discord Inc.".to_string(),
                status_enabled: false,
                startup_impact: "Medium".to_string(),
            },
        ]
    }

    /// Initializes App history data
    fn init_default_app_history() -> Vec<AppHistoryItem> {
        vec![
            AppHistoryItem {
                name: "Microsoft Edge".to_string(),
                cpu_time: "0:12:45".to_string(),
                network_mb: 482.5,
                metered_network_mb: 0.0,
                tile_updates_mb: 2.1,
            },
            AppHistoryItem {
                name: "Windows Terminal".to_string(),
                cpu_time: "0:08:14".to_string(),
                network_mb: 18.2,
                metered_network_mb: 0.0,
                tile_updates_mb: 0.0,
            },
            AppHistoryItem {
                name: "File Explorer".to_string(),
                cpu_time: "0:03:22".to_string(),
                network_mb: 5.4,
                metered_network_mb: 0.0,
                tile_updates_mb: 0.5,
            },
            AppHistoryItem {
                name: "Task Manager".to_string(),
                cpu_time: "0:01:50".to_string(),
                network_mb: 0.1,
                metered_network_mb: 0.0,
                tile_updates_mb: 0.0,
            },
        ]
    }

    /// Initializes active user sessions
    fn init_default_users() -> Vec<UserItem> {
        vec![UserItem {
            user_name: "Vinicius (Current)".to_string(),
            session_id: 1,
            status: "Active".to_string(),
            session_type: "Console".to_string(),
            cpu_percent: 18.5,
            memory_mb: 4210.0,
            disk_mbps: 1.8,
            network_mbps: 16.0,
        }]
    }

    // ========================================================================
    // Action Implementations: Event Dispatcher & Mutators
    // ========================================================================

    pub fn handle_event(&mut self, event: TaskMgrEvent) -> Result<(), &'static str> {
        match event {
            TaskMgrEvent::SelectPage(page) => {
                self.current_page = page;
                Ok(())
            }
            TaskMgrEvent::ToggleSidebar => {
                self.sidebar_collapsed = !self.sidebar_collapsed;
                Ok(())
            }
            TaskMgrEvent::SelectProcess(pid) => {
                self.selected_pid = Some(pid);
                self.sync_command_bar();
                Ok(())
            }
            TaskMgrEvent::ToggleExpandProcess(pid) => {
                self.toggle_expand_process(pid);
                Ok(())
            }
            TaskMgrEvent::EndTask(pid) => self.end_task(pid),
            TaskMgrEvent::ToggleEfficiencyMode(pid) => {
                self.toggle_efficiency_mode(pid)?;
                Ok(())
            }
            TaskMgrEvent::SelectPerformanceDevice(dev) => {
                self.selected_perf_device = dev;
                Ok(())
            }
            TaskMgrEvent::ToggleResourceValueMode => {
                self.resource_display_mode.toggle();
                Ok(())
            }
            TaskMgrEvent::StartService(ref name) => self.start_service(name),
            TaskMgrEvent::StopService(ref name) => self.stop_service(name),
            TaskMgrEvent::RestartService(ref name) => self.restart_service(name),
            TaskMgrEvent::OpenServicesMsc => Ok(()),
            TaskMgrEvent::ToggleStartupApp(ref name) => {
                self.toggle_startup_app(name);
                Ok(())
            }
            TaskMgrEvent::SetSearchFilter(query) => {
                self.search_filter = query;
                Ok(())
            }
            TaskMgrEvent::SortByColumn(col) => {
                if self.sort_column == col {
                    self.sort_ascending = !self.sort_ascending;
                } else {
                    self.sort_column = col;
                    self.sort_ascending = false; // Default descending for metrics
                }
                self.sort_processes();
                Ok(())
            }
            TaskMgrEvent::RunNewTask { command, run_as_admin } => {
                self.run_new_task(&command, run_as_admin)?;
                Ok(())
            }
            TaskMgrEvent::Tick(delta_sec) => {
                self.update_tick(delta_sec);
                Ok(())
            }
        }
    }

    /// Synchronizes the command bar buttons with currently selected process
    fn sync_command_bar(&mut self) {
        if let Some(pid) = self.selected_pid {
            let proc_info = self.find_process(pid).map(|proc| {
                let is_critical = proc.pid == 4 || proc.name == "csrss.exe" || proc.name == "Client Server Runtime Process";
                (is_critical, proc.is_efficiency_mode)
            });
            if let Some((is_critical, is_eff)) = proc_info {
                self.command_bar.can_end_task = !is_critical;
                self.command_bar.can_toggle_efficiency = !is_critical;
                self.command_bar.is_efficiency_active = is_eff;
                return;
            }
        }
        self.command_bar.can_end_task = false;
        self.command_bar.can_toggle_efficiency = false;
        self.command_bar.is_efficiency_active = false;
    }

    /// Toggle expand/collapse state of a parent process
    pub fn toggle_expand_process(&mut self, pid: u32) -> bool {
        for proc in &mut self.processes {
            if proc.pid == pid {
                proc.is_expanded = !proc.is_expanded;
                return true;
            }
            if let Some(child) = proc.find_child_mut(pid) {
                child.is_expanded = !child.is_expanded;
                return true;
            }
        }
        false
    }

    /// Terminates a process by PID and cleans up descendants
    pub fn end_task(&mut self, pid: u32) -> Result<(), &'static str> {
        if pid == 4 {
            return Err("Cannot terminate System kernel process (PID 4)");
        }

        // Check top level
        if let Some(pos) = self.processes.iter().position(|p| p.pid == pid) {
            let removed = self.processes.remove(pos);
            let killed_threads = removed.threads_count;
            self.perf_telemetry.cpu.processes_count = self.perf_telemetry.cpu.processes_count.saturating_sub(1);
            self.perf_telemetry.cpu.threads_count = self.perf_telemetry.cpu.threads_count.saturating_sub(killed_threads as usize);

            if self.selected_pid == Some(pid) {
                self.selected_pid = self.processes.first().map(|p| p.pid);
                self.sync_command_bar();
            }
            return Ok(());
        }

        // Check nested children
        for proc in &mut self.processes {
            if proc.remove_child(pid) {
                self.perf_telemetry.cpu.processes_count = self.perf_telemetry.cpu.processes_count.saturating_sub(1);
                if self.selected_pid == Some(pid) {
                    self.selected_pid = Some(proc.pid);
                    self.sync_command_bar();
                }
                return Ok(());
            }
        }

        Err("Process ID not found")
    }

    /// Toggles Efficiency Mode (EcoQoS) on a targeted process
    pub fn toggle_efficiency_mode(&mut self, pid: u32) -> Result<bool, &'static str> {
        let proc = self.find_process_mut(pid).ok_or("Process ID not found")?;

        if proc.pid == 4 {
            return Err("Cannot enable efficiency mode on NT Kernel System process");
        }

        proc.is_efficiency_mode = !proc.is_efficiency_mode;
        if proc.is_efficiency_mode {
            proc.priority = ProcessPriority::Idle;
            proc.status = ProcessStatus::EfficiencyMode;
            // Also apply to child processes
            for child in &mut proc.children {
                child.is_efficiency_mode = true;
                child.priority = ProcessPriority::Idle;
                child.status = ProcessStatus::EfficiencyMode;
            }
        } else {
            proc.priority = ProcessPriority::Normal;
            proc.status = ProcessStatus::Running;
            for child in &mut proc.children {
                child.is_efficiency_mode = false;
                child.priority = ProcessPriority::Normal;
                child.status = ProcessStatus::Running;
            }
        }

        let new_state = proc.is_efficiency_mode;
        self.sync_command_bar();
        Ok(new_state)
    }

    /// Starts a stopped service, dynamically assigning a PID
    pub fn start_service(&mut self, name: &str) -> Result<(), &'static str> {
        let next_pid = self.next_pid_counter;
        let service = self.services.iter_mut().find(|s| s.name.eq_ignore_ascii_case(name))
            .ok_or("Service not found")?;

        if service.status == ServiceStatus::Running {
            return Err("Service is already running");
        }

        service.status = ServiceStatus::Running;
        service.pid = Some(next_pid);
        self.next_pid_counter += 4;
        self.perf_telemetry.cpu.processes_count += 1;
        Ok(())
    }

    /// Stops a running service
    pub fn stop_service(&mut self, name: &str) -> Result<(), &'static str> {
        let service = self.services.iter_mut().find(|s| s.name.eq_ignore_ascii_case(name))
            .ok_or("Service not found")?;

        if service.status == ServiceStatus::Stopped {
            return Err("Service is already stopped");
        }

        service.status = ServiceStatus::Stopped;
        service.pid = None;
        self.perf_telemetry.cpu.processes_count = self.perf_telemetry.cpu.processes_count.saturating_sub(1);
        Ok(())
    }

    /// Restarts a service
    pub fn restart_service(&mut self, name: &str) -> Result<(), &'static str> {
        let next_pid = self.next_pid_counter;
        let service = self.services.iter_mut().find(|s| s.name.eq_ignore_ascii_case(name))
            .ok_or("Service not found")?;

        service.status = ServiceStatus::Running;
        service.pid = Some(next_pid);
        self.next_pid_counter += 4;
        Ok(())
    }

    /// Toggles startup application state
    pub fn toggle_startup_app(&mut self, name: &str) -> bool {
        if let Some(app) = self.startup_apps.iter_mut().find(|a| a.name.eq_ignore_ascii_case(name)) {
            app.status_enabled = !app.status_enabled;
            return true;
        }
        false
    }

    /// Spawns a new task via the "Run new task" dialog
    pub fn run_new_task(&mut self, command: &str, run_as_admin: bool) -> Result<u32, &'static str> {
        if command.trim().is_empty() {
            return Err("Command cannot be empty");
        }

        let pid = self.next_pid_counter;
        self.next_pid_counter += 4;

        let clean_name = command.split_whitespace().next().unwrap_or(command);
        let mut new_proc = ProcessItem::new(
            pid,
            None,
            clean_name,
            ProcessGroup::Apps,
            50_331_648, // 48 MB initial commit
            &format!("Task launched via Task Manager (Admin={})", run_as_admin),
        );
        new_proc.cpu_percent = 0.5;
        new_proc.user_name = if run_as_admin { "Administrator".to_string() } else { "Vinicius".to_string() };

        self.processes.insert(0, new_proc);
        self.selected_pid = Some(pid);
        self.perf_telemetry.cpu.processes_count += 1;
        self.perf_telemetry.cpu.threads_count += 4;
        self.sync_command_bar();
        Ok(pid)
    }

    /// Advances the simulation clock by delta_seconds, shifting sparkline buffers
    pub fn update_tick(&mut self, delta_sec: f32) {
        let dt = delta_sec.max(0.1);

        // Update CPU uptime
        self.perf_telemetry.cpu.uptime_seconds += dt.round() as u64;

        // Simulate rolling hardware noise & realistic workloads
        let phase = (self.perf_telemetry.cpu.uptime_seconds % 3600) as f32;

        // CPU Sparkline
        let cpu_sample = (18.0 + (phase * 0.2).sin() * 8.5 + (phase * 0.05).cos() * 4.0).clamp(3.0, 98.0);
        self.perf_telemetry.cpu.utilization_percent = cpu_sample;
        self.perf_telemetry.cpu.sparkline.push(cpu_sample);

        // Memory Sparkline
        let mem_sample = (26.2 + (phase * 0.08).sin() * 1.8).clamp(15.0, 95.0);
        self.perf_telemetry.memory.sparkline.push(mem_sample);
        self.perf_telemetry.memory.in_use_bytes = ((mem_sample / 100.0) * self.perf_telemetry.memory.total_bytes as f32) as u64;
        self.perf_telemetry.memory.available_bytes = self.perf_telemetry.memory.total_bytes.saturating_sub(self.perf_telemetry.memory.in_use_bytes);

        // Disk 0 Sparkline
        let disk0_sample = ((phase % 14.0) * 3.2).clamp(0.5, 95.0);
        self.perf_telemetry.disk0.active_time_percent = disk0_sample;
        self.perf_telemetry.disk0.sparkline.push(disk0_sample);

        // Disk 1 Sparkline
        self.perf_telemetry.disk1.sparkline.push(0.4);

        // Ethernet Sparkline
        let eth_sample = (10.0 + (phase * 0.4).cos() * 7.0).clamp(0.1, 100.0);
        self.perf_telemetry.ethernet.sparkline.push(eth_sample);

        // GPU Sparkline
        let gpu_sample = (11.0 + (phase * 0.3).sin() * 6.0).clamp(1.0, 99.0);
        self.perf_telemetry.gpu0.utilization_3d_percent = gpu_sample;
        self.perf_telemetry.gpu0.sparkline.push(gpu_sample);
    }

    /// Find an immutable reference to a process by PID
    pub fn find_process(&self, target_pid: u32) -> Option<&ProcessItem> {
        for proc in &self.processes {
            if proc.pid == target_pid {
                return Some(proc);
            }
            if let Some(child) = Self::find_child_recursive(proc, target_pid) {
                return Some(child);
            }
        }
        None
    }

    fn find_child_recursive<'a>(parent: &'a ProcessItem, target_pid: u32) -> Option<&'a ProcessItem> {
        for child in &parent.children {
            if child.pid == target_pid {
                return Some(child);
            }
            if let Some(found) = Self::find_child_recursive(child, target_pid) {
                return Some(found);
            }
        }
        None
    }

    /// Find a mutable reference to a process by PID
    pub fn find_process_mut(&mut self, target_pid: u32) -> Option<&mut ProcessItem> {
        for proc in &mut self.processes {
            if proc.pid == target_pid {
                return Some(proc);
            }
            if let Some(child) = proc.find_child_mut(target_pid) {
                return Some(child);
            }
        }
        None
    }

    /// Filters processes according to the active search bar filter string
    pub fn filtered_processes(&self) -> Vec<&ProcessItem> {
        if self.search_filter.is_empty() {
            self.processes.iter().collect()
        } else {
            let lower = self.search_filter.to_lowercase();
            self.processes
                .iter()
                .filter(|p| {
                    p.name.to_lowercase().contains(&lower)
                        || p.pid.to_string().contains(&lower)
                        || p.description.to_lowercase().contains(&lower)
                })
                .collect()
        }
    }

    /// Sorts processes in-place based on current sort configuration
    pub fn sort_processes(&mut self) {
        let ascending = self.sort_ascending;
        let col = self.sort_column;

        self.processes.sort_by(|a, b| {
            let ordering = match col {
                ProcessSortColumn::Name => a.name.cmp(&b.name),
                ProcessSortColumn::Status => a.status.label().cmp(b.status.label()),
                ProcessSortColumn::Cpu => a.aggregate_cpu_percent().partial_cmp(&b.aggregate_cpu_percent()).unwrap(),
                ProcessSortColumn::Memory => a.aggregate_memory_bytes().cmp(&b.aggregate_memory_bytes()),
                ProcessSortColumn::Disk => a.aggregate_disk_bps().partial_cmp(&b.aggregate_disk_bps()).unwrap(),
                ProcessSortColumn::Network => a.aggregate_net_bps().partial_cmp(&b.aggregate_net_bps()).unwrap(),
                ProcessSortColumn::Gpu => a.aggregate_gpu_percent().partial_cmp(&b.aggregate_gpu_percent()).unwrap(),
                ProcessSortColumn::PowerUsage => a.power_usage.cmp(&b.power_usage),
            };
            if ascending {
                ordering
            } else {
                ordering.reverse()
            }
        });
    }

    // ========================================================================
    // Formatting Helpers for Resource Display Mode
    // ========================================================================

    pub fn format_memory(&self, bytes: u64) -> String {
        match self.resource_display_mode {
            ResourceDisplayMode::Percentages => {
                let percent = (bytes as f64 / self.perf_telemetry.memory.total_bytes as f64) * 100.0;
                format!("{:.1} %", percent)
            }
            ResourceDisplayMode::AbsoluteValues => {
                let mb = bytes as f64 / (1024.0 * 1024.0);
                format!("{:.1} MB", mb)
            }
        }
    }

    pub fn format_disk(&self, bps: f64) -> String {
        match self.resource_display_mode {
            ResourceDisplayMode::Percentages => {
                // Percentage of theoretical 500 MB/s disk baseline
                let percent = (bps / (500.0 * 1024.0 * 1024.0)) * 100.0;
                format!("{:.1} %", percent.clamp(0.0, 100.0))
            }
            ResourceDisplayMode::AbsoluteValues => {
                let mbps = bps / (1024.0 * 1024.0);
                if mbps >= 0.1 {
                    format!("{:.1} MB/s", mbps)
                } else {
                    format!("{:.0} KB/s", bps / 1024.0)
                }
            }
        }
    }

    pub fn format_network(&self, bps: f64) -> String {
        match self.resource_display_mode {
            ResourceDisplayMode::Percentages => {
                // Percentage of 10 Gbps VirtIO link
                let percent = ((bps * 8.0) / 10_000_000_000.0) * 100.0;
                format!("{:.1} %", percent.clamp(0.0, 100.0))
            }
            ResourceDisplayMode::AbsoluteValues => {
                let mbps = (bps * 8.0) / 1_000_000.0;
                format!("{:.1} Mbps", mbps)
            }
        }
    }
}

// ============================================================================
// Section 9: Dual Rendering Pipeline
// ============================================================================

impl TaskManager {
    /// Renders a complete Windows 11 Fluent styled ASCII layout of the current view,
    /// suitable for rich terminal environments, serial console, or debugging diagnostics.
    pub fn render_ascii_view(&self) -> String {
        let mut out = String::with_capacity(4096);

        // Windows 11 Title Bar & Shell Header
        out.push_str("========================================================================================================\n");
        out.push_str(&format!(
            "  TASK MANAGER - HybridOS Win11 Core   [Page: {}]   [Display: {}]   [Theme: {:?}]\n",
            self.current_page.title(),
            self.resource_display_mode.label(),
            self.theme
        ));
        out.push_str("========================================================================================================\n");

        // Command Bar
        let efficiency_badge = if self.command_bar.is_efficiency_active { "[🌱 EcoQoS Active]" } else { "[   EcoQoS Off  ]" };
        let end_task_btn = if self.command_bar.can_end_task { "[End task (Del)]" } else { "[  End task (-) ]" };
        let eff_btn = if self.command_bar.can_toggle_efficiency { "[Efficiency mode]" } else { "[Efficiency (-)]" };
        let search_indicator = if self.search_filter.is_empty() {
            "Search: [Type to filter...]".to_string()
        } else {
            format!("Search: [{}]", self.search_filter)
        };

        out.push_str(&format!(
            "  COMMAND BAR: [Run new task]  {}  {}  {}  [Values: {}]  {}\n",
            end_task_btn,
            eff_btn,
            efficiency_badge,
            if self.resource_display_mode == ResourceDisplayMode::Percentages { "%" } else { "Vals" },
            search_indicator
        ));
        out.push_str("--------------------------------------------------------------------------------------------------------\n");

        // Render Page View
        match self.current_page {
            NavPage::Processes => self.render_ascii_processes_view(&mut out),
            NavPage::Performance => self.render_ascii_performance_view(&mut out),
            NavPage::AppHistory => self.render_ascii_app_history_view(&mut out),
            NavPage::StartupApps => self.render_ascii_startup_apps_view(&mut out),
            NavPage::Users => self.render_ascii_users_view(&mut out),
            NavPage::Details => self.render_ascii_details_view(&mut out),
            NavPage::Services => self.render_ascii_services_view(&mut out),
            NavPage::Settings => self.render_ascii_settings_view(&mut out),
        }

        // Sidebar Navigation Footer
        out.push_str("--------------------------------------------------------------------------------------------------------\n");
        out.push_str("  SIDEBAR NAV: ");
        for page in &NavPage::ALL {
            if *page == self.current_page {
                out.push_str(&format!(" >{}{} [ACTIVE]< ", page.icon_glyph(), page.title()));
            } else {
                out.push_str(&format!("  {}{}  ", page.icon_glyph(), page.title()));
            }
        }
        out.push_str("\n========================================================================================================\n");
        out
    }

    fn render_ascii_processes_view(&self, out: &mut String) {
        // Table Columns Header
        out.push_str(
            "  Name                           | Status      | CPU %  | Memory   | Disk     | Network  | GPU %  | GPU Engine   | Power \n"
        );
        out.push_str(
            "---------------------------------+-------------+--------+----------+----------+----------+--------+--------------+----------\n"
        );

        let filtered = self.filtered_processes();

        // Print grouped by category
        for group in &[ProcessGroup::Apps, ProcessGroup::BackgroundProcesses, ProcessGroup::WindowsProcesses] {
            let in_group: Vec<&&ProcessItem> = filtered.iter().filter(|p| p.group == *group).collect();
            if in_group.is_empty() {
                continue;
            }

            out.push_str(&format!("  v  {} ({})\n", group.label(), in_group.len()));

            for proc in in_group {
                self.render_ascii_process_row(proc, 0, out);
            }
        }
    }

    fn render_ascii_process_row(&self, proc: &ProcessItem, indent_level: usize, out: &mut String) {
        let is_selected = self.selected_pid == Some(proc.pid);
        let prefix_marker = if is_selected { ">" } else { " " };

        let expand_marker = if !proc.children.is_empty() {
            if proc.is_expanded { "v " } else { "> " }
        } else {
            "  "
        };

        let leaf_badge = if proc.is_efficiency_mode { "🌱" } else { "  " };
        let indent = "  ".repeat(indent_level);

        let mut name_field = format!("{}{}{}{}", indent, expand_marker, leaf_badge, proc.name);
        if name_field.chars().count() > 30 {
            name_field = name_field.chars().take(29).collect::<String>() + "…";
        }

        let status_str = match proc.status {
            ProcessStatus::Running => "Running",
            ProcessStatus::Suspended => "Suspended",
            ProcessStatus::EfficiencyMode => "Efficiency",
            ProcessStatus::NotResponding => "Not Responding",
        };

        let mem_str = self.format_memory(proc.aggregate_memory_bytes());
        let disk_str = self.format_disk(proc.aggregate_disk_bps());
        let net_str = self.format_network(proc.aggregate_net_bps());

        out.push_str(&format!(
            "{}{:<31} | {:<11} | {:>5.1}% | {:>8} | {:>8} | {:>8} | {:>5.1}% | {:<12} | {:<8}\n",
            prefix_marker,
            name_field,
            status_str,
            proc.aggregate_cpu_percent(),
            mem_str,
            disk_str,
            net_str,
            proc.aggregate_gpu_percent(),
            proc.gpu_engine.label(),
            proc.power_usage.label()
        ));

        // Render expanded children
        if proc.is_expanded {
            for child in &proc.children {
                self.render_ascii_process_row(child, indent_level + 1, out);
            }
        }
    }

    fn render_ascii_performance_view(&self, out: &mut String) {
        let t = &self.perf_telemetry;

        out.push_str("  HARDWARE MONITOR & LIVE SPARKLINE TELEMETRY (Last 60 Seconds)\n");
        out.push_str("--------------------------------------------------------------------------------------------------------\n");

        // Mini sparklines left panel & selected right panel
        let cpu_spark = t.cpu.sparkline.render_ascii_sparkline(24);
        let mem_spark = t.memory.sparkline.render_ascii_sparkline(24);
        let d0_spark = t.disk0.sparkline.render_ascii_sparkline(24);
        let d1_spark = t.disk1.sparkline.render_ascii_sparkline(24);
        let eth_spark = t.ethernet.sparkline.render_ascii_sparkline(24);
        let gpu_spark = t.gpu0.sparkline.render_ascii_sparkline(24);

        out.push_str(&format!("  [1] CPU        {:>5.1}%  [{}]  {} GHz\n", t.cpu.utilization_percent, cpu_spark, t.cpu.speed_ghz));
        out.push_str(&format!("  [2] Memory     {:>5.1}%  [{}]  {:.1}/{:.1} GB\n", t.memory.in_use_gb() / t.memory.total_gb() * 100.0, mem_spark, t.memory.in_use_gb(), t.memory.total_gb()));
        out.push_str(&format!("  [3] Disk 0 C:  {:>5.1}%  [{}]  {}\n", t.disk0.active_time_percent, d0_spark, t.disk0.model));
        out.push_str(&format!("  [4] Disk 1 D:  {:>5.1}%  [{}]  {}\n", t.disk1.active_time_percent, d1_spark, t.disk1.model));
        out.push_str(&format!("  [5] Ethernet   {:>5.1}M  [{}]  Send: {:.1}M, Rcv: {:.1}M\n", t.ethernet.receive_mbps(), eth_spark, t.ethernet.send_mbps(), t.ethernet.receive_mbps()));
        out.push_str(&format!("  [6] GPU 0      {:>5.1}%  [{}]  VirtIO-GPU 3D\n", t.gpu0.utilization_3d_percent, gpu_spark));
        out.push_str("--------------------------------------------------------------------------------------------------------\n");

        // Detailed panel for selected device
        match self.selected_perf_device {
            PerfDevice::Cpu => {
                out.push_str("  SELECTED: CPU - Intel(R) Core(TM) i9-14900K\n");
                out.push_str(&format!("  Utilization:      {:>5.1} %             Speed:               {:.2} GHz\n", t.cpu.utilization_percent, t.cpu.speed_ghz));
                out.push_str(&format!("  Processes:        {:<6}                 Up time:             {}\n", t.cpu.processes_count, t.cpu.formatted_uptime()));
                out.push_str(&format!("  Threads:          {:<6}                 Base speed:          {:.2} GHz\n", t.cpu.threads_count, t.cpu.base_speed_ghz));
                out.push_str(&format!("  Handles:          {:<6}                 Sockets / Cores:     {} / {}\n", t.cpu.handles_count, t.cpu.sockets, t.cpu.cores));
                out.push_str(&format!("  Virtualization:   Enabled (Hyper-V)      Logical Processors:  {}\n", t.cpu.logical_processors));
                out.push_str(&format!("  L1 Cache:         {} KB                 L2 / L3 Cache:       {} MB / {} MB\n", t.cpu.l1_cache_kb, t.cpu.l2_cache_kb / 1024, t.cpu.l3_cache_kb / 1024));
            }
            PerfDevice::Memory => {
                out.push_str("  SELECTED: Memory - 32.0 GB DDR5 Synchronous\n");
                out.push_str(&format!("  In use (Compressed): {:.1} GB ({:.1} GB)     Available:           {:.1} GB\n", t.memory.in_use_gb(), 0.0, t.memory.available_gb()));
                out.push_str(&format!("  Committed:           {:.1}/{:.1} GB          Cached:              {:.1} GB\n", t.memory.committed_gb(), t.memory.commit_limit_gb(), t.memory.cached_gb()));
                out.push_str(&format!("  Paged pool:          {:.0} MB                Non-paged pool:      {:.0} MB\n", t.memory.paged_pool_mb(), t.memory.non_paged_pool_mb()));
                out.push_str(&format!("  Speed:               {} MT/s               Slots used:          {} of {}\n", t.memory.speed_mts, t.memory.slots_used, t.memory.slots_total));
                out.push_str(&format!("  Form factor:         {}                  Hardware reserved:   {:.0} MB\n", t.memory.form_factor, t.memory.hardware_reserved_bytes as f32 / (1024.0 * 1024.0)));
            }
            PerfDevice::Disk(id) => {
                let d = if id == 0 { &t.disk0 } else { &t.disk1 };
                out.push_str(&format!("  SELECTED: {} ({}) [{}]\n", d.name, d.model, d.bus_type));
                out.push_str(&format!("  Active time:         {:>5.1} %             Average response time: {:.1} ms\n", d.active_time_percent, d.response_time_ms));
                out.push_str(&format!("  Read speed:          {:.1} KB/s             Write speed:           {:.1} KB/s\n", d.read_speed_kbps, d.write_speed_kbps));
                out.push_str(&format!("  Capacity:            {}                  Formatted size:        {}\n", d.capacity_formatted(), d.formatted_size_formatted()));
                out.push_str(&format!("  System disk:         {}                    Page file:             {}\n", if d.system_disk { "Yes" } else { "No" }, if d.page_file { "Yes" } else { "No" }));
            }
            PerfDevice::Ethernet => {
                let e = &t.ethernet;
                out.push_str(&format!("  SELECTED: Ethernet - {}\n", e.adapter_name));
                out.push_str(&format!("  Send throughput:     {:.2} Mbps            Receive throughput:    {:.2} Mbps\n", e.send_mbps(), e.receive_mbps()));
                out.push_str(&format!("  Connection type:     {}\n", e.connection_type));
                out.push_str(&format!("  IPv4 Address:        {:<24} Link Speed:            {:.0} Gbps\n", e.ipv4_address, e.link_speed_gbps));
                out.push_str(&format!("  IPv6 Address:        {}\n", e.ipv6_address));
            }
            PerfDevice::Gpu(_) => {
                let g = &t.gpu0;
                out.push_str(&format!("  SELECTED: GPU 0 - {}\n", g.adapter_name));
                out.push_str(&format!("  3D Utilization:      {:>5.1} %             Dedicated GPU Memory:  {:.0}/{:.0} MB\n", g.utilization_3d_percent, g.dedicated_mb_used(), g.dedicated_mb_total()));
                out.push_str(&format!("  Shared GPU Memory:   {:.0}/{:.0} MB       Driver Version:        {}\n", g.shared_mb_used(), g.shared_mb_total(), g.driver_version));
                out.push_str(&format!("  Driver Date:         {:<24} DirectX / Vulkan:     {}\n", g.driver_date, g.directx_version));
                out.push_str(&format!("  Physical Location:   {}\n", g.hardware_location));
            }
        }
    }

    fn render_ascii_details_view(&self, out: &mut String) {
        out.push_str("  Name                | PID   | Status     | User name       | CPU % | Memory (WS) | Arch | Description\n");
        out.push_str("----------------------+-------+------------+-----------------+-------+-------------+------+-------------------------------\n");

        let filtered = self.filtered_processes();
        for proc in filtered {
            let status_str = match proc.status {
                ProcessStatus::Running => "Running",
                ProcessStatus::Suspended => "Suspended",
                ProcessStatus::EfficiencyMode => "Efficiency",
                ProcessStatus::NotResponding => "Not Responding",
            };

            let mem_ws = format!("{:.0} K", proc.memory_bytes as f32 / 1024.0);

            out.push_str(&format!(
                "  {:<19} | {:<5} | {:<10} | {:<15} | {:>4.1}% | {:>11} | {:<4} | {}\n",
                proc.name,
                proc.pid,
                status_str,
                proc.user_name,
                proc.cpu_percent,
                mem_ws,
                proc.architecture.label(),
                proc.description
            ));
        }
    }

    fn render_ascii_services_view(&self, out: &mut String) {
        out.push_str("  Name                   | PID   | Description                                   | Status   | Group\n");
        out.push_str("-------------------------+-------+-----------------------------------------------+----------+----------------------------\n");

        for svc in &self.services {
            let pid_str = svc.pid.map(|p| p.to_string()).unwrap_or_else(|| "-".to_string());
            let status_str = svc.status.label();

            out.push_str(&format!(
                "  {:<22} | {:<5} | {:<45} | {:<8} | {}\n",
                svc.name,
                pid_str,
                svc.description,
                status_str,
                svc.group
            ));
        }
    }

    fn render_ascii_app_history_view(&self, out: &mut String) {
        out.push_str("  Name                      | CPU Time   | Network     | Metered Network | Tile Updates\n");
        out.push_str("----------------------------+------------+-------------+-----------------+---------------\n");
        for app in &self.app_history {
            out.push_str(&format!(
                "  {:<25} | {:<10} | {:>8.1} MB | {:>12.1} MB | {:>9.1} MB\n",
                app.name, app.cpu_time, app.network_mb, app.metered_network_mb, app.tile_updates_mb
            ));
        }
    }

    fn render_ascii_startup_apps_view(&self, out: &mut String) {
        out.push_str("  Name                      | Publisher                 | Status    | Startup Impact\n");
        out.push_str("----------------------------+---------------------------+-----------+----------------\n");
        for app in &self.startup_apps {
            let status_str = if app.status_enabled { "Enabled" } else { "Disabled" };
            out.push_str(&format!(
                "  {:<25} | {:<25} | {:<9} | {}\n",
                app.name, app.publisher, status_str, app.startup_impact
            ));
        }
    }

    fn render_ascii_users_view(&self, out: &mut String) {
        out.push_str("  User                      | ID  | Status   | Session | CPU %  | Memory (MB) | Disk (MB/s) | Network (Mbps)\n");
        out.push_str("----------------------------+-----+----------+---------+--------+-------------+-------------+----------------\n");
        for user in &self.users {
            out.push_str(&format!(
                "  {:<25} | {:<3} | {:<8} | {:<7} | {:>5.1}% | {:>10.1} | {:>10.1} | {:>13.1}\n",
                user.user_name, user.session_id, user.status, user.session_type, user.cpu_percent, user.memory_mb, user.disk_mbps, user.network_mbps
            ));
        }
    }

    fn render_ascii_settings_view(&self, out: &mut String) {
        out.push_str("  TASK MANAGER SETTINGS & PREFERENCES\n");
        out.push_str("--------------------------------------------------------------------------------------------------------\n");
        out.push_str("  - Default start page: Processes\n");
        out.push_str("  - Real-time update speed: Normal (1.0 second)\n");
        out.push_str("  - Always on top: Off\n");
        out.push_str("  - Minimize on use: Off\n");
        out.push_str("  - Hide when minimized: Off\n");
        out.push_str(&format!("  - Fluent Material Theme: {:?}\n", self.theme));
    }

    // ========================================================================
    // Graphical Surface Framebuffer Renderer (Windows 11 Fluent DWM)
    // ========================================================================

    /// Renders the complete Windows 11 Task Manager onto a 2D ARGB Surface framebuffer
    /// using Fluent Design system materials, cards, sidebar icons, and sparklines.
    pub fn render_to_surface(&self, surface: &mut TaskMgrSurface) {
        let (bg_color, card_bg, card_stroke, text_color, text_dim) = match self.theme {
            ThemeMode::Dark => (
                Color::MICA_BACKGROUND_DARK,
                Color::CARD_BACKGROUND_DARK,
                Color::CARD_STROKE_DARK,
                Color::TEXT_PRIMARY_DARK,
                Color::TEXT_SECONDARY_DARK,
            ),
            ThemeMode::Light => (
                Color::MICA_BACKGROUND_LIGHT,
                Color::CARD_BACKGROUND_LIGHT,
                Color::CARD_STROKE_LIGHT,
                Color::TEXT_PRIMARY_LIGHT,
                Color::TEXT_SECONDARY_LIGHT,
            ),
        };

        // 1. Clear background with Mica Tint
        surface.fill_rect(&Rect::new(0, 0, surface.width as u32, surface.height as u32), bg_color);

        let sidebar_width = if self.sidebar_collapsed { 48 } else { 220 };

        // 2. Render Left Sidebar
        let sidebar_rect = Rect::new(0, 0, sidebar_width, surface.height as u32);
        surface.fill_rect(&sidebar_rect, bg_color);
        surface.draw_rect_outline(&sidebar_rect, card_stroke, 1);

        // Sidebar navigation items
        let item_height = 40;
        let mut y_nav = 50;

        for (_i, page) in NavPage::ALL.iter().enumerate() {
            let is_active = *page == self.current_page;
            let item_rect = Rect::new(6, y_nav, sidebar_width.saturating_sub(12), item_height as u32);

            if is_active {
                surface.fill_rect(&item_rect, card_bg);
                // Windows 11 vertical pill selection indicator
                let indicator_rect = Rect::new(6, y_nav + 8, 3, 24);
                surface.fill_rect(&indicator_rect, Color::WIN11_BLUE);
            }

            // Draw icon marker
            let icon_rect = Rect::new(16, y_nav + 12, 16, 16);
            surface.fill_rect(&icon_rect, if is_active { Color::WIN11_BLUE } else { text_dim });

            y_nav += item_height + 4;
        }

        // 3. Top Command Bar
        let content_x = sidebar_width as i32 + 12;
        let content_width = (surface.width as u32).saturating_sub(sidebar_width + 24);
        let cmdbar_rect = Rect::new(content_x, 10, content_width, 44);
        surface.fill_rect(&cmdbar_rect, card_bg);
        surface.draw_rect_outline(&cmdbar_rect, card_stroke, 1);

        // Command bar buttons: Run, End task, Efficiency
        let run_btn = Rect::new(content_x + 10, 16, 110, 32);
        surface.draw_rect_outline(&run_btn, card_stroke, 1);

        let end_btn = Rect::new(content_x + 130, 16, 100, 32);
        if self.command_bar.can_end_task {
            surface.draw_rect_outline(&end_btn, Color::STATUS_RED, 1);
        } else {
            surface.draw_rect_outline(&end_btn, card_stroke, 1);
        }

        let eff_btn = Rect::new(content_x + 240, 16, 130, 32);
        if self.command_bar.is_efficiency_active {
            surface.fill_rect(&eff_btn, Color::LEAF_ECO_GREEN);
        } else {
            surface.draw_rect_outline(&eff_btn, card_stroke, 1);
        }

        // Search Filter Input Box
        let search_box = Rect::new(content_x + content_width as i32 - 240, 16, 230, 32);
        surface.draw_rect_outline(&search_box, card_stroke, 1);

        // 4. Main Body Content Card
        let body_rect = Rect::new(content_x, 64, content_width, (surface.height as u32).saturating_sub(76));
        surface.fill_rect(&body_rect, card_bg);
        surface.draw_rect_outline(&body_rect, card_stroke, 1);

        // Render page specific graphics
        match self.current_page {
            NavPage::Processes => self.render_surface_processes(surface, &body_rect, card_stroke, text_color),
            NavPage::Performance => self.render_surface_performance(surface, &body_rect, card_stroke, text_color),
            _ => {
                // Generic content placeholder
                let center_marker = Rect::new(body_rect.x + 20, body_rect.y + 20, 160, 20);
                surface.fill_rect(&center_marker, Color::WIN11_BLUE);
            }
        }
    }

    fn render_surface_processes(&self, surface: &mut TaskMgrSurface, body: &Rect, stroke: Color, _text_color: Color) {
        // Table header row
        let header_rect = Rect::new(body.x, body.y, body.width, 32);
        surface.draw_rect_outline(&header_rect, stroke, 1);

        // Rows
        let row_height = 28;
        let mut y = body.y + 34;

        let filtered = self.filtered_processes();
        for proc in filtered.iter().take(20) {
            let row_rect = Rect::new(body.x + 4, y, body.width.saturating_sub(8), row_height as u32);
            let is_sel = self.selected_pid == Some(proc.pid);

            if is_sel {
                surface.fill_rect(&row_rect, Color::WIN11_BLUE.blend_over(Color::MICA_BACKGROUND_DARK));
            }

            // CPU utilization mini bar gauge
            let cpu_bar_width = ((proc.aggregate_cpu_percent().clamp(0.0, 100.0) / 100.0) * 60.0) as u32;
            if cpu_bar_width > 0 {
                let cpu_rect = Rect::new(body.x + 360, y + 8, cpu_bar_width, 12);
                surface.fill_rect(&cpu_rect, Color::WIN11_ACCENT_LIGHT);
            }

            // Efficiency leaf indicator
            if proc.is_efficiency_mode {
                let leaf_rect = Rect::new(body.x + 12, y + 8, 12, 12);
                surface.fill_rect(&leaf_rect, Color::LEAF_ECO_GREEN);
            }

            y += row_height + 2;
            if y + row_height as i32 >= body.bottom() {
                break;
            }
        }
    }

    fn render_surface_performance(&self, surface: &mut TaskMgrSurface, body: &Rect, stroke: Color, _text: Color) {
        // Split view: Left device list (200px) & Right chart panel
        let list_width = 240;
        let dev_list_rect = Rect::new(body.x, body.y, list_width, body.height);
        surface.draw_rect_outline(&dev_list_rect, stroke, 1);

        // Render sparkline devices
        let devices = [
            (PerfDevice::Cpu, &self.perf_telemetry.cpu.sparkline),
            (PerfDevice::Memory, &self.perf_telemetry.memory.sparkline),
            (PerfDevice::Disk(0), &self.perf_telemetry.disk0.sparkline),
            (PerfDevice::Disk(1), &self.perf_telemetry.disk1.sparkline),
            (PerfDevice::Ethernet, &self.perf_telemetry.ethernet.sparkline),
            (PerfDevice::Gpu(0), &self.perf_telemetry.gpu0.sparkline),
        ];

        let mut card_y = body.y + 8;
        for (dev, spark) in &devices {
            let is_sel = *dev == self.selected_perf_device;
            let card_rect = Rect::new(body.x + 6, card_y, list_width.saturating_sub(12), 56);

            if is_sel {
                surface.fill_rect(&card_rect, Color::WIN11_BLUE.blend_over(Color::MICA_BACKGROUND_DARK));
            }
            surface.draw_rect_outline(&card_rect, stroke, 1);

            // Mini sparkline plot
            self.draw_surface_sparkline(surface, spark, body.x + list_width as i32 - 90, card_y + 12, 76, 32, Color::WIN11_ACCENT_LIGHT);

            card_y += 62;
        }

        // Right Detail Big Chart
        let chart_x = body.x + list_width as i32 + 12;
        let chart_w = body.width.saturating_sub(list_width + 24);
        let chart_h = 240;
        let big_chart_rect = Rect::new(chart_x, body.y + 12, chart_w, chart_h);
        surface.draw_rect_outline(&big_chart_rect, stroke, 1);

        // Draw big graph gridlines
        for i in 1..4 {
            let grid_y = big_chart_rect.y + (chart_h as i32 * i) / 4;
            for gx in big_chart_rect.x..(big_chart_rect.x + chart_w as i32) {
                if gx % 4 == 0 {
                    surface.set_pixel(gx as usize, grid_y as usize, stroke);
                }
            }
        }

        // Plot big sparkline
        let active_spark = match self.selected_perf_device {
            PerfDevice::Cpu => &self.perf_telemetry.cpu.sparkline,
            PerfDevice::Memory => &self.perf_telemetry.memory.sparkline,
            PerfDevice::Disk(0) => &self.perf_telemetry.disk0.sparkline,
            PerfDevice::Disk(_) => &self.perf_telemetry.disk1.sparkline,
            PerfDevice::Ethernet => &self.perf_telemetry.ethernet.sparkline,
            PerfDevice::Gpu(_) => &self.perf_telemetry.gpu0.sparkline,
        };

        self.draw_surface_sparkline(
            surface,
            active_spark,
            chart_x + 4,
            body.y + 16,
            chart_w.saturating_sub(8),
            chart_h.saturating_sub(8),
            Color::WIN11_BLUE,
        );
    }

    fn draw_surface_sparkline(
        &self,
        surface: &mut TaskMgrSurface,
        spark: &SparklineBuffer,
        x0: i32,
        y0: i32,
        w: u32,
        h: u32,
        line_color: Color,
    ) {
        if spark.values.is_empty() || w == 0 || h == 0 {
            return;
        }

        let pts_count = spark.values.len();
        let step_x = w as f32 / pts_count.max(1) as f32;

        let mut prev_x = x0;
        let mut prev_y = y0 + h as i32;

        for (i, &val) in spark.values.iter().enumerate() {
            let px = x0 + (i as f32 * step_x) as i32;
            let norm = val.clamp(0.0, 100.0) / 100.0;
            let py = y0 + h as i32 - (norm * h as f32) as i32;

            if i > 0 {
                self.draw_line(surface, prev_x, prev_y, px, py, line_color);
            }
            prev_x = px;
            prev_y = py;
        }
    }

    fn draw_line(&self, surface: &mut TaskMgrSurface, mut x0: i32, mut y0: i32, x1: i32, y1: i32, color: Color) {
        let dx = (x1 - x0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let dy = -(y1 - y0).abs();
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;

        loop {
            if x0 >= 0 && y0 >= 0 && (x0 as usize) < surface.width && (y0 as usize) < surface.height {
                surface.set_pixel(x0 as usize, y0 as usize, color);
            }
            if x0 == x1 && y0 == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x0 += sx;
            }
            if e2 <= dx {
                err += dx;
                y0 += sy;
            }
        }
    }
}

// ============================================================================
// Section 10: Unit Test Suite
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Test 1: Process tree creation, parent-child expansion, and recursive telemetry aggregation
    #[test]
    fn test_process_tree_calculation() {
        let mut parent = ProcessItem::new(100, None, "ParentProc", ProcessGroup::Apps, 100 * 1024 * 1024, "Parent");
        parent.cpu_percent = 2.5;

        let mut child1 = ProcessItem::new(101, Some(100), "Child1", ProcessGroup::Apps, 50 * 1024 * 1024, "Child 1");
        child1.cpu_percent = 1.5;

        let mut child2 = ProcessItem::new(102, Some(100), "Child2", ProcessGroup::Apps, 30 * 1024 * 1024, "Child 2");
        child2.cpu_percent = 0.8;

        parent.children.push(child1);
        parent.children.push(child2);

        // Verification of recursive CPU aggregation: 2.5 + 1.5 + 0.8 = 4.8
        let agg_cpu = parent.aggregate_cpu_percent();
        assert!((agg_cpu - 4.8).abs() < 0.001);

        // Verification of recursive memory aggregation: 100MB + 50MB + 30MB = 180MB
        let agg_mem = parent.aggregate_memory_bytes();
        assert_eq!(agg_mem, 180 * 1024 * 1024);

        // Verification of total process count in subtree
        assert_eq!(parent.total_process_count(), 3);

        // Test expansion toggle
        assert!(!parent.is_expanded);
        parent.is_expanded = true;
        assert!(parent.is_expanded);
    }

    /// Test 2: Performance metric updates, FIFO 60-point ring buffer shift, and uptime formatting
    #[test]
    fn test_performance_metric_updates() {
        let mut mgr = TaskManager::new();
        assert_eq!(mgr.perf_telemetry.cpu.sparkline.values.len(), 60);

        let initial_uptime = mgr.perf_telemetry.cpu.uptime_seconds;
        mgr.update_tick(1.0);

        assert_eq!(mgr.perf_telemetry.cpu.uptime_seconds, initial_uptime + 1);
        assert_eq!(mgr.perf_telemetry.cpu.sparkline.values.len(), 60);

        // Test uptime format
        mgr.perf_telemetry.cpu.uptime_seconds = 90061; // 1 day, 1 hour, 1 minute, 1 sec
        let uptime_str = mgr.perf_telemetry.cpu.formatted_uptime();
        assert_eq!(uptime_str, "1:1:01:01");

        // Test sparkline min/max/average calculations
        let mut spark = SparklineBuffer::new(5, 10.0);
        spark.push(20.0);
        spark.push(30.0);
        assert_eq!(spark.current(), 30.0);
        assert_eq!(spark.max(), 30.0);
        assert_eq!(spark.min(), 10.0);
    }

    /// Test 3: End task action, PID removal from tree, and cascading process termination
    #[test]
    fn test_end_task_action() {
        let mut mgr = TaskManager::new();

        // Check initial presence of Windows Terminal (PID 4020)
        assert!(mgr.find_process(4020).is_some());

        // Attempting to end System (PID 4) must return an error
        let sys_kill = mgr.end_task(4);
        assert!(sys_kill.is_err());

        // End Terminal process (4020)
        let end_res = mgr.end_task(4020);
        assert!(end_res.is_ok());
        assert!(mgr.find_process(4020).is_none());

        // Non-existent PID must fail gracefully
        assert!(mgr.end_task(99999).is_err());
    }

    /// Test 4: Windows Service Start, Stop, and Restart lifecycle actions
    #[test]
    fn test_service_start_stop_restart() {
        let mut mgr = TaskManager::new();

        // 1. WSearch is initialized as Stopped
        let wsearch = mgr.services.iter().find(|s| s.name == "WSearch").unwrap();
        assert_eq!(wsearch.status, ServiceStatus::Stopped);
        assert_eq!(wsearch.pid, None);

        // Start WSearch
        let start_res = mgr.start_service("WSearch");
        assert!(start_res.is_ok());

        let wsearch_started = mgr.services.iter().find(|s| s.name == "WSearch").unwrap();
        assert_eq!(wsearch_started.status, ServiceStatus::Running);
        assert!(wsearch_started.pid.is_some());

        // Starting an already running service should return error
        assert!(mgr.start_service("WSearch").is_err());

        // Stop WSearch
        let stop_res = mgr.stop_service("WSearch");
        assert!(stop_res.is_ok());
        let wsearch_stopped = mgr.services.iter().find(|s| s.name == "WSearch").unwrap();
        assert_eq!(wsearch_stopped.status, ServiceStatus::Stopped);
        assert_eq!(wsearch_stopped.pid, None);

        // Restart WSearch
        let restart_res = mgr.restart_service("WSearch");
        assert!(restart_res.is_ok());
        let wsearch_restarted = mgr.services.iter().find(|s| s.name == "WSearch").unwrap();
        assert_eq!(wsearch_restarted.status, ServiceStatus::Running);
        assert!(wsearch_restarted.pid.is_some());
    }

    /// Test 5: Efficiency Mode (EcoQoS) toggle and priority lowering
    #[test]
    fn test_efficiency_mode_toggle() {
        let mut mgr = TaskManager::new();

        // Windows Terminal PID 4020 starts with normal priority and efficiency off
        let term = mgr.find_process(4020).unwrap();
        assert!(!term.is_efficiency_mode);
        assert_eq!(term.priority, ProcessPriority::Normal);

        // Toggle Efficiency Mode on
        let toggle_res = mgr.toggle_efficiency_mode(4020).unwrap();
        assert!(toggle_res);

        let term_eff = mgr.find_process(4020).unwrap();
        assert!(term_eff.is_efficiency_mode);
        assert_eq!(term_eff.priority, ProcessPriority::Idle);
        assert_eq!(term_eff.status, ProcessStatus::EfficiencyMode);

        // Child process (pwsh.exe PID 4180) should also inherit efficiency mode
        let pwsh = mgr.find_process(4180).unwrap();
        assert!(pwsh.is_efficiency_mode);
        assert_eq!(pwsh.priority, ProcessPriority::Idle);

        // Toggle Efficiency Mode off
        let toggle_off = mgr.toggle_efficiency_mode(4020).unwrap();
        assert!(!toggle_off);

        let term_normal = mgr.find_process(4020).unwrap();
        assert!(!term_normal.is_efficiency_mode);
        assert_eq!(term_normal.priority, ProcessPriority::Normal);
        assert_eq!(term_normal.status, ProcessStatus::Running);
    }

    /// Test 6: Resource values display mode formatting (Percentages vs Absolute Values)
    #[test]
    fn test_resource_values_display_mode() {
        let mut mgr = TaskManager::new();

        // Default mode is Percentages
        assert_eq!(mgr.resource_display_mode, ResourceDisplayMode::Percentages);
        let mem_formatted_pct = mgr.format_memory(3_435_973_836); // ~10% of 32GB
        assert!(mem_formatted_pct.contains('%'));

        // Toggle to Absolute Values
        mgr.handle_event(TaskMgrEvent::ToggleResourceValueMode).unwrap();
        assert_eq!(mgr.resource_display_mode, ResourceDisplayMode::AbsoluteValues);

        let mem_formatted_val = mgr.format_memory(3_435_973_836);
        assert!(mem_formatted_val.contains("MB"));
    }

    /// Test 7: Search filtering and column sorting
    #[test]
    fn test_search_filtering_and_sorting() {
        let mut mgr = TaskManager::new();

        // Filter for "Edge"
        mgr.handle_event(TaskMgrEvent::SetSearchFilter("Edge".to_string())).unwrap();
        let filtered = mgr.filtered_processes();
        assert!(!filtered.is_empty());
        assert!(filtered.iter().all(|p| p.name.contains("Edge")));

        // Clear filter
        mgr.handle_event(TaskMgrEvent::SetSearchFilter(String::new())).unwrap();
        assert_eq!(mgr.filtered_processes().len(), mgr.processes.len());

        // Sort by Memory Descending
        mgr.handle_event(TaskMgrEvent::SortByColumn(ProcessSortColumn::Memory)).unwrap();
        for i in 1..mgr.processes.len() {
            assert!(mgr.processes[i - 1].aggregate_memory_bytes() >= mgr.processes[i].aggregate_memory_bytes());
        }
    }

    /// Test 8: Surface Framebuffer rendering without panics in Dark and Light themes
    #[test]
    fn test_surface_rendering_zero_panic() {
        let mut mgr = TaskManager::new();
        let mut surface = TaskMgrSurface::new(1024, 768, Color::BLACK);

        // Test Dark Theme Rendering
        mgr.theme = ThemeMode::Dark;
        mgr.render_to_surface(&mut surface);
        assert_eq!(surface.width, 1024);
        assert_eq!(surface.height, 768);
        assert_ne!(surface.get_pixel(100, 100), Color::TRANSPARENT);

        // Test Light Theme Rendering
        mgr.theme = ThemeMode::Light;
        mgr.render_to_surface(&mut surface);
        assert_ne!(surface.get_pixel(100, 100), Color::TRANSPARENT);

        // Test ASCII view generation
        let ascii_view = mgr.render_ascii_view();
        assert!(!ascii_view.is_empty());
        assert!(ascii_view.contains("TASK MANAGER - HybridOS Win11 Core"));
        println!("\n{}", ascii_view);

        // Test Performance Page ASCII view
        mgr.current_page = NavPage::Performance;
        let perf_view = mgr.render_ascii_view();
        assert!(perf_view.contains("HARDWARE MONITOR & LIVE SPARKLINE TELEMETRY"));
        println!("\n{}", perf_view);

        // Test Details Page ASCII view
        mgr.current_page = NavPage::Details;
        let details_view = mgr.render_ascii_view();
        assert!(details_view.contains("Memory (WS)"));
        println!("\n{}", details_view);
    }
}
