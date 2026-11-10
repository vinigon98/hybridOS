//! Windows 11 Commercial Settings App - System & Display Category
//!
//! Dual-personality HybridOS implementation of the Windows 11 `ms-settings:system`
//! control plane in 100% safe, robust Rust.
//!
//! Subsystems covered:
//! 1. Display Settings (Resolution, Scaling, Orientation, Topology, Night Light, Refresh Rates)
//! 2. Sound Settings (Output/Input selectors, Master volume, Per-app volume mixer, Spatial sound, Mic test)
//! 3. Notifications & Focus (Do Not Disturb, Auto rules, App permissions, Focus sessions)
//! 4. Power & Battery (Screen/Sleep timeouts, Power modes, Battery Saver, Usage history)
//! 5. Storage & Storage Sense (Storage Sense automated cleanup, Drive usage breakdown)
//! 6. Multitasking, Recovery & About (Snap Assist, Alt+Tab tabs, Recovery flags, About device specifications)

#![deny(unsafe_code)]

use std::collections::BTreeMap;
use std::fmt;

// ============================================================================
// 1. DISPLAY SETTINGS
// ============================================================================

/// Screen resolution representation (width x height in pixels).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Resolution {
    pub width: u32,
    pub height: u32,
}

impl Resolution {
    pub const RES_1024X768: Resolution = Resolution { width: 1024, height: 768 };
    pub const RES_1920X1080: Resolution = Resolution { width: 1920, height: 1080 };
    pub const RES_2560X1440: Resolution = Resolution { width: 2560, height: 1440 };
    pub const RES_3840X2160: Resolution = Resolution { width: 3840, height: 2160 };

    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    /// List of standard Windows 11 certified display resolutions.
    pub fn standard_resolutions() -> &'static [Resolution] {
        &[
            Self::RES_1024X768,
            Self::RES_1920X1080,
            Self::RES_2560X1440,
            Self::RES_3840X2160,
        ]
    }

    /// Calculate the greatest common divisor for aspect ratio simplification.
    fn gcd(mut a: u32, mut b: u32) -> u32 {
        while b != 0 {
            let t = b;
            b = a % b;
            a = t;
        }
        if a == 0 { 1 } else { a }
    }

    /// Compute display aspect ratio (e.g. (16, 9), (4, 3)).
    pub fn aspect_ratio(&self) -> (u32, u32) {
        if self.width == 0 || self.height == 0 {
            return (0, 0);
        }
        let divisor = Self::gcd(self.width, self.height);
        (self.width / divisor, self.height / divisor)
    }

    /// Format aspect ratio as human-readable string (e.g., "16:9").
    pub fn aspect_ratio_str(&self) -> String {
        let (w, h) = self.aspect_ratio();
        format!("{}:{}", w, h)
    }

    /// Total pixel count.
    pub fn total_pixels(&self) -> u64 {
        (self.width as u64) * (self.height as u64)
    }

    /// Returns true if this resolution is officially recommended/standard.
    pub fn is_standard(&self) -> bool {
        Self::standard_resolutions().contains(self)
    }
}

impl fmt::Display for Resolution {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} x {}", self.width, self.height)
    }
}

/// Display DPI scaling factors supported by Windows 11 Fluent UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DisplayScale {
    Scale100, // 100% (Default for standard DPI)
    Scale125, // 125%
    Scale150, // 150% (Recommended for 1440p/4K laptops)
    Scale175, // 175%
    Scale200, // 200% (Recommended for 4K desktop monitors)
}

impl DisplayScale {
    pub fn percentage(&self) -> u32 {
        match self {
            DisplayScale::Scale100 => 100,
            DisplayScale::Scale125 => 125,
            DisplayScale::Scale150 => 150,
            DisplayScale::Scale175 => 175,
            DisplayScale::Scale200 => 200,
        }
    }

    pub fn factor(&self) -> f32 {
        (self.percentage() as f32) / 100.0
    }

    pub fn from_percentage(pct: u32) -> Result<Self, String> {
        match pct {
            100 => Ok(DisplayScale::Scale100),
            125 => Ok(DisplayScale::Scale125),
            150 => Ok(DisplayScale::Scale150),
            175 => Ok(DisplayScale::Scale175),
            200 => Ok(DisplayScale::Scale200),
            other => Err(format!("Unsupported scaling percentage: {}%. Allowed: 100, 125, 150, 175, 200.", other)),
        }
    }

    pub fn all() -> &'static [DisplayScale] {
        &[
            DisplayScale::Scale100,
            DisplayScale::Scale125,
            DisplayScale::Scale150,
            DisplayScale::Scale175,
            DisplayScale::Scale200,
        ]
    }
}

impl fmt::Display for DisplayScale {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}%", self.percentage())
    }
}

/// Display orientation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DisplayOrientation {
    Landscape,
    Portrait,
    LandscapeFlipped,
    PortraitFlipped,
}

impl DisplayOrientation {
    pub fn rotation_degrees(&self) -> u16 {
        match self {
            DisplayOrientation::Landscape => 0,
            DisplayOrientation::Portrait => 90,
            DisplayOrientation::LandscapeFlipped => 180,
            DisplayOrientation::PortraitFlipped => 270,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            DisplayOrientation::Landscape => "Landscape",
            DisplayOrientation::Portrait => "Portrait",
            DisplayOrientation::LandscapeFlipped => "Landscape (flipped)",
            DisplayOrientation::PortraitFlipped => "Portrait (flipped)",
        }
    }

    pub fn from_str(s: &str) -> Result<Self, String> {
        match s.trim().to_lowercase().as_str() {
            "landscape" => Ok(DisplayOrientation::Landscape),
            "portrait" => Ok(DisplayOrientation::Portrait),
            "landscape (flipped)" | "landscape_flipped" | "landscapeflipped" => Ok(DisplayOrientation::LandscapeFlipped),
            "portrait (flipped)" | "portrait_flipped" | "portraitflipped" => Ok(DisplayOrientation::PortraitFlipped),
            other => Err(format!("Unknown display orientation: '{}'", other)),
        }
    }
}

impl fmt::Display for DisplayOrientation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Multi-monitor layout and topology options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MultiMonitorTopology {
    Extend,        // "Extend these displays"
    Duplicate,     // "Duplicate these displays"
    ShowOnlyOn1,   // "Show only on 1"
    ShowOnlyOn2,   // "Show only on 2"
}

impl MultiMonitorTopology {
    pub fn description(&self) -> &'static str {
        match self {
            MultiMonitorTopology::Extend => "Extend these displays",
            MultiMonitorTopology::Duplicate => "Duplicate these displays",
            MultiMonitorTopology::ShowOnlyOn1 => "Show only on 1",
            MultiMonitorTopology::ShowOnlyOn2 => "Show only on 2",
        }
    }

    pub fn from_str(s: &str) -> Result<Self, String> {
        match s.trim().to_lowercase().as_str() {
            "extend" | "extend these displays" => Ok(MultiMonitorTopology::Extend),
            "duplicate" | "duplicate these displays" => Ok(MultiMonitorTopology::Duplicate),
            "show only on 1" | "showonlyon1" | "1_only" => Ok(MultiMonitorTopology::ShowOnlyOn1),
            "show only on 2" | "showonlyon2" | "2_only" => Ok(MultiMonitorTopology::ShowOnlyOn2),
            other => Err(format!("Unknown multi-monitor topology: '{}'", other)),
        }
    }
}

impl fmt::Display for MultiMonitorTopology {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.description())
    }
}

/// Night Light schedule modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NightLightSchedule {
    None,
    SunsetToSunrise,
    CustomHours {
        turn_on_hour: u8,
        turn_on_minute: u8,
        turn_off_hour: u8,
        turn_off_minute: u8,
    },
}

/// Night Light settings for reduced blue light emissions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NightLightSettings {
    pub enabled: bool,
    /// Color temperature in Kelvin: 1000K (warmest/amber) to 6500K (standard daylight).
    pub color_temperature_k: u32,
    pub schedule: NightLightSchedule,
}

impl NightLightSettings {
    pub const MIN_TEMP_K: u32 = 1000;
    pub const MAX_TEMP_K: u32 = 6500;
    pub const DEFAULT_TEMP_K: u32 = 4500;

    pub fn new() -> Self {
        Self {
            enabled: false,
            color_temperature_k: Self::DEFAULT_TEMP_K,
            schedule: NightLightSchedule::SunsetToSunrise,
        }
    }

    /// Set color temperature clamped to valid range [1000K, 6500K].
    pub fn set_temperature(&mut self, kelvin: u32) {
        self.color_temperature_k = kelvin.clamp(Self::MIN_TEMP_K, Self::MAX_TEMP_K);
    }

    /// Check if night light should currently be active based on time (24h format).
    pub fn is_active_at(&self, hour: u8, minute: u8) -> bool {
        if !self.enabled {
            return false;
        }
        match self.schedule {
            NightLightSchedule::None => true, // Manually toggled on
            NightLightSchedule::SunsetToSunrise => {
                // Approximate Windows Sunset (21:00) to Sunrise (07:00)
                hour >= 21 || hour < 7
            }
            NightLightSchedule::CustomHours {
                turn_on_hour,
                turn_on_minute,
                turn_off_hour,
                turn_off_minute,
            } => {
                let current_mins = (hour as u32) * 60 + (minute as u32);
                let on_mins = (turn_on_hour as u32) * 60 + (turn_on_minute as u32);
                let off_mins = (turn_off_hour as u32) * 60 + (turn_off_minute as u32);

                if on_mins <= off_mins {
                    // Daytime window
                    current_mins >= on_mins && current_mins < off_mins
                } else {
                    // Spans across midnight
                    current_mins >= on_mins || current_mins < off_mins
                }
            }
        }
    }
}

impl Default for NightLightSettings {
    fn default() -> Self {
        Self::new()
    }
}

/// Display refresh rate selector (in Hertz).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RefreshRate {
    Hz60,
    Hz75,
    Hz120,
    Hz144,
    Hz240,
    Custom(u32),
}

impl RefreshRate {
    pub fn hz(&self) -> u32 {
        match self {
            RefreshRate::Hz60 => 60,
            RefreshRate::Hz75 => 75,
            RefreshRate::Hz120 => 120,
            RefreshRate::Hz144 => 144,
            RefreshRate::Hz240 => 240,
            RefreshRate::Custom(hz) => *hz,
        }
    }

    pub fn from_hz(hz: u32) -> Self {
        match hz {
            60 => RefreshRate::Hz60,
            75 => RefreshRate::Hz75,
            120 => RefreshRate::Hz120,
            144 => RefreshRate::Hz144,
            240 => RefreshRate::Hz240,
            other => RefreshRate::Custom(other),
        }
    }

    pub fn standard_rates() -> &'static [RefreshRate] {
        &[
            RefreshRate::Hz60,
            RefreshRate::Hz75,
            RefreshRate::Hz120,
            RefreshRate::Hz144,
            RefreshRate::Hz240,
        ]
    }
}

impl fmt::Display for RefreshRate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} Hz", self.hz())
    }
}

/// Detailed monitor hardware information.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonitorDisplayInfo {
    pub monitor_id: u32,
    pub name: String,
    pub is_primary: bool,
    pub resolution: Resolution,
    pub scale: DisplayScale,
    pub orientation: DisplayOrientation,
    pub refresh_rate: RefreshRate,
    pub hdr_supported: bool,
    pub hdr_enabled: bool,
}

impl MonitorDisplayInfo {
    pub fn new(id: u32, name: &str, is_primary: bool, res: Resolution) -> Self {
        Self {
            monitor_id: id,
            name: name.to_string(),
            is_primary,
            resolution: res,
            scale: DisplayScale::Scale100,
            orientation: DisplayOrientation::Landscape,
            refresh_rate: RefreshRate::Hz60,
            hdr_supported: true,
            hdr_enabled: false,
        }
    }
}

/// Comprehensive Display Settings Subsystem.
#[derive(Debug, Clone, PartialEq)]
pub struct DisplaySettings {
    pub primary_monitor: MonitorDisplayInfo,
    pub secondary_monitors: Vec<MonitorDisplayInfo>,
    pub topology: MultiMonitorTopology,
    pub night_light: NightLightSettings,
    pub supported_resolutions: Vec<Resolution>,
    pub supported_refresh_rates: Vec<RefreshRate>,
}

impl DisplaySettings {
    pub fn new() -> Self {
        let primary = MonitorDisplayInfo::new(1, "Display 1: Internal Generic PnP Monitor", true, Resolution::RES_1920X1080);
        Self {
            primary_monitor: primary,
            secondary_monitors: Vec::new(),
            topology: MultiMonitorTopology::Extend,
            night_light: NightLightSettings::new(),
            supported_resolutions: Resolution::standard_resolutions().to_vec(),
            supported_refresh_rates: RefreshRate::standard_rates().to_vec(),
        }
    }

    /// Set resolution of the primary monitor.
    pub fn set_resolution(&mut self, res: Resolution) -> Result<(), String> {
        if res.width == 0 || res.height == 0 {
            return Err("Invalid resolution dimensions: width and height must be > 0".to_string());
        }
        self.primary_monitor.resolution = res;
        Ok(())
    }

    /// Set scaling factor of the primary monitor.
    pub fn set_scaling(&mut self, scale: DisplayScale) {
        self.primary_monitor.scale = scale;
    }

    /// Set display orientation of the primary monitor.
    pub fn set_orientation(&mut self, orientation: DisplayOrientation) {
        self.primary_monitor.orientation = orientation;
    }

    /// Set refresh rate of the primary monitor.
    pub fn set_refresh_rate(&mut self, rate: RefreshRate) {
        self.primary_monitor.refresh_rate = rate;
    }

    /// Set multi-monitor topology.
    pub fn set_topology(&mut self, topology: MultiMonitorTopology) {
        self.topology = topology;
    }

    /// Add a secondary monitor.
    pub fn add_secondary_monitor(&mut self, monitor: MonitorDisplayInfo) {
        self.secondary_monitors.push(monitor);
    }

    /// Remove a secondary monitor by ID.
    pub fn remove_secondary_monitor(&mut self, monitor_id: u32) -> bool {
        let initial_len = self.secondary_monitors.len();
        self.secondary_monitors.retain(|m| m.monitor_id != monitor_id);
        self.secondary_monitors.len() < initial_len
    }

    /// Total connected monitor count.
    pub fn monitor_count(&self) -> usize {
        1 + self.secondary_monitors.len()
    }
}

impl Default for DisplaySettings {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// 2. SOUND SETTINGS
// ============================================================================

/// Audio device hardware classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AudioOutputType {
    Speakers,
    Headphones,
    HdmiAudio,
    BluetoothAudio,
    OpticalAudio,
}

impl AudioOutputType {
    pub fn as_str(&self) -> &'static str {
        match self {
            AudioOutputType::Speakers => "Speakers",
            AudioOutputType::Headphones => "Headphones",
            AudioOutputType::HdmiAudio => "HDMI Audio",
            AudioOutputType::BluetoothAudio => "Bluetooth Audio",
            AudioOutputType::OpticalAudio => "Digital Optical Audio",
        }
    }
}

/// Output audio device descriptor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioOutputDevice {
    pub id: String,
    pub name: String,
    pub device_type: AudioOutputType,
    pub is_default: bool,
}

/// Audio input device hardware classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AudioInputType {
    MicrophoneArray,
    HeadsetMicrophone,
    ExternalMicrophone,
    LineIn,
}

impl AudioInputType {
    pub fn as_str(&self) -> &'static str {
        match self {
            AudioInputType::MicrophoneArray => "Microphone Array",
            AudioInputType::HeadsetMicrophone => "Headset Microphone",
            AudioInputType::ExternalMicrophone => "External Microphone",
            AudioInputType::LineIn => "Line In",
        }
    }
}

/// Input audio device descriptor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioInputDevice {
    pub id: String,
    pub name: String,
    pub device_type: AudioInputType,
    pub is_default: bool,
}

/// Windows 11 Spatial Sound processing engine options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpatialSound {
    Off,
    WindowsSonicForHeadphones,
    DolbyAtmos,
    DtsHeadphoneX,
}

impl SpatialSound {
    pub fn as_str(&self) -> &'static str {
        match self {
            SpatialSound::Off => "Off",
            SpatialSound::WindowsSonicForHeadphones => "Windows Sonic for Headphones",
            SpatialSound::DolbyAtmos => "Dolby Atmos for Headphones",
            SpatialSound::DtsHeadphoneX => "DTS Headphone:X",
        }
    }

    pub fn from_str(s: &str) -> Result<Self, String> {
        match s.trim().to_lowercase().as_str() {
            "off" => Ok(SpatialSound::Off),
            "windows sonic" | "windows sonic for headphones" | "sonic" => Ok(SpatialSound::WindowsSonicForHeadphones),
            "dolby atmos" | "dolby" | "dolby atmos for headphones" => Ok(SpatialSound::DolbyAtmos),
            "dts" | "dts headphone:x" => Ok(SpatialSound::DtsHeadphoneX),
            other => Err(format!("Unknown spatial sound mode: '{}'", other)),
        }
    }
}

impl fmt::Display for SpatialSound {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Per-app Volume Mixer entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppVolumeEntry {
    pub app_id: String,
    pub display_name: String,
    /// Volume 0..100%
    pub volume: u8,
    pub is_muted: bool,
    pub output_device_override: Option<String>,
    pub input_device_override: Option<String>,
}

impl AppVolumeEntry {
    pub fn new(app_id: &str, display_name: &str, volume: u8) -> Self {
        Self {
            app_id: app_id.to_string(),
            display_name: display_name.to_string(),
            volume: volume.min(100),
            is_muted: false,
            output_device_override: None,
            input_device_override: None,
        }
    }
}

/// Comprehensive Sound Settings Subsystem.
#[derive(Debug, Clone, PartialEq)]
pub struct SoundSettings {
    pub output_devices: Vec<AudioOutputDevice>,
    pub active_output_id: String,
    /// Master volume level 0..100%
    pub master_volume: u8,
    pub is_master_muted: bool,

    pub input_devices: Vec<AudioInputDevice>,
    pub active_input_id: String,
    /// Input microphone volume level 0..100%
    pub input_volume: u8,
    pub is_input_muted: bool,
    /// Test microphone peak meter reading 0..100%
    pub mic_test_meter_level: u8,

    pub spatial_sound: SpatialSound,
    pub app_mixer: BTreeMap<String, AppVolumeEntry>,
}

impl SoundSettings {
    pub fn new() -> Self {
        let default_output = AudioOutputDevice {
            id: "audio_out_speakers_realtek".to_string(),
            name: "Speakers (Realtek High Definition Audio)".to_string(),
            device_type: AudioOutputType::Speakers,
            is_default: true,
        };
        let headphones = AudioOutputDevice {
            id: "audio_out_headphones".to_string(),
            name: "Headphones (HyperX Cloud II)".to_string(),
            device_type: AudioOutputType::Headphones,
            is_default: false,
        };
        let hdmi = AudioOutputDevice {
            id: "audio_out_hdmi".to_string(),
            name: "HDMI Audio (NVIDIA High Definition Audio)".to_string(),
            device_type: AudioOutputType::HdmiAudio,
            is_default: false,
        };

        let default_input = AudioInputDevice {
            id: "audio_in_mic_array".to_string(),
            name: "Microphone Array (Realtek High Definition Audio)".to_string(),
            device_type: AudioInputType::MicrophoneArray,
            is_default: true,
        };

        let mut mixer = BTreeMap::new();
        mixer.insert(
            "chrome.exe".to_string(),
            AppVolumeEntry::new("chrome.exe", "Google Chrome", 85),
        );
        mixer.insert(
            "spotify.exe".to_string(),
            AppVolumeEntry::new("spotify.exe", "Spotify Music Player", 70),
        );
        mixer.insert(
            "hybridos_terminal.exe".to_string(),
            AppVolumeEntry::new("hybridos_terminal.exe", "HybridOS Terminal Host", 100),
        );

        Self {
            output_devices: vec![default_output.clone(), headphones, hdmi],
            active_output_id: default_output.id,
            master_volume: 75,
            is_master_muted: false,

            input_devices: vec![default_input.clone()],
            active_input_id: default_input.id,
            input_volume: 80,
            is_input_muted: false,
            mic_test_meter_level: 0,

            spatial_sound: SpatialSound::Off,
            app_mixer: mixer,
        }
    }

    /// Set master volume level, clamped to 0..100%.
    pub fn set_master_volume(&mut self, vol: u8) {
        self.master_volume = vol.min(100);
    }

    /// Toggle master mute status.
    pub fn toggle_master_mute(&mut self) -> bool {
        self.is_master_muted = !self.is_master_muted;
        self.is_master_muted
    }

    /// Select active output device by ID.
    pub fn select_output_device(&mut self, device_id: &str) -> Result<(), String> {
        if self.output_devices.iter().any(|d| d.id == device_id) {
            self.active_output_id = device_id.to_string();
            for d in &mut self.output_devices {
                d.is_default = d.id == device_id;
            }
            Ok(())
        } else {
            Err(format!("Output device with ID '{}' not found", device_id))
        }
    }

    /// Select active input device by ID.
    pub fn select_input_device(&mut self, device_id: &str) -> Result<(), String> {
        if self.input_devices.iter().any(|d| d.id == device_id) {
            self.active_input_id = device_id.to_string();
            for d in &mut self.input_devices {
                d.is_default = d.id == device_id;
            }
            Ok(())
        } else {
            Err(format!("Input device with ID '{}' not found", device_id))
        }
    }

    /// Set input volume level, clamped to 0..100%.
    pub fn set_input_volume(&mut self, vol: u8) {
        self.input_volume = vol.min(100);
    }

    /// Simulate microphone testing meter with an amplitude level (0..100%).
    pub fn test_microphone_sample(&mut self, sample_peak: u8) {
        if self.is_input_muted {
            self.mic_test_meter_level = 0;
        } else {
            let effective = (sample_peak as u32 * self.input_volume as u32) / 100;
            self.mic_test_meter_level = (effective as u8).min(100);
        }
    }

    /// Set individual app volume in volume mixer.
    pub fn set_app_volume(&mut self, app_id: &str, vol: u8) -> Result<(), String> {
        if let Some(entry) = self.app_mixer.get_mut(app_id) {
            entry.volume = vol.min(100);
            Ok(())
        } else {
            Err(format!("Application '{}' not found in Volume Mixer", app_id))
        }
    }

    /// Toggle app mute status in volume mixer.
    pub fn toggle_app_mute(&mut self, app_id: &str) -> Result<bool, String> {
        if let Some(entry) = self.app_mixer.get_mut(app_id) {
            entry.is_muted = !entry.is_muted;
            Ok(entry.is_muted)
        } else {
            Err(format!("Application '{}' not found in Volume Mixer", app_id))
        }
    }

    /// Register or update an app stream in the volume mixer.
    pub fn register_app_stream(&mut self, app_id: &str, display_name: &str, volume: u8) {
        let entry = self
            .app_mixer
            .entry(app_id.to_string())
            .or_insert_with(|| AppVolumeEntry::new(app_id, display_name, volume));
        entry.display_name = display_name.to_string();
        entry.volume = volume.min(100);
    }

    /// Remove an app stream from volume mixer.
    pub fn unregister_app_stream(&mut self, app_id: &str) -> bool {
        self.app_mixer.remove(app_id).is_some()
    }

    /// Set spatial sound mode.
    pub fn set_spatial_sound(&mut self, mode: SpatialSound) {
        self.spatial_sound = mode;
    }
}

impl Default for SoundSettings {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// 3. NOTIFICATIONS & FOCUS
// ============================================================================

/// Notification display priority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NotificationPriority {
    Normal,
    High,
    Top, // Appears on top of lock screen and full screen games
}

/// Per-app notification permissions and presentation rules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppNotificationConfig {
    pub app_id: String,
    pub app_name: String,
    pub enabled: bool,
    pub show_banners: bool,
    pub play_sound: bool,
    pub priority: NotificationPriority,
    pub show_in_notification_center: bool,
}

impl AppNotificationConfig {
    pub fn new(app_id: &str, app_name: &str) -> Self {
        Self {
            app_id: app_id.to_string(),
            app_name: app_name.to_string(),
            enabled: true,
            show_banners: true,
            play_sound: true,
            priority: NotificationPriority::Normal,
            show_in_notification_center: true,
        }
    }
}

/// Automatic rules for triggering Do Not Disturb.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DndAutoRules {
    pub schedule_enabled: bool,
    pub schedule_start: (u8, u8), // (hour, minute)
    pub schedule_end: (u8, u8),
    pub when_duplicating_display: bool,
    pub when_playing_game: bool,
    pub when_using_fullscreen_app: bool,
    pub first_hour_after_update: bool,
}

impl Default for DndAutoRules {
    fn default() -> Self {
        Self {
            schedule_enabled: false,
            schedule_start: (23, 0),
            schedule_end: (7, 0),
            when_duplicating_display: true,
            when_playing_game: true,
            when_using_fullscreen_app: true,
            first_hour_after_update: true,
        }
    }
}

/// Focus session duration and environment control.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusSession {
    /// Duration in minutes: Windows standard presets are 15, 30, 45, or custom.
    pub duration_minutes: u32,
    pub hide_badge_on_taskbar_apps: bool,
    pub hide_flashing_on_taskbar_apps: bool,
    pub turn_on_dnd_during_focus: bool,
    pub is_active: bool,
    pub remaining_seconds: u32,
}

impl FocusSession {
    pub fn new(duration_minutes: u32) -> Self {
        Self {
            duration_minutes,
            hide_badge_on_taskbar_apps: true,
            hide_flashing_on_taskbar_apps: true,
            turn_on_dnd_during_focus: true,
            is_active: false,
            remaining_seconds: duration_minutes * 60,
        }
    }

    /// Start or restart a focus session with specified duration.
    pub fn start(&mut self, duration_minutes: u32) {
        self.duration_minutes = duration_minutes;
        self.remaining_seconds = duration_minutes * 60;
        self.is_active = true;
    }

    /// End active focus session.
    pub fn stop(&mut self) {
        self.is_active = false;
        self.remaining_seconds = self.duration_minutes * 60;
    }

    /// Advance session timer by elapsed seconds. Returns true when session concludes.
    pub fn tick(&mut self, elapsed_seconds: u32) -> bool {
        if !self.is_active {
            return false;
        }
        if elapsed_seconds >= self.remaining_seconds {
            self.remaining_seconds = 0;
            self.is_active = false;
            true // Session completed
        } else {
            self.remaining_seconds -= elapsed_seconds;
            false
        }
    }
}

/// Comprehensive Notifications & Focus Subsystem.
#[derive(Debug, Clone, PartialEq)]
pub struct NotificationsAndFocusSettings {
    pub dnd_enabled: bool,
    pub dnd_rules: DndAutoRules,
    pub app_permissions: BTreeMap<String, AppNotificationConfig>,
    pub focus_session: FocusSession,
}

impl NotificationsAndFocusSettings {
    pub fn new() -> Self {
        let mut permissions = BTreeMap::new();
        permissions.insert(
            "com.microsoft.teams".to_string(),
            AppNotificationConfig::new("com.microsoft.teams", "Microsoft Teams"),
        );
        permissions.insert(
            "com.microsoft.outlook".to_string(),
            AppNotificationConfig::new("com.microsoft.outlook", "Outlook Mail"),
        );
        permissions.insert(
            "slack.exe".to_string(),
            AppNotificationConfig::new("slack.exe", "Slack"),
        );
        permissions.insert(
            "org.mozilla.firefox".to_string(),
            AppNotificationConfig::new("org.mozilla.firefox", "Firefox"),
        );

        Self {
            dnd_enabled: false,
            dnd_rules: DndAutoRules::default(),
            app_permissions: permissions,
            focus_session: FocusSession::new(30),
        }
    }

    /// Toggle Do Not Disturb manual override.
    pub fn toggle_dnd(&mut self) -> bool {
        self.dnd_enabled = !self.dnd_enabled;
        self.dnd_enabled
    }

    /// Check if Do Not Disturb is triggered either manually, by focus session, or by auto rules.
    pub fn is_dnd_active(&self, hour: u8, minute: u8, is_gaming: bool, is_fullscreen: bool, is_duplicating: bool) -> bool {
        if self.dnd_enabled {
            return true;
        }
        if self.focus_session.is_active && self.focus_session.turn_on_dnd_during_focus {
            return true;
        }
        if self.dnd_rules.when_playing_game && is_gaming {
            return true;
        }
        if self.dnd_rules.when_using_fullscreen_app && is_fullscreen {
            return true;
        }
        if self.dnd_rules.when_duplicating_display && is_duplicating {
            return true;
        }
        if self.dnd_rules.schedule_enabled {
            let cur = (hour as u32) * 60 + (minute as u32);
            let start = (self.dnd_rules.schedule_start.0 as u32) * 60 + (self.dnd_rules.schedule_start.1 as u32);
            let end = (self.dnd_rules.schedule_end.0 as u32) * 60 + (self.dnd_rules.schedule_end.1 as u32);

            if start <= end {
                if cur >= start && cur < end {
                    return true;
                }
            } else if cur >= start || cur < end {
                return true;
            }
        }
        false
    }

    /// Update notification toggles for a specific app.
    pub fn set_app_notification_toggles(
        &mut self,
        app_id: &str,
        enabled: bool,
        banners: bool,
        sound: bool,
    ) -> Result<(), String> {
        if let Some(cfg) = self.app_permissions.get_mut(app_id) {
            cfg.enabled = enabled;
            cfg.show_banners = banners;
            cfg.play_sound = sound;
            Ok(())
        } else {
            Err(format!("App notification config for '{}' not found", app_id))
        }
    }
}

impl Default for NotificationsAndFocusSettings {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// 4. POWER & BATTERY
// ============================================================================

/// Windows 11 Power Mode profiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PowerMode {
    BestPowerEfficiency,
    Balanced,
    BestPerformance,
}

impl PowerMode {
    pub fn description(&self) -> &'static str {
        match self {
            PowerMode::BestPowerEfficiency => "Best power efficiency",
            PowerMode::Balanced => "Balanced (recommended)",
            PowerMode::BestPerformance => "Best performance",
        }
    }

    pub fn from_str(s: &str) -> Result<Self, String> {
        match s.trim().to_lowercase().as_str() {
            "best power efficiency" | "power_efficiency" | "efficiency" => Ok(PowerMode::BestPowerEfficiency),
            "balanced" | "balanced (recommended)" => Ok(PowerMode::Balanced),
            "best performance" | "performance" => Ok(PowerMode::BestPerformance),
            other => Err(format!("Unknown power mode profile: '{}'", other)),
        }
    }
}

impl fmt::Display for PowerMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.description())
    }
}

/// Screen and system sleep timeout configuration (in minutes, 0 = Never).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimeoutTimers {
    pub screen_turn_off_battery_mins: u32,
    pub screen_turn_off_plugged_mins: u32,
    pub sleep_after_battery_mins: u32,
    pub sleep_after_plugged_mins: u32,
}

impl Default for TimeoutTimers {
    fn default() -> Self {
        Self {
            screen_turn_off_battery_mins: 5,  // Windows 11 default on battery: 5 minutes
            screen_turn_off_plugged_mins: 15, // Windows 11 default plugged in: 15 minutes
            sleep_after_battery_mins: 15,
            sleep_after_plugged_mins: 30,
        }
    }
}

/// Automated Battery Saver settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatterySaverConfig {
    pub manual_override_enabled: bool,
    /// Automatic trigger battery threshold percentage (default 20%).
    pub auto_turn_on_threshold_percent: u8,
    pub lower_screen_brightness: bool,
}

impl Default for BatterySaverConfig {
    fn default() -> Self {
        Self {
            manual_override_enabled: false,
            auto_turn_on_threshold_percent: 20,
            lower_screen_brightness: true,
        }
    }
}

/// Hardware battery state and diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatteryStatus {
    pub charge_percent: u8,
    pub is_charging: bool,
    pub is_plugged_in: bool,
    pub estimated_remaining_minutes: Option<u32>,
    pub health_percent: u8,
    pub cycle_count: u32,
}

impl Default for BatteryStatus {
    fn default() -> Self {
        Self {
            charge_percent: 88,
            is_charging: false,
            is_plugged_in: false,
            estimated_remaining_minutes: Some(310), // ~5h 10m
            health_percent: 97,
            cycle_count: 42,
        }
    }
}

/// Battery usage history record per hour interval.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatteryUsageRecord {
    pub timestamp_hour: u64,
    pub battery_level: u8,
    pub screen_on_minutes: u16,
    pub screen_off_minutes: u16,
    pub top_draining_apps: Vec<(String, u8)>, // (app_name, percent_drain)
}

/// Comprehensive Power & Battery Subsystem.
#[derive(Debug, Clone, PartialEq)]
pub struct PowerAndBatterySettings {
    pub power_mode: PowerMode,
    pub timeouts: TimeoutTimers,
    pub battery_saver: BatterySaverConfig,
    pub battery_status: BatteryStatus,
    pub usage_history: Vec<BatteryUsageRecord>,
}

impl PowerAndBatterySettings {
    pub fn new() -> Self {
        let history = vec![
            BatteryUsageRecord {
                timestamp_hour: 1700000000,
                battery_level: 100,
                screen_on_minutes: 45,
                screen_off_minutes: 15,
                top_draining_apps: vec![("Edge".to_string(), 35), ("Code".to_string(), 25)],
            },
            BatteryUsageRecord {
                timestamp_hour: 1700003600,
                battery_level: 94,
                screen_on_minutes: 55,
                screen_off_minutes: 5,
                top_draining_apps: vec![("Code".to_string(), 40), ("Terminal".to_string(), 20)],
            },
            BatteryUsageRecord {
                timestamp_hour: 1700007200,
                battery_level: 88,
                screen_on_minutes: 50,
                screen_off_minutes: 10,
                top_draining_apps: vec![("Teams".to_string(), 45), ("Spotify".to_string(), 15)],
            },
        ];

        Self {
            power_mode: PowerMode::Balanced,
            timeouts: TimeoutTimers::default(),
            battery_saver: BatterySaverConfig::default(),
            battery_status: BatteryStatus::default(),
            usage_history: history,
        }
    }

    /// Check if Battery Saver is currently active.
    pub fn is_battery_saver_active(&self) -> bool {
        if self.battery_saver.manual_override_enabled {
            return true;
        }
        if !self.battery_status.is_plugged_in
            && self.battery_status.charge_percent <= self.battery_saver.auto_turn_on_threshold_percent
        {
            return true;
        }
        false
    }

    /// Select power mode.
    pub fn set_power_mode(&mut self, mode: PowerMode) {
        self.power_mode = mode;
    }

    /// Update battery saver automatic threshold percentage (clamped 0..100%).
    pub fn set_battery_saver_threshold(&mut self, threshold_percent: u8) {
        self.battery_saver.auto_turn_on_threshold_percent = threshold_percent.min(100);
    }
}

impl Default for PowerAndBatterySettings {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// 5. STORAGE & STORAGE SENSE
// ============================================================================

/// Storage Sense automatic cleanup cadence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StorageSenseFrequency {
    EveryDay,
    EveryWeek,
    EveryMonth,
    DuringLowFreeDiskSpace,
}

impl StorageSenseFrequency {
    pub fn description(&self) -> &'static str {
        match self {
            StorageSenseFrequency::EveryDay => "Every day",
            StorageSenseFrequency::EveryWeek => "Every week",
            StorageSenseFrequency::EveryMonth => "Every month",
            StorageSenseFrequency::DuringLowFreeDiskSpace => "During low free disk space (default)",
        }
    }

    pub fn from_str(s: &str) -> Result<Self, String> {
        let norm = s.trim().to_lowercase();
        if norm.contains("day") || norm.contains("daily") {
            Ok(StorageSenseFrequency::EveryDay)
        } else if norm.contains("week") || norm.contains("weekly") {
            Ok(StorageSenseFrequency::EveryWeek)
        } else if norm.contains("month") || norm.contains("monthly") {
            Ok(StorageSenseFrequency::EveryMonth)
        } else if norm.contains("low") || norm.contains("default") {
            Ok(StorageSenseFrequency::DuringLowFreeDiskSpace)
        } else {
            Err(format!("Unknown Storage Sense frequency: '{}'", s))
        }
    }
}

impl fmt::Display for StorageSenseFrequency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.description())
    }
}

/// Storage Sense configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageSenseConfig {
    pub enabled: bool,
    pub run_frequency: StorageSenseFrequency,
    /// Days to keep files in Recycle Bin before deletion (0 = Never, 1, 14, 30, 60).
    pub clean_recycle_bin_days: u32,
    /// Days to keep files in Downloads folder before deletion (0 = Never, 1, 14, 30, 60).
    pub clean_downloads_days: u32,
    pub clean_temporary_files: bool,
}

impl Default for StorageSenseConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            run_frequency: StorageSenseFrequency::DuringLowFreeDiskSpace,
            clean_recycle_bin_days: 30,
            clean_downloads_days: 0, // Never by default in Windows 11
            clean_temporary_files: true,
        }
    }
}

/// Statistics reported after running Storage Sense cleanup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageCleanupReport {
    pub temporary_files_cleaned_bytes: u64,
    pub recycle_bin_cleaned_bytes: u64,
    pub downloads_cleaned_bytes: u64,
    pub total_freed_bytes: u64,
}

impl StorageCleanupReport {
    pub fn formatted_freed(&self) -> String {
        DriveUsageBreakdown::format_bytes(self.total_freed_bytes)
    }
}

/// Detailed drive partition usage metrics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriveUsageBreakdown {
    pub drive_letter: char,
    pub volume_label: String,
    pub total_bytes: u64,
    pub installed_apps_bytes: u64,
    pub temporary_files_bytes: u64,
    pub other_bytes: u64,
    pub system_reserved_bytes: u64,
}

impl DriveUsageBreakdown {
    pub fn used_bytes(&self) -> u64 {
        self.installed_apps_bytes
            .saturating_add(self.temporary_files_bytes)
            .saturating_add(self.other_bytes)
            .saturating_add(self.system_reserved_bytes)
    }

    pub fn free_bytes(&self) -> u64 {
        self.total_bytes.saturating_sub(self.used_bytes())
    }

    pub fn free_percentage(&self) -> f32 {
        if self.total_bytes == 0 {
            0.0
        } else {
            (self.free_bytes() as f32 / self.total_bytes as f32) * 100.0
        }
    }

    pub fn used_percentage(&self) -> f32 {
        if self.total_bytes == 0 {
            0.0
        } else {
            (self.used_bytes() as f32 / self.total_bytes as f32) * 100.0
        }
    }

    /// Formats raw byte count into human-friendly decimal GB/MB/TB string.
    pub fn format_bytes(bytes: u64) -> String {
        const KB: u64 = 1024;
        const MB: u64 = 1024 * KB;
        const GB: u64 = 1024 * MB;
        const TB: u64 = 1024 * GB;

        if bytes >= TB {
            format!("{:.2} TB", bytes as f64 / TB as f64)
        } else if bytes >= GB {
            format!("{:.1} GB", bytes as f64 / GB as f64)
        } else if bytes >= MB {
            format!("{:.1} MB", bytes as f64 / MB as f64)
        } else if bytes >= KB {
            format!("{:.1} KB", bytes as f64 / KB as f64)
        } else {
            format!("{} bytes", bytes)
        }
    }
}

/// Comprehensive Storage Subsystem.
#[derive(Debug, Clone, PartialEq)]
pub struct StorageSettings {
    pub storage_sense: StorageSenseConfig,
    pub drives: Vec<DriveUsageBreakdown>,
}

impl StorageSettings {
    pub fn new() -> Self {
        // Default C: drive breakdown for 1TB NVMe drive
        const GB: u64 = 1024 * 1024 * 1024;
        let c_drive = DriveUsageBreakdown {
            drive_letter: 'C',
            volume_label: "HybridOS System Disk".to_string(),
            total_bytes: 953 * GB, // ~1TB formatted
            installed_apps_bytes: 142 * GB,
            temporary_files_bytes: 18 * GB,
            other_bytes: 85 * GB,
            system_reserved_bytes: 48 * GB,
        };

        Self {
            storage_sense: StorageSenseConfig::default(),
            drives: vec![c_drive],
        }
    }

    /// Execute Storage Sense cleanup cycle, freeing temp files.
    pub fn run_storage_sense_now(&mut self) -> StorageCleanupReport {
        let mut freed_temp = 0u64;
        const GB: u64 = 1024 * 1024 * 1024;

        if self.storage_sense.clean_temporary_files {
            for drive in &mut self.drives {
                let cleaned = drive.temporary_files_bytes.min(15 * GB);
                drive.temporary_files_bytes = drive.temporary_files_bytes.saturating_sub(cleaned);
                freed_temp += cleaned;
            }
        }

        let freed_recycle = 2 * GB;
        let total = freed_temp + freed_recycle;

        StorageCleanupReport {
            temporary_files_cleaned_bytes: freed_temp,
            recycle_bin_cleaned_bytes: freed_recycle,
            downloads_cleaned_bytes: 0,
            total_freed_bytes: total,
        }
    }

    /// Primary system drive usage reference.
    pub fn primary_drive(&self) -> Option<&DriveUsageBreakdown> {
        self.drives.iter().find(|d| d.drive_letter == 'C')
    }
}

impl Default for StorageSettings {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// 6. MULTITASKING, RECOVERY & ABOUT
// ============================================================================

/// Browser tab integration in Alt+Tab window switching.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AltTabTabBehavior {
    AllTabs,
    FiveMostRecentTabs,
    ThreeMostRecentTabs,
    WindowsOnly,
}

impl AltTabTabBehavior {
    pub fn description(&self) -> &'static str {
        match self {
            AltTabTabBehavior::AllTabs => "Open windows and all tabs in Microsoft Edge",
            AltTabTabBehavior::FiveMostRecentTabs => "Open windows and 5 most recent tabs",
            AltTabTabBehavior::ThreeMostRecentTabs => "Open windows and 3 most recent tabs",
            AltTabTabBehavior::WindowsOnly => "Don't show tabs (Open windows only)",
        }
    }

    pub fn from_str(s: &str) -> Result<Self, String> {
        let norm = s.trim().to_lowercase();
        if norm.contains("all tabs") || norm == "all" {
            Ok(AltTabTabBehavior::AllTabs)
        } else if norm.contains("5 most recent") || norm == "5" {
            Ok(AltTabTabBehavior::FiveMostRecentTabs)
        } else if norm.contains("3 most recent") || norm == "3" {
            Ok(AltTabTabBehavior::ThreeMostRecentTabs)
        } else if norm.contains("windows only") || norm.contains("don't show tabs") || norm == "none" || norm == "off" {
            Ok(AltTabTabBehavior::WindowsOnly)
        } else {
            Err(format!("Unknown Alt+Tab tab behavior: '{}'", s))
        }
    }
}

impl fmt::Display for AltTabTabBehavior {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.description())
    }
}

/// Multitasking and Snap Assist settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultitaskingSettings {
    pub snap_windows_enabled: bool,
    /// When I snap a window, show what I can snap next to it.
    pub snap_assist_enabled: bool,
    /// Show snap layouts when I hover over a window's maximize button.
    pub show_snap_layouts_on_hover: bool,
    /// Drag a window to the top of the screen to snap it into a layout.
    pub drag_to_screen_top_snaps: bool,
    pub alt_tab_tab_behavior: AltTabTabBehavior,
}

impl Default for MultitaskingSettings {
    fn default() -> Self {
        Self {
            snap_windows_enabled: true,
            snap_assist_enabled: true,
            show_snap_layouts_on_hover: true,
            drag_to_screen_top_snaps: true,
            alt_tab_tab_behavior: AltTabTabBehavior::FiveMostRecentTabs,
        }
    }
}

/// Recovery subsystem diagnostics and options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoverySettings {
    pub reset_this_pc_available: bool,
    pub advanced_startup_available: bool,
    pub go_back_to_previous_build_available: bool,
}

impl Default for RecoverySettings {
    fn default() -> Self {
        Self {
            reset_this_pc_available: true,
            advanced_startup_available: true,
            go_back_to_previous_build_available: false,
        }
    }
}

/// About section device hardware and OS build specification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AboutSystemInfo {
    pub device_name: String,
    pub processor: String,
    pub installed_ram_bytes: u64,
    pub installed_ram_usable_bytes: u64,
    pub device_id: String,
    pub product_id: String,
    pub system_type: String,
    pub pen_and_touch: String,
    pub edition: String,
    pub version: String,
    pub os_build: String,
    pub hybridos_kernel_version: String,
    pub experience_pack: String,
}

impl AboutSystemInfo {
    pub fn new() -> Self {
        const GB: u64 = 1024 * 1024 * 1024;
        Self {
            device_name: "HYBRIDOS-WORKSTATION".to_string(),
            processor: "13th Gen Intel(R) Core(TM) i9-13900K @ 3.00 GHz".to_string(),
            installed_ram_bytes: 32 * GB,
            installed_ram_usable_bytes: (31 * GB) + (800 * 1024 * 1024), // ~31.8 GB
            device_id: "E45B82A1-0C7D-4F29-9B3E-51DF334A8109".to_string(),
            product_id: "00330-80000-00000-AAOEM".to_string(),
            system_type: "64-bit operating system, x64-based processor".to_string(),
            pen_and_touch: "No pen or touch input is available for this display".to_string(),
            edition: "Windows 11 Pro".to_string(),
            version: "23H2".to_string(),
            os_build: "22631.3000".to_string(),
            hybridos_kernel_version: "HybridOS Dual-Personality Kernel 0.1.0-alpha".to_string(),
            experience_pack: "Windows Feature Experience Pack 1000.22631.1000.0".to_string(),
        }
    }

    pub fn formatted_ram(&self) -> String {
        format!(
            "{} ({:.1} GB usable)",
            DriveUsageBreakdown::format_bytes(self.installed_ram_bytes),
            self.installed_ram_usable_bytes as f64 / (1024.0 * 1024.0 * 1024.0)
        )
    }
}

impl Default for AboutSystemInfo {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// TOP-LEVEL SYSTEM SETTINGS AGGREGATOR
// ============================================================================

/// Windows 11 System Settings (`ms-settings:system`).
#[derive(Debug, Clone, PartialEq)]
pub struct SystemSettings {
    pub display: DisplaySettings,
    pub sound: SoundSettings,
    pub notifications: NotificationsAndFocusSettings,
    pub power: PowerAndBatterySettings,
    pub storage: StorageSettings,
    pub multitasking: MultitaskingSettings,
    pub recovery: RecoverySettings,
    pub about: AboutSystemInfo,
}

impl SystemSettings {
    /// Instantiate a clean production default system configuration.
    pub fn new() -> Self {
        Self {
            display: DisplaySettings::new(),
            sound: SoundSettings::new(),
            notifications: NotificationsAndFocusSettings::new(),
            power: PowerAndBatterySettings::new(),
            storage: StorageSettings::new(),
            multitasking: MultitaskingSettings::default(),
            recovery: RecoverySettings::default(),
            about: AboutSystemInfo::new(),
        }
    }

    /// Reset all sections back to factory settings.
    pub fn reset_to_defaults(&mut self) {
        *self = Self::new();
    }

    /// Serialize entire settings tree into formatted JSON string without third-party crates.
    pub fn to_json(&self) -> String {
        let mut buf = String::with_capacity(4096);
        buf.push_str("{\n");

        // 1. Display
        buf.push_str("  \"display\": {\n");
        buf.push_str(&format!(
            "    \"resolution\": {{\"width\": {}, \"height\": {}}},\n",
            self.display.primary_monitor.resolution.width, self.display.primary_monitor.resolution.height
        ));
        buf.push_str(&format!("    \"scale_percent\": {},\n", self.display.primary_monitor.scale.percentage()));
        buf.push_str(&format!("    \"orientation\": \"{}\",\n", self.display.primary_monitor.orientation.as_str()));
        buf.push_str(&format!("    \"refresh_rate_hz\": {},\n", self.display.primary_monitor.refresh_rate.hz()));
        buf.push_str(&format!("    \"topology\": \"{}\",\n", self.display.topology.description()));
        buf.push_str("    \"night_light\": {\n");
        buf.push_str(&format!("      \"enabled\": {},\n", self.display.night_light.enabled));
        buf.push_str(&format!("      \"temperature_k\": {}\n", self.display.night_light.color_temperature_k));
        buf.push_str("    }\n");
        buf.push_str("  },\n");

        // 2. Sound
        buf.push_str("  \"sound\": {\n");
        buf.push_str(&format!("    \"active_output_id\": \"{}\",\n", escape_json(&self.sound.active_output_id)));
        buf.push_str(&format!("    \"master_volume\": {},\n", self.sound.master_volume));
        buf.push_str(&format!("    \"is_master_muted\": {},\n", self.sound.is_master_muted));
        buf.push_str(&format!("    \"active_input_id\": \"{}\",\n", escape_json(&self.sound.active_input_id)));
        buf.push_str(&format!("    \"input_volume\": {},\n", self.sound.input_volume));
        buf.push_str(&format!("    \"is_input_muted\": {},\n", self.sound.is_input_muted));
        buf.push_str(&format!("    \"spatial_sound\": \"{}\",\n", self.sound.spatial_sound.as_str()));
        buf.push_str("    \"app_mixer\": {\n");
        let mut mixer_items = self.sound.app_mixer.iter().peekable();
        while let Some((k, v)) = mixer_items.next() {
            let comma = if mixer_items.peek().is_some() { "," } else { "" };
            buf.push_str(&format!(
                "      \"{}\": {{\"volume\": {}, \"muted\": {}}}{}\n",
                escape_json(k),
                v.volume,
                v.is_muted,
                comma
            ));
        }
        buf.push_str("    }\n");
        buf.push_str("  },\n");

        // 3. Notifications
        buf.push_str("  \"notifications\": {\n");
        buf.push_str(&format!("    \"dnd_enabled\": {},\n", self.notifications.dnd_enabled));
        buf.push_str("    \"focus_session\": {\n");
        buf.push_str(&format!("      \"duration_minutes\": {},\n", self.notifications.focus_session.duration_minutes));
        buf.push_str(&format!("      \"is_active\": {},\n", self.notifications.focus_session.is_active));
        buf.push_str(&format!(
            "      \"hide_badge_on_taskbar_apps\": {}\n",
            self.notifications.focus_session.hide_badge_on_taskbar_apps
        ));
        buf.push_str("    }\n");
        buf.push_str("  },\n");

        // 4. Power
        buf.push_str("  \"power\": {\n");
        buf.push_str(&format!("    \"power_mode\": \"{}\",\n", self.power.power_mode.description()));
        buf.push_str("    \"timeouts\": {\n");
        buf.push_str(&format!(
            "      \"screen_battery_mins\": {},\n",
            self.power.timeouts.screen_turn_off_battery_mins
        ));
        buf.push_str(&format!(
            "      \"screen_plugged_mins\": {}\n",
            self.power.timeouts.screen_turn_off_plugged_mins
        ));
        buf.push_str("    },\n");
        buf.push_str("    \"battery_saver\": {\n");
        buf.push_str(&format!(
            "      \"auto_threshold_percent\": {},\n",
            self.power.battery_saver.auto_turn_on_threshold_percent
        ));
        buf.push_str(&format!(
            "      \"lower_brightness\": {}\n",
            self.power.battery_saver.lower_screen_brightness
        ));
        buf.push_str("    }\n");
        buf.push_str("  },\n");

        // 5. Storage
        buf.push_str("  \"storage\": {\n");
        buf.push_str("    \"storage_sense\": {\n");
        buf.push_str(&format!("      \"enabled\": {},\n", self.storage.storage_sense.enabled));
        buf.push_str(&format!(
            "      \"frequency\": \"{}\"\n",
            self.storage.storage_sense.run_frequency.description()
        ));
        buf.push_str("    }\n");
        buf.push_str("  },\n");

        // 6. Multitasking
        buf.push_str("  \"multitasking\": {\n");
        buf.push_str(&format!("    \"snap_windows\": {},\n", self.multitasking.snap_windows_enabled));
        buf.push_str(&format!("    \"snap_assist\": {},\n", self.multitasking.snap_assist_enabled));
        buf.push_str(&format!(
            "    \"alt_tab_tabs\": \"{}\"\n",
            self.multitasking.alt_tab_tab_behavior.description()
        ));
        buf.push_str("  },\n");

        // 7. About
        buf.push_str("  \"about\": {\n");
        buf.push_str(&format!("    \"device_name\": \"{}\",\n", escape_json(&self.about.device_name)));
        buf.push_str(&format!("    \"processor\": \"{}\",\n", escape_json(&self.about.processor)));
        buf.push_str(&format!("    \"installed_ram_bytes\": {},\n", self.about.installed_ram_bytes));
        buf.push_str(&format!("    \"system_type\": \"{}\",\n", escape_json(&self.about.system_type)));
        buf.push_str(&format!("    \"edition\": \"{}\",\n", escape_json(&self.about.edition)));
        buf.push_str(&format!("    \"os_build\": \"{}\",\n", escape_json(&self.about.os_build)));
        buf.push_str(&format!(
            "    \"hybridos_kernel_version\": \"{}\"\n",
            escape_json(&self.about.hybridos_kernel_version)
        ));
        buf.push_str("  }\n");

        buf.push_str("}");
        buf
    }

    /// Deserialize settings from JSON string.
    pub fn from_json(json: &str) -> Result<Self, String> {
        let parsed = parse_json_value(json)?;
        let mut settings = Self::new();

        if let JsonVal::Object(root) = parsed {
            // Display
            if let Some(JsonVal::Object(disp)) = root.get("display") {
                if let Some(JsonVal::Object(res)) = disp.get("resolution") {
                    let w = res.get("width").and_then(|v| v.as_u32()).unwrap_or(1920);
                    let h = res.get("height").and_then(|v| v.as_u32()).unwrap_or(1080);
                    settings.display.primary_monitor.resolution = Resolution::new(w, h);
                }
                if let Some(scale_pct) = disp.get("scale_percent").and_then(|v| v.as_u32()) {
                    if let Ok(scale) = DisplayScale::from_percentage(scale_pct) {
                        settings.display.primary_monitor.scale = scale;
                    }
                }
                if let Some(orient_str) = disp.get("orientation").and_then(|v| v.as_str()) {
                    if let Ok(orient) = DisplayOrientation::from_str(orient_str) {
                        settings.display.primary_monitor.orientation = orient;
                    }
                }
                if let Some(hz) = disp.get("refresh_rate_hz").and_then(|v| v.as_u32()) {
                    settings.display.primary_monitor.refresh_rate = RefreshRate::from_hz(hz);
                }
                if let Some(top_str) = disp.get("topology").and_then(|v| v.as_str()) {
                    if let Ok(top) = MultiMonitorTopology::from_str(top_str) {
                        settings.display.topology = top;
                    }
                }
                if let Some(JsonVal::Object(nl)) = disp.get("night_light") {
                    if let Some(enabled) = nl.get("enabled").and_then(|v| v.as_bool()) {
                        settings.display.night_light.enabled = enabled;
                    }
                    if let Some(temp) = nl.get("temperature_k").and_then(|v| v.as_u32()) {
                        settings.display.night_light.set_temperature(temp);
                    }
                }
            }

            // Sound
            if let Some(JsonVal::Object(snd)) = root.get("sound") {
                if let Some(id) = snd.get("active_output_id").and_then(|v| v.as_str()) {
                    let _ = settings.sound.select_output_device(id);
                }
                if let Some(vol) = snd.get("master_volume").and_then(|v| v.as_u32()) {
                    settings.sound.set_master_volume(vol as u8);
                }
                if let Some(muted) = snd.get("is_master_muted").and_then(|v| v.as_bool()) {
                    settings.sound.is_master_muted = muted;
                }
                if let Some(vol) = snd.get("input_volume").and_then(|v| v.as_u32()) {
                    settings.sound.set_input_volume(vol as u8);
                }
                if let Some(muted) = snd.get("is_input_muted").and_then(|v| v.as_bool()) {
                    settings.sound.is_input_muted = muted;
                }
                if let Some(spatial) = snd.get("spatial_sound").and_then(|v| v.as_str()) {
                    if let Ok(mode) = SpatialSound::from_str(spatial) {
                        settings.sound.spatial_sound = mode;
                    }
                }
                if let Some(JsonVal::Object(mixer)) = snd.get("app_mixer") {
                    for (app_id, entry_val) in mixer {
                        if let JsonVal::Object(entry_map) = entry_val {
                            let vol = entry_map.get("volume").and_then(|v| v.as_u32()).unwrap_or(80) as u8;
                            let muted = entry_map.get("muted").and_then(|v| v.as_bool()).unwrap_or(false);
                            let entry = settings
                                .sound
                                .app_mixer
                                .entry(app_id.clone())
                                .or_insert_with(|| AppVolumeEntry::new(app_id, app_id, vol));
                            entry.volume = vol;
                            entry.is_muted = muted;
                        }
                    }
                }
            }

            // Notifications
            if let Some(JsonVal::Object(notif)) = root.get("notifications") {
                if let Some(dnd) = notif.get("dnd_enabled").and_then(|v| v.as_bool()) {
                    settings.notifications.dnd_enabled = dnd;
                }
                if let Some(JsonVal::Object(foc)) = notif.get("focus_session") {
                    if let Some(dur) = foc.get("duration_minutes").and_then(|v| v.as_u32()) {
                        settings.notifications.focus_session.duration_minutes = dur;
                    }
                    if let Some(act) = foc.get("is_active").and_then(|v| v.as_bool()) {
                        settings.notifications.focus_session.is_active = act;
                    }
                    if let Some(badge) = foc.get("hide_badge_on_taskbar_apps").and_then(|v| v.as_bool()) {
                        settings.notifications.focus_session.hide_badge_on_taskbar_apps = badge;
                    }
                }
            }

            // Power
            if let Some(JsonVal::Object(pwr)) = root.get("power") {
                if let Some(mode_str) = pwr.get("power_mode").and_then(|v| v.as_str()) {
                    if let Ok(mode) = PowerMode::from_str(mode_str) {
                        settings.power.power_mode = mode;
                    }
                }
                if let Some(JsonVal::Object(tm)) = pwr.get("timeouts") {
                    if let Some(m) = tm.get("screen_battery_mins").and_then(|v| v.as_u32()) {
                        settings.power.timeouts.screen_turn_off_battery_mins = m;
                    }
                    if let Some(m) = tm.get("screen_plugged_mins").and_then(|v| v.as_u32()) {
                        settings.power.timeouts.screen_turn_off_plugged_mins = m;
                    }
                }
                if let Some(JsonVal::Object(bs)) = pwr.get("battery_saver") {
                    if let Some(th) = bs.get("auto_threshold_percent").and_then(|v| v.as_u32()) {
                        settings.power.battery_saver.auto_turn_on_threshold_percent = th as u8;
                    }
                    if let Some(lb) = bs.get("lower_brightness").and_then(|v| v.as_bool()) {
                        settings.power.battery_saver.lower_screen_brightness = lb;
                    }
                }
            }

            // Storage
            if let Some(JsonVal::Object(stg)) = root.get("storage") {
                if let Some(JsonVal::Object(ss)) = stg.get("storage_sense") {
                    if let Some(en) = ss.get("enabled").and_then(|v| v.as_bool()) {
                        settings.storage.storage_sense.enabled = en;
                    }
                    if let Some(freq_str) = ss.get("frequency").and_then(|v| v.as_str()) {
                        if let Ok(freq) = StorageSenseFrequency::from_str(freq_str) {
                            settings.storage.storage_sense.run_frequency = freq;
                        }
                    }
                }
            }

            // Multitasking
            if let Some(JsonVal::Object(mt)) = root.get("multitasking") {
                if let Some(sw) = mt.get("snap_windows").and_then(|v| v.as_bool()) {
                    settings.multitasking.snap_windows_enabled = sw;
                }
                if let Some(sa) = mt.get("snap_assist").and_then(|v| v.as_bool()) {
                    settings.multitasking.snap_assist_enabled = sa;
                }
                if let Some(att) = mt.get("alt_tab_tabs").and_then(|v| v.as_str()) {
                    if let Ok(mode) = AltTabTabBehavior::from_str(att) {
                        settings.multitasking.alt_tab_tab_behavior = mode;
                    }
                }
            }

            // About
            if let Some(JsonVal::Object(abt)) = root.get("about") {
                if let Some(name) = abt.get("device_name").and_then(|v| v.as_str()) {
                    settings.about.device_name = name.to_string();
                }
                if let Some(proc) = abt.get("processor").and_then(|v| v.as_str()) {
                    settings.about.processor = proc.to_string();
                }
                if let Some(ram) = abt.get("installed_ram_bytes").and_then(|v| v.as_u64()) {
                    settings.about.installed_ram_bytes = ram;
                }
                if let Some(st) = abt.get("system_type").and_then(|v| v.as_str()) {
                    settings.about.system_type = st.to_string();
                }
                if let Some(ed) = abt.get("edition").and_then(|v| v.as_str()) {
                    settings.about.edition = ed.to_string();
                }
                if let Some(bld) = abt.get("os_build").and_then(|v| v.as_str()) {
                    settings.about.os_build = bld.to_string();
                }
                if let Some(kv) = abt.get("hybridos_kernel_version").and_then(|v| v.as_str()) {
                    settings.about.hybridos_kernel_version = kv.to_string();
                }
            }

            Ok(settings)
        } else {
            Err("Root JSON element must be an object".to_string())
        }
    }
}

impl Default for SystemSettings {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// 100% SAFE BUILT-IN JSON PARSER & SERIALIZER
// ============================================================================

fn escape_json(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
    }
    out
}

#[derive(Debug, Clone, PartialEq)]
enum JsonVal {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<JsonVal>),
    Object(BTreeMap<String, JsonVal>),
}

impl JsonVal {
    fn as_str(&self) -> Option<&str> {
        match self {
            JsonVal::String(s) => Some(s.as_str()),
            _ => None,
        }
    }

    fn as_bool(&self) -> Option<bool> {
        match self {
            JsonVal::Bool(b) => Some(*b),
            _ => None,
        }
    }

    fn as_u32(&self) -> Option<u32> {
        match self {
            JsonVal::Number(n) => Some(*n as u32),
            _ => None,
        }
    }

    fn as_u64(&self) -> Option<u64> {
        match self {
            JsonVal::Number(n) => Some(*n as u64),
            _ => None,
        }
    }
}

struct JsonParser<'a> {
    chars: &'a [u8],
    pos: usize,
}

impl<'a> JsonParser<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            chars: input.as_bytes(),
            pos: 0,
        }
    }

    fn skip_whitespace(&mut self) {
        while self.pos < self.chars.len() {
            let b = self.chars[self.pos];
            if b == b' ' || b == b'\t' || b == b'\r' || b == b'\n' {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    fn peek(&mut self) -> Option<u8> {
        self.skip_whitespace();
        if self.pos < self.chars.len() {
            Some(self.chars[self.pos])
        } else {
            None
        }
    }

    fn parse_value(&mut self) -> Result<JsonVal, String> {
        self.skip_whitespace();
        let b = self.peek().ok_or("Unexpected end of JSON input")?;
        match b {
            b'{' => self.parse_object(),
            b'[' => self.parse_array(),
            b'"' => self.parse_string().map(JsonVal::String),
            b't' | b'f' => self.parse_bool(),
            b'n' => self.parse_null(),
            b'-' | b'0'..=b'9' => self.parse_number(),
            other => Err(format!("Unexpected character '{}' at offset {}", other as char, self.pos)),
        }
    }

    fn parse_object(&mut self) -> Result<JsonVal, String> {
        self.pos += 1; // consume '{'
        let mut map = BTreeMap::new();
        self.skip_whitespace();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Ok(JsonVal::Object(map));
        }

        loop {
            self.skip_whitespace();
            let key = self.parse_string()?;
            self.skip_whitespace();
            if self.peek() != Some(b':') {
                return Err(format!("Expected ':' after object key at offset {}", self.pos));
            }
            self.pos += 1; // consume ':'
            let val = self.parse_value()?;
            map.insert(key, val);

            self.skip_whitespace();
            match self.peek() {
                Some(b',') => {
                    self.pos += 1;
                }
                Some(b'}') => {
                    self.pos += 1;
                    break;
                }
                other => return Err(format!("Expected ',' or '}}' at offset {}, found {:?}", self.pos, other)),
            }
        }
        Ok(JsonVal::Object(map))
    }

    fn parse_array(&mut self) -> Result<JsonVal, String> {
        self.pos += 1; // consume '['
        let mut vec = Vec::new();
        self.skip_whitespace();
        if self.peek() == Some(b']') {
            self.pos += 1;
            return Ok(JsonVal::Array(vec));
        }

        loop {
            let val = self.parse_value()?;
            vec.push(val);
            self.skip_whitespace();
            match self.peek() {
                Some(b',') => {
                    self.pos += 1;
                }
                Some(b']') => {
                    self.pos += 1;
                    break;
                }
                other => return Err(format!("Expected ',' or ']' at offset {}, found {:?}", self.pos, other)),
            }
        }
        Ok(JsonVal::Array(vec))
    }

    fn parse_string(&mut self) -> Result<String, String> {
        self.skip_whitespace();
        if self.peek() != Some(b'"') {
            return Err(format!("Expected '\"' at offset {}", self.pos));
        }
        self.pos += 1; // consume opening '"'
        let mut out = String::new();
        while self.pos < self.chars.len() {
            let b = self.chars[self.pos];
            self.pos += 1;
            match b {
                b'"' => return Ok(out),
                b'\\' => {
                    if self.pos >= self.chars.len() {
                        return Err("Unterminated escape sequence".to_string());
                    }
                    let esc = self.chars[self.pos];
                    self.pos += 1;
                    match esc {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'/' => out.push('/'),
                        other => out.push(other as char),
                    }
                }
                other => out.push(other as char),
            }
        }
        Err("Unterminated string literal".to_string())
    }

    fn parse_bool(&mut self) -> Result<JsonVal, String> {
        if self.chars[self.pos..].starts_with(b"true") {
            self.pos += 4;
            Ok(JsonVal::Bool(true))
        } else if self.chars[self.pos..].starts_with(b"false") {
            self.pos += 5;
            Ok(JsonVal::Bool(false))
        } else {
            Err(format!("Expected boolean at offset {}", self.pos))
        }
    }

    fn parse_null(&mut self) -> Result<JsonVal, String> {
        if self.chars[self.pos..].starts_with(b"null") {
            self.pos += 4;
            Ok(JsonVal::Null)
        } else {
            Err(format!("Expected null at offset {}", self.pos))
        }
    }

    fn parse_number(&mut self) -> Result<JsonVal, String> {
        let start = self.pos;
        if self.chars[self.pos] == b'-' {
            self.pos += 1;
        }
        while self.pos < self.chars.len() && self.chars[self.pos].is_ascii_digit() {
            self.pos += 1;
        }
        if self.pos < self.chars.len() && self.chars[self.pos] == b'.' {
            self.pos += 1;
            while self.pos < self.chars.len() && self.chars[self.pos].is_ascii_digit() {
                self.pos += 1;
            }
        }
        let s = std::str::from_utf8(&self.chars[start..self.pos])
            .map_err(|e| format!("Invalid utf8 number: {}", e))?;
        let num: f64 = s.parse().map_err(|e| format!("Invalid number '{}': {}", s, e))?;
        Ok(JsonVal::Number(num))
    }
}

fn parse_json_value(input: &str) -> Result<JsonVal, String> {
    let mut parser = JsonParser::new(input);
    parser.parse_value()
}

// ============================================================================
// UNIT TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolution_change() {
        let mut display = DisplaySettings::new();
        assert_eq!(display.primary_monitor.resolution, Resolution::RES_1920X1080);
        assert_eq!(display.primary_monitor.resolution.aspect_ratio_str(), "16:9");

        // Change to 4K UHD
        assert!(display.set_resolution(Resolution::RES_3840X2160).is_ok());
        assert_eq!(display.primary_monitor.resolution.width, 3840);
        assert_eq!(display.primary_monitor.resolution.height, 2160);
        assert_eq!(display.primary_monitor.resolution.total_pixels(), 8_294_400);

        // Change to 1024x768 (4:3)
        assert!(display.set_resolution(Resolution::RES_1024X768).is_ok());
        assert_eq!(display.primary_monitor.resolution.aspect_ratio(), (4, 3));

        // Invalid resolution dimensions should be rejected
        assert!(display.set_resolution(Resolution::new(0, 1080)).is_err());
        assert!(display.set_resolution(Resolution::new(1920, 0)).is_err());
    }

    #[test]
    fn test_volume_mixer() {
        let mut sound = SoundSettings::new();
        assert_eq!(sound.master_volume, 75);
        assert!(!sound.is_master_muted);

        // Master volume adjustment & mute toggle
        sound.set_master_volume(90);
        assert_eq!(sound.master_volume, 90);
        assert!(sound.toggle_master_mute());
        assert!(sound.is_master_muted);

        // Clamping check
        sound.set_master_volume(150);
        assert_eq!(sound.master_volume, 100);

        // Per-app volume mixer
        assert!(sound.set_app_volume("spotify.exe", 50).is_ok());
        assert_eq!(sound.app_mixer.get("spotify.exe").unwrap().volume, 50);

        let muted = sound.toggle_app_mute("spotify.exe").unwrap();
        assert!(muted);
        assert!(sound.app_mixer.get("spotify.exe").unwrap().is_muted);

        // Test mic meter testing simulation
        sound.is_input_muted = false;
        sound.input_volume = 80;
        sound.test_microphone_sample(50);
        assert_eq!(sound.mic_test_meter_level, 40); // 50 * 80% = 40%

        sound.is_input_muted = true;
        sound.test_microphone_sample(50);
        assert_eq!(sound.mic_test_meter_level, 0); // Muted microphone meter is zero
    }

    #[test]
    fn test_power_mode_selection() {
        let mut power = PowerAndBatterySettings::new();
        assert_eq!(power.power_mode, PowerMode::Balanced);

        // Change power mode
        power.set_power_mode(PowerMode::BestPerformance);
        assert_eq!(power.power_mode, PowerMode::BestPerformance);
        assert_eq!(power.power_mode.description(), "Best performance");

        power.set_power_mode(PowerMode::BestPowerEfficiency);
        assert_eq!(power.power_mode, PowerMode::BestPowerEfficiency);

        // Timers default verification
        assert_eq!(power.timeouts.screen_turn_off_battery_mins, 5);
        assert_eq!(power.timeouts.screen_turn_off_plugged_mins, 15);

        // Battery saver threshold
        power.set_battery_saver_threshold(25);
        assert_eq!(power.battery_saver.auto_turn_on_threshold_percent, 25);
    }

    #[test]
    fn test_storage_sense_toggle() {
        let mut storage = StorageSettings::new();
        assert!(storage.storage_sense.enabled);
        assert_eq!(
            storage.storage_sense.run_frequency,
            StorageSenseFrequency::DuringLowFreeDiskSpace
        );

        // Toggle frequency
        storage.storage_sense.run_frequency = StorageSenseFrequency::EveryWeek;
        assert_eq!(storage.storage_sense.run_frequency, StorageSenseFrequency::EveryWeek);

        // Check drive breakdown
        let drive = storage.primary_drive().unwrap();
        assert_eq!(drive.drive_letter, 'C');
        assert!(drive.total_bytes > 0);
        assert!(drive.free_bytes() > 0);
        assert!(drive.used_percentage() > 0.0);

        // Execute cleanup
        let initial_temp = storage.primary_drive().unwrap().temporary_files_bytes;
        let report = storage.run_storage_sense_now();
        let final_temp = storage.primary_drive().unwrap().temporary_files_bytes;
        assert!(final_temp <= initial_temp);
        assert!(report.total_freed_bytes > 0);
    }

    #[test]
    fn test_settings_serialization() {
        let mut original = SystemSettings::new();
        original.display.primary_monitor.resolution = Resolution::RES_2560X1440;
        original.display.primary_monitor.scale = DisplayScale::Scale125;
        original.sound.set_master_volume(68);
        original.power.set_power_mode(PowerMode::BestPowerEfficiency);
        original.storage.storage_sense.enabled = false;
        original.notifications.dnd_enabled = true;
        original.multitasking.alt_tab_tab_behavior = AltTabTabBehavior::AllTabs;

        let json = original.to_json();
        assert!(json.contains("\"resolution\": {\"width\": 2560, \"height\": 1440}"));
        assert!(json.contains("\"master_volume\": 68"));
        assert!(json.contains("\"dnd_enabled\": true"));

        let restored = SystemSettings::from_json(&json).expect("Failed to deserialize SystemSettings");
        assert_eq!(restored.display.primary_monitor.resolution, Resolution::RES_2560X1440);
        assert_eq!(restored.display.primary_monitor.scale, DisplayScale::Scale125);
        assert_eq!(restored.sound.master_volume, 68);
        assert_eq!(restored.power.power_mode, PowerMode::BestPowerEfficiency);
        assert!(!restored.storage.storage_sense.enabled);
        assert!(restored.notifications.dnd_enabled);
        assert_eq!(restored.multitasking.alt_tab_tab_behavior, AltTabTabBehavior::AllTabs);
    }

    #[test]
    fn test_night_light_and_scaling() {
        let mut night_light = NightLightSettings::new();
        assert!(!night_light.enabled);
        assert_eq!(night_light.color_temperature_k, 4500);

        // Clamping tests
        night_light.set_temperature(800);
        assert_eq!(night_light.color_temperature_k, 1000); // Clamped to min 1000K

        night_light.set_temperature(7500);
        assert_eq!(night_light.color_temperature_k, 6500); // Clamped to max 6500K

        night_light.enabled = true;
        assert!(night_light.is_active_at(22, 30)); // Active at night (sunset to sunrise)
        assert!(!night_light.is_active_at(14, 0));  // Inactive in afternoon

        // Display scaling factors
        assert_eq!(DisplayScale::Scale100.percentage(), 100);
        assert_eq!(DisplayScale::Scale150.factor(), 1.5);
        assert_eq!(DisplayScale::from_percentage(175).unwrap(), DisplayScale::Scale175);
        assert!(DisplayScale::from_percentage(130).is_err());
    }

    #[test]
    fn test_battery_saver_auto_activation() {
        let mut power = PowerAndBatterySettings::new();
        power.battery_status.is_plugged_in = false;
        power.battery_status.charge_percent = 15;
        power.battery_saver.auto_turn_on_threshold_percent = 20;

        // Below 20% on battery -> Battery Saver activates automatically
        assert!(power.is_battery_saver_active());

        // When plugged in -> Battery Saver deactivates
        power.battery_status.is_plugged_in = true;
        assert!(!power.is_battery_saver_active());

        // Manual override works even when plugged in
        power.battery_saver.manual_override_enabled = true;
        assert!(power.is_battery_saver_active());
    }

    #[test]
    fn test_multitasking_and_about_info() {
        let multi = MultitaskingSettings::default();
        assert!(multi.snap_windows_enabled);
        assert!(multi.snap_assist_enabled);
        assert_eq!(multi.alt_tab_tab_behavior, AltTabTabBehavior::FiveMostRecentTabs);

        let about = AboutSystemInfo::new();
        assert_eq!(about.edition, "Windows 11 Pro");
        assert_eq!(about.os_build, "22631.3000");
        assert_eq!(about.system_type, "64-bit operating system, x64-based processor");
        assert!(about.formatted_ram().contains("32.0 GB"));
        assert!(about.hybridos_kernel_version.contains("HybridOS"));
    }

    #[test]
    fn test_notifications_and_focus_session() {
        let mut notif = NotificationsAndFocusSettings::new();
        assert!(!notif.dnd_enabled);

        // Test manual DND toggle
        assert!(notif.toggle_dnd());
        assert!(notif.dnd_enabled);
        assert!(!notif.toggle_dnd());
        assert!(!notif.dnd_enabled);

        // Auto rules test
        assert!(notif.is_dnd_active(12, 0, true, false, false)); // Active while gaming
        assert!(notif.is_dnd_active(12, 0, false, true, false)); // Active while fullscreen
        assert!(!notif.is_dnd_active(12, 0, false, false, false)); // Inactive during normal daytime

        // Focus session
        let mut session = FocusSession::new(15);
        session.start(15);
        assert!(session.is_active);
        assert_eq!(session.remaining_seconds, 15 * 60);

        let finished = session.tick(10 * 60);
        assert!(!finished);
        assert_eq!(session.remaining_seconds, 5 * 60);

        let finished2 = session.tick(5 * 60);
        assert!(finished2);
        assert!(!session.is_active);
        assert_eq!(session.remaining_seconds, 0);
    }

    #[test]
    fn test_sound_devices_and_spatial_sound() {
        let mut sound = SoundSettings::new();

        // Check defaults
        assert_eq!(sound.active_output_id, "audio_out_speakers_realtek");
        assert_eq!(sound.active_input_id, "audio_in_mic_array");
        assert_eq!(sound.spatial_sound, SpatialSound::Off);

        // Switch output to headphones
        assert!(sound.select_output_device("audio_out_headphones").is_ok());
        assert_eq!(sound.active_output_id, "audio_out_headphones");
        assert!(sound.output_devices.iter().find(|d| d.id == "audio_out_headphones").unwrap().is_default);
        assert!(!sound.output_devices.iter().find(|d| d.id == "audio_out_speakers_realtek").unwrap().is_default);

        // Switch to HDMI audio
        assert!(sound.select_output_device("audio_out_hdmi").is_ok());
        assert_eq!(sound.active_output_id, "audio_out_hdmi");

        // Non-existent device
        assert!(sound.select_output_device("invalid_device_id").is_err());

        // Spatial sound modes
        sound.set_spatial_sound(SpatialSound::DolbyAtmos);
        assert_eq!(sound.spatial_sound, SpatialSound::DolbyAtmos);
        sound.set_spatial_sound(SpatialSound::WindowsSonicForHeadphones);
        assert_eq!(sound.spatial_sound, SpatialSound::WindowsSonicForHeadphones);
        assert_eq!(SpatialSound::from_str("dolby atmos").unwrap(), SpatialSound::DolbyAtmos);
        assert_eq!(SpatialSound::from_str("windows sonic").unwrap(), SpatialSound::WindowsSonicForHeadphones);
        assert_eq!(SpatialSound::from_str("off").unwrap(), SpatialSound::Off);
    }

    #[test]
    fn test_multi_monitor_topology_and_secondary_displays() {
        let mut display = DisplaySettings::new();
        assert_eq!(display.monitor_count(), 1);
        assert_eq!(display.topology, MultiMonitorTopology::Extend);

        // Add secondary display (e.g., 4K external monitor)
        let secondary = MonitorDisplayInfo::new(2, "Display 2: DELL UltraSharp U2720Q", false, Resolution::RES_3840X2160);
        display.add_secondary_monitor(secondary);
        assert_eq!(display.monitor_count(), 2);

        // Change topologies
        display.set_topology(MultiMonitorTopology::Duplicate);
        assert_eq!(display.topology, MultiMonitorTopology::Duplicate);
        assert_eq!(display.topology.description(), "Duplicate these displays");

        display.set_topology(MultiMonitorTopology::ShowOnlyOn1);
        assert_eq!(display.topology, MultiMonitorTopology::ShowOnlyOn1);

        display.set_topology(MultiMonitorTopology::ShowOnlyOn2);
        assert_eq!(display.topology, MultiMonitorTopology::ShowOnlyOn2);

        // Test refresh rate selection
        display.set_refresh_rate(RefreshRate::Hz144);
        assert_eq!(display.primary_monitor.refresh_rate.hz(), 144);
        display.set_refresh_rate(RefreshRate::Hz240);
        assert_eq!(display.primary_monitor.refresh_rate.hz(), 240);

        // Orientation changes
        display.set_orientation(DisplayOrientation::Portrait);
        assert_eq!(display.primary_monitor.orientation.rotation_degrees(), 90);
        display.set_orientation(DisplayOrientation::LandscapeFlipped);
        assert_eq!(display.primary_monitor.orientation.rotation_degrees(), 180);

        // Remove secondary monitor
        assert!(display.remove_secondary_monitor(2));
        assert_eq!(display.monitor_count(), 1);
    }
}
