# 04_widgets_and_callbacks_spec.md — Especificação de Widgets Interativos, Hitboxes e Callbacks dos 50 Programas

Este documento define a arquitetura dos componentes imediatos (IMGUI) do **Crystal UI** e a **Lei de Zero Botões Decorativos** para os 50 programas de nível 0 do HybridOS.

---

## 1. Lei de Zero Botões Decorativos
- Nenhum botão, aba, caixa de texto, slider, checkbox ou linha de tabela na tela pode ser meramente decorativo.
- Todo controle deve calcular colisão de coordenadas com `(mx, my)`, exibir estado de *hover* e *clique*, e disparar mutação real no estado do aplicativo, do Kernel, dos dispositivos ou do VFS.

---

## 2. Motor de Componentes "Crystal UI" em `gfx2d.rs`

1. **`ui_glass_button`**:
   - Estilos: `StandardGlass`, `PrimaryAction`, `DangerRuby`, `ToolbarFlat`.
   - Hover: borda destacada `#7DD3FC` e reflexo especular de 14%.
   - Clique: deslocamento de 1px para baixo e retorno booleano `true`.

2. **`ui_tab_bar`**:
   - Abas arredondadas de 26px conectadas ao topo da janela.
   - Alternância de `active_tab` e redesenho imediato.

3. **`ui_text_input`**:
   - Caixa de edição de texto com fundo branco `#FFFFFF` e borda chanfrada.
   - Cursor vertical piscante `|` no ritmo dos ticks do timer PIT.

4. **`ui_table_view`**:
   - Cabeçalho vitrificado com divisórias de colunas.
   - Linhas zebra `#FFFFFF` / `#F8FAFC`, linha selecionada `#BAE6FD`.
   - Barra de rolagem lateral com botões `[ ▲ ]` e `[ ▼ ]`.

5. **`ui_progress_bar`**:
   - Moldura de vidro translúcido escavada.
   - Preenchimento verde-esmeralda/azul (`0..89%`) ou vermelho-rubi (`>= 90%` ou crítico).

6. **`ui_slider`**:
   - Trilho chanfrado com pino vitrificado que atualiza valor proporcional ao arraste do mouse.

7. **`ui_checkbox_and_radio`**:
   - Quadrado de 14x14px ou círculo com marcação ativa `[✔]` ou ponto azul-safira.

8. **`ui_breadcrumbs_bar`**:
   - Barra de caminho navegável dividida em segmentos clicáveis separados por `▸`.

---

## 3. Estado Central dos Aplicativos no Desktop
O estado dos 50 aplicativos é gerenciado na struct `DesktopState` em `desktop.rs`, conectando diretamente:
- VFS (`vfs::vfs_readdir`, `vfs_create_file`, `vfs_read_all`, `vfs_unlink`).
- Scheduler (`sched::get_scheduler().get_processes_snapshot()`).
- Hardware (`PciBusScanner`, driver E1000, driver HDA, ATA PIO).
- CMOS RTC (`0x70`/`0x71`).
- Buffer de Teclado PS/2 (`enqueue_keyboard_char`).
