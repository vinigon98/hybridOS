# Skill 00 — Pesquisa de Referências Open-Source em Rust (Regra Anti-Reinvenção da Roda)

## Objetivo
Instruir qualquer agente ou subagente em loop autônomo a **SEMPRE** consultar projetos open-source consolidados em Rust antes de implementar estruturas complexas ou drivers de baixo nível do zero.

---

## 1. Diretório Base Obrigatório
Ao consultar o código-fonte do kernel neste repositório, o agente deve SEMPRE utilizar o caminho absoluto:
`C:\Users\Vinicius\Documents\HybridOS-Kernel\rust-kernel`

---

## 2. Repositórios e Projetos de Referência Obrigatória

### A. Kernel, Syscalls e Linux ABI em Rust
- **Asterinas (`asterinas/asterinas`):**
  - Modelo de arquitetura segura de sistema operacional e Framekernel em Rust.
  - Implementação canônica de compatibilidade com Linux ABI (SysV syscalls, `mmap`, `futex`, `clone`, `signal`, `fork`/`execve`).
  - Abstrações de VFS, inodes, file descriptors e subsistema de memória virtual.
- **Aero OS (`Andy-Python-Programmer/aero`):**
  - Kernel monolítico moderno em Rust com suporte nativo a binários ELF e POSIX.
  - Implementação de drivers `virtio-drivers` (VirtIO-Net, VirtIO-Gpu, VirtIO-Blk) e suporte ACPI.
- **Redox OS (`redox-os`):**
  - Implementação de microkernel em Rust, `relibc` (biblioteca C em Rust pura) e VFS baseado em esquemas (`scheme:`).
  - Tratamento de IPC, gerenciamento de threads e context switch.
- **Maestro Kernel:**
  - Referência prática de sistema compatível com POSIX em Rust/Assembly.

### B. Comandos de Terminal e Shell Universal (Tri-Shell)
- **`uutils/coreutils` (`uutils/coreutils`):**
  - Reimplementação completa das GNU Coreutils em Rust puro (binário multicall).
  - Consultar como implementar comandos rápidos de manipulação de arquivos, buffers de I/O e argumentos de linha de comando:
    - *POSIX:* `ls`, `cp`, `mv`, `rm`, `cat`, `grep`, `ps`, `kill`, `df`, `ip`, `curl`, `tar`, `chmod`.
    - *Windows CMD (Aliases & Semântica):* `dir`, `copy`, `xcopy`, `move`, `del`, `ren`, `type`, `findstr`, `tasklist`, `taskkill`, `ipconfig`, `cls`, `systeminfo`, `chkdsk`, `winget`.
- **`nushell` (`nushell/nushell`):**
  - Estruturação de dados tabulares no terminal, pipes estruturados (`|`), redirecionamentos (`>`, `>>`) e sintaxe de comandos estilo PowerShell (`Get-ChildItem`, `Set-Location`, `Get-Process`, `Invoke-WebRequest`, `Clear-Host`).

### C. Subsistema Gráfico, Compositor e Desktop Apps
- **COSMIC Desktop & `libcosmic` (`pop-os/libcosmic`):**
  - Arquitetura de interface gráfica moderna em Rust com suporte a cantos arredondados, transparência e alta legibilidade tipográfica.
  - Apps de referência: `cosmic-files` (gerenciador de arquivos), `cosmic-term` (terminal com abas), `cosmic-edit` (editor de texto), `cosmic-store` (central de aplicativos).
- **Orbital (Redox OS Window Manager):**
  - Compositor de janelas bare-metal em Rust: Z-order de janelas, renderização de decorações, botões de controle de janela e tratamento de eventos de mouse/teclado.
- **Crates Gráficas de Referência:**
  - `fontdue` / `cosmic-text`: renderização e rasterização rápida de glifos TrueType em buffers de memória sem dependência de SO hospedeiro.
  - `embedded-graphics` / `slint` / `iced`: primitivas 2D, clipping de retângulos e shaders de desenho geométrico.

### D. Execução de Binários Windows (.exe) e Tradução DirectX
- **Goblin (`m4b/goblin`):**
  - Parser PE32+/COFF e ELF64 em Rust puro para ler seções, símbolos, imports (IAT) e relocações.
- **ReactOS Userland & Wine / Proton:**
  - Estrutura de DLLs de compatibilidade Win32 (`kernel32`, `user32`, `gdi32`, `ntdll`).
  - Tradução de chamadas gráficas para aceleradores virtuais (DXVK/VKD3D).

---

## 3. Checklist do Agente Antes de Implementar
1. [ ] Eu pesquisei a abordagem open-source do Asterinas, Redox ou `uutils` para este problema?
2. [ ] Estou reutilizando padrões testados e tipos idiomáticos de Rust (#![no_std])?
3. [ ] A lógica que planejo construir respeita as invariantes documentadas em `.agents/GUARDRAILS.md`?
