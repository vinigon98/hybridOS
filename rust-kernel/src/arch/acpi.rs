//! ACPI Subsystem & Safe MADT/FADT/DSDT Table Parser for HybridOS.
//!
//! Compliant with ACPI 1.0 - 6.4 specifications in bare metal x86_64 Long Mode.
//! Handles RSDP discovery (UEFI Limine + BIOS/EBDA scan), XSDT/RSDT table routing,
//! MADT core/LAPIC enumeration, and FADT/DSDT PM1a_CNT power-off dispatch.

use alloc::vec::Vec;
use core::ptr::read_unaligned;
use crate::arch::msr;

// --- ACPI Table Descriptors ---

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct RsdpDescriptor {
    pub signature: [u8; 8], // "RSD PTR "
    pub checksum: u8,
    pub oem_id: [u8; 6],
    pub revision: u8, // 0 = ACPI 1.0, 2 = ACPI 2.0+
    pub rsdt_address: u32,
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct XsdpDescriptor {
    pub rsdp: RsdpDescriptor,
    pub length: u32,
    pub xsdt_address: u64,
    pub extended_checksum: u8,
    pub reserved: [u8; 3],
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct SdtHeader {
    pub signature: [u8; 4],
    pub length: u32,
    pub revision: u8,
    pub checksum: u8,
    pub oem_id: [u8; 6],
    pub oem_table_id: [u8; 8],
    pub oem_revision: u32,
    pub creator_id: [u8; 4],
    pub creator_revision: u32,
}

/// Validates ACPI table checksum over `length` bytes (sum % 256 == 0).
pub unsafe fn validate_checksum(ptr: *const u8, length: usize) -> bool {
    if ptr.is_null() || length < core::mem::size_of::<SdtHeader>() {
        return false;
    }
    let slice = core::slice::from_raw_parts(ptr, length);
    let mut sum: u8 = 0;
    for &b in slice {
        sum = sum.wrapping_add(b);
    }
    sum == 0
}

// --- MADT Interrupt Controller Record Structures ---

pub const MADT_TYPE_LOCAL_APIC: u8 = 0;
pub const MADT_TYPE_IO_APIC: u8 = 1;
pub const MADT_TYPE_INTERRUPT_OVERRIDE: u8 = 2;
pub const MADT_TYPE_NMI: u8 = 4;
pub const MADT_TYPE_LAPIC_ADDR_OVERRIDE: u8 = 5;
pub const MADT_TYPE_LOCAL_X2APIC: u8 = 9;

#[derive(Debug, Clone, Copy)]
pub struct CpuCoreInfo {
    pub processor_id: u32,
    pub lapic_id: u32,
    pub is_enabled: bool,
    pub is_x2apic: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct IoApicInfo {
    pub id: u8,
    pub address: u32,
    pub gsi_base: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct InterruptOverride {
    pub bus: u8,
    pub source_irq: u8,
    pub gsi: u32,
    pub flags: u16,
}

#[derive(Debug, Clone)]
pub struct MadtInfo {
    pub lapic_address: u64,
    pub flags: u32,
    pub cores: Vec<CpuCoreInfo>,
    pub io_apics: Vec<IoApicInfo>,
    pub overrides: Vec<InterruptOverride>,
}

#[derive(Debug, Clone, Copy)]
pub struct AcpiShutdownInfo {
    pub pm1a_cnt_blk: u16,
    pub pm1b_cnt_blk: u16,
    pub slp_typa: u8,
    pub slp_typb: u8,
    pub smi_cmd: u32,
    pub acpi_enable: u8,
}

pub static mut ACPI_SHUTDOWN_CONFIG: Option<AcpiShutdownInfo> = None;

// --- Root ACPI Discovery & Parser ---

pub struct AcpiContext {
    pub hhdm_offset: u64,
    pub rsdp_phys: u64,
    pub xsdt_phys: Option<u64>,
    pub rsdt_phys: Option<u32>,
    pub madt: Option<MadtInfo>,
}

impl AcpiContext {
    /// Discovers and validates ACPI tables using Limine response or BIOS scan.
    pub fn init(hhdm_offset: u64) -> Result<Self, &'static str> {
        let rsdp_addr = Self::find_rsdp(hhdm_offset)
            .ok_or("ACPI: Failed to locate RSDP in UEFI or BIOS memory")?;

        let mut ctx = Self {
            hhdm_offset,
            rsdp_phys: rsdp_addr,
            xsdt_phys: None,
            rsdt_phys: None,
            madt: None,
        };

        unsafe {
            ctx.parse_rsdp()?;
            ctx.parse_madt()?;
            ctx.parse_fadt_and_dsdt();
        }

        Ok(ctx)
    }

    /// Finds RSDP via Limine or legacy BIOS / EBDA fallback scan.
    fn find_rsdp(hhdm_offset: u64) -> Option<u64> {
        // 1. Limine RSDP protocol query
        if let Some(addr) = crate::boot::limine::get_rsdp_address() {
            let phys = if addr >= hhdm_offset { addr - hhdm_offset } else { addr };
            let virt = hhdm_offset + phys;
            unsafe {
                let slice = core::slice::from_raw_parts(virt as *const u8, 8);
                if slice == b"RSD PTR " {
                    crate::serial_println!("[ACPI] RSDP located via Limine at physical 0x{:016X}", phys);
                    return Some(phys);
                }
            }
        }

        // 2. Scan EBDA (Extended BIOS Data Area)
        unsafe {
            let bda_ebda_ptr = (hhdm_offset + 0x040E) as *const u16;
            let ebda_segment = read_unaligned(bda_ebda_ptr) as u64;
            if ebda_segment != 0 {
                let ebda_phys = ebda_segment << 4;
                if let Some(found) = Self::scan_memory_for_rsdp(hhdm_offset, ebda_phys, 1024) {
                    crate::serial_println!("[ACPI] RSDP located in EBDA at physical 0x{:016X}", found);
                    return Some(found);
                }
            }

            // 3. Scan BIOS ROM area 0x000E0000 - 0x000FFFFF
            if let Some(found) = Self::scan_memory_for_rsdp(hhdm_offset, 0x000E0000, 0x20000) {
                crate::serial_println!("[ACPI] RSDP located in BIOS ROM at physical 0x{:016X}", found);
                return Some(found);
            }
        }

        None
    }

    unsafe fn scan_memory_for_rsdp(hhdm_offset: u64, base_phys: u64, length: usize) -> Option<u64> {
        let virt = hhdm_offset + base_phys;
        let mut offset = 0;
        while offset + 20 <= length {
            let candidate = (virt + offset as u64) as *const u8;
            let sig = core::slice::from_raw_parts(candidate, 8);
            if sig == b"RSD PTR " {
                // Verify ACPI 1.0 checksum
                let bytes = core::slice::from_raw_parts(candidate, 20);
                let sum = bytes.iter().fold(0u8, |acc, &b| acc.wrapping_add(b));
                if sum == 0 {
                    return Some(base_phys + offset as u64);
                }
            }
            offset += 16; // RSDP is 16-byte aligned per ACPI spec
        }
        None
    }

    unsafe fn parse_rsdp(&mut self) -> Result<(), &'static str> {
        let virt = self.hhdm_offset + self.rsdp_phys;
        let rsdp = &*(virt as *const RsdpDescriptor);

        if rsdp.revision >= 2 {
            let xsdp = &*(virt as *const XsdpDescriptor);
            if xsdp.length >= core::mem::size_of::<XsdpDescriptor>() as u32 {
                if validate_checksum(virt as *const u8, xsdp.length as usize) {
                    let xsdt_addr = xsdp.xsdt_address;
                    self.xsdt_phys = Some(xsdt_addr);
                    crate::serial_println!("[ACPI] ACPI 2.0+ XSDP valid. XSDT physical: 0x{:016X}", xsdt_addr);
                    return Ok(());
                }
            }
        }

        // Fallback to ACPI 1.0 RSDT
        let rsdt_addr = rsdp.rsdt_address;
        self.rsdt_phys = Some(rsdt_addr);
        crate::serial_println!("[ACPI] ACPI 1.0 RSDP valid. RSDT physical: 0x{:08X}", rsdt_addr);
        Ok(())
    }

    /// Finds an ACPI System Description Table by 4-byte signature.
    pub unsafe fn find_table(&self, signature: &[u8; 4]) -> Option<*const SdtHeader> {
        if let Some(xsdt_phys) = self.xsdt_phys {
            let xsdt_virt = self.hhdm_offset + xsdt_phys;
            let header = &*(xsdt_virt as *const SdtHeader);
            if validate_checksum(xsdt_virt as *const u8, header.length as usize) {
                let entry_count = (header.length.saturating_sub(core::mem::size_of::<SdtHeader>() as u32)) / 8;
                let entries = (xsdt_virt + core::mem::size_of::<SdtHeader>() as u64) as *const u64;
                for i in 0..entry_count {
                    let table_phys = read_unaligned(entries.add(i as usize));
                    if table_phys != 0 {
                        let table_header = (self.hhdm_offset + table_phys) as *const SdtHeader;
                        if (*table_header).signature == *signature {
                            if validate_checksum(table_header as *const u8, (*table_header).length as usize) {
                                return Some(table_header);
                            }
                        }
                    }
                }
            }
        }

        if let Some(rsdt_phys) = self.rsdt_phys {
            let rsdt_virt = self.hhdm_offset + rsdt_phys as u64;
            let header = &*(rsdt_virt as *const SdtHeader);
            if validate_checksum(rsdt_virt as *const u8, header.length as usize) {
                let entry_count = (header.length.saturating_sub(core::mem::size_of::<SdtHeader>() as u32)) / 4;
                let entries = (rsdt_virt + core::mem::size_of::<SdtHeader>() as u64) as *const u32;
                for i in 0..entry_count {
                    let table_phys = read_unaligned(entries.add(i as usize)) as u64;
                    if table_phys != 0 {
                        let table_header = (self.hhdm_offset + table_phys) as *const SdtHeader;
                        if (*table_header).signature == *signature {
                            if validate_checksum(table_header as *const u8, (*table_header).length as usize) {
                                return Some(table_header);
                            }
                        }
                    }
                }
            }
        }

        None
    }

    /// Parses the MADT (Multiple APIC Description Table) to discover all CPU cores.
    unsafe fn parse_madt(&mut self) -> Result<(), &'static str> {
        let madt_ptr = self.find_table(b"APIC")
            .ok_or("ACPI: MADT ('APIC') table not found")?;

        let header = &*madt_ptr;
        let total_length = header.length as usize;
        let madt_base = madt_ptr as *const u8;

        let local_apic_addr = read_unaligned((madt_base.add(36)) as *const u32) as u64;
        let flags = read_unaligned((madt_base.add(40)) as *const u32);

        let mut madt_info = MadtInfo {
            lapic_address: local_apic_addr,
            flags,
            cores: Vec::new(),
            io_apics: Vec::new(),
            overrides: Vec::new(),
        };

        let mut offset = 44; // Start of interrupt controller records
        while offset + 2 <= total_length {
            let entry_type = *madt_base.add(offset);
            let entry_len = *madt_base.add(offset + 1) as usize;

            if entry_len == 0 || offset + entry_len > total_length {
                break;
            }

            match entry_type {
                MADT_TYPE_LOCAL_APIC => {
                    if entry_len >= 8 {
                        let proc_id = *madt_base.add(offset + 2) as u32;
                        let lapic_id = *madt_base.add(offset + 3) as u32;
                        let apic_flags = read_unaligned((madt_base.add(offset + 4)) as *const u32);
                        let is_enabled = (apic_flags & 1) != 0 || (apic_flags & 2) != 0;

                        madt_info.cores.push(CpuCoreInfo {
                            processor_id: proc_id,
                            lapic_id,
                            is_enabled,
                            is_x2apic: false,
                        });
                    }
                }
                MADT_TYPE_IO_APIC => {
                    if entry_len >= 12 {
                        let id = *madt_base.add(offset + 2);
                        let address = read_unaligned((madt_base.add(offset + 4)) as *const u32);
                        let gsi_base = read_unaligned((madt_base.add(offset + 8)) as *const u32);
                        madt_info.io_apics.push(IoApicInfo { id, address, gsi_base });
                    }
                }
                MADT_TYPE_INTERRUPT_OVERRIDE => {
                    if entry_len >= 10 {
                        let bus = *madt_base.add(offset + 2);
                        let source_irq = *madt_base.add(offset + 3);
                        let gsi = read_unaligned((madt_base.add(offset + 4)) as *const u32);
                        let flags = read_unaligned((madt_base.add(offset + 8)) as *const u16);
                        madt_info.overrides.push(InterruptOverride { bus, source_irq, gsi, flags });
                    }
                }
                MADT_TYPE_LAPIC_ADDR_OVERRIDE => {
                    if entry_len >= 12 {
                        let override_addr = read_unaligned((madt_base.add(offset + 4)) as *const u64);
                        madt_info.lapic_address = override_addr;
                    }
                }
                MADT_TYPE_LOCAL_X2APIC => {
                    if entry_len >= 16 {
                        let x2apic_id = read_unaligned((madt_base.add(offset + 4)) as *const u32);
                        let apic_flags = read_unaligned((madt_base.add(offset + 8)) as *const u32);
                        let uid = read_unaligned((madt_base.add(offset + 12)) as *const u32);
                        let is_enabled = (apic_flags & 1) != 0 || (apic_flags & 2) != 0;

                        madt_info.cores.push(CpuCoreInfo {
                            processor_id: uid,
                            lapic_id: x2apic_id,
                            is_enabled,
                            is_x2apic: true,
                        });
                    }
                }
                _ => {}
            }

            offset += entry_len;
        }

        crate::serial_println!("[ACPI] MADT parsed: LAPIC Base=0x{:016X}, Cores={}, I/O APICs={}",
            madt_info.lapic_address, madt_info.cores.len(), madt_info.io_apics.len());

        self.madt = Some(madt_info);
        Ok(())
    }

    /// Parses FADT and DSDT to locate PM1a_CNT and extracts `_S5_` sleep state.
    unsafe fn parse_fadt_and_dsdt(&mut self) {
        let fadt_ptr = match self.find_table(b"FACP") {
            Some(p) => p as *const u8,
            None => {
                crate::serial_println!("[ACPI] FADT ('FACP') table not detected.");
                return;
            }
        };

        let fadt_len = (*(fadt_ptr as *const SdtHeader)).length as usize;
        if fadt_len < 116 {
            return;
        }

        let smi_cmd = read_unaligned(fadt_ptr.add(48) as *const u32);
        let acpi_enable = *fadt_ptr.add(52);
        let pm1a_cnt_blk = read_unaligned(fadt_ptr.add(64) as *const u32) as u16;
        let pm1b_cnt_blk = read_unaligned(fadt_ptr.add(68) as *const u32) as u16;

        // DSDT physical address: 32-bit field at offset 40, or 64-bit X_DSDT at offset 140
        let dsdt_phys: u64 = if fadt_len >= 148 {
            let x_dsdt = read_unaligned(fadt_ptr.add(140) as *const u64);
            if x_dsdt != 0 { x_dsdt } else { read_unaligned(fadt_ptr.add(40) as *const u32) as u64 }
        } else {
            read_unaligned(fadt_ptr.add(40) as *const u32) as u64
        };

        if dsdt_phys == 0 {
            crate::serial_println!("[ACPI] DSDT address invalid in FADT.");
            return;
        }

        let dsdt_header = (self.hhdm_offset + dsdt_phys) as *const SdtHeader;
        if !validate_checksum(dsdt_header as *const u8, (*dsdt_header).length as usize) {
            crate::serial_println!("[ACPI] DSDT checksum verification failed.");
            return;
        }

        let dsdt_len = (*dsdt_header).length as usize;
        let dsdt_slice = core::slice::from_raw_parts(dsdt_header as *const u8, dsdt_len);

        if let Some((slp_a, slp_b)) = parse_aml_s5(dsdt_slice) {
            crate::serial_println!("[ACPI] FADT/DSDT Power Management online:");
            crate::serial_println!("       PM1a_CNT=0x{:04X}, PM1b_CNT=0x{:04X}, SLP_TYPa={}, SLP_TYPb={}",
                pm1a_cnt_blk, pm1b_cnt_blk, slp_a, slp_b);

            ACPI_SHUTDOWN_CONFIG = Some(AcpiShutdownInfo {
                pm1a_cnt_blk,
                pm1b_cnt_blk,
                slp_typa: slp_a,
                slp_typb: slp_b,
                smi_cmd,
                acpi_enable,
            });
        }
    }
}

/// AML bytecode scanner to locate and parse `_S5_` System Sleep State 5 (Shutdown).
fn parse_aml_s5(dsdt: &[u8]) -> Option<(u8, u8)> {
    let s5_pattern = b"_S5_";
    let len = dsdt.len();

    for i in 0..len.saturating_sub(16) {
        if &dsdt[i..i + 4] == s5_pattern {
            let mut pos = i + 4;
            // Seek AML PackageOp 0x12
            for _ in 0..4 {
                if pos < len && dsdt[pos] == 0x12 {
                    break;
                }
                pos += 1;
            }

            if pos >= len || dsdt[pos] != 0x12 {
                continue;
            }

            pos += 1; // Skip PackageOp (0x12)

            // Decode AML PkgLength
            if pos >= len { continue; }
            let lead = dsdt[pos];
            let byte_count = (lead >> 6) as usize;
            pos += 1 + byte_count;

            // NumElements
            if pos >= len { continue; }
            pos += 1;

            let extract_int = |p: &mut usize| -> Option<u8> {
                if *p >= len { return None; }
                let op = dsdt[*p];
                *p += 1;
                match op {
                    0x00 => Some(0), // ZeroOp
                    0x01 => Some(1), // OneOp
                    0x0A => {        // BytePrefix
                        if *p >= len { return None; }
                        let val = dsdt[*p];
                        *p += 1;
                        Some(val)
                    }
                    0x0B => {        // WordPrefix
                        if *p + 1 >= len { return None; }
                        let val = dsdt[*p];
                        *p += 2;
                        Some(val)
                    }
                    0x0C => {        // DwordPrefix
                        if *p + 3 >= len { return None; }
                        let val = dsdt[*p];
                        *p += 4;
                        Some(val)
                    }
                    small if small <= 0x0F => Some(small),
                    _ => None,
                }
            };

            let slp_a = extract_int(&mut pos)?;
            let slp_b = extract_int(&mut pos).unwrap_or(0);
            return Some((slp_a, slp_b));
        }
    }

    None
}

/// Executes clean ACPI system shutdown via PM1a_CNT / sleep type 5.
pub unsafe fn acpi_shutdown() -> ! {
    crate::serial_println!("[ACPI] Initiating Bare-Metal Hardware ACPI Shutdown...");

    if let Some(config) = ACPI_SHUTDOWN_CONFIG {
        if config.pm1a_cnt_blk != 0 {
            // Enable ACPI hardware mode if SCI_EN is clear
            if (msr::inw(config.pm1a_cnt_blk) & 1) == 0 && config.smi_cmd != 0 {
                msr::outb(config.smi_cmd as u16, config.acpi_enable);
                for _ in 0..300 {
                    if (msr::inw(config.pm1a_cnt_blk) & 1) != 0 {
                        break;
                    }
                    msr::io_wait();
                }
            }

            let slp_en: u16 = 1 << 13; // Bit 13: Sleep Enable
            let val_a: u16 = ((config.slp_typa as u16) << 10) | slp_en;
            msr::outw(config.pm1a_cnt_blk, val_a);

            if config.pm1b_cnt_blk != 0 {
                let val_b: u16 = ((config.slp_typb as u16) << 10) | slp_en;
                msr::outw(config.pm1b_cnt_blk, val_b);
            }
        }
    }

    // Hardware emulator fallbacks (QEMU, VirtualBox, Bochs)
    msr::outw(0x604, 0x2000);   // QEMU modern ACPI poweroff
    msr::outw(0x4004, 0x3400);  // VirtualBox poweroff
    msr::outw(0xB004, 0x2000);  // Bochs / Old QEMU
    msr::outb(0xF4, 0x00);      // QEMU debug exit device

    loop {
        core::arch::asm!("cli; hlt");
    }
}
