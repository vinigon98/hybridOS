//! HybridOS Crystal Glass Graphical Desktop Environment (Pure Rust #![no_std]).
//!
//! Master Compositor & Dispatcher for all 50 Tier-0 Native Applications across 7 Categories:
//! - Cat 1 (01..08): `apps_cat1_files` (Shell, Navigation, VFS & Disks)
//! - Cat 2 (09..18): `apps_cat2_admin` (Monitoring, Processes & Admin)
//! - Cat 3 (19..25): `apps_cat3_exec`  (Terminals, Execution & App Store)
//! - Cat 4 (26..33): `apps_cat4_net`   (Network, Internet & Security)
//! - Cat 5 (34..41): `apps_cat5_office`(Office, Text & Productivity)
//! - Cat 6 (42..47): `apps_cat6_media` (Multimedia, Paint, HDA Audio & Themes)
//! - Cat 7 (48..50): `apps_cat7_extra` (Classic Games, Accessibility & Recovery)
//!
//! Zero Mocks, Zero Decorative Buttons: every taskbar tab, tray icon, start menu item,
//! category pill, right-column shortcut, desktop icon, and window control has a real hitbox.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;
use crate::sync::SpinLock;
use crate::drivers::gop::{GOP_CONSOLE, FONT_8X16};
use crate::drivers::mouse::MOUSE_STATE;
use crate::drivers::{take_last_scancode, is_alt_pressed, is_ctrl_pressed, is_win_pressed};
use crate::drivers::gfx2d::{
    fill_rounded_rect, fill_rounded_rect_gradient, draw_rounded_rect_border,
    draw_soft_drop_shadow, draw_bmp,
    draw_crystal_glass_window_frame, draw_glass_control_capsule,
    draw_crystal_sphere_emblem, draw_vitrified_superbar,
    draw_asymmetric_start_menu,
    ICON_TERMINAL_BMP, ICON_FOLDER_BMP, ICON_CPU_BMP, ICON_INFO_BMP,
};
use crate::drivers::ux_design_system::{
    BG_WINDOW_ACTIVE, BG_SURFACE, TEXT_PRIMARY, TEXT_SECONDARY, TEXT_MUTED,
    ACCENT_PRIMARY, ACCENT_HOVER, ACCENT_PRESSED, BORDER_SUBTLE, BORDER_FOCUSED,
    ERROR_COLOR, SPACE_S, SPACE_M, BORDER_RADIUS_M, BORDER_RADIUS_L, BTN_HEIGHT, ButtonState,
    Tooltip, Modal, ModalResult, FocusManager, HotkeyRegistry, HotkeyAction,
    draw_focus_ring, HIGH_CONTRAST, set_high_contrast, is_high_contrast,
    theme_bg, theme_text_primary, theme_accent, theme_border,
    Animation, AnimType, SnapLayout, SnapZone, draw_keyboard_focus_ring,
    screen_reader_announce, HC_ACCENT, HC_BG,
};
use crate::drivers::{
    apps_cat1_files,
    apps_cat2_admin,
    apps_cat3_exec,
    apps_cat4_net,
    apps_cat5_office,
    apps_cat6_media,
    apps_cat7_extra,
};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum AppId {
    // Category 1: Shell, Navigation & Files (01..08)
    FileExplorer = 1,
    SuperbarHost = 2,
    GlobalSearch = 3,
    ArchiveManager = 4,
    DiskManager = 5,
    DiskCleanup = 6,
    IsoMounter = 7,
    RecycleBin = 8,

    // Category 2: Monitoring, Processes & Admin (09..18)
    TaskManager = 9,
    ResourceMonitor = 10,
    ControlPanel = 11,
    DeviceManager = 12,
    ServicesManager = 13,
    SystemInfo = 14,
    EventViewer = 15,
    RegistryEditor = 16,
    SystemConfig = 17,
    PowerOptions = 18,

    // Category 3: Terminals, Execution & Packaging (19..25)
    Terminal = 19,
    RunDialog = 20,
    AppStore = 21,
    SetupWizard = 22,
    DefaultPrograms = 23,
    DualAbiCenter = 24,
    EnvVariables = 25,

    // Category 4: Network, Internet & Security (26..33)
    NetworkCenter = 26,
    WebBrowser = 27,
    Firewall = 28,
    NetTools = 29,
    RemoteSerial = 30,
    SecurityCenter = 31,
    UserAccounts = 32,
    CryptoHash = 33,

    // Category 5: Office, Text & Productivity (34..41)
    Notepad = 34,
    WordPad = 35,
    Calculator = 36,
    StickyNotes = 37,
    CalendarClock = 38,
    CharacterMap = 39,
    ClipboardManager = 40,
    AddressBook = 41,

    // Category 6: Multimedia, Graphics, Audio & Themes (42..47)
    MediaViewer = 42,
    PaintStudio = 43,
    SnippingTool = 44,
    MediaPlayer = 45,
    VolumeMixer = 46,
    Personalization = 47,

    // Category 7: Classic Games, Accessibility & Recovery (48..50)
    GamesExplorer = 48,
    Accessibility = 49,
    HelpRecovery = 50,
}

impl AppId {
    #[inline]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    pub const fn category(self) -> u8 {
        match self.as_u8() {
            1..=8 => 1,
            9..=18 => 2,
            19..=25 => 3,
            26..=33 => 4,
            34..=41 => 5,
            42..=47 => 6,
            _ => 7,
        }
    }

    pub fn from_u8(n: u8) -> Option<Self> {
        ALL_APPS.iter().find(|m| m.id.as_u8() == n).map(|m| m.id)
    }

    pub fn from_cli_name(raw: &str) -> Option<Self> {
        let q = raw.trim().to_ascii_lowercase();
        if q.is_empty() {
            return None;
        }
        if let Ok(num) = q.parse::<u8>() {
            if let Some(id) = Self::from_u8(num) {
                return Some(id);
            }
        }
        for meta in ALL_APPS.iter() {
            if meta.cli_name == q {
                return Some(meta.id);
            }
            for alias in meta.aliases.iter() {
                if *alias == q {
                    return Some(meta.id);
                }
            }
        }
        None
    }
}

pub struct AppMeta {
    pub id: AppId,
    pub cli_name: &'static str,
    pub aliases: &'static [&'static str],
    pub title: &'static str,
    pub def_w: usize,
    pub def_h: usize,
}

pub const ALL_APPS: [AppMeta; 50] = [
    // Category 1 (01..08)
    AppMeta { id: AppId::FileExplorer,    cli_name: "explorer",    aliases: &["files", "arquivos"],         title: r#"Explorador de Arquivos - VFS & FAT32 (/ & C:\)"#, def_w: 660, def_h: 440 },
    AppMeta { id: AppId::SuperbarHost,    cli_name: "superbar",    aliases: &["taskbar", "barra"],          title: "Host da Superbarra & Menu Duplo",               def_w: 620, def_h: 420 },
    AppMeta { id: AppId::GlobalSearch,    cli_name: "search",      aliases: &["pesquisa", "findfile"],      title: "Pesquisa Global Instantanea no VFS",            def_w: 640, def_h: 430 },
    AppMeta { id: AppId::ArchiveManager,  cli_name: "archive",     aliases: &["zip", "tar", "7zip"],        title: "Gerenciador de Compactados (TAR/ZIP CRC32)",    def_w: 650, def_h: 430 },
    AppMeta { id: AppId::DiskManager,     cli_name: "diskmgmt",    aliases: &["disks", "discos"],           title: "Gerenciador de Discos & Particoes (MBR/GPT)",   def_w: 680, def_h: 450 },
    AppMeta { id: AppId::DiskCleanup,     cli_name: "cleanmgr",    aliases: &["cleanup", "limpeza"],        title: "Analisador e Limpeza de Disco",                 def_w: 600, def_h: 420 },
    AppMeta { id: AppId::IsoMounter,      cli_name: "isomount",    aliases: &["iso", "mountiso"],           title: "Montador de Imagens ISO / IMG",                 def_w: 600, def_h: 400 },
    AppMeta { id: AppId::RecycleBin,      cli_name: "recycle",     aliases: &["trash", "lixeira"],          title: "Lixeira do Sistema (/.trash)",                  def_w: 620, def_h: 410 },

    // Category 2 (09..18)
    AppMeta { id: AppId::TaskManager,     cli_name: "taskmgr",     aliases: &["tarefas", "tasks"],          title: "Gerenciador de Tarefas em Tempo Real",          def_w: 660, def_h: 440 },
    AppMeta { id: AppId::ResourceMonitor, cli_name: "resmon",      aliases: &["perfmon", "monitor"],        title: "Monitor de Recursos & Desempenho (4 Graficos)", def_w: 680, def_h: 460 },
    AppMeta { id: AppId::ControlPanel,    cli_name: "control",     aliases: &["painel", "settings"],        title: "Painel de Controle Central",                    def_w: 660, def_h: 440 },
    AppMeta { id: AppId::DeviceManager,   cli_name: "devmgmt",     aliases: &["devices", "pci"],            title: "Gerenciador de Dispositivos (PCI Hardware)",    def_w: 650, def_h: 430 },
    AppMeta { id: AppId::ServicesManager, cli_name: "services",    aliases: &["daemons", "servicos"],       title: "Gerenciador de Servicos & Daemons",             def_w: 650, def_h: 430 },
    AppMeta { id: AppId::SystemInfo,      cli_name: "msinfo32",    aliases: &["sysinfo", "sistema"],        title: "Informacoes do Sistema (CPUID / Memmap)",       def_w: 640, def_h: 430 },
    AppMeta { id: AppId::EventViewer,     cli_name: "eventvwr",    aliases: &["events", "logs"],            title: "Visualizador de Eventos & Logs do Kernel",      def_w: 670, def_h: 440 },
    AppMeta { id: AppId::RegistryEditor,  cli_name: "regedit",     aliases: &["registry", "registro"],      title: "Editor de Registro (HKLM / HKCU)",              def_w: 660, def_h: 440 },
    AppMeta { id: AppId::SystemConfig,    cli_name: "msconfig",    aliases: &["bootcfg", "config"],         title: "Configuracao do Sistema (msconfig / Limine)",   def_w: 630, def_h: 420 },
    AppMeta { id: AppId::PowerOptions,    cli_name: "powercfg",    aliases: &["power", "acpi", "energia"],  title: "Opcoes de Energia & ACPI (_S5_)",               def_w: 610, def_h: 410 },

    // Category 3 (19..25)
    AppMeta { id: AppId::Terminal,        cli_name: "terminal",    aliases: &["cmd", "bash", "powershell", "sh"], title: "Terminal Tri-Shell (Bash / CMD / PowerShell)", def_w: 720, def_h: 460 },
    AppMeta { id: AppId::RunDialog,       cli_name: "rundlg",      aliases: &["executar"],                  title: "Executar (Run Dialog)",                         def_w: 540, def_h: 340 },
    AppMeta { id: AppId::AppStore,        cli_name: "store",       aliases: &["appstore", "loja"],          title: "Central de Programas (App Store)",              def_w: 680, def_h: 460 },
    AppMeta { id: AppId::SetupWizard,     cli_name: "setup",       aliases: &["wizard", "installer"],       title: "Assistente de Instalacao (Setup Wizard)",       def_w: 620, def_h: 420 },
    AppMeta { id: AppId::DefaultPrograms, cli_name: "defaults",    aliases: &["assoc", "padrao"],           title: "Programas Padrao & Associacoes de Extensao",    def_w: 640, def_h: 420 },
    AppMeta { id: AppId::DualAbiCenter,   cli_name: "dualabi",     aliases: &["abi", "elf", "pe32"],        title: "Centro de Compatibilidade Dual-ABI (PE32+/ELF)",def_w: 680, def_h: 450 },
    AppMeta { id: AppId::EnvVariables,    cli_name: "envvars",     aliases: &["envgui", "variaveis"],       title: "Editor de Variaveis de Ambiente",               def_w: 640, def_h: 430 },

    // Category 4 (26..33)
    AppMeta { id: AppId::NetworkCenter,   cli_name: "netcenter",   aliases: &["network", "rede", "e1000"],  title: "Central de Rede e Compartilhamento (E1000)",    def_w: 680, def_h: 450 },
    AppMeta { id: AppId::WebBrowser,      cli_name: "browser",     aliases: &["web", "chrome", "http"],     title: "Crystal Web Browser (HTTP & VFS file://)",      def_w: 720, def_h: 480 },
    AppMeta { id: AppId::Firewall,        cli_name: "firewall",    aliases: &["wf", "filter"],              title: "Firewall do Sistema & Filtro de Pacotes",       def_w: 660, def_h: 440 },
    AppMeta { id: AppId::NetTools,        cli_name: "nettools",    aliases: &["netdiag", "pinggui"],        title: "Diagnostico de Rede (Ping / ARP / Netstat)",    def_w: 680, def_h: 450 },
    AppMeta { id: AppId::RemoteSerial,    cli_name: "serialterm",  aliases: &["com1", "telnet", "sshgui"],  title: "Cliente Remoto TCP & Console Serial COM1",      def_w: 660, def_h: 440 },
    AppMeta { id: AppId::SecurityCenter,  cli_name: "security",    aliases: &["seguranca", "fltmgr"],       title: "Central de Seguranca & Integridade (CR0.WP)",   def_w: 650, def_h: 430 },
    AppMeta { id: AppId::UserAccounts,    cli_name: "users",       aliases: &["accounts", "contas"],        title: "Contas de Usuario & Avatares",                  def_w: 640, def_h: 430 },
    AppMeta { id: AppId::CryptoHash,      cli_name: "hasher",      aliases: &["md5", "sha256", "crypto"],   title: "Verificador de Hashes (MD5 / SHA-256)",         def_w: 650, def_h: 430 },

    // Category 5 (34..41)
    AppMeta { id: AppId::Notepad,         cli_name: "notepad",     aliases: &["notas", "editor"],           title: "Bloco de Notas (Notepad VFS)",                  def_w: 640, def_h: 440 },
    AppMeta { id: AppId::WordPad,         cli_name: "wordpad",     aliases: &["write", "doc"],              title: "Editor de Documentos Formatados (WordPad A4)",  def_w: 680, def_h: 470 },
    AppMeta { id: AppId::Calculator,      cli_name: "calc",        aliases: &["calculadora"],               title: "Calculadora Multi-Modo (Padrao/Cient/Prog)",    def_w: 500, def_h: 460 },
    AppMeta { id: AppId::StickyNotes,     cli_name: "sticky",      aliases: &["postit", "lembretes"],       title: "Notas Autoadesivas (Sticky Notes)",             def_w: 620, def_h: 430 },
    AppMeta { id: AppId::CalendarClock,   cli_name: "clock",       aliases: &["calendar", "relogio"],       title: "Calendario, Relogio CMOS RTC & Cronometro",     def_w: 660, def_h: 450 },
    AppMeta { id: AppId::CharacterMap,    cli_name: "charmap",     aliases: &["fonts", "ascii"],            title: "Mapa de Caracteres ASCII & Fontes (Zoom 4x)",   def_w: 660, def_h: 450 },
    AppMeta { id: AppId::ClipboardManager,cli_name: "clipboard",   aliases: &["clip", "transferencia"],     title: "Gerenciador de Historico do Clipboard",         def_w: 600, def_h: 410 },
    AppMeta { id: AppId::AddressBook,     cli_name: "contacts",    aliases: &["agenda", "contatos"],        title: "Catalogo de Contatos (Exportacao CSV)",         def_w: 650, def_h: 440 },

    // Category 6 (42..47)
    AppMeta { id: AppId::MediaViewer,     cli_name: "photos",      aliases: &["viewer", "imagens", "bmp"],  title: "Visualizador de Fotos & Imagens BMP",           def_w: 660, def_h: 460 },
    AppMeta { id: AppId::PaintStudio,     cli_name: "paint",       aliases: &["mspaint", "pintura"],        title: "Estudio de Pintura (Paint Canvas -> .bmp)",     def_w: 680, def_h: 470 },
    AppMeta { id: AppId::SnippingTool,    cli_name: "snip",        aliases: &["screenshot", "captura"],     title: "Ferramenta de Captura de Tela (Snipping Tool)", def_w: 640, def_h: 440 },
    AppMeta { id: AppId::MediaPlayer,     cli_name: "mediaplayer", aliases: &["player", "music", "audio"],  title: "Crystal Media Player (Intel HDA & Espectro)",   def_w: 660, def_h: 450 },
    AppMeta { id: AppId::VolumeMixer,     cli_name: "mixer",       aliases: &["volume", "hda", "som"],      title: "Mixer de Volume Intel HDA (4 Canais)",          def_w: 600, def_h: 420 },
    AppMeta { id: AppId::Personalization, cli_name: "themes",      aliases: &["personalize", "tema"],       title: "Estudio de Personalizacao Crystal Glass",       def_w: 660, def_h: 450 },

    // Category 7 (48..50)
    AppMeta { id: AppId::GamesExplorer,   cli_name: "games",       aliases: &["minesweeper", "jogos", "puzzle"], title: "Central de Jogos (Campo Minado / 15 / 3D)", def_w: 680, def_h: 480 },
    AppMeta { id: AppId::Accessibility,   cli_name: "osk",         aliases: &["magnifier", "acessibilidade"],title: "Facilidade de Acesso (Teclado Virtual & Lupa)", def_w: 700, def_h: 460 },
    AppMeta { id: AppId::HelpRecovery,    cli_name: "helpgui",     aliases: &["recovery", "ajuda", "restore"],title: "Central de Ajuda Tri-Shell & Restauracao",     def_w: 680, def_h: 460 },
];

pub struct Window {
    pub id: AppId,
    pub title: &'static str,
    pub x: usize,
    pub y: usize,
    pub w: usize,
    pub h: usize,
    pub open: bool,
    pub minimized: bool,
    pub maximized: bool,
    pub saved_x: usize,
    pub saved_y: usize,
    pub saved_w: usize,
    pub saved_h: usize,
}

pub struct DesktopState {
    pub active: bool,
    pub needs_full_redraw: bool,
    pub start_menu_open: bool,
    pub start_menu_cat: usize, // 0 = Destaques, 1..=7 = Categorias 1..7
    pub start_menu_scroll: usize,
    pub focused_app: AppId,
    pub prev_mouse_left: bool,
    pub dragging_window: Option<usize>,
    pub drag_offset_x: usize,
    pub drag_offset_y: usize,
    pub current_vfs_path: String,
    pub saved_cursor_x: i32,
    pub saved_cursor_y: i32,
    pub cursor_saved_valid: bool,
    pub cursor_bg: [u32; 12 * 16],
    pub windows: Vec<Window>,
    pub term_lines: Vec<String>,
    pub vfs_entries: Vec<String>,
    pub search_query: String,
    pub notepad_text: String,
    pub calc_display: String,
    pub calc_acc: f64,
    pub calc_op: Option<char>,
    pub active_tab: usize,
    /// Sistema de foco por teclado (navegação Tab)
    pub focus_mgr: FocusManager,
    /// Registry de atalhos de teclado
    pub hotkey_registry: HotkeyRegistry,
    /// Tooltip flutuante para botões ícones
    pub tooltip: Tooltip,
    /// Modal de confirmação (ex: desligar)
    pub confirm_modal: Modal,
    /// Índice da janela focada para Alt+Tab
    pub alt_tab_idx: usize,
    /// Alt+Tab overlay visível
    pub alt_tab_visible: bool,
    /// Tema de alto contraste ativo
    pub high_contrast: bool,
    /// Animação atual (fade/slide)
    pub animation: Animation,
    /// Layout de snap para arrastar janelas
    pub snap_layout: SnapLayout,
    /// Janela sendo arrastada para snap
    pub snap_dragging: Option<usize>,
    /// Leitor de tela habilitado
    pub screen_reader: bool,
}

impl DesktopState {
    pub fn calc_input(&mut self, ch: char) {
        match ch {
            '0'..='9' => {
                if self.calc_display == "0" {
                    self.calc_display.clear();
                }
                self.calc_display.push(ch);
            }
            '+' | '-' | '*' | '/' => {
                if let Ok(v) = self.calc_display.parse::<f64>() {
                    self.calc_acc = v;
                    self.calc_op = Some(ch);
                    self.calc_display = String::from("0");
                }
            }
            '=' => {
                if let (Some(op), Ok(v)) = (self.calc_op, self.calc_display.parse::<f64>()) {
                    let res = match op {
                        '+' => self.calc_acc + v,
                        '-' => self.calc_acc - v,
                        '*' => self.calc_acc * v,
                        '/' => if v != 0.0 { self.calc_acc / v } else { 0.0 },
                        _ => v,
                    };
                    self.calc_display = format!("{}", res as i64);
                    self.calc_op = None;
                }
            }
            'C' | 'c' => {
                self.calc_display = String::from("0");
                self.calc_acc = 0.0;
                self.calc_op = None;
            }
            _ => {}
        }
    }

    pub fn open_window(&mut self, id: AppId) -> &'static str {
        for w in self.windows.iter_mut() {
            if w.id == id {
                w.open = true;
                w.minimized = false;
                self.focused_app = id;
                self.start_menu_open = false;
                self.needs_full_redraw = true;
                return w.title;
            }
        }
        "HybridOS"
    }
}

pub static DESKTOP: SpinLock<Option<DesktopState>> = SpinLock::new(None);

const CURSOR_BITMAP: [u16; 16] = [
    0b1000_0000_0000_0000,
    0b1100_0000_0000_0000,
    0b1110_0000_0000_0000,
    0b1111_0000_0000_0000,
    0b1111_1000_0000_0000,
    0b1111_1100_0000_0000,
    0b1111_1110_0000_0000,
    0b1111_1111_0000_0000,
    0b1111_1111_1000_0000,
    0b1111_1111_1100_0000,
    0b1111_1100_0000_0000,
    0b1101_1100_0000_0000,
    0b1000_1110_0000_0000,
    0b0000_1110_0000_0000,
    0b0000_0111_0000_0000,
    0b0000_0110_0000_0000,
];

/// Read real hardware CMOS RTC clock (hours, minutes, seconds) via I/O ports 0x70/0x71.
fn read_cmos_rtc_hm() -> (u8, u8) {
    unsafe {
        let bcd_to_bin = |v: u8| -> u8 { (v & 0x0F) + ((v >> 4) * 10) };
        crate::arch::msr::outb(0x70, 0x0B);
        let reg_b = crate::arch::msr::inb(0x71);
        let is_binary = (reg_b & 0x04) != 0;

        crate::arch::msr::outb(0x70, 0x04);
        let raw_h = crate::arch::msr::inb(0x71);
        crate::arch::msr::outb(0x70, 0x02);
        let raw_m = crate::arch::msr::inb(0x71);

        let h = if is_binary { raw_h } else { bcd_to_bin(raw_h & 0x7F) } % 24;
        let m = if is_binary { raw_m } else { bcd_to_bin(raw_m) } % 60;
        (h, m)
    }
}

pub fn init_desktop() {
    let mut term_lines = Vec::new();
    term_lines.push(String::from("HybridOS Crystal Glass Shell [Bash | CMD | PowerShell]"));
    term_lines.push(String::from("50 Tier-0 Native Applications Active | Digite 'apps' ou 'start <nome>'"));
    term_lines.push(String::from(""));

    let mut windows = Vec::with_capacity(50);
    for (idx, meta) in ALL_APPS.iter().enumerate() {
        let cascade = (idx % 8) * 26;
        let x = 110 + cascade;
        let y = 44 + (idx % 6) * 24;
        let open = meta.id == AppId::Terminal;
        windows.push(Window {
            id: meta.id,
            title: meta.title,
            x,
            y,
            w: meta.def_w,
            h: meta.def_h,
            open,
            minimized: false,
            maximized: false,
            saved_x: x,
            saved_y: y,
            saved_w: meta.def_w,
            saved_h: meta.def_h,
        });
    }

    let state = DesktopState {
        active: true,
        needs_full_redraw: true,
        start_menu_open: false,
        start_menu_cat: 0,
        start_menu_scroll: 0,
        focused_app: AppId::Terminal,
        prev_mouse_left: false,
        dragging_window: None,
        drag_offset_x: 0,
        drag_offset_y: 0,
        current_vfs_path: String::from("/"),
        saved_cursor_x: 640,
        saved_cursor_y: 400,
        cursor_saved_valid: false,
        cursor_bg: [0; 12 * 16],
        windows,
        term_lines,
        vfs_entries: Vec::new(),
        search_query: String::new(),
        notepad_text: String::from("Bem-vindo ao Bloco de Notas do HybridOS!\nEdite seus arquivos de configuracao ou crie novos documentos diretamente no VFS."),
        calc_display: String::from("0"),
        calc_acc: 0.0,
        calc_op: None,
        active_tab: 0,
        focus_mgr: FocusManager::new(0),
        hotkey_registry: HotkeyRegistry::new(),
        tooltip: Tooltip::new("", 0, 0),
        confirm_modal: Modal::new("", ""),
        alt_tab_idx: 0,
        alt_tab_visible: false,
        high_contrast: false,
        animation: Animation::new(),
        snap_layout: SnapLayout::new(),
        snap_dragging: None,
        screen_reader: false,
    };

    // Inicializa o focus manager com a quantidade de janelas abertas
    let mut desk_lock = DESKTOP.lock();
    if let Some(ref mut desk) = *desk_lock {
        desk.focus_mgr = FocusManager::new(desk.windows.len());
    }
    drop(desk_lock);

    *DESKTOP.lock() = Some(state);
    refresh_vfs_cache();
    redraw_full_desktop("", "root@hybridos:/# ");
}

/// Opens any of the 50 native apps by AppId and focuses it.
pub fn open_app(id: AppId) -> &'static str {
    let title = {
        let mut desk_lock = DESKTOP.lock();
        if let Some(ref mut desk) = *desk_lock {
            desk.open_window(id)
        } else {
            return "Desktop nao inicializado";
        }
    };
    if id == AppId::FileExplorer {
        refresh_vfs_cache();
    }
    title
}

/// Launches any of the 50 native apps by CLI name, alias, or numeric ID (1..=50).
pub fn launch_app_by_name(query: &str) -> Option<&'static str> {
    let id = AppId::from_cli_name(query)?;
    Some(open_app(id))
}

/// Returns a formatted catalog of all 50 native apps for Terminal `apps` command.
pub fn list_all_apps_text() -> String {
    let mut out = String::from("=== HybridOS 50 Tier-0 Native Applications (use: start <cli>) ===\n");
    for meta in ALL_APPS.iter() {
        out.push_str(&format!("[{:02}] {:<12} - {}\n", meta.id.as_u8(), meta.cli_name, meta.title));
    }
    out
}

/// Forwards a keyboard character to Start Menu search or focused Cat5 interactive apps.
/// Returns true if the event was consumed by the GUI (so the shell loop shouldn't echo it to Terminal).
pub fn dispatch_focused_app_key(ch: char) -> bool {
    let mut desk_lock = DESKTOP.lock();
    let desk = match *desk_lock {
        Some(ref mut d) => d,
        None => return false,
    };

    // 1. If Start Menu is open, route typing directly to Start Menu live search box
    if desk.start_menu_open {
        match ch {
            '\x08' => {
                desk.search_query.pop();
                desk.start_menu_scroll = 0;
                desk.needs_full_redraw = true;
                return true;
            }
            '\r' | '\n' => {
                let matches = collect_start_menu_apps(desk);
                if let Some(&first_id) = matches.first() {
                    desk.open_window(first_id);
                    desk.search_query.clear();
                    return true;
                }
                return true;
            }
            c if !c.is_control() => {
                if desk.search_query.len() < 28 {
                    desk.search_query.push(c);
                    desk.start_menu_scroll = 0;
                    desk.needs_full_redraw = true;
                }
                return true;
            }
            _ => return false,
        }
    }

    // 2. If a Category 5 Office app is focused and open, route directly to `handle_cat5_key`
    let focused = desk.focused_app;
    if focused != AppId::Terminal {
        let is_open = desk.windows.iter().any(|w| w.id == focused && w.open && !w.minimized);
        if is_open && (34..=41).contains(&focused.as_u8()) {
            return apps_cat5_office::handle_cat5_key(focused.as_u8(), ch, desk);
        }
    }

    false
}

/// Deterministic verification helper called during kernel self-tests:
/// Renders and dispatches an interactive test click to every app in Category `cat` (1..=7)
/// directly onto the real GOP framebuffer without mocks or panics.
pub fn verify_category_apps(cat: u8) -> usize {
    let range = match cat {
        1 => 1..=8u8,
        2 => 9..=18u8,
        3 => 19..=25u8,
        4 => 26..=33u8,
        5 => 34..=41u8,
        6 => 42..=47u8,
        7 => 48..=50u8,
        _ => return 0,
    };

    let mut con_lock = GOP_CONSOLE.lock();
    let mut desk_lock = DESKTOP.lock();
    let (con, desk) = match (&mut *con_lock, &mut *desk_lock) {
        (Some(ref mut c), Some(ref mut d)) => (c, d),
        _ => return 0,
    };

    if con.fb_ptr.is_null() {
        return 0;
    }

    let fb = con.fb_ptr;
    let pitch = con.pitch;
    let scr_w = con.width;
    let scr_h = con.height;

    let mut verified = 0usize;
    for id_num in range {
        if let Some(app_id) = AppId::from_u8(id_num) {
            let win_idx = desk.windows.iter().position(|w| w.id == app_id);
            let prev_state = win_idx.map(|i| (desk.windows[i].open, desk.windows[i].minimized));
            if let Some(i) = win_idx {
                desk.windows[i].open = true;
                desk.windows[i].minimized = false;
            }
            let (cx, cy, cw, ch) = if let Some(i) = win_idx {
                let w = &desk.windows[i];
                (w.x + 4, w.y + 32, w.w.saturating_sub(8), w.h.saturating_sub(36))
            } else {
                (120, 80, 620, 400)
            };

            unsafe {
                dispatch_app_render(app_id, fb, pitch, scr_w, scr_h, cx, cy, cw, ch, desk, "", "root@hybridos:/# ");
            }
            // Send safe interactive click in toolbar / top tab region (cx + 20, cy + 16)
            let _ = dispatch_app_click(app_id, cx + 20, cy + 16, desk);

            if let (Some(i), Some((was_open, was_min))) = (win_idx, prev_state) {
                desk.windows[i].open = was_open;
                desk.windows[i].minimized = was_min;
            }
            verified += 1;
        }
    }

    verified
}

pub fn refresh_vfs_cache() {
    let mut list = Vec::new();
    let cur_path = {
        let desk_lock = DESKTOP.lock();
        if let Some(ref desk) = *desk_lock {
            desk.current_vfs_path.clone()
        } else {
            String::from("/")
        }
    };

    if cur_path != "/" && cur_path != r#"C:\"# && cur_path != "C:" {
        list.push(String::from("[DIR]  .."));
    }

    if let Ok(entries) = crate::vfs::vfs_readdir(&cur_path) {
        for e in entries {
            let prefix = if e.ino_type.is_dir() { "[DIR]  " } else { "[FILE] " };
            let mut line = String::from(prefix);
            line.push_str(&e.name);
            if e.ino_type.is_file() && e.size > 0 {
                line.push_str(&format!(" ({} B)", e.size));
            }
            if !list.iter().any(|x| x.contains(&e.name)) && list.len() < 32 {
                list.push(line);
            }
        }
    }

    if let Some(ref mut desk) = *DESKTOP.lock() {
        desk.vfs_entries = list;
    }
}

pub fn append_terminal_output(prompt_and_cmd: &str, output: &str) {
    let mut should_refresh_files = false;
    if let Some(ref mut desk) = *DESKTOP.lock() {
        if !prompt_and_cmd.is_empty() {
            desk.term_lines.push(String::from(prompt_and_cmd));
        }
        for line in output.lines() {
            desk.term_lines.push(String::from(line));
        }
        while desk.term_lines.len() > 100 {
            desk.term_lines.remove(0);
        }
        desk.needs_full_redraw = true;
        should_refresh_files = true;
    }
    if should_refresh_files {
        refresh_vfs_cache();
    }
}

pub fn clear_terminal_lines() {
    if let Some(ref mut desk) = *DESKTOP.lock() {
        desk.term_lines.clear();
        desk.needs_full_redraw = true;
    }
}

#[inline]
fn map_char_to_ascii(ch: char) -> char {
    match ch {
        'á' | 'à' | 'ã' | 'â' | 'ä' => 'a',
        'Á' | 'À' | 'Ã' | 'Â' | 'Ä' => 'A',
        'é' | 'è' | 'ê' | 'ë' => 'e',
        'É' | 'È' | 'Ê' | 'Ë' => 'E',
        'í' | 'ì' | 'î' | 'ï' => 'i',
        'Í' | 'Ì' | 'Î' | 'Ï' => 'I',
        'ó' | 'ò' | 'õ' | 'ô' | 'ö' => 'o',
        'Ó' | 'Ò' | 'Õ' | 'Ô' | 'Ö' => 'O',
        'ú' | 'ù' | 'û' | 'ü' => 'u',
        'Ú' | 'Ù' | 'Û' | 'Ü' => 'U',
        'ç' => 'c',
        'Ç' => 'C',
        'ñ' => 'n',
        'Ñ' => 'N',
        c if (c as u32) >= 32 && (c as u32) <= 126 => c,
        _ => '?',
    }
}

pub unsafe fn draw_text_px(fb: *mut u8, pitch: usize, scr_w: usize, scr_h: usize, x: usize, y: usize, text: &str, fg: u32, bg: Option<u32>) {
    draw_text_clipped(fb, pitch, scr_w, scr_h, x, y, scr_w, text, fg, bg);
}

pub unsafe fn draw_text_clipped(
    fb: *mut u8,
    pitch: usize,
    scr_w: usize,
    scr_h: usize,
    x: usize,
    y: usize,
    max_x: usize,
    text: &str,
    fg: u32,
    bg: Option<u32>,
) {
    let limit_x = max_x.min(scr_w);
    if y + 16 > scr_h {
        return;
    }
    for (i, raw_ch) in text.chars().enumerate() {
        let cx = x + i * 8;
        if cx + 8 > limit_x {
            break;
        }
        let ch = map_char_to_ascii(raw_ch);
        let ascii = ch as usize;
        let font_idx = (ascii.saturating_sub(32)) * 16;
        for row in 0..16 {
            let bits = if font_idx + row < FONT_8X16.len() {
                FONT_8X16[font_idx + row]
            } else {
                0
            };
            let row_ptr = fb.add((y + row) * pitch) as *mut u32;
            for col in 0..8 {
                if (bits & (0x80 >> col)) != 0 {
                    *row_ptr.add(cx + col) = fg;
                } else if let Some(bg_col) = bg {
                    *row_ptr.add(cx + col) = bg_col;
                }
            }
        }
    }
}

unsafe fn restore_cursor_bg(fb: *mut u8, pitch: usize, scr_w: usize, scr_h: usize, desk: &mut DesktopState) {
    if !desk.cursor_saved_valid {
        return;
    }
    let sx = desk.saved_cursor_x.max(0) as usize;
    let sy = desk.saved_cursor_y.max(0) as usize;
    for row in 0..16 {
        for col in 0..12 {
            let px = sx + col;
            let py = sy + row;
            if px < scr_w && py < scr_h {
                let ptr = fb.add(py * pitch) as *mut u32;
                *ptr.add(px) = desk.cursor_bg[row * 12 + col];
            }
        }
    }
    desk.cursor_saved_valid = false;
}

unsafe fn save_and_draw_cursor(fb: *mut u8, pitch: usize, scr_w: usize, scr_h: usize, desk: &mut DesktopState, mx: i32, my: i32) {
    let sx = mx.max(0) as usize;
    let sy = my.max(0) as usize;
    for row in 0..16 {
        let mask = CURSOR_BITMAP[row];
        for col in 0..12 {
            let px = sx + col;
            let py = sy + row;
            if px < scr_w && py < scr_h {
                let ptr = fb.add(py * pitch) as *mut u32;
                desk.cursor_bg[row * 12 + col] = *ptr.add(px);
                let bit = (mask & (0x8000 >> col)) != 0;
                if bit {
                    let is_edge = col == 0 || row == 0 || (mask & (0x8000 >> (col + 1))) == 0;
                    *ptr.add(px) = if is_edge { 0x000F172A } else { 0x00FFFFFF };
                }
            }
        }
    }
    desk.saved_cursor_x = mx;
    desk.saved_cursor_y = my;
    desk.cursor_saved_valid = true;
}

/// Returns the list of AppIds visible in the Start Menu left column given active search or category tab.
fn collect_start_menu_apps(desk: &DesktopState) -> Vec<AppId> {
    let q = desk.search_query.trim().to_ascii_lowercase();
    if !q.is_empty() {
        return ALL_APPS
            .iter()
            .filter(|m| {
                m.cli_name.contains(q.as_str())
                    || m.title.to_ascii_lowercase().contains(q.as_str())
                    || m.aliases.iter().any(|a| a.contains(q.as_str()))
            })
            .map(|m| m.id)
            .collect();
    }

    match desk.start_menu_cat {
        0 => vec![
            AppId::Terminal,
            AppId::FileExplorer,
            AppId::WebBrowser,
            AppId::TaskManager,
            AppId::ControlPanel,
            AppId::AppStore,
            AppId::PaintStudio,
            AppId::GamesExplorer,
            AppId::Personalization,
            AppId::Calculator,
            AppId::Notepad,
            AppId::MediaPlayer,
        ],
        cat @ 1..=7 => ALL_APPS
            .iter()
            .filter(|m| m.id.category() == cat as u8)
            .map(|m| m.id)
            .collect(),
        _ => Vec::new(),
    }
}

/// Interactive desktop handler polled during OS execution.
pub fn update_desktop_interactive(current_input: &str, prompt: &str) {
    let (mx, my, left_btn) = {
        let m = MOUSE_STATE.lock();
        (m.x, m.y, m.left_button)
    };

    let mut trigger_full = false;
    let mut clicked_pos: Option<(usize, usize)> = None;

    {
        let mut desk_lock = DESKTOP.lock();
        if let Some(ref mut desk) = *desk_lock {
            if !desk.active {
                return;
            }

            // ---------------- HOTKEYS DE TECLADO ----------------
            if let Some(sc) = take_last_scancode() {
                let action = desk.hotkey_registry.check_hotkey(sc);
                match action {
                    HotkeyAction::AltTab => {
                        // Alternar entre janelas abertas (não minimizadas)
                        let open_indices: Vec<usize> = desk.windows.iter()
                            .enumerate()
                            .filter(|(_, w)| w.open && !w.minimized)
                            .map(|(i, _)| i)
                            .collect();
                        if !open_indices.is_empty() {
                            desk.alt_tab_idx = (desk.alt_tab_idx + 1) % open_indices.len();
                            let target_idx = open_indices[desk.alt_tab_idx];
                            let target_id = desk.windows[target_idx].id;
                            desk.focused_app = target_id;
                            for w in desk.windows.iter_mut() {
                                w.minimized = false;
                                w.open = true;
                            }
                            desk.windows[target_idx].minimized = false;
                            desk.alt_tab_visible = true;
                        }
                        trigger_full = true;
                    }
                    HotkeyAction::WinD => {
                        // Mostrar/ocultar área de trabalho (minimizar todas)
                        let all_minimized = desk.windows.iter().all(|w| !w.open || w.minimized);
                        for w in desk.windows.iter_mut() {
                            if w.open {
                                w.minimized = all_minimized;
                            }
                        }
                        desk.start_menu_open = false;
                        trigger_full = true;
                    }
                    HotkeyAction::WinS => {
                        // Abrir busca global (Start Menu com foco na busca)
                        desk.start_menu_open = true;
                        desk.start_menu_cat = 0;
                        desk.search_query.clear();
                        trigger_full = true;
                    }
                    HotkeyAction::Escape => {
                        if desk.start_menu_open {
                            desk.start_menu_open = false;
                            desk.search_query.clear();
                        } else if desk.confirm_modal.visible {
                            desk.confirm_modal.hide();
                        } else if desk.alt_tab_visible {
                            desk.alt_tab_visible = false;
                        }
                        trigger_full = true;
                    }
                    HotkeyAction::AltF4 => {
                        // Fechar janela focada
                        if let Some(fi) = desk.windows.iter().position(|w| w.id == desk.focused_app && w.open && !w.minimized) {
                            desk.windows[fi].open = false;
                        }
                        trigger_full = true;
                    }
                    _ => {}
                }

                // Navegação por Tab (sem Alt) — move foco entre janelas
                if let Some(sc) = take_last_scancode() {
                    if sc == 0x0F && !desk.hotkey_registry.alt_pressed {
                        // Tab pressionado sozinho
                        let focused_windows: Vec<usize> = desk.windows.iter()
                            .enumerate()
                            .filter(|(_, w)| w.open && !w.minimized)
                            .map(|(i, _)| i)
                            .collect();
                        if !focused_windows.is_empty() {
                            desk.focus_mgr.next();
                            let idx = desk.focus_mgr.focused_idx % focused_windows.len();
                            if let Some(&win_idx) = focused_windows.get(idx) {
                                let win_id = desk.windows[win_idx].id;
                                desk.focused_app = win_id;
                                if desk.screen_reader {
                                    if let Some(meta) = ALL_APPS.iter().find(|m| m.id == win_id) {
                                        screen_reader_announce(&format!("Foco em {}", meta.title));
                                    }
                                }
                            }
                        }
                        trigger_full = true;
                    }
                    // Shift+Tab — foco anterior
                    if sc == 0x0F && desk.hotkey_registry.shift_pressed {
                        let focused_windows: Vec<usize> = desk.windows.iter()
                            .enumerate()
                            .filter(|(_, w)| w.open && !w.minimized)
                            .map(|(i, _)| i)
                            .collect();
                        if !focused_windows.is_empty() {
                            desk.focus_mgr.prev();
                            let idx = desk.focus_mgr.focused_idx % focused_windows.len();
                            if let Some(&win_idx) = focused_windows.get(idx) {
                                let win_id = desk.windows[win_idx].id;
                                desk.focused_app = win_id;
                                if desk.screen_reader {
                                    if let Some(meta) = ALL_APPS.iter().find(|m| m.id == win_id) {
                                        screen_reader_announce(&format!("Foco anterior em {}", meta.title));
                                    }
                                }
                            }
                        }
                        trigger_full = true;
                    }
                    // Alto Contraste (Left Alt + Left Shift + Print Screen)
                    if desk.hotkey_registry.alt_pressed && desk.hotkey_registry.shift_pressed && sc == 0x37 {
                        desk.high_contrast = !desk.high_contrast;
                        set_high_contrast(desk.high_contrast);
                        if desk.screen_reader {
                            if desk.high_contrast {
                                screen_reader_announce("Alto contraste ativado");
                            } else {
                                screen_reader_announce("Alto contraste desativado");
                            }
                        }
                        trigger_full = true;
                    }
                }
            }

            if left_btn {
                if let Some(w_idx) = desk.dragging_window {
                    if w_idx < desk.windows.len() && desk.windows[w_idx].open && !desk.windows[w_idx].minimized {
                        let cur_x = mx.max(0) as usize;
                        let cur_y = my.max(0) as usize;
                        let new_x = cur_x.saturating_sub(desk.drag_offset_x);
                        let new_y = cur_y.saturating_sub(desk.drag_offset_y);
                        if desk.windows[w_idx].x != new_x || desk.windows[w_idx].y != new_y {
                            desk.windows[w_idx].x = new_x.min(1200);
                            desk.windows[w_idx].y = new_y.clamp(24, 700);
                            trigger_full = true;
                        }
                    } else {
                        desk.dragging_window = None;
                    }
                } else if !desk.prev_mouse_left {
                    clicked_pos = Some((mx.max(0) as usize, my.max(0) as usize));
                } else if desk.focused_app == AppId::PaintStudio
                    || desk.focused_app == AppId::VolumeMixer
                    || desk.focused_app == AppId::Personalization
                {
                    // Continuous drag painting / slider updates while left button held
                    trigger_full = true;
                }
            } else {
                if desk.dragging_window.is_some() {
                    // Snap Layout: detectar se a janela está na borda da tela ao soltar
                    let (scr_w, scr_h) = {
                        let con_lock = GOP_CONSOLE.lock();
                        if let Some(ref c) = *con_lock {
                            (c.width.max(640), c.height.max(480))
                        } else {
                            (1280usize, 800usize)
                        }
                    };
                    if let Some(w_idx) = desk.dragging_window {
                        if w_idx < desk.windows.len() {
                            let zone = desk.snap_layout.detect_zone(mx, my, scr_w, scr_h);
                            if zone != SnapZone::None {
                                let (sx, sy, sw, sh) = desk.snap_layout.snap_rect(zone, scr_w, scr_h);
                                // Ajustar para não cobrir a superbarra (40px na base)
                                let sh_adj = if zone == SnapZone::FullScreen || zone == SnapZone::RightHalf || zone == SnapZone::LeftHalf {
                                    sh.saturating_sub(64)
                                } else {
                                    sh
                                };
                                let sy_adj = if zone == SnapZone::FullScreen || zone == SnapZone::RightHalf || zone == SnapZone::LeftHalf {
                                    0
                                } else {
                                    sy
                                };
                                let mut win = &mut desk.windows[w_idx];
                                win.x = sx;
                                win.y = sy_adj;
                                win.w = sw;
                                win.h = sh_adj;
                                win.saved_x = sx;
                                win.saved_y = sy_adj;
                                win.saved_w = sw;
                                win.saved_h = sh_adj;
                                if desk.screen_reader {
                                    screen_reader_announce("Janela ajustada com snap layout");
                                }
                            }
                        }
                    }
                    desk.dragging_window = None;
                    trigger_full = true;
                }
                if desk.prev_mouse_left {
                    // Release edge: finalize Paint Line/Rectangle strokes and button up-states
                    trigger_full = true;
                }
            }

            desk.prev_mouse_left = left_btn;
            if desk.needs_full_redraw {
                trigger_full = true;
            }

            // Live mouse-tracking redraw for Accessibility Magnifier (#49)
            let mouse_moved = desk.saved_cursor_x != mx || desk.saved_cursor_y != my;
            if mouse_moved
                && desk.windows.iter().any(|w| w.id == AppId::Accessibility && w.open && !w.minimized)
                && apps_cat7_extra::is_magnifier_live()
            {
                trigger_full = true;
            }

            // Throttled continuous animation redraw for MediaPlayer (#45) and 3D Benchmark (#48)
            let media_live = desk.windows.iter().any(|w| w.id == AppId::MediaPlayer && w.open && !w.minimized)
                && apps_cat6_media::is_media_playing();
            let bench_live = desk.windows.iter().any(|w| w.id == AppId::GamesExplorer && w.open && !w.minimized)
                && apps_cat7_extra::is_benchmark_running();
            if media_live || bench_live {
                use core::sync::atomic::{AtomicU64, Ordering};
                static LAST_ANIM_TICK: AtomicU64 = AtomicU64::new(0);
                let now = crate::arch::idt::SYSTEM_TICKS.load(Ordering::Relaxed);
                let prev = LAST_ANIM_TICK.load(Ordering::Relaxed);
                if now.wrapping_sub(prev) >= 5 {
                    LAST_ANIM_TICK.store(now, Ordering::Relaxed);
                    trigger_full = true;
                }
            }
        } else {
            return;
        }
    }

    if let Some((cx, cy)) = clicked_pos {
        handle_desktop_click(cx, cy);
        trigger_full = true;
    }

    if trigger_full {
        redraw_full_desktop(current_input, prompt);
    } else {
        let mut con_lock = GOP_CONSOLE.lock();
        let mut desk_lock = DESKTOP.lock();
        if let (Some(ref mut con), Some(ref mut desk)) = (&mut *con_lock, &mut *desk_lock) {
            // Throttled cursor update: only redraw cursor when mouse moves significantly
            // to avoid flicker from continuous restore/draw cycles
            let moved_enough = (desk.saved_cursor_x as i32 - mx).abs() >= 2
                || (desk.saved_cursor_y as i32 - my).abs() >= 2;
            if moved_enough && !con.fb_ptr.is_null() {
                unsafe {
                    restore_cursor_bg(con.fb_ptr, con.pitch, con.width, con.height, desk);
                    save_and_draw_cursor(con.fb_ptr, con.pitch, con.width, con.height, desk, mx, my);
                }
            }
        }
    }
}

fn handle_desktop_click(cx: usize, cy: usize) {
    // Acquire screen dimensions from GOP_CONSOLE before locking DESKTOP (strict lock ordering)
    let (scr_w, scr_h) = {
        let con_lock = GOP_CONSOLE.lock();
        if let Some(ref c) = *con_lock {
            (c.width.max(640), c.height.max(480))
        } else {
            (1280usize, 800usize)
        }
    };

    let mut do_refresh_vfs = false;
    let mut desk_lock = DESKTOP.lock();
    let desk = match *desk_lock {
        Some(ref mut d) => d,
        None => return,
    };

    // 0. Modal de confirmação (ex: desligar) — intercepta cliques antes de tudo
    if desk.confirm_modal.visible {
        let mw = 320;
        let mh = 140;
        let mx_modal = (scr_w / 2).saturating_sub(mw / 2);
        let my_modal = (scr_h / 2).saturating_sub(mh / 2);
        let btn_y = my_modal + mh - SPACE_M - BTN_HEIGHT;
        let sim_x = mx_modal + SPACE_M;
        let nao_x = mx_modal + mw - SPACE_M - BTN_HEIGHT;
        let btn_w2 = BTN_HEIGHT + 16;
        if cy >= btn_y && cy <= btn_y + BTN_HEIGHT {
            if cx >= sim_x && cx <= sim_x + btn_w2 {
                // Sim → executar desligamento
                unsafe {
                    crate::arch::msr::outb(0xf4, 0x00);
                    crate::arch::acpi::acpi_shutdown();
                }
            }
            if cx >= nao_x && cx <= nao_x + btn_w2 {
                // Não → fechar modal
                desk.confirm_modal.hide();
                return;
            }
        }
        // Clique fora do modal fecha o modal
        desk.confirm_modal.hide();
        return;
    }

    // 1. Windows 11 Start Button click (Start Menu Toggle)
    let start_btn_x = 12;
    let start_btn_y = scr_h - 34;
    let start_btn_w = 56;
    let start_btn_h = 26;
    if cx >= start_btn_x && cx <= start_btn_x + start_btn_w && cy >= start_btn_y && cy <= start_btn_y + start_btn_h {
        desk.start_menu_open = !desk.start_menu_open;
        desk.start_menu_cat = 0;
        desk.search_query.clear();
        return;
    }

    // 1b. Crystal Sphere Emblem click (Start Menu Toggle - alternative)
    let emblem_dx = (cx as isize) - 82;
    let emblem_dy = (cy as isize) - ((scr_h - 22) as isize);
    if emblem_dx * emblem_dx + emblem_dy * emblem_dy <= 24 * 24 {
        desk.start_menu_open = !desk.start_menu_open;
        return;
    }

    // 2. Rightmost "Show Desktop" Peek Strip
    if cx >= scr_w.saturating_sub(15) && cy >= scr_h.saturating_sub(40) {
        for w in desk.windows.iter_mut() {
            w.minimized = true;
        }
        desk.start_menu_open = false;
        return;
    }

    // 3. Start Menu interaction (if open) — Windows 11 centered layout
    let sm_w = 640usize;
    let sm_h = 720usize;
    let sm_x = (scr_w / 2).saturating_sub(sm_w / 2);
    let sm_y = (scr_h / 2).saturating_sub(sm_h / 2);
    if desk.start_menu_open && cx >= sm_x && cx <= sm_x + sm_w && cy >= sm_y && cy <= sm_y + sm_h {
        // 3a. Power button [ Desligar | ▸ ] in right column — abre modal de confirmação
        let btn_x = 8 + 250 + 12;
        let btn_y = sm_y + 500 - 42;
        if cx >= btn_x && cx <= btn_x + 146 && cy >= btn_y && cy <= btn_y + 30 {
            desk.confirm_modal.title = String::from("Desligar o HybridOS?");
            desk.confirm_modal.message = String::from("Tem certeza que deseja desligar o computador?");
            desk.confirm_modal.show();
            desk.confirm_modal.focus_yes = true;
            return;
        }

        // 3b. Right column system shortcuts (every single shortcut opens its real native app)
        let right_shortcuts: [AppId; 9] = [
            AppId::FileExplorer,    // Pasta Pessoal
            AppId::WordPad,         // Documentos
            AppId::MediaViewer,     // Imagens
            AppId::MediaPlayer,     // Musicas
            AppId::GamesExplorer,   // Jogos
            AppId::DiskManager,     // Computador
            AppId::ControlPanel,    // Painel de Controle
            AppId::DeviceManager,   // Dispositivos
            AppId::HelpRecovery,    // Ajuda e Restauracao
        ];
        if cx >= 8 + 252 && cx <= 424 {
            for (idx, &target_id) in right_shortcuts.iter().enumerate() {
                let sy = sm_y + 84 + idx * 34;
                if cy >= sy && cy <= sy + 30 {
                    desk.open_window(target_id);
                    if target_id == AppId::FileExplorer {
                        do_refresh_vfs = true;
                    }
                    drop(desk_lock);
                    if do_refresh_vfs { refresh_vfs_cache(); }
                    return;
                }
            }
        }

        // 3c. Windows 11 Start Menu interactions
        let sm_w = 640usize;
        let sm_h = 720usize;
        let sm_x = (scr_w / 2).saturating_sub(sm_w / 2);
        let sm_y = (scr_h / 2).saturating_sub(sm_h / 2);

        // 3c1. Search box click
        let search_x = sm_x + SPACE_M;
        let search_y = sm_y + SPACE_M;
        let search_w = sm_w - 2 * SPACE_M;
        let search_h = 40;
        if cx >= search_x && cx <= search_x + search_w && cy >= search_y && cy <= search_y + search_h {
            // Focus search - typing will go to search_query
            desk.start_menu_scroll = 0;
            return;
        }

        // 3c2. Pinned apps grid (5 columns x 2 rows)
        let pinned_y = sm_y + SPACE_M + search_h + SPACE_M + 22;
        let pin_col_w = 100usize;
        let pin_col_h = 100usize;
        let pin_gap = 12usize;
        let pinned_apps: [AppId; 10] = [
            AppId::FileExplorer, AppId::Terminal, AppId::WebBrowser,
            AppId::ControlPanel, AppId::AppStore, AppId::Calculator,
            AppId::Notepad, AppId::PaintStudio, AppId::TaskManager, AppId::SystemInfo,
        ];
        for (idx, &app_id) in pinned_apps.iter().enumerate() {
            let col = idx % 5;
            let row = idx / 5;
            let px = sm_x + SPACE_M + col * (pin_col_w + pin_gap);
            let py = pinned_y + row * (pin_col_h + pin_gap);
            if py + pin_col_h > sm_y + sm_h - SPACE_M { break; }
            if cx >= px && cx <= px + pin_col_w && cy >= py && cy <= py + pin_col_h {
                desk.open_window(app_id);
                if app_id == AppId::FileExplorer { do_refresh_vfs = true; }
                desk.start_menu_open = false;
                drop(desk_lock);
                if do_refresh_vfs { refresh_vfs_cache(); }
                return;
            }
        }

        // 3c3. Category pills c1-c7
        let all_apps_y = pinned_y + 2 * (pin_col_h + pin_gap) + SPACE_M;
        let cat_pill_y = all_apps_y + 24;
        let cat_labels = ["Todos", "c1 Arquivos", "c2 Admin", "c3 Exec", "c4 Rede", "c5 Office", "c6 Media", "c7 Jogos"];
        for (ci, _clabel) in cat_labels.iter().enumerate() {
            let cpx = sm_x + SPACE_M + ci * 92;
            let cpy = cat_pill_y;
            let cpw = 84;
            let cph = 30;
            if cpx + cpw > sm_x + sm_w - SPACE_M { break; }
            if cx >= cpx && cx <= cpx + cpw && cy >= cpy && cy <= cpy + cph {
                desk.start_menu_cat = ci;
                desk.start_menu_scroll = 0;
                desk.search_query.clear();
                return;
            }
        }

        // 3c4. Category app list clicks
        let cat_list_y = cat_pill_y + 30 + SPACE_M;
        let list_items = collect_start_menu_apps(desk);
        let page_size = 7usize;
        let start_idx = desk.start_menu_scroll.min(list_items.len());
        let visible_end = (start_idx + page_size).min(list_items.len());
        for (row, &app_id) in list_items[start_idx..visible_end].iter().enumerate() {
            let iy = cat_list_y + row * 44;
            if iy + 36 > cat_list_y + 200 { break; } // approximate list height
            if cx >= sm_x + SPACE_M && cx <= sm_x + sm_w - SPACE_M && cy >= iy && cy <= iy + 36 {
                desk.open_window(app_id);
                if app_id == AppId::FileExplorer { do_refresh_vfs = true; }
                desk.start_menu_open = false;
                drop(desk_lock);
                if do_refresh_vfs { refresh_vfs_cache(); }
                return;
            }
        }

        // 3c5. Pagination controls for category list
        if list_items.len() > page_size {
            let pag_y = cat_list_y + 200 + 4; // approximate
            if cy >= pag_y && cy <= pag_y + 22 {
                if cx >= sm_x + SPACE_M && cx <= sm_x + SPACE_M + 100 {
                    desk.start_menu_scroll = desk.start_menu_scroll.saturating_sub(page_size);
                    return;
                }
                if cx >= sm_x + sm_w - SPACE_M - 100 && cx <= sm_x + sm_w - SPACE_M {
                    if start_idx + page_size < list_items.len() {
                        desk.start_menu_scroll += page_size;
                    }
                    return;
                }
            }
        }

        // 3c6. Recommended apps
        let rec_y = sm_y + sm_h - 120;
        let rec_w = 120usize;
        let rec_h = 70usize;
        let rec_apps: [AppId; 4] = [AppId::Terminal, AppId::FileExplorer, AppId::SystemInfo, AppId::ControlPanel];
        for (ri, &app_id) in rec_apps.iter().enumerate() {
            let rx = sm_x + SPACE_M + ri * (rec_w + 12);
            let ry = rec_y + 22;
            if rx + rec_w > sm_x + sm_w - SPACE_M { break; }
            if cx >= rx && cx <= rx + rec_w && cy >= ry && cy <= ry + rec_h {
                desk.open_window(app_id);
                if app_id == AppId::FileExplorer { do_refresh_vfs = true; }
                desk.start_menu_open = false;
                drop(desk_lock);
                if do_refresh_vfs { refresh_vfs_cache(); }
                return;
            }
        }

        // 3c7. Power button
        let pwr_x = sm_x + sm_w - SPACE_M - 100;
        let pwr_y = sm_y + sm_h - SPACE_M - 44;
        let pwr_w = 96;
        let pwr_h = 36;
        if cx >= pwr_x && cx <= pwr_x + pwr_w && cy >= pwr_y && cy <= pwr_y + pwr_h {
            desk.confirm_modal.title = String::from("Desligar o HybridOS?");
            desk.confirm_modal.message = String::from("Tem certeza que deseja desligar o computador?");
            desk.confirm_modal.show();
            desk.confirm_modal.focus_yes = true;
            return;
        }

        return;
    } else if desk.start_menu_open {
        desk.start_menu_open = false;
    }

    // 4. Superbar Taskbar & System Tray
    let tb_y = scr_h - 40;
    if cy >= tb_y {
        // 4a. System Tray interactive hitboxes (Zero decorative tray icons)
        let tray_x = scr_w - 245;
        if cx >= tray_x && cx < tray_x + 95 {
            desk.open_window(AppId::NetworkCenter);
            return;
        }
        if cx >= tray_x + 95 && cx < tray_x + 145 {
            desk.open_window(AppId::VolumeMixer);
            return;
        }
        if cx >= tray_x + 145 && cx < scr_w - 15 {
            desk.open_window(AppId::CalendarClock);
            return;
        }

        // 4b. Open window tabs on Superbar
        let open_indices: Vec<usize> = desk.windows.iter().enumerate()
            .filter(|(_, w)| w.open)
            .map(|(i, _)| i)
            .collect();
        let tab_count = open_indices.len().min(7);
        for slot in 0..tab_count {
            let bx = 60 + slot * 136;
            if cx >= bx && cx < bx + 130 {
                let w_idx = open_indices[slot];
                if !desk.windows[w_idx].minimized && desk.focused_app == desk.windows[w_idx].id {
                    desk.windows[w_idx].minimized = true;
                } else {
                    desk.windows[w_idx].minimized = false;
                    desk.focused_app = desk.windows[w_idx].id;
                }
                return;
            }
        }
        return;
    }

    // 5. Windows interactions (check focused window first, then reverse order)
    let focused_id = desk.focused_app;
    let mut check_order: Vec<usize> = Vec::with_capacity(desk.windows.len());
    if let Some(fi) = desk.windows.iter().position(|w| w.id == focused_id) {
        check_order.push(fi);
    }
    for i in (0..desk.windows.len()).rev() {
        if desk.windows[i].id != focused_id {
            check_order.push(i);
        }
    }

    for i in check_order {
        let (w_open, w_min, w_x, w_y, w_w, w_h, w_id) = {
            let w = &desk.windows[i];
            (w.open, w.minimized, w.x, w.y, w.w, w.h, w.id)
        };

        if !w_open || w_min {
            continue;
        }

        if cx >= w_x && cx < w_x + w_w && cy >= w_y && cy < w_y + w_h {
            desk.focused_app = w_id;

            // Check Padronized Control Capsule (top-right: w_x + w_w - 108 .. w_x + w_w - 12)
            // 3 botões de 32px com 4px de espaçamento
            let btn_w = 32;
            let cap_x = w_x + w_w - (btn_w * 3 + 12);
            let cap_y = w_y + 6;
            if cy >= cap_y && cy <= cap_y + 26 {
                // Minimize [-]
                if cx >= cap_x && cx < cap_x + btn_w {
                    desk.windows[i].minimized = true;
                    return;
                }
                // Maximize/Restore [□]
                if cx >= cap_x + btn_w + 4 && cx < cap_x + (btn_w + 4) * 2 {
                    let win = &mut desk.windows[i];
                    if win.maximized {
                        win.x = win.saved_x;
                        win.y = win.saved_y;
                        win.w = win.saved_w;
                        win.h = win.saved_h;
                        win.maximized = false;
                    } else {
                        win.saved_x = win.x;
                        win.saved_y = win.y;
                        win.saved_w = win.w;
                        win.saved_h = win.h;
                        win.x = 0;
                        win.y = 24;
                        win.w = scr_w;
                        win.h = scr_h - 64;
                        win.maximized = true;
                    }
                    return;
                }
                // Close [×]
                if cx >= cap_x + (btn_w + 4) * 2 && cx <= cap_x + (btn_w + 4) * 2 + btn_w {
                    desk.windows[i].open = false;
                    return;
                }
            }

            // Window titlebar dragging start
            if cy < w_y + 32 {
                desk.dragging_window = Some(i);
                desk.drag_offset_x = cx.saturating_sub(w_x);
                desk.drag_offset_y = cy.saturating_sub(w_y);
                return;
            }

            // Delegate client-area clicks to the dedicated category handler
            let _ = dispatch_app_click(w_id, cx, cy, desk);
            if w_id == AppId::FileExplorer {
                do_refresh_vfs = true;
            }

            drop(desk_lock);
            if do_refresh_vfs { refresh_vfs_cache(); }
            return;
        }
    }

    // 6. Desktop Icons on Left Column (7 quick-launch icons covering all 7 categories)
    let desktop_shortcuts: [AppId; 7] = [
        AppId::Terminal,
        AppId::FileExplorer,
        AppId::WebBrowser,
        AppId::TaskManager,
        AppId::PaintStudio,
        AppId::GamesExplorer,
        AppId::Personalization,
    ];
    if cx >= 16 && cx <= 96 {
        for (i, &target_id) in desktop_shortcuts.iter().enumerate() {
            let iy = 36 + i * 94;
            if cy >= iy && cy <= iy + 82 {
                desk.open_window(target_id);
                if target_id == AppId::FileExplorer {
                    do_refresh_vfs = true;
                }
                break;
            }
        }
    }

    drop(desk_lock);
    if do_refresh_vfs { refresh_vfs_cache(); }
}

/// Central click dispatcher routing all 50 Tier-0 applications to their category modules.
fn dispatch_app_click(id: AppId, cx: usize, cy: usize, desk: &mut DesktopState) -> bool {
    let n = id.as_u8();
    match n {
        1..=8   => apps_cat1_files::handle_cat1_click(n, cx, cy, desk),
        9..=18  => apps_cat2_admin::handle_cat2_click(n, cx, cy, desk),
        19..=25 => apps_cat3_exec::handle_cat3_click(n, cx, cy, desk),
        26..=33 => apps_cat4_net::handle_cat4_click(n, cx, cy, desk),
        34..=41 => apps_cat5_office::handle_cat5_click(n, cx, cy, desk),
        42..=47 => apps_cat6_media::handle_cat6_click(n, cx, cy, desk),
        48..=50 => apps_cat7_extra::handle_cat7_click(n, cx, cy, desk),
        _ => false,
    }
}

/// Central render dispatcher routing all 50 Tier-0 applications to their category modules.
unsafe fn dispatch_app_render(
    id: AppId,
    fb: *mut u8, pitch: usize, scr_w: usize, scr_h: usize,
    cx: usize, cy: usize, cw: usize, ch: usize,
    desk: &mut DesktopState,
    current_input: &str,
    prompt: &str,
) {
    let n = id.as_u8();
    match n {
        1..=8   => apps_cat1_files::render_cat1_app(n, fb, pitch, scr_w, scr_h, cx, cy, cw, ch, desk),
        9..=18  => apps_cat2_admin::render_cat2_app(n, fb, pitch, scr_w, scr_h, cx, cy, cw, ch, desk),
        19 => {
            apps_cat3_exec::render_cat3_app(19, fb, pitch, scr_w, scr_h, cx, cy, cw, ch, desk);
            // Overlay live shell command input line at bottom of Terminal Tri-Shell canvas
            if !current_input.is_empty() || !prompt.is_empty() {
                let mut live = String::from(prompt);
                live.push_str(current_input);
                live.push('_');
                let bar_y = cy + ch.saturating_sub(42);
                fill_rounded_rect(fb, pitch, scr_w, scr_h, cx + 6, bar_y, cw.saturating_sub(12), 18, 2, 0x000B132B);
                draw_text_clipped(fb, pitch, scr_w, scr_h, cx + 10, bar_y + 1, cx + cw.saturating_sub(10), &live, 0x0038BDF8, None);
            }
        }
        20..=25 => apps_cat3_exec::render_cat3_app(n, fb, pitch, scr_w, scr_h, cx, cy, cw, ch, desk),
        26..=33 => apps_cat4_net::render_cat4_app(n, fb, pitch, scr_w, scr_h, cx, cy, cw, ch, desk),
        34..=41 => apps_cat5_office::render_cat5_app(n, fb, pitch, scr_w, scr_h, cx, cy, cw, ch, desk),
        42..=47 => apps_cat6_media::render_cat6_app(n, fb, pitch, scr_w, scr_h, cx, cy, cw, ch, desk),
        48..=50 => apps_cat7_extra::render_cat7_app(n, fb, pitch, scr_w, scr_h, cx, cy, cw, ch, desk),
        _ => {}
    }
}

unsafe fn draw_wallpaper(fb: *mut u8, pitch: usize, scr_w: usize, scr_h: usize, preset: u8, accent: u32) {
    let wall_h = scr_h.saturating_sub(40);
    match preset {
        1 => {
            // Abstrato Cristalino: deep sapphire to cyan-teal with translucent diagonal facets
            fill_rounded_rect_gradient(fb, pitch, scr_w, scr_h, 0, 0, scr_w, wall_h, 0, 0x000A2540, 0x000E7490);
            fill_rounded_rect_gradient(fb, pitch, scr_w, scr_h, scr_w / 3, 80, scr_w / 2, wall_h / 2, 24, 0x00155E75, accent);
        }
        2 => {
            // Minimalista Escuro: sleek obsidian charcoal gradient
            fill_rounded_rect_gradient(fb, pitch, scr_w, scr_h, 0, 0, scr_w, wall_h, 0, 0x00090D16, 0x0018181B);
        }
        _ => {
            // Gradiente Aurora (Default Crystal Glass)
            fill_rounded_rect_gradient(fb, pitch, scr_w, scr_h, 0, 0, scr_w, wall_h, 0, 0x000F172A, 0x001E3A5F);
        }
    }
}

pub fn redraw_full_desktop(current_input: &str, prompt: &str) {
    let (mx, my) = {
        let m = MOUSE_STATE.lock();
        (m.x, m.y)
    };

    let mut con_lock = GOP_CONSOLE.lock();
    let mut desk_lock = DESKTOP.lock();

    let (con, desk) = match (&mut *con_lock, &mut *desk_lock) {
        (Some(ref mut c), Some(ref mut d)) => (c, d),
        _ => return,
    };

    if con.fb_ptr.is_null() {
        return;
    }

    let fb = con.fb_ptr;
    let pitch = con.pitch;
    let scr_w = con.width;
    let scr_h = con.height;

    let theme = apps_cat6_media::get_current_theme();
    let (rtc_h, rtc_m) = read_cmos_rtc_hm();
    let clock_str = format!("{:02}:{:02}", rtc_h, rtc_m);

    unsafe {
        // 1. Live Desktop Wallpaper driven by Personalization Studio (#47)
        draw_wallpaper(fb, pitch, scr_w, scr_h, theme.wallpaper_preset, theme.accent_color);

        // Top Subtle Info Line
        let banner = format!(
            "HybridOS Crystal Glass Desktop | 50 Programas Nativos | Tema: {}",
            theme.theme_name
        );
        draw_text_px(fb, pitch, scr_w, scr_h, 16, 6, &banner, theme.accent_color, None);

        // 2. Desktop Quick-Launch Icons on Left Column (7 categories represented)
        let desktop_icons: [(&[u8], &str); 7] = [
            (&ICON_TERMINAL_BMP[..], "Terminal"),
            (&ICON_FOLDER_BMP[..],   "Arquivos"),
            (&ICON_INFO_BMP[..],     "Browser"),
            (&ICON_CPU_BMP[..],      "Tarefas"),
            (&ICON_FOLDER_BMP[..],   "Paint"),
            (&ICON_CPU_BMP[..],      "Jogos"),
            (&ICON_INFO_BMP[..],     "Temas"),
        ];
        for (i, (bmp_data, label)) in desktop_icons.iter().enumerate() {
            let bx = 18;
            let by = 36 + i * 94;
            let bw = 74;
            let bh = 78;

            draw_soft_drop_shadow(fb, pitch, scr_w, scr_h, bx, by, bw, bh, 6, 4, 3, 30);
            fill_rounded_rect(fb, pitch, scr_w, scr_h, bx, by, bw, bh, 8, theme.frame_inactive_color);
            draw_rounded_rect_border(fb, pitch, scr_w, scr_h, bx, by, bw, bh, 8, theme.accent_color);
            draw_bmp(fb, pitch, scr_w, scr_h, bx + 21, by + 10, bmp_data);

            let text_x = bx + (bw.saturating_sub(label.len() * 8)) / 2;
            draw_text_px(fb, pitch, scr_w, scr_h, text_x, by + 54, label, 0x00F8FAFC, None);
        }

        // 3. Render Open Windows with Crystal Glass Shader & Dynamic Z-ordering
        let focused = desk.focused_app;
        let win_count = desk.windows.len();
        for pass in 0..2 {
            for idx in 0..win_count {
                let (w_open, w_min, w_id) = {
                    let w = &desk.windows[idx];
                    (w.open, w.minimized, w.id)
                };
                let is_focused = w_id == focused;
                if (pass == 0 && is_focused) || (pass == 1 && !is_focused) {
                    continue;
                }
                if !w_open || w_min {
                    continue;
                }
                draw_single_crystal_window(fb, pitch, scr_w, scr_h, desk, idx, is_focused, current_input, prompt);
            }
        }

        // 4. Vitrified Superbar (40px) along screen bottom with live Peek hover
        let peek_hover = mx >= (scr_w.saturating_sub(15)) as i32 && my >= (scr_h.saturating_sub(40)) as i32;
        draw_vitrified_superbar(fb, pitch, scr_w, scr_h, 40, peek_hover);

        // 5. Windows 11 Style Start Button (left corner)
        let start_btn_x = 12;
        let start_btn_y = scr_h - 34;
        let start_btn_w = 56;
        let start_btn_h = 26;
        let start_hover = mx >= start_btn_x as i32 && mx <= (start_btn_x + start_btn_w) as i32
            && my >= start_btn_y as i32 && my <= (start_btn_y + start_btn_h) as i32;
        let start_bg = if desk.start_menu_open {
            ACCENT_PRIMARY
        } else if start_hover {
            ACCENT_HOVER
        } else {
            BG_SURFACE
        };
        fill_rounded_rect(fb, pitch, scr_w, scr_h, start_btn_x, start_btn_y, start_btn_w, start_btn_h, BORDER_RADIUS_M, start_bg);
        draw_rounded_rect_border(fb, pitch, scr_w, scr_h, start_btn_x, start_btn_y, start_btn_w, start_btn_h, BORDER_RADIUS_M, BORDER_FOCUSED);
        draw_text_clipped(fb, pitch, scr_w, scr_h, start_btn_x + 16, start_btn_y + 3, start_btn_x + start_btn_w - 4, "Iniciar", TEXT_PRIMARY, None);
        draw_text_clipped(fb, pitch, scr_w, scr_h, start_btn_x + 6, start_btn_y + 6, start_btn_x + 12, "🪟", if start_hover { TEXT_PRIMARY } else { TEXT_SECONDARY }, None);

        // 6. Crystal Sphere Emblem (44px) on left corner (moved right of Start button)
        draw_crystal_sphere_emblem(fb, pitch, scr_w, scr_h, 82, scr_h - 22, desk.start_menu_open);

        // 7. Superbar Open Window Task Tabs (up to 7 simultaneously)
        let mut slot = 0usize;
        for w in desk.windows.iter() {
            if !w.open || slot >= 7 {
                continue;
            }
            let bx = 60 + slot * 136;
            let by = scr_h - 36;
            let bw = 130;
            let bh = 30;

            let is_focused = !w.minimized && desk.focused_app == w.id;
            let tab_bg = if is_focused {
                theme.frame_active_color
            } else if !w.minimized {
                0x001E293B
            } else {
                0x000F172A
            };

            fill_rounded_rect(fb, pitch, scr_w, scr_h, bx, by, bw, bh, 4, tab_bg);
            draw_rounded_rect_border(
                fb, pitch, scr_w, scr_h, bx, by, bw, bh, 4,
                if is_focused { theme.accent_color } else { 0x00334155 },
            );

            let mut tab_title = String::from(w.title);
            if tab_title.len() > 14 {
                tab_title.truncate(14);
            }
            draw_text_clipped(fb, pitch, scr_w, scr_h, bx + 8, by + 7, bx + bw - 6, &tab_title, 0x00F8FAFC, None);
            slot += 1;
        }

        // 7. System Tray (Real E1000 Status, HDA Audio, Live CMOS RTC Clock)
        let tray_x = scr_w - 245;
        let tray_y = scr_h - 30;
        let net_up = crate::drivers::e1000::GLOBAL_E1000.lock().is_some();
        draw_text_px(
            fb, pitch, scr_w, scr_h, tray_x, tray_y,
            if net_up { "[NET] UP" } else { "[NET] --" },
            if net_up { 0x004ADE80 } else { 0x00F87171 },
            None,
        );
        draw_text_px(fb, pitch, scr_w, scr_h, tray_x + 95, tray_y, "[HDA]", theme.accent_color, None);
        draw_text_px(fb, pitch, scr_w, scr_h, tray_x + 150, tray_y, &clock_str, 0x00F8FAFC, None);

        // 8. Windows 11 Style Start Menu
        if desk.start_menu_open {
            let sm_w = 640usize;
            let sm_h = 720usize;
            let sm_x = (scr_w / 2).saturating_sub(sm_w / 2);
            let sm_y = (scr_h / 2).saturating_sub(sm_h / 2);

            // Soft drop shadow
            draw_soft_drop_shadow(fb, pitch, scr_w, scr_h, sm_x, sm_y, sm_w, sm_h, 10, 16, 6, 100);

            // Main menu background (Mica / Acrylic effect)
            fill_rounded_rect(fb, pitch, scr_w, scr_h, sm_x, sm_y, sm_w, sm_h, BORDER_RADIUS_L, BG_SURFACE);
            draw_rounded_rect_border(fb, pitch, scr_w, scr_h, sm_x, sm_y, sm_w, sm_h, BORDER_RADIUS_L, BORDER_SUBTLE);

            // Header: Search box at top
            let search_x = sm_x + SPACE_M;
            let search_y = sm_y + SPACE_M;
            let search_w = sm_w - 2 * SPACE_M;
            let search_h = 40;
            fill_rounded_rect(fb, pitch, scr_w, scr_h, search_x, search_y, search_w, search_h, BORDER_RADIUS_M, 0x001E293B);
            draw_rounded_rect_border(fb, pitch, scr_w, scr_h, search_x, search_y, search_w, search_h, BORDER_RADIUS_M, BORDER_FOCUSED);
            // Search icon
            draw_text_clipped(fb, pitch, scr_w, scr_h, search_x + 12, search_y + 10, search_x + 24, "🔍", TEXT_MUTED, None);
            // Search text or placeholder
            if desk.search_query.is_empty() {
                draw_text_clipped(fb, pitch, scr_w, scr_h, search_x + 28, search_y + 10, search_x + search_w - 12, "Pesquisar programas, arquivos e configurações", TEXT_MUTED, None);
            } else {
                let mut q = desk.search_query.clone();
                q.push('_');
                draw_text_clipped(fb, pitch, scr_w, scr_h, search_x + 28, search_y + 10, search_x + search_w - 12, &q, TEXT_PRIMARY, None);
            }

            // Section divider
            let section_y = sm_y + SPACE_M + search_h + SPACE_M;

            // Pinned Apps section header
            draw_text_clipped(fb, pitch, scr_w, scr_h, sm_x + SPACE_M, section_y, sm_x + sm_w - SPACE_M, "Fixados", TEXT_SECONDARY, None);
            let pinned_y = section_y + 22;

            // Pinned apps grid (5 columns x 2 rows = 10 pinned apps)
            let pinned_apps: [AppId; 10] = [
                AppId::FileExplorer,
                AppId::Terminal,
                AppId::WebBrowser,
                AppId::ControlPanel,
                AppId::AppStore,
                AppId::Calculator,
                AppId::Notepad,
                AppId::PaintStudio,
                AppId::TaskManager,
                AppId::SystemInfo,
            ];

            let pin_col_w = 100usize;
            let pin_col_h = 100usize;
            let pin_gap = 12usize;
            for (idx, &app_id) in pinned_apps.iter().enumerate() {
                let col = idx % 5;
                let row = idx / 5;
                let px = sm_x + SPACE_M + col * (pin_col_w + pin_gap);
                let py = pinned_y + row * (pin_col_h + pin_gap);
                if py + pin_col_h > sm_y + sm_h - SPACE_M { break; }

                let hovered = mx >= px as i32 && mx <= (px + pin_col_w) as i32 && my >= py as i32 && my <= (py + pin_col_h) as i32;
                if hovered {
                    fill_rounded_rect(fb, pitch, scr_w, scr_h, px, py, pin_col_w, pin_col_h, BORDER_RADIUS_M, ACCENT_HOVER);
                } else {
                    fill_rounded_rect(fb, pitch, scr_w, scr_h, px, py, pin_col_w, pin_col_h, BORDER_RADIUS_M, 0x001E293B);
                }
                draw_rounded_rect_border(fb, pitch, scr_w, scr_h, px, py, pin_col_w, pin_col_h, BORDER_RADIUS_M, if hovered { BORDER_FOCUSED } else { BORDER_SUBTLE });

                if let Some(meta) = ALL_APPS.iter().find(|m| m.id == app_id) {
                    // App icon placeholder (colored circle with first letter)
                    let icon_cx = px + pin_col_w / 2;
                    let icon_cy = py + 28;
                    fill_rounded_rect(fb, pitch, scr_w, scr_h, icon_cx - 16, icon_cy - 16, 32, 32, 8, theme.accent_color);
                    let initial = meta.title.chars().next().unwrap_or('A');
                    draw_text_clipped(fb, pitch, scr_w, scr_h, icon_cx - 4, icon_cy - 4, icon_cx + 4, &initial.to_string(), TEXT_PRIMARY, None);

                    // App name
                    let name = meta.title;
                    let name_trunc = if name.len() > 18 { &name[..18] } else { name };
                    let name_w = name_trunc.len() * 8;
                    draw_text_clipped(fb, pitch, scr_w, scr_h, px + (pin_col_w.saturating_sub(name_w)) / 2, py + 52, px + pin_col_w, name_trunc, if hovered { TEXT_PRIMARY } else { TEXT_SECONDARY }, None);
                }
            }

            // All Apps section (categories c1-c7)
            let all_apps_y = pinned_y + 2 * (pin_col_h + pin_gap) + SPACE_M;
            draw_text_clipped(fb, pitch, scr_w, scr_h, sm_x + SPACE_M, all_apps_y, sm_x + sm_w - SPACE_M, "Todos os Aplicativos", TEXT_SECONDARY, None);

            // Category pills c1-c7
            let cat_pill_y = all_apps_y + 24;
            let cph = 30;
            let cat_labels = ["Todos", "c1 Arquivos", "c2 Admin", "c3 Exec", "c4 Rede", "c5 Office", "c6 Media", "c7 Jogos"];
            for (ci, clabel) in cat_labels.iter().enumerate() {
                let cpx = sm_x + SPACE_M + ci * 92;
                let cpy = cat_pill_y;
                let cpw = 84;
                let cat_active = desk.start_menu_cat == ci;
                if cpx + cpw > sm_x + sm_w - SPACE_M { break; }
                let cbg = if cat_active { ACCENT_PRIMARY } else { 0x001E293B };
                let cfg = if cat_active { TEXT_PRIMARY } else { TEXT_SECONDARY };
                fill_rounded_rect(fb, pitch, scr_w, scr_h, cpx, cpy, cpw, cph, BORDER_RADIUS_M, cbg);
                draw_rounded_rect_border(fb, pitch, scr_w, scr_h, cpx, cpy, cpw, cph, BORDER_RADIUS_M, if cat_active { BORDER_FOCUSED } else { BORDER_SUBTLE });
                draw_text_clipped(fb, pitch, scr_w, scr_h, cpx + 6, cpy + 6, cpx + cpw - 6, clabel, cfg, None);
            }

            // Category app list (scrollable)
            let cat_list_y = cat_pill_y + cph + SPACE_M;
            let cat_list_h = sm_y + sm_h - SPACE_M - 60 - cat_list_y; // reserve space for power button
            let list_items = collect_start_menu_apps(desk);
            let page_size = 7usize;
            let start_idx = desk.start_menu_scroll.min(list_items.len());
            let visible_end = (start_idx + page_size).min(list_items.len());

            for (row, &app_id) in list_items[start_idx..visible_end].iter().enumerate() {
                let iy = cat_list_y + row * 44;
                if iy + 36 > cat_list_y + cat_list_h { break; }
                let hovered = mx >= (sm_x + SPACE_M) as i32 && mx <= (sm_x + sm_w - SPACE_M) as i32 && my >= iy as i32 && my <= (iy + 36) as i32;
                if hovered {
                    fill_rounded_rect(fb, pitch, scr_w, scr_h, sm_x + SPACE_M, iy, sm_w - 2 * SPACE_M, 36, BORDER_RADIUS_M, ACCENT_HOVER);
                }
                if let Some(meta) = ALL_APPS.iter().find(|m| m.id == app_id) {
                    let num_str = format!("{:02}", meta.id.as_u8());
                    draw_text_clipped(fb, pitch, scr_w, scr_h, sm_x + SPACE_M + 8, iy + 8, sm_x + SPACE_M + 24, &num_str, TEXT_MUTED, None);
                    draw_text_clipped(fb, pitch, scr_w, scr_h, sm_x + SPACE_M + 30, iy + 8, sm_x + sm_w - SPACE_M - 8, meta.title, if hovered { TEXT_PRIMARY } else { TEXT_SECONDARY }, None);
                }
            }

            // Pagination for category list
            if list_items.len() > page_size {
                let pag_y = cat_list_y + cat_list_h + 4;
                fill_rounded_rect(fb, pitch, scr_w, scr_h, sm_x + SPACE_M, pag_y, 100, 22, BORDER_RADIUS_M, 0x001E293B);
                draw_text_clipped(fb, pitch, scr_w, scr_h, sm_x + SPACE_M + 6, pag_y + 4, sm_x + SPACE_M + 96, "< Anterior", TEXT_SECONDARY, None);
                fill_rounded_rect(fb, pitch, scr_w, scr_h, sm_x + sm_w - SPACE_M - 100, pag_y, 100, 22, BORDER_RADIUS_M, 0x001E293B);
                draw_text_clipped(fb, pitch, scr_w, scr_h, sm_x + sm_w - SPACE_M - 94, pag_y + 4, sm_x + sm_w - SPACE_M - 6, "Próximo >", TEXT_SECONDARY, None);
            }

            // Recommended section (bottom left)
            let rec_y = sm_y + sm_h - 120;
            draw_text_clipped(fb, pitch, scr_w, scr_h, sm_x + SPACE_M, rec_y, sm_x + sm_w - SPACE_M, "Recomendados", TEXT_SECONDARY, None);
            let rec_apps: [AppId; 4] = [AppId::Terminal, AppId::FileExplorer, AppId::SystemInfo, AppId::ControlPanel];
            let rec_w = 120usize;
            let rec_h = 70usize;
            for (ri, &app_id) in rec_apps.iter().enumerate() {
                let rx = sm_x + SPACE_M + ri * (rec_w + 12);
                let ry = rec_y + 22;
                if rx + rec_w > sm_x + sm_w - SPACE_M { break; }
                let hovered = mx >= rx as i32 && mx <= (rx + rec_w) as i32 && my >= ry as i32 && my <= (ry + rec_h) as i32;
                if hovered {
                    fill_rounded_rect(fb, pitch, scr_w, scr_h, rx, ry, rec_w, rec_h, BORDER_RADIUS_M, ACCENT_HOVER);
                } else {
                    fill_rounded_rect(fb, pitch, scr_w, scr_h, rx, ry, rec_w, rec_h, BORDER_RADIUS_M, 0x001E293B);
                }
                draw_rounded_rect_border(fb, pitch, scr_w, scr_h, rx, ry, rec_w, rec_h, BORDER_RADIUS_M, if hovered { BORDER_FOCUSED } else { BORDER_SUBTLE });
                if let Some(meta) = ALL_APPS.iter().find(|m| m.id == app_id) {
                    draw_text_clipped(fb, pitch, scr_w, scr_h, rx + 8, ry + 8, rx + rec_w - 8, meta.title, if hovered { TEXT_PRIMARY } else { TEXT_SECONDARY }, None);
                }
            }

            // Power button (bottom right)
            let pwr_x = sm_x + sm_w - SPACE_M - 100;
            let pwr_y = sm_y + sm_h - SPACE_M - 44;
            let pwr_w = 96;
            let pwr_h = 36;
            let pwr_hover = mx >= pwr_x as i32 && mx <= (pwr_x + pwr_w) as i32 && my >= pwr_y as i32 && my <= (pwr_y + pwr_h) as i32;
            fill_rounded_rect(fb, pitch, scr_w, scr_h, pwr_x, pwr_y, pwr_w, pwr_h, BORDER_RADIUS_M, if pwr_hover { ERROR_COLOR } else { 0x001E293B });
            draw_rounded_rect_border(fb, pitch, scr_w, scr_h, pwr_x, pwr_y, pwr_w, pwr_h, BORDER_RADIUS_M, BORDER_SUBTLE);
            draw_text_clipped(fb, pitch, scr_w, scr_h, pwr_x + 12, pwr_y + 8, pwr_x + pwr_w - 12, "Desligar", TEXT_PRIMARY, None);
        }

        // 9. Tooltip flutuante (botões ícones, controles)
        if desk.tooltip.visible && !desk.tooltip.text.is_empty() {
            let tx = desk.tooltip.x.max(0) as usize;
            let ty = desk.tooltip.y.max(0) as usize;
            let tw = (desk.tooltip.text.len() * 8 + SPACE_M).min(scr_w.saturating_sub(SPACE_S));
            let th = BTN_HEIGHT;
            let tooltip_x = tx.saturating_sub(tw / 2);
            let tooltip_y = ty.saturating_sub(th + SPACE_S);
            fill_rounded_rect(fb, pitch, scr_w, scr_h, tooltip_x, tooltip_y, tw, th, BORDER_RADIUS_M, BG_SURFACE);
            draw_rounded_rect_border(fb, pitch, scr_w, scr_h, tooltip_x, tooltip_y, tw, th, BORDER_RADIUS_M, BORDER_FOCUSED);
            draw_text_clipped(fb, pitch, scr_w, scr_h, tooltip_x + SPACE_S, tooltip_y + 4, tooltip_x + tw - SPACE_S, &desk.tooltip.text, TEXT_PRIMARY, None);
        }

        // 10. Modal de confirmação (ex: desligar)
        if desk.confirm_modal.visible {
            let mw = 320;
            let mh = 140;
            let mx_modal = (scr_w / 2).saturating_sub(mw / 2);
            let my_modal = (scr_h / 2).saturating_sub(mh / 2);
            fill_rounded_rect(fb, pitch, scr_w, scr_h, mx_modal, my_modal, mw, mh, BORDER_RADIUS_M, BG_SURFACE);
            draw_rounded_rect_border(fb, pitch, scr_w, scr_h, mx_modal, my_modal, mw, mh, BORDER_RADIUS_M, BORDER_FOCUSED);
            draw_text_clipped(fb, pitch, scr_w, scr_h, mx_modal + SPACE_M, my_modal + SPACE_S, mx_modal + mw - SPACE_M, &desk.confirm_modal.title, TEXT_PRIMARY, None);
            draw_text_clipped(fb, pitch, scr_w, scr_h, mx_modal + SPACE_M, my_modal + SPACE_M + 20, mx_modal + mw - SPACE_M, &desk.confirm_modal.message, TEXT_SECONDARY, None);
            // Botões Sim/Não
            let btn_y = my_modal + mh - SPACE_M - BTN_HEIGHT;
            let sim_x = mx_modal + SPACE_M;
            let nao_x = mx_modal + mw - SPACE_M - BTN_HEIGHT;
            let sim_hover = desk.confirm_modal.focus_yes;
            let nao_hover = !desk.confirm_modal.focus_yes;
            fill_rounded_rect(fb, pitch, scr_w, scr_h, sim_x, btn_y, BTN_HEIGHT + 16, BTN_HEIGHT, BORDER_RADIUS_M, if sim_hover { ACCENT_PRIMARY } else { BORDER_SUBTLE });
            draw_text_clipped(fb, pitch, scr_w, scr_h, sim_x + 12, btn_y + 4, sim_x + BTN_HEIGHT + 16, "Sim", TEXT_PRIMARY, None);
            fill_rounded_rect(fb, pitch, scr_w, scr_h, nao_x, btn_y, BTN_HEIGHT + 16, BTN_HEIGHT, BORDER_RADIUS_M, if nao_hover { ACCENT_PRIMARY } else { BORDER_SUBTLE });
            draw_text_clipped(fb, pitch, scr_w, scr_h, nao_x + 12, btn_y + 4, nao_x + BTN_HEIGHT + 16, "Não", TEXT_PRIMARY, None);
        }

        // 11. Focus Ring — indica visual do widget com foco (navegação por teclado Tab)
        if let Some(focused_win) = desk.windows.iter().find(|w| w.id == desk.focused_app && w.open && !w.minimized) {
            let fx = focused_win.x;
            let fy = focused_win.y;
            let fw = focused_win.w;
            let fh = focused_win.h;
            draw_keyboard_focus_ring(fb, pitch, scr_w, scr_h, fx, fy, fw, fh);
        }

        // 12. Indicador de Alto Contraste (pequeno ícone no canto superior esquerdo quando ativo)
        if desk.high_contrast {
            fill_rounded_rect(fb, pitch, scr_w, scr_h, 16, 16, 20, 20, 4, HC_ACCENT);
            draw_text_clipped(fb, pitch, scr_w, scr_h, 18, 20, 36, "HC", HC_BG, None);
        }

        // 13. Save background and draw smooth mouse cursor
        desk.needs_full_redraw = false;
        desk.cursor_saved_valid = false;
        save_and_draw_cursor(fb, pitch, scr_w, scr_h, desk, mx, my);
    }
}

unsafe fn draw_single_crystal_window(
    fb: *mut u8,
    pitch: usize,
    scr_w: usize,
    scr_h: usize,
    desk: &mut DesktopState,
    idx: usize,
    is_focused: bool,
    current_input: &str,
    prompt: &str,
) {
    let (w_id, w_title, w_x, w_y, w_w, w_h) = {
        let w = &desk.windows[idx];
        (w.id, w.title, w.x, w.y, w.w, w.h)
    };

    // 1. Crystal Glass Window Frame with live theme color & specular reflections
    draw_crystal_glass_window_frame(fb, pitch, scr_w, scr_h, w_x, w_y, w_w, w_h, is_focused);

    // 2. Window Title Text
    let title_color = if is_focused { 0x00FFFFFF } else { 0x00CBD5E1 };
    draw_text_clipped(fb, pitch, scr_w, scr_h, w_x + 14, w_y + 8, w_x + w_w - 100, w_title, title_color, None);

    // 3. Padronized 3-Button Control Capsule (Minimize, Maximize, Close)
    //    [-]  [□]  [×]  — tooltips ao hover, fechar em destaque vermelho
    let (mx, my) = {
        let m = MOUSE_STATE.lock();
        (m.x.max(0) as usize, m.y.max(0) as usize)
    };
    let btn_w = 32;
    let btn_h = 26;
    let cap_x = w_x + w_w - (btn_w * 3 + 12);
    let cap_y = w_y + 6;

    let min_x = cap_x;
    let max_x = cap_x + btn_w + 4;
    let cls_x = cap_x + (btn_w + 4) * 2;

    let hov_min = my >= cap_y && my <= cap_y + btn_h && mx >= min_x && mx < min_x + btn_w;
    let hov_max = my >= cap_y && my <= cap_y + btn_h && mx >= max_x && mx < max_x + btn_w;
    let hov_cls = my >= cap_y && my <= cap_y + btn_h && mx >= cls_x && mx <= cls_x + btn_w;

    // Desenha os 3 botões com estados visuais
    let btn_states = [
        (min_x, "Minimizar", hov_min, ButtonState::Hover),
        (max_x, "Maximizar", hov_max, ButtonState::Hover),
        (cls_x, "Fechar", hov_cls, ButtonState::Hover),
    ];

    for (bx, label, hovered, _) in btn_states.iter() {
        let is_close = *label == "Fechar";
        let state = if *hovered {
            ButtonState::Hover
        } else {
            ButtonState::Normal
        };
        let bg = if is_close && *hovered { ERROR_COLOR } else { state.bg_color() };
        fill_rounded_rect(fb, pitch, scr_w, scr_h, *bx, cap_y, btn_w, btn_h, BORDER_RADIUS_M, bg);
        draw_rounded_rect_border(fb, pitch, scr_w, scr_h, *bx, cap_y, btn_w, btn_h, BORDER_RADIUS_M, BORDER_SUBTLE);

        let glyph = if *label == "Minimizar" { "–" } else if *label == "Maximizar" { "□" } else { "×" };
        let glyph_color = if is_close && *hovered { TEXT_PRIMARY } else { state.text_color() };
        let glyph_x = bx + (btn_w.saturating_sub(8)) / 2;
        draw_text_clipped(fb, pitch, scr_w, scr_h, glyph_x, cap_y + 4, bx + btn_w, glyph, glyph_color, None);

        // Tooltip ao hover
        if *hovered {
            desk.tooltip.text = String::from(*label);
            desk.tooltip.x = *bx as i32;
            desk.tooltip.y = (cap_y as i32) - 24;
            desk.tooltip.visible = true;
        }
    }

    // 4. Window Client Canvas Area
    let canvas_x = w_x + 4;
    let canvas_y = w_y + 32;
    let canvas_w = w_w.saturating_sub(8);
    let canvas_h = w_h.saturating_sub(36);

    fill_rounded_rect(fb, pitch, scr_w, scr_h, canvas_x, canvas_y, canvas_w, canvas_h, 4, BG_SURFACE);

    // 5. Render native application content via dedicated category module
    dispatch_app_render(
        w_id, fb, pitch, scr_w, scr_h,
        canvas_x, canvas_y, canvas_w, canvas_h,
        desk, current_input, prompt,
    );
}
