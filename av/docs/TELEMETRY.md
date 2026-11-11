# HyAV Telemetry & Audit Specification (`TELEMETRY.md`)

## 1. Overview
The **HyAV Telemetry Subsystem** (`hyav-service::telemetry`) provides end-to-end auditability, threat detection logging, and SIEM event streaming for HybridOS security activities across Windows and Linux subsystems.

---

## 2. Event Types & Taxonomy

| Event Name | Event Code | Severity | Description |
|---|---|---|---|
| `GateIntercept` | `0x1001` | Info | VFS operation intercepted by kernel gate. |
| `ScanCompleted` | `0x1002` | Info | Scan completed clean with verdict `ALLOW`. |
| `DetectionHit` | `0x2001` | Alert | Malicious signature or suspicious heuristic triggered. |
| `PolicyEnforced`| `0x2002` | Warning | Access denied, process blocked, or execution halted. |
| `QuarantineExec`| `0x3001` | Critical | Target file moved to encrypted quarantine vault. |
| `TrustViolation`| `0x3002` | High | Signature chain invalid, revoked cert, or tamper detected. |
| `EngineError` | `0x4001` | Error | Worker timeout, ring buffer overflow, or parsing failure. |

---

## 3. Unified Event Schema

Events are emitted in a structured format:

```json
{
  "event_id": "7b0b2e81-2856-42f8-958b-c6b758da4b2a",
  "timestamp": "2026-11-10T12:00:00.123456Z",
  "event_code": "0x2001",
  "event_name": "DetectionHit",
  "subsystem": "WINDOWS_NT",
  "actor": {
    "pid": 4128,
    "ppid": 1044,
    "tid": 512,
    "user_identity": "S-1-5-21-3623811015-3361044348-30300820-1013",
    "process_path": "\\Device\\HarddiskVolume2\\Windows\\System32\\cmd.exe",
    "command_line": "cmd.exe /c payload.exe"
  },
  "target": {
    "path": "C:\\Users\\Vinicius\\Downloads\\payload.exe",
    "vfs_mount": "/mnt/c",
    "file_size": 204800,
    "sha256": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    "blake3": "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"
  },
  "detection": {
    "engine": "hyav-sigformat",
    "rule_id": "HSV-WIN-TROJAN-EMOTET-104",
    "threat_name": "Trojan.Win32.Emotet.Heur",
    "confidence_score": 98,
    "heuristic_flags": ["HIGH_ENTROPY", "SUSPICIOUS_IMPORTS"]
  },
  "enforcement": {
    "mode": "Enforcing",
    "action": "QUARANTINE",
    "remediation_status": "Success",
    "quarantine_vault_id": "Q-20261110-001"
  }
}
```

---

## 4. Ingestion & Export Channels

1. **Kernel Log Ring**: Low-latency lock-free ring buffer accessible via device node:
   - Linux: `/dev/hyav_telemetry`
   - Windows: `\\.\Device\HyAVTelemetry`
2. **Platform Bridges**:
   - **Windows**: Dispatches directly into **ETW (Event Tracing for Windows)** under provider GUID `{D42F9920-569A-4B6A-8A71-893EF7915B32}`.
   - **Linux**: Dispatches structured audit events to `auditd` netlink socket and `/var/log/hyav/audit.jsonl`.
3. **Tamper-Resistant Local Ledger**:
   - Sequential events are linked using cryptographic hash chains (`PrevHash` -> `CurrentHash`), ensuring log tampering or truncation is immediately detectable.

---

## 5. Rate Limiting & Performance Guardrails

- **Log De-duplication**: Repeated identical intercept hits within 1 second window are collapsed into count aggregates.
- **Ring Overflow Handling**: If telemetry consumer falls behind, events are prioritized by severity:
  - Critical/Alert events (`DetectionHit`, `QuarantineExec`) are never dropped.
  - Informational events (`GateIntercept`) drop with an `OVERFLOW_DROPPED_COUNT` counter indicator.
