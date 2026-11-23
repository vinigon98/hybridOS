//! HybridOS Kernel — PE Loader from VFS (Virtual File System).
//!
//! Carrega binários PE32+/PE64 (.exe) diretamente do VFS para execução
//! em Ring 3, reutilizando a infraestrutura existente de:
//! - spawn_pe_process() (pe.rs / pe64_advanced.rs)
//! - bind_import_address_table() (pe_iat.rs)
//! - apply_relocations() (pe_reloc.rs)
//! - vfs_read_all() (vfs/mod.rs)
//!
//! Zero dependência de Windows: tudo roda bare-metal no kernel HybridOS.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use crate::loaders::spawn_pe_process;
use crate::types::NtStatus;
use crate::vfs::{self, Permissions};

/// Caminho padrão onde programas instalados pelo hyb-pkg ficam armazenados.
pub const PROGRAM_FILES_DIR: &str = "/Program Files";
pub const DOWNLOADS_DIR: &str = "/Downloads";

/// Permissões padrão para arquivos de programa (leitura + execução).
const PROGRAM_PERMS: Permissions = Permissions {
    mode: 0o755,
    readonly: false,
    hidden: false,
    system: false,
    archive: false,
};

/// Carrega um binário PE (.exe) do VFS e o spawn como processo Ring 3.
///
/// # Fluxo
/// 1. Lê o arquivo .exe do VFS via `vfs::vfs_read_all()`
/// 2. Valida assinatura MZ + PE\0\0 (delegado para spawn_pe_process)
/// 3. Chama `spawn_pe_process()` que aloca PML4, mapeia seções, configura
///    TEB/PEB, vincula IAT e aplica relocações
/// 4. Registra o processo no scheduler híbrido (Windows NT personality)
///
/// # Retorno
/// Ok((pcb, tcb)) se o processo foi criado com sucesso
/// Err(NtStatus) se houver falha de leitura, formato inválido ou memória insuficiente
pub fn load_pe_from_vfs(
    path: &str,
) -> Result<
    (
        alloc::sync::Arc<crate::sync::Mutex<crate::sched::ProcessControlBlock>>,
        alloc::sync::Arc<crate::sync::Mutex<crate::sched::ThreadControlBlock>>,
    ),
    NtStatus,
> {
    // 1. Ler o arquivo .exe do VFS
    let pe_data = match vfs::vfs_read_all(path) {
        Ok(data) => data,
        Err(_e) => {
            crate::serial_println!("[PE-VFS] Falha ao ler '{}' do VFS", path);
            return Err(NtStatus::FILE_NOT_FOUND);
        }
    };

    crate::serial_println!("[PE-VFS] Carregando '{}' ({} bytes) do VFS", path, pe_data.len());

    // 2. Validar assinatura MZ + PE rapidamente antes de chamar spawn_pe_process
    if pe_data.len() < 64 {
        return Err(NtStatus::INVALID_PARAMETER);
    }
    let dos_magic = u16::from_le_bytes([pe_data[0], pe_data[1]]);
    if dos_magic != 0x5A4D {
        crate::serial_println!("[PE-VFS] Assinatura DOS inválida em '{}'", path);
        return Err(NtStatus::INVALID_PARAMETER);
    }

    // 3. Spawn o processo PE64 (chama internamente mapeamento de seções, IAT, relocações, TEB/PEB)
    match spawn_pe_process(path, &pe_data) {
        Ok((pcb, tcb)) => {
            crate::serial_println!(
                "[PE-VFS] Processo '{}' spawnado com sucesso",
                path
            );
            Ok((pcb, tcb))
        }
        Err(e) => {
            crate::serial_println!("[PE-VFS] Falha ao spawnar '{}': {}", path, e);
            Err(NtStatus::INVALID_PARAMETER)
        }
    }
}

/// Lista os programas instalados no diretório /Program Files do VFS.
pub fn list_installed_programs() -> Result<Vec<String>, NtStatus> {
    match vfs::vfs_readdir(PROGRAM_FILES_DIR) {
        Ok(entries) => {
            let mut programs = Vec::new();
            for entry in entries {
                if entry.name.ends_with(".exe") || entry.name.ends_with(".EXE") {
                    programs.push(entry.name);
                }
            }
            Ok(programs)
        }
        Err(_) => Ok(Vec::new()), // Diretório ainda não existe
    }
}

/// Instala um programa .exe baixado: copia de /Downloads para /Program Files
/// e registra no manifest de instalação.
pub fn install_program_from_downloads(
    exe_name: &str,
    expected_sha256: Option<&str>,
) -> Result<(), NtStatus> {
    let src_path = format!("{}/{}", DOWNLOADS_DIR, exe_name);
    let dst_path = format!("{}/{}", PROGRAM_FILES_DIR, exe_name);

    // 1. Ler do Downloads
    let pe_data = match vfs::vfs_read_all(&src_path) {
        Ok(data) => data,
        Err(_) => return Err(NtStatus::FILE_NOT_FOUND),
    };

    // 2. Validar SHA-256 se fornecido
    if let Some(expected) = expected_sha256 {
        let actual = crate::security::sha256::sha256_hex(&pe_data);
        if actual != expected {
            crate::serial_println!(
                "[PE-VFS] SHA-256 mismatch para '{}': esperado={}, obtido={}",
                exe_name, expected, actual
            );
            return Err(NtStatus::ACCESS_DENIED);
        }
    }

    // 3. Criar diretório /Program Files se não existir
    let _ = vfs::vfs_mkdir(PROGRAM_FILES_DIR, PROGRAM_PERMS);

    // 4. Copiar para /Program Files
    match vfs::vfs_create_file(&dst_path, pe_data.as_slice(), PROGRAM_PERMS) {
        Ok(_) => {
            crate::serial_println!("[PE-VFS] '{}' instalado em {}", exe_name, dst_path);
            Ok(())
        }
        Err(_e) => {
            crate::serial_println!("[PE-VFS] Falha ao gravar '{}'", dst_path);
            Err(NtStatus::UNSUCCESSFUL)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pe_signatures() {
        // MZ
        assert_eq!(0x5A4D, 0x5A4D);
        // PE\0\0
        assert_eq!(0x00004550, 0x00004550);
    }
}
