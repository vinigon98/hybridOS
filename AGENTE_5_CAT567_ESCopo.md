# AGENTE 5 — Escopo: Categorias 5, 6 e 7 (Apps 34 a 50)

**Arquivos principais:** `apps_cat5_office.rs`, `apps_cat6_media.rs`, `apps_cat7_extra.rs`  
**Re-exports:** `drivers/apps/cat5_office.rs`, `drivers/apps/cat6_media.rs` (criar cat5/cat7, cat6 já existe duplicado)  
**Apps responsáveis:** Cat 5 (34–41), Cat 6 (42–47), Cat 7 (48–50)

---

## Objetivo

Implementar funcionalidade real (não placeholder) para os 17 apps de Escritório, Multimídia, Jogos, Acessibilidade e Ajuda, integrando com VFS, RTC CMOS, HDA, mouse PS/2, framebuffer GOP e syscalls.

---

## O que já existe (não reescrever)

- `AppMeta` para apps 34–50 já está em `desktop.rs` (`ALL_APPS`)
- `dispatch_app_click(34..=41)`, `(42..=47)`, `(48..=50)` e `dispatch_app_render` já roteiam para os respectivos módulos
- `GLOBAL_CLIPBOARD`, `ClipboardHistory` já existem (Cat 5)
- `NotepadState`, `CalculatorState`, `StickyNotesState`, `CalendarClockState`, `CharMapState`, `AddressBookState` já existem (Cat 5)
- `read_rtc()` (ports 0x70/0x71) já existe (Cat 5)
- `draw_line()` (Bresenham) já existe (Cat 5)
- `GLOBAL_THEME`, `THEME_PALETTE` (8 temas) já existem (Cat 6)
- `PhotoViewerState`, `PaintStudioState`, `SnippingToolState`, `MediaPlayerState`, `HdaMixerState` já existem (Cat 6)
- `encode_bmp_24bpp()` já existe (Cat 6)
- `sync_hda_volume_hardware()` já existe (Cat 6)
- `MinesweeperState`, `PuzzleState`, `Bench3DState`, `OskState`, `MagnifierState`, `RestorePoint`, `SystemRestoreState` já existem (Cat 7)
- `draw_line_clipped`, `fill_triangle_clipped` (rasterizador 3D) já existem (Cat 7)
- `enqueue_keyboard_char` já existe (Cat 7 - injeção PS/2)

---

## O que o Agente 5 deve implementar (funcionalidade real)

### Cat 5 — Escritório, Texto e Produtividade (34–41)

#### App 34 — Notepad
- [ ] Multi-linha com `notepad_text` (já existe em `DesktopState`)
- [ ] Gutter de números de linha
- [ ] Cursor piscante baseado em `SYSTEM_TICKS`
- [ ] Busca com destaque de termo
- [ ] Salvar/Carregar via VFS (`/notepad.txt` ou `C:\notepad.txt`)

#### App 35 — WordPad (Document Writer)
- [ ] Folha A4 simulada no framebuffer
- [ ] Formatação em tempo real: negrito, itálico, sublinhado, cores, alinhamento
- [ ] Régua de margens
- [ ] Salvar como `/documento.rtf` no VFS

#### App 36 — Calculadora Multi-Modo
- [ ] 3 modos: Padrão, Científica, Programador
- [ ] Modo Programador: HEX/DEC/BIN, máscaras QWORD/DWORD/WORD/BYTE, operações bitwise (AND/OR/XOR/Lsh/Rsh/Ash/RoL/RoR)
- [ ] Modo Científico: sin/cos/tan/log/sqrt/fatorial
- [ ] Histórico de cálculos
- [ ] Memória (MC/MR/M+/M-)

#### App 37 — Sticky Notes
- [ ] Cards coloridos flutuantes (passado, presente, futuro)
- [ ] Add / Delete / Cycle color
- [ ] Auto-save em `/etc/stickies.txt` via VFS

#### App 38 — Calendar, Clock & Timers
- [ ] Grid mensal interativo com `calc_day_of_week()` e `days_in_month()`
- [ ] Relógio analógico com ponteiros via `draw_line()` (Bresenham)
- [ ] RTC CMOS real via `read_rtc()`
- [ ] Cronômetro de 100Hz baseado em `SYSTEM_TICKS` com voltas

#### App 39 — Character Map & Font Viewer
- [ ] Grid ASCII 32..255
- [ ] Preview 4x amplificado de glifos
- [ ] Info Dec/Hex do caractere selecionado
- [ ] Cópia para clipboard (`GLOBAL_CLIPBOARD`)

#### App 40 — Clipboard History Manager
- [ ] Histórico dos últimos 10 itens (`GLOBAL_CLIPBOARD`)
- [ ] Selecionar / Colar / Limpar
- [ ] Preview de cada slot

#### App 41 — Contacts & Address Book
- [ ] Fichas de contatos com campos completos (nome, telefone, email, endereço)
- [ ] Busca rápida por nome
- [ ] Exportar CSV para `C:\contatos.csv` via VFS

---

### Cat 6 — Multimídia, Gráficos, Áudio e Personalização (42–47)

#### App 42 — Photo & Image Viewer
- [ ] Zoom (25%–400%), rotação (0/90/180/270), fit-to-screen
- [ ] Slideshow com timer
- [ ] 4 modos: SMPTE color bars, `C:\desenho.bmp`, `C:\screenshot.bmp`, gradiente radial
- [ ] Decodificar BMP real do VFS (`BmpView::from_bytes` + render)

#### App 43 — Paint & Pixel Canvas
- [ ] 6 ferramentas: lápis, pincel, linha, retângulo, balde, borracha
- [ ] 16 cores (`PAINT_PALETTE_16`)
- [ ] Espessura 1/2/4/8px
- [ ] Desenho direto no canvas com Bresenham + flood fill
- [ ] Blit do canvas para framebuffer GOP
- [ ] Botão [Salvar BMP] → grava `C:\desenho.bmp` no VFS (`encode_bmp_24bpp`)

#### App 44 — Snipping & Screenshot Tool
- [ ] 3 modos: tela inteira, janela, retângulo
- [ ] Captura pixels reais do framebuffer `fb`
- [ ] Thumbnail 160x100
- [ ] [Salvar Captura] → grava BMP no VFS

#### App 45 — Crystal Media Player
- [ ] Espectro sonoro animado (16 barras, phosphor glow ciano/esmeralda/rubi)
- [ ] Barra de progresso com seek por clique/arrasto
- [ ] Transportes: play/pause/stop/próximo/anterior
- [ ] Síntese de notas musicais via HDA (`sync_hda_volume_hardware`)

#### App 46 — Volume Mixer & HDA Audio
- [ ] Sliders master/system/wave/synth (verticais)
- [ ] Mute por canal
- [ ] Medidor VU estéreo
- [ ] Sincronização com hardware HDA (verbs 0x3B000)

#### App 47 — Personalization & Crystal Glass Studio
- [ ] 8 temas de vidro (`THEME_PALETTE`)
- [ ] Slider de opacidade do vidro
- [ ] Seletor de wallpaper (3 presets: Aurora, Cristalino, Obsidiano)
- [ ] Alterar `GLOBAL_THEME` em tempo real (afeta `desktop.rs` wallpaper)

---

### Cat 7 — Jogos Clássicos, Acessibilidade e Ajuda (48–50)

#### App 48 — Games Explorer & Minesweeper
- [ ] Campo Minado 10x10 com colocação de minas (LCG)
- [ ] Revelação em cascata (BFS)
- [ ] Vitória em 90 células reveladas
- [ ] Contador de minas, timer, bandeiras
- [ ] 15-Puzzle deslizante 4x4 com embaralhamento e verificação de solução
- [ ] Benchmark 3D DirectX/GDI (cubo/octaedro rotativo com `fill_triangle_clipped`)

#### App 49 — Accessibility Center & OSK
- [ ] Teclado virtual OSK clicável com injeção real via `enqueue_keyboard_char()`
- [ ] Caps/Shift states
- [ ] Lupa 2x/4x (`MagnifierState`) seguindo mouse

#### App 50 — Help & System Recovery Center
- [ ] Manual Tri-Shell: 16 comandos comparando Bash / CMD / PowerShell
- [ ] Busca no manual
- [ ] System Restore: snapshots VFS em `/snapshots` (arquivos `.snap`)
- [ ] Timestamp via RTC CMOS
- [ ] Restaurar snapshot (ler `.snap` e gravar de volta no VFS)

---

## Interfaces a usar (já existentes no kernel)

- `crate::arch::idt::SYSTEM_TICKS`
- `crate::arch::msr::{inb, outb}` (RTC 0x70/0x71)
- `crate::vfs::{vfs_create_file, vfs_read_all, vfs_unlink, vfs_readdir}`
- `crate::drivers::hda::{HDA_CONTROLLER, REG_ICO, REG_ICS}` (ou `sync_hda_volume_hardware`)
- `crate::drivers::desktop::{GLOBAL_THEME, get_current_theme}` (via `apps_cat6_media`)
- `crate::drivers::mouse::MOUSE_STATE`
- `crate::drivers::enqueue_keyboard_char`
- `crate::drivers::gfx2d::{encode_bmp_24bpp, fill_rect, draw_line, draw_text_clipped, ...}`
- `crate::drivers::ui_widgets::{ui_glass_button, ui_slider, ui_checkbox, ui_text_input, ui_progress_bar}`

---

## Critérios de Aceite (Definition of Done)

- [ ] Todos os 17 apps (34–50) têm estado nos respectivos `*State` (já existem, preencher faltantes)
- [ ] Notepad salva/lê arquivo real no VFS
- [ ] Calculadora avalia expressões reais (incluindo programador com bases)
- [ ] Paint salva BMP real no VFS
- [ ] Media Player toca notas via HDA
- [ ] Personalization muda tema visível no desktop
- [ ] Minesweeper tem revelação BFS real e condição de vitória
- [ ] OSK injeta caracteres reais no buffer PS/2
- [ ] Help & Recovery lista 16 comandos e suporta snapshots VFS
- [ ] Sem `todo!()` / `unimplemented!()` / `panic!()` em caminhos normais
- [ ] Compila sem warnings
- [ ] `verify_category_apps(5)`, `(6)`, `(7)` retornam 8, 6, 3 no `main.rs`

---

## Entregável

Commit na sub-branch `feature/fase-d/cat567` com:
1. `apps_cat5_office.rs` completo
2. `apps_cat6_media.rs` completo
3. `apps_cat7_extra.rs` completo
4. `drivers/apps/cat5_office.rs` (criar re-export)
5. `drivers/apps/cat7_extra.rs` (criar re-export)
6. Comentário: `feat(fase-d): categorias 5,6,7 - apps 34-50 funcionais`

---