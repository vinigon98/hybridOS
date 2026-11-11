# HyAV VFS Gate Specification (`GATE.md`)

## 1. Overview
The **HyAV VFS Gate** (`hyav-vfs-gate`) is the kernel-level interception and mediation layer within HybridOS. It hooks virtual filesystem (VFS) operations across both the **Windows NT personality** and the **Linux POSIX personality**, enforcing real-time security inspection before file access, execution, or persistence is permitted.

```
       [Windows Subsystem]               [Linux Subsystem]
      (NtCreateFile/Section)             (sys_openat/sys_execve)
               │                                   │
               └───────────────┬───────────────────┘
                               ▼
                   ┌───────────────────────┐
                   │     HyAV VFS Gate     │
                   │  (Kernel Fast-Path)   │
                   └───────────┬───────────┘
                               │
            ┌──────────────────┴──────────────────┐
            ▼                                     ▼
     [Trust Cache Hit]                     [Cache Miss]
     (Instant Allow)                              │
                                                  ▼
                                       ┌─────────────────────┐
                                       │    Shared Ring/IPC  │
                                       └──────────┬──────────┘
                                                  ▼
                                       ┌─────────────────────┐
                                       │     HyAV Worker     │
                                       │    (User/Kernel)    │
                                       └─────────────────────┘
```

---

## 2. Interception Points

The Gate intercepts operations at distinct execution phases:

| Personality | Native API / Syscall | Interception Hook | Trigger Semantic |
|---|---|---|---|
| **Windows** | `NtCreateFile`, `NtOpenFile` | `HyAvPreCreateFile` | Open with write access, delete access, or execute |
| **Windows** | `NtCreateSection` (`SEC_IMAGE`) | `HyAvPreCreateSection` | PE binary image mapping into process address space |
| **Linux** | `sys_execve`, `sys_execveat` | `HyAvPreExecve` | ELF or script execution attempt |
| **Linux** | `sys_openat`, `sys_open` | `HyAvPreOpen` | File open with `O_CREAT`, `O_TRUNC`, or `O_WRONLY` |
| **Both** | File Close (`vfs_close`) | `HyAvPostCloseWrite` | Asynchronous scan when dirty modified file is committed |

---

## 3. Architecture & Execution Paths

### 3.1 Fast-Path (In-Kernel)
1. **Bypass Checks**: Kernel internal threads, paging files, and critical bootstrap binaries (`ntoskrnl`, `vmlinux`, `init`, `smss.exe`) bypass the gate.
2. **Trust / Clean Cache Lookup**:
   - Key: `(DeviceID, Inode/FileIndex, ModificationTimestamp, FileSize, Hash)`.
   - If marked `TRUST_VALID` and hash matches, return `GATE_ALLOW` immediately (< 250ns latency).
3. **Blacklist Bloom Filter**: Fast rejection of known malicious SHA-256 prefixes.

### 3.2 Slow-Path (Synchronous / Asynchronous Evaluation)
1. If fast-path misses, the thread is held in a synchronous wait state (for `EXEC` and `OPEN_WRITE` hooks).
2. A `GateRequest` structure is submitted to the lockless shared memory ring buffer (`hyav_gate_ring`).
3. Workers (`hyav-worker`) dequeue requests, conduct inspection, and commit a `GateResponse`.
4. The calling thread is resumed with the verdict (`ALLOW` or `DENY`).

---

## 4. IPC & Data Structures

### 4.1 Request Packet (`GateRequest`)
```rust
#[repr(C)]
pub struct GateRequest {
    pub request_id: u64,
    pub timestamp_ns: u64,
    pub subsystem: u8,          // 0 = Windows, 1 = Linux
    pub hook_type: u8,          // 0 = PreExec, 1 = PreOpen, 2 = PostClose
    pub scan_reason: u16,       // Corresponds to SCANREASON.md
    pub pid: u32,
    pub tid: u32,
    pub uid_sid: [u8; 16],      // Linux UID/GID or Windows Token SID
    pub file_handle: u64,
    pub file_size: u64,
    pub path_len: u16,
    pub path_utf8: [u8; 512],
}
```

### 4.2 Response Packet (`GateResponse`)
```rust
#[repr(C)]
pub struct GateResponse {
    pub request_id: u64,
    pub verdict: u8,            // 0 = Allow, 1 = Block, 2 = Quarantine
    pub cache_ttl_secs: u32,
    pub reason_code: u16,
    pub detection_id: [u8; 32], // Threat signature identifier
}
```

---

## 5. Failure Modes & Safety Boundaries

- **Timeout Handling**: Configurable default (2000 ms). If worker pool times out:
  - `Fail-Closed` (Enforcing security profile): Deny access, return `STATUS_ACCESS_DENIED` / `-EACCES`.
  - `Fail-Open` (High-availability profile): Allow access, log emergency telemetry event.
- **Deadlock Prevention**:
  - The gate never initiates I/O against the volume holding the target file from within the hook context.
  - Workers use direct kernel memory buffers or dedicated non-intercepted virtual mounts.
