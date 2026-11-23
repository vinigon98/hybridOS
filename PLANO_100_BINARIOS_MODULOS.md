# PLANO DE IMPLEMENTAÇÃO — 100 Binários Nativos + 100 Módulos de Configuração (~1.000 Parâmetros)

**Projeto:** HybridOS-Kernel  
**Branch base:** `feature/fase-d-tier0-apps` (commit `d07286d`)  
**Especificação:** `C:\Users\Vinicius\Downloads\especifica_o_arquitetural_do_hybridos_os_100_bin_rios_e_1_000_par_metros.md`  
**Status:** Nenhum dos 100 binários nem dos 100 módulos foi criado ainda (estado verificado em 17/11/2026)

---

## 🚫 REGRAS OBRIGATÓRIAS (PROIBIÇÕES)

Estas regras se aplicam a **todos os agentes** sem exceção:

1. **PROIBIDO** afirmar que uma tarefa está "pronta", "concluída" ou "implementada" sem antes:
   - Criar o código-fonte real do binário/módulo
   - Executar `cargo build --release --target x86_64-unknown-none` com sucesso (exit code 0)
   - Testar o uso da ferramenta (execução no QEMU ou modo usuário, conforme aplicável)
   - Anexar o log de execução/teste como comprovante

2. **PROIBIDO** modificar arquivos de outros agentes (escopos isolados)

3. **PROIBIDO** criar stubs, placeholders, `todo!()`, `unimplemented!()` ou `panic!()` em caminhos de execução normal

4. **PROIBIDO** comitar sem que o build compile sem erros

5. **PROIBIDO** alegar conclusão baseado apenas na leitura da especificação — o código deve existir no repositório

---

## 📋 ESTRATÉGIA DE DIVISÃO PARALELA (8 Agentes)

Os 200 itens (100 binários + 100 módulos de configuração) são divididos em 8 escopos isolados. Cada agente trabalha em **arquivos distintos**, sem sobreposição.

| Agente | Escopo | Itens | Diretório de trabalho |
|--------|--------|-------|----------------------|
| Agente 1 | Binários 1–15 (Produtividade e Interface Base) | 15 binários | `userland/apps/` + `hybctl` (parte) |
| Agente 2 | Binários 16–38 (Terminal, Desenvolvimento, Virtualização, Automação) | 23 binários | `userland/apps/` + `userland/libhybrid_sys/` |
| Agente 3 | Binários 39–60 (Monitoramento, Hardware, Performance Gamer) | 22 binários | `userland/apps/` + `rust-kernel/src/sched/` + `rust-kernel/src/mm/` |
| Agente 4 | Binários 61–82 (Administração, Armazenamento, Segurança) | 22 binários | `userland/apps/` + `rust-kernel/src/vfs/` + `rust-kernel/src/security/` |
| Agente 5 | Binários 83–100 (Rede e Conectividade) | 18 binários | `userland/apps/` + `rust-kernel/src/net/` |
| Agente 6 | Módulos de Configuração 1–5 (Compositor, Áudio, Input, Energia, Shell) | ~52 parâmetros | `/etc/hybridos/` + `hybctl` + `hyb-settings` |
| Agente 7 | Módulos de Configuração 6–10 (Dev/FS, Gamer, Rede, Privacidade, Atualizações) | ~48 parâmetros | `/etc/hybridos/` + `hybctl` + `hyb-settings` |
| Agente 8 | `hybctl` unificador + parser TOML + registro de 1.000 parâmetros | 1 utilitário central | `hybctl` CLI |

---

## 📦 AGENTE 1 — Binários 1–15: Produtividade e Interface Base

**Sub-branch:** `feature/100-binarios/agente-1-binarios-1-15`

**Arquivos a criar/modificar:**
- `userland/apps/hyb-files/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-edit/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-calc/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-capture/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-clip/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-chars/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-fonts/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-paint/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-rdp/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-vault/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-cert-user/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-cert-sys/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-focus/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-pdf/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-zoom/Cargo.toml` + `src/main.rs`
- `userland/Cargo.toml` (adicionar membros do workspace)

**Funcionalidade mínima obrigatória por binário:**
- Cada binário deve compilar como ELF64 ou PE64
- Deve ter pelo menos uma função real (não vazia) relacionada ao seu propósito na especificação
- Deve usar `libhybrid_sys` para syscalls quando aplicável

**Comprovante de entrega:**
1. `cargo build --release --target x86_64-unknown-none` → exit code 0
2. Cada pasta de app tem `Cargo.toml` válido e `src/main.rs` com código real
3. Log de build anexado

---

## 📦 AGENTE 2 — Binários 16–38: Terminal, Desenvolvimento, Virtualização, Automação

**Sub-branch:** `feature/100-binarios/agente-2-binarios-16-38`

**Arquivos a criar/modificar:**
- `userland/apps/hyb-term/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-sh/Cargo.toml` + `src/main.rs` (já existe — estender com pipelines estruturados)
- `userland/apps/hyb-script-studio/Cargo.toml` + `src/main.rs`
- `userland/apps/cmd-compat/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-pkg/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-linux/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-vm/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-sandbox/Cargo.toml` + `src/main.rs`
- `userland/apps/ssh/Cargo.toml` + `src/main.rs`
- `userland/apps/scp/Cargo.toml` + `src/main.rs`
- `userland/apps/sftp/Cargo.toml` + `src/main.rs`
- `userland/apps/ssh-keygen/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-ssh-agent/Cargo.toml` + `src/main.rs`
- `userland/apps/curl/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-http/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-archive/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-hash/Cargo.toml` + `src/main.rs`
- `userland/apps/which/Cargo.toml` + `src/main.rs`
- `userland/apps/where/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-grep/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-sync/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-cron/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-cronctl/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-runner/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-pipe/Cargo.toml` + `src/main.rs`

**Comprovante de entrega:**
1. Build sem erros
2. Cada binário tem pelo menos um comando ou função real implementada
3. Log de build anexado

---

## 📦 AGENTE 3 — Binários 39–60: Monitoramento, Hardware, Performance Gamer

**Sub-branch:** `feature/100-binarios/agente-3-binarios-39-60`

**Arquivos a criar/modificar:**
- `userland/apps/hyb-top/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-resmon/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-telemetry-local/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-crashlog/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-gpuinfo/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-hwinfo/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-devices/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-logs/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-memtest/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-driver-check/Cargo.toml` + `src/main.rs`
- `userland/apps/ps/Cargo.toml` + `src/main.rs`
- `userland/apps/kill/Cargo.toml` + `src/main.rs`
- `userland/apps/pkill/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-power/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-joy/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-audio-ctl/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-mixer/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-color/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-font-tune/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-perf-ui/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-env/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-bootcfg/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-reboot/Cargo.toml` + `src/main.rs`

**Integração com kernel (obrigatório para alguns):**
- `hyb-top`, `hyb-resmon` → ler `rust-kernel/src/sched/` e `rust-kernel/src/mm/` reais
- `hyb-power` → usar `rust-kernel/src/arch/acpi.rs` e MSRs reais
- `hyb-devices` → usar `rust-kernel/src/drivers/hardware.rs` (PCI scanner real)

**Comprovante de entrega:**
1. Build sem erros
2. Pelo menos 5 binários lendo dados reais do kernel (scheduler, memória, PCI, ACPI)
3. Log de build anexado

---

## 📦 AGENTE 4 — Binários 61–82: Administração, Armazenamento, Segurança

**Sub-branch:** `feature/100-binarios/agente-4-binarios-61-82`

**Arquivos a criar/modificar:**
- `userland/apps/hyb-settings/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-tui-config/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-admin/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-service/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-config-tree/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-policy/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-secpol/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-users/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-disks/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-disk-cli/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-trim/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-clean/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-verify/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-image/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-fsck/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-fsctl/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-guard/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-firewall/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-crypt/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-snapshot/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-print/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-sudo/Cargo.toml` + `src/main.rs`

**Integração com kernel (obrigatório):**
- `hyb-disks`, `hyb-disk-cli`, `hyb-fsck`, `hyb-fsctl` → usar `rust-kernel/src/vfs/` e `rust-kernel/src/fs/` reais
- `hyb-service` → usar `rust-kernel/src/sched/` real
- `hyb-firewall` → usar `rust-kernel/src/net/filter.rs` real
- `hyb-snapshot` → usar VFS + COW do MM real

**Comprovante de entrega:**
1. Build sem erros
2. Pelo menos 5 binários integrados com VFS/PCI/ACPI reais do kernel
3. Log de build anexado

---

## 📦 AGENTE 5 — Binários 83–100: Rede e Conectividade

**Sub-branch:** `feature/100-binarios/agente-5-binarios-83-100`

**Arquivos a criar/modificar:**
- `userland/apps/hyb-net-adapters/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-shares/Cargo.toml` + `src/main.rs`
- `userland/apps/ip/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-ip/Cargo.toml` + `src/main.rs`
- `userland/apps/ping/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-mtr/Cargo.toml` + `src/main.rs`
- `userland/apps/traceroute/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-ports/Cargo.toml` + `src/main.rs`
- `userland/apps/dig/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-dns/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-netctl/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-lan-scan/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-route/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-mac/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-mount-net/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-tcp-test/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-hosts/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-proxy/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-bt/Cargo.toml` + `src/main.rs`
- `userland/apps/hyb-bundle/Cargo.toml` + `src/main.rs`

**Integração com kernel (obrigatório):**
- `hyb-net-adapters`, `hyb-ip`, `hyb-ports` → usar `rust-kernel/src/net/` real (smoltcp, sockets)
- `hyb-ping`, `hyb-mtr`, `traceroute` → usar `perform_ping` / `perform_curl` reais
- `hyb-netctl` → usar `global_network_filter` real

**Comprovante de entrega:**
1. Build sem erros
2. Pelo menos 5 binários usando pilha de rede real do kernel (smoltcp, E1000)
3. Log de build anexado

---

## 📦 AGENTE 6 — Módulos de Configuração 1–5 (Compositor, Áudio, Input, Energia, Shell)

**Sub-branch:** `feature/100-modulos/agente-6-modulos-1-5`

**Arquivos a criar/modificar:**
- `etc/hybridos/system.toml` (criar — arquivo mestre com seções [display], [audio], [input], [power], [shell])
- `userland/apps/hybctl/Cargo.toml` + `src/main.rs` (parte: parser TOML, registro de parâmetros)
- `userland/apps/hyb-settings/Cargo.toml` + `src/main.rs` (parte: interface gráfica dos módulos 1–5)

**Parâmetros obrigatórios (do especificação):**

**Módulo 1 — Compositor de Vídeo (12):**
- `display.refresh_rate`, `display.scaling`, `display.multi_monitor`, `graphics.scheduler_hags`, `graphics.direct_scanout`, `graphics.gpu_preference`, `display.night_light`, `display.hdr_pipeline`, `display.disable_cabc`, `graphics.tdr_recovery`, `graphics.legacy_dpi_fix`, `graphics.frame_limiter_global`

**Módulo 2 — Áudio (8):**
- `audio.devices_profile`, `audio.bit_perfect`, `audio.quantum_buffer`, `audio.lock_mic_gain`, `audio.ducking`, `audio.rnnoise_filter`, `audio.per_app_routing`, `audio.system_sounds`

**Módulo 3 — Input (10):**
- `input.mouse.raw_input`, `input.mouse.polling_rate_support`, `input.mouse.scroll_inactive`, `input.mouse.middle_click_paste`, `input.keyboard.repeat_rate`, `input.keyboard.no_sticky_keys`, `input.keyboard.remap_caps`, `input.keyboard.clean_layout`, `input.touchpad.gestures`, `input.usb.no_autosuspend`

**Módulo 4 — Energia (8):**
- `power.profile`, `power.pcie_aspm`, `power.cpu_scheduler`, `power.boost_control`, `power.clean_shutdown`, `power.hibernation`, `power.battery_charge_limit`, `power.disk_spindown`

**Módulo 5 — Shell/Explorador (14):**
- `files.show_extensions`, `files.show_dotfiles`, `files.default_view`, `files.zero_ads`, `files.header_path`, `shell.context_menu`, `shell.taskbar_clean`, `shell.panel_position`, `shell.force_kill_shortcut`, `shell.launcher`, `shell.window_tiling`, `shell.virtual_desktops`, `shell.compositor_effects`, `shell.tray_telemetry`

**Comprovante de entrega:**
1. `system.toml` existe com todos os 52 parâmetros listados acima
2. `hybctl` consegue ler e gravar pelo menos 10 parâmetros destes módulos
3. Build sem erros
4. Log de execução do `hybctl get <parametro>` anexado

---

## 📦 AGENTE 7 — Módulos de Configuração 6–10 (Dev/FS, Gamer, Rede, Privacidade, Atualizações)

**Sub-branch:** `feature/100-modulos/agente-7-modulos-6-10`

**Arquivos a criar/modificar:**
- `etc/hybridos/system.toml` (extender — adicionar seções [developer], [fs], [security], [gaming], [net], [privacy], [update], [system])
- `userland/apps/hybctl/Cargo.toml` + `src/main.rs` (parte: mais parâmetros)
- `userland/apps/hyb-settings/Cargo.toml` + `src/main.rs` (parte: interface dos módulos 6–10)

**Parâmetros obrigatórios (do especificação):**

**Módulo 6 — Dev/FS/Memória (13):**
- `dev.mode_default`, `fs.unlimited_paths`, `fs.case_sensitivity_per_dir`, `fs.dev_optimization`, `security.dev_trust_folders`, `locale.utf8_everywhere`, `locale.technical_formats`, `env.session_reload`, `memory.zram_swap`, `virt.dynamic_memory`, `virt.port_auto_bind`, `dev.localhost_domains`, `storage.temp_tmpfs`

**Módulo 7 — Gamer/Kernel (8):**
- `gaming.kernel_boost`, `gaming.replay_buffer_ram`, `gaming.fullscreen_silence`, `gaming.hvci_toggle_per_boot`, `system.startup_sandbox`, `system.background_freeze`, `gaming.shader_cache_manager`, `gaming.overlay_minimal`

**Módulo 8 — Rede/Segurança (11):**
- `net.dns_encrypted`, `net.congestion_control_bbr`, `net.nic_low_latency`, `net.qos_gaming_voip`, `net.ipv6_privacy`, `net.zero_config_share`, `net.ntp_high_precision`, `net.wifi_band_steering`, `net.no_p2p_updates`, `net.wireguard_native`, `net.dev_firewall_profiles`

**Módulo 9 — Privacidade (9):**
- `privacy.zero_telemetry`, `privacy.no_ad_id`, `privacy.offline_search_only`, `privacy.smart_indexer`, `privacy.hardware_kill_switches`, `privacy.clipboard_guard`, `privacy.no_nag_screens`, `privacy.no_silent_installs`, `privacy.clean_uninstall`

**Módulo 10 — Atualizações/Imutabilidade (7):**
- `update.manual_or_scheduled`, `update.driver_pinning`, `update.channel_selection`, `system.default_apps_lock`, `system.atomic_rollback`, `system.dotfiles_sync`, `system.master_palette`

**Comprovante de entrega:**
1. `system.toml` tem todos os ~48 parâmetros destes módulos + os do Agente 6 (total ~100)
2. `hybctl get <parametro>` funciona para qualquer parâmetro dos 10 módulos
3. Build sem erros
4. Log de execução anexado

---

## 📦 AGENTE 8 — `hybctl` Unificador + Parser TOML + Registro de 1.000 Parâmetros

**Sub-branch:** `feature/100-modulos/agente-8-hybctl-unificador`

**Arquivos a criar/modificar:**
- `userland/apps/hybctl/Cargo.toml` + `src/main.rs` (principal)
- `userland/apps/hybctl/src/parser.rs` (parser TOML)
- `userland/apps/hybctl/src/registry.rs` (registro de 1.000 parâmetros)
- `userland/apps/hybctl/src/commands.rs` (get, set, list, reset, export, import)
- `userland/Cargo.toml` (adicionar `hybctl` como membro)

**Funcionalidade obrigatória do `hybctl`:**
- `hybctl get <caminho>` — lê um parâmetro do `system.toml`
- `hybctl set <caminho> <valor>` — grava um parâmetro no `system.toml`
- `hybctl list [modulo]` — lista todos parâmetros de um módulo ou todos
- `hybctl reset <caminho>` — reseta para valor padrão
- `hybctl export` — exporta toda configuração para TOML
- `hybctl import <arquivo>` — importa configuração de arquivo TOML

**Integração obrigatória:**
- `hybctl` deve ler/escrever em `etc/hybridos/system.toml` (via VFS ou arquivo real)
- Todos os 100 parâmetros dos 10 módulos devem estar registrados no `hybctl`

**Comprovante de entrega:**
1. `cargo build --release` do `hybctl` → exit code 0
2. `hybctl get display.refresh_rate` retorna um valor válido
3. `hybctl set input.mouse.raw_input true` grava no `system.toml`
4. `hybctl list` mostra todos os módulos e parâmetros
5. Log de execução de todos os comandos acima anexado

---

## ✅ DEFINITION OF DONE (CRITÉRIOS DE ACEITE GLOBAIS)

Para considerar a tarefa completa, **todos** os seguintes devem ser verdadeiros:

1. **Build completo:** `cargo build --release --target x86_64-unknown-none` compila sem erros no `rust-kernel/` E no `userland/`
2. **100 binários:** Cada um dos 100 binários tem pasta em `userland/apps/` com `Cargo.toml` e `src/main.rs` contendo código real (não vazio)
3. **100 módulos:** `etc/hybridos/system.toml` contém ~100 parâmetros organizados em 10 módulos
4. **hybctl funcional:** `hybctl get`, `hybctl set`, `hybctl list` executam e retornam valores reais do `system.toml`
5. **Logs de comprovação:** Cada agente anexou log de build e/ou execução à sua resposta
6. **Nenhum stub:** Não há `todo!()`, `unimplemented!()` ou `panic!()` em caminhos normais de execução

---

## 🔄 FLUXO DE EXECUÇÃO ESPERADO

```
1. Agente N lê este plano e o arquivo de especificação
2. Agente N cria checkout na sub-branch própria
3. Agente N implementa seu escopo (código real, não placeholder)
4. Agente N executa cargo build e testa o uso
5. Agente N anexa log de comprovação na resposta
6. Orquestador verifica cada log e código
7. Merge das 8 sub-branches na branch principal
8. Build final completo + verificação de não-regressão
```

---

## ⚠️ O QUE NÃO DEVE SER FEITO

- Não criar arquivos `.md` de documentação no lugar de código-fonte
- Não criar apenas estruturas vazias sem implementação
- Não modificar `main.rs`, `desktop.rs` ou arquivos da Fase D já existentes
- Não afirmar conclusão sem o log de build e teste anexado