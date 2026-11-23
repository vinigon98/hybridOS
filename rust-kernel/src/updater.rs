//! updater.rs — Módulo de Atualização do Sistema (Estilo Windows Update)
//!
//! Responsável por:
//!   - Detectar atualizações pendentes no disco
//!   - Criar pontos de restauração (snapshots) antes de atualizar
//!   - Aplicar pacotes de atualização durante o boot
//!   - Fazer rollback automático se o novo kernel falhar
//!   - Manter dados do usuário intocados

#![no_std]

use crate::sync::SpinLock;
use core::sync::atomic::{AtomicBool, Ordering};

// Estrutura de estado da atualização
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum UpdateState {
    None = 0,           // Nenhuma atualização pendente
    Pending = 1,        // Atualização baixada, aguardando reboot
    Applying = 2,       // Aplicando atualização (em progresso)
    Applied = 3,        // Aplicada com sucesso
    Failed = 4,         // Falha na aplicação
    Rollback = 5,       // Rollback em progresso
    RolledBack = 6,     // Rollback concluído
}

/// Estado global do sistema de atualização
pub static UPDATE_STATE: SpinLock<UpdateState> = SpinLock::new(UpdateState::None);
pub static UPDATE_PENDING: AtomicBool = AtomicBool::new(false);
pub static RESTORE_COUNT: AtomicBool = AtomicBool::new(false);

/// Caminhos importantes
pub const KERNEL_PATH: &str = "/boot/hybridos-kernel";
pub const KERNEL_BACKUP_PATH: &str = "/boot/hybridos-kernel.backup";
pub const UPDATE_PENDING_PATH: &str = "/update/pending";
pub const RESTORE_DIR: &str = "/update/restore";
pub const VFS_USER_DIR: &str = "/home";  // Dados do usuário (nunca tocar)

/// Verifica se há uma atualização pendente
pub fn is_update_pending() -> bool {
    UPDATE_PENDING.load(Ordering::Relaxed)
}

/// Retorna o estado atual da atualização
pub fn get_update_state() -> UpdateState {
    *UPDATE_STATE.lock()
}

/// Define o estado da atualização
pub fn set_update_state(state: UpdateState) {
    *UPDATE_STATE.lock() = state;
    match state {
        UpdateState::Pending => UPDATE_PENDING.store(true, Ordering::Relaxed),
        UpdateState::Applied | UpdateState::RolledBack | UpdateState::None => {
            UPDATE_PENDING.store(false, Ordering::Relaxed);
        }
        _ => {}
    }
}

/// Cria um ponto de restauração (snapshot do kernel atual)
/// Retorna true se conseguiu criar com sucesso
pub fn create_restore_point() -> bool {
    // Em implementação real, isso copiaria o kernel atual para /update/restore/
    // e salvaria o estado do VFS de usuários
    set_update_state(UpdateState::Applied); // Marca que o ponto foi criado
    true
}

/// Aplica uma atualização pendente
/// Retorna true se a aplicação foi bem-sucedida
pub fn apply_pending_update() -> bool {
    set_update_state(UpdateState::Applying);

    // Verifica se há um pacote de atualização pendente
    if !is_update_pending() {
        set_update_state(UpdateState::None);
        return false;
    }

    // Aqui seria a lógica real de copiar arquivos do /update/pending/ para /
    // e substituir o kernel

    set_update_state(UpdateState::Applied);
    true
}

/// Executa rollback para o último ponto de restauração
/// Retorna true se o rollback foi bem-sucedido
pub fn rollback() -> bool {
    set_update_state(UpdateState::Rollback);

    // Copiaria o kernel de backup de volta
    // e restauraria o VFS do usuário

    set_update_state(UpdateState::RolledBack);
    true
}

/// Inicializa o sistema de atualização durante o boot
pub fn init() {
    // Verifica se há uma atualização pendente no disco
    // Se houver, tenta aplicá-la
    // Se falhar, tenta rollback

    set_update_state(UpdateState::None);
}

/// Verifica se deve fazer boot com o kernel de backup (rollback)
pub fn should_boot_backup() -> bool {
    matches!(get_update_state(), UpdateState::RolledBack)
        || matches!(get_update_state(), UpdateState::Failed)
}

/// Limpa o estado de atualização após boot bem-sucedido
pub fn clear_pending() {
    set_update_state(UpdateState::None);
    UPDATE_PENDING.store(false, Ordering::Relaxed);
}
