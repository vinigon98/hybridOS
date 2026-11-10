//! HybridOS Windows 11 Commercial Settings App - Time, Language, Gaming & Accessibility Categories
//!
//! Conforms to Windows 11 commercial UI specifications and Windows NT registry schemas
//! (`ms-settings:dateandtime`, `ms-settings:regionformatting`, `ms-settings:gaming-gamebar`,
//! `ms-settings:gaming-gamemode`, `ms-settings:display-advancedgraphics`, `ms-settings:easeofaccess-display`).
//!
//! 100% Pure, Safe, and Robust Rust.

use std::fmt;

// ============================================================================
// 1. TIME & LANGUAGE CATEGORY
// ============================================================================

/// Represents days of the week in standard Windows order (0 = Sunday).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DayOfWeek {
    Sunday = 0,
    Monday = 1,
    Tuesday = 2,
    Wednesday = 3,
    Thursday = 4,
    Friday = 5,
    Saturday = 6,
}

impl DayOfWeek {
    pub fn name_en(&self) -> &'static str {
        match self {
            Self::Sunday => "Sunday",
            Self::Monday => "Monday",
            Self::Tuesday => "Tuesday",
            Self::Wednesday => "Wednesday",
            Self::Thursday => "Thursday",
            Self::Friday => "Friday",
            Self::Saturday => "Saturday",
        }
    }

    pub fn name_pt(&self) -> &'static str {
        match self {
            Self::Sunday => "Domingo",
            Self::Monday => "Segunda-feira",
            Self::Tuesday => "Terça-feira",
            Self::Wednesday => "Quarta-feira",
            Self::Thursday => "Quinta-feira",
            Self::Friday => "Sexta-feira",
            Self::Saturday => "Sábado",
        }
    }
}

/// Calendar type for regional formatting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CalendarType {
    Gregorian,
    GregorianTranslated,
    Hebrew,
    Hijri,
    JapaneseEmperor,
}

/// Full date and time structure for display, NTP sync, and manual picking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KernelDateTime {
    pub year: i32,
    pub month: u8,       // 1..=12
    pub day: u8,         // 1..=31
    pub hour: u8,        // 0..=23
    pub minute: u8,      // 0..=59
    pub second: u8,      // 0..=59
    pub millisecond: u16, // 0..=999
}

impl KernelDateTime {
    pub const fn new(year: i32, month: u8, day: u8, hour: u8, minute: u8, second: u8, millisecond: u16) -> Self {
        Self {
            year,
            month,
            day,
            hour,
            minute,
            second,
            millisecond,
        }
    }

    /// Default epoch baseline: 2026-11-10 12:00:00 UTC
    pub const fn baseline() -> Self {
        Self {
            year: 2026,
            month: 11,
            day: 10,
            hour: 12,
            minute: 0,
            second: 0,
            millisecond: 0,
        }
    }

    pub fn is_leap_year(year: i32) -> bool {
        (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
    }

    pub fn days_in_month(year: i32, month: u8) -> u8 {
        match month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 => {
                if Self::is_leap_year(year) {
                    29
                } else {
                    28
                }
            }
            _ => 30,
        }
    }

    pub fn is_valid(&self) -> bool {
        if self.year < 1601 || self.year > 9999 {
            return false;
        }
        if self.month < 1 || self.month > 12 {
            return false;
        }
        if self.day < 1 || self.day > Self::days_in_month(self.year, self.month) {
            return false;
        }
        if self.hour > 23 || self.minute > 59 || self.second > 59 || self.millisecond > 999 {
            return false;
        }
        true
    }

    /// Calculate Day of Week using Sakamoto's algorithm.
    pub fn day_of_week(&self) -> DayOfWeek {
        static T: [i32; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
        let mut y = self.year;
        if self.month < 3 {
            y -= 1;
        }
        let m_idx = (self.month.saturating_sub(1) % 12) as usize;
        let d = (y + y / 4 - y / 100 + y / 400 + T[m_idx] + self.day as i32) % 7;
        let d = if d < 0 { d + 7 } else { d };
        match d {
            0 => DayOfWeek::Sunday,
            1 => DayOfWeek::Monday,
            2 => DayOfWeek::Tuesday,
            3 => DayOfWeek::Wednesday,
            4 => DayOfWeek::Thursday,
            5 => DayOfWeek::Friday,
            _ => DayOfWeek::Saturday,
        }
    }

    /// Convert Unix epoch seconds (since 1970-01-01 00:00:00 UTC) to `KernelDateTime`.
    pub fn from_unix_epoch_seconds(epoch: i64) -> Self {
        let days = (epoch / 86400) + 719468; // Days relative to 0000-03-01
        let secs_of_day = (epoch % 86400).rem_euclid(86400);

        let hour = (secs_of_day / 3600) as u8;
        let minute = ((secs_of_day % 3600) / 60) as u8;
        let second = (secs_of_day % 60) as u8;

        let era = (if days >= 0 { days } else { days - 146096 }) / 146097;
        let doe = (days - era * 146097) as u32;
        let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
        let mut y = (yoe as i32) + (era as i32) * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = (doy - (153 * mp + 2) / 5 + 1) as u8;
        let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u8;
        if m <= 2 {
            y += 1;
        }

        Self {
            year: y,
            month: m,
            day: d,
            hour,
            minute,
            second,
            millisecond: 0,
        }
    }

    /// Convert `KernelDateTime` to Unix epoch seconds.
    pub fn to_unix_epoch_seconds(&self) -> i64 {
        let y = self.year - if self.month <= 2 { 1 } else { 0 };
        let era = (if y >= 0 { y } else { y - 399 }) / 400;
        let yoe = (y - era * 400) as u32;
        let m = self.month as u32;
        let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + (self.day as u32) - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        let days = (era as i64) * 146097 + (doe as i64) - 719468;
        days * 86400 + (self.hour as i64) * 3600 + (self.minute as i64) * 60 + (self.second as i64)
    }

    /// Add an offset in minutes (can be negative).
    pub fn add_minutes(&self, minutes: i32) -> Self {
        let secs = self.to_unix_epoch_seconds() + (minutes as i64) * 60;
        let mut res = Self::from_unix_epoch_seconds(secs);
        res.millisecond = self.millisecond;
        res
    }
}

impl fmt::Display for KernelDateTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
            self.year, self.month, self.day, self.hour, self.minute, self.second
        )
    }
}

/// NTP Synchronization status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NtpSyncStatus {
    NeverSynced,
    Synchronized,
    SyncFailedTimeout,
    SyncFailedNetworkUnreachable,
    SyncInProgress,
}

impl NtpSyncStatus {
    pub fn message(&self) -> &'static str {
        match self {
            Self::NeverSynced => "Time has not been synchronized yet.",
            Self::Synchronized => "The clock was successfully synchronized with the NTP time server.",
            Self::SyncFailedTimeout => "An error occurred while Windows was synchronizing with the time server (Connection timed out).",
            Self::SyncFailedNetworkUnreachable => "Network interface unreachable. Check your internet connection.",
            Self::SyncInProgress => "Contacting time server...",
        }
    }
}

/// Representation of a standard Windows Time Zone.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TimeZoneInfo {
    pub id: &'static str,
    pub display_name: &'static str,
    /// Offset from UTC in minutes (e.g. -180 for UTC-03:00, +540 for UTC+09:00).
    pub standard_bias_minutes: i32,
    /// Whether the region observes Daylight Saving Time (DST).
    pub supports_dst: bool,
    /// Additional bias during DST (usually +60 minutes).
    pub dst_bias_minutes: i32,
    /// Whether DST is currently in effect for this time zone.
    pub dst_active: bool,
}

impl TimeZoneInfo {
    pub const fn new(
        id: &'static str,
        display_name: &'static str,
        standard_bias_minutes: i32,
        supports_dst: bool,
        dst_bias_minutes: i32,
    ) -> Self {
        Self {
            id,
            display_name,
            standard_bias_minutes,
            supports_dst,
            dst_bias_minutes,
            dst_active: false,
        }
    }

    /// Effective offset considering standard bias and active DST.
    pub fn effective_offset_minutes(&self) -> i32 {
        if self.supports_dst && self.dst_active {
            self.standard_bias_minutes + self.dst_bias_minutes
        } else {
            self.standard_bias_minutes
        }
    }

    /// Formats the effective offset as `UTC+HH:mm` or `UTC-HH:mm`.
    pub fn formatted_offset(&self) -> String {
        let eff = self.effective_offset_minutes();
        let sign = if eff >= 0 { '+' } else { '-' };
        let abs_min = eff.abs();
        let hours = abs_min / 60;
        let mins = abs_min % 60;
        format!("UTC{}{:02}:{:02}", sign, hours, mins)
    }
}

/// Registry of standard time zones from UTC-12:00 to UTC+14:00.
pub fn get_standard_time_zones() -> Vec<TimeZoneInfo> {
    vec![
        TimeZoneInfo::new("Dateline Standard Time", "(UTC-12:00) International Date Line West", -720, false, 0),
        TimeZoneInfo::new("Samoa Standard Time", "(UTC-11:00) Midway Island, American Samoa", -660, false, 0),
        TimeZoneInfo::new("Hawaiian Standard Time", "(UTC-10:00) Hawaii", -600, false, 0),
        TimeZoneInfo::new("Alaskan Standard Time", "(UTC-09:00) Alaska", -540, true, 60),
        TimeZoneInfo::new("Pacific Standard Time", "(UTC-08:00) Pacific Time (US & Canada)", -480, true, 60),
        TimeZoneInfo::new("Mountain Standard Time", "(UTC-07:00) Mountain Time (US & Canada)", -420, true, 60),
        TimeZoneInfo::new("Central Standard Time", "(UTC-06:00) Central Time (US & Canada)", -360, true, 60),
        TimeZoneInfo::new("Eastern Standard Time", "(UTC-05:00) Eastern Time (US & Canada)", -300, true, 60),
        TimeZoneInfo::new("Atlantic Standard Time", "(UTC-04:00) Atlantic Time (Canada)", -240, true, 60),
        TimeZoneInfo::new("E. South America Standard Time", "(UTC-03:00) Brasilia, Sao Paulo", -180, true, 60),
        TimeZoneInfo::new("Argentina Standard Time", "(UTC-03:00) Buenos Aires", -180, false, 0),
        TimeZoneInfo::new("Mid-Atlantic Standard Time", "(UTC-02:00) Mid-Atlantic - Old", -120, false, 0),
        TimeZoneInfo::new("Azores Standard Time", "(UTC-01:00) Azores", -60, true, 60),
        TimeZoneInfo::new("GMT Standard Time", "(UTC+00:00) Dublin, Edinburgh, Lisbon, London", 0, true, 60),
        TimeZoneInfo::new("UTC", "(UTC+00:00) Coordinated Universal Time", 0, false, 0),
        TimeZoneInfo::new("W. Europe Standard Time", "(UTC+01:00) Amsterdam, Berlin, Bern, Rome, Stockholm, Vienna", 60, true, 60),
        TimeZoneInfo::new("GTB Standard Time", "(UTC+02:00) Athens, Bucharest", 120, true, 60),
        TimeZoneInfo::new("Egypt Standard Time", "(UTC+02:00) Cairo", 120, true, 60),
        TimeZoneInfo::new("Russian Standard Time", "(UTC+03:00) Moscow, St. Petersburg", 180, false, 0),
        TimeZoneInfo::new("Arab Standard Time", "(UTC+03:00) Kuwait, Riyadh", 180, false, 0),
        TimeZoneInfo::new("Iran Standard Time", "(UTC+03:30) Tehran", 210, true, 60),
        TimeZoneInfo::new("Arabian Standard Time", "(UTC+04:00) Abu Dhabi, Muscat", 240, false, 0),
        TimeZoneInfo::new("Afghanistan Standard Time", "(UTC+04:30) Kabul", 270, false, 0),
        TimeZoneInfo::new("West Asia Standard Time", "(UTC+05:00) Islamabad, Karachi, Tashkent", 300, false, 0),
        TimeZoneInfo::new("India Standard Time", "(UTC+05:30) Chennai, Kolkata, Mumbai, New Delhi", 330, false, 0),
        TimeZoneInfo::new("Nepal Standard Time", "(UTC+05:45) Kathmandu", 345, false, 0),
        TimeZoneInfo::new("Central Asia Standard Time", "(UTC+06:00) Astana, Dhaka", 360, false, 0),
        TimeZoneInfo::new("Myanmar Standard Time", "(UTC+06:30) Yangon (Rangoon)", 390, false, 0),
        TimeZoneInfo::new("SE Asia Standard Time", "(UTC+07:00) Bangkok, Hanoi, Jakarta", 420, false, 0),
        TimeZoneInfo::new("China Standard Time", "(UTC+08:00) Beijing, Chongqing, Hong Kong, Urumqi", 480, false, 0),
        TimeZoneInfo::new("Singapore Standard Time", "(UTC+08:00) Singapore, Kuala Lumpur", 480, false, 0),
        TimeZoneInfo::new("Tokyo Standard Time", "(UTC+09:00) Osaka, Sapporo, Tokyo, Seoul", 540, false, 0),
        TimeZoneInfo::new("AUS Central Standard Time", "(UTC+09:30) Adelaide, Darwin", 570, true, 60),
        TimeZoneInfo::new("AUS Eastern Standard Time", "(UTC+10:00) Canberra, Melbourne, Sydney, Brisbane", 600, true, 60),
        TimeZoneInfo::new("Vladivostok Standard Time", "(UTC+11:00) Vladivostok, Magadan", 660, false, 0),
        TimeZoneInfo::new("New Zealand Standard Time", "(UTC+12:00) Auckland, Wellington", 720, true, 60),
        TimeZoneInfo::new("Fiji Standard Time", "(UTC+12:00) Fiji, Marshall Is.", 720, true, 60),
        TimeZoneInfo::new("Tonga Standard Time", "(UTC+13:00) Nuku'alofa, Samoa", 780, false, 0),
        TimeZoneInfo::new("Line Islands Standard Time", "(UTC+14:00) Kiritimati Island", 840, false, 0),
    ]
}

/// Taskbar additional clock model.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AdditionalClock {
    pub enabled: bool,
    pub time_zone_id: String,
    pub display_name: String,
}

impl Default for AdditionalClock {
    fn default() -> Self {
        Self {
            enabled: false,
            time_zone_id: "UTC".to_string(),
            display_name: "Clock 1".to_string(),
        }
    }
}

/// Date and Time Settings Subsystem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DateAndTimeSettings {
    /// Toggle to synchronize time automatically via NTP.
    pub set_time_automatically: bool,
    /// NTP server endpoint (default: time.windows.com).
    pub ntp_server: String,
    /// Last successful NTP sync Unix timestamp.
    pub last_sync_epoch: Option<i64>,
    /// Last sync status result.
    pub last_sync_status: NtpSyncStatus,
    /// Currently selected time zone.
    pub time_zone: TimeZoneInfo,
    /// Adjust for daylight saving time automatically.
    pub adjust_for_dst_automatically: bool,
    /// Manual override date/time (used when set_time_automatically is false).
    pub manual_override_time: Option<KernelDateTime>,
    /// Up to two additional taskbar clocks.
    pub additional_clocks: [AdditionalClock; 2],
}

impl Default for DateAndTimeSettings {
    fn default() -> Self {
        let default_tz = TimeZoneInfo::new(
            "E. South America Standard Time",
            "(UTC-03:00) Brasilia, Sao Paulo",
            -180,
            true,
            60,
        );

        Self {
            set_time_automatically: true,
            ntp_server: "time.windows.com".to_string(),
            last_sync_epoch: Some(1794312000), // Nov 10, 2026
            last_sync_status: NtpSyncStatus::Synchronized,
            time_zone: default_tz,
            adjust_for_dst_automatically: true,
            manual_override_time: None,
            additional_clocks: [
                AdditionalClock {
                    enabled: false,
                    time_zone_id: "GMT Standard Time".to_string(),
                    display_name: "London (GMT)".to_string(),
                },
                AdditionalClock {
                    enabled: false,
                    time_zone_id: "Tokyo Standard Time".to_string(),
                    display_name: "Tokyo (JST)".to_string(),
                },
            ],
        }
    }
}

impl DateAndTimeSettings {
    /// Select a new time zone by ID from standard catalog.
    pub fn set_time_zone_by_id(&mut self, tz_id: &str) -> Result<(), &'static str> {
        let catalog = get_standard_time_zones();
        if let Some(found) = catalog.into_iter().find(|tz| tz.id == tz_id) {
            let mut updated = found;
            if self.adjust_for_dst_automatically && updated.supports_dst {
                updated.dst_active = self.time_zone.dst_active;
            }
            self.time_zone = updated;
            Ok(())
        } else {
            Err("Time zone ID not found in system catalog")
        }
    }

    /// Set manual date and time. Fails if automatic time sync is currently enabled.
    pub fn set_manual_date_time(&mut self, dt: KernelDateTime) -> Result<(), &'static str> {
        if self.set_time_automatically {
            return Err("Cannot set date/time manually while 'Set time automatically' is enabled.");
        }
        if !dt.is_valid() {
            return Err("Invalid date/time parameters provided.");
        }
        self.manual_override_time = Some(dt);
        Ok(())
    }

    /// Perform NTP synchronization. Simulates real network handshake.
    pub fn sync_ntp_now(&mut self, simulated_remote_epoch: i64) -> Result<KernelDateTime, &'static str> {
        if !self.set_time_automatically {
            return Err("Cannot synchronize NTP when automatic time is disabled.");
        }
        if simulated_remote_epoch <= 0 {
            self.last_sync_status = NtpSyncStatus::SyncFailedNetworkUnreachable;
            return Err("NTP network unreachable");
        }

        self.last_sync_epoch = Some(simulated_remote_epoch);
        self.last_sync_status = NtpSyncStatus::Synchronized;
        self.manual_override_time = None;

        let local_dt = self.get_current_local_time(simulated_remote_epoch);
        Ok(local_dt)
    }

    /// Calculate the current local time for a given UTC epoch.
    pub fn get_current_local_time(&self, utc_epoch: i64) -> KernelDateTime {
        let base_dt = if let Some(manual) = self.manual_override_time {
            manual
        } else {
            KernelDateTime::from_unix_epoch_seconds(utc_epoch)
        };

        let offset = self.time_zone.effective_offset_minutes();
        base_dt.add_minutes(offset)
    }

    /// Calculate time for an additional clock.
    pub fn get_additional_clock_time(&self, clock_idx: usize, utc_epoch: i64) -> Option<(String, KernelDateTime)> {
        if clock_idx >= self.additional_clocks.len() {
            return None;
        }
        let clock = &self.additional_clocks[clock_idx];
        if !clock.enabled {
            return None;
        }

        let catalog = get_standard_time_zones();
        let tz = catalog.into_iter().find(|t| t.id == clock.time_zone_id)?;
        let utc_dt = KernelDateTime::from_unix_epoch_seconds(utc_epoch);
        let clock_dt = utc_dt.add_minutes(tz.effective_offset_minutes());
        Some((clock.display_name.clone(), clock_dt))
    }
}

// ----------------------------------------------------------------------------
// Language & Regional Formatting
// ----------------------------------------------------------------------------

/// Windows Display Language options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WindowsDisplayLanguage {
    PtBr, // Português (Brasil)
    EnUs, // English (United States)
    EsEs, // Español (España)
    DeDe, // Deutsch (Deutschland)
    JaJp, // 日本語 (日本)
    FrFr, // Français (France)
    ZhCn, // 中文 (简体, 中国)
}

impl WindowsDisplayLanguage {
    pub fn bcp47(&self) -> &'static str {
        match self {
            Self::PtBr => "pt-BR",
            Self::EnUs => "en-US",
            Self::EsEs => "es-ES",
            Self::DeDe => "de-DE",
            Self::JaJp => "ja-JP",
            Self::FrFr => "fr-FR",
            Self::ZhCn => "zh-CN",
        }
    }

    pub fn native_name(&self) -> &'static str {
        match self {
            Self::PtBr => "Português (Brasil)",
            Self::EnUs => "English (United States)",
            Self::EsEs => "Español (España)",
            Self::DeDe => "Deutsch (Deutschland)",
            Self::JaJp => "日本語 (日本)",
            Self::FrFr => "Français (France)",
            Self::ZhCn => "中文 (简体, 中国)",
        }
    }

    pub fn english_name(&self) -> &'static str {
        match self {
            Self::PtBr => "Portuguese (Brazil)",
            Self::EnUs => "English (United States)",
            Self::EsEs => "Spanish (Spain)",
            Self::DeDe => "German (Germany)",
            Self::JaJp => "Japanese (Japan)",
            Self::FrFr => "French (France)",
            Self::ZhCn => "Chinese (Simplified, China)",
        }
    }

    pub fn from_bcp47(tag: &str) -> Option<Self> {
        match tag.to_ascii_lowercase().as_str() {
            "pt-br" | "pt_br" | "pt" => Some(Self::PtBr),
            "en-us" | "en_us" | "en" => Some(Self::EnUs),
            "es-es" | "es_es" | "es" => Some(Self::EsEs),
            "de-de" | "de_de" | "de" => Some(Self::DeDe),
            "ja-jp" | "ja_jp" | "ja" => Some(Self::JaJp),
            "fr-fr" | "fr_fr" | "fr" => Some(Self::FrFr),
            "zh-cn" | "zh_cn" | "zh" => Some(Self::ZhCn),
            _ => None,
        }
    }
}

/// Keyboard layout definitions (Windows KLID identifiers).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyboardLayout {
    /// Brazilian ABNT2 with standard ç key. KLID 00010416
    Abnt2,
    /// United States-International with dead keys. KLID 00020409
    UsInternational,
    /// US Standard QWERTY. KLID 00000409
    UsStandard,
    /// German QWERTZ. KLID 00000407
    GermanQwertz,
    /// Japanese IME (Romaji / Kana). KLID E0010411
    JapaneseIme,
    /// Spanish (Spain) layout. KLID 0000040A
    SpanishEs,
}

impl KeyboardLayout {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Abnt2 => "Portuguese (Brazil ABNT2)",
            Self::UsInternational => "United States-International",
            Self::UsStandard => "US Standard QWERTY",
            Self::GermanQwertz => "German (Germany)",
            Self::JapaneseIme => "Japanese IME (Hiragana/Katakana)",
            Self::SpanishEs => "Spanish (Spain)",
        }
    }

    pub fn klid(&self) -> &'static str {
        match self {
            Self::Abnt2 => "00010416",
            Self::UsInternational => "00020409",
            Self::UsStandard => "00000409",
            Self::GermanQwertz => "00000407",
            Self::JapaneseIme => "E0010411",
            Self::SpanishEs => "0000040A",
        }
    }
}

/// Regional format parameters for date, time, and numbers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionalFormat {
    pub calendar: CalendarType,
    pub first_day_of_week: DayOfWeek,
    pub short_date_pattern: String,
    pub long_date_pattern: String,
    pub short_time_pattern: String,
    pub long_time_pattern: String,
}

impl RegionalFormat {
    pub fn pt_br() -> Self {
        Self {
            calendar: CalendarType::Gregorian,
            first_day_of_week: DayOfWeek::Sunday,
            short_date_pattern: "dd/MM/yyyy".to_string(),
            long_date_pattern: "dddd, d 'de' MMMM 'de' yyyy".to_string(),
            short_time_pattern: "HH:mm".to_string(),
            long_time_pattern: "HH:mm:ss".to_string(),
        }
    }

    pub fn en_us() -> Self {
        Self {
            calendar: CalendarType::Gregorian,
            first_day_of_week: DayOfWeek::Sunday,
            short_date_pattern: "M/d/yyyy".to_string(),
            long_date_pattern: "dddd, MMMM d, yyyy".to_string(),
            short_time_pattern: "h:mm tt".to_string(),
            long_time_pattern: "h:mm:ss tt".to_string(),
        }
    }

    pub fn de_de() -> Self {
        Self {
            calendar: CalendarType::Gregorian,
            first_day_of_week: DayOfWeek::Monday,
            short_date_pattern: "dd.MM.yyyy".to_string(),
            long_date_pattern: "dddd, d. MMMM yyyy".to_string(),
            short_time_pattern: "HH:mm".to_string(),
            long_time_pattern: "HH:mm:ss".to_string(),
        }
    }

    pub fn ja_jp() -> Self {
        Self {
            calendar: CalendarType::Gregorian,
            first_day_of_week: DayOfWeek::Sunday,
            short_date_pattern: "yyyy/MM/dd".to_string(),
            long_date_pattern: "yyyy'年'M'月'd'日' dddd".to_string(),
            short_time_pattern: "H:mm".to_string(),
            long_time_pattern: "H:mm:ss".to_string(),
        }
    }

    /// Formats a `KernelDateTime` into the short date pattern.
    pub fn format_short_date(&self, dt: &KernelDateTime) -> String {
        match self.short_date_pattern.as_str() {
            "dd/MM/yyyy" => format!("{:02}/{:02}/{:04}", dt.day, dt.month, dt.year),
            "M/d/yyyy" => format!("{}/{}/{:04}", dt.month, dt.day, dt.year),
            "dd.MM.yyyy" => format!("{:02}.{:02}.{:04}", dt.day, dt.month, dt.year),
            "yyyy/MM/dd" => format!("{:04}/{:02}/{:02}", dt.year, dt.month, dt.day),
            _ => format!("{:04}-{:02}-{:02}", dt.year, dt.month, dt.day),
        }
    }

    /// Formats a `KernelDateTime` into the long time pattern.
    pub fn format_long_time(&self, dt: &KernelDateTime) -> String {
        if self.long_time_pattern.contains("tt") {
            let am_pm = if dt.hour >= 12 { "PM" } else { "AM" };
            let h12 = match dt.hour % 12 {
                0 => 12,
                h => h,
            };
            format!("{:02}:{:02}:{:02} {}", h12, dt.minute, dt.second, am_pm)
        } else {
            format!("{:02}:{:02}:{:02}", dt.hour, dt.minute, dt.second)
        }
    }
}

/// Language and Region Settings Container.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageAndRegionSettings {
    pub display_language: WindowsDisplayLanguage,
    pub country_or_region: String,
    pub country_code_iso2: String,
    pub regional_format: RegionalFormat,
    pub active_keyboard_layout: KeyboardLayout,
    pub installed_keyboard_layouts: Vec<KeyboardLayout>,
}

impl Default for LanguageAndRegionSettings {
    fn default() -> Self {
        Self {
            display_language: WindowsDisplayLanguage::PtBr,
            country_or_region: "Brazil".to_string(),
            country_code_iso2: "BR".to_string(),
            regional_format: RegionalFormat::pt_br(),
            active_keyboard_layout: KeyboardLayout::Abnt2,
            installed_keyboard_layouts: vec![
                KeyboardLayout::Abnt2,
                KeyboardLayout::UsInternational,
            ],
        }
    }
}

impl LanguageAndRegionSettings {
    pub fn switch_keyboard_layout(&mut self, target: KeyboardLayout) -> Result<(), &'static str> {
        if !self.installed_keyboard_layouts.contains(&target) {
            return Err("Keyboard layout is not installed in the system.");
        }
        self.active_keyboard_layout = target;
        Ok(())
    }

    pub fn install_keyboard_layout(&mut self, layout: KeyboardLayout) {
        if !self.installed_keyboard_layouts.contains(&layout) {
            self.installed_keyboard_layouts.push(layout);
        }
    }

    pub fn set_display_language(&mut self, lang: WindowsDisplayLanguage) {
        self.display_language = lang;
        match lang {
            WindowsDisplayLanguage::PtBr => {
                self.regional_format = RegionalFormat::pt_br();
                self.country_or_region = "Brazil".to_string();
                self.country_code_iso2 = "BR".to_string();
            }
            WindowsDisplayLanguage::EnUs => {
                self.regional_format = RegionalFormat::en_us();
                self.country_or_region = "United States".to_string();
                self.country_code_iso2 = "US".to_string();
            }
            WindowsDisplayLanguage::DeDe => {
                self.regional_format = RegionalFormat::de_de();
                self.country_or_region = "Germany".to_string();
                self.country_code_iso2 = "DE".to_string();
            }
            WindowsDisplayLanguage::JaJp => {
                self.regional_format = RegionalFormat::ja_jp();
                self.country_or_region = "Japan".to_string();
                self.country_code_iso2 = "JP".to_string();
            }
            _ => {}
        }
    }
}

// ============================================================================
// 2. GAMING SETTINGS CATEGORY
// ============================================================================

/// Video capture frame rate options for Game DVR.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VideoFrameRate {
    Fps30 = 30,
    Fps60 = 60,
}

/// Video quality options for Game DVR.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VideoQuality {
    Standard,
    High,
}

/// Xbox Game Bar configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XboxGameBarSettings {
    /// Open Xbox Game Bar using shortcut (Win+G) toggle.
    pub open_game_bar_shortcut_enabled: bool,
    /// Open Xbox Game Bar using controller guide button.
    pub open_with_controller_guide_button: bool,
    /// List of overlay pinned widgets.
    pub pinned_widgets: Vec<String>,
}

impl Default for XboxGameBarSettings {
    fn default() -> Self {
        Self {
            open_game_bar_shortcut_enabled: true,
            open_with_controller_guide_button: true,
            pinned_widgets: vec![
                "Audio Mixer".to_string(),
                "Performance Monitor".to_string(),
                "Capture Widget".to_string(),
            ],
        }
    }
}

/// Captures & Game DVR configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturesSettings {
    /// Record what happened (background recording toggle).
    pub record_what_happened: bool,
    /// Background recording buffer duration in seconds (30s..600s).
    pub background_recording_duration_secs: u32,
    /// Video frame rate (30fps or 60fps).
    pub video_frame_rate: VideoFrameRate,
    /// Video quality (Standard or High).
    pub video_quality: VideoQuality,
    /// Record audio when recording a game.
    pub record_audio_enabled: bool,
    /// Captures directory path.
    pub captures_folder: String,
}

impl Default for CapturesSettings {
    fn default() -> Self {
        Self {
            record_what_happened: true,
            background_recording_duration_secs: 30, // 30 seconds default
            video_frame_rate: VideoFrameRate::Fps60,
            video_quality: VideoQuality::High,
            record_audio_enabled: true,
            captures_folder: "C:\\Users\\Vinicius\\Videos\\Captures".to_string(),
        }
    }
}

impl CapturesSettings {
    pub fn set_recording_duration(&mut self, seconds: u32) -> Result<(), &'static str> {
        match seconds {
            30 | 60 | 120 | 180 | 300 | 600 => {
                self.background_recording_duration_secs = seconds;
                Ok(())
            }
            _ => Err("Invalid recording duration. Allowed: 30s, 60s, 2m, 3m, 5m, 10m (30..600s)."),
        }
    }
}

/// Game Mode Subsystem state & kernel scheduler optimizer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameModeSettings {
    /// Master toggle for Game Mode.
    pub game_mode_enabled: bool,
    /// Currently active gaming process ID receiving thread optimization.
    pub active_game_pid: Option<u32>,
    /// Background tasks suppression flag.
    pub background_tasks_throttled: bool,
    /// CPU core affinity pinned to Performance cores (P-cores).
    pub p_cores_priority_allocated: bool,
    /// GPU priority queue elevated to High Priority.
    pub gpu_high_priority_active: bool,
}

impl Default for GameModeSettings {
    fn default() -> Self {
        Self {
            game_mode_enabled: true,
            active_game_pid: None,
            background_tasks_throttled: false,
            p_cores_priority_allocated: false,
            gpu_high_priority_active: false,
        }
    }
}

impl GameModeSettings {
    /// Optimize CPU/GPU thread allocation and suppress background services when game launches.
    pub fn on_game_launched(&mut self, pid: u32) {
        if !self.game_mode_enabled {
            return;
        }
        self.active_game_pid = Some(pid);
        self.background_tasks_throttled = true;
        self.p_cores_priority_allocated = true;
        self.gpu_high_priority_active = true;
    }

    /// Revert CPU/GPU priorities when game closes.
    pub fn on_game_terminated(&mut self) {
        self.active_game_pid = None;
        self.background_tasks_throttled = false;
        self.p_cores_priority_allocated = false;
        self.gpu_high_priority_active = false;
    }

    pub fn set_game_mode(&mut self, enabled: bool) {
        self.game_mode_enabled = enabled;
        if !enabled {
            self.on_game_terminated();
        }
    }
}

/// Graphics performance preference setting per application.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GraphicsPreference {
    LetWindowsDecide = 0,
    PowerSaving = 1,       // Integrated GPU
    HighPerformance = 2,   // Discrete GPU (NVIDIA / AMD)
    SpecificGpu = 3,       // Specific assigned adapter index
}

impl GraphicsPreference {
    pub fn label(&self) -> &'static str {
        match self {
            Self::LetWindowsDecide => "Let Windows decide",
            Self::PowerSaving => "Power saving",
            Self::HighPerformance => "High performance",
            Self::SpecificGpu => "Specific GPU",
        }
    }
}

/// Type of application registered for graphics preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ApplicationType {
    DesktopApp,
    MicrosoftStoreApp,
}

/// Entry for graphics performance preference per app.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppGraphicsPreference {
    pub app_name: String,
    pub executable_path: String,
    pub app_type: ApplicationType,
    pub preference: GraphicsPreference,
}

/// Graphics Performance Settings Subsystem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphicsSettings {
    /// Hardware-accelerated GPU scheduling (HAGS).
    pub hardware_accelerated_gpu_scheduling: bool,
    /// Variable refresh rate (VRR) / G-Sync / FreeSync.
    pub variable_refresh_rate: bool,
    /// Auto HDR toggle.
    pub auto_hdr: bool,
    /// Per-application graphics preference entries.
    pub app_preferences: Vec<AppGraphicsPreference>,
}

impl Default for GraphicsSettings {
    fn default() -> Self {
        Self {
            hardware_accelerated_gpu_scheduling: true,
            variable_refresh_rate: true,
            auto_hdr: true,
            app_preferences: vec![
                AppGraphicsPreference {
                    app_name: "Cyberpunk 2077".to_string(),
                    executable_path: "C:\\Games\\Cyberpunk2077\\bin\\x64\\Cyberpunk2077.exe".to_string(),
                    app_type: ApplicationType::DesktopApp,
                    preference: GraphicsPreference::HighPerformance,
                },
                AppGraphicsPreference {
                    app_name: "Microsoft Edge".to_string(),
                    executable_path: "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe".to_string(),
                    app_type: ApplicationType::DesktopApp,
                    preference: GraphicsPreference::PowerSaving,
                },
            ],
        }
    }
}

impl GraphicsSettings {
    pub fn add_or_update_app(
        &mut self,
        app_name: &str,
        path: &str,
        app_type: ApplicationType,
        preference: GraphicsPreference,
    ) {
        if let Some(existing) = self.app_preferences.iter_mut().find(|a| a.executable_path.eq_ignore_ascii_case(path)) {
            existing.preference = preference;
            existing.app_name = app_name.to_string();
            existing.app_type = app_type;
        } else {
            self.app_preferences.push(AppGraphicsPreference {
                app_name: app_name.to_string(),
                executable_path: path.to_string(),
                app_type,
                preference,
            });
        }
    }

    pub fn remove_app(&mut self, path: &str) -> bool {
        let initial_len = self.app_preferences.len();
        self.app_preferences.retain(|a| !a.executable_path.eq_ignore_ascii_case(path));
        self.app_preferences.len() < initial_len
    }

    pub fn get_preference_for_app(&self, path: &str) -> GraphicsPreference {
        self.app_preferences
            .iter()
            .find(|a| a.executable_path.eq_ignore_ascii_case(path))
            .map(|a| a.preference)
            .unwrap_or(GraphicsPreference::LetWindowsDecide)
    }
}

/// Aggregated Gaming Settings Category.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GamingSettings {
    pub game_bar: XboxGameBarSettings,
    pub captures: CapturesSettings,
    pub game_mode: GameModeSettings,
    pub graphics: GraphicsSettings,
}

// ============================================================================
// 3. ACCESSIBILITY CATEGORY
// ============================================================================

/// Notification dismissal timeout options in Windows 11.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NotificationTimeout {
    Sec5 = 5,
    Sec7 = 7,
    Sec15 = 15,
    Sec30 = 30,
    Min1 = 60,
    Min5 = 300,
}

impl NotificationTimeout {
    pub fn seconds(&self) -> u32 {
        *self as u32
    }

    pub fn display_label(&self) -> &'static str {
        match self {
            Self::Sec5 => "5 seconds",
            Self::Sec7 => "7 seconds",
            Self::Sec15 => "15 seconds",
            Self::Sec30 => "30 seconds",
            Self::Min1 => "1 minute",
            Self::Min5 => "5 minutes",
        }
    }

    pub fn from_seconds(s: u32) -> Option<Self> {
        match s {
            5 => Some(Self::Sec5),
            7 => Some(Self::Sec7),
            15 => Some(Self::Sec15),
            30 => Some(Self::Sec30),
            60 => Some(Self::Min1),
            300 => Some(Self::Min5),
            _ => None,
        }
    }
}

/// Mouse pointer visual styles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PointerStyle {
    White,
    Black,
    Inverted,
    CustomColor(u32), // 0x00RRGGBB
}

/// Vision Accessibility Settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisionSettings {
    /// Text size percentage slider: 100%..225%
    pub text_size_percent: u32,
    /// Always show scrollbars toggle.
    pub always_show_scrollbars: bool,
    /// Transparency effects toggle (Acrylic & Mica).
    pub transparency_effects: bool,
    /// Animation effects toggle.
    pub animation_effects: bool,
    /// Notification dismissal timeout.
    pub notification_timeout: NotificationTimeout,
    /// Mouse pointer style.
    pub pointer_style: PointerStyle,
    /// Mouse pointer size slider: 1..15.
    pub pointer_size: u32,
    /// Touch indicator ripple effect toggle.
    pub touch_indicator_enabled: bool,
}

impl Default for VisionSettings {
    fn default() -> Self {
        Self {
            text_size_percent: 100,
            always_show_scrollbars: false,
            transparency_effects: true,
            animation_effects: true,
            notification_timeout: NotificationTimeout::Sec5,
            pointer_style: PointerStyle::White,
            pointer_size: 1,
            touch_indicator_enabled: true,
        }
    }
}

impl VisionSettings {
    pub fn set_text_size_percent(&mut self, percent: u32) -> Result<(), &'static str> {
        if !(100..=225).contains(&percent) {
            return Err("Text size must be within 100% and 225%.");
        }
        self.text_size_percent = percent;
        Ok(())
    }

    /// Scaling factor for UI typography (e.g. 1.0 for 100%, 2.25 for 225%).
    pub fn text_scale_factor(&self) -> f32 {
        self.text_size_percent as f32 / 100.0
    }

    pub fn set_pointer_size(&mut self, size: u32) -> Result<(), &'static str> {
        if !(1..=15).contains(&size) {
            return Err("Pointer size must be between 1 and 15.");
        }
        self.pointer_size = size;
        Ok(())
    }
}

/// Flash screen notification options during audio events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AudioNotificationFlash {
    Never = 0,
    FlashTitleBar = 1,
    FlashActiveWindow = 2,
    FlashEntireScreen = 3,
}

/// Closed caption styles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CaptionStyle {
    WhiteOnBlack,
    LargeText,
    YellowOnBlue,
    SmallCaps,
}

/// Hearing Accessibility Settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HearingSettings {
    /// Mono audio toggle: Combines left and right audio channels into one.
    pub mono_audio_enabled: bool,
    /// Flash screen during audio notifications option.
    pub flash_screen: AudioNotificationFlash,
    /// Selected caption appearance style.
    pub caption_style: CaptionStyle,
}

impl Default for HearingSettings {
    fn default() -> Self {
        Self {
            mono_audio_enabled: false,
            flash_screen: AudioNotificationFlash::Never,
            caption_style: CaptionStyle::WhiteOnBlack,
        }
    }
}

/// Interaction Accessibility Settings (Speech, Keyboard).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InteractionSettings {
    /// Windows Speech Recognition toggle.
    pub speech_recognition_enabled: bool,
    /// Voice typing shortcut (Win+H) enabled.
    pub voice_typing_shortcut_enabled: bool,
    /// Speech recognition primary language.
    pub speech_language: String,

    // Keyboard accessibility options
    /// Sticky Keys: Press one key at a time for keyboard shortcuts.
    pub sticky_keys_enabled: bool,
    /// Filter Keys: Ignore brief or repeated keystrokes.
    pub filter_keys_enabled: bool,
    /// Toggle Keys: Play a sound when Caps Lock, Num Lock, or Scroll Lock is pressed.
    pub toggle_keys_enabled: bool,
    /// On-screen keyboard active.
    pub on_screen_keyboard_active: bool,

    // Hardware shortcut bindings
    /// Press Shift key 5 times to turn on Sticky Keys.
    pub shift_five_times_sticky_keys: bool,
    /// Hold right Shift key for 8 seconds to turn on Filter Keys.
    pub hold_right_shift_filter_keys: bool,
}

impl Default for InteractionSettings {
    fn default() -> Self {
        Self {
            speech_recognition_enabled: false,
            voice_typing_shortcut_enabled: true,
            speech_language: "pt-BR".to_string(),
            sticky_keys_enabled: false,
            filter_keys_enabled: false,
            toggle_keys_enabled: false,
            on_screen_keyboard_active: false,
            shift_five_times_sticky_keys: true,
            hold_right_shift_filter_keys: true,
        }
    }
}

/// Aggregated Accessibility Category.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AccessibilitySettings {
    pub vision: VisionSettings,
    pub hearing: HearingSettings,
    pub interaction: InteractionSettings,
}

// ============================================================================
// 4. MASTER SETTINGS CONTAINER & REGISTRY INTEROPERABILITY
// ============================================================================

/// Master settings controller for Time, Language, Gaming, and Accessibility.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TimeGamingAccessSettings {
    pub date_and_time: DateAndTimeSettings,
    pub language_and_region: LanguageAndRegionSettings,
    pub gaming: GamingSettings,
    pub accessibility: AccessibilitySettings,
}

impl TimeGamingAccessSettings {
    pub fn new() -> Self {
        Self::default()
    }

    /// Exports settings into standard Windows Registry keys and values.
    /// Returns vector of tuples: (Registry Key Path, Value Name, Value Data String).
    pub fn export_to_windows_registry(&self) -> Vec<(String, String, String)> {
        let mut reg = Vec::new();

        // 1. Time & NTP
        reg.push((
            "HKLM\\SYSTEM\\CurrentControlSet\\Services\\W32Time\\Parameters".to_string(),
            "NtpServer".to_string(),
            format!("{},0x9", self.date_and_time.ntp_server),
        ));
        reg.push((
            "HKLM\\SYSTEM\\CurrentControlSet\\Services\\W32Time\\Parameters".to_string(),
            "Type".to_string(),
            if self.date_and_time.set_time_automatically { "NTP" } else { "NoSync" }.to_string(),
        ));
        reg.push((
            "HKLM\\SYSTEM\\CurrentControlSet\\Control\\TimeZoneInformation".to_string(),
            "TimeZoneKeyName".to_string(),
            self.date_and_time.time_zone.id.to_string(),
        ));
        reg.push((
            "HKLM\\SYSTEM\\CurrentControlSet\\Control\\TimeZoneInformation".to_string(),
            "DynamicDaylightTimeDisabled".to_string(),
            if self.date_and_time.adjust_for_dst_automatically { "0" } else { "1" }.to_string(),
        ));

        // 2. Language & Regional Formats
        reg.push((
            "HKCU\\Control Panel\\International".to_string(),
            "LocaleName".to_string(),
            self.language_and_region.display_language.bcp47().to_string(),
        ));
        reg.push((
            "HKCU\\Control Panel\\International".to_string(),
            "sShortDate".to_string(),
            self.language_and_region.regional_format.short_date_pattern.clone(),
        ));
        reg.push((
            "HKCU\\Control Panel\\International".to_string(),
            "sTimeFormat".to_string(),
            self.language_and_region.regional_format.long_time_pattern.clone(),
        ));
        reg.push((
            "HKCU\\Keyboard Layout\\Preload".to_string(),
            "1".to_string(),
            self.language_and_region.active_keyboard_layout.klid().to_string(),
        ));

        // 3. Gaming & Game Mode
        reg.push((
            "HKCU\\Software\\Microsoft\\GameBar".to_string(),
            "UseNexusForGameBarEnabled".to_string(),
            if self.gaming.game_bar.open_game_bar_shortcut_enabled { "1" } else { "0" }.to_string(),
        ));
        reg.push((
            "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\GameDVR".to_string(),
            "AppCaptureEnabled".to_string(),
            if self.gaming.captures.record_what_happened { "1" } else { "0" }.to_string(),
        ));
        reg.push((
            "HKCU\\Software\\Microsoft\\GameBar".to_string(),
            "AutoGameModeEnabled".to_string(),
            if self.gaming.game_mode.game_mode_enabled { "1" } else { "0" }.to_string(),
        ));

        // 4. Accessibility
        reg.push((
            "HKCU\\Software\\Microsoft\\Accessibility".to_string(),
            "TextScaleFactor".to_string(),
            self.accessibility.vision.text_size_percent.to_string(),
        ));
        reg.push((
            "HKCU\\Control Panel\\Desktop".to_string(),
            "CursorSize".to_string(),
            self.accessibility.vision.pointer_size.to_string(),
        ));
        reg.push((
            "HKCU\\Control Panel\\Accessibility\\SoundSentry".to_string(),
            "Flags".to_string(),
            (self.accessibility.hearing.flash_screen as u32).to_string(),
        ));
        reg.push((
            "HKCU\\Control Panel\\Accessibility\\StickyKeys".to_string(),
            "Flags".to_string(),
            if self.accessibility.interaction.sticky_keys_enabled { "511" } else { "506" }.to_string(),
        ));

        reg
    }
}

// ============================================================================
// 5. COMPREHENSIVE UNIT TESTS (MINIMUM 6 TESTS)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Test 1: Time zone calculations, UTC offsets, and Daylight Saving Time adjustment.
    #[test]
    fn test_time_zone_calculation() {
        let zones = get_standard_time_zones();
        assert!(zones.len() >= 35, "Should have a comprehensive list of standard time zones");

        // UTC-03:00 Brasilia
        let mut brasilia = zones.iter().find(|tz| tz.id == "E. South America Standard Time").unwrap().clone();
        assert_eq!(brasilia.standard_bias_minutes, -180);
        assert_eq!(brasilia.effective_offset_minutes(), -180);
        assert_eq!(brasilia.formatted_offset(), "UTC-03:00");

        // Enable DST (+60 min)
        brasilia.dst_active = true;
        assert_eq!(brasilia.effective_offset_minutes(), -120);
        assert_eq!(brasilia.formatted_offset(), "UTC-02:00");

        // UTC+09:00 Tokyo
        let tokyo = zones.iter().find(|tz| tz.id == "Tokyo Standard Time").unwrap();
        assert_eq!(tokyo.standard_bias_minutes, 540);
        assert_eq!(tokyo.effective_offset_minutes(), 540);
        assert_eq!(tokyo.formatted_offset(), "UTC+09:00");

        // UTC+14:00 Line Islands (Extreme positive)
        let kiritimati = zones.iter().find(|tz| tz.id == "Line Islands Standard Time").unwrap();
        assert_eq!(kiritimati.standard_bias_minutes, 840);
        assert_eq!(kiritimati.formatted_offset(), "UTC+14:00");

        // UTC-12:00 Dateline West (Extreme negative)
        let dateline = zones.iter().find(|tz| tz.id == "Dateline Standard Time").unwrap();
        assert_eq!(dateline.standard_bias_minutes, -720);
        assert_eq!(dateline.formatted_offset(), "UTC-12:00");
    }

    /// Test 2: Regional date formatting across locales (pt-BR, en-US, de-DE, ja-JP).
    #[test]
    fn test_regional_date_formatting() {
        let dt = KernelDateTime::new(2026, 11, 10, 14, 30, 45, 0);
        assert!(dt.is_valid());

        // pt-BR: dd/MM/yyyy
        let fmt_br = RegionalFormat::pt_br();
        assert_eq!(fmt_br.format_short_date(&dt), "10/11/2026");
        assert_eq!(fmt_br.format_long_time(&dt), "14:30:45");

        // en-US: M/d/yyyy
        let fmt_us = RegionalFormat::en_us();
        assert_eq!(fmt_us.format_short_date(&dt), "11/10/2026");
        assert_eq!(fmt_us.format_long_time(&dt), "02:30:45 PM");

        // de-DE: dd.MM.yyyy
        let fmt_de = RegionalFormat::de_de();
        assert_eq!(fmt_de.format_short_date(&dt), "10.11.2026");
        assert_eq!(fmt_de.first_day_of_week, DayOfWeek::Monday);

        // ja-JP: yyyy/MM/dd
        let fmt_jp = RegionalFormat::ja_jp();
        assert_eq!(fmt_jp.format_short_date(&dt), "2026/11/10");

        // Day of week calculation for 2026-11-10 (Tuesday)
        assert_eq!(dt.day_of_week(), DayOfWeek::Tuesday);
        assert_eq!(dt.day_of_week().name_pt(), "Terça-feira");
    }

    /// Test 3: Game Mode optimizer toggle, process affinity, and background task suppression.
    #[test]
    fn test_game_mode_optimizer_toggle() {
        let mut gm = GameModeSettings::default();
        assert!(gm.game_mode_enabled);
        assert_eq!(gm.active_game_pid, None);
        assert!(!gm.background_tasks_throttled);

        // Launch game process 4128
        gm.on_game_launched(4128);
        assert_eq!(gm.active_game_pid, Some(4128));
        assert!(gm.background_tasks_throttled);
        assert!(gm.p_cores_priority_allocated);
        assert!(gm.gpu_high_priority_active);

        // Game exits
        gm.on_game_terminated();
        assert_eq!(gm.active_game_pid, None);
        assert!(!gm.background_tasks_throttled);
        assert!(!gm.p_cores_priority_allocated);
        assert!(!gm.gpu_high_priority_active);

        // Disabling master toggle reverts active gaming optimizations
        gm.on_game_launched(9999);
        assert!(gm.background_tasks_throttled);
        gm.set_game_mode(false);
        assert!(!gm.game_mode_enabled);
        assert_eq!(gm.active_game_pid, None);
        assert!(!gm.background_tasks_throttled);
    }

    /// Test 4: Accessibility text scale slider (100%..225%) and pointer size clamping.
    #[test]
    fn test_accessibility_text_scale() {
        let mut vision = VisionSettings::default();
        assert_eq!(vision.text_size_percent, 100);
        assert!((vision.text_scale_factor() - 1.0).abs() < 1e-4);

        // Valid scaling values
        assert!(vision.set_text_size_percent(150).is_ok());
        assert!((vision.text_scale_factor() - 1.5).abs() < 1e-4);

        assert!(vision.set_text_size_percent(225).is_ok());
        assert!((vision.text_scale_factor() - 2.25).abs() < 1e-4);

        // Clamping / bounds check: < 100 or > 225 must fail
        assert!(vision.set_text_size_percent(99).is_err());
        assert!(vision.set_text_size_percent(226).is_err());
        assert!(vision.set_text_size_percent(300).is_err());

        // Pointer size: 1..15
        assert_eq!(vision.pointer_size, 1);
        assert!(vision.set_pointer_size(8).is_ok());
        assert_eq!(vision.pointer_size, 8);
        assert!(vision.set_pointer_size(15).is_ok());
        assert!(vision.set_pointer_size(0).is_err());
        assert!(vision.set_pointer_size(16).is_err());
    }

    /// Test 5: Graphics performance preference mapping and app management.
    #[test]
    fn test_graphics_performance_preference_mapping() {
        let mut gfx = GraphicsSettings::default();
        let app_path = "C:\\Games\\ForzaMotorsport\\forza.exe";

        // Querying unregistered app returns LetWindowsDecide
        assert_eq!(gfx.get_preference_for_app(app_path), GraphicsPreference::LetWindowsDecide);

        // Add high performance preference for desktop game
        gfx.add_or_update_app(
            "Forza Motorsport",
            app_path,
            ApplicationType::DesktopApp,
            GraphicsPreference::HighPerformance,
        );
        assert_eq!(gfx.get_preference_for_app(app_path), GraphicsPreference::HighPerformance);

        // Update preference to Power Saving
        gfx.add_or_update_app(
            "Forza Motorsport",
            app_path,
            ApplicationType::DesktopApp,
            GraphicsPreference::PowerSaving,
        );
        assert_eq!(gfx.get_preference_for_app(app_path), GraphicsPreference::PowerSaving);

        // Removal
        assert!(gfx.remove_app(app_path));
        assert_eq!(gfx.get_preference_for_app(app_path), GraphicsPreference::LetWindowsDecide);
    }

    /// Test 6: Notification dismissal timeout and hearing notification flash options.
    #[test]
    fn test_notification_dismissal_timeout_and_flash() {
        assert_eq!(NotificationTimeout::Sec5.seconds(), 5);
        assert_eq!(NotificationTimeout::Sec7.seconds(), 7);
        assert_eq!(NotificationTimeout::Sec15.seconds(), 15);
        assert_eq!(NotificationTimeout::Sec30.seconds(), 30);
        assert_eq!(NotificationTimeout::Min1.seconds(), 60);
        assert_eq!(NotificationTimeout::Min5.seconds(), 300);

        assert_eq!(NotificationTimeout::from_seconds(60), Some(NotificationTimeout::Min1));
        assert_eq!(NotificationTimeout::from_seconds(99), None);

        let mut hearing = HearingSettings::default();
        assert_eq!(hearing.flash_screen, AudioNotificationFlash::Never);
        hearing.flash_screen = AudioNotificationFlash::FlashEntireScreen;
        assert_eq!(hearing.flash_screen as u32, 3);
    }

    /// Test 7: Keyboard layout switcher and Windows Registry export synchronization.
    #[test]
    fn test_keyboard_and_registry_export() {
        let mut settings = TimeGamingAccessSettings::new();

        // Keyboard layout switching
        assert_eq!(settings.language_and_region.active_keyboard_layout, KeyboardLayout::Abnt2);
        assert_eq!(settings.language_and_region.active_keyboard_layout.klid(), "00010416");

        // Switch to US International
        assert!(settings.language_and_region.switch_keyboard_layout(KeyboardLayout::UsInternational).is_ok());
        assert_eq!(settings.language_and_region.active_keyboard_layout, KeyboardLayout::UsInternational);
        assert_eq!(settings.language_and_region.active_keyboard_layout.klid(), "00020409");

        // Switch to uninstalled German layout fails
        assert!(settings.language_and_region.switch_keyboard_layout(KeyboardLayout::GermanQwertz).is_err());

        // Install German layout and switch
        settings.language_and_region.install_keyboard_layout(KeyboardLayout::GermanQwertz);
        assert!(settings.language_and_region.switch_keyboard_layout(KeyboardLayout::GermanQwertz).is_ok());

        // Test Windows Registry Export
        let reg_entries = settings.export_to_windows_registry();
        assert!(!reg_entries.is_empty());

        let tz_entry = reg_entries.iter().find(|(k, v, _)| k.contains("TimeZoneInformation") && v == "TimeZoneKeyName");
        assert!(tz_entry.is_some(), "Time zone registry entry must exist");

        let kbd_entry = reg_entries.iter().find(|(k, v, _)| k.contains("Keyboard Layout") && v == "1");
        assert_eq!(kbd_entry.unwrap().2, "00000407"); // German KLID
    }
}
