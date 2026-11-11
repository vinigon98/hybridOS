# HyAV Trust & Verification Model (`TRUST.md`)

## 1. Overview
HybridOS runs binaries from both the Windows NT ecosystem and the Linux POSIX ecosystem. The **HyAV Trust Model** establishes unified cryptographic assurance and identity verification across heterogeneous execution environments.

---

## 2. Trust Levels

Every inspected binary or library is assigned an authoritative `TrustLevel`:

| Level | Identifier | Criteria | Policy Treatment |
|---|---|---|---|
| **0** | `KnownMalicious` | Exact hash match in blacklist or verified malware signature. | Immediate `BLOCK` or `QUARANTINE`. |
| **1** | `Untrusted` | Unsigned binary, invalid certificate chain, or self-signed cert. | Subjected to full heuristic engine and deep inspection. |
| **2** | `Neutral` | Standard application binary, unsigned or commercial, passed scan clean. | Standard access allowed, logged. |
| **3** | `TrustedVendor` | Valid digital signature chained to known trusted root CA or vendor keyring. | Fast-path cached execution permitted. |
| **4** | `SystemCore` | Signed by HybridOS Base Authority or protected kernel system vendor. | Bypass deep inspection; immutable file protections. |

---

## 3. Subsystem Trust Verification

```
                      Binary Ingestion
                             │
              ┌──────────────┴──────────────┐
              ▼                             ▼
       [PE/COFF Binary]              [ELF/Script Binary]
     (Windows Subsystem)              (Linux Subsystem)
              │                             │
    [Authenticode Check]           [IMA / GPG Keyring]
    - PKCS#7 Embedded Sig          - Extended Attributes (security.ima)
    - Catalog Files (.cat)         - RPM/dpkg Package Signatures
    - Certificate Chain            - Module Signatures (X.509)
              │                             │
              └──────────────┬──────────────┘
                             ▼
              [HybridOS Native Trust Bridge]
              - Cross-personality Certificate Store
              - Ed25519 System Root Authority
              - Revocation Evaluation (CRL / Hash DB)
```

### 3.1 Windows Subsystem: Authenticode
- **Embedded Signatures**: Parses PE header Data Directory (`IMAGE_DIRECTORY_ENTRY_SECURITY`), extracts `WIN_CERTIFICATE`, verifies PKCS#7 structure.
- **Catalog Verification**: For unsigned binaries with external catalogs (Windows system files), queries system catalog database (`catroot2`).
- **Chain of Trust**: Validated against Microsoft Trusted Root Certification Authorities and HybridOS Trusted Root Store.

### 3.2 Linux Subsystem: IMA & Package Keyrings
- **Linux IMA (Integrity Measurement Architecture)**: Reads `security.ima` extended attribute and verifies hash or digital signature against kernel keyring (`.builtin_trusted_keys`).
- **Kernel Modules**: Strict enforcement of `CONFIG_MODULE_SIG_FORCE` style verification for `.ko` files loaded via `init_module` / `finit_module`.
- **System Package Keyring**: Verification against system GPG keyrings (`/etc/pki/` or `/usr/share/keyrings/`).

### 3.3 HybridOS Native Authority
- Cryptographic anchors for HybridOS hybrid binaries and services utilize **Ed25519** public keys embedded directly in kernel memory at compile time.
- Guarantees immunity against user-mode certificate store corruption or injection attacks.

---

## 4. Revocation & Dynamic Invalidation

- **Hash Revocation DB**: Fast-lookup binary search list of revoked leaf certificate serial numbers and file hashes.
- **Revocation Cache TTL**: Certificate revocation evaluations are cached with a maximum TTL of 24 hours.
- **File Mutation Reaction**: Any write handle or atomic replacement immediately revokes trust cache for the corresponding inode/file ID.
