//! HybridOS Native Applications Suite — Category 3: Terminals, Execution & App Store (19 to 25).
//!
//! Implements completely bare-metal Crystal Glass native views and real execution for:
//! 19. Terminal Tri-Shell Gráfico (Bash, CMD, PowerShell com buffer de rolagem e interpretador unificado)
//! 20. Caixa Executar / Run Dialog (Histórico combobox, VFS browser, dispatch de GUI e CLI)
//! 21. Central de Programas / App Store & Software Hub (Catálogo em abas, busca e gravação real no VFS)
//! 22. Assistente de Instalação / Setup Wizard (3 etapas: EULA, diretório VFS, extração animada com arquivos reais)
//! 23. Programas Padrão e Associações de Extensões (.txt, .sh, .bat, .bmp, .bin, .elf, .exe com persistência no VFS)
//! 24. Centro de Compatibilidade Dual-ABI (Inspetor de binários ELF64 vs PE32+, seções, imports e spawn_process)
//! 25. Editor de Variáveis de Ambiente (Variáveis de usuário e sistema com persistência em /etc/environment)

#[allow(unused_imports)]
use alloc::format;
#[allow(unused_imports)]
use alloc::string::{String, ToString};
#[allow(unused_imports)]
use alloc::vec::Vec;
#[allow(unused_imports)]
use alloc::vec;

use crate::arch::idt::SYSTEM_TICKS;
use crate::drivers::desktop::{DesktopState, AppId, draw_text_clipped, draw_text_px};
use crate::drivers::gfx2d::*;
use crate::drivers::pop_keyboard_char;
use crate::personality::ExecutionPersonality;
use crate::security::sha256::sha256_hex;
use crate::sync::SpinLock;
use core::sync::atomic::Ordering;
use crate::vfs::{self, Permissions};
use crate::loaders::{spawn_elf_process, spawn_pe_process};
use crate::userland::shell::{create_synthetic_elf, create_synthetic_pe, HybridShell, ShellFlavor};

// =============================================================================
// DATA TYPES & DATA STRUCTURES FOR CATEGORY 3
// =============================================================================

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ShellMode {
    Bash,
    Cmd,
    PowerShell,
}

#[derive(Clone, Copy, Debug)]
pub struct SoftwarePackage {
    pub id: &'static str,
    pub name: &'static str,
    pub version: &'static str,
    pub category: usize, // 0: Destaques, 1: Ferramentas, 2: Jogos, 3: Desenvolvedor
    pub size_str: &'static str,
    pub abi_type: &'static str,
    pub desc: &'static str,
    pub bin_name: &'static str,
    pub win_path: &'static str,
    pub is_elf: bool,
}

pub static SOFTWARE_CATALOG: &[SoftwarePackage] = &[
    SoftwarePackage {
        id: "chrome",
        name: "Google Chrome",
        version: "124.0.6367",
        category: 0,
        size_str: "48.2 MB",
        abi_type: "Dual-ABI",
        desc: "Navegador Web ultra-veloz com engine V8 e isolamento Ring 3.",
        bin_name: "chrome",
        win_path: "C:\\Program Files\\Google\\Chrome\\chrome.exe",
        is_elf: false,
    },
    SoftwarePackage {
        id: "vscode",
        name: "Visual Studio Code",
        version: "1.88.1",
        category: 0,
        size_str: "82.5 MB",
        abi_type: "Dual-ABI",
        desc: "Editor de codigo moderno com suporte a extensoes e debugger.",
        bin_name: "code",
        win_path: "C:\\Program Files\\Microsoft VS Code\\code.exe",
        is_elf: true,
    },
    SoftwarePackage {
        id: "7zip",
        name: "7-Zip Archiver",
        version: "23.01",
        category: 1,
        size_str: "4.2 MB",
        abi_type: "Windows PE32+",
        desc: "Compactador e descompactador com alta taxa de compressao LZMA/ZIP.",
        bin_name: "7z",
        win_path: "C:\\Program Files\\7-Zip\\7z.exe",
        is_elf: false,
    },
    SoftwarePackage {
        id: "vlc",
        name: "VLC Media Player",
        version: "3.0.20",
        category: 1,
        size_str: "28.4 MB",
        abi_type: "Linux ELF64",
        desc: "Reprodutor multimidia universal com suporte a codecs e som HDA.",
        bin_name: "vlc",
        win_path: "C:\\Program Files\\VideoLAN\\VLC\\vlc.exe",
        is_elf: true,
    },
    SoftwarePackage {
        id: "directx",
        name: "DirectX Runtime",
        version: "11.2",
        category: 2,
        size_str: "35.1 MB",
        abi_type: "Windows PE32+",
        desc: "Bibliotecas graficas Direct3D 9/11 e DirectInput para aceleracao 3D.",
        bin_name: "dxdiag",
        win_path: "C:\\Windows\\System32\\dxdiag.exe",
        is_elf: false,
    },
    SoftwarePackage {
        id: "doom",
        name: "Doom Classic 1993",
        version: "1.9",
        category: 2,
        size_str: "12.8 MB",
        abi_type: "Linux ELF64",
        desc: "Port nativo x86_64 do classico da id Software para o Kernel Ring 0.",
        bin_name: "doom",
        win_path: "C:\\Games\\Doom\\doom.exe",
        is_elf: true,
    },
    SoftwarePackage {
        id: "git",
        name: "Git SCM",
        version: "2.44.0",
        category: 3,
        size_str: "18.6 MB",
        abi_type: "Linux ELF64",
        desc: "Controle de versao distribuido e gestao de branches em linha de comando.",
        bin_name: "git",
        win_path: "C:\\Program Files\\Git\\bin\\git.exe",
        is_elf: true,
    },
    SoftwarePackage {
        id: "rustc",
        name: "Rust Toolchain",
        version: "1.78.0",
        category: 3,
        size_str: "95.0 MB",
        abi_type: "Dual-ABI",
        desc: "Compilador rustc nativo e gerenciador cargo para HybridOS.",
        bin_name: "rustc",
        win_path: "C:\\Program Files\\Rust\\bin\\rustc.exe",
        is_elf: true,
    },
];

pub struct ExtMapping {
    pub ext: &'static str,
    pub desc: &'static str,
    pub app_name: String,
    pub subsystem: &'static str,
}

pub struct BinaryInspection {
    pub path: String,
    pub is_elf: bool,
    pub is_pe: bool,
    pub format_name: String,
    pub magic_hex: String,
    pub machine_desc: String,
    pub entry_point_hex: String,
    pub image_base_hex: String,
    pub subsystem_desc: String,
    pub sections: Vec<(String, String, String, String)>, // (Name, Type, Size, Flags)
    pub imports: Vec<String>,
}

pub static SAMPLE_BIN_PATHS: &[&str] = &[
    "/bin/init",
    "/bin/sh",
    "C:\\Windows\\System32\\cmd.exe",
    "/bin/sample.elf",
    "C:\\sample.exe",
];

pub static SAMPLE_BIN_LABELS: &[&str] = &[
    "init",
    "sh",
    "cmd.exe",
    "sample.elf",
    "sample.exe",
];

pub struct Cat3State {
    pub last_canvas_rect: (usize, usize, usize, usize), // (cx, cy, cw, ch)

    // 19. Terminal Tri-Shell
    pub shell_mode: ShellMode,
    pub term_history: Vec<String>,
    pub term_scroll: usize,
    pub term_input: String,
    pub shell_status: String,

    // 20. Run Dialog
    pub run_input: String,
    pub run_history: Vec<String>,
    pub run_status: String,
    pub browse_index: usize,

    // 21. App Store
    pub store_tab: usize,
    pub store_search: String,
    pub store_search_focused: bool,
    pub installed_apps: Vec<String>,
    pub store_status: String,

    // 22. Setup Wizard
    pub wizard_step: usize, // 1: EULA, 2: Directory, 3: Extraction/Done
    pub wizard_eula_accepted: bool,
    pub wizard_target_dir: String,
    pub wizard_progress: usize,
    pub wizard_status: String,

    // 23. Default Apps
    pub selected_ext_idx: usize,
    pub ext_mappings: Vec<ExtMapping>,
    pub ext_status: String,

    // 24. Dual-ABI Compatibility Center
    pub selected_bin_chip: usize,
    pub inspection: Option<BinaryInspection>,
    pub abi_status: String,

    // 25. Environment Variables
    pub selected_env_idx: usize,
    pub is_user_env_selected: bool,
    pub user_env: Vec<(String, String)>,
    pub sys_env: Vec<(String, String)>,
    pub env_status: String,
}

fn default_ext_mappings() -> Vec<ExtMapping> {
    vec![
        ExtMapping { ext: ".txt",  desc: "Documento de Texto Simples",   app_name: String::from("Bloco de Notas (Notepad)"), subsystem: "WinNT / POSIX" },
        ExtMapping { ext: ".sh",   desc: "Script de Shell Unix/POSIX",   app_name: String::from("Terminal Tri-Shell (Bash)"), subsystem: "Linux POSIX" },
        ExtMapping { ext: ".bat",  desc: "Arquivo de Lote CMD Windows",  app_name: String::from("Terminal Tri-Shell (CMD)"), subsystem: "Windows NT" },
        ExtMapping { ext: ".bmp",  desc: "Imagem Bitmap Raster",         app_name: String::from("Visualizador de Midia"), subsystem: "Crystal UI" },
        ExtMapping { ext: ".bin",  desc: "Imagem Binaria Bruta / Hex",   app_name: String::from("Descompactador / Hex"), subsystem: "Kernel Raw" },
        ExtMapping { ext: ".elf",  desc: "Binario Executavel Linux",     app_name: String::from("Carregador ELF64 POSIX"), subsystem: "Linux POSIX" },
        ExtMapping { ext: ".exe",  desc: "Executavel Portavel Windows",  app_name: String::from("Carregador PE32+ WinNT"), subsystem: "Windows NT" },
        ExtMapping { ext: ".conf", desc: "Arquivo de Configuracao",      app_name: String::from("Bloco de Notas (Notepad)"), subsystem: "WinNT / POSIX" },
        ExtMapping { ext: ".log",  desc: "Relatorio de Log do Sistema",  app_name: String::from("Visualizador de Eventos"), subsystem: "WinNT / POSIX" },
    ]
}

fn default_user_env() -> Vec<(String, String)> {
    vec![
        (String::from("USER"), String::from("Administrator")),
        (String::from("HOME"), String::from("/home/root")),
        (String::from("USERPROFILE"), String::from("C:\\Users\\Administrator")),
        (String::from("LANG"), String::from("pt_BR.UTF-8")),
        (String::from("APPDATA"), String::from("C:\\Users\\Administrator\\AppData\\Roaming")),
    ]
}

fn default_sys_env() -> Vec<(String, String)> {
    vec![
        (String::from("PATH"), String::from("/bin:/usr/bin:C:\\Windows:C:\\Windows\\System32")),
        (String::from("SYSTEMROOT"), String::from("C:\\Windows")),
        (String::from("COMSPEC"), String::from("C:\\Windows\\System32\\cmd.exe")),
        (String::from("SHELL"), String::from("/bin/bash")),
        (String::from("OS"), String::from("HybridOS_x86_64")),
        (String::from("NUMBER_OF_PROCESSORS"), String::from("4")),
        (String::from("TEMP"), String::from("C:\\Temp")),
        (String::from("TMPDIR"), String::from("/tmp")),
    ]
}

/// Builds a valid minimal ELF64 (x86_64) binary with real program & section headers for VFS inspection.
fn build_sample_elf64_binary() -> Vec<u8> {
    let mut elf = vec![0u8; 512];
    // e_ident
    elf[0..4].copy_from_slice(b"\x7fELF");
    elf[4] = 2; // ELFCLASS64
    elf[5] = 1; // ELFDATA2LSB
    elf[6] = 1; // EV_CURRENT
    elf[7] = 0; // ELFOSABI_NONE (System V)

    // e_type = ET_EXEC (2), e_machine = EM_X86_64 (62 = 0x3E), e_version = 1
    elf[16..18].copy_from_slice(&2u16.to_le_bytes());
    elf[18..20].copy_from_slice(&62u16.to_le_bytes());
    elf[20..24].copy_from_slice(&1u32.to_le_bytes());

    // e_entry = 0x0000_0000_0040_1000
    elf[24..32].copy_from_slice(&0x0000_0000_0040_1000u64.to_le_bytes());
    // e_phoff = 64, e_shoff = 160
    elf[32..40].copy_from_slice(&64u64.to_le_bytes());
    elf[40..48].copy_from_slice(&160u64.to_le_bytes());
    // e_ehsize = 64, e_phentsize = 56, e_phnum = 1
    elf[52..54].copy_from_slice(&64u16.to_le_bytes());
    elf[54..56].copy_from_slice(&56u16.to_le_bytes());
    elf[56..58].copy_from_slice(&1u16.to_le_bytes());
    // e_shentsize = 64, e_shnum = 4, e_shstrndx = 3
    elf[58..60].copy_from_slice(&64u16.to_le_bytes());
    elf[60..62].copy_from_slice(&4u16.to_le_bytes());
    elf[62..64].copy_from_slice(&3u16.to_le_bytes());

    // Program Header 0 at offset 64 (56 bytes): PT_LOAD
    let ph = 64;
    elf[ph..ph + 4].copy_from_slice(&1u32.to_le_bytes()); // PT_LOAD
    elf[ph + 4..ph + 8].copy_from_slice(&5u32.to_le_bytes()); // PF_R | PF_X
    elf[ph + 8..ph + 16].copy_from_slice(&0u64.to_le_bytes()); // p_offset
    elf[ph + 16..ph + 24].copy_from_slice(&0x0040_0000u64.to_le_bytes()); // p_vaddr
    elf[ph + 24..ph + 32].copy_from_slice(&0x0040_0000u64.to_le_bytes()); // p_paddr
    elf[ph + 32..ph + 40].copy_from_slice(&512u64.to_le_bytes()); // p_filesz
    elf[ph + 40..ph + 48].copy_from_slice(&0x3000u64.to_le_bytes()); // p_memsz
    elf[ph + 48..ph + 56].copy_from_slice(&0x1000u64.to_le_bytes()); // p_align

    // .shstrtab at offset 120 (25 bytes): "\0.text\0.rodata\0.shstrtab\0"
    let shstrtab = b"\0.text\0.rodata\0.shstrtab\0";
    elf[120..120 + shstrtab.len()].copy_from_slice(shstrtab);

    // Section Header Table at offset 160 (4 * 64 = 256 bytes -> ends at 416)
    // Section 0 (160..224): SHT_NULL (zeros)
    // Section 1 (224..288): .text (sh_name=1)
    let s1 = 224;
    elf[s1..s1 + 4].copy_from_slice(&1u32.to_le_bytes()); // sh_name
    elf[s1 + 4..s1 + 8].copy_from_slice(&1u32.to_le_bytes()); // SHT_PROGBITS
    elf[s1 + 8..s1 + 16].copy_from_slice(&0x6u64.to_le_bytes()); // SHF_ALLOC | SHF_EXECINSTR
    elf[s1 + 16..s1 + 24].copy_from_slice(&0x0040_1000u64.to_le_bytes()); // sh_addr
    elf[s1 + 24..s1 + 32].copy_from_slice(&416u64.to_le_bytes()); // sh_offset
    elf[s1 + 32..s1 + 40].copy_from_slice(&64u64.to_le_bytes()); // sh_size

    // Section 2 (288..352): .rodata (sh_name=7)
    let s2 = 288;
    elf[s2..s2 + 4].copy_from_slice(&7u32.to_le_bytes()); // sh_name
    elf[s2 + 4..s2 + 8].copy_from_slice(&1u32.to_le_bytes()); // SHT_PROGBITS
    elf[s2 + 8..s2 + 16].copy_from_slice(&0x2u64.to_le_bytes()); // SHF_ALLOC
    elf[s2 + 16..s2 + 24].copy_from_slice(&0x0040_2000u64.to_le_bytes()); // sh_addr
    elf[s2 + 24..s2 + 32].copy_from_slice(&480u64.to_le_bytes()); // sh_offset
    elf[s2 + 32..s2 + 40].copy_from_slice(&32u64.to_le_bytes()); // sh_size

    // Section 3 (352..416): .shstrtab (sh_name=15)
    let s3 = 352;
    elf[s3..s3 + 4].copy_from_slice(&15u32.to_le_bytes()); // sh_name
    elf[s3 + 4..s3 + 8].copy_from_slice(&3u32.to_le_bytes()); // SHT_STRTAB
    elf[s3 + 8..s3 + 16].copy_from_slice(&0u64.to_le_bytes()); // sh_flags
    elf[s3 + 24..s3 + 32].copy_from_slice(&120u64.to_le_bytes()); // sh_offset
    elf[s3 + 32..s3 + 40].copy_from_slice(&(shstrtab.len() as u64).to_le_bytes()); // sh_size

    // Minimal x86_64 exit(0) machine code in .text (416..425)
    let code: [u8; 9] = [0xB8, 0x3C, 0x00, 0x00, 0x00, 0x31, 0xFF, 0x0F, 0x05];
    elf[416..416 + code.len()].copy_from_slice(&code);

    elf
}

/// Builds a valid minimal PE32+ (Windows x86_64) binary with real COFF, Optional & Section headers.
fn build_sample_pe32plus_binary() -> Vec<u8> {
    let mut pe = vec![0u8; 512];
    // DOS Header
    pe[0] = b'M';
    pe[1] = b'Z';
    pe[0x3C..0x40].copy_from_slice(&128u32.to_le_bytes()); // e_lfanew = 128

    // PE Signature at 128
    let pe_off = 128;
    pe[pe_off..pe_off + 4].copy_from_slice(b"PE\0\0");

    // COFF File Header at 132 (20 bytes)
    let coff = pe_off + 4;
    pe[coff..coff + 2].copy_from_slice(&0x8664u16.to_le_bytes()); // IMAGE_FILE_MACHINE_AMD64
    pe[coff + 2..coff + 4].copy_from_slice(&3u16.to_le_bytes()); // NumberOfSections = 3
    pe[coff + 16..coff + 18].copy_from_slice(&240u16.to_le_bytes()); // SizeOfOptionalHeader = 240
    pe[coff + 18..coff + 20].copy_from_slice(&0x0022u16.to_le_bytes()); // EXECUTABLE_IMAGE | LARGE_ADDRESS_AWARE

    // Optional Header (PE32+) at 152 (240 bytes -> ends at 392)
    let opt = coff + 20;
    pe[opt..opt + 2].copy_from_slice(&0x020Bu16.to_le_bytes()); // PE32+ Magic
    pe[opt + 16..opt + 20].copy_from_slice(&0x1000u32.to_le_bytes()); // AddressOfEntryPoint
    pe[opt + 24..opt + 32].copy_from_slice(&0x0000_0001_4000_0000u64.to_le_bytes()); // ImageBase
    pe[opt + 32..opt + 36].copy_from_slice(&0x1000u32.to_le_bytes()); // SectionAlignment
    pe[opt + 36..opt + 40].copy_from_slice(&0x200u32.to_le_bytes()); // FileAlignment
    pe[opt + 56..opt + 60].copy_from_slice(&0x8000u32.to_le_bytes()); // SizeOfImage
    pe[opt + 60..opt + 64].copy_from_slice(&0x200u32.to_le_bytes()); // SizeOfHeaders
    pe[opt + 68..opt + 70].copy_from_slice(&3u16.to_le_bytes()); // IMAGE_SUBSYSTEM_WINDOWS_CUI

    // IMAGE_SECTION_HEADER table at 392 (3 * 40 = 120 bytes -> ends at 512)
    let secs: [([u8; 8], u32, u32, u32, u32, u32); 3] = [
        (*b".text\0\0\0", 0x1000, 0x1000, 0x0200, 0x0200, 0x6000_0020),
        (*b".rdata\0\0", 0x0400, 0x2000, 0x0200, 0x0400, 0x4000_0040),
        (*b".data\0\0\0", 0x0200, 0x3000, 0x0200, 0x0600, 0xC000_0040),
    ];
    for (idx, (name, vsize, vaddr, raw_sz, raw_ptr, chars)) in secs.iter().enumerate() {
        let off = opt + 240 + idx * 40;
        pe[off..off + 8].copy_from_slice(name);
        pe[off + 8..off + 12].copy_from_slice(&vsize.to_le_bytes());
        pe[off + 12..off + 16].copy_from_slice(&vaddr.to_le_bytes());
        pe[off + 16..off + 20].copy_from_slice(&raw_sz.to_le_bytes());
        pe[off + 20..off + 24].copy_from_slice(&raw_ptr.to_le_bytes());
        pe[off + 36..off + 40].copy_from_slice(&chars.to_le_bytes());
    }

    pe
}

fn read_cstr_from_slice(table: &[u8], offset: usize) -> Option<String> {
    if offset >= table.len() {
        return None;
    }
    let slice = &table[offset..];
    let end = slice.iter().position(|&b| b == 0).unwrap_or(slice.len());
    let name_bytes = &slice[..end];
    if name_bytes.is_empty() {
        return None;
    }
    let mut s = String::new();
    for &b in name_bytes {
        if b.is_ascii_graphic() || b == b' ' {
            s.push(b as char);
        }
    }
    if s.is_empty() { None } else { Some(s) }
}

fn inspect_target_path(path: &str) -> BinaryInspection {
    let data = match vfs::vfs_read_all(path) {
        Ok(bytes) => bytes,
        Err(_) => {
            return BinaryInspection {
                path: String::from(path),
                is_elf: false,
                is_pe: false,
                format_name: String::from("Arquivo nao encontrado no VFS"),
                magic_hex: String::from("--"),
                machine_desc: String::from("--"),
                entry_point_hex: String::from("--"),
                image_base_hex: String::from("--"),
                subsystem_desc: String::from("Arquivo nao encontrado no VFS"),
                sections: Vec::new(),
                imports: vec![String::from("Arquivo nao encontrado no VFS")],
            };
        }
    };

    if data.len() >= 64 && &data[0..4] == b"\x7fELF" {
        let ei_class = data[4];
        let e_machine = u16::from_le_bytes([data[18], data[19]]);
        let e_entry = u64::from_le_bytes([
            data[24], data[25], data[26], data[27],
            data[28], data[29], data[30], data[31],
        ]);
        let e_phoff = u64::from_le_bytes([
            data[32], data[33], data[34], data[35],
            data[36], data[37], data[38], data[39],
        ]) as usize;
        let e_shoff = u64::from_le_bytes([
            data[40], data[41], data[42], data[43],
            data[44], data[45], data[46], data[47],
        ]) as usize;
        let e_phentsize = u16::from_le_bytes([data[54], data[55]]) as usize;
        let e_phnum = u16::from_le_bytes([data[56], data[57]]) as usize;
        let e_shentsize = u16::from_le_bytes([data[58], data[59]]) as usize;
        let e_shnum = u16::from_le_bytes([data[60], data[61]]) as usize;
        let e_shstrndx = u16::from_le_bytes([data[62], data[63]]) as usize;

        let machine_desc = match e_machine {
            0x003E => String::from("x86_64 / AMD64 (0x003E)"),
            0x0003 => String::from("x86 / i386 (0x0003)"),
            0x00B7 => String::from("AArch64 (0x00B7)"),
            0x00F3 => String::from("RISC-V (0x00F3)"),
            other => format!("ELF Machine 0x{:04X}", other),
        };

        // Determine image base from first PT_LOAD segment if present
        let mut image_base_opt: Option<u64> = None;
        if e_phoff > 0 && e_phentsize >= 56 {
            for i in 0..e_phnum {
                let off = e_phoff.saturating_add(i.saturating_mul(e_phentsize));
                if off + 56 > data.len() { break; }
                let p_type = u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]]);
                if p_type == 1 {
                    let p_vaddr = u64::from_le_bytes([
                        data[off + 16], data[off + 17], data[off + 18], data[off + 19],
                        data[off + 20], data[off + 21], data[off + 22], data[off + 23],
                    ]);
                    image_base_opt = Some(p_vaddr);
                    break;
                }
            }
        }
        let image_base_hex = match image_base_opt {
            Some(v) => format!("0x{:016X}", v),
            None => String::from("N/A (sem PT_LOAD)"),
        };

        // Resolve .shstrtab for real section names
        let shstrtab_slice: Option<&[u8]> = if e_shoff > 0 && e_shentsize >= 64 && e_shstrndx < e_shnum {
            let str_hdr = e_shoff.saturating_add(e_shstrndx.saturating_mul(e_shentsize));
            if str_hdr + 40 <= data.len() {
                let st_off = u64::from_le_bytes([
                    data[str_hdr + 24], data[str_hdr + 25], data[str_hdr + 26], data[str_hdr + 27],
                    data[str_hdr + 28], data[str_hdr + 29], data[str_hdr + 30], data[str_hdr + 31],
                ]) as usize;
                let st_sz = u64::from_le_bytes([
                    data[str_hdr + 32], data[str_hdr + 33], data[str_hdr + 34], data[str_hdr + 35],
                    data[str_hdr + 36], data[str_hdr + 37], data[str_hdr + 38], data[str_hdr + 39],
                ]) as usize;
                if st_off.saturating_add(st_sz) <= data.len() {
                    Some(&data[st_off..st_off + st_sz])
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        let mut sections: Vec<(String, String, String, String)> = Vec::new();
        if e_shoff > 0 && e_shnum > 0 && e_shentsize >= 64 {
            for i in 0..e_shnum {
                let off = e_shoff.saturating_add(i.saturating_mul(e_shentsize));
                if off + 64 > data.len() { break; }
                let sh_name_idx = u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]]) as usize;
                let sh_type = u32::from_le_bytes([data[off + 4], data[off + 5], data[off + 6], data[off + 7]]);
                if sh_type == 0 { continue; } // SHT_NULL
                let sh_flags = u64::from_le_bytes([
                    data[off + 8], data[off + 9], data[off + 10], data[off + 11],
                    data[off + 12], data[off + 13], data[off + 14], data[off + 15],
                ]);
                let sh_size = u64::from_le_bytes([
                    data[off + 32], data[off + 33], data[off + 34], data[off + 35],
                    data[off + 36], data[off + 37], data[off + 38], data[off + 39],
                ]);
                let name = shstrtab_slice
                    .and_then(|tbl| read_cstr_from_slice(tbl, sh_name_idx))
                    .unwrap_or_else(|| format!("sec#{}", i));
                let stype = match sh_type {
                    1 => String::from("PROGBITS"),
                    2 => String::from("SYMTAB"),
                    3 => String::from("STRTAB"),
                    4 => String::from("RELA"),
                    6 => String::from("DYNAMIC"),
                    8 => String::from("NOBITS"),
                    other => format!("0x{:X}", other),
                };
                let mut fl = String::new();
                if sh_flags & 0x2 != 0 { fl.push('A'); }
                if sh_flags & 0x1 != 0 { fl.push('W'); }
                if sh_flags & 0x4 != 0 { fl.push('X'); }
                if fl.is_empty() { fl.push('-'); }
                sections.push((name, stype, format!("{} B", sh_size), fl));
            }
        } else if e_phoff > 0 && e_phnum > 0 && e_phentsize >= 56 {
            for i in 0..e_phnum {
                let off = e_phoff.saturating_add(i.saturating_mul(e_phentsize));
                if off + 56 > data.len() { break; }
                let p_type = u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]]);
                let p_flags = u32::from_le_bytes([data[off + 4], data[off + 5], data[off + 6], data[off + 7]]);
                let p_memsz = u64::from_le_bytes([
                    data[off + 40], data[off + 41], data[off + 42], data[off + 43],
                    data[off + 44], data[off + 45], data[off + 46], data[off + 47],
                ]);
                let ptype = match p_type {
                    1 => String::from("PT_LOAD"),
                    2 => String::from("PT_DYN"),
                    other => format!("PT_{}", other),
                };
                let mut fl = String::new();
                if p_flags & 4 != 0 { fl.push('R'); }
                if p_flags & 2 != 0 { fl.push('W'); }
                if p_flags & 1 != 0 { fl.push('X'); }
                if fl.is_empty() { fl.push('-'); }
                sections.push((format!("seg#{}", i), ptype, format!("{} B", p_memsz), fl));
            }
        }

        let bits = if ei_class == 2 { 64 } else { 32 };
        BinaryInspection {
            path: String::from(path),
            is_elf: true,
            is_pe: false,
            format_name: format!("ELF{} ({} B)", bits, data.len()),
            magic_hex: format!("{:02X} {:02X} {:02X} {:02X} (\\x7fELF)", data[0], data[1], data[2], data[3]),
            machine_desc,
            entry_point_hex: format!("0x{:016X}", e_entry),
            image_base_hex,
            subsystem_desc: format!("POSIX SysV | phoff=0x{:X} shoff=0x{:X}", e_phoff, e_shoff),
            sections,
            imports: vec![
                format!("Tamanho real no VFS: {} bytes", data.len()),
                format!("PH: {} (ent {} B) | SH: {} (ent {} B)", e_phnum, e_phentsize, e_shnum, e_shentsize),
            ],
        }
    } else if data.len() >= 64 && &data[0..2] == b"MZ" {
        let e_lfanew = u32::from_le_bytes([data[0x3C], data[0x3D], data[0x3E], data[0x3F]]) as usize;
        if e_lfanew.saturating_add(24) > data.len() || &data[e_lfanew..e_lfanew + 4] != b"PE\0\0" {
            return BinaryInspection {
                path: String::from(path),
                is_elf: false,
                is_pe: false,
                format_name: format!("MZ invalido / sem assinatura PE ({} B)", data.len()),
                magic_hex: format!("{:02X} {:02X} ('MZ') e_lfanew=0x{:X}", data[0], data[1], e_lfanew),
                machine_desc: String::from("N/A"),
                entry_point_hex: String::from("N/A"),
                image_base_hex: String::from("N/A"),
                subsystem_desc: String::from("Cabecalho PE\\0\\0 ausente ou truncado"),
                sections: Vec::new(),
                imports: vec![format!("Tamanho real no VFS: {} bytes", data.len())],
            };
        }

        let coff = e_lfanew + 4;
        let machine = u16::from_le_bytes([data[coff], data[coff + 1]]);
        let num_sections = u16::from_le_bytes([data[coff + 2], data[coff + 3]]) as usize;
        let opt_size = u16::from_le_bytes([data[coff + 16], data[coff + 17]]) as usize;
        let opt = coff + 20;

        let machine_desc = match machine {
            0x8664 => String::from("AMD64 / x86_64 (0x8664)"),
            0x014C => String::from("i386 / x86 (0x014C)"),
            0xAA64 => String::from("ARM64 (0xAA64)"),
            other => format!("PE Machine 0x{:04X}", other),
        };

        let opt_magic = if opt + 2 <= data.len() {
            u16::from_le_bytes([data[opt], data[opt + 1]])
        } else {
            0
        };
        let is_pe32_plus = opt_magic == 0x020B;

        let entry_rva = if opt + 20 <= data.len() {
            u32::from_le_bytes([data[opt + 16], data[opt + 17], data[opt + 18], data[opt + 19]])
        } else {
            0
        };

        let image_base = if is_pe32_plus && opt + 32 <= data.len() {
            u64::from_le_bytes([
                data[opt + 24], data[opt + 25], data[opt + 26], data[opt + 27],
                data[opt + 28], data[opt + 29], data[opt + 30], data[opt + 31],
            ])
        } else if !is_pe32_plus && opt + 32 <= data.len() {
            u32::from_le_bytes([data[opt + 28], data[opt + 29], data[opt + 30], data[opt + 31]]) as u64
        } else {
            0
        };

        let subsystem_val = if opt + 70 <= data.len() {
            u16::from_le_bytes([data[opt + 68], data[opt + 69]])
        } else {
            0
        };
        let subsystem_desc = match subsystem_val {
            1 => String::from("Windows Native (1)"),
            2 => String::from("Windows GUI (2)"),
            3 => String::from("Windows CUI / Console (3)"),
            10 => String::from("EFI Application (10)"),
            other => format!("PE Subsystem ({})", other),
        };

        // Parse real IMAGE_SECTION_HEADER entries (40 bytes each)
        let mut sections: Vec<(String, String, String, String)> = Vec::new();
        let sec_table = opt.saturating_add(opt_size);
        for i in 0..num_sections {
            let off = sec_table.saturating_add(i.saturating_mul(40));
            if off + 40 > data.len() { break; }
            let raw_name = &data[off..off + 8];
            let name_end = raw_name.iter().position(|&b| b == 0).unwrap_or(8);
            let mut sec_name = String::new();
            for &b in &raw_name[..name_end] {
                if b.is_ascii_graphic() {
                    sec_name.push(b as char);
                }
            }
            if sec_name.is_empty() {
                sec_name = format!("sec#{}", i);
            }
            let vsize = u32::from_le_bytes([data[off + 8], data[off + 9], data[off + 10], data[off + 11]]);
            let raw_size = u32::from_le_bytes([data[off + 16], data[off + 17], data[off + 18], data[off + 19]]);
            let chars = u32::from_le_bytes([data[off + 36], data[off + 37], data[off + 38], data[off + 39]]);

            let stype = if chars & 0x0000_0020 != 0 {
                String::from("Code")
            } else if chars & 0x0000_0040 != 0 {
                String::from("InitData")
            } else if chars & 0x0000_0080 != 0 {
                String::from("Uninit")
            } else {
                String::from("Section")
            };

            let mut fl = String::new();
            if chars & 0x4000_0000 != 0 { fl.push('R'); }
            if chars & 0x8000_0000 != 0 { fl.push('W'); }
            if chars & 0x2000_0000 != 0 { fl.push('X'); }
            if fl.is_empty() { fl.push('-'); }

            let disp_sz = if vsize > 0 { vsize } else { raw_size };
            sections.push((sec_name, stype, format!("{} B", disp_sz), fl));
        }

        let fmt_label = if is_pe32_plus { "PE32+" } else { "PE32" };
        BinaryInspection {
            path: String::from(path),
            is_elf: false,
            is_pe: true,
            format_name: format!("{} ({} B)", fmt_label, data.len()),
            magic_hex: format!("4D 5A ('MZ') -> PE\\0\\0 @0x{:X}", e_lfanew),
            machine_desc,
            entry_point_hex: format!("0x{:016X} (RVA 0x{:X})", image_base.saturating_add(entry_rva as u64), entry_rva),
            image_base_hex: format!("0x{:016X}", image_base),
            subsystem_desc,
            sections,
            imports: vec![
                format!("Tamanho real no VFS: {} bytes", data.len()),
                format!("e_lfanew=0x{:X} | Secoes={} | OptHdr={} B", e_lfanew, num_sections, opt_size),
            ],
        }
    } else {
        let mut magic_hex = String::new();
        for (i, b) in data.iter().take(4).enumerate() {
            if i > 0 { magic_hex.push(' '); }
            magic_hex.push_str(&format!("{:02X}", b));
        }
        if magic_hex.is_empty() {
            magic_hex.push_str("(vazio)");
        }
        BinaryInspection {
            path: String::from(path),
            is_elf: false,
            is_pe: false,
            format_name: format!("Formato desconhecido ({} B)", data.len()),
            magic_hex,
            machine_desc: String::from("N/A"),
            entry_point_hex: String::from("N/A"),
            image_base_hex: String::from("N/A"),
            subsystem_desc: String::from("Nao e ELF64 nem PE32+"),
            sections: Vec::new(),
            imports: vec![format!("Tamanho real no VFS: {} bytes", data.len())],
        }
    }
}

pub static CAT3_STATE: SpinLock<Cat3State> = SpinLock::new(Cat3State {
    last_canvas_rect: (100, 100, 680, 420),

    // 19. Terminal Tri-Shell
    shell_mode: ShellMode::Bash,
    term_history: Vec::new(),
    term_scroll: 0,
    term_input: String::new(),
    shell_status: String::new(),

    // 20. Run Dialog
    run_input: String::new(),
    run_history: Vec::new(),
    run_status: String::new(),
    browse_index: 0,

    // 21. App Store
    store_tab: 0,
    store_search: String::new(),
    store_search_focused: false,
    installed_apps: Vec::new(),
    store_status: String::new(),

    // 22. Setup Wizard
    wizard_step: 1,
    wizard_eula_accepted: false,
    wizard_target_dir: String::new(),
    wizard_progress: 0,
    wizard_status: String::new(),

    // 23. Default Apps
    selected_ext_idx: 0,
    ext_mappings: Vec::new(),
    ext_status: String::new(),

    // 24. Dual-ABI Compatibility Center
    selected_bin_chip: 0,
    inspection: None,
    abi_status: String::new(),

    // 25. Environment Variables
    selected_env_idx: 0,
    is_user_env_selected: true,
    user_env: Vec::new(),
    sys_env: Vec::new(),
    env_status: String::new(),
});

fn ensure_cat3_state_initialized() {
    let needs_init = {
        let state = CAT3_STATE.lock();
        state.inspection.is_none()
    };
    if !needs_init {
        return;
    }

    // Ensure valid real ELF64 & PE32+ sample binaries exist in VFS for real inspection
    let exec_perms = Permissions::from_mode(0o755);
    if vfs::vfs_read_all("/bin/sample.elf").is_err() {
        let _ = vfs::vfs_create_dir_all("/bin", exec_perms);
        let elf_bytes = build_sample_elf64_binary();
        let _ = vfs::vfs_create_file("/bin/sample.elf", &elf_bytes, exec_perms);
    }
    if vfs::vfs_read_all("C:\\sample.exe").is_err() {
        let pe_bytes = build_sample_pe32plus_binary();
        let _ = vfs::vfs_create_file("C:\\sample.exe", &pe_bytes, exec_perms);
    }

    let initial_idx = if vfs::vfs_read_all(SAMPLE_BIN_PATHS[0]).is_ok() { 0 } else { 3 };
    let initial_insp = inspect_target_path(SAMPLE_BIN_PATHS[initial_idx]);

    let mut state = CAT3_STATE.lock();
    if state.term_history.is_empty() {
        state.term_history.push(String::from("HybridOS Tri-Shell Environment [Ring 0 Bare-Metal Kernel]"));
        state.term_history.push(String::from("Dual-ABI Subsystem Active: Linux POSIX (ELF64) & Windows NT (PE32+)"));
        state.term_history.push(String::from("Digite um comando abaixo ou clique nos seletores de modo."));
        state.term_history.push(String::from(""));
        state.shell_status = String::from("VFS Conectado | Ring 0 Compositor | Subsystem: Dual-ABI");
    }
    if state.run_history.is_empty() {
        state.run_history.push(String::from("cmd"));
        state.run_history.push(String::from("bash"));
        state.run_history.push(String::from("explorer"));
        state.run_history.push(String::from("taskmgr"));
        state.run_history.push(String::from("notepad"));
        state.run_history.push(String::from("calc"));
        state.run_history.push(String::from("sysinfo"));
        state.run_history.push(String::from("winget list"));
        state.run_input = String::from("taskmgr");
        state.run_status = String::from("Pronto para executar.");
    }
    if state.wizard_target_dir.is_empty() {
        state.wizard_target_dir = String::from("C:\\Program Files\\HybridOS_Package");
        state.wizard_status = String::from("Aguardando aceite da licenca EULA...");
    }
    if state.ext_mappings.is_empty() {
        state.ext_mappings = default_ext_mappings();
        state.ext_status = String::from("Associações de extensões carregadas com sucesso.");
    }
    if state.user_env.is_empty() {
        state.user_env = default_user_env();
        state.sys_env = default_sys_env();
        state.env_status = String::from("Variaveis de ambiente prontas.");
    }
    if state.inspection.is_none() {
        state.selected_bin_chip = initial_idx;
        state.abi_status = format!("Binario inspecionado: {} ({})", initial_insp.path, initial_insp.format_name);
        state.inspection = Some(initial_insp);
    }
    if state.store_status.is_empty() {
        state.store_status = String::from("Catálogo atualizado. Pronto para instalação.");
    }
}

// =============================================================================
// PROGRAM 19: TERMINAL TRI-SHELL GRÁFICO
// =============================================================================

unsafe fn render_terminal_trishell(
    fb: *mut u8, pitch: usize, scr_w: usize, scr_h: usize,
    cx: usize, cy: usize, cw: usize, ch: usize,
    desk: &mut DesktopState,
) {
    let state = CAT3_STATE.lock();

    // 1. Top Glass Toolbar (36px)
    draw_glass_toolbar(fb, pitch, scr_w, scr_h, cx, cy, cw, 36);

    // Label
    draw_text_px(fb, pitch, scr_w, scr_h, cx + 10, cy + 10, "Modo Shell:", 0x001E3A8A, None);

    // Mode Buttons: [ Bash | CMD | PowerShell ]
    let bash_active = state.shell_mode == ShellMode::Bash;
    let cmd_active = state.shell_mode == ShellMode::Cmd;
    let ps_active = state.shell_mode == ShellMode::PowerShell;

    draw_glass_button(fb, pitch, scr_w, scr_h, cx + 90, cy + 6, 68, 24, "Bash", bash_active);
    draw_glass_button(fb, pitch, scr_w, scr_h, cx + 162, cy + 6, 60, 24, "CMD", cmd_active);
    draw_glass_button(fb, pitch, scr_w, scr_h, cx + 226, cy + 6, 96, 24, "PowerShell", ps_active);

    // Quick Command Chips
    let chip_y = cy + 6;
    draw_glass_button(fb, pitch, scr_w, scr_h, cx + 330, chip_y, 75, 24, "uname -a", false);
    draw_glass_button(fb, pitch, scr_w, scr_h, cx + 409, chip_y, 50, 24, "ls /", false);
    draw_glass_button(fb, pitch, scr_w, scr_h, cx + 463, chip_y, 65, 24, "dir C:\\", false);
    draw_glass_button(fb, pitch, scr_w, scr_h, cx + 532, chip_y, 70, 24, "meminfo", false);

    // Limpar Button (at top right)
    let clear_x = (cx + cw).saturating_sub(75);
    draw_glass_button(fb, pitch, scr_w, scr_h, clear_x, chip_y, 65, 24, "Limpar", false);

    // 2. Terminal Screen Area (Dark Slate - visible dark background)
    let term_y = cy + 36;
    let term_h = ch.saturating_sub(36 + 32 + 22);
    fill_rounded_rect(fb, pitch, scr_w, scr_h, cx, term_y, cw, term_h, 0, 0x001E293B);

    // Display Primary Terminal Buffer Lines directly from desk.term_lines
    let max_lines = (term_h / 18).max(1);
    let total_lines = desk.term_lines.len();
    let start_idx = if total_lines > max_lines {
        total_lines - max_lines
    } else {
        0
    };

    let mut line_y = term_y + 8;
    for i in start_idx..total_lines {
        if line_y + 16 > term_y + term_h { break; }
        let line = &desk.term_lines[i];

        let col = if line.starts_with("user@hybridos") || line.starts_with("root@hybridos") || line.starts_with("hybridos[ring0]") {
            0x004ADE80 // Emerald / Green prompt
        } else if line.starts_with("PS C:\\") {
            0x0038BDF8 // Sky blue prompt
        } else if line.starts_with("C:\\") {
            0x00F8FAFC // Silver prompt
        } else if line.starts_with("[ERRO]") || line.starts_with("error") {
            0x00F87171 // Ruby red error
        } else if line.starts_with("[OK]") || line.starts_with("Sucesso") {
            0x0034D399 // Green success
        } else {
            0x00CBD5E1 // Slate 300 text
        };

        draw_text_clipped(fb, pitch, scr_w, scr_h, cx + 12, line_y, cx + cw - 12, line, col, None);
        line_y += 18;
    }

    // 3. Command Input Bar (32px)
    let input_y = term_y + term_h;
    fill_rounded_rect(fb, pitch, scr_w, scr_h, cx, input_y, cw, 32, 0, 0x001E293B);    let (prompt_str, prompt_col) = match state.shell_mode {
        ShellMode::Bash => ("user@hybridos:~$ ", 0x004ADE80),
        ShellMode::Cmd => ("C:\\Users\\Default> ", 0x00F8FAFC),
        ShellMode::PowerShell => ("PS C:\\> ", 0x0038BDF8),
    };

    draw_text_px(fb, pitch, scr_w, scr_h, cx + 12, input_y + 8, prompt_str, prompt_col, None);
    let prompt_w = prompt_str.len() * 8;
    let input_field_x = cx + 12 + prompt_w;

    let input_display = if state.term_input.is_empty() {
        "help (pressione Executar)"
    } else {
        &state.term_input
    };
    let input_col = if state.term_input.is_empty() { 0x0064748B } else { 0x00FFFFFF };
    draw_text_clipped(fb, pitch, scr_w, scr_h, input_field_x, input_y + 8, cx + cw - 95, input_display, input_col, None);

    // Blinking cursor indication based on SYSTEM_TICKS
    let cursor_visible = SYSTEM_TICKS.load(Ordering::Relaxed) % 2 == 0;
    let cursor_x = input_field_x + (if state.term_input.is_empty() { 0 } else { state.term_input.len() * 8 });
    if cursor_visible && cursor_x + 2 < cx + cw - 95 {
        for cy_step in (input_y + 6)..(input_y + 22) {
            if cy_step < scr_h {
                let p = fb.add(cy_step * pitch) as *mut u32;
                *p.add(cursor_x) = prompt_col;
                *p.add(cursor_x + 1) = prompt_col;
            }
        }
    }

    // [ Executar ] Button
    let exec_btn_x = (cx + cw).saturating_sub(88);
    draw_glass_button(fb, pitch, scr_w, scr_h, exec_btn_x, input_y + 4, 80, 24, "Executar", true);

    // 4. Status Bar (22px)
    let status_y = cy + ch.saturating_sub(22);
    let mode_desc = match state.shell_mode {
        ShellMode::Bash => "Modo: Bash POSIX | Prompt: user@hybridos:~$",
        ShellMode::Cmd => "Modo: CMD Win32 | Prompt: C:\\Users\\Default>",
        ShellMode::PowerShell => "Modo: PowerShell | Prompt: PS C:\\>",
    };
    draw_glass_statusbar(fb, pitch, scr_w, scr_h, cx, status_y, cw, 22, mode_desc, &state.shell_status);
}

fn execute_trishell_command(cmd_line: &str, desk: &mut DesktopState) {
    let mut state = CAT3_STATE.lock();
    let trimmed = cmd_line.trim();
    if trimmed.is_empty() {
        return;
    }

    let prompt_prefix = match state.shell_mode {
        ShellMode::Bash => "user@hybridos:~$ ",
        ShellMode::Cmd => "C:\\Users\\Default> ",
        ShellMode::PowerShell => "PS C:\\> ",
    };

    let full_prompt_cmd = format!("{}{}", prompt_prefix, trimmed);
    state.term_history.push(full_prompt_cmd.clone());
    desk.term_lines.push(full_prompt_cmd);

    if trimmed == "clear" || trimmed == "cls" {
        state.term_history.clear();
        desk.term_lines.clear();
        state.shell_status = String::from("Tela limpa.");
        desk.needs_full_redraw = true;
        return;
    }

    // Execute real command in userland::shell::HybridShell matching active shell_mode personality
    let mut shell = match state.shell_mode {
        ShellMode::Bash => {
            let mut s = HybridShell::with_personality(ExecutionPersonality::LinuxPosix);
            s.flavor = ShellFlavor::LinuxBash;
            s
        }
        ShellMode::Cmd => {
            let mut s = HybridShell::with_personality(ExecutionPersonality::WindowsNt);
            s.flavor = ShellFlavor::WindowsCmd;
            s
        }
        ShellMode::PowerShell => {
            let mut s = HybridShell::with_personality(ExecutionPersonality::HybridNative);
            s.flavor = ShellFlavor::PowerShell;
            s.cwd = String::from("C:\\");
            s
        }
    };
    let output = shell.execute(trimmed);

    for line in output.lines() {
        state.term_history.push(String::from(line));
        desk.term_lines.push(String::from(line));
    }

    while state.term_history.len() > 200 {
        state.term_history.remove(0);
    }
    while desk.term_lines.len() > 100 {
        desk.term_lines.remove(0);
    }

    state.term_input.clear();
    state.shell_status = format!("Executado: '{}' (Codigo: {})", trimmed, shell.last_exit_code);
    desk.needs_full_redraw = true;
}

// =============================================================================
// PROGRAM 20: CAIXA EXECUTAR (RUN DIALOG)
// =============================================================================

unsafe fn render_run_dialog(
    fb: *mut u8, pitch: usize, scr_w: usize, scr_h: usize,
    cx: usize, cy: usize, cw: usize, ch: usize,
    _desk: &mut DesktopState,
) {
    let state = CAT3_STATE.lock();

    // Background Canvas: Pure Crystal White
    fill_rounded_rect(fb, pitch, scr_w, scr_h, cx, cy, cw, ch, 0, 0x00FFFFFF);

    // Top Header Banner (56px)
    fill_rounded_rect_gradient(fb, pitch, scr_w, scr_h, cx, cy, cw, 56, 0, 0x00F8FAFC, 0x00E2E8F0);
    draw_rounded_rect_border(fb, pitch, scr_w, scr_h, cx, cy + 55, cw, 1, 0, 0x00CBD5E1);

    // Classic Run Icon (32x32 Cyan Sphere Emblem)
    draw_crystal_sphere_emblem(fb, pitch, scr_w, scr_h, cx + 30, cy + 28, false);

    // Informational Text
    draw_text_px(fb, pitch, scr_w, scr_h, cx + 60, cy + 12, "Digite o nome de um programa, pasta, documento ou comando,", 0x000F172A, None);
    draw_text_px(fb, pitch, scr_w, scr_h, cx + 60, cy + 30, "e o HybridOS o executara no subsistema nativo correspondente.", 0x00475569, None);

    // Combobox Field Area
    let box_y = cy + 70;
    draw_text_px(fb, pitch, scr_w, scr_h, cx + 18, box_y + 6, "Abrir:", 0x001E3A8A, None);

    let input_w = cw.saturating_sub(130).max(120);
    let input_box_x = cx + 68;
    draw_glass_textbox(fb, pitch, scr_w, scr_h, input_box_x, box_y, input_w, 26, &state.run_input, true);

    // History Picker Quick Button [ ▼ ]
    let drop_btn_x = input_box_x + input_w + 4;
    draw_glass_button(fb, pitch, scr_w, scr_h, drop_btn_x, box_y, 45, 26, "Hist", false);

    // Quick Command History Chips
    let chips_y = cy + 108;
    draw_text_px(fb, pitch, scr_w, scr_h, cx + 18, chips_y + 4, "Atalhos:", 0x0064748B, None);

    let history_chips = ["taskmgr", "explorer", "cmd", "bash", "calc", "notepad", "winget"];
    let mut chip_x = cx + 80;
    for chip in history_chips.iter() {
        let chip_w = chip.len() * 8 + 14;
        if chip_x + chip_w >= cx + cw - 10 { break; }
        let is_cur = state.run_input == *chip;
        draw_glass_button(fb, pitch, scr_w, scr_h, chip_x, chips_y, chip_w, 22, chip, is_cur);
        chip_x += chip_w + 6;
    }

    // Status Message Area
    let msg_y = cy + 144;
    fill_rounded_rect(fb, pitch, scr_w, scr_h, cx + 18, msg_y, cw - 36, 26, 3, 0x00F1F5F9);
    draw_rounded_rect_border(fb, pitch, scr_w, scr_h, cx + 18, msg_y, cw - 36, 26, 3, 0x00E2E8F0);
    draw_text_clipped(fb, pitch, scr_w, scr_h, cx + 24, msg_y + 5, cx + cw - 40, &state.run_status, 0x000284C7, None);

    // Bottom Action Strip (40px)
    let bottom_y = cy + ch.saturating_sub(44);
    fill_rounded_rect(fb, pitch, scr_w, scr_h, cx, bottom_y, cw, 44, 0, 0x00F8FAFC);
    draw_rounded_rect_border(fb, pitch, scr_w, scr_h, cx, bottom_y, cw, 1, 0, 0x00E2E8F0);

    // Action Buttons: [ OK ] [ Cancelar ] [ Procurar no VFS... ]
    let btn_ok_x = (cx + cw).saturating_sub(290);
    let btn_cancel_x = (cx + cw).saturating_sub(205);
    let btn_browse_x = (cx + cw).saturating_sub(120);

    draw_glass_button(fb, pitch, scr_w, scr_h, btn_ok_x, bottom_y + 9, 80, 26, "OK", true);
    draw_glass_button(fb, pitch, scr_w, scr_h, btn_cancel_x, bottom_y + 9, 80, 26, "Cancelar", false);
    draw_glass_button(fb, pitch, scr_w, scr_h, btn_browse_x, bottom_y + 9, 110, 26, "Procurar...", false);
}

fn execute_run_dialog_action(desk: &mut DesktopState) {
    let mut state = CAT3_STATE.lock();
    let cmd = state.run_input.trim().to_string();
    if cmd.is_empty() {
        state.run_status = String::from("Aviso: Digite um comando para executar.");
        return;
    }

    // Check for GUI window dispatching
    let mut window_opened = false;
    match cmd.as_str() {
        "explorer" => {
            for w in desk.windows.iter_mut() {
                if w.id == AppId::FileExplorer {
                    w.open = true;
                    w.minimized = false;
                    window_opened = true;
                }
            }
        }
        "taskmgr" | "ps" => {
            for w in desk.windows.iter_mut() {
                if w.id == AppId::TaskManager {
                    w.open = true;
                    w.minimized = false;
                    window_opened = true;
                }
            }
        }
        "notepad" => {
            for w in desk.windows.iter_mut() {
                if w.id == AppId::Notepad {
                    w.open = true;
                    w.minimized = false;
                    window_opened = true;
                }
            }
        }
        "calc" => {
            for w in desk.windows.iter_mut() {
                if w.id == AppId::Calculator {
                    w.open = true;
                    w.minimized = false;
                    window_opened = true;
                }
            }
        }
        "sysinfo" | "winver" => {
            for w in desk.windows.iter_mut() {
                if w.id == AppId::SystemInfo {
                    w.open = true;
                    w.minimized = false;
                    window_opened = true;
                }
            }
        }
        "appstore" => {
            for w in desk.windows.iter_mut() {
                if w.id == AppId::AppStore {
                    w.open = true;
                    w.minimized = false;
                    window_opened = true;
                }
            }
        }
        _ => {}
    }

    // Also execute real command via HybridShell
    let mut shell = HybridShell::new();
    let out = shell.execute(&cmd);

    // Append to terminal lines so the user can inspect output
    desk.term_lines.push(format!("Executar: {}", cmd));
    for line in out.lines().take(5) {
        desk.term_lines.push(String::from(line));
    }

    if window_opened {
        state.run_status = format!("Aplicativo '{}' despachado com sucesso na interface grafica!", cmd);
    } else {
        state.run_status = format!("Comando '{}' executado com sucesso no subsistema nativo!", cmd);
    }

    if !state.run_history.iter().any(|h| h == &cmd) {
        state.run_history.push(cmd);
    }
    desk.needs_full_redraw = true;
}

// =============================================================================
// PROGRAM 21: CENTRAL DE PROGRAMAS (APP STORE & SOFTWARE HUB)
// =============================================================================

fn filter_store_packages(store_tab: usize, store_search: &str) -> Vec<&'static SoftwarePackage> {
    let query = store_search.to_lowercase();
    SOFTWARE_CATALOG
        .iter()
        .filter(|pkg| {
            pkg.category == store_tab
                && (query.is_empty() || pkg.name.to_lowercase().contains(&query))
        })
        .collect()
}

unsafe fn render_app_store(
    fb: *mut u8, pitch: usize, scr_w: usize, scr_h: usize,
    cx: usize, cy: usize, cw: usize, ch: usize,
    desk: &mut DesktopState,
) {
    // Poll keyboard input when search box is focused
    {
        let mut st = CAT3_STATE.lock();
        if st.store_search_focused {
            let mut changed = false;
            while let Some(ch_key) = pop_keyboard_char() {
                match ch_key {
                    '\x08' | '\x7f' => {
                        st.store_search.pop();
                        changed = true;
                    }
                    '\n' | '\r' => {
                        st.store_search_focused = false;
                        changed = true;
                    }
                    '\x1b' => {
                        st.store_search.clear();
                        st.store_search_focused = false;
                        changed = true;
                    }
                    c if c.is_ascii() && !c.is_ascii_control() => {
                        if st.store_search.len() < 40 {
                            st.store_search.push(c);
                            changed = true;
                        }
                    }
                    _ => {}
                }
            }
            if changed {
                if st.store_search.is_empty() {
                    st.store_status = String::from("Filtro de busca limpo.");
                } else {
                    st.store_status = format!("Filtrando pacotes por: '{}'", st.store_search);
                }
                desk.needs_full_redraw = true;
            }
        }
    }

    let state = CAT3_STATE.lock();

    // 1. Top Tabs Strip (32px)
    draw_glass_toolbar(fb, pitch, scr_w, scr_h, cx, cy, cw, 32);

    let tab_names = ["Destaques", "Ferramentas", "Jogos", "Desenvolvedor"];
    let mut tab_x = cx + 8;
    for (idx, tname) in tab_names.iter().enumerate() {
        let is_act = state.store_tab == idx;
        let tab_w = tname.len() * 8 + 24;
        draw_glass_tab(fb, pitch, scr_w, scr_h, tab_x, cy + 4, tab_w, 24, tname, is_act);
        tab_x += tab_w + 6;
    }

    // 2. Search Box Bar (34px)
    let search_bar_y = cy + 32;
    fill_rounded_rect(fb, pitch, scr_w, scr_h, cx, search_bar_y, cw, 34, 0, 0x00F8FAFC);
    draw_rounded_rect_border(fb, pitch, scr_w, scr_h, cx, search_bar_y + 33, cw, 1, 0, 0x00E2E8F0);

    draw_text_px(fb, pitch, scr_w, scr_h, cx + 12, search_bar_y + 9, "Buscar Pacote:", 0x001E3A8A, None);
    let s_box_w = cw.saturating_sub(215).max(120);
    let search_display = if state.store_search.is_empty() {
        if state.store_search_focused {
            String::from("_")
        } else {
            String::from("Clique para filtrar aplicativos...")
        }
    } else if state.store_search_focused {
        format!("{}_", state.store_search)
    } else {
        state.store_search.clone()
    };
    draw_glass_textbox(fb, pitch, scr_w, scr_h, cx + 125, search_bar_y + 5, s_box_w, 24, &search_display, state.store_search_focused);

    let clear_btn_x = cx + 125 + s_box_w + 6;
    if clear_btn_x + 72 <= cx + cw.saturating_sub(4) {
        draw_glass_button(fb, pitch, scr_w, scr_h, clear_btn_x, search_bar_y + 5, 72, 24, "Limpar", !state.store_search.is_empty());
    }

    // 3. Main Catalog Card Grid Area
    let grid_y = search_bar_y + 34;
    let grid_h = ch.saturating_sub(32 + 34 + 24);
    fill_rounded_rect(fb, pitch, scr_w, scr_h, cx, grid_y, cw, grid_h, 0, 0x00FFFFFF);

    // Filter packages matching category and search text (shared predicate with click handler)
    let filtered = filter_store_packages(state.store_tab, &state.store_search);

    let card_w = (cw.saturating_sub(36)) / 2;
    let card_h = 100usize;

    for (idx, pkg) in filtered.iter().enumerate() {
        let col_idx = idx % 2;
        let row_idx = idx / 2;

        let card_x = cx + 12 + col_idx * (card_w + 12);
        let card_y = grid_y + 12 + row_idx * (card_h + 10);

        if card_y + card_h > grid_y + grid_h { break; }

        let is_installed = state.installed_apps.iter().any(|id| id == pkg.id);

        // Vitrified Card Background with subtle gradient
        let card_bg_top = if is_installed { 0x00F0FDF4 } else { 0x00F8FAFC };
        let card_bg_bot = if is_installed { 0x00DCFCE7 } else { 0x00EBF3FC };
        fill_rounded_rect_gradient(fb, pitch, scr_w, scr_h, card_x, card_y, card_w, card_h, 6, card_bg_top, card_bg_bot);
        draw_rounded_rect_border(fb, pitch, scr_w, scr_h, card_x, card_y, card_w, card_h, 6, if is_installed { 0x0022C55E } else { 0x00CBD5E1 });

        // App Name and Version
        draw_text_px(fb, pitch, scr_w, scr_h, card_x + 12, card_y + 10, pkg.name, 0x000F172A, None);
        let v_str = format!("v{}", pkg.version);
        draw_text_px(fb, pitch, scr_w, scr_h, card_x + 16 + pkg.name.len() * 8, card_y + 10, &v_str, 0x000284C7, None);

        // Size & ABI Badge
        let meta_str = format!("Tamanho: {} | ABI: {}", pkg.size_str, pkg.abi_type);
        draw_text_px(fb, pitch, scr_w, scr_h, card_x + 12, card_y + 30, &meta_str, 0x0064748B, None);

        // Description
        draw_text_clipped(fb, pitch, scr_w, scr_h, card_x + 12, card_y + 50, card_x + card_w - 90, pkg.desc, 0x00334155, None);

        // Functional Install / Update / Installed Button
        let btn_x = card_x + card_w.saturating_sub(95);
        let btn_y = card_y + card_h.saturating_sub(34);
        if is_installed {
            draw_glass_button(fb, pitch, scr_w, scr_h, btn_x, btn_y, 85, 24, "Remover", false);
        } else {
            draw_glass_button(fb, pitch, scr_w, scr_h, btn_x, btn_y, 85, 24, "Instalar", true);
        }
    }

    // 4. Status Bar (24px)
    let status_y = cy + ch.saturating_sub(24);
    draw_glass_statusbar(fb, pitch, scr_w, scr_h, cx, status_y, cw, 24, &state.store_status, "VFS: /bin/ & C:\\Program Files\\");
}

fn install_software_package(pkg_id: &str, desk: &mut DesktopState) {
    let mut state = CAT3_STATE.lock();
    let pkg = match SOFTWARE_CATALOG.iter().find(|p| p.id == pkg_id) {
        Some(p) => *p,
        None => return,
    };

    let is_already = state.installed_apps.iter().any(|id| id == pkg.id);
    if is_already {
        // Uninstall: Remove from VFS
        let bin_path = format!("/bin/{}", pkg.bin_name);
        let _ = vfs::vfs_unlink(&bin_path);
        let _ = vfs::vfs_unlink(pkg.win_path);
        state.installed_apps.retain(|id| id != pkg.id);
        state.store_status = format!("Pacote '{}' removido do VFS com sucesso.", pkg.name);
        desk.needs_full_redraw = true;
        return;
    }

    // Generate real binary payload
    let binary_bytes = if pkg.is_elf {
        create_synthetic_elf()
    } else {
        create_synthetic_pe()
    };

    let perms = Permissions::from_mode(0o755);

    // Create directories in VFS
    let _ = vfs::vfs_create_dir_all("/bin", perms);
    if let Some(parent_dir) = pkg.win_path.rfind('\\') {
        let dir = &pkg.win_path[..parent_dir];
        let _ = vfs::vfs_create_dir_all(dir, perms);
    }

    // Unlink any pre-existing file before writing new real binaries to VFS
    let bin_path = format!("/bin/{}", pkg.bin_name);
    let _ = vfs::vfs_unlink(&bin_path);
    let _ = vfs::vfs_unlink(pkg.win_path);
    let w1 = vfs::vfs_create_file(&bin_path, &binary_bytes, perms);
    let w2 = vfs::vfs_create_file(pkg.win_path, &binary_bytes, perms);

    if w1.is_ok() || w2.is_ok() {
        state.installed_apps.push(String::from(pkg.id));
        state.store_status = format!("Sucesso: '{}' instalado em /bin/{} e C:\\Program Files\\!", pkg.name, pkg.bin_name);
    } else {
        state.store_status = format!("[ERRO] Falha ao gravar pacote no VFS ({})", pkg.name);
    }

    desk.needs_full_redraw = true;
}

// =============================================================================
// PROGRAM 22: ASSISTENTE DE INSTALAÇÃO (SETUP WIZARD)
// =============================================================================

unsafe fn render_setup_wizard(
    fb: *mut u8, pitch: usize, scr_w: usize, scr_h: usize,
    cx: usize, cy: usize, cw: usize, ch: usize,
    _desk: &mut DesktopState,
) {
    let state = CAT3_STATE.lock();

    // Background
    fill_rounded_rect(fb, pitch, scr_w, scr_h, cx, cy, cw, ch, 0, 0x00FFFFFF);

    // Top Header Banner (52px)
    fill_rounded_rect_gradient(fb, pitch, scr_w, scr_h, cx, cy, cw, 52, 0, 0x000284C7, 0x000369A1);
    draw_text_px(fb, pitch, scr_w, scr_h, cx + 20, cy + 10, "Assistente de Instalacao do HybridOS Suite", 0x00FFFFFF, None);

    let step_subtitle = match state.wizard_step {
        1 => "Etapa 1 de 3: Boas-vindas e Contrato de Licenca EULA",
        2 => "Etapa 2 de 3: Escolha do Diretorio de Destino no VFS",
        _ => "Etapa 3 de 3: Extracao de Arquivos e Conclusao",
    };
    draw_text_px(fb, pitch, scr_w, scr_h, cx + 20, cy + 30, step_subtitle, 0x00BAE6FD, None);

    // Body Area by Step
    let body_y = cy + 52;
    let body_h = ch.saturating_sub(52 + 48);

    match state.wizard_step {
        1 => {
            // EULA License Scroll Box
            let box_y = body_y + 14;
            let box_h = body_h.saturating_sub(60);
            fill_rounded_rect(fb, pitch, scr_w, scr_h, cx + 18, box_y, cw - 36, box_h, 3, 0x00F8FAFC);
            draw_rounded_rect_border(fb, pitch, scr_w, scr_h, cx + 18, box_y, cw - 36, box_h, 3, 0x00CBD5E1);

            let eula_lines = [
                "CONTRATO DE LICENÇA DE USUÁRIO FINAL (EULA) - HYBRIDOS SUITE",
                "",
                "1. REGIME DE COMPATIBILIDADE DUAL-ABI:",
                "   O software instala componentes executaveis sob regime compativel com",
                "   GNU General Public License v3 (POSIX ELF64) e Microsoft Windows NT (PE32+).",
                "",
                "2. ACESSO AO SISTEMA E ARMAZENAMENTO:",
                "   Os arquivos serao gravados diretamente no sistema de arquivos virtual (VFS)",
                "   nas pastas de sistema C:\\Program Files e /opt/hybridos_package.",
                "",
                "3. ISOLAMENTO E SEGURANCA RING 0:",
                "   O kernel monitora as transicoes de syscalls e rotinas de interrupcao.",
            ];

            let mut ey = box_y + 10;
            for line in eula_lines.iter() {
                if ey + 16 > box_y + box_h { break; }
                let col = if line.starts_with("CONTRATO") || line.starts_with("1.") || line.starts_with("2.") || line.starts_with("3.") {
                    0x000F172A
                } else {
                    0x00475569
                };
                draw_text_clipped(fb, pitch, scr_w, scr_h, cx + 28, ey, cx + cw - 40, line, col, None);
                ey += 18;
            }

            // Checkbox: "Eu aceito os termos do Contrato de Licença"
            let chk_y = box_y + box_h + 10;
            fill_rounded_rect(fb, pitch, scr_w, scr_h, cx + 20, chk_y, 16, 16, 3, 0x00FFFFFF);
            draw_rounded_rect_border(fb, pitch, scr_w, scr_h, cx + 20, chk_y, 16, 16, 3, if state.wizard_eula_accepted { 0x000284C7 } else { 0x0094A3B8 });
            if state.wizard_eula_accepted {
                fill_rounded_rect(fb, pitch, scr_w, scr_h, cx + 23, chk_y + 3, 10, 10, 2, 0x0016A34A);
            }
            draw_text_px(fb, pitch, scr_w, scr_h, cx + 44, chk_y, "Eu aceito os termos do Contrato de Licenca EULA", 0x000F172A, None);
        }
        2 => {
            // Target Directory Selection
            draw_text_px(fb, pitch, scr_w, scr_h, cx + 20, body_y + 20, "Pasta de Destino para os Arquivos do Pacote:", 0x000F172A, None);
            draw_glass_textbox(fb, pitch, scr_w, scr_h, cx + 20, body_y + 44, cw - 40, 28, &state.wizard_target_dir, true);

            draw_text_px(fb, pitch, scr_w, scr_h, cx + 20, body_y + 88, "Locais Sugeridos no VFS:", 0x0064748B, None);

            // Path chips
            let chips = ["C:\\Program Files\\HybridOS_Package", "/opt/hybridos_package", "/bin/package", "C:\\Users\\Default\\AppData"];
            let mut chip_y = body_y + 110;
            for chip in chips.iter() {
                let is_sel = state.wizard_target_dir == *chip;
                draw_glass_button(fb, pitch, scr_w, scr_h, cx + 20, chip_y, 320, 24, chip, is_sel);
                chip_y += 30;
            }

            // Disk Space telemetry
            draw_text_px(fb, pitch, scr_w, scr_h, cx + 20, body_y + 240, "Espaco em disco necessario: 14.5 MB", 0x001E3A8A, None);
            draw_text_px(fb, pitch, scr_w, scr_h, cx + 20, body_y + 260, "Espaco livre disponivel no VFS: 512 MB (RAMFS + FAT32 OK)", 0x0016A34A, None);
        }
        _ => {
            // Extraction Progress Bar & Live Status
            draw_text_px(fb, pitch, scr_w, scr_h, cx + 20, body_y + 30, "Progresso da Instalacao e Gravacao no Disco:", 0x000F172A, None);

            // Large Progress Bar
            let pb_y = body_y + 60;
            draw_glass_progressbar(fb, pitch, scr_w, scr_h, cx + 20, pb_y, cw - 40, 24, state.wizard_progress, 0x0034D399, 0x0010B981);

            let pct_str = format!("{}%", state.wizard_progress);
            draw_text_px(fb, pitch, scr_w, scr_h, cx + cw / 2 - 10, pb_y + 4, &pct_str, 0x000F172A, None);

            // Extraction Status Details
            let log_y = body_y + 110;
            fill_rounded_rect(fb, pitch, scr_w, scr_h, cx + 20, log_y, cw - 40, 100, 3, 0x001E293B);
            draw_text_px(fb, pitch, scr_w, scr_h, cx + 30, log_y + 10, "Status da Operacao:", 0x0038BDF8, None);
            draw_text_clipped(fb, pitch, scr_w, scr_h, cx + 30, log_y + 32, cx + cw - 30, &state.wizard_status, 0x004ADE80, None);

            if state.wizard_progress >= 100 {
                draw_text_px(fb, pitch, scr_w, scr_h, cx + 30, log_y + 60, "Instalacao Concluida com Sucesso! Binarios prontos para execucao.", 0x0034D399, None);
            } else {
                draw_text_px(fb, pitch, scr_w, scr_h, cx + 30, log_y + 60, "Clique em 'Extrair Proximo Bloco' abaixo para avancar a gravacao.", 0x00CBD5E1, None);
            }
        }
    }

    // Bottom Navigation Strip (48px)
    let nav_y = cy + ch.saturating_sub(48);
    fill_rounded_rect(fb, pitch, scr_w, scr_h, cx, nav_y, cw, 48, 0, 0x00F8FAFC);
    draw_rounded_rect_border(fb, pitch, scr_w, scr_h, cx, nav_y, cw, 1, 0, 0x00E2E8F0);

    let btn_next_x = (cx + cw).saturating_sub(115);
    let btn_back_x = (cx + cw).saturating_sub(220);
    let btn_cancel_x = (cx + cw).saturating_sub(325);

    match state.wizard_step {
        1 => {
            draw_glass_button(fb, pitch, scr_w, scr_h, btn_cancel_x, nav_y + 10, 95, 26, "Cancelar", false);
            draw_glass_button(fb, pitch, scr_w, scr_h, btn_next_x, nav_y + 10, 95, 26, "Avancar >", state.wizard_eula_accepted);
        }
        2 => {
            draw_glass_button(fb, pitch, scr_w, scr_h, btn_cancel_x, nav_y + 10, 95, 26, "Cancelar", false);
            draw_glass_button(fb, pitch, scr_w, scr_h, btn_back_x, nav_y + 10, 95, 26, "< Voltar", false);
            draw_glass_button(fb, pitch, scr_w, scr_h, btn_next_x, nav_y + 10, 95, 26, "Instalar", true);
        }
        _ => {
            if state.wizard_progress >= 100 {
                draw_glass_button(fb, pitch, scr_w, scr_h, btn_next_x, nav_y + 10, 95, 26, "Concluir", true);
            } else {
                draw_glass_button(fb, pitch, scr_w, scr_h, btn_next_x.saturating_sub(60), nav_y + 10, 160, 26, "Extrair Bloco", true);
            }
        }
    }
}

fn advance_wizard_step(desk: &mut DesktopState) {
    let mut state = CAT3_STATE.lock();
    match state.wizard_step {
        1 => {
            if state.wizard_eula_accepted {
                state.wizard_step = 2;
                state.wizard_status = String::from("Diretorio selecionado. Pronto para iniciar.");
            }
        }
        2 => {
            state.wizard_step = 3;
            state.wizard_progress = 25;
            state.wizard_status = String::from("Gravando manifesto de instalacao package.manifest no VFS...");

            // Step 1 Extraction: write manifest
            let perms = Permissions::from_mode(0o755);
            vfs::vfs_create_dir_all(&state.wizard_target_dir, perms).ok();
            let mf_path = format!("{}/package.manifest", state.wizard_target_dir);
            let mf_data = b"HybridOS Suite Manifest v1.0\nComponents: Core, Runtime, Drivers\n";
            vfs::vfs_create_file(&mf_path, mf_data, perms).ok();
            let mf_hash = sha256_hex(mf_data);
            state.wizard_status = format!("Gravado: {} (SHA-256: {})", mf_path, &mf_hash[..16]);
        }
        _ => {
            if state.wizard_progress < 100 {
                state.wizard_progress += 25;
                let perms = Permissions::from_mode(0o755);
                match state.wizard_progress {
                    50 => {
                        let pe_data = create_synthetic_pe();
                        let p1 = format!("{}/hybridos_core.dll", state.wizard_target_dir);
                        vfs::vfs_create_file(&p1, &pe_data, perms).ok();
                        let h1 = sha256_hex(&pe_data);
                        state.wizard_status = format!("Extraido: {} (PE32+ 64-bit OK | SHA-256: {})", p1, &h1[..16]);
                    }
                    75 => {
                        let elf_data = create_synthetic_elf();
                        let p2 = format!("{}/hybridos_daemon.elf", state.wizard_target_dir);
                        vfs::vfs_create_file(&p2, &elf_data, perms).ok();
                        let h2 = sha256_hex(&elf_data);
                        state.wizard_status = format!("Extraido: {} (Linux ELF64 OK | SHA-256: {})", p2, &h2[..16]);
                    }
                    _ => {
                        state.wizard_progress = 100;
                        let p3 = format!("{}/readme.txt", state.wizard_target_dir);
                        let r3 = b"HybridOS Package instalado com sucesso!\n";
                        vfs::vfs_create_file(&p3, r3, perms).ok();
                        let h3 = sha256_hex(r3);
                        state.wizard_status = format!("Extracao 100% concluida! Todos os arquivos sincronizados no VFS. (readme.txt SHA-256: {})", &h3[..16]);
                    }
                }
            } else {
                // Done clicked: reset wizard
                state.wizard_step = 1;
                state.wizard_progress = 0;
                state.wizard_eula_accepted = false;
                state.wizard_status = String::from("Instalacao finalizada com sucesso!");
            }
        }
    }
    desk.needs_full_redraw = true;
}

// =============================================================================
// PROGRAM 23: PROGRAMAS PADRÃO E ASSOCIAÇÕES DE EXTENSÕES
// =============================================================================

unsafe fn render_default_apps(
    fb: *mut u8, pitch: usize, scr_w: usize, scr_h: usize,
    cx: usize, cy: usize, cw: usize, ch: usize,
    _desk: &mut DesktopState,
) {
    let state = CAT3_STATE.lock();

    // 1. Top Glass Toolbar (36px)
    draw_glass_toolbar(fb, pitch, scr_w, scr_h, cx, cy, cw, 36);
    draw_text_px(fb, pitch, scr_w, scr_h, cx + 12, cy + 10, "Associacoes de Arquivos & Programas Padrao do HybridOS", 0x001E3A8A, None);

    // 2. Table Header Strip (24px)
    let header_y = cy + 36;
    fill_rounded_rect(fb, pitch, scr_w, scr_h, cx, header_y, cw, 24, 0, 0x00E2E8F0);
    draw_text_px(fb, pitch, scr_w, scr_h, cx + 12, header_y + 5, "Extensao", 0x00475569, None);
    draw_text_px(fb, pitch, scr_w, scr_h, cx + 90, header_y + 5, "Descricao do Tipo", 0x00475569, None);
    draw_text_px(fb, pitch, scr_w, scr_h, cx + 290, header_y + 5, "Programa Associado Padrao", 0x00475569, None);
    draw_text_px(fb, pitch, scr_w, scr_h, cx + 510, header_y + 5, "Subsistema", 0x00475569, None);

    // 3. Table Rows
    let row_start_y = header_y + 24;
    let table_h = ch.saturating_sub(36 + 24 + 48 + 22);
    fill_rounded_rect(fb, pitch, scr_w, scr_h, cx, row_start_y, cw, table_h, 0, 0x00FFFFFF);

    let mut ry = row_start_y;
    for (idx, mapping) in state.ext_mappings.iter().enumerate() {
        if ry + 22 > row_start_y + table_h { break; }
        let is_sel = state.selected_ext_idx == idx;

        let bg_col = if is_sel {
            0x00BAE6FD // Selected light cyan
        } else if idx % 2 == 1 {
            0x00F8FAFC
        } else {
            0x00FFFFFF
        };

        fill_rounded_rect(fb, pitch, scr_w, scr_h, cx, ry, cw, 22, 0, bg_col);

        let ext_col = if is_sel { 0x000C4A6E } else { 0x000284C7 };
        draw_text_px(fb, pitch, scr_w, scr_h, cx + 12, ry + 3, mapping.ext, ext_col, None);
        draw_text_clipped(fb, pitch, scr_w, scr_h, cx + 90, ry + 3, cx + 280, mapping.desc, 0x000F172A, None);
        draw_text_clipped(fb, pitch, scr_w, scr_h, cx + 290, ry + 3, cx + 500, &mapping.app_name, 0x001E3A8A, None);
        draw_text_px(fb, pitch, scr_w, scr_h, cx + 510, ry + 3, mapping.subsystem, 0x0016A34A, None);

        ry += 22;
    }

    // 4. Action Strip (48px)
    let act_y = row_start_y + table_h;
    fill_rounded_rect(fb, pitch, scr_w, scr_h, cx, act_y, cw, 48, 0, 0x00F8FAFC);
    draw_rounded_rect_border(fb, pitch, scr_w, scr_h, cx, act_y, cw, 1, 0, 0x00E2E8F0);

    draw_glass_button(fb, pitch, scr_w, scr_h, cx + 12, act_y + 11, 210, 26, "Alterar Programa...", true);
    draw_glass_button(fb, pitch, scr_w, scr_h, cx + 230, act_y + 11, 140, 26, "Restaurar Padroes", false);

    let save_btn_x = (cx + cw).saturating_sub(185);
    draw_glass_button(fb, pitch, scr_w, scr_h, save_btn_x, act_y + 11, 175, 26, "Salvar no VFS", true);

    // 5. Status Bar (22px)
    let status_y = cy + ch.saturating_sub(22);
    draw_glass_statusbar(fb, pitch, scr_w, scr_h, cx, status_y, cw, 22, &state.ext_status, "Arquivo: /etc/mime.types & associations.ini");
}

fn cycle_default_app_mapping(desk: &mut DesktopState) {
    let mut state = CAT3_STATE.lock();
    let idx = state.selected_ext_idx;
    if idx >= state.ext_mappings.len() { return; }

    let current = state.ext_mappings[idx].app_name.clone();
    let next_app = match current.as_str() {
        "Bloco de Notas (Notepad)" => "Terminal Tri-Shell (Bash)",
        "Terminal Tri-Shell (Bash)" => "Terminal Tri-Shell (CMD)",
        "Terminal Tri-Shell (CMD)" => "Visualizador de Midia",
        "Visualizador de Midia" => "Carregador Dual-ABI",
        "Carregador Dual-ABI" => "Descompactador / Hex",
        _ => "Bloco de Notas (Notepad)",
    };

    state.ext_mappings[idx].app_name = String::from(next_app);
    state.ext_status = format!("Extensao '{}' alterada para '{}'.", state.ext_mappings[idx].ext, next_app);
    desk.needs_full_redraw = true;
}

fn save_associations_to_vfs(desk: &mut DesktopState) {
    let mut state = CAT3_STATE.lock();
    let mut mime_content = String::from("# HybridOS File Association Table\n");
    let mut ini_content = String::from("[FileAssociations]\n");

    for m in state.ext_mappings.iter() {
        mime_content.push_str(&format!("{} {}\n", m.ext, m.app_name));
        ini_content.push_str(&format!("{}={}\n", m.ext, m.app_name));
    }

    let perms = Permissions::from_mode(0o644);
    vfs::vfs_create_file("/etc/mime.types", mime_content.as_bytes(), perms).ok();
    vfs::vfs_create_file("C:\\Windows\\associations.ini", ini_content.as_bytes(), perms).ok();

    state.ext_status = String::from("Associacoes gravadas com sucesso em /etc/mime.types e C:\\Windows\\associations.ini!");
    desk.needs_full_redraw = true;
}

// =============================================================================
// PROGRAM 24: CENTRO DE COMPATIBILIDADE DUAL-ABI
// =============================================================================

unsafe fn render_dual_abi_center(
    fb: *mut u8, pitch: usize, scr_w: usize, scr_h: usize,
    cx: usize, cy: usize, cw: usize, ch: usize,
    _desk: &mut DesktopState,
) {
    let state = CAT3_STATE.lock();

    // 1. Top Glass Toolbar (36px)
    draw_glass_toolbar(fb, pitch, scr_w, scr_h, cx, cy, cw, 36);
    draw_text_px(fb, pitch, scr_w, scr_h, cx + 12, cy + 10, "Centro de Compatibilidade Dual-ABI (ELF64 & PE32+)", 0x001E3A8A, None);

    // 2. Binary Chips Selector Strip (34px)
    let sel_y = cy + 36;
    fill_rounded_rect(fb, pitch, scr_w, scr_h, cx, sel_y, cw, 34, 0, 0x00F8FAFC);
    draw_rounded_rect_border(fb, pitch, scr_w, scr_h, cx, sel_y + 33, cw, 1, 0, 0x00CBD5E1);

    draw_text_px(fb, pitch, scr_w, scr_h, cx + 12, sel_y + 9, "Inspecionar:", 0x00475569, None);

    let mut chip_x = cx + 100;
    for (c_idx, label) in SAMPLE_BIN_LABELS.iter().enumerate() {
        let is_act = state.selected_bin_chip == c_idx;
        let c_w = label.len() * 8 + 14;
        if chip_x + c_w > cx + cw.saturating_sub(10) { break; }
        draw_glass_button(fb, pitch, scr_w, scr_h, chip_x, sel_y + 5, c_w, 24, label, is_act);
        chip_x += c_w + 6;
    }

    // 3. Main Inspection Panel (Split in 2 Columns)
    let body_y = sel_y + 34;
    let body_h = ch.saturating_sub(36 + 34 + 50 + 22);
    fill_rounded_rect(fb, pitch, scr_w, scr_h, cx, body_y, cw, body_h, 0, 0x00FFFFFF);

    let half_w = (cw.saturating_sub(24)) / 2;

    if let Some(ref info) = state.inspection {
        // Left Column: Header & Architecture Info
        let left_x = cx + 12;
        fill_rounded_rect(fb, pitch, scr_w, scr_h, left_x, body_y + 10, half_w, body_h.saturating_sub(20), 4, 0x00F8FAFC);
        draw_rounded_rect_border(fb, pitch, scr_w, scr_h, left_x, body_y + 10, half_w, body_h.saturating_sub(20), 4, 0x00E2E8F0);

        draw_text_px(fb, pitch, scr_w, scr_h, left_x + 10, body_y + 20, "Cabecalho e Metadados do Binario:", 0x001E3A8A, None);

        let rows = [
            ("Arquivo:", info.path.as_str()),
            ("Formato:", info.format_name.as_str()),
            ("Magic Bytes:", info.magic_hex.as_str()),
            ("Arquitetura:", info.machine_desc.as_str()),
            ("Ponto de Entrada:", info.entry_point_hex.as_str()),
            ("Endereco Base:", info.image_base_hex.as_str()),
            ("Subsistema:", info.subsystem_desc.as_str()),
        ];

        let mut row_y = body_y + 44;
        for (label, val) in rows.iter() {
            if row_y + 18 > body_y + body_h - 10 { break; }
            draw_text_px(fb, pitch, scr_w, scr_h, left_x + 12, row_y, label, 0x00475569, None);
            draw_text_clipped(fb, pitch, scr_w, scr_h, left_x + 140, row_y, left_x + half_w - 10, val, 0x000F172A, None);
            row_y += 22;
        }

        // Right Column: Sections & Imports Table
        let right_x = left_x + half_w + 12;
        fill_rounded_rect(fb, pitch, scr_w, scr_h, right_x, body_y + 10, half_w, body_h.saturating_sub(20), 4, 0x00F8FAFC);
        draw_rounded_rect_border(fb, pitch, scr_w, scr_h, right_x, body_y + 10, half_w, body_h.saturating_sub(20), 4, 0x00E2E8F0);

        draw_text_px(fb, pitch, scr_w, scr_h, right_x + 10, body_y + 20, "Tabela de Secoes do Binario:", 0x001E3A8A, None);

        // Section headers
        let sec_hdr_y = body_y + 44;
        fill_rounded_rect(fb, pitch, scr_w, scr_h, right_x + 8, sec_hdr_y, half_w - 16, 20, 0, 0x00E2E8F0);
        draw_text_px(fb, pitch, scr_w, scr_h, right_x + 12, sec_hdr_y + 3, "Nome", 0x00475569, None);
        draw_text_px(fb, pitch, scr_w, scr_h, right_x + 80, sec_hdr_y + 3, "Tipo", 0x00475569, None);
        draw_text_px(fb, pitch, scr_w, scr_h, right_x + 160, sec_hdr_y + 3, "Tamanho", 0x00475569, None);
        draw_text_px(fb, pitch, scr_w, scr_h, right_x + 230, sec_hdr_y + 3, "Flags", 0x00475569, None);

        let mut sec_y = sec_hdr_y + 22;
        for (name, stype, size, flags) in info.sections.iter() {
            if sec_y + 18 > body_y + 160 { break; }
            draw_text_px(fb, pitch, scr_w, scr_h, right_x + 12, sec_y, name, 0x000284C7, None);
            draw_text_px(fb, pitch, scr_w, scr_h, right_x + 80, sec_y, stype, 0x000F172A, None);
            draw_text_px(fb, pitch, scr_w, scr_h, right_x + 160, sec_y, size, 0x0016A34A, None);
            draw_text_clipped(fb, pitch, scr_w, scr_h, right_x + 230, sec_y, right_x + half_w - 10, flags, 0x0064748B, None);
            sec_y += 20;
        }

        // Header Summary / Telemetry
        draw_text_px(fb, pitch, scr_w, scr_h, right_x + 10, sec_y + 10, "Metadados e Resumo de Cabecalhos:", 0x001E3A8A, None);
        let mut imp_y = sec_y + 30;
        for imp in info.imports.iter() {
            if imp_y + 16 > body_y + body_h - 10 { break; }
            draw_text_clipped(fb, pitch, scr_w, scr_h, right_x + 12, imp_y, right_x + half_w - 10, imp, 0x00334155, None);
            imp_y += 18;
        }
    }

    // 4. Execution Dispatch Strip (50px)
    let disp_y = body_y + body_h;
    fill_rounded_rect(fb, pitch, scr_w, scr_h, cx, disp_y, cw, 50, 0, 0x00F8FAFC);
    draw_rounded_rect_border(fb, pitch, scr_w, scr_h, cx, disp_y, cw, 1, 0, 0x00E2E8F0);

    draw_glass_button(fb, pitch, scr_w, scr_h, cx + 16, disp_y + 11, 280, 28, "Disparar Execucao no Subsistema", true);

    let reload_btn_x = cx + 306;
    draw_glass_button(fb, pitch, scr_w, scr_h, reload_btn_x, disp_y + 11, 140, 28, "Reinspecionar", false);

    // 5. Status Bar (22px)
    let status_y = cy + ch.saturating_sub(22);
    draw_glass_statusbar(fb, pitch, scr_w, scr_h, cx, status_y, cw, 22, &state.abi_status, "Dual-ABI Dispatcher: Ring 0 Kernel");
}

fn dispatch_dual_abi_execution(desk: &mut DesktopState) {
    let mut state = CAT3_STATE.lock();
    let bin_path = if let Some(ref insp) = state.inspection {
        insp.path.clone()
    } else {
        String::from(SAMPLE_BIN_PATHS[0])
    };

    let data = match vfs::vfs_read_all(&bin_path) {
        Ok(bytes) => bytes,
        Err(_) => {
            state.abi_status = format!("[ERRO] Arquivo nao encontrado no VFS: {}", bin_path);
            desk.term_lines.push(format!("Dual-ABI Dispatch: {}", state.abi_status));
            desk.needs_full_redraw = true;
            return;
        }
    };

    if data.len() >= 4 && &data[0..4] == b"\x7fELF" {
        // Spawn real ELF process
        let res = spawn_elf_process(&bin_path, &data);
        match res {
            Ok(_) => {
                state.abi_status = format!("Processo ELF64 '{}' despachado com sucesso no escalonador!", bin_path);
            }
            Err(e) => {
                state.abi_status = format!("[ERRO] Falha ELF64 '{}': {}", bin_path, e);
            }
        }
    } else if data.len() >= 2 && &data[0..2] == b"MZ" {
        // Spawn real PE process
        let res = spawn_pe_process(&bin_path, &data);
        match res {
            Ok(_) => {
                state.abi_status = format!("Processo PE32+ '{}' despachado com sucesso no subsistema Windows NT!", bin_path);
            }
            Err(e) => {
                state.abi_status = format!("[ERRO] Falha PE32+ '{}': {}", bin_path, e);
            }
        }
    } else {
        state.abi_status = format!("[ERRO] Formato binario nao suportado para '{}'", bin_path);
    }

    desk.term_lines.push(format!("Dual-ABI Dispatch: {}", state.abi_status));
    desk.needs_full_redraw = true;
}

// =============================================================================
// PROGRAM 25: EDITOR DE VARIÁVEIS DE AMBIENTE
// =============================================================================

unsafe fn render_env_variables(
    fb: *mut u8, pitch: usize, scr_w: usize, scr_h: usize,
    cx: usize, cy: usize, cw: usize, ch: usize,
    _desk: &mut DesktopState,
) {
    let state = CAT3_STATE.lock();

    // 1. Top Glass Toolbar (36px)
    draw_glass_toolbar(fb, pitch, scr_w, scr_h, cx, cy, cw, 36);
    draw_text_px(fb, pitch, scr_w, scr_h, cx + 12, cy + 10, "Gerenciador de Variaveis de Ambiente do Sistema e Usuario", 0x001E3A8A, None);

    // 2. Body Area (Split between User Variables and System Variables)
    let body_y = cy + 36;
    let body_h = ch.saturating_sub(36 + 48 + 22);
    fill_rounded_rect(fb, pitch, scr_w, scr_h, cx, body_y, cw, body_h, 0, 0x00FFFFFF);

    let half_h = (body_h.saturating_sub(10)) / 2;

    // Table 1: User Variables (Top Half)
    let u_hdr_y = body_y + 4;
    draw_text_px(fb, pitch, scr_w, scr_h, cx + 12, u_hdr_y + 2, "Variaveis de Usuario para Administrator:", 0x000F172A, None);

    fill_rounded_rect(fb, pitch, scr_w, scr_h, cx + 8, u_hdr_y + 20, cw - 16, 20, 0, 0x00E2E8F0);
    draw_text_px(fb, pitch, scr_w, scr_h, cx + 14, u_hdr_y + 23, "Variavel", 0x00475569, None);
    draw_text_px(fb, pitch, scr_w, scr_h, cx + 180, u_hdr_y + 23, "Valor", 0x00475569, None);

    let mut uy = u_hdr_y + 42;
    for (u_idx, (k, v)) in state.user_env.iter().enumerate() {
        if uy + 20 > u_hdr_y + half_h { break; }
        let is_sel = state.is_user_env_selected && state.selected_env_idx == u_idx;
        let bg_col = if is_sel { 0x00BAE6FD } else if u_idx % 2 == 1 { 0x00F8FAFC } else { 0x00FFFFFF };
        fill_rounded_rect(fb, pitch, scr_w, scr_h, cx + 8, uy, cw - 16, 20, 0, bg_col);

        let k_col = if is_sel { 0x000C4A6E } else { 0x000284C7 };
        draw_text_px(fb, pitch, scr_w, scr_h, cx + 14, uy + 2, k, k_col, None);
        draw_text_clipped(fb, pitch, scr_w, scr_h, cx + 180, uy + 2, cx + cw - 20, v, 0x000F172A, None);
        uy += 20;
    }

    // Table 2: System Variables (Bottom Half)
    let s_hdr_y = body_y + half_h + 6;
    draw_text_px(fb, pitch, scr_w, scr_h, cx + 12, s_hdr_y + 2, "Variaveis do Sistema (Kernel & Subsystems):", 0x000F172A, None);

    fill_rounded_rect(fb, pitch, scr_w, scr_h, cx + 8, s_hdr_y + 20, cw - 16, 20, 0, 0x00E2E8F0);
    draw_text_px(fb, pitch, scr_w, scr_h, cx + 14, s_hdr_y + 23, "Variavel", 0x00475569, None);
    draw_text_px(fb, pitch, scr_w, scr_h, cx + 180, s_hdr_y + 23, "Valor", 0x00475569, None);

    let mut sy = s_hdr_y + 42;
    for (s_idx, (k, v)) in state.sys_env.iter().enumerate() {
        if sy + 20 > body_y + body_h { break; }
        let is_sel = !state.is_user_env_selected && state.selected_env_idx == s_idx;
        let bg_col = if is_sel { 0x00BAE6FD } else if s_idx % 2 == 1 { 0x00F8FAFC } else { 0x00FFFFFF };
        fill_rounded_rect(fb, pitch, scr_w, scr_h, cx + 8, sy, cw - 16, 20, 0, bg_col);

        let k_col = if is_sel { 0x000C4A6E } else { 0x000284C7 };
        draw_text_px(fb, pitch, scr_w, scr_h, cx + 14, sy + 2, k, k_col, None);
        draw_text_clipped(fb, pitch, scr_w, scr_h, cx + 180, sy + 2, cx + cw - 20, v, 0x000F172A, None);
        sy += 20;
    }

    // 3. Action Strip (48px)
    let act_y = body_y + body_h;
    fill_rounded_rect(fb, pitch, scr_w, scr_h, cx, act_y, cw, 48, 0, 0x00F8FAFC);
    draw_rounded_rect_border(fb, pitch, scr_w, scr_h, cx, act_y, cw, 1, 0, 0x00E2E8F0);

    draw_glass_button(fb, pitch, scr_w, scr_h, cx + 12, act_y + 11, 140, 26, "Alternar Tabela", false);
    draw_glass_button(fb, pitch, scr_w, scr_h, cx + 160, act_y + 11, 80, 26, "Nova...", false);
    draw_glass_button(fb, pitch, scr_w, scr_h, cx + 248, act_y + 11, 80, 26, "Editar...", false);
    draw_glass_button(fb, pitch, scr_w, scr_h, cx + 336, act_y + 11, 80, 26, "Excluir", false);

    let save_btn_x = (cx + cw).saturating_sub(170);
    draw_glass_button(fb, pitch, scr_w, scr_h, save_btn_x, act_y + 11, 160, 26, "Salvar no VFS", true);

    // 4. Status Bar (22px)
    let status_y = cy + ch.saturating_sub(22);
    draw_glass_statusbar(fb, pitch, scr_w, scr_h, cx, status_y, cw, 22, &state.env_status, "Destino: /etc/environment & C:\\system.env");
}

fn save_env_variables_to_vfs(desk: &mut DesktopState) {
    let mut state = CAT3_STATE.lock();
    let mut env_content = String::from("# HybridOS System Environment Variables\n");

    for (k, v) in state.sys_env.iter() {
        env_content.push_str(&format!("{}={}\n", k, v));
    }
    for (k, v) in state.user_env.iter() {
        env_content.push_str(&format!("USER_{}={}\n", k, v));
    }

    let perms = Permissions::from_mode(0o644);
    vfs::vfs_create_file("/etc/environment", env_content.as_bytes(), perms).ok();
    vfs::vfs_create_file("C:\\Windows\\system.env", env_content.as_bytes(), perms).ok();

    state.env_status = String::from("Variaveis salvas com sucesso em /etc/environment e C:\\Windows\\system.env!");
    desk.needs_full_redraw = true;
}

// =============================================================================
// PUBLIC INTERFACES: RENDER & HITBOX DISPATCHERS
// =============================================================================

/// Top-level dispatcher to render Category 3 applications (Programs 19 to 25).
pub unsafe fn render_cat3_app(
    id: u8,
    fb: *mut u8, pitch: usize, scr_w: usize, scr_h: usize,
    cx: usize, cy: usize, cw: usize, ch: usize,
    desk: &mut DesktopState,
) {
    ensure_cat3_state_initialized();

    // Cache the active window canvas rect for click coordinate translation
    {
        let mut st = CAT3_STATE.lock();
        st.last_canvas_rect = (cx, cy, cw, ch);
    }

    match id {
        19 | 0 => render_terminal_trishell(fb, pitch, scr_w, scr_h, cx, cy, cw, ch, desk),
        20 => render_run_dialog(fb, pitch, scr_w, scr_h, cx, cy, cw, ch, desk),
        21 | 8 => render_app_store(fb, pitch, scr_w, scr_h, cx, cy, cw, ch, desk),
        22 => render_setup_wizard(fb, pitch, scr_w, scr_h, cx, cy, cw, ch, desk),
        23 => render_default_apps(fb, pitch, scr_w, scr_h, cx, cy, cw, ch, desk),
        24 => render_dual_abi_center(fb, pitch, scr_w, scr_h, cx, cy, cw, ch, desk),
        25 => render_env_variables(fb, pitch, scr_w, scr_h, cx, cy, cw, ch, desk),
        _ => {}
    }
}

/// Top-level click event handler with hitboxes for all buttons and interactive controls.
pub fn handle_cat3_click(
    id: u8,
    click_x: usize,
    click_y: usize,
    desk: &mut DesktopState,
) -> bool {
    ensure_cat3_state_initialized();

    let canonical_id = match id {
        0 => 19,
        8 => 21,
        other => other,
    };

    let (wx, wy, ww, wh) = desk
        .windows
        .iter()
        .find(|w| w.id.as_u8() == canonical_id || w.id.as_u8() == id)
        .map(|w| {
            (
                w.x + 4,
                w.y + 32,
                w.w.saturating_sub(8),
                w.h.saturating_sub(36),
            )
        })
        .unwrap_or_else(|| {
            let st = CAT3_STATE.lock();
            st.last_canvas_rect
        });

    // Translate coordinate to client-relative
    let (rx, ry) = if click_x >= wx && click_x < wx + ww && click_y >= wy && click_y < wy + wh {
        (click_x - wx, click_y - wy)
    } else if click_x < ww && click_y < wh {
        (click_x, click_y)
    } else {
        return false;
    };

    match id {
        19 | 0 => {
            // Program 19: Terminal Tri-Shell
            // Toolbar (cy: 0..36)
            if ry <= 36 {
                if rx >= 90 && rx <= 158 {
                    // [ Bash ]
                    let mut st = CAT3_STATE.lock();
                    st.shell_mode = ShellMode::Bash;
                    st.shell_status = String::from("Modo alterado para Bash POSIX.");
                    desk.needs_full_redraw = true;
                    return true;
                } else if rx >= 162 && rx <= 222 {
                    // [ CMD ]
                    let mut st = CAT3_STATE.lock();
                    st.shell_mode = ShellMode::Cmd;
                    st.shell_status = String::from("Modo alterado para CMD Win32.");
                    desk.needs_full_redraw = true;
                    return true;
                } else if rx >= 226 && rx <= 322 {
                    // [ PowerShell ]
                    let mut st = CAT3_STATE.lock();
                    st.shell_mode = ShellMode::PowerShell;
                    st.shell_status = String::from("Modo alterado para Windows PowerShell.");
                    desk.needs_full_redraw = true;
                    return true;
                } else if rx >= 330 && rx <= 405 {
                    // Chip [ uname -a ]
                    execute_trishell_command("uname -a", desk);
                    return true;
                } else if rx >= 409 && rx <= 459 {
                    // Chip [ ls / ]
                    execute_trishell_command("ls /", desk);
                    return true;
                } else if rx >= 463 && rx <= 528 {
                    // Chip [ dir C:\ ]
                    execute_trishell_command("dir C:\\", desk);
                    return true;
                } else if rx >= 532 && rx <= 602 {
                    // Chip [ meminfo ]
                    execute_trishell_command("meminfo", desk);
                    return true;
                } else if rx >= ww.saturating_sub(75) {
                    // [ Limpar ]
                    execute_trishell_command("clear", desk);
                    return true;
                }
            }

            // Command input bar execution (near bottom)
            let input_y = wh.saturating_sub(54);
            if ry >= input_y && ry <= input_y + 32 {
                if rx >= ww.saturating_sub(90) {
                    let cmd_to_run = {
                        let st = CAT3_STATE.lock();
                        if st.term_input.is_empty() {
                            String::from("help")
                        } else {
                            st.term_input.clone()
                        }
                    };
                    execute_trishell_command(&cmd_to_run, desk);
                    return true;
                }
            }
            true
        }
        20 => {
            // Program 20: Caixa Executar (Run Dialog)
            let box_y = 70;
            // Combobox text or dropdown button
            if ry >= box_y && ry <= box_y + 26 {
                let input_w = ww.saturating_sub(130).max(120);
                let drop_btn_x = 68 + input_w + 4;
                if rx >= drop_btn_x && rx <= drop_btn_x + 45 {
                    // Cycle history
                    let mut st = CAT3_STATE.lock();
                    if !st.run_history.is_empty() {
                        st.browse_index = (st.browse_index + 1) % st.run_history.len();
                        st.run_input = st.run_history[st.browse_index].clone();
                        st.run_status = format!("Selecionado do historico: {}", st.run_input);
                        desk.needs_full_redraw = true;
                    }
                    return true;
                }
            }

            // Quick chips
            if ry >= 108 && ry <= 130 {
                let history_chips = ["taskmgr", "explorer", "cmd", "bash", "calc", "notepad", "winget"];
                let mut chip_x = 80usize;
                for chip in history_chips.iter() {
                    let chip_w = chip.len() * 8 + 14;
                    if rx >= chip_x && rx <= chip_x + chip_w {
                        let mut st = CAT3_STATE.lock();
                        st.run_input = String::from(*chip);
                        st.run_status = format!("Comando pronto: {}", chip);
                        desk.needs_full_redraw = true;
                        return true;
                    }
                    chip_x += chip_w + 6;
                }
            }

            // Bottom Buttons
            let bottom_y = wh.saturating_sub(44);
            if ry >= bottom_y {
                let btn_ok_x = ww.saturating_sub(290);
                let btn_cancel_x = ww.saturating_sub(205);
                let btn_browse_x = ww.saturating_sub(120);

                if rx >= btn_ok_x && rx < btn_ok_x + 80 {
                    // [ OK ]
                    execute_run_dialog_action(desk);
                    return true;
                } else if rx >= btn_cancel_x && rx < btn_cancel_x + 80 {
                    // [ Cancelar ]
                    let mut st = CAT3_STATE.lock();
                    st.run_input.clear();
                    st.run_status = String::from("Operacao cancelada.");
                    desk.needs_full_redraw = true;
                    return true;
                } else if rx >= btn_browse_x {
                    // [ Procurar... ]
                    let mut st = CAT3_STATE.lock();
                    st.browse_index = (st.browse_index + 1) % SAMPLE_BIN_PATHS.len();
                    st.run_input = String::from(SAMPLE_BIN_PATHS[st.browse_index]);
                    st.run_status = format!("Localizado no VFS: {}", st.run_input);
                    desk.needs_full_redraw = true;
                    return true;
                }
            }
            true
        }
        21 | 8 => {
            // Program 21: Central de Programas (App Store)
            // Tabs Strip (ry: 0..32)
            if ry <= 32 {
                let tab_names = ["Destaques", "Ferramentas", "Jogos", "Desenvolvedor"];
                let mut tab_x = 8usize;
                for (idx, tname) in tab_names.iter().enumerate() {
                    let tab_w = tname.len() * 8 + 24;
                    if rx >= tab_x && rx <= tab_x + tab_w {
                        let mut st = CAT3_STATE.lock();
                        st.store_tab = idx;
                        st.store_search_focused = false;
                        st.store_status = format!("Aba selecionada: {}", tname);
                        desk.needs_full_redraw = true;
                        return true;
                    }
                    tab_x += tab_w + 6;
                }
            }

            // Search Bar (ry: 32..66)
            if ry > 32 && ry <= 66 {
                let s_box_w = ww.saturating_sub(215).max(120);
                let clear_btn_x = 125 + s_box_w + 6;
                let mut st = CAT3_STATE.lock();
                if rx >= clear_btn_x && rx <= clear_btn_x + 72 {
                    st.store_search.clear();
                    st.store_search_focused = false;
                    st.store_status = String::from("Filtro de busca limpo.");
                    desk.needs_full_redraw = true;
                    return true;
                } else if rx >= 12 && rx < clear_btn_x {
                    if !st.store_search_focused {
                        st.store_search_focused = true;
                        st.store_status = String::from("Busca ativa: digite para filtrar (Enter/Esc para sair).");
                    } else if !st.store_search.is_empty() {
                        st.store_search.clear();
                        st.store_status = String::from("Busca limpa. Digite novo filtro.");
                    } else {
                        st.store_search_focused = false;
                        st.store_status = String::from("Busca desfocada.");
                    }
                    desk.needs_full_redraw = true;
                    return true;
                }
            }

            // Card Install Buttons Click (synchronized with render_app_store filter)
            let grid_y = 66usize;
            let grid_h = wh.saturating_sub(32 + 34 + 24);
            let card_w = (ww.saturating_sub(36)) / 2;
            let card_h = 100usize;

            let (cur_tab, cur_search) = {
                let st = CAT3_STATE.lock();
                (st.store_tab, st.store_search.clone())
            };
            let filtered = filter_store_packages(cur_tab, &cur_search);

            for (idx, pkg) in filtered.iter().enumerate() {
                let col_idx = idx % 2;
                let row_idx = idx / 2;

                let card_x = 12 + col_idx * (card_w + 12);
                let card_y = grid_y + 12 + row_idx * (card_h + 10);

                if card_y + card_h > grid_y + grid_h { break; }

                let btn_x = card_x + card_w.saturating_sub(95);
                let btn_y = card_y + card_h.saturating_sub(34);

                if rx >= btn_x && rx <= btn_x + 85 && ry >= btn_y && ry <= btn_y + 24 {
                    {
                        let mut st = CAT3_STATE.lock();
                        st.store_search_focused = false;
                    }
                    install_software_package(pkg.id, desk);
                    return true;
                }
            }
            true
        }
        22 => {
            // Program 22: Assistente de Instalação (Setup Wizard)
            let (cur_step, progress) = {
                let st = CAT3_STATE.lock();
                (st.wizard_step, st.wizard_progress)
            };

            if cur_step == 1 {
                // Checkbox toggle
                let box_h = wh.saturating_sub(52 + 48 + 60);
                let chk_y = 52 + 14 + box_h + 10;
                if ry >= chk_y && ry <= chk_y + 20 && rx >= 20 && rx <= 400 {
                    let mut st = CAT3_STATE.lock();
                    st.wizard_eula_accepted = !st.wizard_eula_accepted;
                    desk.needs_full_redraw = true;
                    return true;
                }
            } else if cur_step == 2 {
                // Predefined path chips
                let chips = ["C:\\Program Files\\HybridOS_Package", "/opt/hybridos_package", "/bin/package", "C:\\Users\\Default\\AppData"];
                let mut chip_y = 52 + 110;
                for chip in chips.iter() {
                    if ry >= chip_y && ry <= chip_y + 24 && rx >= 20 && rx <= 340 {
                        let mut st = CAT3_STATE.lock();
                        st.wizard_target_dir = String::from(*chip);
                        desk.needs_full_redraw = true;
                        return true;
                    }
                    chip_y += 30;
                }
            }

            // Bottom Buttons
            let nav_y = wh.saturating_sub(48);
            if ry >= nav_y {
                let btn_next_x = ww.saturating_sub(115);
                let btn_back_x = ww.saturating_sub(220);
                let btn_cancel_x = ww.saturating_sub(325);

                if cur_step >= 3 && progress < 100 {
                    if rx >= ww.saturating_sub(175) && rx <= ww.saturating_sub(15) {
                        advance_wizard_step(desk);
                        return true;
                    }
                } else if rx >= btn_next_x && rx <= btn_next_x + 95 {
                    advance_wizard_step(desk);
                    return true;
                } else if cur_step == 2 && rx >= btn_back_x && rx <= btn_back_x + 95 {
                    let mut st = CAT3_STATE.lock();
                    st.wizard_step = 1;
                    desk.needs_full_redraw = true;
                    return true;
                } else if (cur_step == 1 || cur_step == 2) && rx >= btn_cancel_x && rx <= btn_cancel_x + 95 {
                    let mut st = CAT3_STATE.lock();
                    st.wizard_step = 1;
                    st.wizard_progress = 0;
                    st.wizard_status = String::from("Instalacao cancelada pelo usuario.");
                    desk.needs_full_redraw = true;
                    return true;
                }
            }
            true
        }
        23 => {
            // Program 23: Programas Padrão e Associações de Extensões
            let header_y = 36usize;
            let row_start_y = header_y + 24;
            let table_h = wh.saturating_sub(36 + 24 + 48 + 22);

            // Row Selection Click
            if ry >= row_start_y && ry <= row_start_y + table_h {
                let clicked_idx = (ry - row_start_y) / 22;
                let mut st = CAT3_STATE.lock();
                if clicked_idx < st.ext_mappings.len() {
                    st.selected_ext_idx = clicked_idx;
                    st.ext_status = format!("Selecionado: {} -> {}", st.ext_mappings[clicked_idx].ext, st.ext_mappings[clicked_idx].app_name);
                    desk.needs_full_redraw = true;
                    return true;
                }
            }

            // Bottom Action Strip
            let act_y = row_start_y + table_h;
            if ry >= act_y && ry <= act_y + 48 {
                if rx >= 12 && rx <= 222 {
                    // [ Alterar Programa... ]
                    cycle_default_app_mapping(desk);
                    return true;
                } else if rx >= 230 && rx <= 370 {
                    // [ Restaurar Padroes ]
                    let mut st = CAT3_STATE.lock();
                    st.ext_mappings = default_ext_mappings();
                    st.ext_status = String::from("Associacoes restauradas para os padroes de fabrica.");
                    desk.needs_full_redraw = true;
                    return true;
                } else if rx >= ww.saturating_sub(185) {
                    // [ Salvar no VFS ]
                    save_associations_to_vfs(desk);
                    return true;
                }
            }
            true
        }
        24 => {
            // Program 24: Centro de Compatibilidade Dual-ABI
            let sel_y = 36usize;
            // Chips row click (aligned with render_dual_abi_center short labels & bounds guard)
            if ry >= sel_y && ry <= sel_y + 34 {
                let mut chip_x = 100usize;
                for (c_idx, label) in SAMPLE_BIN_LABELS.iter().enumerate() {
                    let c_w = label.len() * 8 + 14;
                    if chip_x + c_w > ww.saturating_sub(10) { break; }
                    if rx >= chip_x && rx <= chip_x + c_w {
                        let path = SAMPLE_BIN_PATHS[c_idx];
                        let insp = inspect_target_path(path);
                        let mut st = CAT3_STATE.lock();
                        st.selected_bin_chip = c_idx;
                        st.abi_status = format!("Inspecionado: {} ({})", path, insp.format_name);
                        st.inspection = Some(insp);
                        desk.needs_full_redraw = true;
                        return true;
                    }
                    chip_x += c_w + 6;
                }
            }

            // Dispatch Action Button Click
            let disp_y = wh.saturating_sub(50 + 22);
            if ry >= disp_y && ry <= disp_y + 50 {
                if rx >= 16 && rx <= 296 {
                    // [ Disparar Execucao no Subsistema ]
                    dispatch_dual_abi_execution(desk);
                    return true;
                } else if rx >= 306 && rx <= 446 {
                    // [ Reinspecionar ]
                    let cur_path = {
                        let st = CAT3_STATE.lock();
                        if let Some(ref insp) = st.inspection {
                            insp.path.clone()
                        } else {
                            String::from(SAMPLE_BIN_PATHS[0])
                        }
                    };
                    let insp = inspect_target_path(&cur_path);
                    let mut st = CAT3_STATE.lock();
                    st.abi_status = format!("Reinspecionado: {} ({})", cur_path, insp.format_name);
                    st.inspection = Some(insp);
                    desk.needs_full_redraw = true;
                    return true;
                }
            }
            true
        }
        25 => {
            // Program 25: Editor de Variáveis de Ambiente
            let body_y = 36usize;
            let body_h = wh.saturating_sub(36 + 48 + 22);
            let half_h = (body_h.saturating_sub(10)) / 2;

            // Click inside User Table
            let u_hdr_y = body_y + 4;
            if ry >= u_hdr_y + 42 && ry < u_hdr_y + half_h {
                let clicked_idx = (ry - (u_hdr_y + 42)) / 20;
                let mut st = CAT3_STATE.lock();
                if clicked_idx < st.user_env.len() {
                    st.is_user_env_selected = true;
                    st.selected_env_idx = clicked_idx;
                    st.env_status = format!("Usuario: {} = {}", st.user_env[clicked_idx].0, st.user_env[clicked_idx].1);
                    desk.needs_full_redraw = true;
                    return true;
                }
            }

            // Click inside System Table
            let s_hdr_y = body_y + half_h + 6;
            if ry >= s_hdr_y + 42 && ry < body_y + body_h {
                let clicked_idx = (ry - (s_hdr_y + 42)) / 20;
                let mut st = CAT3_STATE.lock();
                if clicked_idx < st.sys_env.len() {
                    st.is_user_env_selected = false;
                    st.selected_env_idx = clicked_idx;
                    st.env_status = format!("Sistema: {} = {}", st.sys_env[clicked_idx].0, st.sys_env[clicked_idx].1);
                    desk.needs_full_redraw = true;
                    return true;
                }
            }

            // Bottom Buttons
            let act_y = body_y + body_h;
            if ry >= act_y && ry <= act_y + 48 {
                if rx >= 12 && rx <= 152 {
                    // [ Alternar Tabela ]
                    let mut st = CAT3_STATE.lock();
                    st.is_user_env_selected = !st.is_user_env_selected;
                    st.selected_env_idx = 0;
                    st.env_status = if st.is_user_env_selected {
                        String::from("Foco: Variaveis de Usuario.")
                    } else {
                        String::from("Foco: Variaveis do Sistema.")
                    };
                    desk.needs_full_redraw = true;
                    return true;
                } else if rx >= 160 && rx <= 240 {
                    // [ Nova... ]
                    let mut st = CAT3_STATE.lock();
                    if st.is_user_env_selected {
                        let count = st.user_env.len();
                        st.user_env.push((format!("NEW_VAR_{}", count), String::from("default_value")));
                        st.selected_env_idx = count;
                    } else {
                        let count = st.sys_env.len();
                        st.sys_env.push((format!("SYS_VAR_{}", count), String::from("sys_default")));
                        st.selected_env_idx = count;
                    }
                    st.env_status = String::from("Nova variavel adicionada.");
                    desk.needs_full_redraw = true;
                    return true;
                } else if rx >= 248 && rx <= 328 {
                    // [ Editar... ]
                    let mut st = CAT3_STATE.lock();
                    let sel = st.selected_env_idx;
                    if st.is_user_env_selected && sel < st.user_env.len() {
                        let cur_val = st.user_env[sel].1.clone();
                        st.user_env[sel].1 = format!("{}_mod", cur_val);
                        st.env_status = format!("Variavel '{}' modificada.", st.user_env[sel].0);
                    } else if !st.is_user_env_selected && sel < st.sys_env.len() {
                        let cur_val = st.sys_env[sel].1.clone();
                        st.sys_env[sel].1 = format!("{}_mod", cur_val);
                        st.env_status = format!("Variavel '{}' modificada.", st.sys_env[sel].0);
                    }
                    desk.needs_full_redraw = true;
                    return true;
                } else if rx >= 336 && rx <= 416 {
                    // [ Excluir ]
                    let mut st = CAT3_STATE.lock();
                    let sel = st.selected_env_idx;
                    if st.is_user_env_selected && sel < st.user_env.len() {
                        let removed = st.user_env.remove(sel);
                        st.env_status = format!("Variavel '{}' excluida.", removed.0);
                        if st.selected_env_idx > 0 { st.selected_env_idx -= 1; }
                    } else if !st.is_user_env_selected && sel < st.sys_env.len() {
                        let removed = st.sys_env.remove(sel);
                        st.env_status = format!("Variavel '{}' excluida.", removed.0);
                        if st.selected_env_idx > 0 { st.selected_env_idx -= 1; }
                    }
                    desk.needs_full_redraw = true;
                    return true;
                } else if rx >= ww.saturating_sub(170) {
                    // [ Salvar no VFS ]
                    save_env_variables_to_vfs(desk);
                    return true;
                }
            }
            true
        }
        _ => true,
    }
}
