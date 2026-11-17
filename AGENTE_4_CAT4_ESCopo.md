# AGENTE 4 — Escopo: Categoria 4 (Apps 26 a 33)

**Arquivo principal:** `rust-kernel/src/drivers/apps_cat4_net.rs`  
**Re-export:** `rust-kernel/src/drivers/apps/cat4_net.rs` (já existe)  
**Apps responsáveis:** 26 Network Center, 27 Crystal Web Browser, 28 System Firewall, 29 NetTools, 30 Remote & Serial, 31 Security Center, 32 User Accounts, 33 Crypto Hash

---

## Objetivo

Implementar funcionalidade real (não placeholder) para os 8 apps de Rede, Internet e Segurança, integrando com smoltcp, E1000, firewall HyAV, HDA, CR0.WP/EFER.NXE, VFS e criptografia.

---

## O que já existe (não reescrever)

- `AppMeta` para apps 26–33 já está em `desktop.rs` (`ALL_APPS`)
- `dispatch_app_click(26..=33)` e `dispatch_app_render(26..=33)` já roteiam para `apps_cat4_net`
- `WebTab`, `RuleDir`, `RuleAction`, `FirewallRuleItem`, `UserAccountItem` structs já existem
- `Cat4State` struct já existe com campos para todos os 8 apps
- `CAT4_STATE: SpinLock<Cat4State>` já existe
- `ensure_state_init()` já existe (carrega regras no `global_network_filter()`)
- `parse_ipv4()` já existe
- `refresh_arp_entries()` já existe (lê `GLOBAL_E1000` e `GLOBAL_NET_CONTEXT`)
- `refresh_netstat_entries()` já existe (itera sockets smoltcp)
- `render_cat4_app` roteador já existe

---

## O que o Agente 4 deve implementar (funcionalidade real)

### App 26 — Network & Sharing Center
- [ ] Mapa visual: Este Computador → Roteador (10.0.2.2) → Internet
- [ ] Card do adaptador Intel 82540EM: MAC, IPv4, máscara, gateway, DNS, RX/TX bytes (de `GLOBAL_E1000` + `GLOBAL_NET_CONTEXT`)
- [ ] Botão [Renovar DHCP] → disparar `perform_ping` ou renovar via smoltcp
- [ ] Botão [Propriedades] → modal com BAR0 MMIO, DMA descriptors, MTU
- [ ] Botão [Testar Conectividade] → `perform_ping(10.0.2.2)` real via smoltcp

### App 27 — Crystal Web Browser
- [ ] Tabs (máx 3 visíveis + [+]) com `browser_tabs: Vec<WebTab>`
- [ ] Barra de endereço com suporte `file:///` (ler VFS via `vfs_read_all`) e `http://` (fazer `perform_curl(10.0.2.2, 80, path)`)
- [ ] Renderizador HTML simples (headers, divisores, bullets)
- [ ] Scrollbar funcional (`browser_scroll_y`)
- [ ] Botão [Salvar] → `vfs_create_file("/downloaded_page.html", conteúdo)`

### App 28 — System Firewall & Packet Filter
- [ ] Master toggle (chamar `global_network_filter().set_enabled(true/false)`)
- [ ] Filtros Inbound / Outbound / Both
- [ ] Tabela de 8 regras padrão: HTTP 80 Allow, HTTPS 443 Allow, SSH 22 Block, DNS 53 Allow, ICMP Allow, FTP 21 Block, Telnet 23 Block, Proxy 8080 Block
- [ ] Cores por ação (verde Allow, vermelho Block)
- [ ] Adicionar/remover regras dinamicamente

### App 29 — Network Diagnostics & NetTools
- [ ] 4 abas: Ping, ARP, Netstat, Traceroute
- [ ] Ping: input IP, log de respostas (usar `perform_ping`)
- [ ] ARP: tabela real (de `refresh_arp_entries`)
- [ ] Netstat: tabela real de sockets smoltcp (de `refresh_netstat_entries`)
- [ ] Traceroute: lista de saltos simulada real

### App 30 — Remote & Serial Console Client
- [ ] Modo TCP Raw Socket: conectar, enviar `GET / HTTP/1.1`, receber resposta
- [ ] Modo Serial UART COM1 (0x3F8, 115200 8N1) via `crate::arch::serial::COM1`
- [ ] Dual terminal: alternar entre TCP e Serial

### App 31 — Security & Action Center
- [ ] CR0.WP via `crate::arch::msr::read_cr0()` + checar bit WP
- [ ] EFER.NXE via `crate::arch::msr::rdmsr(IA32_EFER)` + checar bit NXE
- [ ] FLTMGR integridade via `crate::fs::fltmgr::get_fltmgr()`
- [ ] Scan de integridade de páginas críticas
- [ ] Status visual: seguro / aviso / crítico

### App 32 — User Accounts Manager
- [ ] 3 contas: Administrador (root, hash SHA-256 de "password"), hybridos (padrão), guest (travado, hash vazio)
- [ ] Seleção de avatar (ícones visuais)
- [ ] Troca de senha (recalcular SHA-256)

### App 33 — Crypto Hash & Cert Tool
- [ ] Caminho de arquivo no VFS (input)
- [ ] Cálculo MD5 (`crate::security::md5::Md5`) e SHA-256 (`sha256_hex`)
- [ ] Comparador de integridade (digitar hash esperado, comparar)
- [ ] Veredito: CORRESPONDE / NÃO CORRESPONDE

---

## Interfaces a usar (já existentes no kernel)

- `crate::net::smoltcp_bridge::{perform_ping, perform_curl, GLOBAL_NET_CONTEXT}`
- `crate::drivers::e1000::GLOBAL_E1000`
- `crate::net::filter::global_network_filter`
- `crate::arch::serial::COM1`
- `crate::arch::msr::{read_cr0, rdmsr, IA32_EFER, EFER_NXE, CR0_WP}`
- `crate::fs::fltmgr::get_fltmgr`
- `crate::security::md5::Md5`
- `crate::security::sha256::sha256_hex`
- `crate::vfs::{vfs_create_file, vfs_read_all, vfs_readdir}`

---

## Critérios de Aceite (Definition of Done)

- [ ] Todos os 8 apps têm estado em `Cat4State` (já existe, preencher faltantes)
- [ ] Network Center mostra MAC/IP real do E1000
- [ ] Browser faz `perform_curl` real para `http://` e `vfs_read_all` para `file:///`
- [ ] Firewall master toggle funciona via `global_network_filter()`
- [ ] NetTools Ping mostra resultados reais de `perform_ping`
- [ ] Security Center lê CR0.WP e EFER.NXE reais
- [ ] Crypto Hash calcula MD5 e SHA-256 de arquivos VFS
- [ ] Sem `todo!()` / `unimplemented!()` / `panic!()` em caminhos normais
- [ ] Compila sem warnings
- [ ] `verify_category_apps(4)` retorna 8 no `main.rs`

---

## Entregável

Commit na sub-branch `feature/fase-d/cat4` com:
1. `apps_cat4_net.rs` completo
2. Comentário: `feat(fase-d): categoria 4 - apps 26-33 funcionais`

---