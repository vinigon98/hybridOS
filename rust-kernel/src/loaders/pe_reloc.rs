// ============================================================================
// HybridOS Kernel — PE32+ Dynamic Linker & Base Relocator (RTL)
// Arquivo: rust-kernel/src/loaders/pe_reloc.rs
// Target: x86_64-unknown-none (#![no_std], alloc)
// ============================================================================

use alloc::vec::Vec;
use crate::types::NtStatus;

pub const IMAGE_REL_BASED_ABSOLUTE: u8 = 0;
pub const IMAGE_REL_BASED_HIGH: u8     = 1;
pub const IMAGE_REL_BASED_LOW: u8      = 2;
pub const IMAGE_REL_BASED_HIGHLOW: u8  = 3;
pub const IMAGE_REL_BASED_HIGHADJ: u8  = 4;
pub const IMAGE_REL_BASED_DIR64: u8    = 10;

#[derive(Debug, Clone, Copy)]
pub struct RelocationEntry {
    pub reloc_type: u8,
    pub offset: u16,
    pub rva: u32,
}

#[derive(Debug, Clone)]
pub struct RelocationBlock {
    pub page_rva: u32,
    pub block_size: u32,
    pub entries: Vec<RelocationEntry>,
}

/// Analisador bare-metal da seção .reloc (Image Base Relocation Table).
pub fn parse_base_relocations(
    raw_pe: &[u8],
    reloc_dir_rva: u32,
    reloc_dir_size: u32,
    _file_alignment: u32,
    sections: &[(u32, u32, u32)], // (VirtualAddress, PointerToRawData, SizeOfRawData)
) -> Result<Vec<RelocationBlock>, NtStatus> {
    if reloc_dir_size == 0 || reloc_dir_rva == 0 {
        return Ok(Vec::new());
    }

    // Tradução de RVA para File Offset
    let file_offset = rva_to_file_offset(reloc_dir_rva, sections)
        .ok_or(NtStatus::INVALID_PARAMETER)? as usize;

    let dir_size = reloc_dir_size as usize;
    if file_offset + dir_size > raw_pe.len() {
        return Err(NtStatus::BUFFER_TOO_SMALL);
    }

    let reloc_slice = &raw_pe[file_offset..file_offset + dir_size];
    let mut blocks = Vec::new();
    let mut offset = 0usize;

    while offset + 8 <= reloc_slice.len() {
        let page_rva = u32::from_le_bytes([
            reloc_slice[offset], reloc_slice[offset + 1],
            reloc_slice[offset + 2], reloc_slice[offset + 3],
        ]);
        let block_size = u32::from_le_bytes([
            reloc_slice[offset + 4], reloc_slice[offset + 5],
            reloc_slice[offset + 6], reloc_slice[offset + 7],
        ]);

        if block_size < 8 {
            break; // Fim da tabela ou bloco inválido
        }

        let num_entries = ((block_size - 8) / 2) as usize;
        let mut entries = Vec::with_capacity(num_entries);

        let entry_base = offset + 8;
        for i in 0..num_entries {
            let cur_off = entry_base + (i * 2);
            if cur_off + 2 > reloc_slice.len() {
                break;
            }
            let val = u16::from_le_bytes([reloc_slice[cur_off], reloc_slice[cur_off + 1]]);
            let reloc_type = ((val >> 12) & 0x0F) as u8;
            let page_offset = val & 0x0FFF;

            entries.push(RelocationEntry {
                reloc_type,
                offset: page_offset,
                rva: page_rva + (page_offset as u32),
            });
        }

        blocks.push(RelocationBlock {
            page_rva,
            block_size,
            entries,
        });

        offset += block_size as usize;
    }

    Ok(blocks)
}

/// Aplica as relocações sobre uma imagem de memória já projetada (Bare-Metal In-Memory Rebasing).
pub fn apply_image_base_relocations(
    image_memory: &mut [u8],
    blocks: &[RelocationBlock],
    preferred_base: u64,
    actual_base: u64,
) -> Result<usize, NtStatus> {
    if preferred_base == actual_base {
        return Ok(0);
    }

    let delta = actual_base.wrapping_sub(preferred_base);
    let mut count = 0usize;

    for block in blocks {
        for entry in &block.entries {
            let target = entry.rva as usize;

            match entry.reloc_type {
                IMAGE_REL_BASED_ABSOLUTE => {
                    // Padding de alinhamento; no-op obrigatório
                }
                IMAGE_REL_BASED_DIR64 => {
                    if target + 8 > image_memory.len() {
                        return Err(NtStatus::ACCESS_VIOLATION);
                    }
                    let current = u64::from_le_bytes([
                        image_memory[target],     image_memory[target + 1],
                        image_memory[target + 2], image_memory[target + 3],
                        image_memory[target + 4], image_memory[target + 5],
                        image_memory[target + 6], image_memory[target + 7],
                    ]);
                    let fixed = current.wrapping_add(delta);
                    image_memory[target..target + 8].copy_from_slice(&fixed.to_le_bytes());
                    count += 1;
                }
                IMAGE_REL_BASED_HIGHLOW => {
                    if target + 4 > image_memory.len() {
                        return Err(NtStatus::ACCESS_VIOLATION);
                    }
                    let current = u32::from_le_bytes([
                        image_memory[target],     image_memory[target + 1],
                        image_memory[target + 2], image_memory[target + 3],
                    ]);
                    let fixed = current.wrapping_add(delta as u32);
                    image_memory[target..target + 4].copy_from_slice(&fixed.to_le_bytes());
                    count += 1;
                }
                _ => {
                    // Tipos de relocação legados ignorados em x86_64
                }
            }
        }
    }

    Ok(count)
}

fn rva_to_file_offset(rva: u32, sections: &[(u32, u32, u32)]) -> Option<u32> {
    for &(sec_va, sec_ptr, sec_raw_size) in sections {
        if rva >= sec_va && rva < sec_va + sec_raw_size {
            return Some(sec_ptr + (rva - sec_va));
        }
    }
    Some(rva)
}
