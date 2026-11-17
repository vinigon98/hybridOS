//! main.rs - Entry point of hybctl (HybridOS central configuration controller).
//! Builds with the host toolchain (std) and reads/writes etc/hybridos/system.toml.

mod commands;
mod parser;
mod registry;

use commands::{default_config_path, App};
use std::env;
use std::process;
use std::path::PathBuf;

fn usage() {
    println!("hybctl - Controlador Central de Configuracao do Hybridos OS");
    println!();
    println!("  hybctl [--config <file>] get <path>");
    println!("  hybctl [--config <file>] set <path> <value>");
    println!("  hybctl [--config <file>] list [module]");
    println!("  hybctl [--config <file>] reset <path>");
    println!("  hybctl [--config <file>] export");
    println!("  hybctl [--config <file>] import <file>");
    println!();
    println!("Examples:");
    println!("  hybctl get display.refresh_rate");
    println!("  hybctl set input.mouse.raw_input true");
    println!("  hybctl list input");
    println!("  hybctl reset input.mouse.raw_input");
    println!("  hybctl export > backup.toml");
    println!("  hybctl import backup.toml");
}

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().collect();
    let mut config_path: Option<PathBuf> = None;
    let mut positional: Vec<String> = Vec::new();
    let mut i = 1usize;
    while i < args.len() {
        let a = &args[i];
        if a == "--config" {
            i += 1;
            if i >= args.len() {
                return Err("--config requer um caminho de arquivo".to_string());
            }
            config_path = Some(PathBuf::from(args[i].clone()));
        } else if a == "--help" || a == "-h" {
            usage();
            process::exit(0);
        } else {
            positional.push(a.clone());
        }
        i += 1;
    }
    if positional.is_empty() {
        usage();
        process::exit(0);
    }
    let command = positional[0].as_str();
    let rest = &positional[1..];
    let path = config_path.unwrap_or_else(default_config_path);
    let mut app = App::new(path.clone())?;
    match command {
        "get" => {
            if rest.len() != 1 {
                return Err("uso: hybctl get <caminho>".to_string());
            }
            app.cmd_get(rest[0].as_str())
        }
        "set" => {
            if rest.len() != 2 {
                return Err("uso: hybctl set <caminho> <valor>".to_string());
            }
            app.cmd_set(rest[0].as_str(), rest[1].as_str())
        }
        "list" => {
            if rest.len() > 1 {
                return Err("uso: hybctl list [modulo]".to_string());
            }
            app.cmd_list(rest.first().map(|s| s.as_str()))
        }
        "reset" => {
            if rest.len() != 1 {
                return Err("uso: hybctl reset <caminho>".to_string());
            }
            app.cmd_reset(rest[0].as_str())
        }
        "export" => {
            if !rest.is_empty() {
                return Err("uso: hybctl export".to_string());
            }
            app.cmd_export()
        }
        "import" => {
            if rest.len() != 1 {
                return Err("uso: hybctl import <arquivo>".to_string());
            }
            app.cmd_import(rest[0].as_str())
        }
        other => {
            println!("comando desconhecido: '{}'", other);
            usage();
            process::exit(1);
        }
    }?;
    Ok(())
}

fn main() {
    match run() {
        Ok(()) => process::exit(0),
        Err(e) => {
            eprintln!("hybctl: erro: {}", e);
            process::exit(1);
        }
    }
}
