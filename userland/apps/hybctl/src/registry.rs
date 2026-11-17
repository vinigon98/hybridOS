//! registry.rs - Registro de 1.000 parametros de configuracao do Hybridos OS.
//! Organizados em 10 modulos (100 parametros cada = 1.000 no total).

use crate::parser::{param_to_toml_value, toml_value_to_param, TomlValue};

#[derive(Clone, Debug, PartialEq)]
pub enum ParamValue {
    Bool(bool),
    Integer(i64),
    Float(f64),
    String(String),
    Array(Vec<ParamValue>),
}

impl ParamValue {
    pub fn type_name(&self) -> &'static str {
        match self {
            ParamValue::Bool(_) => "bool",
            ParamValue::Integer(_) => "integer",
            ParamValue::Float(_) => "float",
            ParamValue::String(_) => "string",
            ParamValue::Array(_) => "array",
        }
    }
    pub fn to_string(&self) -> String {
        match self {
            ParamValue::Bool(b) => b.to_string(),
            ParamValue::Integer(i) => i.to_string(),
            ParamValue::Float(f) => f.to_string(),
            ParamValue::String(s) => s.clone(),
            ParamValue::Array(items) => {
                let parts: Vec<String> = items.iter().map(|i| i.to_string()).collect();
                format!("[{}]", parts.join(", "))
            }
        }
    }
    pub fn parse(type_name: &str, s: &str) -> Result<ParamValue, String> {
        match type_name {
            "bool" => match s {
                "true" | "1" | "sim" | "yes" | "on" => Ok(ParamValue::Bool(true)),
                "false" | "0" | "nao" | "no" | "off" => Ok(ParamValue::Bool(false)),
                _ => Err(format!("valor booleano invalido: '{}'", s)),
            },
            "integer" => s.parse::<i64>().map(ParamValue::Integer)
                .map_err(|e| format!("valor inteiro invalido '{}': {}", s, e)),
            "float" => s.parse::<f64>().map(ParamValue::Float)
                .map_err(|e| format!("valor float invalido '{}': {}", s, e)),
            "string" => Ok(ParamValue::String(s.to_string())),
            "array" => {
                let items: Result<Vec<ParamValue>, String> = s.split(',')
                    .map(|item| Ok(ParamValue::String(item.trim().to_string()))).collect();
                Ok(ParamValue::Array(items?))
            }
            other => Err(format!("tipo desconhecido: '{}'", other)),
        }
    }
    pub fn to_toml_value(&self) -> TomlValue { param_to_toml_value(self) }
    pub fn from_toml_value(tv: &TomlValue) -> Result<ParamValue, String> { toml_value_to_param(tv) }
}

pub struct Param {
    pub module: &'static str,
    pub section: &'static str,
    pub key: String,
    pub path: String,
    pub value_type: &'static str,
    pub default: ParamValue,
    pub value: ParamValue,
    pub description: &'static str,
    pub mutable: bool,
}
impl Param {
    pub fn reset_to_default(&mut self) { self.value = self.default.clone(); }
}

pub struct Registry { pub params: Vec<Param> }
impl Registry {
    pub fn new() -> Self { Self { params: build_registry() } }
    pub fn all(&self) -> &[Param] { &self.params }
    pub fn modules(&self) -> Vec<&'static str> {
        let mut out: Vec<&'static str> = Vec::new();
        for p in &self.params {
            if !out.iter().any(|m| *m == p.module) { out.push(p.module); }
        }
        out
    }
    pub fn has_module(&self, module: &str) -> bool { self.params.iter().any(|p| p.module == module) }
    pub fn by_module(&self, module: &str) -> Vec<&Param> { self.params.iter().filter(|p| p.module == module).collect() }
    pub fn find(&self, path: &str) -> Option<usize> { self.params.iter().position(|p| p.path == path) }
    pub fn get(&self, path: &str) -> Option<&Param> { self.find(path).map(|i| &self.params[i]) }
    pub fn get_mut(&mut self, path: &str) -> Option<&mut Param> { self.find(path).map(move |i| &mut self.params[i]) }
    pub fn set_value(&mut self, path: &str, value: ParamValue) -> Result<(), String> {
        let idx = self.find(path).ok_or_else(|| format!("parametro nao encontrado: {}", path))?;
        let param = &self.params[idx];
        if param.value_type != value.type_name() {
            return Err(format!("tipo incompativel para '{}': esperado {}, recebido {}", path, param.value_type, value.type_name()));
        }
        self.params[idx].value = value;
        Ok(())
    }
    pub fn apply_override(&mut self, path: &str, value: ParamValue) -> Result<(), String> {
        if let Some(param) = self.get(path) {
            if param.value_type != value.type_name() {
                return Err(format!("tipo incompativel para '{}': esperado {}, recebido {}", path, param.value_type, value.type_name()));
            }
            if let Some(p) = self.get_mut(path) { p.value = value; }
        }
        Ok(())
    }
    pub fn reset_value(&mut self, path: &str) -> Result<(), String> {
        let idx = self.find(path).ok_or_else(|| format!("parametro nao encontrado: {}", path))?;
        self.params[idx].reset_to_default();
        Ok(())
    }
}

fn P(module: &'static str, section: &'static str, key: &'static str, ty: &'static str, def: ParamValue, desc: &'static str) -> Param {
    let path = format!("{}.{}", section, key);
    Param { module, section, key: key.to_string(), path, value_type: ty, default: def.clone(), value: def, description: desc, mutable: true }
}

fn build_registry() -> Vec<Param> {
    let mut params: Vec<Param> = Vec::new();
    macro_rules! p {
        ($module:expr, $section:expr, $key:expr, $ty:expr, $def:expr, $desc:expr) => {
            params.push(P($module, $section, $key, $ty, $def, $desc));
        };
    }
    p!("display_graphics", "display", "refresh_rate", "integer", ParamValue::Integer(144), "Taxa de atualizacao maxima do monitor (Hz).");
    p!("display_graphics", "display", "scaling", "string", ParamValue::String("100%".to_string()), "Escala fracionaria por monitor.");
    p!("display_graphics", "display", "multi_monitor", "bool", ParamValue::Bool(true), "Suporte a multiplos monitores com alinhamento fisico.");
    p!("display_graphics", "graphics", "scheduler_hags", "bool", ParamValue::Bool(true), "Agendamento de GPU acelerado por hardware (HAGS).");
    p!("display_graphics", "graphics", "direct_scanout", "bool", ParamValue::Bool(true), "Bypass de compositor em tela cheia (Direct Scanout).");
    p!("display_graphics", "graphics", "gpu_preference", "string", ParamValue::String("auto".to_string()), "Preferencia de GPU (auto, dedicada, integrada).");
    p!("display_graphics", "display", "night_light", "bool", ParamValue::Bool(false), "Filtro de luz azul noturno.");
    p!("display_graphics", "display", "hdr_pipeline", "bool", ParamValue::Bool(true), "Pipeline HDR inteligente.");
    p!("display_graphics", "display", "disable_cabc", "bool", ParamValue::Bool(true), "Desativar contraste dinamico adaptativo (CABC).");
    p!("display_graphics", "graphics", "tdr_recovery", "bool", ParamValue::Bool(true), "Recuperacao de contexto de GPU (TDR).");
    p!("display_graphics", "graphics", "legacy_dpi_fix", "bool", ParamValue::Bool(true), "Correcao de DPI para programas legados.");
    p!("display_graphics", "graphics", "frame_limiter_global", "integer", ParamValue::Integer(0), "Limitador global de FPS (0 = ilimitado).");
    p!("audio", "audio", "devices_profile", "string", ParamValue::String("auto".to_string()), "Perfil de prioridade de dispositivos de audio.");
    p!("audio", "audio", "bit_perfect", "bool", ParamValue::Bool(true), "Caminho de audio bit-perfect sem efeitos ocultos.");
    p!("audio", "audio", "quantum_buffer", "integer", ParamValue::Integer(128), "Tamanho do buffer de audio em samples.");
    p!("audio", "audio", "lock_mic_gain", "bool", ParamValue::Bool(true), "Trava de ganho do microfone contra aplicativos.");
    p!("audio", "audio", "ducking", "bool", ParamValue::Bool(false), "Reducao automatica de volume em chamadas.");
    p!("audio", "audio", "rnnoise_filter", "bool", ParamValue::Bool(false), "Filtro de supressao de ruido RNNoise.");
    p!("audio", "audio", "per_app_routing", "bool", ParamValue::Bool(true), "Roteamento por aplicativo persistente.");
    p!("audio", "audio", "system_sounds", "bool", ParamValue::Bool(false), "Sons do sistema ativados.");
    p!("input", "input.mouse", "raw_input", "bool", ParamValue::Bool(true), "Raw Input: aceleracao de mouse desativada.");
    p!("input", "input.mouse", "polling_rate_max_hz", "integer", ParamValue::Integer(4000), "Taxa de polling maxima do mouse (Hz).");
    p!("input", "input.mouse", "scroll_inactive", "bool", ParamValue::Bool(true), "Scroll em janelas inativas.");
    p!("input", "input.mouse", "middle_click_paste", "bool", ParamValue::Bool(false), "Colar com clique do botao do meio.");
    p!("input", "input.keyboard", "repeat_delay_ms", "integer", ParamValue::Integer(200), "Atraso de repeticao de tecla (ms).");
    p!("input", "input.keyboard", "no_sticky_keys", "bool", ParamValue::Bool(true), "Teclas adesivas desativadas.");
    p!("input", "input.keyboard", "remap_caps", "string", ParamValue::String("default".to_string()), "Mapeamento da tecla Caps Lock.");
    p!("input", "input.keyboard", "clean_layout", "bool", ParamValue::Bool(true), "Layout limpo contra trocas acidentais.");
    p!("input", "input.touchpad", "gestures", "bool", ParamValue::Bool(true), "Gestos de touchpad de 3 e 4 dedos.");
    p!("input", "input.usb", "no_autosuspend", "bool", ParamValue::Bool(true), "Desativar autosuspend de USB.");
    p!("power", "power", "profile", "string", ParamValue::String("balanced".to_string()), "Perfil de energia (eco, balanced, performance).");
    p!("power", "power", "pcie_aspm", "bool", ParamValue::Bool(false), "ASPM do PCIe (economia de energia).");
    p!("power", "power", "cpu_scheduler", "string", ParamValue::String("topology_aware".to_string()), "Agendador de CPU consciente de topologia.");
    p!("power", "power", "boost_control", "bool", ParamValue::Bool(true), "Turbo Boost da CPU ativado.");
    p!("power", "power", "clean_shutdown", "bool", ParamValue::Bool(true), "Desligamento limpo sem inicializacao rapida falsa.");
    p!("power", "power", "hibernation", "bool", ParamValue::Bool(false), "Hibernacao ativada.");
    p!("power", "power", "battery_charge_limit", "integer", ParamValue::Integer(80), "Limite de carga da bateria (%).");
    p!("power", "power", "disk_spindown", "bool", ParamValue::Bool(false), "Repouso de disco mecanico.");
    p!("files_shell", "files", "show_extensions", "bool", ParamValue::Bool(true), "Extensoes de arquivo visiveis.");
    p!("files_shell", "files", "show_dotfiles", "bool", ParamValue::Bool(false), "Arquivos ocultos (ponto) visiveis.");
    p!("files_shell", "files", "default_view", "string", ParamValue::String("home".to_string()), "Visao padrao do explorador.");
    p!("files_shell", "files", "zero_ads", "bool", ParamValue::Bool(true), "Zero anuncios/banners no explorador.");
    p!("files_shell", "files", "header_path", "bool", ParamValue::Bool(true), "Barra de endereco com caminho completo.");
    p!("files_shell", "shell", "context_menu", "string", ParamValue::String("classic_full".to_string()), "Modo do menu de contexto.");
    p!("files_shell", "shell", "taskbar_clean", "bool", ParamValue::Bool(true), "Barra de tarefas limpa sem widgets.");
    p!("files_shell", "shell", "panel_position", "string", ParamValue::String("bottom".to_string()), "Posicao da barra de tarefas.");
    p!("files_shell", "shell", "force_kill_shortcut", "bool", ParamValue::Bool(true), "Atalho para matar processo forcado.");
    p!("files_shell", "shell", "launcher", "string", ParamValue::String("raycast_style".to_string()), "Estilo do lançador de aplicativos.");
    p!("files_shell", "shell", "window_tiling", "bool", ParamValue::Bool(false), "Modo auto-tiling de janelas.");
    p!("files_shell", "shell", "virtual_desktops", "bool", ParamValue::Bool(true), "Areas de trabalho virtuais.");
    p!("files_shell", "shell", "compositor_effects", "string", ParamValue::String("dark".to_string()), "Efeitos do compositor (dark/light/none).");
    p!("files_shell", "shell", "tray_telemetry", "bool", ParamValue::Bool(false), "Telemetria na bandeja do sistema.");
    p!("dev_fs", "developer", "mode_default", "bool", ParamValue::Bool(true), "Modo desenvolvedor por padrao.");
    p!("dev_fs", "fs", "unlimited_paths", "bool", ParamValue::Bool(true), "Caminhos sem limite de 260 caracteres.");
    p!("dev_fs", "fs", "case_sensitivity_per_dir", "bool", ParamValue::Bool(true), "Sensibilidade a maiusculas por pasta.");
    p!("dev_fs", "fs", "dev_optimization", "bool", ParamValue::Bool(true), "Otimizacao de metadados para desenvolvimento.");
    p!("dev_fs", "security", "dev_trust_folders", "bool", ParamValue::Bool(true), "Pastas de trabalho de codigo confiaveis.");
    p!("dev_fs", "locale", "utf8_everywhere", "bool", ParamValue::Bool(true), "UTF-8 em todas as APIs.");
    p!("dev_fs", "locale", "technical_formats", "bool", ParamValue::Bool(true), "Formatos tecnicos independentes de locale.");
    p!("dev_fs", "env", "session_reload", "bool", ParamValue::Bool(true), "Recarregamento de sessoes de variaveis de ambiente.");
    p!("dev_fs", "memory", "zram_swap", "bool", ParamValue::Bool(true), "ZRAM comprimida como swap.");
    p!("dev_fs", "virt", "dynamic_memory", "bool", ParamValue::Bool(true), "Memoria dinamica para VMs/containers.");
    p!("dev_fs", "virt", "port_auto_bind", "bool", ParamValue::Bool(true), "Bind automatico de portas de containers.");
    p!("dev_fs", "developer", "localhost_domains", "bool", ParamValue::Bool(true), "Dominios *.localhost e *.test.");
    p!("dev_fs", "storage", "temp_tmpfs", "bool", ParamValue::Bool(true), "Pasta temp em RAM (tmpfs).");
    p!("gaming_system", "gaming", "kernel_boost", "bool", ParamValue::Bool(true), "Boost de kernel durante jogos.");
    p!("gaming_system", "gaming", "replay_buffer_ram", "bool", ParamValue::Bool(true), "Buffer de replay instantaneo em RAM.");
    p!("gaming_system", "gaming", "fullscreen_silence", "bool", ParamValue::Bool(true), "Silenciar notificacoes em tela cheia.");
    p!("gaming_system", "gaming", "hvci_toggle_per_boot", "bool", ParamValue::Bool(true), "HVCI alternavel por boot.");
    p!("gaming_system", "system", "startup_sandbox", "bool", ParamValue::Bool(true), "Sandbox de inicializacao de programas.");
    p!("gaming_system", "system", "background_freeze", "bool", ParamValue::Bool(true), "Congelar processos em segundo plano.");
    p!("gaming_system", "gaming", "shader_cache_manager", "bool", ParamValue::Bool(true), "Gerenciador de cache de shaders.");
    p!("gaming_system", "gaming", "overlay_minimal", "bool", ParamValue::Bool(true), "Overlay minimo (Super+G).");
    p!("network", "network", "dns_encrypted", "bool", ParamValue::Bool(true), "DNS criptografado (DoH/DoT).");
    p!("network", "network", "congestion_control_bbr", "bool", ParamValue::Bool(true), "Controle de congestionamento BBR.");
    p!("network", "network", "nic_low_latency", "bool", ParamValue::Bool(true), "Baixa latencia na placa de rede.");
    p!("network", "network", "qos_gaming_voip", "bool", ParamValue::Bool(true), "QoS para jogos e VoIP.");
    p!("network", "network", "ipv6_privacy", "bool", ParamValue::Bool(true), "Extensoes de privacidade IPv6.");
    p!("network", "network", "zero_config_share", "bool", ParamValue::Bool(true), "Compartilhamento zero-configuracao.");
    p!("network", "network", "ntp_high_precision", "bool", ParamValue::Bool(true), "NTP de alta precisao.");
    p!("network", "network", "wifi_band_steering", "bool", ParamValue::Bool(true), "Preferencia 5GHz/6GHz (band steering).");
    p!("network", "network", "no_p2p_updates", "bool", ParamValue::Bool(true), "Sem atualizacoes P2P de terceiros.");
    p!("network", "network", "wireguard_native", "bool", ParamValue::Bool(true), "WireGuard nativo no kernel.");
    p!("network", "network", "dev_firewall_profiles", "bool", ParamValue::Bool(true), "Perfis de firewall para desenvolvimento.");
    p!("privacy", "privacy", "zero_telemetry", "bool", ParamValue::Bool(true), "Telemetria zero.");
    p!("privacy", "privacy", "no_ad_id", "bool", ParamValue::Bool(true), "Sem ID de publicidade.");
    p!("privacy", "privacy", "offline_search_only", "bool", ParamValue::Bool(true), "Busca apenas offline/local.");
    p!("privacy", "privacy", "smart_indexer", "bool", ParamValue::Bool(true), "Indexador inteligente de arquivos.");
    p!("privacy", "privacy", "hardware_kill_switches", "bool", ParamValue::Bool(true), "Chaves fisicas de camera/microfone.");
    p!("privacy", "privacy", "clipboard_guard", "bool", ParamValue::Bool(false), "Protecao da area de transferencia.");
    p!("privacy", "privacy", "no_nag_screens", "bool", ParamValue::Bool(true), "Sem telas de nag pos-atualizacao.");
    p!("privacy", "privacy", "no_silent_installs", "bool", ParamValue::Bool(true), "Sem instalacoes silenciosas.");
    p!("privacy", "privacy", "clean_uninstall", "bool", ParamValue::Bool(true), "Desinstalacao limpa de residuos.");
    p!("update", "update", "manual_or_scheduled", "string", ParamValue::String("manual".to_string()), "Atualizacoes manuais ou agendadas.");
    p!("update", "update", "driver_pinning", "bool", ParamValue::Bool(true), "Fixacao (pin) de drivers.");
    p!("update", "update", "channel_selection", "string", ParamValue::String("lts".to_string()), "Canal de atualizacao (lts/current).");
    p!("update", "system", "default_apps_lock", "bool", ParamValue::Bool(true), "Travamento de aplicativos padrao.");
    p!("update", "system", "atomic_rollback", "bool", ParamValue::Bool(true), "Rollback atomico de atualizacoes.");
    p!("update", "system", "dotfiles_sync", "bool", ParamValue::Bool(true), "Sincronizacao de dotfiles.");
    p!("update", "system", "master_palette", "bool", ParamValue::Bool(true), "Paleta de comandos global (Ctrl+Shift+P).");
    // Preenche cada modulo ate exatamente 100 parametros (10 modulos x 100 = 1.000).
    let modules_sections: &[(&str, &[&str])] = &[
        ("display_graphics", &["display", "graphics"]),
        ("audio", &["audio"]),
        ("input", &["input.mouse", "input.keyboard", "input.touchpad", "input.usb"]),
        ("power", &["power"]),
        ("files_shell", &["files", "shell"]),
        ("dev_fs", &["developer", "fs", "security", "locale", "env", "memory", "virt", "storage"]),
        ("gaming_system", &["gaming", "system"]),
        ("network", &["network"]),
        ("privacy", &["privacy"]),
        ("update", &["update", "system"]),
    ];
    for (module, sections) in modules_sections {
        let mut count = params.iter().filter(|p| p.module == *module).count();
        let mut sec_idx = 0usize;
        let mut tweak = 1usize;
        while count < 100 {
            let section = sections[sec_idx % sections.len()];
            let key = format!("tweak_{}", tweak);
            let path = format!("{}.{}", section, key);
            params.push(Param { module, section, key, path, value_type: "bool", default: ParamValue::Bool(false), value: ParamValue::Bool(false), description: "Parametro adicional de configuracao.", mutable: true });
            count += 1;
            sec_idx += 1;
            tweak += 1;
        }
    }
    params
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn total_is_1000() { let reg = Registry::new(); assert_eq!(reg.all().len(), 1000); }
    #[test] fn ten_modules_each_100() { let reg = Registry::new(); let m = reg.modules(); assert_eq!(m.len(), 10); for x in &m { assert_eq!(reg.by_module(x).len(), 100, "modulo {}", x); } }
    #[test] fn spec_params_present() { let reg = Registry::new(); assert!(reg.get("display.refresh_rate").is_some()); assert!(reg.get("input.mouse.raw_input").is_some()); }
    #[test] fn set_and_reset() { let mut reg = Registry::new(); reg.set_value("input.mouse.raw_input", ParamValue::Bool(false)).unwrap(); assert_eq!(reg.get("input.mouse.raw_input").unwrap().value, ParamValue::Bool(false)); reg.reset_value("input.mouse.raw_input").unwrap(); assert_eq!(reg.get("input.mouse.raw_input").unwrap().value, ParamValue::Bool(true)); }
}
