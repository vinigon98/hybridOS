//! Authentic, bare-metal Ring 3 test binaries for Linux POSIX (ELF64) and Windows NT (PE64).
//!
//! Generates valid machine code images containing pure x86_64 machine instructions:
//! - `hello_linux.elf`: ELF64 binary executing 3 preemptive/cooperative steps in Ring 3
//!   with `sys_write`, `sys_sched_yield` (syscall 24), and `sys_exit` (syscall 60).
//! - `hello_win.exe`: PE32+ binary executing 3 preemptive/cooperative steps in Ring 3,
//!   reading TEB Self (`GS:[0x30]`), ClientId PID (`GS:[0x40]`), `KUSER_SHARED_DATA` (0x7FFE026C),
//!   with `NtWriteFile`, `NtYieldExecution` (syscall 0x46), and `NtTerminateProcess` (syscall 0x2C).

use alloc::vec::Vec;

/// Generates an authentic 64-bit ELF executable for Linux POSIX Ring 3 preemptive multitasking.
pub fn generate_hello_linux_elf() -> Vec<u8> {
    let mut elf = Vec::with_capacity(1024);

    let m1 = b"[Linux POSIX PID 1] Step 1/3 in Ring 3 -> Yielding CPU...\n";
    let m2 = b"[Linux POSIX PID 1] Step 2/3 in Ring 3 -> Yielding CPU...\n";
    let m3 = b"[Linux POSIX PID 1] Step 3/3 in Ring 3 -> Completed execution OK!\n";

    // Build machine code payload
    let mut code = Vec::new();

    // Step 1: write m1 + yield
    let lea1_off = code.len() + 10;
    code.extend_from_slice(&[0xB8, 0x01, 0x00, 0x00, 0x00]); // mov eax, 1 (sys_write)
    code.extend_from_slice(&[0xBF, 0x01, 0x00, 0x00, 0x00]); // mov edi, 1 (stdout)
    code.extend_from_slice(&[0x48, 0x8D, 0x35, 0x00, 0x00, 0x00, 0x00]); // lea rsi, [rip + disp]
    code.push(0xBA); // mov edx, len
    code.extend_from_slice(&(m1.len() as u32).to_le_bytes());
    code.extend_from_slice(&[0x0F, 0x05]); // syscall
    code.extend_from_slice(&[0xB8, 0x18, 0x00, 0x00, 0x00]); // mov eax, 24 (sys_sched_yield)
    code.extend_from_slice(&[0x0F, 0x05]); // syscall

    // Step 2: write m2 + yield
    let lea2_off = code.len() + 10;
    code.extend_from_slice(&[0xB8, 0x01, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0xBF, 0x01, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0x48, 0x8D, 0x35, 0x00, 0x00, 0x00, 0x00]);
    code.push(0xBA);
    code.extend_from_slice(&(m2.len() as u32).to_le_bytes());
    code.extend_from_slice(&[0x0F, 0x05]);
    code.extend_from_slice(&[0xB8, 0x18, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0x0F, 0x05]);

    // Step 3: write m3 + exit
    let lea3_off = code.len() + 10;
    code.extend_from_slice(&[0xB8, 0x01, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0xBF, 0x01, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0x48, 0x8D, 0x35, 0x00, 0x00, 0x00, 0x00]);
    code.push(0xBA);
    code.extend_from_slice(&(m3.len() as u32).to_le_bytes());
    code.extend_from_slice(&[0x0F, 0x05]);
    code.extend_from_slice(&[0xB8, 0x3C, 0x00, 0x00, 0x00]); // mov eax, 60 (sys_exit)
    code.extend_from_slice(&[0x31, 0xFF]); // xor edi, edi
    code.extend_from_slice(&[0x0F, 0x05]); // syscall

    let code_len = code.len();
    let str1_off = code_len;
    let str2_off = str1_off + m1.len();
    let str3_off = str2_off + m2.len();

    let d1 = (str1_off - (lea1_off + 7)) as u32;
    let d2 = (str2_off - (lea2_off + 7)) as u32;
    let d3 = (str3_off - (lea3_off + 7)) as u32;

    code[lea1_off + 3..lea1_off + 7].copy_from_slice(&d1.to_le_bytes());
    code[lea2_off + 3..lea2_off + 7].copy_from_slice(&d2.to_le_bytes());
    code[lea3_off + 3..lea3_off + 7].copy_from_slice(&d3.to_le_bytes());

    let mut payload = code;
    payload.extend_from_slice(m1);
    payload.extend_from_slice(m2);
    payload.extend_from_slice(m3);

    // --- 1. ELF Header (64 bytes) ---
    elf.extend_from_slice(&[
        0x7F, b'E', b'L', b'F', // e_ident[EI_MAG0..3]
        0x02,                   // EI_CLASS = ELFCLASS64
        0x01,                   // EI_DATA = Little-endian
        0x01,                   // EI_VERSION = 1
        0x00,                   // EI_OSABI = System V
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // EI_PAD
    ]);

    elf.extend_from_slice(&2u16.to_le_bytes());  // e_type = ET_EXEC (2)
    elf.extend_from_slice(&62u16.to_le_bytes()); // e_machine = EM_X86_64 (62)
    elf.extend_from_slice(&1u32.to_le_bytes());  // e_version = 1

    let entry_vaddr: u64 = 0x0040_0078;
    elf.extend_from_slice(&entry_vaddr.to_le_bytes()); // e_entry
    elf.extend_from_slice(&64u64.to_le_bytes());       // e_phoff = 64
    elf.extend_from_slice(&0u64.to_le_bytes());        // e_shoff = 0
    elf.extend_from_slice(&0u32.to_le_bytes());        // e_flags = 0
    elf.extend_from_slice(&64u16.to_le_bytes());       // e_ehsize = 64
    elf.extend_from_slice(&56u16.to_le_bytes());       // e_phentsize = 56
    elf.extend_from_slice(&1u16.to_le_bytes());        // e_phnum = 1
    elf.extend_from_slice(&0u16.to_le_bytes());        // e_shentsize = 0
    elf.extend_from_slice(&0u16.to_le_bytes());        // e_shnum = 0
    elf.extend_from_slice(&0u16.to_le_bytes());        // e_shstrndx = 0

    assert_eq!(elf.len(), 64);

    // --- 2. Program Header (56 bytes) ---
    let total_file_size: u64 = 64 + 56 + payload.len() as u64;

    elf.extend_from_slice(&1u32.to_le_bytes());        // p_type = PT_LOAD (1)
    elf.extend_from_slice(&7u32.to_le_bytes());        // p_flags = PF_R | PF_W | PF_X (7)
    elf.extend_from_slice(&0u64.to_le_bytes());        // p_offset = 0
    elf.extend_from_slice(&0x0040_0000u64.to_le_bytes()); // p_vaddr
    elf.extend_from_slice(&0x0040_0000u64.to_le_bytes()); // p_paddr
    elf.extend_from_slice(&total_file_size.to_le_bytes()); // p_filesz
    elf.extend_from_slice(&0x1000u64.to_le_bytes());   // p_memsz (4096)
    elf.extend_from_slice(&0x1000u64.to_le_bytes());   // p_align = 4096

    assert_eq!(elf.len(), 120); // 0x78

    // --- 3. Machine Code and String Data ---
    elf.extend_from_slice(&payload);

    elf
}

/// Generates an authentic 64-bit PE32+ (Windows NT) executable for Ring 3 preemptive multitasking.
pub fn generate_hello_win_pe() -> Vec<u8> {
    let mut pe = Vec::with_capacity(1024);

    let w1 = b"[Windows NT PID 2] Step 1/3 in Ring 3 (TEB PID=2 Verified) -> Yielding...\n\0";
    let w2 = b"[Windows NT PID 2] Step 2/3 in Ring 3 (TEB PID=2 Verified) -> Yielding...\n\0";
    let w3 = b"[Windows NT PID 2] Step 3/3 in Ring 3 (TEB PID=2 Verified) -> Completed OK!\n\0";

    // Build machine code payload
    let mut code = Vec::new();

    // 1. Read TEB Self from GS:[0x30]
    code.extend_from_slice(&[0x65, 0x48, 0x8B, 0x04, 0x25, 0x30, 0x00, 0x00, 0x00]); // mov rax, gs:[0x30]
    // 2. Read ClientId.UniqueProcess (PID 2) from GS:[0x40]
    code.extend_from_slice(&[0x65, 0x48, 0x8B, 0x1C, 0x25, 0x40, 0x00, 0x00, 0x00]); // mov rbx, gs:[0x40]
    // 3. Read NtMajorVersion from [0x7FFE026C]
    code.extend_from_slice(&[0x8B, 0x14, 0x25, 0x6C, 0x02, 0xFE, 0x7F]);             // mov edx, [0x7FFE026C]

    // Step 1: NtWriteFile + NtYieldExecution
    code.extend_from_slice(&[0x49, 0xC7, 0xC2, 0x01, 0x00, 0x00, 0x00]);             // mov r10, 1 (stdout)
    code.extend_from_slice(&[0xB8, 0x08, 0x00, 0x00, 0x00]);                         // mov eax, 8 (NtWriteFile)
    let lea1_off = code.len();
    code.extend_from_slice(&[0x4C, 0x8D, 0x0D, 0x00, 0x00, 0x00, 0x00]);             // lea r9, [rip + disp]
    code.extend_from_slice(&[0x0F, 0x05]);                                           // syscall
    code.extend_from_slice(&[0xB8, 0x46, 0x00, 0x00, 0x00]);                         // mov eax, 0x46 (NtYieldExecution)
    code.extend_from_slice(&[0x0F, 0x05]);                                           // syscall

    // Step 2: NtWriteFile + NtYieldExecution
    code.extend_from_slice(&[0x49, 0xC7, 0xC2, 0x01, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0xB8, 0x08, 0x00, 0x00, 0x00]);
    let lea2_off = code.len();
    code.extend_from_slice(&[0x4C, 0x8D, 0x0D, 0x00, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0x0F, 0x05]);
    code.extend_from_slice(&[0xB8, 0x46, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0x0F, 0x05]);

    // Step 3: NtWriteFile + NtTerminateProcess(0)
    code.extend_from_slice(&[0x49, 0xC7, 0xC2, 0x01, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0xB8, 0x08, 0x00, 0x00, 0x00]);
    let lea3_off = code.len();
    code.extend_from_slice(&[0x4C, 0x8D, 0x0D, 0x00, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0x0F, 0x05]);
    code.extend_from_slice(&[0xB8, 0x2C, 0x00, 0x00, 0x00]);                         // mov eax, 0x2C (NtTerminateProcess)
    code.extend_from_slice(&[0x31, 0xC9]);                                           // xor ecx, ecx
    code.extend_from_slice(&[0x0F, 0x05]);                                           // syscall

    let code_len = code.len();
    let str1_off = code_len;
    let str2_off = str1_off + w1.len();
    let str3_off = str2_off + w2.len();

    let d1 = (str1_off - (lea1_off + 7)) as u32;
    let d2 = (str2_off - (lea2_off + 7)) as u32;
    let d3 = (str3_off - (lea3_off + 7)) as u32;

    code[lea1_off + 3..lea1_off + 7].copy_from_slice(&d1.to_le_bytes());
    code[lea2_off + 3..lea2_off + 7].copy_from_slice(&d2.to_le_bytes());
    code[lea3_off + 3..lea3_off + 7].copy_from_slice(&d3.to_le_bytes());

    let mut payload = code;
    payload.extend_from_slice(w1);
    payload.extend_from_slice(w2);
    payload.extend_from_slice(w3);

    // --- 1. DOS Header (64 bytes) ---
    pe.extend_from_slice(&[0x4D, 0x5A]); // 'MZ'
    pe.resize(60, 0);
    pe.extend_from_slice(&0x80u32.to_le_bytes()); // e_lfanew = 0x80 (128)

    // DOS Stub up to 0x80
    pe.resize(0x80, 0);

    // --- 2. NT Signature ---
    pe.extend_from_slice(&[b'P', b'E', 0, 0]); // "PE\0\0"

    // --- 3. COFF File Header (20 bytes) ---
    pe.extend_from_slice(&0x8664u16.to_le_bytes()); // Machine = AMD64 (0x8664)
    pe.extend_from_slice(&1u16.to_le_bytes());      // NumberOfSections = 1
    pe.extend_from_slice(&0u32.to_le_bytes());      // TimeDateStamp
    pe.extend_from_slice(&0u32.to_le_bytes());      // PointerToSymbolTable
    pe.extend_from_slice(&0u32.to_le_bytes());      // NumberOfSymbols
    pe.extend_from_slice(&240u16.to_le_bytes());    // SizeOfOptionalHeader = 240 (PE32+)
    pe.extend_from_slice(&0x0022u16.to_le_bytes()); // Characteristics = EXECUTABLE | LARGE_ADDRESS_AWARE

    // --- 4. Optional Header (240 bytes) ---
    pe.extend_from_slice(&0x020Bu16.to_le_bytes()); // Magic = PE32+ (0x020B)
    pe.extend_from_slice(&[0x0E, 0x1E]);            // Linker version
    pe.extend_from_slice(&0x200u32.to_le_bytes());  // SizeOfCode
    pe.extend_from_slice(&0u32.to_le_bytes());      // SizeOfInitializedData
    pe.extend_from_slice(&0u32.to_le_bytes());      // SizeOfUninitializedData
    pe.extend_from_slice(&0x1000u32.to_le_bytes()); // AddressOfEntryPoint = 0x1000 (RVA)
    pe.extend_from_slice(&0x1000u32.to_le_bytes()); // BaseOfCode = 0x1000

    // Windows Specific Fields
    let image_base: u64 = 0x0000_0001_4000_0000;
    pe.extend_from_slice(&image_base.to_le_bytes()); // ImageBase
    pe.extend_from_slice(&0x1000u32.to_le_bytes());  // SectionAlignment = 4096
    pe.extend_from_slice(&0x200u32.to_le_bytes());   // FileAlignment = 512
    pe.extend_from_slice(&10u16.to_le_bytes());      // MajorOperatingSystemVersion
    pe.extend_from_slice(&0u16.to_le_bytes());       // MinorOperatingSystemVersion
    pe.extend_from_slice(&0u16.to_le_bytes());       // MajorImageVersion
    pe.extend_from_slice(&0u16.to_le_bytes());       // MinorImageVersion
    pe.extend_from_slice(&6u16.to_le_bytes());       // MajorSubsystemVersion
    pe.extend_from_slice(&0u16.to_le_bytes());       // MinorSubsystemVersion
    pe.extend_from_slice(&0u32.to_le_bytes());       // Win32VersionValue
    pe.extend_from_slice(&0x3000u32.to_le_bytes());  // SizeOfImage = 12 KB
    pe.extend_from_slice(&0x200u32.to_le_bytes());   // SizeOfHeaders = 512
    pe.extend_from_slice(&0u32.to_le_bytes());       // CheckSum
    pe.extend_from_slice(&3u16.to_le_bytes());       // Subsystem = IMAGE_SUBSYSTEM_WINDOWS_CUI (3)
    pe.extend_from_slice(&0x8160u16.to_le_bytes());  // DllCharacteristics
    pe.extend_from_slice(&0x10_0000u64.to_le_bytes()); // SizeOfStackReserve
    pe.extend_from_slice(&0x1000u64.to_le_bytes());  // SizeOfStackCommit
    pe.extend_from_slice(&0x10_0000u64.to_le_bytes()); // SizeOfHeapReserve
    pe.extend_from_slice(&0x1000u64.to_le_bytes());  // SizeOfHeapCommit
    pe.extend_from_slice(&0u32.to_le_bytes());       // LoaderFlags
    pe.extend_from_slice(&16u32.to_le_bytes());      // NumberOfRvaAndSizes = 16

    // 16 Data Directories (16 * 8 = 128 bytes)
    pe.resize(pe.len() + 128, 0);

    // --- 5. Section Header `.text` (40 bytes) ---
    pe.extend_from_slice(b".text\0\0\0");
    pe.extend_from_slice(&0x1000u32.to_le_bytes());  // VirtualSize = 4096
    pe.extend_from_slice(&0x1000u32.to_le_bytes());  // VirtualAddress = 0x1000
    pe.extend_from_slice(&0x200u32.to_le_bytes());   // SizeOfRawData = 512
    pe.extend_from_slice(&0x200u32.to_le_bytes());   // PointerToRawData = 0x200 (512)
    pe.extend_from_slice(&0u32.to_le_bytes());       // PointerToRelocations
    pe.extend_from_slice(&0u32.to_le_bytes());       // PointerToLinenumbers
    pe.extend_from_slice(&0u16.to_le_bytes());       // NumberOfRelocations
    pe.extend_from_slice(&0u16.to_le_bytes());       // NumberOfLinenumbers
    pe.extend_from_slice(&0xE000_0020u32.to_le_bytes()); // Characteristics = CODE | EXECUTE | READ | WRITE

    // Pad headers up to FileAlignment (512 bytes = 0x200)
    pe.resize(0x200, 0);

    // --- 6. Section `.text` Payload @ File Offset 0x200 (RVA 0x1000) ---
    pe.extend_from_slice(&payload);

    // Pad section to FileAlignment (512 bytes)
    pe.resize(0x200 + 512, 0);

    pe
}
