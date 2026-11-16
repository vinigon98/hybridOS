// ============================================================================
// HybridOS Kernel — IAT Parser & Synthetic DLL Export Resolver
// Arquivo: rust-kernel/src/loaders/pe_iat.rs
// Target: x86_64-unknown-none (#![no_std], alloc)
// ============================================================================

use alloc::string::String;
use alloc::vec::Vec;
use crate::types::NtStatus;

pub const IMAGE_ORDINAL_FLAG64: u64 = 0x8000_0000_0000_0000;

// Base Virtual dos Thunks do Kernel em Ring 3
pub const WIN_STUB_BASE_NTDLL: u64    = 0x0000_7FFE_3000;
pub const WIN_STUB_BASE_KERNEL32: u64 = 0x0000_7FFE_3800;

#[derive(Debug, Clone)]
pub struct ImportModuleDescriptor {
    pub dll_name: String,
    pub first_thunk_rva: u32,
    pub original_first_thunk_rva: u32,
}

/// Resolve o ponteiro do stub sintético para uma função exportada do kernel
pub fn resolve_synthetic_export(dll_name: &str, function_name: &str) -> Option<u64> {
    let dll_lower = dll_name.to_ascii_lowercase();

    if dll_lower.starts_with("ntdll") {
        match function_name {
            "NtWriteFile"               => Some(WIN_STUB_BASE_NTDLL + 0x00),
            "NtReadFile"                => Some(WIN_STUB_BASE_NTDLL + 0x10),
            "NtCreateFile"              => Some(WIN_STUB_BASE_NTDLL + 0x20),
            "NtAllocateVirtualMemory"   => Some(WIN_STUB_BASE_NTDLL + 0x30),
            "NtClose"                   => Some(WIN_STUB_BASE_NTDLL + 0x40),
            "NtTerminateProcess"        => Some(WIN_STUB_BASE_NTDLL + 0x50),
            "NtYieldExecution"          => Some(WIN_STUB_BASE_NTDLL + 0x60),
            "NtQuerySystemInformation"  => Some(WIN_STUB_BASE_NTDLL + 0x70),
            "RtlInitUnicodeString"      => Some(WIN_STUB_BASE_NTDLL + 0x80),
            "RtlAllocateHeap"           => Some(WIN_STUB_BASE_NTDLL + 0x90),
            "RtlFreeHeap"               => Some(WIN_STUB_BASE_NTDLL + 0xA0),
            _ => None,
        }
    } else if dll_lower.starts_with("kernel32") {
        match function_name {
            "ExitProcess"          => Some(WIN_STUB_BASE_KERNEL32 + 0x00),
            "WriteFile"            => Some(WIN_STUB_BASE_KERNEL32 + 0x10),
            "ReadFile"             => Some(WIN_STUB_BASE_KERNEL32 + 0x20),
            "CreateFileA"          => Some(WIN_STUB_BASE_KERNEL32 + 0x30),
            "CreateFileW"          => Some(WIN_STUB_BASE_KERNEL32 + 0x40),
            "CloseHandle"          => Some(WIN_STUB_BASE_KERNEL32 + 0x50),
            "VirtualAlloc"         => Some(WIN_STUB_BASE_KERNEL32 + 0x60),
            "VirtualFree"          => Some(WIN_STUB_BASE_KERNEL32 + 0x70),
            "GetLastError"         => Some(WIN_STUB_BASE_KERNEL32 + 0x80),
            "SetLastError"         => Some(WIN_STUB_BASE_KERNEL32 + 0x90),
            "GetCurrentProcessId"  => Some(WIN_STUB_BASE_KERNEL32 + 0xA0),
            "GetCurrentThreadId"   => Some(WIN_STUB_BASE_KERNEL32 + 0xB0),
            "Sleep"                => Some(WIN_STUB_BASE_KERNEL32 + 0xC0),
            _ => None,
        }
    } else {
        None
    }
}

/// Percorre a diretiva IMAGE_DIRECTORY_ENTRY_IMPORT e vincula a IAT no buffer de memória
pub fn bind_import_address_table(
    image_memory: &mut [u8],
    _raw_pe: &[u8],
    import_dir_rva: u32,
    import_dir_size: u32,
    _sections: &[(u32, u32, u32)],
) -> Result<usize, NtStatus> {
    if import_dir_rva == 0 || import_dir_size == 0 {
        return Ok(0);
    }

    let mut desc_rva = import_dir_rva;
    let mut bound_count = 0usize;

    loop {
        let desc_offset = rva_to_image_offset(desc_rva, image_memory.len())?;
        if desc_offset + 20 > image_memory.len() {
            break;
        }

        let orig_first_thunk = u32::from_le_bytes([
            image_memory[desc_offset],     image_memory[desc_offset + 1],
            image_memory[desc_offset + 2], image_memory[desc_offset + 3],
        ]);
        let name_rva = u32::from_le_bytes([
            image_memory[desc_offset + 12], image_memory[desc_offset + 13],
            image_memory[desc_offset + 14], image_memory[desc_offset + 15],
        ]);
        let first_thunk = u32::from_le_bytes([
            image_memory[desc_offset + 16], image_memory[desc_offset + 17],
            image_memory[desc_offset + 18], image_memory[desc_offset + 19],
        ]);

        if name_rva == 0 && first_thunk == 0 {
            break; // Fim dos descriptors
        }

        // Lê nome da DLL
        let name_offset = rva_to_image_offset(name_rva, image_memory.len())?;
        let dll_name = read_null_terminated_ascii(&image_memory[name_offset..])?;

        // Tabela de thunks: usa ILT se presente, senão usa a própria IAT
        let thunk_table_rva = if orig_first_thunk != 0 { orig_first_thunk } else { first_thunk };
        let mut thunk_idx = 0usize;

        loop {
            let ilt_entry_rva = thunk_table_rva + (thunk_idx as u32 * 8);
            let iat_entry_rva = first_thunk + (thunk_idx as u32 * 8);

            let ilt_off = rva_to_image_offset(ilt_entry_rva, image_memory.len())?;
            let iat_off = rva_to_image_offset(iat_entry_rva, image_memory.len())?;

            let entry_val = u64::from_le_bytes([
                image_memory[ilt_off],     image_memory[ilt_off + 1],
                image_memory[ilt_off + 2], image_memory[ilt_off + 3],
                image_memory[ilt_off + 4], image_memory[ilt_off + 5],
                image_memory[ilt_off + 6], image_memory[ilt_off + 7],
            ]);

            if entry_val == 0 {
                break; // Fim dos thunks deste módulo
            }

            let resolved_addr = if (entry_val & IMAGE_ORDINAL_FLAG64) != 0 {
                // Import por Ordinal
                WIN_STUB_BASE_KERNEL32 + 0x200 + ((entry_val & 0xFFFF) * 8)
            } else {
                // Import por Nome (IMAGE_IMPORT_BY_NAME)
                let hint_name_rva = (entry_val & 0x7FFF_FFFF) as u32;
                let hint_off = rva_to_image_offset(hint_name_rva, image_memory.len())?;
                let func_name = read_null_terminated_ascii(&image_memory[hint_off + 2..])?;

                resolve_synthetic_export(&dll_name, &func_name).unwrap_or_else(|| {
                    // Fallback para stub dinâmico universal
                    WIN_STUB_BASE_KERNEL32 + 0x400 + ((thunk_idx as u64) * 0x10)
                })
            };

            // Escreve o ponteiro resolvido na IAT
            image_memory[iat_off..iat_off + 8].copy_from_slice(&resolved_addr.to_le_bytes());
            bound_count += 1;
            thunk_idx += 1;
        }

        desc_rva += 20;
    }

    Ok(bound_count)
}

fn rva_to_image_offset(rva: u32, len: usize) -> Result<usize, NtStatus> {
    let off = rva as usize;
    if off >= len {
        Err(NtStatus::ACCESS_VIOLATION)
    } else {
        Ok(off)
    }
}

fn read_null_terminated_ascii(slice: &[u8]) -> Result<String, NtStatus> {
    let mut s = String::new();
    for &b in slice {
        if b == 0 {
            break;
        }
        s.push(b as char);
    }
    Ok(s)
}
