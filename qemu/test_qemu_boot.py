"""
Automated QEMU Boot Test for HybridOS
Runs QEMU headless with serial/console connected,
verifies the boot sequence and sends commands to the HybridShell.
"""

import subprocess
import time
import sys
import os
import re

def test_qemu():
    qemu_dir = r"C:\Users\Vinicius\Documents\HybridOS-Kernel\qemu"
    qemu_bin = os.path.join(qemu_dir, "bin", "qemu-system-x86_64.exe")
    qemu_share = os.path.join(qemu_dir, "bin", "share")
    system_img = os.path.join(qemu_dir, "hybridos_system.img")
    nvme_img = os.path.join(qemu_dir, "hybridos_nvme.img")
    sata_img = os.path.join(qemu_dir, "hybridos_sata.img")

    cmd = [
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
        "-nographic"
    ]

    print(">>> [TEST] Starting QEMU with HybridOS Kernel...")
    
    proc = subprocess.Popen(
        cmd,
        cwd=qemu_dir,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        bufsize=0
    )

    # Allow kernel to boot and print banner
    time.sleep(2.0)

    # Commands to execute in HybridShell
    test_commands = [
        "ver\r",
        "vol\r",
        "df\r",
        "ps\r",
        "powershell\r",
        "cmd\r",
        "bash\r",
        "hybrid\r",
        "exit\r"
    ]

    for c in test_commands:
        try:
            proc.stdin.write(c)
            proc.stdin.flush()
            time.sleep(0.4)
        except Exception as e:
            print(f"[!] Error sending '{c.strip()}': {e}")
            break

    try:
        stdout, stderr = proc.communicate(timeout=8)
    except subprocess.TimeoutExpired:
        proc.kill()
        stdout, stderr = proc.communicate()

    # De-duplicate characters (caused by QEMU -nographic multiplexing VGA + COM1 to stdio)
    def clean_multiplex(s):
        # Collapse doubled characters (e.g., 'HHyybbrriiddOOSS' -> 'HybridOS')
        cleaned = re.sub(r'(.)\1', r'\1', s)
        return cleaned

    cleaned_stdout = clean_multiplex(stdout)

    print("\n=================== QEMU SERIAL OUTPUT (CLEAN) ===================")
    print(cleaned_stdout[:3000])
    if len(cleaned_stdout) > 3000:
        print(f"... [{len(cleaned_stdout) - 3000} bytes truncated] ...")
        print(cleaned_stdout[-1500:])
    print("==================================================================")
    if stderr:
        print("[STDERR]:", stderr)

    # Assertions
    assert "HybridOS" in cleaned_stdout or "HybridOS" in stdout or "HHyybbrriiddOOSS" in stdout, "Kernel banner missing from QEMU output!"
    assert "Vortex Driver Model" in cleaned_stdout or "VVoorrtteexx" in stdout, "VDM initialization missing!"
    assert "Dual-ABI" in cleaned_stdout or "DDuuaall--AABBII" in stdout, "Dual-ABI missing!"
    print("\n>>> [TEST SUCCESS] HybridOS Kernel successfully booted and validated in QEMU x86_64!")

if __name__ == "__main__":
    test_qemu()
