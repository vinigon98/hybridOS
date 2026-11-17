# AGENTE 3 — Escopo: Categoria 3 (Apps 19 a 25)

**Arquivo principal:** `rust-kernel/src/drivers/apps_cat3_exec.rs`  
**Re-export:** `rust-kernel/src/drivers/apps/cat3_exec.rs` (criar, não existe)  
**Apps responsáveis:** 19 Terminal Tri-Shell, 20 Run Dialog, 21 App Store, 22 Setup Wizard, 23 Default Programs, 24 Dual-ABI Center, 25 Environment Variables

---

## Objetivo

Implementar funcionalidade real (não placeholder) para os 7 apps de Terminais, Execução e Instalação, integrando com HybridShell, loaders PE/ELF, VFS e syscalls.

---

## O que já existe (não reescrever)

- `AppMeta` para apps 19–25 já está em `desktop.rs` (`ALL_APPS`)
- `dispatch_app_click(19..=25)` e `dispatch_app_render(19..=25)` já roteiam para `apps_cat3_exec`
- `ShellMode` enum (Bash/Cmd/PowerShell) já existe
- `SoftwarePackage` + `SOFTWARE_CATALOG` (8 pacotes) já existe
- `ExtMapping` (associações .txt/.sh/.bat/etc.) já existe
- `BinaryInspection` (detecção ELF vs PE) já existe
- `Cat3State` struct já existe com campos para todos os 7 apps
- `CAT3_STATE: SpinLock<Cat3State>` já existe
- `inspect_target_path()` já existe (lê binário do VFS, detecta ELF/PE)
- `render_cat3_app` roteador já existe

---

## O que o Agente 3 deve implementar (funcionalidade real)

### App 19 — Terminal Tri-Shell Gráfico
- [ ] Toolbar com 3 botões de modo (Bash / CMD / PowerShell) — trocar `shell_mode`
- [ ] Chips de comando rápido (dir, ls, ps, etc.)
- [ ] Área de terminal com `term_history: Vec<String>` e coloração por tipo (prompt verde/prata/azul, erro vermelho)
- [ ] Barra de input com cursor piscante baseado em `crate::arch::idt::SYSTEM_TICKS`
- [ ] Botão [Executar] → instanciar `HybridShell::new()` e `shell.execute(trimmed)`
- [ ] Suporte a `clear` / `cls` → limpar `term_history`
- [ ] Pipe `|` e redirecionamento `>`/`>>` já funcionam no `HybridShell` (verificar)

### App 20 — Run Dialog
- [ ] Campo de input para comando/caminho
- [ ] Chips de histórico (taskmgr, explorer, cmd, bash, powershell, calc, notepad)
- [ ] Botões [OK] / [Cancelar] / [Procurar]
- [ ] `execute_run_dialog_action()` — despachar janelas GUI (explorer→FileExplorer, taskmgr→TaskManager, notepad→Notepad, calc→Calculator, sysinfo→SystemInfo, appstore→AppStore)
- [ ] Execução via `HybridShell` para comandos de texto

### App 21 — App Store & Software Hub
- [ ] 4 abas: Destaques, Ferramentas, Jogos, Desenvolvedor
- [ ] Busca filtrando `SOFTWARE_CATALOG`
- [ ] Grid de cards com nome, versão, tamanho, descrição, tipo ABI
- [ ] Botão [Instalar] → gravar binário sintético real no VFS via `create_synthetic_elf()` / `create_synthetic_pe()` em `/bin/` e `C:\Program Files\`
- [ ] Botão [Remover] → `vfs_unlink` do instalado
- [ ] Barra de progresso de instalação

### App 22 — Setup Wizard (MSI/EXE)
- [ ] 3 etapas: EULA (checkbox), Diretório alvo (chips), Extração (progress bar animada)
- [ ] `advance_wizard_step()` — gravar `package.manifest`, `.dll` (PE), `.elf`, `readme.txt` no VFS
- [ ] Validação de hash SHA-256 do pacote (usar `crate::security::sha256::sha256_hex`)

### App 23 — Default Programs Manager
- [ ] Tabela de associações de extensão (.txt, .sh, .bat, .bmp, .bin, .elf, .exe, .conf, .log)
- [ ] Programa padrão e subsistema (Windows NT / Linux POSIX)
- [ ] Botão [Salvar no VFS] → gravar `/etc/associations.conf`

### App 24 — Dual-ABI Center
- [ ] Inspetor de binários ELF64 vs PE32+
- [ ] Selecionar arquivo no VFS, exibir: magic bytes, entry point, seções, imports, personalidade
- [ ] Botão [Spawn Process] → `loaders::spawn_elf_process` ou `loaders::spawn_pe_process`

### App 25 — Environment Variables Editor
- [ ] `user_env` e `sys_env` (já existem no estado)
- [ ] Edição de campos (adicionar/remover variável)
- [ ] Busca filtrada
- [ ] Persistência em `/etc/environment` via VFS

---

## Interfaces a usar (já existentes no kernel)

- `crate::userland::shell::{HybridShell, create_synthetic_elf, create_synthetic_pe}`
- `crate::loaders::{spawn_elf_process, spawn_pe_process}`
- `crate::vfs::{vfs_create_file, vfs_read_all, vfs_unlink, vfs_readdir}`
- `crate::security::sha256::sha256_hex`
- `crate::drivers::desktop::{refresh_vfs_cache}`
- `crate::arch::idt::SYSTEM_TICKS`

---

## Critérios de Aceite (Definition of Done)

- [ ] Todos os 7 apps têm estado em `Cat3State` (já existe, preencher faltantes)
- [ ] Terminal executa comandos reais via `HybridShell::execute()`
- [ ] App Store instala binário sintético real no VFS
- [ ] Setup Wizard grava arquivos no VFS em 3 etapas
- [ ] Dual-ABI Center spawn processo ELF ou PE real
- [ ] Sem `todo!()` / `unimplemented!()` / `panic!()` em caminhos normais
- [ ] Compila sem warnings
- [ ] `verify_category_apps(3)` retorna 7 no `main.rs`

---

## Entregável

Commit na sub-branch `feature/fase-d/cat3` com:
1. `apps_cat3_exec.rs` completo
2. `drivers/apps/cat3_exec.rs` (criar re-export)
3. Comentário: `feat(fase-d): categoria 3 - apps 19-25 funcionais`

---