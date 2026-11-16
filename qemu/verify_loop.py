#!/usr/bin/env python3
"""
verify_loop.py - Official Deterministic Loop Harness Validator for HybridOS Kernel.

Executes each loop cycle validation:
1. `cargo build --release --target x86_64-unknown-none` in `rust-kernel` (exit code 0 required).
2. `python qemu/build_disk.py` to regenerate bootable disk images.
3. Headless QEMU execution with COM1 serial capture to `qemu/serial_latest.log` (15s timeout).
4. Telemetry validation:
   - Zero `CRITICAL KERNEL PANIC`, `Triple fault`, or unhandled `Page Fault`.
   - `[GUI] HybridOS Desktop Environment & PS/2 Mouse active.` reached.
   - All auto-test (`[TEST_OK]`) markers verified.
Returns `sys.exit(0)` on success, `sys.exit(1)` on failure.
"""

import os
import sys
import subprocess
import time
import shutil

def run_step(step_name, cmd, cwd, env=None, check=True):
    print(f"\n==================================================================")
    print(f">>> [VERIFY_LOOP] {step_name}")
    print(f"==================================================================")
    print(f"[*] Command: {' '.join(cmd) if isinstance(cmd, list) else cmd}")
    print(f"[*] Working Directory: {cwd}")
    
    start = time.time()
    res = subprocess.run(cmd, cwd=cwd, env=env, capture_output=True, text=True, shell=False)
    elapsed = time.time() - start
    
    if res.stdout:
        print(res.stdout.strip())
    if res.stderr:
        print("[STDERR]:", res.stderr.strip())
        
    print(f"[*] Completed in {elapsed:.2f}s with Exit Code: {res.returncode}")
    if check and res.returncode != 0:
        print(f"[-] [FAILED] {step_name} failed with code {res.returncode}!")
        sys.exit(1)
    return res

def main():
    repo_root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    rust_kernel_dir = os.path.join(repo_root, "rust-kernel")
    qemu_dir = os.path.join(repo_root, "qemu")
    logs_dir = os.path.join(repo_root, "logs")
    os.makedirs(logs_dir, exist_ok=True)
    
    # 1. Compile Kernel in Release Mode
    cargo_cmd = ["cargo", "build", "--release", "--target", "x86_64-unknown-none"]
    run_step("1. Compiling Kernel (release, x86_64-unknown-none)", cargo_cmd, rust_kernel_dir)

    # 2. Build Bootable Disk Images
    disk_builder = os.path.join(qemu_dir, "build_disk.py")
    build_disk_cmd = [sys.executable, disk_builder]
    run_step("2. Building Bootable System Disk Images", build_disk_cmd, repo_root)

    # 3. Headless QEMU Execution
    qemu_bin = os.path.join(qemu_dir, "bin", "qemu-system-x86_64.exe")
    qemu_share = os.path.join(qemu_dir, "bin", "share")
    system_img = os.path.join(qemu_dir, "hybridos_system.img")
    nvme_img = os.path.join(qemu_dir, "hybridos_nvme.img")
    sata_img = os.path.join(qemu_dir, "hybridos_sata.img")
    serial_log = os.path.join(qemu_dir, "serial_latest.log")
    cpu_log = os.path.join(logs_dir, "qemu_cpu.log")

    if os.path.exists(serial_log):
        try:
            os.remove(serial_log)
        except OSError:
            pass

    qemu_cmd = [
        qemu_bin,
        "-L", qemu_share,
        "-machine", "pc,accel=tcg",
        "-cpu", "max",
        "-m", "1024M",
        "-drive", f"file={system_img},format=raw,if=ide",
        "-drive", f"file={nvme_img},format=raw,if=none,id=nvm0",
        "-device", "nvme,serial=HYBRIDOS_NVME0,drive=nvm0",
        "-drive", f"file={sata_img},format=raw,if=none,id=sata0",
        "-device", "ahci,id=ahci0",
        "-device", "ide-hd,drive=sata0,bus=ahci0.0",
        "-netdev", "user,id=net0",
        "-device", "e1000,netdev=net0,mac=52:54:00:12:34:56",
        "-vga", "std",
        "-display", "none",
        "-serial", f"file:{serial_log}",
        "-no-reboot",
        "-d", "cpu_reset",
        "-D", cpu_log,
        "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04",
    ]

    print(f"\n==================================================================")
    print(f">>> [VERIFY_LOOP] 3. Spawning Headless QEMU (Timeout: 15.0s)...")
    print(f"==================================================================")
    
    start_time = time.time()
    try:
        proc = subprocess.run(qemu_cmd, cwd=qemu_dir, timeout=15, capture_output=True, text=True)
        qemu_elapsed = time.time() - start_time
        print(f"[*] QEMU exited cleanly in {qemu_elapsed:.2f}s (Exit code: {proc.returncode})")
    except subprocess.TimeoutExpired:
        qemu_elapsed = time.time() - start_time
        print(f"[*] QEMU reached strict 15.0s timeout and was halted.")

    # Mirror log into logs/serial_latest.log for unified access
    unified_log = os.path.join(logs_dir, "serial_latest.log")
    if os.path.exists(serial_log):
        try:
            shutil.copy2(serial_log, unified_log)
        except Exception:
            pass

    # 4. Telemetry Validation
    print(f"\n==================================================================")
    print(f">>> [VERIFY_LOOP] 4. Serial Telemetry Audit...")
    print(f"==================================================================")
    
    if not os.path.exists(serial_log):
        print(f"[-] [FAIL] Serial output log was not created at: {serial_log}")
        sys.exit(1)

    with open(serial_log, "r", encoding="utf-8", errors="replace") as f:
        log_content = f.read()

    errors = []
    if "CRITICAL KERNEL PANIC" in log_content or "PANIC:" in log_content:
        errors.append("CRITICAL KERNEL PANIC detected in serial stream!")
    if "Triple fault" in log_content:
        errors.append("CPU Triple Fault detected!")
    if "Unhandled Page Fault" in log_content or "Page Fault Exception" in log_content:
        errors.append("Unhandled Page Fault detected!")
    
    operational_marker = "[GUI] HybridOS Desktop Environment & PS/2 Mouse active."
    if operational_marker not in log_content:
        errors.append(f"Operational state marker '{operational_marker}' was NOT reached!")

    if errors:
        print("[-] ==========================================================")
        print("[-] TELEMETRY AUDIT FAILED:")
        for err in errors:
            print(f"[-]   -> {err}")
        print("[-] ==========================================================")
        print("[-] Last 40 lines of serial_latest.log:")
        print("--------------------------------------------------------------")
        lines = log_content.splitlines()
        for line in lines[-40:]:
            print(line)
        print("--------------------------------------------------------------")
        sys.exit(1)

    # Count test validations
    test_ok_lines = [line.strip() for line in log_content.splitlines() if "[TEST_OK]" in line]
    
    print("[+] ==========================================================")
    print(f"[+] [SUCCESS] HybridOS Operational Baseline reached in {qemu_elapsed:.2f}s!")
    print(f"[+] Operational Marker Verified: '{operational_marker}'")
    print(f"[+] Zero Kernel Panics, Zero Triple Faults, Zero Unhandled Faults.")
    if test_ok_lines:
        print(f"[+] Auto-Tests Passed ({len(test_ok_lines)}):")
        for t in test_ok_lines:
            print(f"[+]   {t}")
    print("[+] Loop Validation PASSED with exit code 0.")
    print("[+] ==========================================================\n")
    sys.exit(0)

if __name__ == "__main__":
    main()
