use hyav_sigformat::elf::*;
use hyav_sigformat::{Elf64File, ParserError};

fn build_mock_elf64_le() -> Vec<u8> {
    let mut elf = vec![0u8; 1024];

    // 1. Ehdr (64 bytes)
    elf[0..4].copy_from_slice(&ELF_MAGIC); // \x7fELF
    elf[4] = ELFCLASS64; // 64-bit
    elf[5] = ELFDATA2LSB; // Little-endian
    elf[6] = 1; // EV_CURRENT
    elf[7] = 0; // System V
    elf[16..18].copy_from_slice(&2u16.to_le_bytes()); // ET_EXEC
    elf[18..20].copy_from_slice(&0x3Eu16.to_le_bytes()); // EM_X86_64
    elf[20..24].copy_from_slice(&1u32.to_le_bytes()); // e_version = 1
    elf[24..32].copy_from_slice(&0x401000u64.to_le_bytes()); // e_entry
    elf[32..40].copy_from_slice(&64u64.to_le_bytes()); // e_phoff = 64
    elf[40..48].copy_from_slice(&120u64.to_le_bytes()); // e_shoff = 120
    elf[48..52].copy_from_slice(&0u32.to_le_bytes()); // e_flags
    elf[52..54].copy_from_slice(&64u16.to_le_bytes()); // e_ehsize = 64
    elf[54..56].copy_from_slice(&56u16.to_le_bytes()); // e_phentsize = 56
    elf[56..58].copy_from_slice(&1u16.to_le_bytes()); // e_phnum = 1
    elf[58..60].copy_from_slice(&64u16.to_le_bytes()); // e_shentsize = 64
    elf[60..62].copy_from_slice(&5u16.to_le_bytes()); // e_shnum = 5
    elf[62..64].copy_from_slice(&2u16.to_le_bytes()); // e_shstrndx = 2 (.shstrtab)

    // 2. Phdr at offset 64 (56 bytes)
    let ph_off = 64;
    elf[ph_off..ph_off + 4].copy_from_slice(&PT_LOAD.to_le_bytes()); // p_type = PT_LOAD
    elf[ph_off + 4..ph_off + 8].copy_from_slice(&(PF_R | PF_X).to_le_bytes()); // p_flags = R | X
    elf[ph_off + 8..ph_off + 16].copy_from_slice(&0x200u64.to_le_bytes()); // p_offset = 512
    elf[ph_off + 16..ph_off + 24].copy_from_slice(&0x401000u64.to_le_bytes()); // p_vaddr
    elf[ph_off + 24..ph_off + 32].copy_from_slice(&0x401000u64.to_le_bytes()); // p_paddr
    elf[ph_off + 32..ph_off + 40].copy_from_slice(&0x100u64.to_le_bytes()); // p_filesz
    elf[ph_off + 40..ph_off + 48].copy_from_slice(&0x100u64.to_le_bytes()); // p_memsz
    elf[ph_off + 48..ph_off + 56].copy_from_slice(&0x1000u64.to_le_bytes()); // p_align

    // 3. Shdrs at offset 120 (5 * 64 = 320 bytes, ending at 440)
    // Section 0: NULL (all zeros at 120..184)

    // Section 1: .text at 184..248
    let s1 = 120 + 64;
    elf[s1..s1 + 4].copy_from_slice(&1u32.to_le_bytes()); // sh_name = 1 (offset into .shstrtab)
    elf[s1 + 4..s1 + 8].copy_from_slice(&SHT_PROGBITS.to_le_bytes()); // sh_type = PROGBITS
    elf[s1 + 8..s1 + 16].copy_from_slice(&(SHF_ALLOC | SHF_EXECINSTR).to_le_bytes()); // sh_flags
    elf[s1 + 16..s1 + 24].copy_from_slice(&0x401000u64.to_le_bytes()); // sh_addr
    elf[s1 + 24..s1 + 32].copy_from_slice(&0x200u64.to_le_bytes()); // sh_offset = 512
    elf[s1 + 32..s1 + 40].copy_from_slice(&0x100u64.to_le_bytes()); // sh_size = 256
    elf[s1 + 48..s1 + 56].copy_from_slice(&16u64.to_le_bytes()); // sh_addralign = 16

    // Section 2: .shstrtab at 248..312
    let s2 = s1 + 64;
    elf[s2..s2 + 4].copy_from_slice(&7u32.to_le_bytes()); // sh_name = 7 (".shstrtab")
    elf[s2 + 4..s2 + 8].copy_from_slice(&SHT_STRTAB.to_le_bytes());
    elf[s2 + 24..s2 + 32].copy_from_slice(&0x300u64.to_le_bytes()); // sh_offset = 768
    elf[s2 + 32..s2 + 40].copy_from_slice(&0x40u64.to_le_bytes()); // sh_size = 64

    // Section 3: .symtab at 312..376
    let s3 = s2 + 64;
    elf[s3..s3 + 4].copy_from_slice(&17u32.to_le_bytes()); // sh_name = 17 (".symtab")
    elf[s3 + 4..s3 + 8].copy_from_slice(&SHT_SYMTAB.to_le_bytes());
    elf[s3 + 24..s3 + 32].copy_from_slice(&0x340u64.to_le_bytes()); // sh_offset = 832
    elf[s3 + 32..s3 + 40].copy_from_slice(&48u64.to_le_bytes()); // sh_size = 48 (2 symbols)
    elf[s3 + 40..s3 + 44].copy_from_slice(&4u32.to_le_bytes()); // sh_link = 4 (associated .strtab)
    elf[s3 + 56..s3 + 64].copy_from_slice(&24u64.to_le_bytes()); // sh_entsize = 24

    // Section 4: .strtab at 376..440
    let s4 = s3 + 64;
    elf[s4..s4 + 4].copy_from_slice(&25u32.to_le_bytes()); // sh_name = 25 (".strtab")
    elf[s4 + 4..s4 + 8].copy_from_slice(&SHT_STRTAB.to_le_bytes());
    elf[s4 + 24..s4 + 32].copy_from_slice(&0x370u64.to_le_bytes()); // sh_offset = 880
    elf[s4 + 32..s4 + 40].copy_from_slice(&0x30u64.to_le_bytes()); // sh_size = 48

    // Payloads:
    // .text at offset 0x200 (512):
    elf[0x200] = 0x48; // mov rax, 60 (sys_exit)
    elf[0x201] = 0xC7;
    elf[0x202] = 0xC0;
    elf[0x203] = 0x3C;
    elf[0x204] = 0x00;
    elf[0x205] = 0x00;
    elf[0x206] = 0x00;
    elf[0x207] = 0x0F; // syscall
    elf[0x208] = 0x05;

    // .shstrtab at offset 0x300 (768):
    // \0.text\0.shstrtab\0.symtab\0.strtab\0
    let shstrtab_data = b"\0.text\0.shstrtab\0.symtab\0.strtab\0";
    elf[0x300..0x300 + shstrtab_data.len()].copy_from_slice(shstrtab_data);

    // .symtab at offset 0x340 (832): 2 symbols * 24 bytes
    // Sym 0: null symbol (zeros)
    // Sym 1: at 0x340 + 24 = 0x358:
    let sym1_off = 0x358;
    elf[sym1_off..sym1_off + 4].copy_from_slice(&1u32.to_le_bytes()); // st_name = 1 in .strtab ("main")
    elf[sym1_off + 4] = 0x12; // STB_GLOBAL (1 << 4) | STT_FUNC (2)
    elf[sym1_off + 6..sym1_off + 8].copy_from_slice(&1u16.to_le_bytes()); // st_shndx = 1 (.text)
    elf[sym1_off + 8..sym1_off + 16].copy_from_slice(&0x401000u64.to_le_bytes()); // st_value
    elf[sym1_off + 16..sym1_off + 24].copy_from_slice(&9u64.to_le_bytes()); // st_size = 9

    // .strtab at offset 0x370 (880):
    // \0main\0
    let strtab_data = b"\0main\0";
    elf[0x370..0x370 + strtab_data.len()].copy_from_slice(strtab_data);

    elf
}

#[test]
fn test_parse_valid_elf64_le() {
    let raw = build_mock_elf64_le();
    let elf = Elf64File::parse(&raw).expect("Parsing valid ELF64 should succeed");

    // Header validation
    assert_eq!(elf.header.ident_magic, ELF_MAGIC);
    assert_eq!(elf.header.ident_class, ELFCLASS64);
    assert_eq!(elf.header.endian, Endian::Little);
    assert_eq!(elf.header.e_type, 2); // ET_EXEC
    assert_eq!(elf.header.e_machine, 0x3E); // x86_64
    assert_eq!(elf.header.e_entry, 0x401000);
    assert_eq!(elf.entry_point(), 0x401000);

    // Program headers
    assert_eq!(elf.program_headers.len(), 1);
    assert_eq!(elf.program_headers[0].p_type, PT_LOAD);
    assert!(elf.program_headers[0].is_readable());
    assert!(elf.program_headers[0].is_executable());
    assert!(!elf.program_headers[0].is_writable());

    // Section headers and name resolution
    assert_eq!(elf.section_headers.len(), 5);
    let sec_text = elf.find_section(".text").expect(".text section found");
    assert_eq!(sec_text.sh_type, SHT_PROGBITS);
    assert!(sec_text.is_executable());
    assert!(sec_text.is_alloc());
    assert_eq!(sec_text.sh_addr, 0x401000);

    let text_code = elf.section_data(sec_text).unwrap();
    assert_eq!(text_code[0], 0x48);
    assert_eq!(text_code[7], 0x0F);
    assert_eq!(text_code[8], 0x05);

    // Symbol parsing
    let sec_symtab = elf.find_section(".symtab").expect(".symtab section found");
    let symbols = elf.parse_symbols(sec_symtab).expect("Symbols parsed");
    assert_eq!(symbols.len(), 2);
    assert_eq!(symbols[1].name, "main");
    assert_eq!(symbols[1].st_value, 0x401000);
    assert_eq!(symbols[1].st_size, 9);
    assert_eq!(symbols[1].bind(), 1); // GLOBAL
    assert_eq!(symbols[1].symbol_type(), 2); // FUNC
}

#[test]
fn test_elf_reject_invalid_magic() {
    let mut raw = build_mock_elf64_le();
    raw[0] = 0x00;
    let err = Elf64File::parse(&raw).unwrap_err();
    assert!(matches!(err, ParserError::InvalidMagic { .. }));
}

#[test]
fn test_elf_reject_32bit() {
    let mut raw = build_mock_elf64_le();
    raw[4] = 1; // ELFCLASS32
    let err = Elf64File::parse(&raw).unwrap_err();
    assert_eq!(err, ParserError::UnsupportedElfClass(1));
}

#[test]
fn test_elf_truncated_headers() {
    let raw = build_mock_elf64_le();
    assert!(Elf64File::parse(&raw[..32]).is_err());
    assert!(Elf64File::parse(&raw[..64]).is_err()); // Has Ehdr but cuts off Phdr at 64
    assert!(Elf64File::parse(&raw[..150]).is_err()); // Cuts off Shdrs
}

#[test]
fn test_elf64_big_endian_header() {
    let mut raw = vec![0u8; 64];
    raw[0..4].copy_from_slice(&ELF_MAGIC);
    raw[4] = ELFCLASS64;
    raw[5] = ELFDATA2MSB; // Big endian
    raw[6] = 1;
    raw[16..18].copy_from_slice(&2u16.to_be_bytes()); // ET_EXEC
    raw[18..20].copy_from_slice(&0xF3u16.to_be_bytes()); // RISC-V
    raw[24..32].copy_from_slice(&0x80000000u64.to_be_bytes()); // e_entry

    let reader = hyav_sigformat::ByteReader::new(&raw);
    let hdr = Elf64Header::parse(&reader).expect("Big endian header parse");
    assert_eq!(hdr.endian, Endian::Big);
    assert_eq!(hdr.e_type, 2);
    assert_eq!(hdr.e_machine, 0xF3);
    assert_eq!(hdr.e_entry, 0x80000000);
}
