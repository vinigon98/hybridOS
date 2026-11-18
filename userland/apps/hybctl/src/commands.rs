//! commands.rs - Implementacao dos comandos do hybctl: get, set, list, reset, export, import.

use crate::parser::{parse_toml, serialize_toml, TomlDoc};
use crate::registry::{Param, ParamValue, Registry};
use std::fs;
use std::path::PathBuf;

pub struct App { pub config_path: PathBuf, pub registry: Registry }

impl App {
    pub fn new(config_path: PathBuf) -> Result<Self, String> {
        let mut registry = Registry::new();
        let path = PathBuf::from(&config_path);
        if path.exists() {
            let text = fs::read_to_string(&path)
                .map_err(|e| format!("nao foi possivel ler {}: {}", path.display(), e))?;
            let doc = parse_toml(&text)?;
            for (section, map) in &doc {
                for (key, tv) in map {
                    let p = format!("{}.{}", section, key);
                    let v = ParamValue::from_toml_value(tv)?;
                    registry.apply_override(&p, v)?;
                }
            }
        }
        Ok(App { config_path: path, registry })
    }
    pub fn save(&self) -> Result<(), String> {
        if let Some(parent) = self.config_path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("nao foi possivel criar diretorio {}: {}", parent.display(), e))?;
            }
        }
        let doc = registry_to_doc(self.registry.all());
        fs::write(&self.config_path, serialize_toml(&doc))
            .map_err(|e| format!("nao foi possivel escrever {}: {}", self.config_path.display(), e))?;
        Ok(())
    }
    pub fn cmd_get(&self, path: &str) -> Result<(), String> {
        let param = self.registry.get(path).ok_or_else(|| format!("parametro nao encontrado: {}", path))?;
        println!("{} = {}", param.path, param.value.to_string());
        Ok(())
    }
    pub fn cmd_set(&mut self, path: &str, value: &str) -> Result<(), String> {
        let param = self.registry.get(path).ok_or_else(|| format!("parametro nao encontrado: {}", path))?;
        if !param.mutable { return Err(format!("parametro somente leitura: {}", path)); }
        let parsed = ParamValue::parse(&param.value_type, value)?;
        self.registry.set_value(path, parsed)?;
        self.save()?;
        println!("definido {} = {} em {}", path, value, self.config_path.display());
        Ok(())
    }
    pub fn cmd_list(&self, module: Option<&str>) -> Result<(), String> {
        match module {
            Some(m) => {
                if !self.registry.has_module(m) { return Err(format!("modulo nao encontrado: {}", m)); }
                let params = self.registry.by_module(m);
                println!("=== Modulo: {} ===", m);
                for p in &params { println!("  {} = {}  [tipo: {}]  {}", p.path, p.value.to_string(), p.value_type, p.description); }
                println!("Total: {} parametros no modulo {}", params.len(), m);
            }
            None => {
                let modules = self.registry.modules();
                let total = self.registry.all().len();
                for m in &modules {
                    let params = self.registry.by_module(m);
                    println!("[{}] {} parametros", m, params.len());
                    for p in &params { println!("    {} = {}  [tipo: {}]", p.path, p.value.to_string(), p.value_type); }
                }
                println!("Total geral: {} parametros em {} modulos", total, modules.len());
            }
        }
        Ok(())
    }
    pub fn cmd_reset(&mut self, path: &str) -> Result<(), String> {
        self.registry.reset_value(path)
            .map_err(|e| format!("nao foi possivel resetar {}: {}", path, e))?;
        self.save()?;
        println!("resetado {} para o valor padrao de fabrica", path);
        Ok(())
    }
    pub fn cmd_export(&self) -> Result<(), String> {
        print!("{}", serialize_toml(&registry_to_doc(self.registry.all())));
        Ok(())
    }
    pub fn cmd_import(&mut self, file: &str) -> Result<(), String> {
        let text = fs::read_to_string(file).map_err(|e| format!("nao foi possivel ler {}: {}", file, e))?;
        let doc = parse_toml(&text)?;
        let mut count = 0usize;
        for (section, map) in &doc {
            for (key, tv) in map {
                let p = format!("{}.{}", section, key);
                let v = ParamValue::from_toml_value(tv)?;
                if self.registry.apply_override(&p, v).is_ok() { count += 1; }
            }
        }
        self.save()?;
        println!("importado {} parametros de {} para {}", count, file, self.config_path.display());
        Ok(())
    }
}

fn registry_to_doc(params: &[Param]) -> TomlDoc {
    let mut doc: TomlDoc = TomlDoc::new();
    for p in params {
        let map = doc.entry(p.section.to_string()).or_default();
        map.insert(p.key.clone(), p.value.to_toml_value());
    }
    doc
}
pub fn default_config_path() -> PathBuf {
    if let Ok(v) = std::env::var("HYBCTL_CONFIG") { if !v.is_empty() { return PathBuf::from(v); } }
    PathBuf::from("etc/hybridos/system.toml")
}
