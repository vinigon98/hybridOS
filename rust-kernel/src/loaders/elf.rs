//! Safe ELF64 (Linux 64-bit Executable) Parser and Loader.

use crate::process::HybridProcess;
use crate::types::NtStatus;

pub const ELFMAG: [u8; 4] = [0x7F, b'E', b'L', b'F'];
pub const ELFCLASS64: u8 = 2;
pub const EM_X86_64: u16 = 62;
pub const PT_LOAD: u32 = 1;

pub fn load_elf(proc: &mut HybridProcess, buffer: &[u8]) -> Result<(), NtStatus> {
    if buffer.len() < 64 {
        return Err(NtStatus::INVALID_PARAMETER);
    }

    if &buffer[0..4] != ELFMAG {
        println!("[ELF Loader] Invalid ELF magic header");
        return Err(NtStatus::INVALID_PARAMETER);
    }

    if buffer[4] != ELFCLASS64 {
        println!("[ELF Loader] Non-64-bit ELF binary");
        return Err(NtStatus::NOT_IMPLEMENTED);
    }

    let machine = u16::from_le_bytes([buffer[18], buffer[19]]);
    if machine != EM_X86_64 {
        println!("[ELF Loader] Non-x86_64 architecture: {}", machine);
        return Err(NtStatus::NOT_IMPLEMENTED);
    }

    let entry_point = u64::from_le_bytes([
        buffer[24], buffer[25], buffer[26], buffer[27],
        buffer[28], buffer[29], buffer[30], buffer[31]
    ]);

    let phoff = u64::from_le_bytes([
        buffer[32], buffer[33], buffer[34], buffer[35],
        buffer[36], buffer[37], buffer[38], buffer[39]
    ]) as usize;

    let phentsize = u16::from_le_bytes([buffer[54], buffer[55]]) as usize;
    let phnum = u16::from_le_bytes([buffer[56], buffer[57]]) as usize;

    proc.entry_point = entry_point;

    println!("[ELF Loader - Rust] Linux ELF64 Binary Successfully Loaded:");
    println!("                   Entry Point:     0x{:016X}", proc.entry_point);
    println!("                   Program Headers: {} (offset 0x{:X})", phnum, phoff);

    let mut min_vaddr = u64::MAX;
    let mut total_memsz = 0;

    for i in 0..phnum {
        let offset = phoff + (i * phentsize);
        if offset + 56 <= buffer.len() {
            let p_type = u32::from_le_bytes([buffer[offset], buffer[offset + 1], buffer[offset + 2], buffer[offset + 3]]);
            let p_flags = u32::from_le_bytes([buffer[offset + 4], buffer[offset + 5], buffer[offset + 6], buffer[offset + 7]]);
            let p_vaddr = u64::from_le_bytes([
                buffer[offset + 16], buffer[offset + 17], buffer[offset + 18], buffer[offset + 19],
                buffer[offset + 20], buffer[offset + 21], buffer[offset + 22], buffer[offset + 23]
            ]);
            let _p_filesz = u64::from_le_bytes([
                buffer[offset + 32], buffer[offset + 33], buffer[offset + 34], buffer[offset + 35],
                buffer[offset + 36], buffer[offset + 37], buffer[offset + 38], buffer[offset + 39]
            ]);
            let p_memsz = u64::from_le_bytes([
                buffer[offset + 40], buffer[offset + 41], buffer[offset + 42], buffer[offset + 43],
                buffer[offset + 44], buffer[offset + 45], buffer[offset + 46], buffer[offset + 47]
            ]);

            if p_type == PT_LOAD {
                if p_vaddr < min_vaddr {
                    min_vaddr = p_vaddr;
                }
                total_memsz += p_memsz;
                let r = if p_flags & 4 != 0 { 'R' } else { '-' };
                let w = if p_flags & 2 != 0 { 'W' } else { '-' };
                let x = if p_flags & 1 != 0 { 'X' } else { '-' };
                println!("                   - Segment PT_LOAD: VAddr=0x{:016X}, MemSz=0x{:X}, Flags=[{}{}{}]",
                         p_vaddr, p_memsz, r, w, x);
            }
        }
    }

    proc.image_base = if min_vaddr == u64::MAX { 0 } else { min_vaddr };
    proc.image_size = total_memsz as usize;

    Ok(())
}
