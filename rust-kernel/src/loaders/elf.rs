//! Safe Bare-Metal ELF64 (Linux 64-bit Executable) Parser and Physical Memory Loader.
//!
//! Loads Linux ELF binaries directly into hardware x86_64 address spaces:
//! - Validates ELF magic, machine (x86_64), and segment headers
//! - Allocates physical memory frames and maps virtual memory pages in process PML4
//! - Enforces page protection: Present, User Accessible, Writable, No-Execute
//! - Configures user stack with System V AMD64 ABI layout (argc, argv, envp, auxv)
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

pub const ELFMAG: [u8; 4] = [0x7F, b'E', b'L', b'F'];
pub const ELFCLASS64: u8 = 2;
pub const EM_X86_64: u16 = 62;
pub const PT_LOAD: u32 = 1;

/// Standard virtual memory address for user stack allocation.
pub const USER_STACK_TOP: u64 = 0x0000_7FFF_FFFF_0000;
pub const USER_STACK_PAGES: usize = 16; // 64 KiB stack

// System V AMD64 Auxiliary Vector types
pub const AT_NULL: u64   = 0;
pub const AT_PHDR: u64   = 3;
pub const AT_PHENT: u64  = 4;
pub const AT_PHNUM: u64  = 5;
pub const AT_PAGESZ: u64 = 6;
pub const AT_ENTRY: u64  = 9;

/// Legacy lightweight parser updating a `HybridProcess` descriptor.
pub fn load_elf(proc: &mut HybridProcess, buffer: &[u8]) -> Result<(), NtStatus> {
    if buffer.len() < 64 || &buffer[0..4] != ELFMAG || buffer[4] != ELFCLASS64 {
        return Err(NtStatus::INVALID_PARAMETER);
    }
    let machine = u16::from_le_bytes([buffer[18], buffer[19]]);
    if machine != EM_X86_64 {
        return Err(NtStatus::NOT_IMPLEMENTED);
    }

    let entry_point = u64::from_le_bytes([
        buffer[24], buffer[25], buffer[26], buffer[27],
        buffer[28], buffer[29], buffer[30], buffer[31]
    ]);
    proc.entry_point = entry_point;
    Ok(())
}

/// Spawns an executable ELF64 binary into a real hardware address space.
pub fn spawn_elf_process(
    name: &str,
    elf_data: &[u8],
) -> Result<(Arc<Mutex<ProcessControlBlock>>, Arc<Mutex<ThreadControlBlock>>), &'static str> {
    if elf_data.len() < 64 {
        return Err("ELF data too short");
    }
    if &elf_data[0..4] != ELFMAG {
        return Err("Invalid ELF magic");
    }
    if elf_data[4] != ELFCLASS64 {
        return Err("Not a 64-bit ELF binary");
    }
    let machine = u16::from_le_bytes([elf_data[18], elf_data[19]]);
    if machine != EM_X86_64 {
        return Err("Unsupported architecture (expected x86_64)");
    }

    let entry_point = u64::from_le_bytes([
        elf_data[24], elf_data[25], elf_data[26], elf_data[27],
        elf_data[28], elf_data[29], elf_data[30], elf_data[31],
    ]);

    let phoff = u64::from_le_bytes([
        elf_data[32], elf_data[33], elf_data[34], elf_data[35],
        elf_data[36], elf_data[37], elf_data[38], elf_data[39],
    ]) as usize;

    let phentsize = u16::from_le_bytes([elf_data[54], elf_data[55]]) as usize;
    let phnum = u16::from_le_bytes([elf_data[56], elf_data[57]]) as usize;

    // 1. Allocate isolated PML4 address space with cloned kernel higher half
    let (mut page_mgr, pml4_frame) = mm::create_user_page_table()
        .map_err(|_| "Failed to allocate user PML4 page table")?;
    let hhdm_offset = crate::boot::get_hhdm_offset();

    let mut min_vaddr = u64::MAX;
    let mut total_memsz = 0u64;

    // 2. Map all PT_LOAD segments into the process address space
    {
        let mut alloc_guard = mm::GLOBAL_FRAME_ALLOCATOR.lock();
        let alloc = alloc_guard.as_mut().ok_or("Physical frame allocator offline")?;

        for i in 0..phnum {
            let offset = phoff + (i * phentsize);
            if offset + 56 > elf_data.len() {
                continue;
            }

            let p_type = u32::from_le_bytes([
                elf_data[offset], elf_data[offset + 1], elf_data[offset + 2], elf_data[offset + 3],
            ]);
            if p_type != PT_LOAD {
                continue;
            }

            let p_flags = u32::from_le_bytes([
                elf_data[offset + 4], elf_data[offset + 5], elf_data[offset + 6], elf_data[offset + 7],
            ]);
            let p_offset = u64::from_le_bytes([
                elf_data[offset + 8], elf_data[offset + 9], elf_data[offset + 10], elf_data[offset + 11],
                elf_data[offset + 12], elf_data[offset + 13], elf_data[offset + 14], elf_data[offset + 15],
            ]) as usize;
            let p_vaddr = u64::from_le_bytes([
                elf_data[offset + 16], elf_data[offset + 17], elf_data[offset + 18], elf_data[offset + 19],
                elf_data[offset + 20], elf_data[offset + 21], elf_data[offset + 22], elf_data[offset + 23],
            ]);
            let p_filesz = u64::from_le_bytes([
                elf_data[offset + 32], elf_data[offset + 33], elf_data[offset + 34], elf_data[offset + 35],
                elf_data[offset + 36], elf_data[offset + 37], elf_data[offset + 38], elf_data[offset + 39],
            ]) as usize;
            let p_memsz = u64::from_le_bytes([
                elf_data[offset + 40], elf_data[offset + 41], elf_data[offset + 42], elf_data[offset + 43],
                elf_data[offset + 44], elf_data[offset + 45], elf_data[offset + 46], elf_data[offset + 47],
            ]) as usize;

            if p_vaddr < min_vaddr {
                min_vaddr = p_vaddr;
            }
            total_memsz += p_memsz as u64;

            // Page alignment
            let start_vaddr = p_vaddr & !0xFFF;
            let end_vaddr = (p_vaddr + p_memsz as u64 + 0xFFF) & !0xFFF;

            let mut flags = PageTableFlags::PRESENT | PageTableFlags::USER;
            if p_flags & 2 != 0 {
                flags |= PageTableFlags::WRITABLE;
            }
            if p_flags & 1 == 0 {
                flags |= PageTableFlags::NO_EXECUTE;
            }

            let mut curr_vaddr = start_vaddr;
            while curr_vaddr < end_vaddr {
                let frame = alloc.allocate_frame().ok_or("OOM allocating segment frame")?;
                let frame_hhdm = (frame.start_address().as_u64() + hhdm_offset) as *mut u8;

                // Zero out the page
                unsafe {
                    core::ptr::write_bytes(frame_hhdm, 0, 4096);
                }

                // Copy binary slice if overlapping with file payload
                let page_start = curr_vaddr;
                let page_end = curr_vaddr + 4096;

                if page_start < p_vaddr + p_filesz as u64 && page_end > p_vaddr {
                    let file_copy_start = if page_start > p_vaddr { (page_start - p_vaddr) as usize } else { 0 };
                    let file_copy_end = (page_end - p_vaddr).min(p_filesz as u64) as usize;
                    let file_src_offset = p_offset + file_copy_start;
                    let dest_offset = if page_start < p_vaddr { (p_vaddr - page_start) as usize } else { 0 };
                    let copy_len = file_copy_end.saturating_sub(file_copy_start);

                    if file_src_offset + copy_len <= elf_data.len() && copy_len > 0 {
                        unsafe {
                            core::ptr::copy_nonoverlapping(
                                elf_data.as_ptr().add(file_src_offset),
                                frame_hhdm.add(dest_offset),
                                copy_len,
                            );
                        }
                    }
                }

                page_mgr.map_page(VirtAddr::new_truncate(curr_vaddr), frame, flags, alloc)
                    .map_err(|_| "Failed to map segment page in user PML4")?;

                curr_vaddr += 4096;
            }
        }

        // 3. Allocate and map user-space stack (64 KiB)
        let stack_base = USER_STACK_TOP - (USER_STACK_PAGES as u64 * 4096);
        let mut top_frame_hhdm = 0usize;

        for p in 0..USER_STACK_PAGES {
            let vaddr = stack_base + (p as u64 * 4096);
            let frame = alloc.allocate_frame().ok_or("OOM allocating user stack frame")?;
            let frame_hhdm = (frame.start_address().as_u64() + hhdm_offset) as *mut u8;

            unsafe {
                core::ptr::write_bytes(frame_hhdm, 0, 4096);
            }

            if p == USER_STACK_PAGES - 1 {
                top_frame_hhdm = frame_hhdm as usize;
            }

            let stack_flags = PageTableFlags::PRESENT
                | PageTableFlags::WRITABLE
                | PageTableFlags::USER
                | PageTableFlags::NO_EXECUTE;

            page_mgr.map_page(VirtAddr::new_truncate(vaddr), frame, stack_flags, alloc)
                .map_err(|_| "Failed to map user stack page")?;
        }

        // 4. Populate System V AMD64 initial user stack:
        // [argc=0] [argv_null=0] [envp_null=0] [auxv: AT_ENTRY, AT_PAGESZ, AT_NULL]
        if top_frame_hhdm != 0 {
            let stack_slice = unsafe {
                let ptr = (top_frame_hhdm + 4096 - 128) as *mut u64;
                // argc
                *ptr.add(0) = 0;
                // argv[0] = NULL
                *ptr.add(1) = 0;
                // envp[0] = NULL
                *ptr.add(2) = 0;
                // auxv AT_ENTRY
                *ptr.add(3) = AT_ENTRY;
                *ptr.add(4) = entry_point;
                // auxv AT_PAGESZ
                *ptr.add(5) = AT_PAGESZ;
                *ptr.add(6) = 4096;
                // auxv AT_NULL
                *ptr.add(7) = AT_NULL;
                *ptr.add(8) = 0;
            };
            let _ = stack_slice;
        }
    }

    let user_rsp = USER_STACK_TOP - 128;
    let cr3 = pml4_frame.start_address().as_u64();

    // 5. Register Process and Thread in Scheduler
    let pcb_arc = sched::create_process(name, ExecutionPersonality::LinuxPosix, PriorityClass::Normal);
    let pid = {
        let mut pcb = pcb_arc.lock().unwrap();
        pcb.page_table_base = cr3;
        pcb.entry_point = entry_point;
        pcb.image_base = min_vaddr;
        pcb.image_size = total_memsz as usize;
        pcb.pid
    };

    let tcb_arc = sched::create_thread(pid, "main", entry_point, user_rsp, Priority::NORMAL)?;
    {
        let mut tcb = tcb_arc.lock().unwrap();
        tcb.context.cr3 = cr3;
        tcb.context.cs = 0x23; // USER_CS
        tcb.context.ss = 0x1B; // USER_DS
        tcb.context.rflags = 0x202; // IF = 1
        tcb.user_stack_top = user_rsp;
        tcb.user_stack_size = USER_STACK_PAGES * 4096;
    }

    crate::serial_println!(
        "[ELF LOADER] Successfully spawned Linux process '{}' (PID {}): Entry=0x{:016X}, CR3=0x{:016X}, RSP=0x{:016X}",
        name, pid, entry_point, cr3, user_rsp
    );

    Ok((pcb_arc, tcb_arc))
}
