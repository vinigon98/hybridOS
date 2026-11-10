//! Safe PE32+ (Windows 64-bit Executable) Parser and Loader.

use crate::process::HybridProcess;
use crate::types::NtStatus;

pub const IMAGE_DOS_SIGNATURE: u16 = 0x5A4D; // "MZ"
pub const IMAGE_NT_SIGNATURE: u32 = 0x00004550; // "PE\0\0"
pub const IMAGE_FILE_MACHINE_AMD64: u16 = 0x8664; // x86_64
pub const OPTIONAL_HEADER_MAGIC_PE32_PLUS: u16 = 0x20B; // 64-bit PE

#[derive(Debug, Default)]
pub struct SectionHeader {
    pub name: String,
    pub virtual_size: u32,
    pub virtual_address: u32,
    pub raw_size: u32,
    pub raw_pointer: u32,
    pub characteristics: u32,
}

pub fn load_pe(proc: &mut HybridProcess, buffer: &[u8]) -> Result<(), NtStatus> {
    if buffer.len() < 64 {
        return Err(NtStatus::INVALID_PARAMETER);
    }

    // Verify DOS Magic "MZ"
    let dos_magic = u16::from_le_bytes([buffer[0], buffer[1]]);
    if dos_magic != IMAGE_DOS_SIGNATURE {
        println!("[PE Loader] Invalid DOS signature: 0x{:04X}", dos_magic);
        return Err(NtStatus::INVALID_PARAMETER);
    }

    let e_lfanew = u32::from_le_bytes([buffer[60], buffer[61], buffer[62], buffer[63]]) as usize;
    if e_lfanew + 264 > buffer.len() {
        println!("[PE Loader] Invalid e_lfanew offset: 0x{:X}", e_lfanew);
        return Err(NtStatus::INVALID_PARAMETER);
    }

    // Verify PE Signature "PE\0\0"
    let pe_sig = u32::from_le_bytes([
        buffer[e_lfanew], buffer[e_lfanew + 1], buffer[e_lfanew + 2], buffer[e_lfanew + 3]
    ]);
    if pe_sig != IMAGE_NT_SIGNATURE {
        println!("[PE Loader] Invalid PE signature: 0x{:08X}", pe_sig);
        return Err(NtStatus::INVALID_PARAMETER);
    }

    let file_header_offset = e_lfanew + 4;
    let machine = u16::from_le_bytes([buffer[file_header_offset], buffer[file_header_offset + 1]]);
    if machine != IMAGE_FILE_MACHINE_AMD64 {
        println!("[PE Loader] Non-AMD64 architecture: 0x{:04X}", machine);
        return Err(NtStatus::NOT_IMPLEMENTED);
    }

    let num_sections = u16::from_le_bytes([buffer[file_header_offset + 2], buffer[file_header_offset + 3]]);
    let size_of_opt_header = u16::from_le_bytes([buffer[file_header_offset + 16], buffer[file_header_offset + 17]]) as usize;

    let opt_offset = file_header_offset + 20;
    let magic = u16::from_le_bytes([buffer[opt_offset], buffer[opt_offset + 1]]);
    if magic != OPTIONAL_HEADER_MAGIC_PE32_PLUS {
        println!("[PE Loader] Not a 64-bit PE32+ binary (Magic: 0x{:04X})", magic);
        return Err(NtStatus::INVALID_PARAMETER);
    }

    let entry_point_rva = u32::from_le_bytes([
        buffer[opt_offset + 16], buffer[opt_offset + 17], buffer[opt_offset + 18], buffer[opt_offset + 19]
    ]) as u64;

    let image_base = u64::from_le_bytes([
        buffer[opt_offset + 24], buffer[opt_offset + 25], buffer[opt_offset + 26], buffer[opt_offset + 27],
        buffer[opt_offset + 28], buffer[opt_offset + 29], buffer[opt_offset + 30], buffer[opt_offset + 31]
    ]);

    let size_of_image = u32::from_le_bytes([
        buffer[opt_offset + 56], buffer[opt_offset + 57], buffer[opt_offset + 58], buffer[opt_offset + 59]
    ]) as usize;

    proc.image_base = image_base;
    proc.entry_point = image_base + entry_point_rva;
    proc.image_size = size_of_image;

    println!("[PE Loader - Rust] Windows NT PE32+ Binary Successfully Loaded:");
    println!("                  ImageBase:       0x{:016X}", proc.image_base);
    println!("                  Entry Point:     0x{:016X}", proc.entry_point);
    println!("                  SizeOfImage:     {} bytes", proc.image_size);
    println!("                  Section Count:   {}", num_sections);

    let sec_offset_start = opt_offset + size_of_opt_header;
    for i in 0..num_sections as usize {
        let sec_offset = sec_offset_start + (i * 40);
        if sec_offset + 40 <= buffer.len() {
            let name_bytes = &buffer[sec_offset..sec_offset + 8];
            let name = String::from_utf8_lossy(name_bytes).trim_end_matches('\0').to_string();
            let va = u32::from_le_bytes([buffer[sec_offset + 12], buffer[sec_offset + 13], buffer[sec_offset + 14], buffer[sec_offset + 15]]);
            let raw_size = u32::from_le_bytes([buffer[sec_offset + 16], buffer[sec_offset + 17], buffer[sec_offset + 18], buffer[sec_offset + 19]]);
            println!("                  - Section [{}]: VA=0x{:08X}, RawSize=0x{:08X}", name, va, raw_size);
        }
    }

    Ok(())
}
