# AGENTE 1 — Escopo: Categoria 1 (Apps 01 a 08)

**Arquivo principal:** `rust-kernel/src/drivers/apps_cat1_files.rs`  
**Re-export:** `rust-kernel/src/drivers/apps/cat1_files.rs`  
**Apps responsáveis:** 01 File Explorer, 02 Superbar Host, 03 Global Search, 04 Archive Manager, 05 Disk Manager, 06 Disk Cleanup, 07 ISO Mounter, 08 Recycle Bin

---

## Objetivo

Implementar funcionalidade real (não placeholder) para os 8 apps de Shell, Navegação e Arquivos, integrando com VFS, FAT32, ATA e o desktop Crystal Glass existente.

---

## O que já existe (não reescrever)

- `AppMeta` para apps 01–08 já está em `desktop.rs` (`ALL_APPS`)
- `dispatch_app_click(1..=8)` e `dispatch_app_render(1..=8)` já roteiam para `apps_cat1_files`
- `compute_crc32()` já implementado
- `Cat1AppState` struct já existe com campos para todos os 8 apps
- `CAT1_STATE: SpinLock<Cat1AppState>` já existe
- `ensure_cat1_initialized()` já existe (cria `/.trash`, `/tmp`, etc.)
- `render_cat1_app` / `handle_cat1_click` roteadores já existem

---

## O que o Agente 1 deve implementar (funcionalidade real)

### App 01 — File Explorer
- [ ] Listagem real de `desk.vfs_entries` (preenchida por `refresh_vfs_cache()`)
- [ ] Breadcrumbs navegáveis: clique em segmento do caminho muda `desk.current_vfs_path` e chama `refresh_vfs_cache()`
- [ ] Botão [Nova Pasta] → `vfs::vfs_mkdir(path)`
- [ ] Botão [Copiar] / [Colar] → `vfs::vfs_read_all` + `vfs::vfs_create_file`
- [ ] Botão [Excluir] → `vfs::vfs_unlink(path)`
- [ ] Barra de capacidade do disco (usar `mm::get_memory_info()` ou contador de frames alocados)
- [ ] Ícones `[DIR]` / `[FILE]` diferenciados (já existem no `refresh_vfs_cache`)

### App 02 — Superbar Host
- [ ] Toggle de altura da superbarra (32/40/48px) refletido em `desk` (já existe `superbar_height` no estado)
- [ ] Menu duplo assimétrico (esquerda sólida / direita vidro) — já renderizado em `desktop.rs`
- [ ] Lista de apps fixados (`superbar_pinned_apps`) com add/remover

### App 03 — Global Search
- [ ] `recursive_vfs_search(path, query, depth)` real — varre `vfs_readdir` recursivamente (limite profundidade 4)
- [ ] Filtros em pílulas: Tudo / Programas / Documentos / Pastas
- [ ] Clique em resultado muda `desk.current_vfs_path` e abre File Explorer lá

### App 04 — Archive Manager
- [ ] Tabela com arquivos reais do VFS, mostrando `size` e `crc32` (usar `compute_crc32`)
- [ ] Botão [Extrair] → lê arquivo com `vfs_read_all`, grava partes em `/tmp/`
- [ ] Cálculo de taxa de compactação real

### App 05 — Disk & Partition Manager
- [ ] Leitura real de setores via `drivers::ata::GLOBAL_ATA` (identify device, LBA)
- [ ] Parser MBR/GPT simples (ler setor 0, identificar partições)
- [ ] Diagrama de blocos coloridos mostrando partições detectadas

### App 06 — Disk Cleanup & Analyzer
- [ ] `scan_cleanup_vfs_sizes()` — percorre VFS somando tamanhos por pasta
- [ ] `clean_vfs_dir_files()` — exclui arquivos temporários (`/tmp/*`, `C:\Windows\Temp\*`)
- [ ] Barra de progresso de limpeza real

### App 07 — ISO & Disk Image Mounter
- [ ] Detecção de arquivos `.img`/`.iso` no VFS
- [ ] "Montar em D:\" / `/mnt/iso` — criar entrada no VFS apontando para offset do arquivo

### App 08 — Recycle Bin Manager
- [ ] `sync_trash_from_vfs()` — ler `/.trash` e listar itens excluídos
- [ ] Botão [Restaurar] → `vfs::vfs_create_file(caminho_original, conteúdo)`
- [ ] Botão [Esvaziar] → `vfs::vfs_unlink` em todos os itens de `/.trash`

---

## Interfaces a usar (já existentes no kernel)

- `crate::vfs::{vfs_readdir, vfs_read_all, vfs_create_file, vfs_unlink, vfs_mkdir}`
- `crate::vfs::{DirEntry, InodeType}` — `e.ino_type.is_dir()` / `e.ino_type.is_file()`
- `crate::drivers::ata::GLOBAL_ATA` — leitura de setores físicos
- `crate::drivers::desktop::{refresh_vfs_cache, DESKTOP, DesktopState}`
- `crate::drivers::gfx2d::{fill_rounded_rect, draw_text_clipped, draw_text_px, draw_glass_control_capsule, ...}`
- `crate::drivers::ui_widgets::{ui_glass_button, ui_table_view, ui_progress_bar, ui_text_input, ui_breadcrumbs_bar}`

---

## Critérios de Aceite (Definition of Done)

- [ ] Todos os 8 apps têm struct de estado em `Cat1AppState` (já existe, preencher campos faltantes)
- [ ] `render_cat1_app` e `handle_cat1_click` implementados para todos os 8 apps
- [ ] Pelo menos 1 operação real de VFS por app (ler/escrever/excluir/criar diretório)
- [ ] Sem `todo!()` / `unimplemented!()` / `panic!()` em caminhos normais
- [ ] Compila sem warnings no `cargo build --release --target x86_64-unknown-none`
- [ ] `verify_category_apps(1)` retorna 8 no `main.rs`

---

## Entregável

Commit na sub-branch `feature/fase-d/cat1` com:
1. `apps_cat1_files.rs` completo
2. `drivers/apps/cat1_files.rs` (re-export)
3. Comentário no commit: `feat(fase-d): categoria 1 - apps 01-08 funcionais`

---