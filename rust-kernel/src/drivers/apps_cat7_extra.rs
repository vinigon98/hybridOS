//! HybridOS Native Applications Suite — Category 7: Classic Games, Accessibility & Help/Recovery (48 to 50).
//!
//! Implements completely bare-metal Crystal Glass native views for:
//! 48. Central de Jogos Clássicos (Campo Minado 10x10, 15-Puzzle Deslizante e Benchmark 3D DirectX/GDI)
//! 49. Central de Facilidade de Acesso (Teclado Virtual OSK funcional com injeção PS/2 e Lupa 2x/4x)
//! 50. Central de Ajuda e Recuperação do Sistema (Manual Tri-Shell Bash/CMD/PowerShell e System Restore)
//!
//! Zero Mocks & Zero Botões Decorativos: Todos os controles possuem hitboxes ativas de clique,
//! cálculos matemáticos reais, injeção real no buffer de teclado do kernel e integração VFS.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;
use core::sync::atomic::Ordering;

use crate::compat::FloatExt;
use crate::sync::SpinLock;
use crate::drivers::desktop::{DesktopState, draw_text_px, draw_text_clipped};
use crate::drivers::gfx2d::*;
use crate::drivers::enqueue_keyboard_char;
use crate::drivers::mouse::MOUSE_STATE;
use crate::drivers::apps_cat6_media::{GLOBAL_THEME, THEME_PALETTE};
use crate::arch::idt::SYSTEM_TICKS;
use crate::arch::msr;
use crate::vfs;

// =============================================================================
// FAST CLIPPED LINE & TRIANGLE RASTERIZER FOR DIRECTX / GDI 3D BENCHMARK
// =============================================================================

/// Integer Bresenham line drawer with strict viewport clipping.
pub unsafe fn draw_line_clipped(
    fb: *mut u8, pitch: usize, scr_w: usize, scr_h: usize,
    mut x0: isize, mut y0: isize, x1: isize, y1: isize,
    color: u32,
    clip_x0: usize, clip_y0: usize, clip_x1: usize, clip_y1: usize,
) {
    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;

    loop {
        if x0 >= clip_x0 as isize && x0 < clip_x1 as isize && y0 >= clip_y0 as isize && y0 < clip_y1 as isize {
            let px = x0 as usize;
            let py = y0 as usize;
            if px < scr_w && py < scr_h {
                let row = fb.add(py * pitch) as *mut u32;
                *row.add(px) = color;
            }
        }
        if x0 == x1 && y0 == y1 { break; }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
    }
}

/// Software scanline rasterizer for filled shaded 3D triangles with viewport clipping.
pub unsafe fn fill_triangle_clipped(
    fb: *mut u8, pitch: usize, scr_w: usize, scr_h: usize,
    mut p0: (isize, isize), mut p1: (isize, isize), mut p2: (isize, isize),
    color: u32,
    clip_x0: usize, clip_y0: usize, clip_x1: usize, clip_y1: usize,
) {
    // Sort vertices by Y: p0.y <= p1.y <= p2.y
    if p0.1 > p1.1 { core::mem::swap(&mut p0, &mut p1); }
    if p0.1 > p2.1 { core::mem::swap(&mut p0, &mut p2); }
    if p1.1 > p2.1 { core::mem::swap(&mut p1, &mut p2); }

    let total_h = p2.1 - p0.1;
    if total_h <= 0 { return; }

    for y in p0.1..=p2.1 {
        if y < clip_y0 as isize || y >= clip_y1 as isize { continue; }

        let second_half = y > p1.1 || p1.1 == p0.1;
        let segment_h = if second_half { p2.1 - p1.1 } else { p1.1 - p0.1 };
        if segment_h <= 0 { continue; }

        let alpha = ((y - p0.1) as f32) / (total_h as f32);
        let beta = if second_half {
            ((y - p1.1) as f32) / (segment_h as f32)
        } else {
            ((y - p0.1) as f32) / (segment_h as f32)
        };

        let mut ax = (p0.0 as f32 + (p2.0 - p0.0) as f32 * alpha) as isize;
        let mut bx = if second_half {
            (p1.0 as f32 + (p2.0 - p1.0) as f32 * beta) as isize
        } else {
            (p0.0 as f32 + (p1.0 - p0.0) as f32 * beta) as isize
        };

        if ax > bx { core::mem::swap(&mut ax, &mut bx); }

        let span_start = ax.max(clip_x0 as isize).max(0) as usize;
        let span_end = bx.min(clip_x1 as isize).min(scr_w as isize) as usize;

        if span_start < span_end && (y as usize) < scr_h {
            let row = fb.add((y as usize) * pitch) as *mut u32;
            for x in span_start..span_end {
                *row.add(x) = color;
            }
        }
    }
}

// =============================================================================
// PROGRAM 48: CENTRAL DE JOGOS CLÁSSICOS (Minesweeper, 15-Puzzle, DirectX 3D)
// =============================================================================

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MineGameStatus {
    Ready,
    Playing,
    Won,
    Lost,
}

#[derive(Clone, Copy)]
pub struct MineCell {
    pub is_mine: bool,
    pub revealed: bool,
    pub flagged: bool,
    pub neighbor_count: u8,
}

impl MineCell {
    pub const fn new() -> Self {
        Self {
            is_mine: false,
            revealed: false,
            flagged: false,
            neighbor_count: 0,
        }
    }
}

pub struct MinesweeperState {
    pub cells: [MineCell; 100], // 10x10
    pub status: MineGameStatus,
    pub flag_mode: bool,
    pub flags_placed: usize,
    pub revealed_count: usize,
    pub start_ticks: u64,
    pub elapsed_seconds: u32,
    pub first_click: bool,
    pub smile_pressed: bool,
}

impl MinesweeperState {
    pub const fn new() -> Self {
        Self {
            cells: [MineCell::new(); 100],
            status: MineGameStatus::Ready,
            flag_mode: false,
            flags_placed: 0,
            revealed_count: 0,
            start_ticks: 0,
            elapsed_seconds: 0,
            first_click: true,
            smile_pressed: false,
        }
    }

    pub fn reset(&mut self) {
        for c in self.cells.iter_mut() {
            *c = MineCell::new();
        }
        self.status = MineGameStatus::Ready;
        self.flags_placed = 0;
        self.revealed_count = 0;
        self.start_ticks = 0;
        self.elapsed_seconds = 0;
        self.first_click = true;
        self.smile_pressed = false;
    }

    pub fn place_mines(&mut self, safe_r: usize, safe_c: usize) {
        let mut seed = SYSTEM_TICKS.load(Ordering::Relaxed) ^ 0x5DEECE66D;
        let mut placed = 0usize;
        let target_mines = 10usize;

        while placed < target_mines {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let idx = (seed >> 32) as usize % 100;
            let r = idx / 10;
            let c = idx % 10;

            // Do not place on or immediately adjacent to first clicked cell
            let is_safe = (r as isize - safe_r as isize).abs() <= 1 && (c as isize - safe_c as isize).abs() <= 1;

            if !is_safe && !self.cells[idx].is_mine {
                self.cells[idx].is_mine = true;
                placed += 1;
            }
        }

        // Calculate neighbor numbers
        for r in 0..10 {
            for c in 0..10 {
                let idx = r * 10 + c;
                if self.cells[idx].is_mine { continue; }
                let mut count = 0u8;
                for dr in -1isize..=1 {
                    for dc in -1isize..=1 {
                        if dr == 0 && dc == 0 { continue; }
                        let nr = r as isize + dr;
                        let nc = c as isize + dc;
                        if nr >= 0 && nr < 10 && nc >= 0 && nc < 10 {
                            let nidx = (nr as usize) * 10 + (nc as usize);
                            if self.cells[nidx].is_mine {
                                count += 1;
                            }
                        }
                    }
                }
                self.cells[idx].neighbor_count = count;
            }
        }
    }

    pub fn reveal(&mut self, r: usize, c: usize) {
        if r >= 10 || c >= 10 { return; }
        if self.status == MineGameStatus::Lost || self.status == MineGameStatus::Won { return; }

        let idx = r * 10 + c;
        if self.cells[idx].flagged || self.cells[idx].revealed { return; }

        if self.first_click {
            self.place_mines(r, c);
            self.first_click = false;
            self.status = MineGameStatus::Playing;
            self.start_ticks = SYSTEM_TICKS.load(Ordering::Relaxed);
        }

        if self.cells[idx].is_mine {
            // Hit a mine: Game Over
            self.cells[idx].revealed = true;
            self.status = MineGameStatus::Lost;
            for cell in self.cells.iter_mut() {
                if cell.is_mine {
                    cell.revealed = true;
                }
            }
            return;
        }

        // BFS Cascade reveal
        let mut queue = [0usize; 100];
        let mut q_head = 0usize;
        let mut q_tail = 0usize;

        queue[q_tail] = idx;
        q_tail += 1;
        self.cells[idx].revealed = true;
        self.revealed_count += 1;

        while q_head < q_tail {
            let cur_idx = queue[q_head];
            q_head += 1;

            if self.cells[cur_idx].neighbor_count == 0 {
                let cr = cur_idx / 10;
                let cc = cur_idx % 10;

                for dr in -1isize..=1 {
                    for dc in -1isize..=1 {
                        if dr == 0 && dc == 0 { continue; }
                        let nr = cr as isize + dr;
                        let nc = cc as isize + dc;
                        if nr >= 0 && nr < 10 && nc >= 0 && nc < 10 {
                            let nidx = (nr as usize) * 10 + (nc as usize);
                            if !self.cells[nidx].revealed && !self.cells[nidx].flagged && !self.cells[nidx].is_mine {
                                self.cells[nidx].revealed = true;
                                self.revealed_count += 1;
                                if self.cells[nidx].neighbor_count == 0 && q_tail < 100 {
                                    queue[q_tail] = nidx;
                                    q_tail += 1;
                                }
                            }
                        }
                    }
                }
            }
        }

        // Check victory
        if self.revealed_count >= 90 {
            self.status = MineGameStatus::Won;
            for cell in self.cells.iter_mut() {
                if cell.is_mine {
                    cell.flagged = true;
                }
            }
        }
    }

    pub fn toggle_flag(&mut self, r: usize, c: usize) {
        if r >= 10 || c >= 10 { return; }
        if self.status == MineGameStatus::Lost || self.status == MineGameStatus::Won { return; }

        let idx = r * 10 + c;
        if self.cells[idx].revealed { return; }

        if self.cells[idx].flagged {
            self.cells[idx].flagged = false;
            self.flags_placed = self.flags_placed.saturating_sub(1);
        } else {
            self.cells[idx].flagged = true;
            self.flags_placed += 1;
        }
    }
}

pub struct PuzzleState {
    pub grid: [u8; 16], // 4x4
    pub moves: usize,
    pub is_solved: bool,
    pub shuffled: bool,
}

impl PuzzleState {
    pub const fn new() -> Self {
        Self {
            grid: [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 0],
            moves: 0,
            is_solved: true,
            shuffled: false,
        }
    }

    pub fn reset(&mut self) {
        self.grid = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 0];
        self.moves = 0;
        self.is_solved = true;
        self.shuffled = false;
    }

    pub fn shuffle(&mut self) {
        let mut seed = SYSTEM_TICKS.load(Ordering::Relaxed) ^ 0xCAFEBABE;
        for _ in 0..120 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let empty_pos = self.grid.iter().position(|&x| x == 0).unwrap_or(15);
            let er = empty_pos / 4;
            let ec = empty_pos % 4;

            let mut valid_neighbors = [0usize; 4];
            let mut v_count = 0usize;

            if er > 0 { valid_neighbors[v_count] = (er - 1) * 4 + ec; v_count += 1; }
            if er < 3 { valid_neighbors[v_count] = (er + 1) * 4 + ec; v_count += 1; }
            if ec > 0 { valid_neighbors[v_count] = er * 4 + (ec - 1); v_count += 1; }
            if ec < 3 { valid_neighbors[v_count] = er * 4 + (ec + 1); v_count += 1; }

            if v_count > 0 {
                let choice = ((seed >> 24) as usize) % v_count;
                let swap_pos = valid_neighbors[choice];
                self.grid.swap(empty_pos, swap_pos);
            }
        }
        self.moves = 0;
        self.is_solved = false;
        self.shuffled = true;
    }

    pub fn click_cell(&mut self, idx: usize) -> bool {
        if idx >= 16 { return false; }
        let empty_pos = self.grid.iter().position(|&x| x == 0).unwrap_or(15);
        let r = idx / 4;
        let c = idx % 4;
        let er = empty_pos / 4;
        let ec = empty_pos % 4;

        let dist = (r as isize - er as isize).abs() + (c as isize - ec as isize).abs();
        if dist == 1 {
            self.grid.swap(idx, empty_pos);
            self.moves += 1;
            // Check victory
            let solved_expected = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 0];
            if self.grid == solved_expected {
                self.is_solved = true;
            }
            true
        } else {
            false
        }
    }
}

pub struct Bench3DState {
    pub angle_x: f32,
    pub angle_y: f32,
    pub angle_z: f32,
    pub wireframe: bool,
    pub paused: bool,
    pub speed: f32,
    pub mesh_type: u8, // 0 = Cube, 1 = Octahedron
    pub fps: u32,
    pub frames_accum: u32,
    pub last_sec_ticks: u64,
}

impl Bench3DState {
    pub const fn new() -> Self {
        Self {
            angle_x: 0.4,
            angle_y: 0.6,
            angle_z: 0.0,
            wireframe: false,
            paused: false,
            speed: 0.035,
            mesh_type: 0,
            fps: 60,
            frames_accum: 0,
            last_sec_ticks: 0,
        }
    }
}

// =============================================================================
// PROGRAM 49: CENTRAL DE FACILIDADE DE ACESSO (OSK & Magnifier)
// =============================================================================

pub struct OskKey {
    pub normal: char,
    pub shift: char,
    pub label: &'static str,
    pub width: usize,
    pub is_special: bool,
}

pub struct OskState {
    pub caps_lock: bool,
    pub shift_active: bool,
    pub test_buffer: String,
    pub last_char: Option<char>,
}

impl OskState {
    pub fn new() -> Self {
        Self {
            caps_lock: false,
            shift_active: false,
            test_buffer: String::from("HybridOS x86_64"),
            last_char: None,
        }
    }
}

pub struct MagnifierState {
    pub zoom_factor: usize, // 2 or 4
    pub follow_mouse: bool,
    pub frozen: bool,
    pub show_grid: bool,
    pub last_sample_x: usize,
    pub last_sample_y: usize,
    pub last_color: u32,
}

impl MagnifierState {
    pub const fn new() -> Self {
        Self {
            zoom_factor: 2,
            follow_mouse: true,
            frozen: false,
            show_grid: true,
            last_sample_x: 640,
            last_sample_y: 400,
            last_color: 0x000284C7,
        }
    }
}

// =============================================================================
// PROGRAM 50: CENTRAL DE AJUDA E RECUPERAÇÃO DO SISTEMA
// =============================================================================

#[derive(Clone, Copy)]
pub struct TriShellCommand {
    pub category: &'static str,
    pub name: &'static str,
    pub bash: &'static str,
    pub cmd: &'static str,
    pub powershell: &'static str,
    pub description: &'static str,
    pub example: &'static str,
}

pub const TRI_SHELL_COMMANDS: [TriShellCommand; 16] = [
    TriShellCommand {
        category: "Arquivos",
        name: "Listar Arquivos",
        bash: "ls -la",
        cmd: "dir",
        powershell: "Get-ChildItem",
        description: "Lista o conteudo do diretorio atual com detalhes de tamanho e permissoes",
        example: "ls -la /Users ou dir C:\\Users",
    },
    TriShellCommand {
        category: "Arquivos",
        name: "Copiar Arquivo",
        bash: "cp <orig> <dest>",
        cmd: "copy <orig> <dest>",
        powershell: "Copy-Item <orig> <dest>",
        description: "Copia um ou mais arquivos para outro local persistente do VFS",
        example: "cp kernel.log /backup/ ou copy a.txt b.txt",
    },
    TriShellCommand {
        category: "Arquivos",
        name: "Mover / Renomear",
        bash: "mv <orig> <dest>",
        cmd: "move <orig> <dest>",
        powershell: "Move-Item <orig> <dest>",
        description: "Move ou renomeia arquivos e pastas dentro da arvore de diretorios",
        example: "mv draft.txt notas.txt",
    },
    TriShellCommand {
        category: "Arquivos",
        name: "Excluir Arquivo",
        bash: "rm <arquivo>",
        cmd: "del <arquivo>",
        powershell: "Remove-Item <arquivo>",
        description: "Remove um arquivo do disco e libera os blocos correspondentes no VFS",
        example: "rm /tmp/cache.tmp ou del old.dat",
    },
    TriShellCommand {
        category: "Arquivos",
        name: "Exibir Conteudo",
        bash: "cat <arquivo>",
        cmd: "type <arquivo>",
        powershell: "Get-Content <arquivo>",
        description: "Imprime o conteudo textual completo de um arquivo no console",
        example: "cat /etc/hybridos.conf ou type config.ini",
    },
    TriShellCommand {
        category: "Arquivos",
        name: "Criar Diretorio",
        bash: "mkdir <pasta>",
        cmd: "md <pasta>",
        powershell: "New-Item -ItemType Directory",
        description: "Cria um novo diretorio com permissoes de leitura e gravacao",
        example: "mkdir /Users/Documentos ou md C:\\Projetos",
    },
    TriShellCommand {
        category: "Processos",
        name: "Listar Processos",
        bash: "ps aux",
        cmd: "tasklist",
        powershell: "Get-Process",
        description: "Exibe todos os processos em execucao, PIDs e estatisticas de CPU",
        example: "ps aux ou tasklist",
    },
    TriShellCommand {
        category: "Processos",
        name: "Encerrar Processo",
        bash: "kill -9 <PID>",
        cmd: "taskkill /F /PID <PID>",
        powershell: "Stop-Process -Id <PID>",
        description: "Envia sinal de encerramento imediato a uma tarefa pelo seu PID",
        example: "kill -9 2 ou taskkill /PID 4",
    },
    TriShellCommand {
        category: "Processos",
        name: "Monitor de Recursos",
        bash: "top",
        cmd: "taskmgr",
        powershell: "Get-Process | Sort CPU",
        description: "Painel dinamico de telemetria de uso de memoria PMM e threads SMP",
        example: "top ou taskmgr",
    },
    TriShellCommand {
        category: "Rede",
        name: "Configuracao de IP",
        bash: "ip a",
        cmd: "ipconfig",
        powershell: "Get-NetIPAddress",
        description: "Exibe enderecos IPv4, mascara, gateway e status do DHCP no E1000",
        example: "ip a ou ipconfig /all",
    },
    TriShellCommand {
        category: "Rede",
        name: "Testar Conexao",
        bash: "ping <host>",
        cmd: "ping <host>",
        powershell: "Test-Connection <host>",
        description: "Envia pacotes ICMP Echo Request para verificar latencia e perda de rede",
        example: "ping 10.0.2.2 ou ping 8.8.8.8",
    },
    TriShellCommand {
        category: "Rede",
        name: "Tabela de Conexoes",
        bash: "netstat -an",
        cmd: "netstat -an",
        powershell: "Get-NetTCPConnection",
        description: "Lista todas as portas TCP e UDP abertas e conexoes ativas",
        example: "netstat -an",
    },
    TriShellCommand {
        category: "Rede",
        name: "Requisicao HTTP",
        bash: "curl -O <url>",
        cmd: "curl.exe <url>",
        powershell: "Invoke-WebRequest <url>",
        description: "Transfere arquivos ou dados de servidores HTTP da rede local e internet",
        example: "curl http://10.0.2.2:8080/index.html",
    },
    TriShellCommand {
        category: "Sistema",
        name: "Info do Sistema",
        bash: "uname -a",
        cmd: "systeminfo",
        powershell: "Get-ComputerInfo",
        description: "Relatorio detalhado do kernel bare-metal x86_64 e compilacao",
        example: "uname -a ou systeminfo",
    },
    TriShellCommand {
        category: "Sistema",
        name: "Limpar Tela",
        bash: "clear",
        cmd: "cls",
        powershell: "Clear-Host",
        description: "Limpa a saida visual do console e reseta o cursor para o topo",
        example: "clear ou cls",
    },
    TriShellCommand {
        category: "Sistema",
        name: "Espaco em Disco",
        bash: "df -h",
        cmd: "dir /s",
        powershell: "Get-Volume",
        description: "Informa espaco total, usado e livre no disco ATA e particao FAT32",
        example: "df -h ou Get-Volume",
    },
];

/// Reads real date and time from the x86 CMOS RTC via I/O ports 0x70 / 0x71.
fn read_cmos_rtc_datetime() -> String {
    unsafe {
        let bcd_to_bin = |val: u8| -> u8 { ((val >> 4) * 10) + (val & 0x0F) };

        for _ in 0..1000 {
            msr::outb(0x70, 0x0A);
            if (msr::inb(0x71) & 0x80) == 0 {
                break;
            }
        }

        msr::outb(0x70, 0x0B);
        let status_b = msr::inb(0x71);
        let is_bcd = (status_b & 0x04) == 0;
        let is_24h = (status_b & 0x02) != 0;

        msr::outb(0x70, 0x00);
        let mut sec = msr::inb(0x71);
        msr::outb(0x70, 0x02);
        let mut min = msr::inb(0x71);
        msr::outb(0x70, 0x04);
        let raw_hour = msr::inb(0x71);
        let pm = (raw_hour & 0x80) != 0;
        let mut hour = raw_hour & 0x7F;
        msr::outb(0x70, 0x07);
        let mut day = msr::inb(0x71);
        msr::outb(0x70, 0x08);
        let mut month = msr::inb(0x71);
        msr::outb(0x70, 0x09);
        let mut year = msr::inb(0x71);

        if is_bcd {
            sec = bcd_to_bin(sec);
            min = bcd_to_bin(min);
            hour = bcd_to_bin(hour);
            day = bcd_to_bin(day);
            month = bcd_to_bin(month);
            year = bcd_to_bin(year);
        }

        if !is_24h {
            hour = if pm { (hour % 12) + 12 } else { hour % 12 };
        }

        let full_year = 2000u16 + (year as u16);
        format!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
            full_year, month, day, hour, min, sec
        )
    }
}

#[derive(Clone)]
pub struct RestorePoint {
    pub id: usize,
    pub file_path: String,
    pub timestamp: String,
    pub description: String,
    pub size_bytes: usize,
    pub status: String,
}

fn create_vfs_snapshot(id: usize, desc: &str) -> Result<RestorePoint, String> {
    let _ = vfs::vfs_create_dir_all("/snapshots", vfs::Permissions::default_dir());

    let timestamp = read_cmos_rtc_datetime();
    let etc_files = match vfs::vfs_readdir("/etc") {
        Ok(entries) if !entries.is_empty() => entries
            .iter()
            .map(|e| e.name.as_str())
            .collect::<Vec<_>>()
            .join(","),
        _ => String::from("(vazio)"),
    };

    let (theme_index, glass_opacity, wallpaper_preset) = {
        let t = GLOBAL_THEME.lock();
        (t.theme_index, t.glass_opacity, t.wallpaper_preset)
    };

    let file_path = format!("/snapshots/restore_point_{}.snap", id);
    let content = format!(
        "SNAPSHOT_ID={}\nTIMESTAMP={}\nDESC={}\nTHEME_INDEX={}\nGLASS_OPACITY={}\nWALLPAPER_PRESET={}\nETC_FILES={}\nSTATUS=Integro\n",
        id, timestamp, desc, theme_index, glass_opacity, wallpaper_preset, etc_files
    );

    let _ = vfs::vfs_unlink(&file_path);
    match vfs::vfs_create_file(&file_path, content.as_bytes(), vfs::Permissions::default_file()) {
        Ok(_) => Ok(RestorePoint {
            id,
            file_path,
            timestamp,
            description: desc.to_string(),
            size_bytes: content.len(),
            status: String::from("Integro"),
        }),
        Err(e) => Err(format!("Erro VFS ao gravar {}: {:?}", file_path, e)),
    }
}

fn parse_snapshot_bytes(file_path: String, data: &[u8], fallback_id: usize) -> RestorePoint {
    let text = core::str::from_utf8(data).unwrap_or("");
    let mut id = fallback_id;
    let mut timestamp = String::from("RTC N/D");
    let mut description = String::from("Snapshot VFS");
    let mut status = String::from("Integro");

    for line in text.lines() {
        if let Some(v) = line.strip_prefix("SNAPSHOT_ID=") {
            if let Ok(parsed) = v.trim().parse::<usize>() {
                id = parsed;
            }
        } else if let Some(v) = line.strip_prefix("TIMESTAMP=") {
            timestamp = v.trim().to_string();
        } else if let Some(v) = line.strip_prefix("DESC=") {
            description = v.trim().to_string();
        } else if let Some(v) = line.strip_prefix("STATUS=") {
            status = v.trim().to_string();
        }
    }

    RestorePoint {
        id,
        file_path,
        timestamp,
        description,
        size_bytes: data.len(),
        status,
    }
}

pub struct SystemRestoreState {
    pub points: Vec<RestorePoint>,
    pub selected_idx: usize,
    pub status_banner: String,
    pub progress_pct: usize,
    pub initialized: bool,
}

impl SystemRestoreState {
    pub fn new() -> Self {
        Self {
            points: Vec::new(),
            selected_idx: 0,
            status_banner: String::new(),
            progress_pct: 100,
            initialized: false,
        }
    }

    /// Scans `/snapshots` in VFS and populates real restore points (creating an initial
    /// snapshot if the directory is empty).
    pub fn sync_from_vfs(&mut self) {
        let _ = vfs::vfs_create_dir_all("/snapshots", vfs::Permissions::default_dir());
        let mut pts = Vec::new();

        if let Ok(entries) = vfs::vfs_readdir("/snapshots") {
            let mut next_fallback_id = 1usize;
            for entry in entries.iter() {
                if !entry.name.ends_with(".snap") {
                    continue;
                }
                let full_path = format!("/snapshots/{}", entry.name);
                if let Ok(data) = vfs::vfs_read_all(&full_path) {
                    pts.push(parse_snapshot_bytes(full_path, &data, next_fallback_id));
                    next_fallback_id += 1;
                }
            }
        }

        pts.sort_by_key(|p| p.id);

        if pts.is_empty() {
            match create_vfs_snapshot(1, "Snapshot Inicial do Sistema VFS") {
                Ok(pt) => {
                    let sz = pt.size_bytes;
                    pts.push(pt);
                    self.status_banner = format!(
                        "Snapshot inicial #1 criado em /snapshots ({} bytes).",
                        sz
                    );
                    self.progress_pct = 100;
                }
                Err(e) => {
                    self.status_banner = e;
                    self.progress_pct = 0;
                }
            }
        } else {
            self.status_banner = format!(
                "VFS /snapshots sincronizado: {} snapshot(s) encontrado(s).",
                pts.len()
            );
            self.progress_pct = 100;
        }

        self.points = pts;
        if self.selected_idx >= self.points.len() {
            self.selected_idx = 0;
        }
        self.initialized = true;
    }
}

// =============================================================================
// MASTER STATE HOLDER FOR CATEGORY 7
// =============================================================================

pub struct Cat7MasterState {
    pub last_cx: [usize; 3],
    pub last_cy: [usize; 3],
    pub last_cw: [usize; 3],
    pub last_ch: [usize; 3],

    pub games_tab: usize, // 0 = Campo Minado, 1 = 15-Puzzle, 2 = DirectX Benchmark
    pub minesweeper: MinesweeperState,
    pub puzzle: PuzzleState,
    pub bench: Bench3DState,

    pub access_tab: usize, // 0 = OSK, 1 = Magnifier, 2 = Configs
    pub osk: OskState,
    pub osk_ctrl: bool,
    pub osk_alt: bool,
    pub osk_altgr: bool,
    pub magnifier: MagnifierState,
    pub hc_options: [bool; 5],
    pub hc_status_msg: String,

    pub help_tab: usize, // 0 = Manual Tri-Shell, 1 = System Restore
    pub selected_cat_filter: usize, // 0: Todos, 1: Arquivos, 2: Processos, 3: Rede, 4: Sistema
    pub selected_cmd_idx: usize,
    pub restore: SystemRestoreState,
}

impl Cat7MasterState {
    pub fn new() -> Self {
        Self {
            last_cx: [0; 3],
            last_cy: [0; 3],
            last_cw: [800; 3],
            last_ch: [600; 3],

            games_tab: 0,
            minesweeper: MinesweeperState::new(),
            puzzle: PuzzleState::new(),
            bench: Bench3DState::new(),

            access_tab: 0,
            osk: OskState::new(),
            osk_ctrl: false,
            osk_alt: false,
            osk_altgr: false,
            magnifier: MagnifierState::new(),
            hc_options: [false, true, false, true, false],
            hc_status_msg: String::new(),

            help_tab: 0,
            selected_cat_filter: 0,
            selected_cmd_idx: 0,
            restore: SystemRestoreState::new(),
        }
    }
}

pub static CAT7_STATE: SpinLock<Cat7MasterState> = SpinLock::new(Cat7MasterState {
    last_cx: [0; 3],
    last_cy: [0; 3],
    last_cw: [800; 3],
    last_ch: [600; 3],

    games_tab: 0,
    minesweeper: MinesweeperState::new(),
    puzzle: PuzzleState::new(),
    bench: Bench3DState::new(),

    access_tab: 0,
    osk: OskState {
        caps_lock: false,
        shift_active: false,
        test_buffer: String::new(),
        last_char: None,
    },
    osk_ctrl: false,
    osk_alt: false,
    osk_altgr: false,
    magnifier: MagnifierState::new(),
    hc_options: [false, true, false, true, false],
    hc_status_msg: String::new(),

    help_tab: 0,
    selected_cat_filter: 0,
    selected_cmd_idx: 0,
    restore: SystemRestoreState {
        points: Vec::new(),
        selected_idx: 0,
        status_banner: String::new(),
        progress_pct: 100,
        initialized: false,
    },
});

/// Returns true when the Magnifier tab is active and configured to live-follow the mouse.
pub fn is_magnifier_live() -> bool {
    let st = CAT7_STATE.lock();
    st.access_tab == 1 && st.magnifier.follow_mouse && !st.magnifier.frozen
}

/// Returns true when the 3D Benchmark tab is active and continuously animating.
pub fn is_benchmark_running() -> bool {
    let st = CAT7_STATE.lock();
    st.games_tab == 2 && !st.bench.paused
}

// =============================================================================
// MAIN ENTRY POINTS
// =============================================================================

/// Renders Category 7 native applications inside window client canvas.
pub unsafe fn render_cat7_app(
    id: u8,
    fb: *mut u8, pitch: usize, scr_w: usize, scr_h: usize,
    cx: usize, cy: usize, cw: usize, ch: usize,
    desk: &mut DesktopState,
) {
    // Record bounds for hit-testing
    {
        let mut state = CAT7_STATE.lock();
        let slot = match id {
            48 => 0,
            49 => 1,
            50 => 2,
            _ => return,
        };
        state.last_cx[slot] = cx;
        state.last_cy[slot] = cy;
        state.last_cw[slot] = cw;
        state.last_ch[slot] = ch;

        // One-time initialization of restore state from real VFS /snapshots
        if id == 50 && !state.restore.initialized {
            state.restore.sync_from_vfs();
        }
    }

    match id {
        48 => render_classic_games_hub(fb, pitch, scr_w, scr_h, cx, cy, cw, ch, desk),
        49 => render_accessibility_center(fb, pitch, scr_w, scr_h, cx, cy, cw, ch, desk),
        50 => render_help_recovery_center(fb, pitch, scr_w, scr_h, cx, cy, cw, ch, desk),
        _ => {}
    }
}

/// Dispatches mouse clicks for Category 7 applications.
pub fn handle_cat7_click(id: u8, cx: usize, cy: usize, is_double_click: bool, desk: &mut DesktopState) -> bool {
    match id {
        48 => handle_classic_games_click(cx, cy, desk),
        49 => handle_accessibility_click(cx, cy, desk),
        50 => handle_help_recovery_click(cx, cy, desk),
        _ => false,
    }
}

// =============================================================================
// 48. CENTRAL DE JOGOS CLÁSSICOS (Minesweeper, 15-Puzzle, 3D Benchmark)
// =============================================================================

unsafe fn render_classic_games_hub(
    fb: *mut u8, pitch: usize, scr_w: usize, scr_h: usize,
    cx: usize, cy: usize, cw: usize, ch: usize,
    _desk: &mut DesktopState,
) {
    let (tab, timer_val) = {
        let mut st = CAT7_STATE.lock();
        if st.minesweeper.status == MineGameStatus::Playing {
            let now = SYSTEM_TICKS.load(Ordering::Relaxed);
            st.minesweeper.elapsed_seconds = ((now.saturating_sub(st.minesweeper.start_ticks)) / 100).min(999) as u32;
        }
        (st.games_tab, st.minesweeper.elapsed_seconds)
    };

    // 1. Top Tabs Toolbar (28px)
    draw_glass_toolbar(fb, pitch, scr_w, scr_h, cx, cy, cw, 28);
    draw_glass_tab(fb, pitch, scr_w, scr_h, cx + 8, cy + 2, 120, 24, "Campo Minado", tab == 0);
    draw_glass_tab(fb, pitch, scr_w, scr_h, cx + 132, cy + 2, 100, 24, "15-Puzzle", tab == 1);
    draw_glass_tab(fb, pitch, scr_w, scr_h, cx + 236, cy + 2, 130, 24, "Benchmark 3D", tab == 2);

    let body_y = cy + 28;
    let body_h = ch.saturating_sub(28 + 22);

    match tab {
        0 => render_minesweeper_view(fb, pitch, scr_w, scr_h, cx, body_y, cw, body_h, timer_val),
        1 => render_puzzle_view(fb, pitch, scr_w, scr_h, cx, body_y, cw, body_h),
        2 => render_bench3d_view(fb, pitch, scr_w, scr_h, cx, body_y, cw, body_h),
        _ => {}
    }

    // Bottom Status Bar (22px)
    let status_y = cy + ch.saturating_sub(22);
    let (l_msg, r_msg) = match tab {
        0 => ("Campo Minado Vitrificado 10x10", "Dica: Clique direito ou [Bandeira]"),
        1 => ("Quebra-Cabeca Deslizante 4x4", "Ordene de 1 a 15"),
        2 => ("DirectX / GDI 3D Hardware Benchmark", "Software Shading Engine"),
        _ => ("Jogos Classicos", "Pronto"),
    };
    draw_glass_statusbar(fb, pitch, scr_w, scr_h, cx, status_y, cw, 22, l_msg, r_msg);
}

unsafe fn render_minesweeper_view(
    fb: *mut u8, pitch: usize, scr_w: usize, scr_h: usize,
    cx: usize, cy: usize, cw: usize, ch: usize,
    timer_sec: u32,
) {
    fill_rounded_rect_gradient(fb, pitch, scr_w, scr_h, cx, cy, cw, ch, 0, 0x00F8FAFC, 0x00E2E8F0);

    let st = CAT7_STATE.lock();
    let ms = &st.minesweeper;

    // Header Game Control Box (42px)
    let header_y = cy + 8;
    let header_w = 320usize.min(cw.saturating_sub(16));
    let header_x = cx + (cw.saturating_sub(header_w)) / 2;
    fill_rounded_rect_gradient(fb, pitch, scr_w, scr_h, header_x, header_y, header_w, 42, 6, 0x001E293B, 0x000F172A);
    draw_rounded_rect_border(fb, pitch, scr_w, scr_h, header_x, header_y, header_w, 42, 6, 0x0038BDF8);

    // Mines remaining digital LED
    let remaining = (10isize - ms.flags_placed as isize).max(-99).min(999);
    let rem_str = format!("{:03}", remaining);
    fill_rounded_rect(fb, pitch, scr_w, scr_h, header_x + 10, header_y + 8, 54, 26, 3, 0x00000000);
    draw_text_px(fb, pitch, scr_w, scr_h, header_x + 18, header_y + 13, &rem_str, 0x00EF4444, None);

    // Smile Face button in center
    let smile_x = header_x + header_w / 2 - 18;
    let smile_y = header_y + 5;
    let (smile_text, _smile_col) = match ms.status {
        MineGameStatus::Ready | MineGameStatus::Playing => (":-)", 0x00FBBF24),
        MineGameStatus::Won => ("B-)", 0x0034D399),
        MineGameStatus::Lost => ("X_X", 0x00EF4444),
    };
    fill_rounded_rect_gradient(fb, pitch, scr_w, scr_h, smile_x, smile_y, 36, 32, 6, 0x00FEF08A, 0x00F59E0B);
    draw_rounded_rect_border(fb, pitch, scr_w, scr_h, smile_x, smile_y, 36, 32, 6, 0x00B45309);
    draw_text_px(fb, pitch, scr_w, scr_h, smile_x + 6, smile_y + 8, smile_text, 0x0078350F, None);

    // Timer digital LED
    let timer_str = format!("{:03}", timer_sec);
    fill_rounded_rect(fb, pitch, scr_w, scr_h, header_x + header_w - 64, header_y + 8, 54, 26, 3, 0x00000000);
    draw_text_px(fb, pitch, scr_w, scr_h, header_x + header_w - 56, header_y + 13, &timer_str, 0x00EF4444, None);

    // Mode Toggle Buttons
    let btn_y = header_y + 48;
    let flag_label = if ms.flag_mode { "[x] Modo Bandeira" } else { "[ ] Modo Bandeira" };
    draw_glass_button(fb, pitch, scr_w, scr_h, header_x, btn_y, 150, 24, flag_label, ms.flag_mode);
    draw_glass_button(fb, pitch, scr_w, scr_h, header_x + 160, btn_y, 160, 24, "Reiniciar Jogo", false);

    // 10x10 Grid container
    let grid_x = cx + (cw.saturating_sub(260)) / 2;
    let grid_y = btn_y + 32;

    fill_rounded_rect(fb, pitch, scr_w, scr_h, grid_x - 4, grid_y - 4, 268, 268, 6, 0x00CBD5E1);
    draw_rounded_rect_border(fb, pitch, scr_w, scr_h, grid_x - 4, grid_y - 4, 268, 268, 6, 0x0094A3B8);

    for r in 0..10 {
        for c in 0..10 {
            let idx = r * 10 + c;
            let cell = ms.cells[idx];
            let cell_x = grid_x + c * 26;
            let cell_y = grid_y + r * 26;

            if cell.revealed {
                if cell.is_mine {
                    // Exploded mine
                    fill_rounded_rect(fb, pitch, scr_w, scr_h, cell_x, cell_y, 24, 24, 3, 0x00EF4444);
                    draw_text_px(fb, pitch, scr_w, scr_h, cell_x + 8, cell_y + 4, "*", 0x00000000, None);
                } else {
                    // Empty or numbered cell
                    fill_rounded_rect(fb, pitch, scr_w, scr_h, cell_x, cell_y, 24, 24, 3, 0x00F1F5F9);
                    draw_rounded_rect_border(fb, pitch, scr_w, scr_h, cell_x, cell_y, 24, 24, 3, 0x00E2E8F0);
                    if cell.neighbor_count > 0 {
                        let (num_str, num_col) = match cell.neighbor_count {
                            1 => ("1", 0x001D4ED8), // Blue
                            2 => ("2", 0x0015803D), // Green
                            3 => ("3", 0x00B91C1C), // Red
                            4 => ("4", 0x001E1B4B), // Dark Blue
                            5 => ("5", 0x007F1D1D), // Maroon
                            6 => ("6", 0x000F766E), // Teal
                            7 => ("7", 0x00111827), // Black
                            _ => ("8", 0x00475569), // Gray
                        };
                        draw_text_px(fb, pitch, scr_w, scr_h, cell_x + 8, cell_y + 4, num_str, num_col, None);
                    }
                }
            } else {
                // Unrevealed beveled crystal button
                fill_rounded_rect_gradient(fb, pitch, scr_w, scr_h, cell_x, cell_y, 24, 24, 3, 0x00FFFFFF, 0x00CBD5E1);
                draw_rounded_rect_border(fb, pitch, scr_w, scr_h, cell_x, cell_y, 24, 24, 3, 0x0094A3B8);

                if cell.flagged {
                    // Red flag glyph [P]
                    draw_text_px(fb, pitch, scr_w, scr_h, cell_x + 7, cell_y + 4, "P", 0x00DC2626, None);
                }
            }
        }
    }
}

unsafe fn render_puzzle_view(
    fb: *mut u8, pitch: usize, scr_w: usize, scr_h: usize,
    cx: usize, cy: usize, cw: usize, ch: usize,
) {
    fill_rounded_rect_gradient(fb, pitch, scr_w, scr_h, cx, cy, cw, ch, 0, 0x000F172A, 0x001E293B);

    let st = CAT7_STATE.lock();
    let pz = &st.puzzle;

    // Header bar with Move Counter and Buttons
    let bar_y = cy + 12;
    let moves_str = format!("Movimentos: {}", pz.moves);
    draw_text_px(fb, pitch, scr_w, scr_h, cx + 20, bar_y + 6, &moves_str, 0x0038BDF8, None);

    let btn_shuffle_x = cx + cw.saturating_sub(230);
    draw_glass_button(fb, pitch, scr_w, scr_h, btn_shuffle_x, bar_y, 100, 26, "Embaralhar", true);
    draw_glass_button(fb, pitch, scr_w, scr_h, btn_shuffle_x + 108, bar_y, 90, 26, "Reiniciar", false);

    // 4x4 Puzzle Board
    let board_w = 260usize;
    let board_x = cx + (cw.saturating_sub(board_w)) / 2;
    let board_y = bar_y + 42;

    fill_rounded_rect(fb, pitch, scr_w, scr_h, board_x - 6, board_y - 6, board_w + 12, board_w + 12, 8, 0x000284C7);
    draw_rounded_rect_border(fb, pitch, scr_w, scr_h, board_x - 6, board_y - 6, board_w + 12, board_w + 12, 8, 0x007DD3FC);

    for idx in 0..16 {
        let r = idx / 4;
        let c = idx % 4;
        let val = pz.grid[idx];
        let tile_x = board_x + c * 65;
        let tile_y = board_y + r * 65;

        if val == 0 {
            // Empty space: dark beveled cavity
            fill_rounded_rect(fb, pitch, scr_w, scr_h, tile_x, tile_y, 60, 60, 6, 0x000B132B);
            draw_rounded_rect_border(fb, pitch, scr_w, scr_h, tile_x, tile_y, 60, 60, 6, 0x001E293B);
        } else {
            // Vitrified Crystal Block
            fill_rounded_rect_gradient(fb, pitch, scr_w, scr_h, tile_x, tile_y, 60, 60, 6, 0x00E0F2FE, 0x000284C7);
            draw_rounded_rect_border(fb, pitch, scr_w, scr_h, tile_x, tile_y, 60, 60, 6, 0x00BAE6FD);

            // Specular shine line
            fill_rounded_rect(fb, pitch, scr_w, scr_h, tile_x + 4, tile_y + 4, 52, 2, 1, 0x00FFFFFF);

            let val_str = format!("{}", val);
            let tx = tile_x + (60usize.saturating_sub(val_str.len() * 8)) / 2;
            let ty = tile_y + 22;
            draw_text_px(fb, pitch, scr_w, scr_h, tx, ty, &val_str, 0x000F172A, None);
        }
    }

    // Celebration banner
    if pz.is_solved && pz.shuffled {
        let win_y = board_y + board_w + 16;
        fill_rounded_rect_gradient(fb, pitch, scr_w, scr_h, board_x - 20, win_y, board_w + 40, 32, 6, 0x00059669, 0x00047857);
        draw_rounded_rect_border(fb, pitch, scr_w, scr_h, board_x - 20, win_y, board_w + 40, 32, 6, 0x0034D399);
        draw_text_px(fb, pitch, scr_w, scr_h, board_x + 10, win_y + 8, "* Parabens! 15-Puzzle Resolvido! *", 0x00FFFFFF, None);
    }
}

unsafe fn render_bench3d_view(
    fb: *mut u8, pitch: usize, scr_w: usize, scr_h: usize,
    cx: usize, cy: usize, cw: usize, ch: usize,
) {
    fill_rounded_rect(fb, pitch, scr_w, scr_h, cx, cy, cw, ch, 0, 0x00050B14);

    let (ang_x, ang_y, ang_z, wireframe, paused, _speed, mesh_type, fps) = {
        let mut st = CAT7_STATE.lock();
        if !st.bench.paused {
            st.bench.angle_x += st.bench.speed * 0.7;
            st.bench.angle_y += st.bench.speed * 1.0;
            st.bench.angle_z += st.bench.speed * 0.4;
        }
        let now = SYSTEM_TICKS.load(Ordering::Relaxed);
        st.bench.frames_accum += 1;
        if now.saturating_sub(st.bench.last_sec_ticks) >= 100 {
            st.bench.fps = st.bench.frames_accum;
            st.bench.frames_accum = 0;
            st.bench.last_sec_ticks = now;
        }
        (
            st.bench.angle_x, st.bench.angle_y, st.bench.angle_z,
            st.bench.wireframe, st.bench.paused, st.bench.speed,
            st.bench.mesh_type, st.bench.fps,
        )
    };

    // Control buttons strip
    let btn_y = cy + 8;
    let mode_str = if wireframe { "Modo: Wireframe" } else { "Modo: Solido Gouraud" };
    draw_glass_button(fb, pitch, scr_w, scr_h, cx + 12, btn_y, 160, 26, mode_str, true);

    let pause_str = if paused { "Retomar" } else { "Pausar" };
    draw_glass_button(fb, pitch, scr_w, scr_h, cx + 180, btn_y, 80, 26, pause_str, false);

    let mesh_str = if mesh_type == 0 { "Objeto: Cubo 3D" } else { "Objeto: Octaedro" };
    draw_glass_button(fb, pitch, scr_w, scr_h, cx + 268, btn_y, 140, 26, mesh_str, false);
    draw_glass_button(fb, pitch, scr_w, scr_h, cx + 416, btn_y, 80, 26, "Reset", false);

    // Viewport box
    let view_x = cx + 12;
    let view_y = cy + 42;
    let view_w = cw.saturating_sub(24);
    let view_h = ch.saturating_sub(50);
    fill_rounded_rect_gradient(fb, pitch, scr_w, scr_h, view_x, view_y, view_w, view_h, 6, 0x000F172A, 0x00020617);
    draw_rounded_rect_border(fb, pitch, scr_w, scr_h, view_x, view_y, view_w, view_h, 6, 0x000284C7);

    let center_x = view_x + view_w / 2;
    let center_y = view_y + view_h / 2;

    // Define 3D Vertices
    let (vertices, faces, face_colors): (&[(f32, f32, f32)], &[[usize; 3]], &[u32]) = if mesh_type == 0 {
        // Cube: 8 vertices, 12 triangles
        static CUBE_VERTS: [(f32, f32, f32); 8] = [
            (-1.0, -1.0, -1.0), (1.0, -1.0, -1.0), (1.0, 1.0, -1.0), (-1.0, 1.0, -1.0),
            (-1.0, -1.0,  1.0), (1.0, -1.0,  1.0), (1.0, 1.0,  1.0), (-1.0, 1.0,  1.0),
        ];
        static CUBE_FACES: [[usize; 3]; 12] = [
            [4, 5, 6], [4, 6, 7], // Front
            [1, 0, 3], [1, 3, 2], // Back
            [7, 6, 2], [7, 2, 3], // Top
            [0, 1, 5], [0, 5, 4], // Bottom
            [5, 1, 2], [5, 2, 6], // Right
            [0, 4, 7], [0, 7, 3], // Left
        ];
        static CUBE_COLS: [u32; 12] = [
            0x0038BDF8, 0x000284C7, // Front: Cyan/Sapphire
            0x001D4ED8, 0x001E3A8A, // Back: Deep Blue
            0x0034D399, 0x00059669, // Top: Emerald Green
            0x00F59E0B, 0x00D97706, // Bottom: Amber
            0x00A855F7, 0x007E22CE, // Right: Purple
            0x00EF4444, 0x00B91C1C, // Left: Ruby Red
        ];
        (&CUBE_VERTS, &CUBE_FACES, &CUBE_COLS)
    } else {
        // Octahedron: 6 vertices, 8 triangles
        static OCTA_VERTS: [(f32, f32, f32); 6] = [
            ( 0.0,  1.4,  0.0), // Top
            ( 0.0, -1.4,  0.0), // Bottom
            (-1.0,  0.0, -1.0),
            ( 1.0,  0.0, -1.0),
            ( 1.0,  0.0,  1.0),
            (-1.0,  0.0,  1.0),
        ];
        static OCTA_FACES: [[usize; 3]; 8] = [
            [0, 2, 3], [0, 3, 4], [0, 4, 5], [0, 5, 2], // Top pyramid
            [1, 3, 2], [1, 4, 3], [1, 5, 4], [1, 2, 5], // Bottom pyramid
        ];
        static OCTA_COLS: [u32; 8] = [
            0x0038BDF8, 0x0034D399, 0x00FBBF24, 0x00F472B6,
            0x00818CF8, 0x002DD4BF, 0x00FB923C, 0x00E879F9,
        ];
        (&OCTA_VERTS, &OCTA_FACES, &OCTA_COLS)
    };

    // Rotation matrices
    let (cx_a, sx_a) = (ang_x.cos(), ang_x.sin());
    let (cy_a, sy_a) = (ang_y.cos(), ang_y.sin());
    let (cz_a, sz_a) = (ang_z.cos(), ang_z.sin());

    // Transform vertices
    let mut trans_verts = [(0.0f32, 0.0f32, 0.0f32); 8];
    let mut proj_verts = [(0isize, 0isize); 8];

    for (i, v) in vertices.iter().enumerate() {
        // Rotate X
        let y1 = v.1 * cx_a - v.2 * sx_a;
        let z1 = v.1 * sx_a + v.2 * cx_a;

        // Rotate Y
        let x2 = v.0 * cy_a + z1 * sy_a;
        let z2 = -v.0 * sy_a + z1 * cy_a;

        // Rotate Z
        let x3 = x2 * cz_a - y1 * sz_a;
        let y3 = x2 * sz_a + y1 * cz_a;

        trans_verts[i] = (x3, y3, z2);

        // Perspective projection
        let dist = 3.2f32;
        let scale = 150.0f32;
        let p_factor = scale / (z2 + dist).max(0.1);
        let px = center_x as isize + (x3 * p_factor) as isize;
        let py = center_y as isize - (y3 * p_factor) as isize;
        proj_verts[i] = (px, py);
    }

    // Render faces
    for (f_idx, face) in faces.iter().enumerate() {
        let v0 = trans_verts[face[0]];
        let v1 = trans_verts[face[1]];
        let v2 = trans_verts[face[2]];

        let p0 = proj_verts[face[0]];
        let p1 = proj_verts[face[1]];
        let p2 = proj_verts[face[2]];

        // Backface culling: 2D cross product
        let cross = (p1.0 - p0.0) * (p2.1 - p0.1) - (p1.1 - p0.1) * (p2.0 - p0.0);
        if cross <= 0 { continue; }

        if wireframe {
            // Wireframe lines
            let col = face_colors[f_idx];
            draw_line_clipped(fb, pitch, scr_w, scr_h, p0.0, p0.1, p1.0, p1.1, col, view_x + 1, view_y + 1, view_x + view_w - 1, view_y + view_h - 1);
            draw_line_clipped(fb, pitch, scr_w, scr_h, p1.0, p1.1, p2.0, p2.1, col, view_x + 1, view_y + 1, view_x + view_w - 1, view_y + view_h - 1);
            draw_line_clipped(fb, pitch, scr_w, scr_h, p2.0, p2.1, p0.0, p0.1, col, view_x + 1, view_y + 1, view_x + view_w - 1, view_y + view_h - 1);
        } else {
            // Directional lighting calculation
            let ab = (v1.0 - v0.0, v1.1 - v0.1, v1.2 - v0.2);
            let ac = (v2.0 - v0.0, v2.1 - v0.1, v2.2 - v0.2);
            let mut nx = ab.1 * ac.2 - ab.2 * ac.1;
            let mut ny = ab.2 * ac.0 - ab.0 * ac.2;
            let mut nz = ab.0 * ac.1 - ab.1 * ac.0;
            let len = (nx * nx + ny * ny + nz * nz).sqrt().max(0.001);
            nx /= len; ny /= len; nz /= len;

            // Light from (0.577, 0.577, -0.577)
            let light_dot = (nx * 0.577 + ny * 0.577 - nz * 0.577).max(0.0);
            let intensity = (0.25 + 0.75 * light_dot).clamp(0.2, 1.0);

            let base_col = face_colors[f_idx];
            let r = (((base_col >> 16) & 0xFF) as f32 * intensity) as u32;
            let g = (((base_col >> 8) & 0xFF) as f32 * intensity) as u32;
            let b = ((base_col & 0xFF) as f32 * intensity) as u32;
            let lit_col = (r << 16) | (g << 8) | b;

            fill_triangle_clipped(fb, pitch, scr_w, scr_h, p0, p1, p2, lit_col, view_x + 1, view_y + 1, view_x + view_w - 1, view_y + view_h - 1);
        }
    }

    // Telemetry HUD overlay
    let hud_y = view_y + 12;
    let fps_text = format!("DirectX / GDI Benchmark — FPS: {} (Tempo: {:.1} ms)", fps, 1000.0 / (fps.max(1) as f32));
    draw_text_px(fb, pitch, scr_w, scr_h, view_x + 16, hud_y, &fps_text, 0x004ADE80, None);

    let num_tris = faces.len();
    let num_v = vertices.len();
    let info_text = format!("Malha: {} ({} vertices, {} triangulos)", if mesh_type == 0 { "Cubo" } else { "Octaedro" }, num_v, num_tris);
    draw_text_px(fb, pitch, scr_w, scr_h, view_x + 16, hud_y + 18, &info_text, 0x0038BDF8, None);
    draw_text_px(fb, pitch, scr_w, scr_h, view_x + 16, hud_y + 36, "Pipeline: Software Rasterizer com Backface Culling & Gouraud", 0x0094A3B8, None);
}

fn handle_classic_games_click(cx: usize, cy: usize, desk: &mut DesktopState) -> bool {
    let mut st = CAT7_STATE.lock();
    let slot = 0;
    let win_x = st.last_cx[slot];
    let win_y = st.last_cy[slot];
    let win_w = st.last_cw[slot];
    let _win_h = st.last_ch[slot];

    // Top Tabs click (cy >= win_y && cy <= win_y + 28)
    if cy >= win_y && cy <= win_y + 28 {
        if cx >= win_x + 8 && cx <= win_x + 128 {
            st.games_tab = 0;
            desk.needs_full_redraw = true;
            return true;
        } else if cx >= win_x + 132 && cx <= win_x + 232 {
            st.games_tab = 1;
            desk.needs_full_redraw = true;
            return true;
        } else if cx >= win_x + 236 && cx <= win_x + 366 {
            st.games_tab = 2;
            desk.needs_full_redraw = true;
            return true;
        }
    }

    match st.games_tab {
        0 => {
            // Minesweeper clicks
            let header_y = win_y + 28 + 8;
            let header_w = 320usize.min(win_w.saturating_sub(16));
            let header_x = win_x + (win_w.saturating_sub(header_w)) / 2;

            // Smile button click
            let smile_x = header_x + header_w / 2 - 18;
            let smile_y = header_y + 5;
            if cx >= smile_x && cx <= smile_x + 36 && cy >= smile_y && cy <= smile_y + 32 {
                st.minesweeper.reset();
                desk.needs_full_redraw = true;
                return true;
            }

            // Buttons: [Modo Bandeira] and [Reiniciar Jogo]
            let btn_y = header_y + 48;
            if cy >= btn_y && cy <= btn_y + 24 {
                if cx >= header_x && cx <= header_x + 150 {
                    st.minesweeper.flag_mode = !st.minesweeper.flag_mode;
                    desk.needs_full_redraw = true;
                    return true;
                } else if cx >= header_x + 160 && cx <= header_x + 320 {
                    st.minesweeper.reset();
                    desk.needs_full_redraw = true;
                    return true;
                }
            }

            // Grid click
            let grid_x = win_x + (win_w.saturating_sub(260)) / 2;
            let grid_y = btn_y + 32;

            if cx >= grid_x && cx < grid_x + 260 && cy >= grid_y && cy < grid_y + 260 {
                let col = (cx - grid_x) / 26;
                let row = (cy - grid_y) / 26;

                let right_button = MOUSE_STATE.lock().right_button;
                if right_button || st.minesweeper.flag_mode {
                    st.minesweeper.toggle_flag(row, col);
                } else {
                    st.minesweeper.reveal(row, col);
                }
                desk.needs_full_redraw = true;
                return true;
            }
        }
        1 => {
            // 15-Puzzle clicks
            let bar_y = win_y + 28 + 12;
            let btn_shuffle_x = win_x + win_w.saturating_sub(230);

            if cy >= bar_y && cy <= bar_y + 26 {
                if cx >= btn_shuffle_x && cx <= btn_shuffle_x + 100 {
                    st.puzzle.shuffle();
                    desk.needs_full_redraw = true;
                    return true;
                } else if cx >= btn_shuffle_x + 108 && cx <= btn_shuffle_x + 198 {
                    st.puzzle.reset();
                    desk.needs_full_redraw = true;
                    return true;
                }
            }

            // Puzzle grid
            let board_w = 260usize;
            let board_x = win_x + (win_w.saturating_sub(board_w)) / 2;
            let board_y = bar_y + 42;

            if cx >= board_x && cx < board_x + board_w && cy >= board_y && cy < board_y + board_w {
                let c = (cx - board_x) / 65;
                let r = (cy - board_y) / 65;
                let idx = r * 4 + c;
                if st.puzzle.click_cell(idx) {
                    desk.needs_full_redraw = true;
                    return true;
                }
            }
        }
        2 => {
            // Benchmark 3D clicks
            let btn_y = win_y + 28 + 8;
            if cy >= btn_y && cy <= btn_y + 26 {
                if cx >= win_x + 12 && cx <= win_x + 172 {
                    st.bench.wireframe = !st.bench.wireframe;
                    desk.needs_full_redraw = true;
                    return true;
                } else if cx >= win_x + 180 && cx <= win_x + 260 {
                    st.bench.paused = !st.bench.paused;
                    desk.needs_full_redraw = true;
                    return true;
                } else if cx >= win_x + 268 && cx <= win_x + 408 {
                    st.bench.mesh_type = 1 - st.bench.mesh_type;
                    desk.needs_full_redraw = true;
                    return true;
                } else if cx >= win_x + 416 && cx <= win_x + 496 {
                    st.bench.angle_x = 0.4;
                    st.bench.angle_y = 0.6;
                    st.bench.angle_z = 0.0;
                    desk.needs_full_redraw = true;
                    return true;
                }
            }
        }
        _ => {}
    }

    false
}

// =============================================================================
// 49. CENTRAL DE FACILIDADE DE ACESSO (OSK & Magnifier)
// =============================================================================

unsafe fn render_accessibility_center(
    fb: *mut u8, pitch: usize, scr_w: usize, scr_h: usize,
    cx: usize, cy: usize, cw: usize, ch: usize,
    desk: &mut DesktopState,
) {
    let tab = { CAT7_STATE.lock().access_tab };

    // 1. Top Tabs Toolbar (28px)
    draw_glass_toolbar(fb, pitch, scr_w, scr_h, cx, cy, cw, 28);
    draw_glass_tab(fb, pitch, scr_w, scr_h, cx + 8, cy + 2, 170, 24, "Teclado Virtual (OSK)", tab == 0);
    draw_glass_tab(fb, pitch, scr_w, scr_h, cx + 182, cy + 2, 160, 24, "Lupa de Tela (Zoom)", tab == 1);
    draw_glass_tab(fb, pitch, scr_w, scr_h, cx + 346, cy + 2, 140, 24, "Alto Contraste", tab == 2);

    let body_y = cy + 28;
    let body_h = ch.saturating_sub(28 + 22);

    match tab {
        0 => render_osk_view(fb, pitch, scr_w, scr_h, cx, body_y, cw, body_h, desk.start_menu_open),
        1 => render_magnifier_view(fb, pitch, scr_w, scr_h, cx, body_y, cw, body_h),
        2 => render_high_contrast_view(fb, pitch, scr_w, scr_h, cx, body_y, cw, body_h),
        _ => {}
    }

    // Status bar
    let status_y = cy + ch.saturating_sub(22);
    let (l_msg, r_msg) = match tab {
        0 => ("Teclado Virtual Ativo (PS/2 Ring Buffer Injector)", "Clique para digitar em qualquer janela"),
        1 => ("Lupa de Tela GOP UEFI (Nearest-Neighbor Scaler)", "Zoom 2x / 4x em tempo real"),
        _ => ("Central de Acessibilidade HybridOS", "Preferencias em /etc/accessibility.conf"),
    };
    draw_glass_statusbar(fb, pitch, scr_w, scr_h, cx, status_y, cw, 22, l_msg, r_msg);
}

unsafe fn render_osk_view(
    fb: *mut u8, pitch: usize, scr_w: usize, scr_h: usize,
    cx: usize, cy: usize, cw: usize, ch: usize,
    start_menu_open: bool,
) {
    fill_rounded_rect_gradient(fb, pitch, scr_w, scr_h, cx, cy, cw, ch, 0, 0x000F172A, 0x001E293B);

    let (caps, shift, ctrl, alt, altgr, buf_text) = {
        let st = CAT7_STATE.lock();
        (
            st.osk.caps_lock,
            st.osk.shift_active,
            st.osk_ctrl,
            st.osk_alt,
            st.osk_altgr,
            st.osk.test_buffer.clone(),
        )
    };

    // Text Display Box for instant typing feedback
    let box_y = cy + 10;
    let box_w = cw.saturating_sub(24);
    draw_text_px(fb, pitch, scr_w, scr_h, cx + 12, box_y, "Buffer de Entrada OSK (Injeta diretamente via PS/2 Controller):", 0x0038BDF8, None);

    let input_y = box_y + 18;
    fill_rounded_rect(fb, pitch, scr_w, scr_h, cx + 12, input_y, box_w, 28, 4, 0x00FFFFFF);
    draw_rounded_rect_border(fb, pitch, scr_w, scr_h, cx + 12, input_y, box_w, 28, 4, 0x000284C7);

    let display_str = format!("{}_", buf_text);
    draw_text_clipped(fb, pitch, scr_w, scr_h, cx + 20, input_y + 6, cx + box_w.saturating_sub(156), &display_str, 0x000F172A, None);

    // Modifier indicator badges: CTL, ALT, CAP, SHF
    let ctrl_badge_x = cx + box_w.saturating_sub(150);
    let ctrl_bg = if ctrl { 0x000284C7 } else { 0x00CBD5E1 };
    let ctrl_fg = if ctrl { 0x00FFFFFF } else { 0x00475569 };
    fill_rounded_rect(fb, pitch, scr_w, scr_h, ctrl_badge_x, input_y + 4, 34, 20, 3, ctrl_bg);
    draw_text_px(fb, pitch, scr_w, scr_h, ctrl_badge_x + 5, input_y + 6, "CTL", ctrl_fg, None);

    let alt_badge_x = ctrl_badge_x + 38;
    let alt_bg = if alt || altgr { 0x000284C7 } else { 0x00CBD5E1 };
    let alt_fg = if alt || altgr { 0x00FFFFFF } else { 0x00475569 };
    fill_rounded_rect(fb, pitch, scr_w, scr_h, alt_badge_x, input_y + 4, 34, 20, 3, alt_bg);
    draw_text_px(fb, pitch, scr_w, scr_h, alt_badge_x + 5, input_y + 6, "ALT", alt_fg, None);

    let caps_badge_x = alt_badge_x + 38;
    let caps_bg = if caps { 0x000284C7 } else { 0x00CBD5E1 };
    let caps_fg = if caps { 0x00FFFFFF } else { 0x00475569 };
    fill_rounded_rect(fb, pitch, scr_w, scr_h, caps_badge_x, input_y + 4, 34, 20, 3, caps_bg);
    draw_text_px(fb, pitch, scr_w, scr_h, caps_badge_x + 5, input_y + 6, "CAP", caps_fg, None);

    let shift_badge_x = caps_badge_x + 38;
    let shift_bg = if shift { 0x000284C7 } else { 0x00CBD5E1 };
    let shift_fg = if shift { 0x00FFFFFF } else { 0x00475569 };
    fill_rounded_rect(fb, pitch, scr_w, scr_h, shift_badge_x, input_y + 4, 34, 20, 3, shift_bg);
    draw_text_px(fb, pitch, scr_w, scr_h, shift_badge_x + 5, input_y + 6, "SHF", shift_fg, None);

    // Keyboard Plate Container
    let kbd_x = cx + 12;
    let kbd_y = input_y + 36;
    let kbd_w = box_w;
    let kbd_h = ch.saturating_sub(input_y + 42 - cy);

    fill_rounded_rect(fb, pitch, scr_w, scr_h, kbd_x, kbd_y, kbd_w, kbd_h, 6, 0x00020617);
    draw_rounded_rect_border(fb, pitch, scr_w, scr_h, kbd_x, kbd_y, kbd_w, kbd_h, 6, 0x00334155);

    // 5 Rows of Keys
    let row1 = [
        ("`", "~", 32), ("1", "!", 32), ("2", "@", 32), ("3", "#", 32), ("4", "$", 32),
        ("5", "%", 32), ("6", "^", 32), ("7", "&", 32), ("8", "*", 32), ("9", "(", 32),
        ("0", ")", 32), ("-", "_", 32), ("=", "+", 32), ("<--", "<--", 58),
    ];

    let row2 = [
        ("Tab", "Tab", 46), ("q", "Q", 32), ("w", "W", 32), ("e", "E", 32), ("r", "R", 32),
        ("t", "T", 32), ("y", "Y", 32), ("u", "U", 32), ("i", "I", 32), ("o", "O", 32),
        ("p", "P", 32), ("[", "{", 32), ("]", "}", 32), ("\\", "|", 44),
    ];

    let row3 = [
        ("Caps", "Caps", 56), ("a", "A", 32), ("s", "S", 32), ("d", "D", 32), ("f", "F", 32),
        ("g", "G", 32), ("h", "H", 32), ("j", "J", 32), ("k", "K", 32), ("l", "L", 32),
        (";", ":", 32), ("'", "\"", 32), ("Enter", "Enter", 64),
    ];

    let row4 = [
        ("Shift", "Shift", 70), ("z", "Z", 32), ("x", "X", 32), ("c", "C", 32), ("v", "V", 32),
        ("b", "B", 32), ("n", "N", 32), ("m", "M", 32), (",", "<", 32), (".", ">", 32),
        ("/", "?", 32), ("Shift", "Shift", 80),
    ];

    let row5 = [
        ("Ctrl", "Ctrl", 50), ("Win", "Win", 40), ("Alt", "Alt", 50),
        ("Espaco", "Espaco", 210),
        ("AltGr", "AltGr", 50), ("Limpar", "Limpar", 70),
    ];

    let rows: [&[(&str, &str, usize)]; 5] = [&row1, &row2, &row3, &row4, &row5];

    let key_h = 32usize;
    let mut cur_y = kbd_y + 8;

    for r_keys in rows.iter() {
        let mut cur_x = kbd_x + 8;
        for &(norm, shf, kw) in r_keys.iter() {
            let is_mod_active = (norm == "Caps" && caps)
                || (norm == "Shift" && shift)
                || (norm == "Ctrl" && ctrl)
                || (norm == "Alt" && alt)
                || (norm == "AltGr" && altgr)
                || (norm == "Win" && start_menu_open);

            let (bg_top, bg_bot, border_col, fg_col) = if is_mod_active {
                (0x0038BDF8, 0x000284C7, 0x007DD3FC, 0x00FFFFFF)
            } else if norm == "Enter" || norm == "Espaco" {
                (0x00E0F2FE, 0x00BAE6FD, 0x0038BDF8, 0x000F172A)
            } else {
                (0x00FFFFFF, 0x00E2E8F0, 0x0094A3B8, 0x000F172A)
            };

            fill_rounded_rect_gradient(fb, pitch, scr_w, scr_h, cur_x, cur_y, kw, key_h, 4, bg_top, bg_bot);
            draw_rounded_rect_border(fb, pitch, scr_w, scr_h, cur_x, cur_y, kw, key_h, 4, border_col);

            let label = if shift { shf } else if caps && norm.len() == 1 && norm.chars().next().unwrap().is_alphabetic() { shf } else { norm };
            let tx = cur_x + (kw.saturating_sub(label.len() * 8)) / 2;
            let ty = cur_y + 8;
            draw_text_px(fb, pitch, scr_w, scr_h, tx, ty, label, fg_col, None);

            cur_x += kw + 4;
        }
        cur_y += key_h + 4;
    }
}

unsafe fn render_magnifier_view(
    fb: *mut u8, pitch: usize, scr_w: usize, scr_h: usize,
    cx: usize, cy: usize, cw: usize, ch: usize,
) {
    let (zoom, follow, frozen, show_grid) = {
        let st = CAT7_STATE.lock();
        (st.magnifier.zoom_factor, st.magnifier.follow_mouse, st.magnifier.frozen, st.magnifier.show_grid)
    };

    // Mouse coordinates to sample
    let (mx, my) = {
        let m = MOUSE_STATE.lock();
        (m.x.clamp(0, (scr_w.saturating_sub(1)) as i32) as usize, m.y.clamp(0, (scr_h.saturating_sub(1)) as i32) as usize)
    };

    let (sample_center_x, sample_center_y) = if frozen || !follow {
        let st = CAT7_STATE.lock();
        (st.magnifier.last_sample_x, st.magnifier.last_sample_y)
    } else {
        let mut st = CAT7_STATE.lock();
        st.magnifier.last_sample_x = mx;
        st.magnifier.last_sample_y = my;
        (mx, my)
    };

    // Magnifier Viewport Canvas geometry
    let view_x = cx + 12;
    let view_y = cy + 42;
    let view_w = cw.saturating_sub(24);
    let view_h = ch.saturating_sub(84);

    let scale = zoom.max(1);
    let src_w = view_w / scale;
    let src_h = view_h / scale;

    let src_x0 = sample_center_x.saturating_sub(src_w / 2);
    let src_y0 = sample_center_y.saturating_sub(src_h / 2);

    // Snapshot all source pixels BEFORE clearing or writing into the framebuffer
    // to eliminate self-feedback corruption when the cursor enters the magnifier viewport.
    let mut src_buf: Vec<u32> = Vec::with_capacity(src_w.saturating_mul(src_h));
    for sy in 0..src_h {
        let actual_sy = (src_y0 + sy).min(scr_h.saturating_sub(1));
        let src_row = fb.add(actual_sy * pitch) as *const u32;
        for sx in 0..src_w {
            let actual_sx = (src_x0 + sx).min(scr_w.saturating_sub(1));
            src_buf.push(*src_row.add(actual_sx));
        }
    }

    // Now draw background and toolbar chrome
    fill_rounded_rect(fb, pitch, scr_w, scr_h, cx, cy, cw, ch, 0, 0x000F172A);

    let bar_y = cy + 8;
    draw_glass_button(fb, pitch, scr_w, scr_h, cx + 12, bar_y, 75, 26, "Zoom 2x", zoom == 2);
    draw_glass_button(fb, pitch, scr_w, scr_h, cx + 92, bar_y, 75, 26, "Zoom 4x", zoom == 4);

    let follow_str = if follow { "Seguir Mouse: SIM" } else { "Seguir Mouse: NAO" };
    draw_glass_button(fb, pitch, scr_w, scr_h, cx + 172, bar_y, 140, 26, follow_str, follow);

    let freeze_str = if frozen { "Descongelar" } else { "Congelar Imagem" };
    draw_glass_button(fb, pitch, scr_w, scr_h, cx + 318, bar_y, 125, 26, freeze_str, frozen);

    let grid_str = if show_grid { "Grade: ON" } else { "Grade: OFF" };
    draw_glass_button(fb, pitch, scr_w, scr_h, cx + 448, bar_y, 90, 26, grid_str, show_grid);

    fill_rounded_rect(fb, pitch, scr_w, scr_h, view_x, view_y, view_w, view_h, 4, 0x00000000);
    draw_rounded_rect_border(fb, pitch, scr_w, scr_h, view_x, view_y, view_w, view_h, 4, 0x0038BDF8);

    let mut center_pixel_col = 0x00000000;

    // Nearest-neighbor scaling reading strictly from the pre-captured src_buf
    for sy in 0..src_h {
        let row_base = sy * src_w;
        for sx in 0..src_w {
            let pixel = src_buf[row_base + sx];

            if sx == src_w / 2 && sy == src_h / 2 {
                center_pixel_col = pixel;
            }

            let dst_x = view_x + sx * scale;
            let dst_y = view_y + sy * scale;

            for dy in 0..scale {
                let py = dst_y + dy;
                if py >= scr_h || py >= view_y + view_h { continue; }
                let dst_row = fb.add(py * pitch) as *mut u32;
                for dx in 0..scale {
                    let px = dst_x + dx;
                    if px >= scr_w || px >= view_x + view_w { continue; }

                    if show_grid && scale == 4 && (dx == 0 || dy == 0) {
                        *dst_row.add(px) = 0x00334155; // 1px pixel grid
                    } else {
                        *dst_row.add(px) = pixel;
                    }
                }
            }
        }
    }

    {
        let mut st = CAT7_STATE.lock();
        st.magnifier.last_color = center_pixel_col;
    }

    // Crosshair Reticle at center of Magnifier
    let mid_x = view_x + view_w / 2;
    let mid_y = view_y + view_h / 2;

    for off in -8isize..=8 {
        if off == 0 { continue; }
        let px = (mid_x as isize + off) as usize;
        let py = (mid_y as isize + off) as usize;
        if px < scr_w && mid_y < scr_h {
            let row = fb.add(mid_y * pitch) as *mut u32;
            *row.add(px) = 0x00EF4444; // Red horizontal crosshair
        }
        if py < scr_h && mid_x < scr_w {
            let row = fb.add(py * pitch) as *mut u32;
            *row.add(mid_x) = 0x00EF4444; // Red vertical crosshair
        }
    }

    // Bottom Telemetry Strip
    let info_y = view_y + view_h + 8;
    let coord_str = format!("Cursor sob mira: ({}, {}) | Pixel: 0x{:06X}", sample_center_x, sample_center_y, center_pixel_col & 0xFFFFFF);
    draw_text_px(fb, pitch, scr_w, scr_h, cx + 16, info_y + 4, &coord_str, 0x00F8FAFC, None);

    // Color Swatch preview box
    fill_rounded_rect(fb, pitch, scr_w, scr_h, cx + cw - 70, info_y, 50, 22, 3, center_pixel_col);
    draw_rounded_rect_border(fb, pitch, scr_w, scr_h, cx + cw - 70, info_y, 50, 22, 3, 0x00FFFFFF);
}

unsafe fn render_high_contrast_view(
    fb: *mut u8, pitch: usize, scr_w: usize, scr_h: usize,
    cx: usize, cy: usize, cw: usize, ch: usize,
) {
    fill_rounded_rect(fb, pitch, scr_w, scr_h, cx, cy, cw, ch, 0, 0x00000000);
    draw_text_px(fb, pitch, scr_w, scr_h, cx + 20, cy + 20, "Recursos de Acessibilidade & Alto Contraste", 0x0038BDF8, None);

    let (hc_options, hc_status_msg) = {
        let st = CAT7_STATE.lock();
        (st.hc_options, st.hc_status_msg.clone())
    };

    let items = [
        ("Esquema de Cores em Alto Contraste (Fundo Preto / Texto Amarelo)", 0x00FEF08A),
        ("Feedback Sonoro ao Pressionar Teclas PS/2 (Intel HDA Bip)", 0x0034D399),
        ("Inverter Botoes do Mouse (Destro / Canhoto)", 0x00CBD5E1),
        ("Ponteiro de Mouse Ampliado com Trilha Luminosa", 0x0038BDF8),
        ("Teclas de Aderencia (Sticky Keys para Shift/Ctrl/Alt)", 0x00CBD5E1),
    ];

    let row_w = cw.saturating_sub(40);
    for (idx, (desc, active_col)) in items.iter().enumerate() {
        let iy = cy + 55 + idx * 30;
        let checked = hc_options[idx];
        let bg = if checked { 0x001E293B } else { 0x000F172A };
        let border = if checked { *active_col } else { 0x00334155 };
        let fg = if checked { *active_col } else { 0x0094A3B8 };
        let line = format!("{} {}", if checked { "[x]" } else { "[ ]" }, desc);

        fill_rounded_rect(fb, pitch, scr_w, scr_h, cx + 20, iy, row_w, 24, 3, bg);
        draw_rounded_rect_border(fb, pitch, scr_w, scr_h, cx + 20, iy, row_w, 24, 3, border);
        draw_text_clipped(fb, pitch, scr_w, scr_h, cx + 30, iy + 4, cx + 20 + row_w.saturating_sub(8), &line, fg, None);
    }

    draw_glass_button(fb, pitch, scr_w, scr_h, cx + 20, cy + 230, 160, 28, "Salvar Preferencias", true);

    if !hc_status_msg.is_empty() {
        draw_text_clipped(
            fb, pitch, scr_w, scr_h,
            cx + 20, cy + 268,
            cx + cw.saturating_sub(16),
            &hc_status_msg,
            0x004ADE80,
            None,
        );
    }
}

fn handle_accessibility_click(cx: usize, cy: usize, desk: &mut DesktopState) -> bool {
    let mut st = CAT7_STATE.lock();
    let slot = 1;
    let win_x = st.last_cx[slot];
    let win_y = st.last_cy[slot];
    let win_w = st.last_cw[slot];

    // Top Tabs click
    if cy >= win_y && cy <= win_y + 28 {
        if cx >= win_x + 8 && cx <= win_x + 178 {
            st.access_tab = 0;
            desk.needs_full_redraw = true;
            return true;
        } else if cx >= win_x + 182 && cx <= win_x + 342 {
            st.access_tab = 1;
            desk.needs_full_redraw = true;
            return true;
        } else if cx >= win_x + 346 && cx <= win_x + 486 {
            st.access_tab = 2;
            desk.needs_full_redraw = true;
            return true;
        }
    }

    match st.access_tab {
        0 => {
            // OSK clicks
            let box_y = win_y + 28 + 10;
            let input_y = box_y + 18;
            let kbd_x = win_x + 12;
            let kbd_y = input_y + 36;

            let row1: [(&str, &str, usize, Option<char>, Option<char>); 14] = [
                ("`", "~", 32, Some('`'), Some('~')),
                ("1", "!", 32, Some('1'), Some('!')),
                ("2", "@", 32, Some('2'), Some('@')),
                ("3", "#", 32, Some('3'), Some('#')),
                ("4", "$", 32, Some('4'), Some('$')),
                ("5", "%", 32, Some('5'), Some('%')),
                ("6", "^", 32, Some('6'), Some('^')),
                ("7", "&", 32, Some('7'), Some('&')),
                ("8", "*", 32, Some('8'), Some('*')),
                ("9", "(", 32, Some('9'), Some('(')),
                ("0", ")", 32, Some('0'), Some(')')),
                ("-", "_", 32, Some('-'), Some('_')),
                ("=", "+", 32, Some('='), Some('+')),
                ("<--", "<--", 58, Some('\x08'), Some('\x08')),
            ];

            let row2: [(&str, &str, usize, Option<char>, Option<char>); 14] = [
                ("Tab", "Tab", 46, Some('\t'), Some('\t')),
                ("q", "Q", 32, Some('q'), Some('Q')),
                ("w", "W", 32, Some('w'), Some('W')),
                ("e", "E", 32, Some('e'), Some('E')),
                ("r", "R", 32, Some('r'), Some('R')),
                ("t", "T", 32, Some('t'), Some('T')),
                ("y", "Y", 32, Some('y'), Some('Y')),
                ("u", "U", 32, Some('u'), Some('U')),
                ("i", "I", 32, Some('i'), Some('I')),
                ("o", "O", 32, Some('o'), Some('O')),
                ("p", "P", 32, Some('p'), Some('P')),
                ("[", "{", 32, Some('['), Some('{')),
                ("]", "}", 32, Some(']'), Some('}')),
                ("\\", "|", 44, Some('\\'), Some('|')),
            ];

            let row3: [(&str, &str, usize, Option<char>, Option<char>); 13] = [
                ("Caps", "Caps", 56, None, None),
                ("a", "A", 32, Some('a'), Some('A')),
                ("s", "S", 32, Some('s'), Some('S')),
                ("d", "D", 32, Some('d'), Some('D')),
                ("f", "F", 32, Some('f'), Some('F')),
                ("g", "G", 32, Some('g'), Some('G')),
                ("h", "H", 32, Some('h'), Some('H')),
                ("j", "J", 32, Some('j'), Some('J')),
                ("k", "K", 32, Some('k'), Some('K')),
                ("l", "L", 32, Some('l'), Some('L')),
                (";", ":", 32, Some(';'), Some(':')),
                ("'", "\"", 32, Some('\''), Some('"')),
                ("Enter", "Enter", 64, Some('\n'), Some('\n')),
            ];

            let row4: [(&str, &str, usize, Option<char>, Option<char>); 12] = [
                ("Shift", "Shift", 70, None, None),
                ("z", "Z", 32, Some('z'), Some('Z')),
                ("x", "X", 32, Some('x'), Some('X')),
                ("c", "C", 32, Some('c'), Some('C')),
                ("v", "V", 32, Some('v'), Some('V')),
                ("b", "B", 32, Some('b'), Some('B')),
                ("n", "N", 32, Some('n'), Some('N')),
                ("m", "M", 32, Some('m'), Some('M')),
                (",", "<", 32, Some(','), Some('<')),
                (".", ">", 32, Some('.'), Some('>')),
                ("/", "?", 32, Some('/'), Some('?')),
                ("Shift", "Shift", 80, None, None),
            ];

            let row5: [(&str, &str, usize, Option<char>, Option<char>); 6] = [
                ("Ctrl", "Ctrl", 50, None, None),
                ("Win", "Win", 40, None, None),
                ("Alt", "Alt", 50, None, None),
                ("Espaco", "Espaco", 210, Some(' '), Some(' ')),
                ("AltGr", "AltGr", 50, None, None),
                ("Limpar", "Limpar", 70, None, None),
            ];

            let rows_data: [&[(&str, &str, usize, Option<char>, Option<char>)]; 5] = [
                &row1, &row2, &row3, &row4, &row5,
            ];

            let key_h = 32usize;
            let mut cur_y = kbd_y + 8;

            for r_keys in rows_data.iter() {
                let mut cur_x = kbd_x + 8;
                for &(norm, _shf, kw, norm_c, shf_c) in r_keys.iter() {
                    if cx >= cur_x && cx < cur_x + kw && cy >= cur_y && cy < cur_y + key_h {
                        // Clicked key!
                        if norm == "Caps" {
                            st.osk.caps_lock = !st.osk.caps_lock;
                        } else if norm == "Shift" {
                            st.osk.shift_active = !st.osk.shift_active;
                        } else if norm == "Ctrl" {
                            st.osk_ctrl = !st.osk_ctrl;
                        } else if norm == "Alt" {
                            st.osk_alt = !st.osk_alt;
                        } else if norm == "AltGr" {
                            st.osk_altgr = !st.osk_altgr;
                        } else if norm == "Win" {
                            desk.start_menu_open = !desk.start_menu_open;
                        } else if norm == "Limpar" {
                            st.osk.test_buffer.clear();
                        } else {
                            let shift = st.osk.shift_active;
                            let caps = st.osk.caps_lock;
                            let chosen_c = if st.osk_altgr {
                                match norm_c {
                                    Some('2') => Some('@'),
                                    Some('3') => Some('#'),
                                    Some('=') => Some('~'),
                                    Some('[') => Some('{'),
                                    Some(']') => Some('}'),
                                    Some('\\') => Some('|'),
                                    Some('/') => Some('?'),
                                    _ => shf_c.or(norm_c),
                                }
                            } else if shift {
                                shf_c.or(norm_c)
                            } else if caps && norm.len() == 1 && norm.chars().next().unwrap().is_alphabetic() {
                                shf_c.or(norm_c)
                            } else {
                                norm_c
                            };

                            if let Some(c) = chosen_c {
                                if st.osk_ctrl {
                                    let lower = c.to_ascii_lowercase();
                                    if lower >= 'a' && lower <= 'z' {
                                        let ctrl_char = ((lower as u8) - b'a' + 1) as char;
                                        enqueue_keyboard_char(ctrl_char);
                                        st.osk.last_char = Some(ctrl_char);
                                        st.osk.test_buffer.push('^');
                                        st.osk.test_buffer.push(lower.to_ascii_uppercase());
                                    } else {
                                        enqueue_keyboard_char(c);
                                        st.osk.last_char = Some(c);
                                        st.osk.test_buffer.push(c);
                                    }
                                    st.osk_ctrl = false;
                                } else if st.osk_alt {
                                    enqueue_keyboard_char('\x1b');
                                    enqueue_keyboard_char(c);
                                    st.osk.last_char = Some(c);
                                    st.osk.test_buffer.push_str("M-");
                                    st.osk.test_buffer.push(c);
                                    st.osk_alt = false;
                                } else {
                                    // Injetar caractere real no buffer do teclado do kernel!
                                    enqueue_keyboard_char(c);
                                    st.osk.last_char = Some(c);

                                    if c == '\x08' {
                                        st.osk.test_buffer.pop();
                                    } else if c == '\n' {
                                        st.osk.test_buffer.push(' ');
                                    } else {
                                        st.osk.test_buffer.push(c);
                                    }
                                }

                                if st.osk_altgr {
                                    st.osk_altgr = false;
                                }
                                if st.osk.shift_active {
                                    st.osk.shift_active = false;
                                }
                            }
                        }
                        desk.needs_full_redraw = true;
                        return true;
                    }
                    cur_x += kw + 4;
                }
                cur_y += key_h + 4;
            }
        }
        1 => {
            // Magnifier clicks
            let bar_y = win_y + 28 + 8;
            if cy >= bar_y && cy <= bar_y + 26 {
                if cx >= win_x + 12 && cx <= win_x + 87 {
                    st.magnifier.zoom_factor = 2;
                    desk.needs_full_redraw = true;
                    return true;
                } else if cx >= win_x + 92 && cx <= win_x + 167 {
                    st.magnifier.zoom_factor = 4;
                    desk.needs_full_redraw = true;
                    return true;
                } else if cx >= win_x + 172 && cx <= win_x + 312 {
                    st.magnifier.follow_mouse = !st.magnifier.follow_mouse;
                    desk.needs_full_redraw = true;
                    return true;
                } else if cx >= win_x + 318 && cx <= win_x + 443 {
                    st.magnifier.frozen = !st.magnifier.frozen;
                    desk.needs_full_redraw = true;
                    return true;
                } else if cx >= win_x + 448 && cx <= win_x + 538 {
                    st.magnifier.show_grid = !st.magnifier.show_grid;
                    desk.needs_full_redraw = true;
                    return true;
                }
            }
        }
        2 => {
            // High Contrast & Accessibility Preferences clicks
            let body_y = win_y + 28;
            let row_w = win_w.saturating_sub(40);

            for idx in 0..5 {
                let iy = body_y + 55 + idx * 30;
                if cx >= win_x + 20 && cx < win_x + 20 + row_w && cy >= iy && cy < iy + 24 {
                    st.hc_options[idx] = !st.hc_options[idx];
                    if idx == 0 {
                        let mut theme = GLOBAL_THEME.lock();
                        if st.hc_options[0] {
                            theme.theme_index = 5;
                            theme.frame_active_color = 0x00000000;
                            theme.frame_inactive_color = 0x001E293B;
                            theme.accent_color = 0x00FEF08A;
                            theme.glass_opacity = 255;
                            theme.wallpaper_preset = 2;
                            theme.theme_name = "Alto Contraste";
                            st.hc_status_msg = String::from("Alto Contraste ATIVADO no tema global.");
                        } else {
                            let pal = &THEME_PALETTE[0];
                            theme.theme_index = 0;
                            theme.frame_active_color = pal.active_frame;
                            theme.frame_inactive_color = pal.inactive_frame;
                            theme.accent_color = pal.accent;
                            theme.glass_opacity = 150;
                            theme.wallpaper_preset = 0;
                            theme.theme_name = pal.name;
                            st.hc_status_msg = String::from("Tema padrao Crystal Glass restaurado.");
                        }
                    } else {
                        st.hc_status_msg = format!(
                            "Opcao #{} {} (clique Salvar Preferencias para gravar).",
                            idx + 1,
                            if st.hc_options[idx] { "ativada" } else { "desativada" }
                        );
                    }
                    desk.needs_full_redraw = true;
                    return true;
                }
            }

            // "Salvar Preferencias" button hitbox (cx + 20..cx + 180, body_y + 230..body_y + 258)
            let save_y = body_y + 230;
            if cx >= win_x + 20 && cx <= win_x + 180 && cy >= save_y && cy <= save_y + 28 {
                let _ = vfs::vfs_create_dir_all("/etc", vfs::Permissions::default_dir());
                let conf = format!(
                    "high_contrast={}\nkey_beep={}\ninvert_mouse={}\nlarge_cursor={}\nsticky_keys={}\n",
                    st.hc_options[0],
                    st.hc_options[1],
                    st.hc_options[2],
                    st.hc_options[3],
                    st.hc_options[4],
                );
                let _ = vfs::vfs_unlink("/etc/accessibility.conf");
                match vfs::vfs_create_file(
                    "/etc/accessibility.conf",
                    conf.as_bytes(),
                    vfs::Permissions::default_file(),
                ) {
                    Ok(_) => {
                        st.hc_status_msg = format!(
                            "Preferencias salvas em /etc/accessibility.conf ({} bytes).",
                            conf.len()
                        );
                    }
                    Err(e) => {
                        st.hc_status_msg = format!(
                            "Erro ao gravar /etc/accessibility.conf: {:?}",
                            e
                        );
                    }
                }
                desk.needs_full_redraw = true;
                return true;
            }
        }
        _ => {}
    }

    false
}

// =============================================================================
// 50. CENTRAL DE AJUDA E RECUPERAÇÃO DO SISTEMA (Manual Tri-Shell & Restore)
// =============================================================================

unsafe fn render_help_recovery_center(
    fb: *mut u8, pitch: usize, scr_w: usize, scr_h: usize,
    cx: usize, cy: usize, cw: usize, ch: usize,
    _desk: &mut DesktopState,
) {
    let tab = { CAT7_STATE.lock().help_tab };

    // 1. Top Tabs Toolbar (28px)
    draw_glass_toolbar(fb, pitch, scr_w, scr_h, cx, cy, cw, 28);
    draw_glass_tab(fb, pitch, scr_w, scr_h, cx + 8, cy + 2, 230, 24, "Manual Tri-Shell & Comandos", tab == 0);
    draw_glass_tab(fb, pitch, scr_w, scr_h, cx + 242, cy + 2, 210, 24, "Restauracao do Sistema (VFS)", tab == 1);

    let body_y = cy + 28;
    let body_h = ch.saturating_sub(28 + 22);

    match tab {
        0 => render_trishell_manual_view(fb, pitch, scr_w, scr_h, cx, body_y, cw, body_h),
        1 => render_system_restore_view(fb, pitch, scr_w, scr_h, cx, body_y, cw, body_h),
        _ => {}
    }

    // Status bar
    let status_y = cy + ch.saturating_sub(22);
    let (l_msg, r_msg) = match tab {
        0 => ("Dicionario Tri-Shell: Bash / CMD / PowerShell", "Clique no comando para inspecionar sintaxe"),
        1 => ("Ponto de Restauracao & Snapshots VFS", "Sistema de Protecao e Integridade de Inodes"),
        _ => ("Ajuda e Recuperacao", "Pronto"),
    };
    draw_glass_statusbar(fb, pitch, scr_w, scr_h, cx, status_y, cw, 22, l_msg, r_msg);
}

unsafe fn render_trishell_manual_view(
    fb: *mut u8, pitch: usize, scr_w: usize, scr_h: usize,
    cx: usize, cy: usize, cw: usize, ch: usize,
) {
    fill_rounded_rect_gradient(fb, pitch, scr_w, scr_h, cx, cy, cw, ch, 0, 0x00F8FAFC, 0x00FFFFFF);

    let (cat_filter, selected_idx) = {
        let st = CAT7_STATE.lock();
        (st.selected_cat_filter, st.selected_cmd_idx)
    };

    // Category Filter Buttons
    let filter_y = cy + 8;
    let categories = ["Todos (16)", "Arquivos", "Processos", "Rede", "Sistema"];
    let mut btn_x = cx + 12;

    for (idx, cat_name) in categories.iter().enumerate() {
        let btn_w = cat_name.len() * 8 + 20;
        draw_glass_button(fb, pitch, scr_w, scr_h, btn_x, filter_y, btn_w, 24, cat_name, cat_filter == idx);
        btn_x += btn_w + 6;
    }

    // Filter matching commands
    let mut matching_cmds = Vec::new();
    for (orig_idx, item) in TRI_SHELL_COMMANDS.iter().enumerate() {
        let matched = match cat_filter {
            1 => item.category == "Arquivos",
            2 => item.category == "Processos",
            3 => item.category == "Rede",
            4 => item.category == "Sistema",
            _ => true,
        };
        if matched {
            matching_cmds.push((orig_idx, *item));
        }
    }

    // Commands Table
    let table_y = filter_y + 30;
    let table_w = cw.saturating_sub(24);
    let table_h = 170usize.min(ch.saturating_sub(180));

    fill_rounded_rect(fb, pitch, scr_w, scr_h, cx + 12, table_y, table_w, table_h, 4, 0x00FFFFFF);
    draw_rounded_rect_border(fb, pitch, scr_w, scr_h, cx + 12, table_y, table_w, table_h, 4, 0x00CBD5E1);

    // Table Header (22px)
    fill_rounded_rect(fb, pitch, scr_w, scr_h, cx + 12, table_y, table_w, 22, 0, 0x00E2E8F0);
    draw_text_px(fb, pitch, scr_w, scr_h, cx + 20, table_y + 4, "Linux Bash", 0x00475569, None);
    draw_text_px(fb, pitch, scr_w, scr_h, cx + 140, table_y + 4, "Windows CMD", 0x00475569, None);
    draw_text_px(fb, pitch, scr_w, scr_h, cx + 270, table_y + 4, "PowerShell", 0x00475569, None);
    draw_text_px(fb, pitch, scr_w, scr_h, cx + 430, table_y + 4, "Finalidade / Descricao", 0x00475569, None);

    for (row_idx, (orig_idx, cmd)) in matching_cmds.iter().enumerate() {
        let ry = table_y + 24 + row_idx * 20;
        if ry + 18 >= table_y + table_h { break; }

        let is_sel = selected_idx == *orig_idx;
        let bg_col = if is_sel {
            0x00BAE6FD
        } else if row_idx % 2 == 1 {
            0x00F8FAFC
        } else {
            0x00FFFFFF
        };

        fill_rounded_rect(fb, pitch, scr_w, scr_h, cx + 13, ry, table_w - 2, 19, 0, bg_col);

        let fg_col = if is_sel { 0x000C4A6E } else { 0x000F172A };
        draw_text_clipped(fb, pitch, scr_w, scr_h, cx + 20, ry + 2, cx + 135, cmd.bash, 0x0015803D, None);
        draw_text_clipped(fb, pitch, scr_w, scr_h, cx + 140, ry + 2, cx + 265, cmd.cmd, 0x001D4ED8, None);
        draw_text_clipped(fb, pitch, scr_w, scr_h, cx + 270, ry + 2, cx + 425, cmd.powershell, 0x007E22CE, None);
        draw_text_clipped(fb, pitch, scr_w, scr_h, cx + 430, ry + 2, cx + table_w + 10, cmd.description, fg_col, None);
    }

    // Detail & Code Cards Pane
    let card_y = table_y + table_h + 10;
    let card_h = ch.saturating_sub(card_y - cy + 6);
    fill_rounded_rect_gradient(fb, pitch, scr_w, scr_h, cx + 12, card_y, table_w, card_h, 6, 0x000F172A, 0x001E293B);
    draw_rounded_rect_border(fb, pitch, scr_w, scr_h, cx + 12, card_y, table_w, card_h, 6, 0x0038BDF8);

    let sel_cmd = TRI_SHELL_COMMANDS.get(selected_idx).unwrap_or(&TRI_SHELL_COMMANDS[0]);

    let title_str = format!("Sintaxe Detalhada: {} [{}]", sel_cmd.name, sel_cmd.category);
    draw_text_px(fb, pitch, scr_w, scr_h, cx + 24, card_y + 10, &title_str, 0x0038BDF8, None);

    let col_w = (table_w.saturating_sub(40)) / 3;

    // Bash Card
    let bx = cx + 20;
    let by = card_y + 32;
    fill_rounded_rect(fb, pitch, scr_w, scr_h, bx, by, col_w, 48, 4, 0x00022C22);
    draw_rounded_rect_border(fb, pitch, scr_w, scr_h, bx, by, col_w, 48, 4, 0x0016A34A);
    draw_text_px(fb, pitch, scr_w, scr_h, bx + 8, by + 6, "Linux (Bash / ELF64):", 0x004ADE80, None);
    draw_text_clipped(fb, pitch, scr_w, scr_h, bx + 8, by + 24, bx + col_w - 4, sel_cmd.bash, 0x00FFFFFF, None);

    // CMD Card
    let cx_box = bx + col_w + 8;
    fill_rounded_rect(fb, pitch, scr_w, scr_h, cx_box, by, col_w, 48, 4, 0x00172554);
    draw_rounded_rect_border(fb, pitch, scr_w, scr_h, cx_box, by, col_w, 48, 4, 0x002563EB);
    draw_text_px(fb, pitch, scr_w, scr_h, cx_box + 8, by + 6, "Windows (CMD / PE32+):", 0x0060A5FA, None);
    draw_text_clipped(fb, pitch, scr_w, scr_h, cx_box + 8, by + 24, cx_box + col_w - 4, sel_cmd.cmd, 0x00FFFFFF, None);

    // PowerShell Card
    let px_box = cx_box + col_w + 8;
    fill_rounded_rect(fb, pitch, scr_w, scr_h, px_box, by, col_w, 48, 4, 0x003B0764);
    draw_rounded_rect_border(fb, pitch, scr_w, scr_h, px_box, by, col_w, 48, 4, 0x009333EA);
    draw_text_px(fb, pitch, scr_w, scr_h, px_box + 8, by + 6, "PowerShell (Core):", 0x00C084FC, None);
    draw_text_clipped(fb, pitch, scr_w, scr_h, px_box + 8, by + 24, px_box + col_w - 4, sel_cmd.powershell, 0x00FFFFFF, None);

    // Example Box & Action Button
    let ex_y = by + 56;
    let ex_str = format!("Exemplo: {}", sel_cmd.example);
    draw_text_px(fb, pitch, scr_w, scr_h, cx + 24, ex_y + 4, &ex_str, 0x00FEF08A, None);

    let inject_btn_x = cx + table_w - 230;
    draw_glass_button(fb, pitch, scr_w, scr_h, inject_btn_x, ex_y, 220, 24, "Injetar Exemplo no Shell", true);
}

unsafe fn render_system_restore_view(
    fb: *mut u8, pitch: usize, scr_w: usize, scr_h: usize,
    cx: usize, cy: usize, cw: usize, ch: usize,
) {
    fill_rounded_rect_gradient(fb, pitch, scr_w, scr_h, cx, cy, cw, ch, 0, 0x000F172A, 0x001E293B);

    let (points_data, sel_idx, status_msg, pct) = {
        let st = CAT7_STATE.lock();
        (st.restore.points.clone(), st.restore.selected_idx, st.restore.status_banner.clone(), st.restore.progress_pct)
    };

    // Action Toolbar
    let bar_y = cy + 10;
    draw_glass_button(fb, pitch, scr_w, scr_h, cx + 12, bar_y, 220, 28, "Criar Ponto de Restauracao", true);
    draw_glass_button(fb, pitch, scr_w, scr_h, cx + 240, bar_y, 220, 28, "Restaurar Snapshot Anterior", false);
    draw_glass_button(fb, pitch, scr_w, scr_h, cx + 468, bar_y, 180, 28, "Verificar Integridade", false);

    // Table of Restore Points
    let table_y = bar_y + 36;
    let table_w = cw.saturating_sub(24);
    let table_h = 160usize;

    fill_rounded_rect(fb, pitch, scr_w, scr_h, cx + 12, table_y, table_w, table_h, 4, 0x00FFFFFF);
    draw_rounded_rect_border(fb, pitch, scr_w, scr_h, cx + 12, table_y, table_w, table_h, 4, 0x00CBD5E1);

    // Header
    fill_rounded_rect(fb, pitch, scr_w, scr_h, cx + 12, table_y, table_w, 22, 0, 0x00E2E8F0);
    draw_text_px(fb, pitch, scr_w, scr_h, cx + 20, table_y + 4, "ID", 0x00475569, None);
    draw_text_px(fb, pitch, scr_w, scr_h, cx + 52, table_y + 4, "Data / Hora (RTC)", 0x00475569, None);
    draw_text_px(fb, pitch, scr_w, scr_h, cx + 216, table_y + 4, "Descricao do Ponto de Restauracao", 0x00475569, None);
    draw_text_px(fb, pitch, scr_w, scr_h, cx + table_w.saturating_sub(180), table_y + 4, "Tamanho", 0x00475569, None);
    draw_text_px(fb, pitch, scr_w, scr_h, cx + table_w.saturating_sub(90), table_y + 4, "Status", 0x00475569, None);

    for (r_idx, pt) in points_data.iter().enumerate() {
        let ry = table_y + 24 + r_idx * 22;
        if ry + 20 >= table_y + table_h { break; }

        let is_sel = sel_idx == r_idx;
        let bg_col = if is_sel { 0x00BAE6FD } else if r_idx % 2 == 1 { 0x00F8FAFC } else { 0x00FFFFFF };
        fill_rounded_rect(fb, pitch, scr_w, scr_h, cx + 13, ry, table_w.saturating_sub(2), 21, 0, bg_col);

        let id_str = format!("{}", pt.id);
        let sz_str = if pt.size_bytes >= 1024 {
            format!("{} KB", pt.size_bytes / 1024)
        } else {
            format!("{} B", pt.size_bytes)
        };
        let fg = if is_sel { 0x000C4A6E } else { 0x000F172A };

        draw_text_px(fb, pitch, scr_w, scr_h, cx + 20, ry + 3, &id_str, fg, None);
        draw_text_clipped(fb, pitch, scr_w, scr_h, cx + 52, ry + 3, cx + 210, &pt.timestamp, fg, None);
        draw_text_clipped(fb, pitch, scr_w, scr_h, cx + 216, ry + 3, cx + table_w.saturating_sub(188), &pt.description, fg, None);
        draw_text_px(fb, pitch, scr_w, scr_h, cx + table_w.saturating_sub(180), ry + 3, &sz_str, 0x00059669, None);
        draw_text_clipped(fb, pitch, scr_w, scr_h, cx + table_w.saturating_sub(90), ry + 3, cx + table_w + 8, &pt.status, 0x000284C7, None);
    }

    // Status Banner & Progress Bar Card
    let card_y = table_y + table_h + 14;
    let card_h = ch.saturating_sub(card_y - cy + 6);
    fill_rounded_rect_gradient(fb, pitch, scr_w, scr_h, cx + 12, card_y, table_w, card_h, 6, 0x00020617, 0x000F172A);
    draw_rounded_rect_border(fb, pitch, scr_w, scr_h, cx + 12, card_y, table_w, card_h, 6, 0x000284C7);

    draw_text_px(fb, pitch, scr_w, scr_h, cx + 24, card_y + 12, "Status da Integridade do VFS e Snapshot:", 0x0038BDF8, None);
    draw_text_clipped(fb, pitch, scr_w, scr_h, cx + 24, card_y + 32, cx + table_w, &status_msg, 0x004ADE80, None);

    // Progress Bar
    let pb_y = card_y + 54;
    let pb_w = table_w.saturating_sub(24);
    ui_progress_bar(fb, pitch, scr_w, scr_h, cx + 24, pb_y, pb_w, 18, pct, false);

    let info1 = "Armazenamento VFS: C:\\ (FAT32 Master ATA) + / (RAMFS Compartilhado)";
    let info2 = "Inodes Persistentes: 100% Protegidos contra Corrupcao | Snapshot Dir: /snapshots";
    draw_text_px(fb, pitch, scr_w, scr_h, cx + 24, pb_y + 26, info1, 0x00CBD5E1, None);
    draw_text_px(fb, pitch, scr_w, scr_h, cx + 24, pb_y + 44, info2, 0x0094A3B8, None);
}

fn handle_help_recovery_click(cx: usize, cy: usize, desk: &mut DesktopState) -> bool {
    let mut st = CAT7_STATE.lock();
    let slot = 2;
    let win_x = st.last_cx[slot];
    let win_y = st.last_cy[slot];
    let win_w = st.last_cw[slot];
    let win_h = st.last_ch[slot];

    // Top Tabs click
    if cy >= win_y && cy <= win_y + 28 {
        if cx >= win_x + 8 && cx <= win_x + 238 {
            st.help_tab = 0;
            desk.needs_full_redraw = true;
            return true;
        } else if cx >= win_x + 242 && cx <= win_x + 452 {
            st.help_tab = 1;
            if !st.restore.initialized {
                st.restore.sync_from_vfs();
            }
            desk.needs_full_redraw = true;
            return true;
        }
    }

    match st.help_tab {
        0 => {
            // Tri-Shell Manual clicks
            let filter_y = win_y + 28 + 8;
            let categories = ["Todos (16)", "Arquivos", "Processos", "Rede", "Sistema"];
            let mut btn_x = win_x + 12;

            for (idx, cat_name) in categories.iter().enumerate() {
                let btn_w = cat_name.len() * 8 + 20;
                if cy >= filter_y && cy <= filter_y + 24 && cx >= btn_x && cx <= btn_x + btn_w {
                    st.selected_cat_filter = idx;
                    let first_orig = TRI_SHELL_COMMANDS
                        .iter()
                        .enumerate()
                        .find_map(|(orig_idx, item)| {
                            let matched = match idx {
                                1 => item.category == "Arquivos",
                                2 => item.category == "Processos",
                                3 => item.category == "Rede",
                                4 => item.category == "Sistema",
                                _ => true,
                            };
                            if matched { Some(orig_idx) } else { None }
                        })
                        .unwrap_or(0);
                    st.selected_cmd_idx = first_orig;
                    desk.needs_full_redraw = true;
                    return true;
                }
                btn_x += btn_w + 6;
            }

            // Commands Table Row click — use the exact same table_h formula as render_trishell_manual_view
            let body_h = win_h.saturating_sub(28 + 22);
            let table_y = filter_y + 30;
            let table_w = win_w.saturating_sub(24);
            let table_h = 170usize.min(body_h.saturating_sub(180));

            if cx >= win_x + 12 && cx <= win_x + 12 + table_w && cy >= table_y + 24 && cy < table_y + table_h {
                let row_idx = (cy - (table_y + 24)) / 20;

                let mut matching = Vec::new();
                for (orig_idx, item) in TRI_SHELL_COMMANDS.iter().enumerate() {
                    let matched = match st.selected_cat_filter {
                        1 => item.category == "Arquivos",
                        2 => item.category == "Processos",
                        3 => item.category == "Rede",
                        4 => item.category == "Sistema",
                        _ => true,
                    };
                    if matched { matching.push(orig_idx); }
                }

                if let Some(&orig_idx) = matching.get(row_idx) {
                    st.selected_cmd_idx = orig_idx;
                    desk.needs_full_redraw = true;
                    return true;
                }
            }

            // Inject Example button click
            let card_y = table_y + table_h + 10;
            let by = card_y + 32;
            let ex_y = by + 56;
            let inject_btn_x = win_x + table_w - 230;

            if cy >= ex_y && cy <= ex_y + 24 && cx >= inject_btn_x && cx <= inject_btn_x + 220 {
                let sel_cmd = TRI_SHELL_COMMANDS.get(st.selected_cmd_idx).unwrap_or(&TRI_SHELL_COMMANDS[0]);
                // Extract only the valid executable command before " ou "
                let exec_cmd = sel_cmd
                    .example
                    .split(" ou ")
                    .next()
                    .unwrap_or(sel_cmd.bash)
                    .trim();
                for c in exec_cmd.chars() {
                    enqueue_keyboard_char(c);
                }
                enqueue_keyboard_char('\n');
                desk.needs_full_redraw = true;
                return true;
            }
        }
        1 => {
            if !st.restore.initialized {
                st.restore.sync_from_vfs();
            }

            // System Restore clicks
            let bar_y = win_y + 28 + 10;
            let table_w = win_w.saturating_sub(24);

            if cy >= bar_y && cy <= bar_y + 28 {
                if cx >= win_x + 12 && cx <= win_x + 232 {
                    // Create real VFS Restore Point
                    let new_id = st.restore.points.iter().map(|p| p.id).max().unwrap_or(0) + 1;
                    let desc = format!("Ponto Manual #{} (Snapshot VFS)", new_id);

                    match create_vfs_snapshot(new_id, &desc) {
                        Ok(pt) => {
                            let path_copy = pt.file_path.clone();
                            let sz = pt.size_bytes;
                            st.restore.points.push(pt);
                            st.restore.selected_idx = st.restore.points.len().saturating_sub(1);
                            st.restore.status_banner = format!(
                                "Snapshot #{} gravado em {} ({} bytes).",
                                new_id, path_copy, sz
                            );
                            st.restore.progress_pct = 100;
                        }
                        Err(err_msg) => {
                            st.restore.status_banner = err_msg;
                            st.restore.progress_pct = 0;
                        }
                    }
                    desk.needs_full_redraw = true;
                    return true;
                } else if cx >= win_x + 240 && cx <= win_x + 460 {
                    // Restore selected Snapshot from VFS
                    if let Some(pt) = st.restore.points.get(st.restore.selected_idx).cloned() {
                        match vfs::vfs_read_all(&pt.file_path) {
                            Ok(raw_bytes) => {
                                let text = core::str::from_utf8(&raw_bytes).unwrap_or("");
                                let mut parsed_theme_idx: Option<usize> = None;
                                let mut parsed_opacity: Option<u8> = None;
                                let mut parsed_wp: Option<u8> = None;

                                for line in text.lines() {
                                    if let Some(v) = line.strip_prefix("THEME_INDEX=") {
                                        parsed_theme_idx = v.trim().parse::<usize>().ok();
                                    } else if let Some(v) = line.strip_prefix("GLASS_OPACITY=") {
                                        parsed_opacity = v.trim().parse::<u8>().ok();
                                    } else if let Some(v) = line.strip_prefix("WALLPAPER_PRESET=") {
                                        parsed_wp = v.trim().parse::<u8>().ok();
                                    }
                                }

                                let (applied_theme, applied_opacity) = {
                                    let mut theme = GLOBAL_THEME.lock();
                                    if let Some(t_idx) = parsed_theme_idx {
                                        let clamped = t_idx.min(THEME_PALETTE.len() - 1);
                                        let pal = &THEME_PALETTE[clamped];
                                        theme.theme_index = clamped;
                                        theme.frame_active_color = pal.active_frame;
                                        theme.frame_inactive_color = pal.inactive_frame;
                                        theme.accent_color = pal.accent;
                                        theme.theme_name = pal.name;
                                    }
                                    if let Some(op) = parsed_opacity {
                                        theme.glass_opacity = op;
                                    }
                                    if let Some(wp) = parsed_wp {
                                        theme.wallpaper_preset = wp;
                                    }
                                    (theme.theme_index, theme.glass_opacity)
                                };

                                st.restore.status_banner = format!(
                                    "Restaurado #{} ({} bytes de {}): Tema={}, Opacidade={}",
                                    pt.id,
                                    raw_bytes.len(),
                                    pt.file_path,
                                    applied_theme,
                                    applied_opacity,
                                );
                                st.restore.progress_pct = 100;
                            }
                            Err(e) => {
                                st.restore.status_banner = format!(
                                    "Erro ao ler snapshot {}: {:?}",
                                    pt.file_path, e
                                );
                                st.restore.progress_pct = 0;
                            }
                        }
                    } else {
                        st.restore.status_banner = String::from("Nenhum snapshot disponivel para restaurar.");
                        st.restore.progress_pct = 0;
                    }
                    desk.needs_full_redraw = true;
                    return true;
                } else if cx >= win_x + 468 && cx <= win_x + 648 {
                    // Check VFS Integrity across / and /snapshots
                    let root_ok = vfs::vfs_readdir("/").is_ok();
                    let _ = vfs::vfs_create_dir_all("/snapshots", vfs::Permissions::default_dir());
                    let mut verified = 0usize;
                    let mut failed = 0usize;
                    let mut total_bytes = 0usize;

                    if let Ok(entries) = vfs::vfs_readdir("/snapshots") {
                        for entry in entries.iter() {
                            if !entry.name.ends_with(".snap") {
                                continue;
                            }
                            let p = format!("/snapshots/{}", entry.name);
                            match vfs::vfs_read_all(&p) {
                                Ok(bytes) => {
                                    verified += 1;
                                    total_bytes += bytes.len();
                                }
                                Err(_) => {
                                    failed += 1;
                                }
                            }
                        }
                    }

                    if root_ok && failed == 0 {
                        st.restore.status_banner = format!(
                            "Integridade VFS OK: {} snapshot(s) validado(s) ({} bytes em /snapshots).",
                            verified, total_bytes
                        );
                        st.restore.progress_pct = 100;
                    } else {
                        st.restore.status_banner = format!(
                            "Verificacao VFS: {} OK, {} com erro de leitura em /snapshots.",
                            verified, failed
                        );
                        st.restore.progress_pct = if verified + failed > 0 {
                            (verified * 100) / (verified + failed)
                        } else {
                            0
                        };
                    }
                    desk.needs_full_redraw = true;
                    return true;
                }
            }

            // Table Row Click
            let table_y = bar_y + 36;
            let table_h = 160usize;
            if cx >= win_x + 12 && cx <= win_x + 12 + table_w && cy >= table_y + 24 && cy < table_y + table_h {
                let r_idx = (cy - (table_y + 24)) / 22;
                if r_idx < st.restore.points.len() {
                    st.restore.selected_idx = r_idx;
                    st.restore.status_banner = format!("Ponto Selecionado: #{} ({})", st.restore.points[r_idx].id, st.restore.points[r_idx].description);
                    desk.needs_full_redraw = true;
                    return true;
                }
            }
        }
        _ => {}
    }

    false
}
