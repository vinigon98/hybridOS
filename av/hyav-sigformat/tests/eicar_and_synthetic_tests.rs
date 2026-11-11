use hyav_sigformat::crypto::{sha256, Keypair};
use hyav_sigformat::*;

const EICAR_STR: &[u8] = b"X5O!P%@AP[4\\PZX54(P^)7CC)7}$EICAR-STANDARD-ANTIVIRUS-TEST-FILE!$H+H*";

#[test]
fn test_eicar_exact_hash_and_bloom() {
    let eicar_hash = sha256(EICAR_STR);
    let clean_hash = sha256(b"This is completely safe legitimate content.");

    let eicar_rec = SignatureRecord::new(
        1001,
        "Virus.Test.EICAR-ExactHash",
        Severity::Critical,
        Action::Block,
        RecordPayload::HashExact(HashExactRecord::new(
            HashAlgo::Sha256,
            eicar_hash,
            EICAR_STR.len() as u64,
        )),
    );

    let db = DatabaseBuilder::new(DbType::Main)
        .add_record(eicar_rec)
        .build();

    // 1. Verify Bloom filter
    assert!(db.bloom.contains_sha256(&eicar_hash));
    assert!(!db.bloom.contains_sha256(&clean_hash));

    // 2. Exact hash matching
    let hit = db.find_hash_exact(&eicar_hash, EICAR_STR.len() as u64);
    assert!(hit.is_some());
    assert_eq!(hit.unwrap().name, "Virus.Test.EICAR-ExactHash");

    // Clean hash returns None
    assert!(db.find_hash_exact(&clean_hash, 43).is_none());
}

#[test]
fn test_eicar_pattern_matching() {
    let pattern_bytes = b"EICAR-STANDARD-ANTIVIRUS-TEST-FILE!".to_vec();
    let pat_rec = SignatureRecord::new(
        1002,
        "Virus.Test.EICAR-Pattern",
        Severity::Critical,
        Action::Quarantine,
        RecordPayload::Pattern(PatternRecord::exact(
            TargetFormat::Any,
            pattern_bytes,
            0,
            u32::MAX,
        )),
    );

    let db = DatabaseBuilder::new(DbType::Daily)
        .add_record(pat_rec)
        .build();

    // Test buffer with EICAR inside
    let mut infected_file = Vec::new();
    infected_file.extend_from_slice(b"Header padding 123456 ");
    infected_file.extend_from_slice(EICAR_STR);
    infected_file.extend_from_slice(b" trailing safe data");

    let matches = db.scan_patterns(&infected_file);
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].name, "Virus.Test.EICAR-Pattern");

    // Clean file
    let clean_file = b"Standard document text without any test string";
    let matches_clean = db.scan_patterns(clean_file);
    assert!(matches_clean.is_empty());
}

#[test]
fn test_synthetic_pe_heuristics() {
    let pe_rec = SignatureRecord::new(
        2001,
        "Heur.Suspicious.UPXPacked",
        Severity::High,
        Action::Alert,
        RecordPayload::PeHeader(PeHeaderRecord {
            machine: Some(0x8664), // AMD64
            characteristics: None,
            subsystem: Some(3), // CUI
            min_sections: Some(2),
            max_sections: Some(10),
            suspicious_section_names: vec!["UPX0".to_string(), "UPX1".to_string()],
            check_entry_in_last_section: true,
            max_entropy: Some(7.5),
        }),
    );

    let db = DatabaseBuilder::new(DbType::Main)
        .add_record(pe_rec)
        .build();

    // Synthetic malicious PE: has UPX0 and UPX1 sections, high entropy
    let malicious_pe = SyntheticPeInfo {
        machine: 0x8664,
        characteristics: 0x0022,
        subsystem: 3,
        entry_point: 0x2400,
        sections: vec![
            SyntheticPeSection {
                name: "UPX0".to_string(),
                virtual_size: 0x2000,
                virtual_address: 0x1000,
                raw_data_size: 0,
                raw_data_pointer: 0,
                characteristics: 0xE0000080,
            },
            SyntheticPeSection {
                name: "UPX1".to_string(),
                virtual_size: 0x1000,
                virtual_address: 0x2000,
                raw_data_size: 0x1000,
                raw_data_pointer: 0x400,
                characteristics: 0xE0000040,
            },
        ],
        max_section_entropy: 7.85,
    };

    let hits = db.scan_pe(&malicious_pe);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].name, "Heur.Suspicious.UPXPacked");

    // Synthetic clean PE: .text and .data, normal entropy
    let clean_pe = SyntheticPeInfo {
        machine: 0x8664,
        characteristics: 0x0022,
        subsystem: 3,
        entry_point: 0x1100,
        sections: vec![
            SyntheticPeSection {
                name: ".text".to_string(),
                virtual_size: 0x2000,
                virtual_address: 0x1000,
                raw_data_size: 0x2000,
                raw_data_pointer: 0x400,
                characteristics: 0x60000020,
            },
            SyntheticPeSection {
                name: ".data".to_string(),
                virtual_size: 0x1000,
                virtual_address: 0x3000,
                raw_data_size: 0x800,
                raw_data_pointer: 0x2400,
                characteristics: 0xC0000040,
            },
        ],
        max_section_entropy: 5.4,
    };

    let clean_hits = db.scan_pe(&clean_pe);
    assert!(clean_hits.is_empty());
}

#[test]
fn test_synthetic_elf_heuristics() {
    let elf_rec = SignatureRecord::new(
        3001,
        "Heur.Suspicious.ElfExecStack",
        Severity::High,
        Action::Block,
        RecordPayload::ElfHeader(ElfHeaderRecord {
            machine: Some(0x3E), // EM_X86_64
            elf_type: Some(2),   // ET_EXEC
            suspicious_interpreter: Some("/tmp/".to_string()),
            requires_exec_stack: true,
        }),
    );

    let db = DatabaseBuilder::new(DbType::Main)
        .add_record(elf_rec)
        .build();

    // Synthetic malicious ELF with executable stack and /tmp interpreter
    let malicious_elf = SyntheticElfInfo {
        machine: 0x3E,
        elf_type: 2,
        interpreter: Some("/tmp/fake_ld.so".to_string()),
        has_executable_stack: true,
    };

    let hits = db.scan_elf(&malicious_elf);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].name, "Heur.Suspicious.ElfExecStack");

    // Synthetic clean ELF
    let clean_elf = SyntheticElfInfo {
        machine: 0x3E,
        elf_type: 2,
        interpreter: Some("/lib64/ld-linux-x86-64.so.2".to_string()),
        has_executable_stack: false,
    };

    let clean_hits = db.scan_elf(&clean_elf);
    assert!(clean_hits.is_empty());
}

#[test]
fn test_path_rule_matching() {
    let path_rec1 = SignatureRecord::new(
        4001,
        "Threat.HackTool.Mimikatz",
        Severity::Critical,
        Action::Kill,
        RecordPayload::PathRule(PathRuleRecord::new("*mimikatz*.exe".to_string(), false, false)),
    );
    let path_rec2 = SignatureRecord::new(
        4002,
        "Suspicious.TempScript",
        Severity::Medium,
        Action::Alert,
        RecordPayload::PathRule(PathRuleRecord::new("*/temp/*.scr".to_string(), false, false)),
    );

    let db = DatabaseBuilder::new(DbType::Custom)
        .add_record(path_rec1)
        .add_record(path_rec2)
        .build();

    let hits1 = db.scan_path(r"C:\Tools\Bin\mimikatz_trunk.exe");
    assert_eq!(hits1.len(), 1);
    assert_eq!(hits1[0].name, "Threat.HackTool.Mimikatz");

    let hits2 = db.scan_path(r"C:\Users\User\AppData\Local\Temp\invoice.scr");
    assert_eq!(hits2.len(), 1);
    assert_eq!(hits2[0].name, "Suspicious.TempScript");

    let clean_hits = db.scan_path(r"C:\Windows\System32\notepad.exe");
    assert!(clean_hits.is_empty());
}

#[test]
fn test_container_codec_signing_and_tamper_proofing() {
    let signing_seed = [42u8; 32];
    let keypair = Keypair::from_seed(&signing_seed);

    let eicar_hash = sha256(EICAR_STR);
    let record = SignatureRecord::new(
        999,
        "Virus.Test.EICAR",
        Severity::Critical,
        Action::Block,
        RecordPayload::HashExact(HashExactRecord::new(
            HashAlgo::Sha256,
            eicar_hash,
            EICAR_STR.len() as u64,
        )),
    );

    let db = DatabaseBuilder::new(DbType::Main)
        .add_record(record)
        .build();

    // Encode and sign
    let encoded = db.encode(Some(&signing_seed)).expect("encoding");

    // 1. Verify container magic
    assert_eq!(&encoded[0..8], &SIG_MAGIC);

    // 2. Decode and verify raw binary
    let loaded = SignatureDatabase::verify_raw(&encoded, &keypair.public).expect("verification");
    assert_eq!(loaded.records.len(), 1);
    assert_eq!(loaded.records[0].name, "Virus.Test.EICAR");

    // 3. Test tampering resistance: flip one byte in the records section
    let mut tampered = encoded.clone();
    let records_offset = loaded.header.records_offset as usize;
    tampered[records_offset + 5] ^= 0xFF; // corrupt record content

    let tamper_result = SignatureDatabase::verify_raw(&tampered, &keypair.public);
    assert!(tamper_result.is_err());
    match tamper_result {
        Err(CodecError::VerificationFailed(TrailerError::Sha256Mismatch)) => {}
        other => panic!("Expected Sha256Mismatch, got {:?}", other),
    }

    // 4. Test wrong public key rejection
    let wrong_keypair = Keypair::from_seed(&[99u8; 32]);
    let wrong_key_result = SignatureDatabase::verify_raw(&encoded, &wrong_keypair.public);
    assert!(wrong_key_result.is_err());
    match wrong_key_result {
        Err(CodecError::VerificationFailed(TrailerError::Ed25519Invalid(_))) => {}
        other => panic!("Expected Ed25519Invalid, got {:?}", other),
    }
}
