# GUARDRAILS.md — Memória Anti-Regressão e Diretrizes Arquiteturais (HybridOS)

Este documento registra as invariantes absolutas, regras de segurança de baixo nível e guardrails de desenvolvimento do **HybridOS Kernel**. Todo agente ou subagente atuando neste repositório DEVE ler e respeitar integralmente estas regras antes de propor ou alterar qualquer linha de código.

---

## 1. REGRAS SUPREMAS DE ARQUITETURA E DESENVOLVIMENTO

### REGRA 1: Caminho Absoluto para Subagentes
- Todo subagente autônomo é inicializado por padrão no diretório da IDE (`C:\Users\Vinicius\AppData\Local\Programs\Antigravity`).
- **OBRIGATÓRIO:** Todo prompt enviado a um subagente DEVE conter explicitamente no topo o caminho absoluto do kernel:
  `C:\Users\Vinicius\Documents\HybridOS-Kernel\rust-kernel`
  e da raiz do repositório:
  `C:\Users\Vinicius\Documents\HybridOS-Kernel`.
- Nenhum subagente deve executar buscas cegas pelo sistema de arquivos.

### REGRA 2: Proibição Estrita de Dados Falsos / Mocks
- É **terminantemente proibido** inserir strings estáticas ou dados simulados em:
  - `refresh_vfs_cache()` em `drivers/desktop.rs`.
  - `cmd_top`, `cmd_df`, `cmd_ps`, `cmd_mem` em `userland/shell.rs`.
  - Janelas do Gerenciador de Tarefas, Explorador de Arquivos, Painel de Controle e Monitor de Recursos.
- **Origem Obrigatória dos Dados em Tempo Real:**
  - Arquivos e Diretórios: consultar diretamente `vfs::vfs_readdir()`, `vfs::vfs_stat()` e `fs::fat32`.
  - Discos e Partições: consultar `storage::get_storage_subsystem()` e volumes montados.
  - Processos e CPU: consultar a runqueue do `sched::get_scheduler()`.
  - Memória RAM e Frames: consultar contadores reais em `mm::get_memory_info()` e bitmap do PMM.
  - Rede: estado real dos sockets e interface E1000 em `net::smoltcp_bridge`.

### REGRA 3: Controle Unificado de I/O PS/2 e Renderização Gráfica
- **Entrada PS/2:** Nunca invocar `inb(0x60)` ou manipular registradores do teclado/mouse fora de `drivers::poll_ps2_controller()`. Toda leitura de teclado e mouse deve ser roteada através da fila circular centralizada de eventos para evitar perda de pacotes e descompasso de sincronia do mouse PS/2 (pacotes de 3 a 4 bytes).
- **Renderização Gráfica Sem Flickering:** Nunca escrever diretamente no framebuffer físico GOP (`gop::get_framebuffer()`) para desenhar janelas e cursor de forma imediata. Toda renderização da interface DEVE operar via Backbuffer em RAM com transferência por Dirty-Rectangles (`gfx2d`), redesenhando o cursor sobreposto sem corromper os pixels subjacentes.

### REGRA 4: Identidade Visual Proprietária "Crystal Glass Desktop"
- A linguagem de design visual oficial da interface gráfica do SO chama-se estritamente **Crystal Glass Desktop**.
- É **proibido** utilizar nomes comerciais de terceiros (ex: Windows, Microsoft, Linux, Apple, macOS) em identificadores de código, cabeçalhos, textos da UI, títulos de janelas ou recursos visuais. O kernel é um sistema operacional híbrido original com estética de vidro translúcido jateado, reflexos especulares e alta elegância.

---

## 2. INVARIANTES CRÍTICAS DE HARDWARE x86_64 BARE-METAL

1. **Sincronização e Spinlocks Seguros contra IRQ (Deadlock Prevention):**
   - Nunca adquirir ou manter um `SpinLock` sem desabilitar as interrupções de hardware da CPU (`cli`) se o mesmo lock ou recurso puder ser tocado por um tratador de interrupção (IRQ0 Timer, IRQ1 Keyboard, etc.). Utilize sempre guards que salvem o estado do flag `IF` do `RFLAGS` e restaurem-no no drop.

2. **Limites de Stack do Kernel:**
   - A stack de execução em Ring 0 possui tamanho estrito definido por CPU core (16 KiB a 64 KiB).
   - **Nunca** aloque buffers massivos (ex: `[u8; 65536]`) diretamente na stack. Estruturas grandes devem ser alocadas no Kernel Heap (`alloc::boxed::Box`, `alloc::vec::Vec`) ou em buffers estáticos paginados.

3. **Preservação de Registradores e ABI em Rotinas Naked / Assembly:**
   - Em rotinas com `#[unsafe(naked)]` (como `syscall_trampoline` e tratadores de interrupções), todos os registradores gerais (`rax`, `rbx`, `rcx`, `rdx`, `rsi`, `rdi`, `rbp`, `r8`-`r15`) devem ser explicitamente salvos na pilha na entrada e restaurados na saída.
   - O alinhamento de 16 bytes da stack da ABI AMD64 deve ser estritamente preservado antes de instruções `call`.

4. **Tratamento de Exceções e Resolução de Erros:**
   - **Nunca** utilizar `.unwrap()` desavisado em código crítico de boot, MMU, VFS ou dispatch de syscalls.
   - Utilizar correspondência explícita (`match`, `if let`) ou `.expect("motivo exato com contexto de hardware")`.

5. **Gerenciamento de Memória Virtual e PML4:**
   - Todo mapeamento de memória virtual de userspace deve manter a integridade dos 512 GiB superiores do espaço do kernel (`0xFFFF_8000_0000_0000` em diante).
   - Páginas de código em Ring 3 nunca devem ter privilégio de escrita simultâneo com execução (`NX/XD bit` ativo para dados/stack).
