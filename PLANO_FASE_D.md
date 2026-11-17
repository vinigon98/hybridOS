# PLANO_FASE_D.md — Fase D: 50 Programas Nativos de Nível 0

**Branch:** `feature/fase-d-tier0-apps`  
**Base:** HybridOS-Kernel `main` (Fases A, B, C concluídas e auditadas)  
**Objetivo:** Implementar os 50 aplicativos nativos de nível 0 (Categorias 1 a 7) descritos no `PROGRESS.md`, integrando-os ao desktop existente (`drivers/desktop.rs` + `apps_cat*.rs`).

---

## Estado Atual (verificado no código-fonte)

- ✅ Fases A, B e C implementadas e com self-tests passando (11/11 `[TEST_OK]`)
- ✅ `drivers/desktop.rs` já contém `ALL_APPS: [AppMeta; 50]`, `DesktopState`, `dispatch_app_click/render` roteando para `apps_cat1_files` até `apps_cat7_extra`
- ✅ `apps_cat1_files.rs` até `apps_cat7_extra.rs` já possuem structs de estado, handlers e renderizadores parciais
- ⏳ **Fase D (50 apps):** estrutura existe, mas muitas telas interativas estão incompletas ou são placeholders visuais sem funcionalidade real de VFS/PCI/Shell/Redes

---

## Estratégia de Execução Paralela (5 Agentes)

Os 50 apps são divididos em 5 escopos isolados, cada um atribuído a um agente. Os agentes trabalham em paralelo sobre **arquivos distintos** para evitar conflitos de merge.

| Agente | Escopo | Arquivos Principais | Apps |
|--------|--------|---------------------|------|
| **Agente 1** | Categoria 1 — Shell, Navegação e Arquivos | `apps_cat1_files.rs`, `drivers/apps/cat1_files.rs` | 01–08 |
| **Agente 2** | Categoria 2 — Monitoramento, Processos, Administração | `apps_cat2_admin.rs`, `drivers/apps/cat2_admin.rs` | 09–18 |
| **Agente 3** | Categoria 3 — Terminais, Execução, Instalação | `apps_cat3_exec.rs`, `drivers/apps/cat3_exec.rs` | 19–25 |
| **Agente 4** | Categoria 4 — Rede, Internet, Segurança | `apps_cat4_net.rs`, `drivers/apps/cat4_net.rs` | 26–33 |
| **Agente 5** | Categorias 5, 6, 7 — Escritório, Multimídia, Jogos, Acessibilidade, Ajuda | `apps_cat5_office.rs`, `apps_cat6_media.rs`, `apps_cat7_extra.rs`, `drivers/apps/cat5_office.rs`, `drivers/apps/cat6_media.rs`, `drivers/apps/cat7_extra.rs` | 34–50 |

---

## Interface Comum (Contrato entre Agentes e Kernel)

Todos os agentes devem seguir o padrão já estabelecido em `drivers/desktop.rs`:

1. **Estado:** `pub struct CatXState` com campos para cada app da categoria.
2. **Singleton:** `pub static CATX_STATE: SpinLock<Option<CatXState>>` + `fn ensure_catX_state_initialized()`.
3. **Render:** `pub unsafe fn render_catX_app(id: u8, fb, pitch, scr_w, scr_h, cx, cy, cw, ch, desk, input, prompt)`.
4. **Clique:** `pub fn handle_catX_click(id: u8, cx, cy, desk) -> bool`.
5. **Teclado (Cat5+):** `pub fn handle_catX_key(id: u8, ch, desk) -> bool`.
6. **Inicialização:** chamada de `ensure_*_state_initialized()` no `init_desktop()` de `desktop.rs`.

---

## Critérios de Aceite Globais (Definition of Done)

Para cada app implementado:

- [ ] Struct de estado dedicada no módulo da categoria
- [ ] Função `render_*` que desenha na framebuffer GOP usando `gfx2d` + `ui_widgets`
- [ ] Função `handle_*_click` com hitboxes reais (não decorativos)
- [ ] Integração com VFS real quando aplicável (leitura/escrita de arquivos)
- [ ] Integração com hardware real quando aplicável (PCI, E1000, HDA, RTC, ATA)
- [ ] Integração com Syscall/SSDT quando aplicável (Win32k, NT, POSIX)
- [ ] Registro em `ALL_APPS` já existente (não modificar, apenas garantir que `id` bate)
- [ ] Chamada via `dispatch_app_click` / `dispatch_app_render` já existe em `desktop.rs` (não modificar)
- [ ] Sem `todo!()`, `unimplemented!()`, `panic!()` em caminhos de execução normal
- [ ] Compila com `cargo build --release --target x86_64-unknown-none`
- [ ] Passou no `verify_category_apps(cat)` do `main.rs`

---

## Dependências entre Agentes

- **Nenhum agente depende de outro** para escrever código (arquivos isolados).
- **Agente 5** depende dos temas (`GLOBAL_THEME` em `apps_cat6_media.rs`) que já existem — não precisa modificar, apenas consumir.
- **Agente 3** depende de `userland/shell.rs` (já existente) para `HybridShell::execute()`.
- **Agente 4** depende de `net::smoltcp_bridge` e `drivers::e1000::GLOBAL_E1000` (já existentes).
- **Agente 1** depende de `vfs::vfs_readdir`, `vfs::vfs_read_all`, `vfs::vfs_create_file`, `vfs::vfs_unlink` (já existentes).
- **Agente 2** depende de `sched::get_scheduler()`, `mm::get_memory_info()`, `drivers::hardware::PciBusScanner` (já existentes).

---

## Merge e Validação

Após conclusão de todos os 5 agentes:

1. Cada agente faz commit em sua própria sub-branch (ex: `feature/fase-d/cat1`, `feature/fase-d/cat2`, etc.)
2. Merge das 5 sub-branches em `feature/fase-d-tier0-apps`
3. Executar `cargo build --release --target x86_64-unknown-none`
4. Executar `python qemu/verify_loop.py` — deve passar com `[TEST_OK] TIER0_APPS_CAT_1` até `TIER0_APPS_CAT_7` e `ALL_50_TIER0_APPS_INTERACTIVE`

---