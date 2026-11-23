//! parser.rs - Leitura e escrita minina de TOML (sem dependencias externas).
//! Suporta secoes [secao.subsecao], chaves pontoadas e valores bool/int/float/string/array.

use crate::registry::ParamValue;
use std::collections::BTreeMap;

/// Um valor TOML simples.
#[derive(Clone, Debug, PartialEq)]
pub enum TomlValue {
    Str(String),
    Bool(bool),
    Int(i64),
    Float(f64),
    Array(Vec<TomlValue>),
}

/// Documento TOML: secao -> (chave -> valor).
pub type TomlDoc = BTreeMap<String, BTreeMap<String, TomlValue>>;

/// Remove um comentario TOML (tudo a partir de '#' que nao esteja dentro de aspas).
fn strip_comment(line: &str) -> &str {
    let mut in_quotes = false;
    let mut cut = line.len();
    for (i, c) in line.char_indices() {
        if c == '"' {
            in_quotes = !in_quotes;
        } else if c == '#' && !in_quotes {
            cut = i;
            break;
        }
    }
    &line[..cut]
}

/// Analisa um valor TOML simples.
fn parse_toml_value(raw: &str) -> Result<TomlValue, String> {
    let s = raw.trim();
    if s.len() >= 2 && s.starts_with('"') && s.ends_with('"') {
        return Ok(TomlValue::Str(s[1..s.len() - 1].to_string()));
    }
    match s {
        "true" => Ok(TomlValue::Bool(true)),
        "false" => Ok(TomlValue::Bool(false)),
        _ => {
            if let Ok(n) = s.parse::<i64>() {
                return Ok(TomlValue::Int(n));
            }
            if let Ok(f) = s.parse::<f64>() {
                return Ok(TomlValue::Float(f));
            }
            Ok(TomlValue::Str(s.to_string()))
        }
    }
}

/// Analisa um texto TOML simplificado.
pub fn parse_toml(text: &str) -> Result<TomlDoc, String> {
    let mut doc: TomlDoc = BTreeMap::new();
    let mut current_section: Option<String> = None;
    for raw_line in text.lines() {
        let line = strip_comment(raw_line);
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            let section = line[1..line.len() - 1].trim();
            if section.is_empty() {
                return Err("secao TOML vazia".to_string());
            }
            current_section = Some(section.to_string());
            doc.entry(section.to_string()).or_default();
            continue;
        }
        if let Some(eq) = line.find('=') {
            let key = line[..eq].trim();
            let value = line[eq + 1..].trim();
            if key.is_empty() {
                continue;
            }
            let section =
                current_section.clone().unwrap_or_else(|| "__root__".to_string());
            let tv = parse_toml_value(value)?;
            doc.entry(section).or_default().insert(key.to_string(), tv);
            continue;
        }
        return Err(format!("linha TOML invalida: {}", line));
    }
    Ok(doc)
}

/// Converte um ParamValue em TomlValue.
pub fn param_to_toml_value(pv: &ParamValue) -> TomlValue {
    match pv {
        ParamValue::Bool(b) => TomlValue::Bool(*b),
        ParamValue::Integer(i) => TomlValue::Int(*i),
        ParamValue::Float(f) => TomlValue::Float(*f),
        ParamValue::String(s) => TomlValue::Str(s.clone()),
        ParamValue::Array(items) => {
            TomlValue::Array(items.iter().map(param_to_toml_value).collect())
        }
    }
}

/// Converte um TomlValue em ParamValue.
pub fn toml_value_to_param(tv: &TomlValue) -> Result<ParamValue, String> {
    match tv {
        TomlValue::Bool(b) => Ok(ParamValue::Bool(*b)),
        TomlValue::Int(i) => Ok(ParamValue::Integer(*i)),
        TomlValue::Float(f) => Ok(ParamValue::Float(*f)),
        TomlValue::Str(s) => Ok(ParamValue::String(s.clone())),
        TomlValue::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for it in items {
                out.push(toml_value_to_param(it)?);
            }
            Ok(ParamValue::Array(out))
        }
    }
}

/// Serializa um documento TOML para string.
pub fn serialize_toml(doc: &TomlDoc) -> String {
    let mut out = String::new();
    for (section, map) in doc {
        out.push_str(&format!("[{}]\n", section));
        let mut keys: Vec<&String> = map.keys().collect();
        keys.sort();
        for key in keys {
            out.push_str(&format!("{} = {}\n", key, toml_value_to_string(&map[key])));
        }
        out.push('\n');
    }
    out
}

fn toml_value_to_string(v: &TomlValue) -> String {
    match v {
        TomlValue::Str(s) => format!("\"{}\"", s),
        TomlValue::Bool(b) => b.to_string(),
        TomlValue::Int(n) => n.to_string(),
        TomlValue::Float(f) => f.to_string(),
        TomlValue::Array(items) => {
            let parts: Vec<String> = items.iter().map(toml_value_to_string).collect();
            format!("[{}]", parts.join(", "))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_basic() {
        let text = r#"[display]
refresh_rate = 144
scaling = "100%"
night_light = false

[input.mouse]
raw_input = true
polling_rate_max_hz = 4000
"#;
        let doc = parse_toml(text).unwrap();
        assert_eq!(
            doc.get("display").unwrap().get("refresh_rate").unwrap(),
            &TomlValue::Int(144)
        );
        assert_eq!(
            doc.get("input.mouse").unwrap().get("raw_input").unwrap(),
            &TomlValue::Bool(true)
        );
        let out = serialize_toml(&doc);
        assert!(out.contains("refresh_rate = 144"));
        assert!(out.contains("raw_input = true"));
    }
}
