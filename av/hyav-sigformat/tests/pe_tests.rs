use hyav_sigformat::pe::*;
use hyav_sigformat::{ParserError, Pe64File};

/// Helper to build a minimal valid PE32+ binary in memory with sections, imports, and exports.
fn build_mock_pe64() -> Vec<u8> {
    let mut pe = vec![0u8; 1024];

    // 1. DOS Header
    pe[0..2].copy_from_slice(&DOS_MAGIC.to_le_bytes()); // e_magic = "MZ"
    pe[0x3C..0x40].copy_from_slice(&64u32.to_le_bytes()); // e_lfanew = 64

    // 2. PE Signature at offset 64
    let pe_sig_off = 64;
    pe[pe_sig_off..pe_sig_off + 4].copy_from_slice(&PE_SIGNATURE.to_le_bytes()); // "PE\0\0"

    // 3. COFF File Header at offset 68 (20 bytes)
    let coff_off = pe_sig_off + 4;
    pe[coff_off..coff_off + 2].copy_from_slice(&0x8664u16.to_le_bytes()); // Machine: AMD64
    pe[coff_off + 2..coff_off + 4].copy_from_slice(&2u16.to_le_bytes()); // NumberOfSections: 2
    pe[coff_off + 4..coff_off + 8].copy_from_slice(&1700000000u32.to_le_bytes()); // TimeDateStamp
    pe[coff_off + 16..coff_off + 18].copy_from_slice(&240u16.to_le_bytes()); // SizeOfOptionalHeader = 240
    pe[coff_off + 18..coff_off + 20].copy_from_slice(&0x0022u16.to_le_bytes()); // Characteristics: EXECUTABLE_IMAGE | LARGE_ADDRESS_AWARE

    // 4. Optional Header 64 at offset 88 (240 bytes)
    let opt_off = coff_off + 20;
    pe[opt_off..opt_off + 2].copy_from_slice(&PE32_PLUS_MAGIC.to_le_bytes()); // Magic: 0x020B
    pe[opt_off + 2] = 14; // MajorLinkerVersion
    pe[opt_off + 3] = 0; // MinorLinkerVersion
    pe[opt_off + 16..opt_off + 20].copy_from_slice(&0x1000u32.to_le_bytes()); // AddressOfEntryPoint (RVA 0x1000)
    pe[opt_off + 24..opt_off + 32].copy_from_slice(&0x0000000140000000u64.to_le_bytes()); // ImageBase
    pe[opt_off + 32..opt_off + 36].copy_from_slice(&0x1000u32.to_le_bytes()); // SectionAlignment
    pe[opt_off + 36..opt_off + 40].copy_from_slice(&0x200u32.to_le_bytes()); // FileAlignment
    pe[opt_off + 56..opt_off + 60].copy_from_slice(&0x3000u32.to_le_bytes()); // SizeOfImage
    pe[opt_off + 60..opt_off + 64].copy_from_slice(&0x400u32.to_le_bytes()); // SizeOfHeaders
    pe[opt_off + 68..opt_off + 70].copy_from_slice(&3u16.to_le_bytes()); // Subsystem: WINDOWS_CUI
    pe[opt_off + 108..opt_off + 112].copy_from_slice(&16u32.to_le_bytes()); // NumberOfRvaAndSizes

    // Data Directories start at opt_off + 112
    let dd_off = opt_off + 112;
    // Export Table (Index 0) at RVA 0x2000, size 0x100
    pe[dd_off..dd_off + 4].copy_from_slice(&0x2000u32.to_le_bytes());
    pe[dd_off + 4..dd_off + 8].copy_from_slice(&0x100u32.to_le_bytes());
    // Import Table (Index 1) at RVA 0x2100, size 0x100
    pe[dd_off + 8..dd_off + 12].copy_from_slice(&0x2100u32.to_le_bytes());
    pe[dd_off + 12..dd_off + 16].copy_from_slice(&0x100u32.to_le_bytes());

    // 5. Section Headers at offset opt_off + 240 = 328
    let sec1_off = opt_off + 240;
    // Section 1: .text
    pe[sec1_off..sec1_off + 5].copy_from_slice(b".text");
    pe[sec1_off + 8..sec1_off + 12].copy_from_slice(&0x200u32.to_le_bytes()); // VirtualSize: 0x200
    pe[sec1_off + 12..sec1_off + 16].copy_from_slice(&0x1000u32.to_le_bytes()); // VirtualAddress: 0x1000
    pe[sec1_off + 16..sec1_off + 20].copy_from_slice(&0x200u32.to_le_bytes()); // SizeOfRawData: 0x200
    pe[sec1_off + 20..sec1_off + 24].copy_from_slice(&0x400u32.to_le_bytes()); // PointerToRawData: 0x400 (offset 1024)
    pe[sec1_off + 36..sec1_off + 40].copy_from_slice(&0x60000020u32.to_le_bytes()); // CODE | EXECUTE | READ

    // Section 2: .rdata (holds export and import tables)
    let sec2_off = sec1_off + 40;
    pe[sec2_off..sec2_off + 6].copy_from_slice(b".rdata");
    pe[sec2_off + 8..sec2_off + 12].copy_from_slice(&0x400u32.to_le_bytes()); // VirtualSize: 0x400
    pe[sec2_off + 12..sec2_off + 16].copy_from_slice(&0x2000u32.to_le_bytes()); // VirtualAddress: 0x2000
    pe[sec2_off + 16..sec2_off + 20].copy_from_slice(&0x400u32.to_le_bytes()); // SizeOfRawData: 0x400
    pe[sec2_off + 20..sec2_off + 24].copy_from_slice(&0x600u32.to_le_bytes()); // PointerToRawData: 0x600 (offset 1536)
    pe[sec2_off + 36..sec2_off + 40].copy_from_slice(&0x40000040u32.to_le_bytes()); // INITIALIZED_DATA | READ

    // Resize file to accommodate raw data sections:
    // .text at 0x400..0x600 (1024..1536)
    // .rdata at 0x600..0xA00 (1536..2560)
    pe.resize(2560, 0);

    // .text section dummy payload: NOPsled + RET
    pe[0x400] = 0x90;
    pe[0x401] = 0xC3;

    // --- .rdata Section Payload (offset 0x600 = RVA 0x2000) ---
    // A. Export Directory Table at RVA 0x2000 (offset 0x600)
    let exp_off = 0x600;
    pe[exp_off + 12..exp_off + 16].copy_from_slice(&0x2050u32.to_le_bytes()); // NameRva = 0x2050
    pe[exp_off + 16..exp_off + 20].copy_from_slice(&1u32.to_le_bytes()); // OrdinalBase = 1
    pe[exp_off + 20..exp_off + 24].copy_from_slice(&1u32.to_le_bytes()); // NumberOfFunctions = 1
    pe[exp_off + 24..exp_off + 28].copy_from_slice(&1u32.to_le_bytes()); // NumberOfNames = 1
    pe[exp_off + 28..exp_off + 32].copy_from_slice(&0x2030u32.to_le_bytes()); // AddressOfFunctionsRva = 0x2030
    pe[exp_off + 32..exp_off + 36].copy_from_slice(&0x2038u32.to_le_bytes()); // AddressOfNamesRva = 0x2038
    pe[exp_off + 36..exp_off + 40].copy_from_slice(&0x2040u32.to_le_bytes()); // AddressOfNameOrdinalsRva = 0x2040

    // Export functions table at RVA 0x2030 (offset 0x630): fn RVA = 0x1000
    pe[0x630..0x634].copy_from_slice(&0x1000u32.to_le_bytes());
    // Export names table at RVA 0x2038 (offset 0x638): name RVA = 0x2060
    pe[0x638..0x63C].copy_from_slice(&0x2060u32.to_le_bytes());
    // Export ordinals table at RVA 0x2040 (offset 0x640): ord index = 0
    pe[0x640..0x642].copy_from_slice(&0u16.to_le_bytes());

    // Export DLL name at RVA 0x2050 (offset 0x650): "sample.dll\0"
    let dll_bytes = b"sample.dll\0";
    pe[0x650..0x650 + dll_bytes.len()].copy_from_slice(dll_bytes);

    // Export Function name at RVA 0x2060 (offset 0x660): "PayloadInit\0"
    let fn_bytes = b"PayloadInit\0";
    pe[0x660..0x660 + fn_bytes.len()].copy_from_slice(fn_bytes);

    // B. Import Directory Table at RVA 0x2100 (offset 0x700)
    let imp_off = 0x700;
    // Descriptor 1:
    pe[imp_off..imp_off + 4].copy_from_slice(&0x2140u32.to_le_bytes()); // OriginalFirstThunk = 0x2140
    pe[imp_off + 12..imp_off + 16].copy_from_slice(&0x2180u32.to_le_bytes()); // NameRva = 0x2180 ("kernel32.dll")
    pe[imp_off + 16..imp_off + 20].copy_from_slice(&0x2160u32.to_le_bytes()); // FirstThunk = 0x2160
    // Descriptor 2 is null terminator (zeros at imp_off + 20..imp_off + 40)

    // OriginalFirstThunk table at RVA 0x2140 (offset 0x740)
    // Entry 1: Name import at RVA 0x21A0
    pe[0x740..0x748].copy_from_slice(&0x21A0u64.to_le_bytes());
    // Entry 2: null terminator (0u64)

    // Import DLL Name at RVA 0x2180 (offset 0x780): "kernel32.dll\0"
    let k32 = b"kernel32.dll\0";
    pe[0x780..0x780 + k32.len()].copy_from_slice(k32);

    // Hint/Name entry at RVA 0x21A0 (offset 0x7A0):
    // Hint: 0x0001 (2 bytes)
    pe[0x7A0..0x7A2].copy_from_slice(&1u16.to_le_bytes());
    // Name: "ExitProcess\0"
    let imp_fn = b"ExitProcess\0";
    pe[0x7A2..0x7A2 + imp_fn.len()].copy_from_slice(imp_fn);

    pe
}

#[test]
fn test_parse_valid_pe64() {
    let raw = build_mock_pe64();
    let pe = Pe64File::parse(&raw).expect("Parsing valid PE64 should succeed");

    // DOS Header
    assert_eq!(pe.dos_header.e_magic, DOS_MAGIC);
    assert_eq!(pe.dos_header.e_lfanew, 64);

    // COFF Header
    assert_eq!(pe.coff_header.machine, 0x8664);
    assert_eq!(pe.coff_header.number_of_sections, 2);

    // Optional Header
    assert_eq!(pe.optional_header.magic, PE32_PLUS_MAGIC);
    assert_eq!(pe.optional_header.address_of_entry_point, 0x1000);
    assert_eq!(pe.optional_header.image_base, 0x0000000140000000);

    // Section Headers
    assert_eq!(pe.sections.len(), 2);
    assert_eq!(pe.sections[0].name, ".text");
    assert_eq!(pe.sections[0].virtual_address, 0x1000);
    assert_eq!(pe.sections[1].name, ".rdata");
    assert_eq!(pe.sections[1].virtual_address, 0x2000);

    // RVA Translation
    let file_off = pe.rva_to_file_offset(0x1000).expect("RVA should resolve");
    assert_eq!(file_off, 0x400);

    // Read Section Data
    let text_data = pe.section_data(&pe.sections[0]).unwrap();
    assert_eq!(text_data[0], 0x90);
    assert_eq!(text_data[1], 0xC3);

    // Imports
    let imports = pe.parse_imports().expect("Imports parsing should succeed");
    assert_eq!(imports.len(), 1);
    assert_eq!(imports[0].dll_name, "kernel32.dll");
    assert_eq!(imports[0].symbols.len(), 1);
    assert_eq!(imports[0].symbols[0].name.as_deref(), Some("ExitProcess"));
    assert_eq!(imports[0].symbols[0].hint, Some(1));

    // Exports
    let export_dir = pe.parse_exports().expect("Exports parsing should succeed").unwrap();
    assert_eq!(export_dir.name, "sample.dll");
    assert_eq!(export_dir.exports.len(), 1);
    assert_eq!(export_dir.exports[0].name.as_deref(), Some("PayloadInit"));
    assert_eq!(export_dir.exports[0].ordinal, 1);
    assert_eq!(export_dir.exports[0].rva, 0x1000);
}

#[test]
fn test_pe_invalid_dos_magic() {
    let mut raw = build_mock_pe64();
    raw[0] = b'X';
    raw[1] = b'X';
    let err = Pe64File::parse(&raw).unwrap_err();
    assert!(matches!(err, ParserError::InvalidDosHeader(_)));
}

#[test]
fn test_pe_invalid_pe_signature() {
    let mut raw = build_mock_pe64();
    raw[64] = b'N';
    raw[65] = b'E';
    let err = Pe64File::parse(&raw).unwrap_err();
    assert!(matches!(err, ParserError::InvalidPeSignature(64)));
}

#[test]
fn test_pe_reject_pe32_magic() {
    let mut raw = build_mock_pe64();
    let opt_off = 64 + 4 + 20;
    // Set magic to 0x010B (standard 32-bit PE)
    raw[opt_off..opt_off + 2].copy_from_slice(&0x010Bu16.to_le_bytes());
    let err = Pe64File::parse(&raw).unwrap_err();
    assert_eq!(err, ParserError::InvalidOptionalHeaderMagic(0x010B));
}

#[test]
fn test_pe_truncated_buffer() {
    let raw = build_mock_pe64();
    // Test truncation at various boundary levels
    assert!(Pe64File::parse(&raw[..32]).is_err());
    assert!(Pe64File::parse(&raw[..68]).is_err());
    assert!(Pe64File::parse(&raw[..120]).is_err());
    assert!(Pe64File::parse(&raw[..350]).is_err());
}

#[test]
fn test_pe_entropy_calculation() {
    let uniform = vec![0xAA; 1000];
    let entropy_zero = Pe64File::calculate_entropy(&uniform);
    assert!(entropy_zero.abs() < 1e-6);

    let mut random_bytes = Vec::new();
    for i in 0..256 {
        random_bytes.push(i as u8);
    }
    let entropy_max = Pe64File::calculate_entropy(&random_bytes);
    // 256 equally likely bytes should have entropy ~8.0
    assert!((entropy_max - 8.0).abs() < 1e-4);
}
