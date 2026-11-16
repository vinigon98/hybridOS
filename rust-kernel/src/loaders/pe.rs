//! Safe Bare-Metal PE32+ (Windows 64-bit Executable) Parser and Physical Memory Loader.
//!
//! Loads Windows PE binaries directly into hardware x86_64 address spaces:
//! - Validates DOS magic ("MZ") and PE signature ("PE\0\0")
//! - Allocates physical frames and maps PE sections (.text, .data, .rdata) in process PML4
//! - Maps KUSER_SHARED_DATA page at fixed address 0x7FFE0000 (read-only for user mode)
//! - Maps TEB (Thread Environment Block) and PEB (Process Environment Block)
//! - Configures user GS_BASE to point to TEB (GS:[0x30] = TEB, GS:[0x60] = PEB)
//! - Registers the process and thread under the HybridOS scheduler

use alloc::string::{String, ToString};
use alloc::sync::Arc;
use std::sync::Mutex;

use crate::mm::{
    self, PhysAddr, VirtAddr, PageTableFlags,
};
use crate::personality::ExecutionPersonality;
use crate::process::HybridProcess;
use crate::sched::{
    self, Priority, PriorityClass, ProcessControlBlock, ThreadControlBlock,
};
use crate::types::NtStatus;

pub const IMAGE_DOS_SIGNATURE: u16 = 0x5A4D; // "MZ"
pub const IMAGE_NT_SIGNATURE: u32 = 0x00004550; // "PE\0\0"
pub const IMAGE_FILE_MACHINE_AMD64: u16 = 0x8664; // x86_64
pub const OPTIONAL_HEADER_MAGIC_PE32_PLUS: u16 = 0x20B; // 64-bit PE

/// Fixed Windows NT user-space addresses
pub const KUSER_SHARED_DATA_ADDR: u64 = 0x7FFE_0000;
pub const USER_TEB_ADDR: u64           = 0x7FFE_1000;
pub const USER_PEB_ADDR: u64           = 0x7FFE_2000;
pub const WIN_SYSCALL_THUNKS_ADDR: u64 = 0x7FFE_3000;
pub const WIN_USER_STACK_TOP: u64      = 0x0000_7FFF_E000_0000;
pub const WIN_USER_STACK_PAGES: usize  = 16; // 64 KiB stack

// Section header characteristics flags
pub const IMAGE_SCN_MEM_EXECUTE: u32 = 0x2000_0000;
pub const IMAGE_SCN_MEM_READ: u32    = 0x4000_0000;
pub const IMAGE_SCN_MEM_WRITE: u32   = 0x8000_0000;

/// Legacy lightweight parser updating a `HybridProcess` descriptor.
pub fn load_pe(_proc: &mut HybridProcess, buffer: &[u8]) -> Result<(), NtStatus> {
    if buffer.len() < 64 {
        return Err(NtStatus::INVALID_PARAMETER);
    }
    let dos_magic = u16::from_le_bytes([buffer[0], buffer[1]]);
    if dos_magic != IMAGE_DOS_SIGNATURE {
        return Err(NtStatus::INVALID_PARAMETER);
    }
    let e_lfanew = u32::from_le_bytes([buffer[60], buffer[61], buffer[62], buffer[63]]) as usize;
    if e_lfanew + 264 > buffer.len() {
        return Err(NtStatus::INVALID_PARAMETER);
    }
    let pe_sig = u32::from_le_bytes([
        buffer[e_lfanew], buffer[e_lfanew + 1], buffer[e_lfanew + 2], buffer[e_lfanew + 3]
    ]);
    if pe_sig != IMAGE_NT_SIGNATURE {
        return Err(NtStatus::INVALID_PARAMETER);
    }
    Ok(())
}

/// Spawns an executable PE64 (Windows NT) binary into a real hardware address space.
pub fn spawn_pe_process(
    name: &str,
    pe_data: &[u8],
) -> Result<(Arc<Mutex<ProcessControlBlock>>, Arc<Mutex<ThreadControlBlock>>), &'static str> {
    if pe_data.len() < 64 {
        return Err("PE image too short");
    }

    let dos_magic = u16::from_le_bytes([pe_data[0], pe_data[1]]);
    if dos_magic != IMAGE_DOS_SIGNATURE {
        return Err("Invalid DOS signature");
    }

    let e_lfanew = u32::from_le_bytes([pe_data[60], pe_data[61], pe_data[62], pe_data[63]]) as usize;
    if e_lfanew + 264 > pe_data.len() {
        return Err("Invalid PE header offset");
    }

    let pe_sig = u32::from_le_bytes([
        pe_data[e_lfanew], pe_data[e_lfanew + 1], pe_data[e_lfanew + 2], pe_data[e_lfanew + 3]
    ]);
    if pe_sig != IMAGE_NT_SIGNATURE {
        return Err("Invalid PE signature");
    }

    let file_header_offset = e_lfanew + 4;
    let machine = u16::from_le_bytes([pe_data[file_header_offset], pe_data[file_header_offset + 1]]);
    if machine != IMAGE_FILE_MACHINE_AMD64 {
        return Err("Unsupported machine type (expected x86_64)");
    }

    let num_sections = u16::from_le_bytes([pe_data[file_header_offset + 2], pe_data[file_header_offset + 3]]) as usize;
    let opt_header_size = u16::from_le_bytes([pe_data[file_header_offset + 16], pe_data[file_header_offset + 17]]) as usize;

    let opt_header_offset = file_header_offset + 20;
    let opt_magic = u16::from_le_bytes([pe_data[opt_header_offset], pe_data[opt_header_offset + 1]]);
    if opt_magic != OPTIONAL_HEADER_MAGIC_PE32_PLUS {
        return Err("Non-64-bit PE binary (expected PE32+)");
    }

    let entry_point_rva = u32::from_le_bytes([
        pe_data[opt_header_offset + 16], pe_data[opt_header_offset + 17],
        pe_data[opt_header_offset + 18], pe_data[opt_header_offset + 19],
    ]) as u64;

    let image_base = u64::from_le_bytes([
        pe_data[opt_header_offset + 24], pe_data[opt_header_offset + 25],
        pe_data[opt_header_offset + 26], pe_data[opt_header_offset + 27],
        pe_data[opt_header_offset + 28], pe_data[opt_header_offset + 29],
        pe_data[opt_header_offset + 30], pe_data[opt_header_offset + 31],
    ]);

    let size_of_image = u32::from_le_bytes([
        pe_data[opt_header_offset + 56], pe_data[opt_header_offset + 57],
        pe_data[opt_header_offset + 58], pe_data[opt_header_offset + 59],
    ]) as usize;

    let size_of_headers = u32::from_le_bytes([
        pe_data[opt_header_offset + 60], pe_data[opt_header_offset + 61],
        pe_data[opt_header_offset + 62], pe_data[opt_header_offset + 63],
    ]) as usize;

    // 1. Allocate isolated PML4 address space with cloned kernel higher half
    let (mut page_mgr, pml4_frame) = mm::create_user_page_table()
        .map_err(|_| "Failed to allocate user PML4 page table")?;
    let hhdm_offset = crate::boot::get_hhdm_offset();

    let entry_point = image_base + entry_point_rva;

    // 2. Map PE Headers and Sections
    {
        let mut alloc_guard = mm::GLOBAL_FRAME_ALLOCATOR.lock();
        let alloc = alloc_guard.as_mut().ok_or("Physical frame allocator offline")?;

        // 2a. Map PE Header Page(s) at ImageBase
        let header_pages = (size_of_headers + 4095) / 4096;
        for p in 0..header_pages {
            let vaddr = image_base + (p as u64 * 4096);
            let frame = alloc.allocate_frame().ok_or("OOM allocating PE header frame")?;
            let frame_hhdm = (frame.start_address().as_u64() + hhdm_offset) as *mut u8;

            unsafe {
                core::ptr::write_bytes(frame_hhdm, 0, 4096);
                let copy_len = (size_of_headers.saturating_sub(p * 4096)).min(4096);
                if copy_len > 0 && (p * 4096) < pe_data.len() {
                    let actual_copy = copy_len.min(pe_data.len() - (p * 4096));
                    core::ptr::copy_nonoverlapping(pe_data.as_ptr().add(p * 4096), frame_hhdm, actual_copy);
                }
            }

            let flags = PageTableFlags::PRESENT | PageTableFlags::USER | PageTableFlags::WRITABLE;
            page_mgr.map_page(VirtAddr::new_truncate(vaddr), frame, flags, alloc)
                .map_err(|_| "Failed to map PE header page")?;
        }

        // 2b. Map PE Sections
        let section_table_offset = opt_header_offset + opt_header_size;
        for s in 0..num_sections {
            let sec_offset = section_table_offset + (s * 40);
            if sec_offset + 40 > pe_data.len() {
                break;
            }

            let virt_size = u32::from_le_bytes([
                pe_data[sec_offset + 8], pe_data[sec_offset + 9],
                pe_data[sec_offset + 10], pe_data[sec_offset + 11],
            ]) as usize;

            let virt_addr = u32::from_le_bytes([
                pe_data[sec_offset + 12], pe_data[sec_offset + 13],
                pe_data[sec_offset + 14], pe_data[sec_offset + 15],
            ]) as u64;

            let raw_data_size = u32::from_le_bytes([
                pe_data[sec_offset + 16], pe_data[sec_offset + 17],
                pe_data[sec_offset + 18], pe_data[sec_offset + 19],
            ]) as usize;

            let raw_data_ptr = u32::from_le_bytes([
                pe_data[sec_offset + 20], pe_data[sec_offset + 21],
                pe_data[sec_offset + 22], pe_data[sec_offset + 23],
            ]) as usize;

            let characteristics = u32::from_le_bytes([
                pe_data[sec_offset + 36], pe_data[sec_offset + 37],
                pe_data[sec_offset + 38], pe_data[sec_offset + 39],
            ]);

            let mut flags = PageTableFlags::PRESENT | PageTableFlags::USER;
            if characteristics & IMAGE_SCN_MEM_WRITE != 0 {
                flags |= PageTableFlags::WRITABLE;
            }
            if characteristics & IMAGE_SCN_MEM_EXECUTE == 0 {
                flags |= PageTableFlags::NO_EXECUTE;
            }

            let sec_vaddr = image_base + virt_addr;
            let sec_pages = (virt_size.max(raw_data_size) + 4095) / 4096;

            for p in 0..sec_pages {
                let page_vaddr = sec_vaddr + (p as u64 * 4096);
                let frame = alloc.allocate_frame().ok_or("OOM allocating section frame")?;
                let frame_hhdm = (frame.start_address().as_u64() + hhdm_offset) as *mut u8;

                unsafe {
                    core::ptr::write_bytes(frame_hhdm, 0, 4096);
                    let page_file_start = p * 4096;
                    if page_file_start < raw_data_size && raw_data_ptr + page_file_start < pe_data.len() {
                        let bytes_available = raw_data_size - page_file_start;
                        let copy_len = bytes_available.min(4096).min(pe_data.len() - (raw_data_ptr + page_file_start));
                        core::ptr::copy_nonoverlapping(
                            pe_data.as_ptr().add(raw_data_ptr + page_file_start),
                            frame_hhdm,
                            copy_len,
                        );
                    }
                }

                page_mgr.map_page(VirtAddr::new_truncate(page_vaddr), frame, flags, alloc)
                    .map_err(|_| "Failed to map section page")?;
            }
        }

        // 3. Map KUSER_SHARED_DATA at 0x7FFE0000 (read-only for user mode)
        {
            let kuser_frame = alloc.allocate_frame().ok_or("OOM allocating KUSER_SHARED_DATA")?;
            let kuser_hhdm = (kuser_frame.start_address().as_u64() + hhdm_offset) as *mut u8;
            unsafe {
                core::ptr::write_bytes(kuser_hhdm, 0, 4096);
                // NtMajorVersion = 10, NtMinorVersion = 0, SystemTime fields
                let ptr32 = kuser_hhdm as *mut u32;
                *ptr32.add(0x026C / 4) = 10; // NtMajorVersion
                *ptr32.add(0x0270 / 4) = 0;  // NtMinorVersion
            }
            let kuser_flags = PageTableFlags::PRESENT | PageTableFlags::USER | PageTableFlags::NO_EXECUTE;
            page_mgr.map_page(VirtAddr::new_truncate(KUSER_SHARED_DATA_ADDR), kuser_frame, kuser_flags, alloc)
                .map_err(|_| "Failed to map KUSER_SHARED_DATA")?;
        }

        // 4. Map TEB (0x7FFE1000) and PEB (0x7FFE2000)
        {
            let teb_frame = alloc.allocate_frame().ok_or("OOM allocating TEB frame")?;
            let peb_frame = alloc.allocate_frame().ok_or("OOM allocating PEB frame")?;

            let teb_hhdm = (teb_frame.start_address().as_u64() + hhdm_offset) as *mut u8;
            let peb_hhdm = (peb_frame.start_address().as_u64() + hhdm_offset) as *mut u8;

            unsafe {
                core::ptr::write_bytes(teb_hhdm, 0, 4096);
                core::ptr::write_bytes(peb_hhdm, 0, 4096);

                // TEB initialization:
                // GS:[0x30] = LinearAddress (self pointer)
                // GS:[0x40] = ClientId.UniqueProcess (PID = 2)
                // GS:[0x48] = ClientId.UniqueThread (TID = 2)
                // GS:[0x60] = ProcessEnvironmentBlock (pointer to PEB)
                let teb64 = teb_hhdm as *mut u64;
                *teb64.add(0x30 / 8) = USER_TEB_ADDR;
                *teb64.add(0x40 / 8) = 2; // PID 2
                *teb64.add(0x48 / 8) = 2; // TID 2
                *teb64.add(0x60 / 8) = USER_PEB_ADDR;

                // PEB initialization:
                // PEB[0x10] = ImageBaseAddress
                let peb64 = peb_hhdm as *mut u64;
                *peb64.add(0x10 / 8) = image_base;
            }

            let uflags = PageTableFlags::PRESENT
                | PageTableFlags::WRITABLE
                | PageTableFlags::USER
                | PageTableFlags::NO_EXECUTE;

            page_mgr.map_page(VirtAddr::new_truncate(USER_TEB_ADDR), teb_frame, uflags, alloc)
                .map_err(|_| "Failed to map TEB page")?;
            page_mgr.map_page(VirtAddr::new_truncate(USER_PEB_ADDR), peb_frame, uflags, alloc)
                .map_err(|_| "Failed to map PEB page")?;
        }

        // 4b. Map Native NT Syscall Thunks at 0x7FFE3000 (USER | PRESENT | EXECUTABLE)
        {
            let thunk_frame = alloc.allocate_frame().ok_or("OOM allocating NT syscall thunks frame")?;
            let thunk_hhdm = (thunk_frame.start_address().as_u64() + hhdm_offset) as *mut u8;

            unsafe {
                core::ptr::write_bytes(thunk_hhdm, 0xCC, 4096); // Trap by default

                // Helper to emit x86_64 NT syscall thunk:
                // mov r10, rcx (49 89 CA); mov eax, imm32 (B8 ..); syscall (0F 05); ret (C3)
                let make_thunk = |dest: *mut u8, syscall_id: u32| {
                    *dest.add(0) = 0x49;
                    *dest.add(1) = 0x89;
                    *dest.add(2) = 0xCA;
                    *dest.add(3) = 0xB8;
                    let b = syscall_id.to_le_bytes();
                    *dest.add(4) = b[0];
                    *dest.add(5) = b[1];
                    *dest.add(6) = b[2];
                    *dest.add(7) = b[3];
                    *dest.add(8) = 0x0F;
                    *dest.add(9) = 0x05;
                    *dest.add(10) = 0xC3;
                };

                make_thunk(thunk_hhdm.add(0x00), 0x0008); // Slot 0: NtWriteFile
                make_thunk(thunk_hhdm.add(0x10), 0x0006); // Slot 1: NtReadFile
                make_thunk(thunk_hhdm.add(0x20), 0x0055); // Slot 2: NtCreateFile
                make_thunk(thunk_hhdm.add(0x30), 0x0018); // Slot 3: NtAllocateVirtualMemory
                make_thunk(thunk_hhdm.add(0x40), 0x000F); // Slot 4: NtClose
                make_thunk(thunk_hhdm.add(0x50), 0x002C); // Slot 5: NtTerminateProcess

                // kernel32.dll thunks at offset 0x800 (0x7FFE3800):
                make_thunk(thunk_hhdm.add(0x800), 0x002C); // ExitProcess -> NtTerminateProcess (0x002C)
                make_thunk(thunk_hhdm.add(0x810), 0x0008); // WriteFile -> NtWriteFile (0x0008)
                make_thunk(thunk_hhdm.add(0x820), 0x0006); // ReadFile -> NtReadFile (0x0006)
                make_thunk(thunk_hhdm.add(0x830), 0x0055); // CreateFileA -> NtCreateFile (0x0055)
                make_thunk(thunk_hhdm.add(0x840), 0x0055); // CreateFileW -> NtCreateFile (0x0055)
                make_thunk(thunk_hhdm.add(0x850), 0x000F); // CloseHandle -> NtClose (0x000F)
                make_thunk(thunk_hhdm.add(0x860), 0x0018); // VirtualAlloc -> NtAllocateVirtualMemory (0x0018)
            }

            let tflags = PageTableFlags::PRESENT | PageTableFlags::USER; // Executable in Ring 3
            page_mgr.map_page(VirtAddr::new_truncate(WIN_SYSCALL_THUNKS_ADDR), thunk_frame, tflags, alloc)
                .map_err(|_| "Failed to map NT syscall thunk page")?;
        }

        // 5. Map User Stack (64 KiB)
        let stack_base = WIN_USER_STACK_TOP - (WIN_USER_STACK_PAGES as u64 * 4096);
        for p in 0..WIN_USER_STACK_PAGES {
            let vaddr = stack_base + (p as u64 * 4096);
            let frame = alloc.allocate_frame().ok_or("OOM allocating user stack frame")?;
            let frame_hhdm = (frame.start_address().as_u64() + hhdm_offset) as *mut u8;
            unsafe {
                core::ptr::write_bytes(frame_hhdm, 0, 4096);
            }
            let sflags = PageTableFlags::PRESENT
                | PageTableFlags::WRITABLE
                | PageTableFlags::USER
                | PageTableFlags::NO_EXECUTE;

            page_mgr.map_page(VirtAddr::new_truncate(vaddr), frame, sflags, alloc)
                .map_err(|_| "Failed to map user stack page")?;
        }
    }

    // Windows x64 calling convention: 32-byte shadow space + 8-byte alignment
    let user_rsp = WIN_USER_STACK_TOP - 40;
    let cr3 = pml4_frame.start_address().as_u64();

    // 6. Register Process and Thread in Scheduler
    let pcb_arc = sched::create_process(name, ExecutionPersonality::WindowsNt, PriorityClass::Normal);
    let pid = {
        let mut pcb = pcb_arc.lock().unwrap();
        pcb.page_table_base = cr3;
        pcb.entry_point = entry_point;
        pcb.image_base = image_base;
        pcb.image_size = size_of_image;
        pcb.pid
    };

    let tcb_arc = sched::create_thread(pid, "main", entry_point, user_rsp, Priority::NORMAL)?;
    {
        let mut tcb = tcb_arc.lock().unwrap();
        tcb.context.cr3 = cr3;
        tcb.context.cs = 0x23; // USER_CS
        tcb.context.ss = 0x1B; // USER_DS
        tcb.context.rflags = 0x202; // IF = 1
        tcb.context.gs_base = USER_TEB_ADDR; // Windows TEB base loaded into GS
        tcb.user_stack_top = user_rsp;
        tcb.user_stack_size = WIN_USER_STACK_PAGES * 4096;
    }

    crate::serial_println!(
        "[PE LOADER] Successfully spawned Windows NT process '{}' (PID {}): Entry=0x{:016X}, CR3=0x{:016X}, TEB=0x{:016X}",
        name, pid, entry_point, cr3, USER_TEB_ADDR
    );

    Ok((pcb_arc, tcb_arc))
}
