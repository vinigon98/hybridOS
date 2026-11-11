# HyAV Scan Reason Specification (`SCANREASON.md`)

## 1. Overview
The `ScanReason` contract defines every event or condition that causes HyAV to inspect a filesystem object or memory buffer. It guarantees deterministic handling, priority scheduling, and cache invalidation policies across HybridOS subsystems.

---

## 2. Enumeration & Value Taxonomy

| Code (u16) | Identifier | Execution Subsystem | Mode | Priority | Description |
|---|---|---|---|---|---|
| `0x0001` | `EXEC_PE` | Windows | Synchronous | Real-Time (P0) | Executing a PE32/PE32+ executable (`.exe`, `.scr`, `.com`). |
| `0x0002` | `LOAD_DLL` | Windows | Synchronous | Real-Time (P0) | Dynamic library load via `NtCreateSection` or `LdrLoadDll`. |
| `0x0003` | `EXEC_ELF` | Linux | Synchronous | Real-Time (P0) | Execution of an ELF binary via `sys_execve` / `sys_execveat`. |
| `0x0004` | `LOAD_SO` | Linux | Synchronous | High (P1) | Shared object loaded dynamically (`dlopen`). |
| `0x0005` | `EXEC_SCRIPT` | Both | Synchronous | High (P1) | Script runner invocation (e.g., `bash`, `cmd.exe`, `powershell`). |
| `0x0010` | `FILE_CREATE_CLOSE` | Both | Asynchronous | Normal (P2) | File closed after write or modification (`vfs_close` dirty flag set). |
| `0x0011` | `FILE_RENAME_EXT` | Both | Asynchronous | Normal (P2) | File renamed into an executable or risky extension. |
| `0x0020` | `MEM_INJECT_DETECT`| Both | Synchronous | Real-Time (P0) | Memory region marked `PAGE_EXECUTE_READWRITE` / `PROT_EXEC`. |
| `0x0030` | `DRIVER_LOAD` | Both | Synchronous | Critical (P0) | Loading a kernel module (`.ko`) or NT driver (`.sys`). |
| `0x0100` | `ON_DEMAND_PATH` | Both | Asynchronous | Normal (P2) | User or administrator directed manual scan of path or file. |
| `0x0101` | `SCHEDULED_SCAN` | Both | Background | Low (P3) | Background maintenance sweep of system files and storage. |
| `0x0102` | `REMEDIATION_RES` | Both | Synchronous | High (P1) | Verification scan following signature database update. |

---

## 3. Reason Characteristics & Caching Rules

### 3.1 `EXEC_PE` / `EXEC_ELF` (Execution Gates)
- **Synchronous Blocking**: Calling thread suspended until `GateResponse` is returned.
- **Cache Strategy**: Result cached under inode + mtime + cryptographic hash. Cache lifetime is invalidated immediately if file is opened for writing.
- **Fail Policy**: Strictly enforced according to `POLICY.md`.

### 3.2 `FILE_CREATE_CLOSE` (Persistence Interception)
- **Asynchronous Queue**: Non-blocking for the writing process. File is placed in verification queue.
- **Marked State**: If verification detects malware, file is converted into quarantined tombstone before subsequent execution.

### 3.3 `DRIVER_LOAD` (Kernel Integrity Gate)
- **Zero-Tolerance Policy**: Must pass full Authenticode / IMA / HyAV signature checks.
- **No Cache Bypass**: Drivers must be fully inspected on every load attempt.

---

## 4. Metadata Envelope

Every scan reason is accompanied by context metadata:

```rust
pub struct ScanContext {
    pub reason: ScanReason,
    pub caller_pid: u32,
    pub caller_sid_uid: u64,
    pub parent_pid: u32,
    pub target_file_size: u64,
    pub target_vfs_path: String,
    pub entropy_hint: Option<f32>,
    pub client_session_id: u32,
}
```
