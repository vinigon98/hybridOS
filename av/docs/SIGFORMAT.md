# HyAV Signature Format Specification (`SIGFORMAT.md`)

## 1. Overview
The **HyAV Signature Format** (`.hsv` - HybridOS Signature Vault) is an optimized, memory-mappable binary container designed for deterministic, low-latency pattern matching in `hyav-sigformat` and `hyav-worker`.

Key requirements:
- **Zero-Copy Ingestion**: Readily memory-mapped (`mmap`) into kernel or worker spaces without runtime serialization overhead.
- **Cryptographic Authenticity**: Signed via Ed25519; verified before mounting.
- **Multi-Engine Support**: Supports exact hashes, byte pattern tries (Aho-Corasick), fuzzy locality hashes, and structural PE/ELF heuristics.

---

## 2. Binary Layout

```
+-------------------------------------------------------------+
| Header (64 bytes)                                           |
| Magic: 'HYAV' | Version | DB Type | Timestamp | Sig Count   |
+-------------------------------------------------------------+
| Section Table (Array of SectionDescriptor, 32 bytes each)   |
+-------------------------------------------------------------+
| Exact Hash Table (BLAKE3 / SHA-256 Sorted Lookup Array)     |
+-------------------------------------------------------------+
| Byte Pattern Trie Section (Aho-Corasick DFA / Wildcard)     |
+-------------------------------------------------------------+
| Structural & Heuristic Rule Section (Bytecode / AST)        |
+-------------------------------------------------------------+
| String Table (Threat Names & Metadata)                      |
+-------------------------------------------------------------+
| Ed25519 Digital Signature Block (64 bytes)                  |
+-------------------------------------------------------------+
```

---

## 3. Structural Definitions

### 3.1 Container Header
```rust
#[repr(C, packed)]
pub struct HsvHeader {
    pub magic: [u8; 4],           // b"HYAV" (0x48, 0x59, 0x41, 0x56)
    pub format_major: u16,        // 1
    pub format_minor: u16,        // 0
    pub db_type: u16,             // 0x01 = Main, 0x02 = Daily, 0x03 = Custom
    pub flags: u16,               // Bit 0: Compressed Sections, Bit 1: Strict Mode
    pub build_timestamp: u64,     // Unix epoch timestamp (seconds)
    pub total_signatures: u32,    // Total rules count
    pub section_count: u16,       // Number of sections
    pub reserved: [u8; 10],       // Alignment padding
    pub signature_offset: u64,    // Offset to Ed25519 signature
    pub string_table_offset: u64, // Offset to string table
}
```

### 3.2 Signature Record Types

#### Type 0x01: Exact Cryptographic Hash (`SigExactHash`)
Used for instant O(log N) binary search lookup of known clean or malicious files:
```rust
#[repr(C, packed)]
pub struct SigExactHash {
    pub hash_algo: u8,            // 0 = SHA256, 1 = BLAKE3
    pub severity: u8,             // 0 = Whitelist, 1 = Suspicious, 2 = Critical
    pub threat_name_offset: u32,  // Offset into String Table
    pub target_size_min: u64,     // File size filter (0 = any)
    pub target_size_max: u64,     // File size filter (0 = any)
    pub hash_value: [u8; 32],     // Raw 256-bit hash
}
```

#### Type 0x02: Byte Sequence / Wildcard (`SigBytePattern`)
```rust
#[repr(C, packed)]
pub struct SigBytePattern {
    pub target_format: u8,        // 0 = Any, 1 = PE, 2 = ELF, 3 = Script
    pub min_offset: u32,          // Scan window start
    pub max_offset: u32,          // Scan window end
    pub pattern_len: u16,         // Pattern length in bytes
    pub threat_name_offset: u32,
    // Followed by `pattern_len` bytes and `pattern_len` bitmask bytes
}
```

#### Type 0x03: Structural / Heuristic Rule (`SigHeuristic`)
Rules targeting PE/ELF anomalies:
- Entry point outside valid code sections.
- High entropy (> 7.4) in non-resource sections indicating packing.
- Abnormal section headers (e.g., UPX, `.vmp`, suspicious writable+executable flags).
- Suspicious Win32/POSIX API import combinations (e.g., `VirtualAllocEx` + `WriteProcessMemory` + `CreateRemoteThread`).

---

## 4. Verification & Loading

1. **Root Trust Verification**: Before mounting an `.hsv` container, the engine checks the digital signature against the trusted public key stored at `/etc/hyav/certs/root.pub` or Windows Registry `HKLM\SOFTWARE\HybridOS\HyAV\TrustedPublishers`.
2. **Integrity Validation**: SHA-256 digest of header + sections must strictly match signature payload.
3. **Mmap Ingestion**: Once verified, the database is mapped with read-only memory protections (`PAGE_READONLY` / `PROT_READ`).
