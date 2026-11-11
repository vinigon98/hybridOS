use hyav_sigformat::hsv::*;
use hyav_sigformat::{ByteReader, ParserError};

#[test]
fn test_hsv_header_parse() {
    let mut data = vec![0u8; 64];
    data[0..4].copy_from_slice(&HSV_MAGIC);
    data[4..6].copy_from_slice(&1u16.to_le_bytes()); // major = 1
    data[6..8].copy_from_slice(&0u16.to_le_bytes()); // minor = 0
    data[8..10].copy_from_slice(&0x01u16.to_le_bytes()); // db_type = Main
    data[10..12].copy_from_slice(&0x02u16.to_le_bytes()); // flags = strict
    data[12..20].copy_from_slice(&1700000000u64.to_le_bytes()); // timestamp
    data[20..24].copy_from_slice(&100u32.to_le_bytes()); // 100 sigs
    data[24..26].copy_from_slice(&4u16.to_le_bytes()); // 4 sections
    data[36..44].copy_from_slice(&512u64.to_le_bytes()); // sig offset
    data[44..52].copy_from_slice(&1024u64.to_le_bytes()); // strtab offset

    let reader = ByteReader::new(&data);
    let hdr = HsvHeader::parse(&reader).expect("HSV header parsing");
    assert_eq!(hdr.magic, HSV_MAGIC);
    assert_eq!(hdr.format_major, 1);
    assert_eq!(hdr.db_type, 1);
    assert_eq!(hdr.total_signatures, 100);
}

#[test]
fn test_hsv_exact_hash_record() {
    let mut data = vec![0u8; 54];
    data[0] = 1; // BLAKE3
    data[1] = 2; // Critical
    data[2..6].copy_from_slice(&128u32.to_le_bytes()); // threat name offset
    data[6..14].copy_from_slice(&1000u64.to_le_bytes()); // min size
    data[14..22].copy_from_slice(&50000u64.to_le_bytes()); // max size
    data[22..54].fill(0xDE); // 32-byte hash

    let reader = ByteReader::new(&data);
    let exact = SigExactHash::parse(&reader, 0).expect("Exact hash parse");
    assert_eq!(exact.hash_algo, 1);
    assert_eq!(exact.severity, 2);
    assert_eq!(exact.threat_name_offset, 128);
    assert_eq!(exact.target_size_min, 1000);
    assert_eq!(exact.target_size_max, 50000);
    assert_eq!(exact.hash_value, [0xDE; 32]);
}

#[test]
fn test_hsv_byte_pattern_record() {
    let mut data = vec![0u8; 32];
    data[0] = 1; // PE
    data[1..5].copy_from_slice(&0u32.to_le_bytes()); // min_offset
    data[5..9].copy_from_slice(&1024u32.to_le_bytes()); // max_offset
    data[9..11].copy_from_slice(&4u16.to_le_bytes()); // len = 4 bytes
    data[11..15].copy_from_slice(&256u32.to_le_bytes()); // threat_name_offset

    // Pattern (4 bytes) at offset 15
    data[15..19].copy_from_slice(&[0x90, 0x90, 0xCC, 0xC3]);
    // Mask (4 bytes) at offset 19
    data[19..23].copy_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF]);

    let reader = ByteReader::new(&data);
    let (pat, total_size) = SigBytePattern::parse(&reader, 0).expect("Pattern parse");
    assert_eq!(pat.target_format, 1);
    assert_eq!(pat.pattern_len, 4);
    assert_eq!(pat.pattern, vec![0x90, 0x90, 0xCC, 0xC3]);
    assert_eq!(pat.mask, vec![0xFF, 0xFF, 0xFF, 0xFF]);
    assert_eq!(total_size, 15 + 4 + 4);
}

#[test]
fn test_hsv_invalid_magic() {
    let data = vec![0u8; 64];
    let reader = ByteReader::new(&data);
    let err = HsvHeader::parse(&reader).unwrap_err();
    assert!(matches!(err, ParserError::InvalidMagic { .. }));
}
