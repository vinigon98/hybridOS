# AGENTE 2 — Escopo: Categoria 2 (Apps 09 a 18)

**Arquivo principal:** `rust-kernel/src/drivers/apps_cat2_admin.rs`  
**Re-export:** `rust-kernel/src/drivers/apps/cat2_admin.rs`  
**Apps responsáveis:** 09 Task Manager, 10 Resource Monitor, 11 Control Panel, 12 Device Manager, 13 Services Manager, 14 System Information, 15 Event Viewer, 16 Registry Editor, 17 System Config, 18 Power Options

---

## Objetivo

Implementar funcionalidade real (não placeholder) para os 10 apps de Monitoramento, Processos e Administração, integrando com scheduler, MMU, PCI scanner, ACPI e VFS.

---

## O que já existe (não reescrever)

- `AppMeta` para apps 09–18 já está em `desktop.rs` (`ALL_APPS`)
- `dispatch_app_click(9..=18)` e `dispatch_app_render(9..=18)` já roteiam para `apps_cat2_admin`
- `AdminState` struct já existe com campos para todos os 10 apps
- `ADMIN_STATE: SpinLock<Option<AdminState>>` já existe
- `query_hardware_cpuid()` já existe (CPUID via `__cpuid`)
- `read_cmos_rtc_time()` já existe (RTC ports 0x70/0x71)
- `render_cat2_app` roteador já existe

---

## O que o Agente 2 deve implementar (funcionalidade real)

### App 09 — Task Manager
- [ ] Tab Processos: ler `sched::get_scheduler().get_processes_snapshot()` real, tabela com PID, nome, personalidade (Windows/Linux/Kernel), status
- [ ] Botão [Finalizar Processo] → `sched::terminate_process(pid, code)`
- [ ] Botão [Atualizar] → recarregar snapshot
- [ ] Tab Desempenho: gráficos de CPU, RAM (PMM), context switches usando `sched::system_ticks()` e `mm::get_memory_info()`
- [ ] Tab Serviços: lista de 6 daemons kernel com status Running/Stopped

### App 10 — Resource Monitor
- [ ] 4 osciloscópios circulares em tempo real: CPU, RAM, Disco ATA, Rede E1000
- [ ] Amostrar `sched::get_scheduler().stats()` (context switches, ticks)
- [ ] Amostrar `mm::get_memory_info()` (frames usados/livres)
- [ ] Amostrar `drivers::e1000::GLOBAL_E1000` (RX/TX packets)
- [ ] Histórico circular de 64 amostras (já existe `history_cpu`, `history_ram`, etc.)

### App 11 — Control Panel
- [ ] Grid de 8 categorias vitrificadas (já esboçado)
- [ ] Subpainéis funcionais: Sistema, Rede, Tela, Armazenamento, UAC, etc.
- [ ] Cada categoria abre um subpanel real com controles interativos

### App 12 — Device Manager
- [ ] `PciBusScanner::scan_all()` real — lista dispositivos PCI detectados (Intel 82540EM, ATA, Host Bridge)
- [ ] Árvore hierárquica de dispositivos
- [ ] Painel de propriedades do dispositivo selecionado (vendor ID, device ID, classe)

### App 13 — Services Manager
- [ ] 6 serviços kernel: DHCP, FLTMGR, HDA, Win32k, Scheduler, VFS
- [ ] Botões [Iniciar] / [Parar] / [Reiniciar] que mudam `ServiceLifecycle` (Running/Stopped/Paused)
- [ ] Status visual por serviço

### App 14 — System Information Profiler
- [ ] CPUID real via `query_hardware_cpuid()` (vendor, brand string, SSE/AVX, LM, NX)
- [ ] CR3 via `crate::arch::msr::read_cr3()`
- [ ] HHDM offset via `crate::boot::get_hhdm_offset()`
- [ ] Memória física via `mm::get_memory_info()`
- [ ] Exportar para `C:\sysinfo.txt` via VFS (`vfs_create_file`)

### App 15 — Event & Kernel Log Viewer
- [ ] Logs cronológicos: `event_logs: Vec<KernelLogEntry>` (já existe)
- [ ] Filtros: [Todos | Erros | Avisos | Info]
- [ ] Exportar logs para VFS (`/var/log/kernel.log`)

### App 16 — Registry Editor (regedit)
- [ ] Árvore hierárquica HKLM / HKCU
- [ ] Edição de valores REG_SZ e REG_DWORD
- [ ] Persistência em `/etc/hybridos.conf` (ler/escrever via VFS)

### App 17 — System Configuration (msconfig)
- [ ] Abas: Geral, Boot, Serviços, Inicialização
- [ ] Configurar timeout do Limine (salvar em `limine.conf` via VFS ou variável)
- [ ] Modo de diagnóstico de boot

### App 18 — Power & ACPI Options
- [ ] Planos de energia: High Perf, Balanced, Power Saver
- [ ] [Desligar] → `crate::arch::msr::outb(0xf4, 0x00)` + `crate::arch::acpi::acpi_shutdown()`
- [ ] [Reiniciar] → porta 0xCF9 / 0x64 (reset CPU)
- [ ] [Suspender] → ACPI _S5_ ou sleep state

---

## Interfaces a usar (já existentes no kernel)

- `crate::sched::{get_scheduler, get_processes_snapshot, terminate_process, create_process}`
- `crate::mm::{get_memory_info}`
- `crate::drivers::hardware::{PciBusScanner, PciDevice, HardwarePortIo}`
- `crate::drivers::e1000::GLOBAL_E1000`
- `crate::arch::msr::{read_cr3, read_cr0, rdmsr, outb, inb}`
- `crate::arch::acpi::{acpi_shutdown}`
- `crate::vfs::{vfs_create_file, vfs_read_all, vfs_readdir}`
- `crate::drivers::desktop::{refresh_vfs_cache}`

---

## Critérios de Aceite (Definition of Done)

- [ ] Todos os 10 apps têm estado em `AdminState` (já existe, preencher faltantes)
- [ ] `render_cat2_app` e handlers de clique implementados para todos os 10 apps
- [ ] Task Manager lê processos reais do scheduler (não mock)
- [ ] Device Manager lê PCI real via `PciBusScanner::scan_all()`
- [ ] System Info lê CPUID, CR3, memória real
- [ ] Power Options executa desligamento/ACPI real
- [ ] Sem `todo!()` / `unimplemented!()` / `panic!()` em caminhos normais
- [ ] Compila sem warnings
- [ ] `verify_category_apps(2)` retorna 10 no `main.rs`

---

## Entregável

Commit na sub-branch `feature/fase-d/cat2` com:
1. `apps_cat2_admin.rs` completo
2. `drivers/apps/cat2_admin.rs` (re-export)
3. Comentário: `feat(fase-d): categoria 2 - apps 09-18 funcionais`

---