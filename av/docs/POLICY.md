# HyAV Policy Engine Specification (`POLICY.md`)

## 1. Overview
The **HyAV Policy Engine** (`hyav-core::policy`) defines the decision matrix evaluated by the VFS Gate and Worker services. It maps inspection results, trust evaluations, file origins, and process capabilities into concrete security enforcement actions.

---

## 2. Enforcement Modes

The system operates under one of three global modes:

| Mode | Behavior | Use Case |
|---|---|---|
| `AuditMode` | All operations allowed; detections recorded to telemetry. | Baseline analysis, policy staging. |
| `Enforcing` | Detections blocked or quarantined according to rule severity. | Standard production runtime. |
| `StrictLockdown` | Default-Deny. Only cryptographically trusted system binaries run. | High-security or kiosk appliances. |

---

## 3. Decision Matrix & Action Hierarchy

When an event triggers inspection, the Policy Engine resolves verdicts in descending priority order:

```
  ┌───────────────────────────────────────────────┐
  │  Level 1: System-Critical Bypass / Allowlist   │ ──► ALLOW (Non-overrideable)
  └──────────────────────┬────────────────────────┘
                         │ Pass
  ┌──────────────────────▼────────────────────────┐
  │  Level 2: Administrator Path / Hash Exclusion │ ──► ALLOW
  └──────────────────────┬────────────────────────┘
                         │ Pass
  ┌──────────────────────▼────────────────────────┐
  │  Level 3: Cryptographic Trust (Authenticode/IMA)│ ──► ALLOW (If valid signature)
  └──────────────────────┬────────────────────────┘
                         │ Pass / Untrusted
  ┌──────────────────────▼────────────────────────┐
  │  Level 4: Signature Vault Match (Malware DB)   │ ──► BLOCK / QUARANTINE
  └──────────────────────┬────────────────────────┘
                         │ Clean
  ┌──────────────────────▼────────────────────────┐
  │  Level 5: Heuristic & Behavioral Thresholds   │ ──► ACTION BY SCORE
  └──────────────────────┬────────────────────────┘
                         │ Below threshold
  ┌──────────────────────▼────────────────────────┐
  │  Level 6: Default Subsystem Fallback          │ ──► ALLOW (In Enforcing)
  └───────────────────────────────────────────────┘
```

---

## 4. Policy Actions

- **`ALLOW`**: Permits VFS request to proceed. Sets cache entry with TTL.
- **`BLOCK`**: Denies operation. Returns `STATUS_ACCESS_DENIED` (Windows) or `-EPERM` (Linux).
- **`QUARANTINE`**:
  - Atomically moves target file to secure quarantine vault (`/var/hyav/quarantine` or `C:\System\HyAV\Quarantine`).
  - Replaces file with 0-byte tombstone or generates access denial.
  - Strips all execute and read permissions (`chmod 000` / Deny ACL).
  - Encrypts target payload using AES-256-GCM.
- **`REMEDIATE`**:
  - Executes `QUARANTINE` on target.
  - Terminates offending process tree (`SIGKILL` or `ZwTerminateProcess`).
  - Emits critical telemetry alert.
- **`ALERT_ONLY`**: Generates high-priority telemetry notice without interrupting execution.

---

## 5. Configuration Format (`policy.toml`)

```toml
[engine]
mode = "Enforcing" # "AuditMode" | "Enforcing" | "StrictLockdown"
scan_timeout_ms = 2000
fail_strategy = "FailClosed" # "FailOpen" | "FailClosed"

[exclusions]
paths = [
    "/proc/*",
    "/sys/*",
    "\\Device\\HarddiskVolume*\\Windows\\System32\\ntoskrnl.exe",
    "\\Device\\HarddiskVolume*\\Windows\\System32\\hal.dll",
]
hashes = [
    # SHA-256 trusted deployment hashes
]

[rules]
quarantine_severity_threshold = "High"
block_untrusted_drivers = true
heuristic_sensitivity = "Medium" # "Low" | "Medium" | "Aggressive"
isolate_suspicious_scripts = true
```
