# SPEC.md — Especificação Técnica e Arquitetura do HybridOS

O **HybridOS** é um sistema operacional bare-metal de 64 bits para arquitetura x86_64, escrito integralmente em Rust (`#![no_std]`, `#![no_main]`). Ele combina um núcleo híbrido de alta performance com dupla personalidade nativa (execução simultânea de binários e drivers de estilo Windows NT e Linux POSIX em Ring 3) e uma interface de usuário rica e moderna inspirada na elegância de vidro jateado e alta usabilidade (**Crystal Glass Desktop**).

---

## 1. MAPA DE MEMÓRIA E LAYOUT DE ENDEREÇOS VIRTUAIS (PML4)

O kernel opera no Higher-Half superior da memória virtual de 64 bits utilizando paginação x86_64 de 4 níveis (PML4, PDPT, PD, PT):

| Faixa Virtual (Canonical x86_64) | Tamanho | Uso e Permissões |
| :--- | :--- | :--- |
| `0x0000_0000_0000_0000` - `0x0000_7FFF_FFFF_FFFF` | 128 TiB | **Espaço de Usuário (Ring 3)**: código ELF/PE, pilha do usuário, heap de processos e TEB/TLS. |
| `0xFFFF_8000_0000_0000` - `0xFFFF_8000_3FFF_FFFF` | 1 TiB | **Direct Physical Memory Mapping (HHDM)**: acesso a toda a RAM física sem remapeamento constante. |
| `0xFFFF_9000_0000_0000` - `0xFFFF_9000_0100_0000` | 16 MiB | **Kernel Dynamic Heap**: gerenciado pelo `GlobalAllocWrapper` para estruturas `Box`, `Vec` e `String`. |
| `0xFFFF_A000_0000_0000` - `0xFFFF_A000_0FFF_FFFF` | 256 MiB | **MMIO & Framebuffer GOP**: mapeamento do buffer gráfico de vídeo e periféricos de hardware. |
| `0xFFFF_FFFF_8000_0000` - `0xFFFF_FFFF_FFFF_FFFF` | 2 GiB | **Kernel Code, Data & Drivers (Ring 0)**: seções `.text`, `.rodata`, `.data`, `.bss` carregadas pelo Limine. |

---

## 2. ARQUITETURA DE MULTITAREFA, ABI E CONVENSÕES DE CHAMADA

### 2.1 Despachante Universal Dual-ABI
O processador programa os MSRs x86_64 (`IA32_STAR`, `IA32_LSTAR`, `IA32_SFMASK`) para interceptar a instrução rápida `SYSCALL`:
- **Linux POSIX ABI (SysV AMD64):**
  - Número da Syscall: `%rax`
  - Argumentos: `%rdi`, `%rsi`, `%rdx`, `%r10`, `%r8`, `%r9`
  - Retorno: `%rax`
  - GS Base: aponta para a estrutura per-CPU do kernel (`PerCpuData`) em Ring 0 e zero em Ring 3.
- **Windows NT ABI (Microsoft x64 Fast System Call):**
  - Número do Serviço SSDT: `%rax`
  - Argumentos: `%rcx`, `%rdx`, `%r8`, `%r9`, parâmetros adicionais na pilha do usuário
  - Retorno: `%rax` (NTSTATUS: `0x00000000` para `STATUS_SUCCESS`)
  - GS Base em Ring 3: aponta para o Bloco de Ambiente de Thread (TEB / `0x0000_0000_7FFE_1000`), restaurado via `swapgs` na transição de anéis.

### 2.2 Troca de Contexto Preemptiva
- O temporizador de hardware (PIT IRQ0 a 100Hz ou APIC Timer) interrompe a execução de Ring 3.
- O tratador de interrupção salva o `TrapFrame` da tarefa atual, comuta o registrador `CR3` (PML4) para o processo seguinte da runqueue e reconfigura `TSS.RSP0` e `IA32_KERNEL_GS_BASE`.
- A saída ocorre via `iretq` ou `sysretq`, garantindo isolamento estrito entre tarefas independentes.

---

## 3. ARQUITETURA GRÁFICA "CRYSTAL GLASS DESKTOP"

O subsistema gráfico do HybridOS entrega uma experiência de usuário rica, fluida e amigável, dispensando telas pretas puramente acadêmicas e proporcionando conforto visual imediato.

```
+---------------------------------------------------------------------------------+
|                                 ÁREA DE TRABALHO                                |
|  [Computador] [Explorador] [Navegador] [Loja de Apps]                           |
|                                                                                 |
|       +------------------- Bloco de Notas / Janela --------------------+        |
|       | [~] Bloco de Notas - Crystal Glass                     [-][□][X]|        |
|       +----------------------------------------------------------------+        |
|       | Interface com vidro jateado, reflexo especular e tipografia    |        |
|       | TrueType nítida renderizada em tempo real sobre backbuffer.    |        |
|       +----------------------------------------------------------------+        |
|                                                                                 |
+---------------------------------------------------------------------------------+
| (Emblema) | [Prog 1] [Prog 2] [Prog 3]           | [Rede] [Som] [ 20:30 ] | [|] |
+---------------------------------------------------------------------------------+
```

### 3.1 Pipeline de Renderização
1. **Backbuffer em Memória Central:** Alocação de buffer gráfico RGBA32 de 1280x800x4 bytes em RAM do kernel.
2. **Transferência por Dirty-Rectangles:** Atualizações de tela rastreiam regiões modificadas, evitando repinturas completas de 4MB por frame e garantindo 60 FPS estáveis no QEMU.
3. **Shader 2D de Vidro Jateado (Crystal Glass Effect):**
   - Efeito de dispersão de luz (Box Blur suavizado do fundo sob a janela).
   - Tintura translúcida azul-safira elegante: `RGBA(22, 50, 84, 150)`.
   - Faixas diagonais sutis de reflexo especular branco com variação de opacidade.
   - Chanfro de 1px branco translúcido nas bordas internas e cantos superiores arredondados (raio de 6px).
4. **Cápsula de Controle Vitrificada:**
   - Três botões convexos no canto superior direito: Minimizar `[-]`, Maximizar `[□]` e Fechar `[X]`.
   - O botão Fechar destaca-se em vermelho-rubi brilhante com iluminação interna ativada por aproximação do cursor.
5. **Superbarra e Emblema Esférico:**
   - Barra inferior de 40px com efeito de vidro espelhado.
   - À esquerda, o Emblema Esférico de Cristal com 44px de diâmetro eleva-se sutilmente sobre a barra.
   - À direita, a Bandeja de Sistema exibe status dinâmico da controladora de rede E1000, nível de áudio, relógio e data, finalizada pelo botão retangular de "Revelar Área de Trabalho".

---

## 4. SUÍTE DE APLICATIVOS NATIVOS E SUBSISTEMA DE PROGRAMAS

### 4.1 Aplicativos Essenciais de Usuário
- **File Explorer:** Navegador de arquivos com abas, painel lateral de favoritos/discos, histórico de navegação, visualização de capacidade das unidades de disco com barras de progresso e manipulação real de arquivos no VFS.
- **Task Manager:** Monitor de desempenho em tempo real com gráficos vetoriais contínuos de uso de CPU, alocação de frames de RAM, tráfego de rede E1000 e tabela dinâmica de processos ativos com finalização forçada.
- **Settings & Control Panel:** Central de controle para personalização de display, endereçamento IP (DHCP/Estático), configuração de dispositivos PCI e perfis de usuário.
- **Tri-Shell Terminal:** Emulador de terminal gráfico que reconhece transparentemente comandos das 3 famílias: POSIX (`ls`, `grep`, `ps`), Windows CMD (`dir`, `tasklist`, `ipconfig`) e PowerShell (`Get-ChildItem`, `Get-Process`).
- **App Store & Setup Wizard:** Gerenciador gráfico de instalação de aplicativos com download via HTTP (`smoltcp`), barra de progresso vitrificada e integração imediata de atalhos.
- **Utilitários Adicionais:** Notepad (editor de texto com VFS), Calculadora (básica e científica) e Media Viewer (renderizador de imagens e reprodução sonora via Intel HDA).

### 4.2 Compatibilidade de Programas e Jogos
- **Execução ELF64 Nativa:** Camada de compatibilidade Linux ABI para carregamento de binários dinâmicos e bibliotecas compartilhadas, viabilizando ferramentas complexas de código aberto e navegadores web (como Google Chrome/Chromium).
- **Execução PE32+ (.exe):** Loader PE/COFF de 64 bits com resolução de IAT, relocação e stubs de emulação de subsistema Win32/GDI para aplicativos do ecossistema Windows.
- **Suporte a Jogos e DirectX:** Camada de tradução gráfica mapeando chamadas DirectX/GDI para comandos 2D/3D sobre aceleradores de hardware e emulação gráfica do QEMU (VirtIO-GPU/Vulkan/Virgl).
