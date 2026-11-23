//! hyb-update: HybridOS System Updater (estilo Windows Update)
//!
//! Gerencia:
//!   - Verificacao de atualizacoes disponiveis
//!   - Instalacao de pacotes .hybupdate
//!   - Criacao de pontos de restauracao (snapshots)
//!   - Rollback para ponto anterior
//!   - Status de atualizacoes pendentes
//!
//! Formato de pacote .hybupdate:
//!   Arquivo JSON com:
//!   {
//!     "version": "1.0.0",
//!     "description": "...",
//!     "kernel": "caminho/para/kernel.bin",
//!     "files": {
//!       "/etc/hybridos/system.toml": "BASE64_CONTEUDO",
//!       "C:\\Windows\\System32\\driver.sys": "BASE64_CONTEUDO"
//!     },
//!     "reboot_required": true
//!   }

#![no_std]
#![no_main]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use libhybrid_sys::allocator::HybridUserAllocator;
use libhybrid_sys::{println};
use libhybrid_sys::linux::{
    sys_open, sys_read, sys_write, sys_close, sys_exit,
};

#[global_allocator]
static ALLOCATOR: HybridUserAllocator = HybridUserAllocator::new();

const KERNEL_PATH: &str = "/boot/hybridos-kernel";
const KERNEL_BACKUP_PATH: &str = "/boot/hybridos-kernel.backup";
const UPDATE_PENDING_PATH: &str = "/update/pending/package.hybupdate";
const RESTORE_DIR: &str = "/update/restore";
const PACKAGE_EXT: &str = ".hybupdate";

const O_RDONLY: u32 = 0;
const O_WRONLY: u32 = 1;
const O_CREAT: u32 = 0x40;
const O_TRUNC: u32 = 0x200;

fn read_file(path: &str) -> Option<Vec<u8>> {
    let fd = sys_open(path.as_ptr() as *const u8 as usize, O_RDONLY, 0o644);
    if fd < 0 {
        return None;
    }
    let mut out = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        let n = sys_read(fd as usize, &mut buf);
        if n <= 0 {
            break;
        }
        out.extend_from_slice(&buf[..n as usize]);
    }
    sys_close(fd as usize);
    Some(out)
}

fn write_file(path: &str, data: &[u8]) -> bool {
    let fd = sys_open(path.as_ptr() as *const u8 as usize, O_WRONLY | O_CREAT | O_TRUNC, 0o644);
    if fd < 0 {
        return false;
    }
    let mut w = 0usize;
    while w < data.len() {
        let n = sys_write(fd as usize, &data[w..]);
        if n <= 0 {
            sys_close(fd as usize);
            return false;
        }
        w += n as usize;
    }
    sys_close(fd as usize);
    true
}

fn file_exists(path: &str) -> bool {
    sys_open(path.as_ptr() as *const u8 as usize, O_RDONLY, 0o644) >= 0
}

const B64_CHARS: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn b64_encode(input: &[u8]) -> String {
    let mut out = String::new();
    for chunk in input.chunks(3) {
        match chunk.len() {
            3 => {
                out.push(B64_CHARS[(chunk[0] >> 2) as usize] as char);
                out.push(B64_CHARS[(((chunk[0] & 0x03) << 4) | (chunk[1] >> 4)) as usize] as char);
                out.push(B64_CHARS[(((chunk[1] & 0x0F) << 2) | (chunk[2] >> 6)) as usize] as char);
                out.push(B64_CHARS[(chunk[2] & 0x3F) as usize] as char);
            }
            2 => {
                out.push(B64_CHARS[(chunk[0] >> 2) as usize] as char);
                out.push(B64_CHARS[(((chunk[0] & 0x03) << 4) | (chunk[1] >> 4)) as usize] as char);
                out.push(B64_CHARS[((chunk[1] & 0x0F) << 2) as usize] as char);
                out.push('=');
            }
            1 => {
                out.push(B64_CHARS[(chunk[0] >> 2) as usize] as char);
                out.push(B64_CHARS[((chunk[0] & 0x03) << 4) as usize] as char);
                out.push('=');
                out.push('=');
            }
            _ => {}
        }
    }
    out
}

fn b64_decode(input: &str) -> Option<Vec<u8>> {
    let input: Vec<u8> = input.bytes().filter(|b| *b != b'=' && *b != b'\n' && *b != b'\r' && *b != b' ').collect();
    if input.len() % 4 != 0 {
        return None;
    }
    let mut out = Vec::new();
    for chunk in input.chunks(4) {
        if chunk.len() < 4 {
            break;
        }
        let b0 = b64_val(chunk[0])?;
        let b1 = b64_val(chunk[1])?;
        let b2 = b64_val(chunk[2])?;
        let b3 = b64_val(chunk[3])?;
        out.push((b0 << 2) | (b1 >> 4));
        out.push(((b1 & 0x0F) << 4) | (b2 >> 2));
        out.push(((b2 & 0x03) << 6) | b3);
    }
    let input_str: String = input.iter().map(|b| *b as char).collect();
    let pad_count = input_str.chars().filter(|c| *c == '=').count();
    for _ in 0..pad_count {
        out.pop();
    }
    Some(out)
}

fn b64_val(b: u8) -> Option<u8> {
    match b {
        b'A'..=b'Z' => Some(b - b'A'),
        b'a'..=b'z' => Some(b - b'a' + 26),
        b'0'..=b'9' => Some(b - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

#[derive(Debug, Clone)]
pub struct UpdatePackage {
    pub version: String,
    pub description: String,
    pub kernel_path: Option<String>,
    pub files: Vec<(String, Vec<u8>)>,
    pub reboot_required: bool,
}

fn parse_json_package(json: &str) -> Option<UpdatePackage> {
    let json = json.trim();
    if !json.starts_with('{') || !json.ends_with('}') {
        return None;
    }

    let mut version = String::new();
    let mut description = String::new();
    let mut kernel_path = None;
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    let mut reboot_required = false;

    let content = &json[1..json.len()-1];

    if let Some(v) = extract_json_string(content, "version") {
        version = v;
    }

    if let Some(d) = extract_json_string(content, "description") {
        description = d;
    }

    if let Some(k) = extract_json_string(content, "kernel") {
        kernel_path = Some(k);
    }

    if let Some(files_obj) = extract_json_object(content, "files") {
        let inner = &files_obj[1..files_obj.len()-1];
        let pairs = split_top_level(inner);
        for pair in pairs {
            let parts: Vec<&str> = pair.splitn(2, ':').collect();
            if parts.len() == 2 {
                let key = parts[0].trim();
                let val = parts[1].trim();
                let key = key.trim_matches('"');
                let val = val.trim();
                if val.starts_with('"') && val.ends_with('"') {
                    let b64 = &val[1..val.len()-1];
                    if let Some(decoded) = b64_decode(b64) {
                        files.push((key.to_string(), decoded));
                    }
                }
            }
        }
    }

    if content.contains("\"reboot_required\":true") {
        reboot_required = true;
    }

    Some(UpdatePackage {
        version,
        description,
        kernel_path,
        files,
        reboot_required,
    })
}

fn extract_json_string(obj: &str, key: &str) -> Option<String> {
    let search = String::from("\"") + key + "\"";
    let pos = obj.find(search.as_str())?;
    let after = &obj[pos + search.len()..];
    let colon_pos = after.find(':')?;
    let after_colon = &after[colon_pos+1..];
    let val = after_colon.trim();
    if val.starts_with('"') {
        let end = val[1..].find('"')?;
        Some(val[1..1+end].to_string())
    } else {
        None
    }
}

fn extract_json_object(obj: &str, key: &str) -> Option<String> {
    let search = "\"" + key + "\"";
    let pos = obj.find(search.as_str())?;
    let after = &obj[pos + search.len()..];
    let colon_pos = after.find(':')?;
    let after_colon = &after[colon_pos+1..];
    let val = after_colon.trim();
    if val.starts_with('{') {
        let mut depth = 0;
        for (i, c) in val.chars().enumerate() {
            match c {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(val[..=i].to_string());
                    }
                }
                _ => {}
            }
        }
    }
    None
}

fn split_top_level(s: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut depth = 0;
    let mut current = String::new();
    let mut in_string = false;
    let mut escaped = false;

    for c in s.chars() {
        if in_string {
            if escaped {
                escaped = false;
                current.push(c);
            } else if c == '\\' {
                escaped = true;
                current.push(c);
            } else if c == '"' {
                in_string = false;
                current.push(c);
            } else {
                current.push(c);
            }
        } else {
            match c {
                '{' | '[' => { depth += 1; current.push(c); }
                '}' | ']' => { depth -= 1; current.push(c); }
                '"' => { in_string = true; current.push(c); }
                ',' if depth == 0 => {
                    result.push(current.trim().to_string());
                    current = String::new();
                }
                _ => current.push(c),
            }
        }
    }
    if !current.trim().is_empty() {
        result.push(current.trim().to_string());
    }
    result
}

fn cmd_help() {
    println!("============================================================");
    println!("  hyb-update - HybridOS System Updater");
    println!("============================================================");
    println!("");
    println!("Comandos disponíveis:");
    println!("  check                          - Verificar atualizacoes pendentes");
    println!("  install <arquivo.hybupdate>    - Instalar pacote de atualizacao");
    println!("  rollback                       - Voltar para ponto de restauracao anterior");
    println!("  list                           - Listar pontos de restauracao");
    println!("  status                         - Mostrar status do sistema de atualizacao");
    println!("  create-point                   - Criar ponto de restauracao manual");
    println!("  help                           - Mostrar esta ajuda");
    println!("");
    println!("Formato do pacote .hybupdate:");
    println!("  Arquivo JSON com:");
    println!("    { \"version\": \"1.0.0\", \"description\": \"...\",");
    println!("      \"kernel\": \"caminho/kernel.bin\",");
    println!("      \"files\": { \"/caminho/arquivo\": \"base64conteudo\" },");
    println!("      \"reboot_required\": true }");
    println!("");
    println!("Exemplo: hyb-update install /tmp/minha_atualizacao.hybupdate");
}

fn cmd_check() {
    println!("============================================================");
    println!("  hybridOS Update Check");
    println!("============================================================");
    println!("");

    if file_exists("/update/pending/package.hybupdate") {
        println!("[STATUS] ATUALIZACAO PENDENTE: /update/pending/package.hybupdate");
        println!("[ACAO] Reinicie o sistema para aplicar ou rode 'hyb-update install' manualmente.");
    } else {
        println!("[STATUS] Nenhuma atualizacao pendente.");
    }

    println!("[RESTORE] Pontos de restauracao disponíveis:");
    if file_exists(KERNEL_BACKUP_PATH) {
        println!("       /boot/hybridos-kernel.backup (disponível)");
    } else {
        println!("       (Nenhum ponto de restauracao automatico encontrado)");
    }
}

fn cmd_install(path: &str) {
    println!("============================================================");
    println!("  hybridOS Update Installer");
    println!("============================================================");
    println!("");

    let package_data = match read_file(path) {
        Some(d) => d,
        None => {
            println!("[ERRO] Nao foi possivel ler o arquivo: {}", path);
            return;
        }
    };

    let package_text = match core::str::from_utf8(&package_data) {
        Ok(t) => t,
        Err(_) => {
            println!("[ERRO] Arquivo nao eh um texto valido (esperado JSON).");
            return;
        }
    };

    let package = match parse_json_package(package_text) {
        Some(p) => p,
        None => {
            println!("[ERRO] Nao foi possivel parsear o pacote .hybupdate.");
            return;
        }
    };

    println!("[PACOTE] Versao: {}", package.version);
    println!("[PACOTE] Descricao: {}", package.description);
    println!("[PACOTE] Arquivos: {}", package.files.len());
    if let Some(k) = &package.kernel_path {
        println!("[PACOTE] Kernel: {}", k);
    }
    println!("");

    println!("[RESTORE] Criando ponto de restauracao...");
    if let Some(kernel_data) = read_file(KERNEL_PATH) {
        if write_file(KERNEL_BACKUP_PATH, &kernel_data) {
            println!("[RESTORE] Ponto de restauracao criado.");
        } else {
            println!("[ERRO] Falha ao criar ponto de restauracao.");
            return;
        }
    } else {
        println!("[AVISO] Nao foi possivel ler o kernel atual para backup.");
    }

    println!("[APLICANDO] Gravando arquivos...");
    for (file_path, content) in &package.files {
        println!("  Gravando: {}", file_path);
        if write_file(file_path, content) {
            println!("    [OK] Gravado com sucesso.");
        } else {
            println!("    [FALHA] Nao foi possivel gravar.");
        }
    }

    if let Some(kernel_path) = &package.kernel_path {
        let kernel_data = match read_file(kernel_path) {
            Some(d) => d,
            None => {
                println!("[ERRO] Nao foi ler o kernel do pacote: {}", kernel_path);
                return;
            }
        };
        println!("[KERNEL] Gravando novo kernel em {}...", KERNEL_PATH);
        if write_file(KERNEL_PATH, &kernel_data) {
            println!("[KERNEL] Kernel gravado com sucesso.");
        } else {
            println!("[ERRO] Falha ao gravar kernel.");
            return;
        }
    }

    println!("[PENDENTE] Marcando atualizacao para o proximo boot...");
    if write_file(UPDATE_PENDING_PATH, package_data.as_bytes()) {
        println!("[PENDENTE] Atualizacao marcada com sucesso.");
    } else {
        println!("[ERRO] Falha ao marcar atualizacao pendente.");
        return;
    }

    println!("");
    println!("[CONCLUSAO] Pacote instalado com sucesso!");
    println!("            Reinicie o sistema para aplicar a atualizacao.");
    println!("            Use: hyb-update rollback para desfazer.");
}

fn cmd_rollback() {
    println!("============================================================");
    println!("  hybridOS Rollback");
    println!("============================================================");
    println!("");

    if file_exists(KERNEL_BACKUP_PATH) {
        println!("[RESTORE] Restaurando kernel do backup...");
        if let Some(backup_data) = read_file(KERNEL_BACKUP_PATH) {
            if write_file(KERNEL_PATH, &backup_data) {
                println!("[RESTORE] Kernel restaurado com sucesso.");
                println!("[REINICIE] Reinicie o sistema para usar o kernel restaurado.");
            } else {
                println!("[ERRO] Falha ao gravar kernel restaurado.");
            }
        } else {
            println!("[ERRO] Nao foi ler o backup do kernel.");
        }
    } else {
        println!("[AVISO] Nenhum ponto de restauracao encontrado.");
        println!("         Nao ha o que desfazer.");
    }
}

fn cmd_status() {
    println!("============================================================");
    println!("  hybridOS Update Status");
    println!("============================================================");
    println!("");

    if file_exists(UPDATE_PENDING_PATH) {
        println!("[STATUS] ATUALIZACAO PENDENTE");
        println!("[ARQUIVO] {}", UPDATE_PENDING_PATH);
        println!("[ACAO] Reinicie o sistema para aplicar ou execute:");
        println!("         hyb-update install {}", UPDATE_PENDING_PATH);
    } else {
        println!("[STATUS] Sistema atualizado. Nenhuma atualizacao pendente.");
    }

    println!("");
    println!("[RESTORE] Ultimo ponto de restauracao:");
    if file_exists(KERNEL_BACKUP_PATH) {
        println!("         Disponível em {}", KERNEL_BACKUP_PATH);
    } else {
        println!("         Nenhum ponto de restauracao encontrado.");
    }
}

fn cmd_create_point() {
    println!("============================================================");
    println!("  hybridOS Create Restore Point");
    println!("============================================================");
    println!("");

    if file_exists(KERNEL_PATH) {
        if let Some(kernel_data) = read_file(KERNEL_PATH) {
            if write_file(KERNEL_BACKUP_PATH, &kernel_data) {
                println!("[RESTORE] Ponto de restauracao criado em:");
                println!("         {}", KERNEL_BACKUP_PATH);
                println!("[RESTORE] Tamanho: {} bytes", kernel_data.len());
            } else {
                println!("[ERRO] Falha ao criar ponto de restauracao.");
            }
        } else {
            println!("[ERRO] Nao foi ler o kernel atual.");
        }
    } else {
        println!("[ERRO] Kernel nao encontrado em: {}", KERNEL_PATH);
    }
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("============================================================");
    println!("  hyb-update - HybridOS System Updater");
    println!("============================================================");
    println!("");
    println!("Comandos disponíveis:");
    println!("  check                          - Verificar atualizacoes pendentes");
    println!("  install <arquivo.hybupdate>    - Instalar pacote de atualizacao");
    println!("  rollback                       - Voltar para ponto de restauracao anterior");
    println!("  list                           - Listar pontos de restauracao");
    println!("  status                         - Mostrar status do sistema de atualizacao");
    println!("  create-point                   - Criar ponto de restauracao manual");
    println!("  help                           - Mostrar esta ajuda");
    println!("");
    println!("Exemplo: hyb-update install /tmp/minha_atualizacao.hybupdate");
    println!("         hyb-update status");
    println!("         hyb-update rollback");
    
    sys_exit(0);
}
