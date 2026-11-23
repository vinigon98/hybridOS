#!/usr/bin/env python3
"""
HybridOS User-Space Build & Verification Orchestrator (build_userland.py)

Compiles all Ring 3 ELF64 and PE64 binaries, verifies compliance with the
HybridOS physical memory loader specifications, and prepares distribution
artifacts in userland/dist/ ready for FAT32 / VFS injection.
"""

import os
import sys
import shutil
import struct
import subprocess
from pathlib import Path

USERLAND_DIR = Path(__file__).resolve().parent
DIST_DIR = USERLAND_DIR / "dist"
DIST_BIN = DIST_DIR / "bin"
DIST_SYS32 = DIST_DIR / "system32"

TARGET_ELF = "x86_64-unknown-none"
TARGET_PE = "x86_64-pc-windows-msvc"

def run_cmd(cmd, cwd=USERLAND_DIR):
    print(f"[BUILD] Running: {' '.join(cmd)}")
    res = subprocess.run(cmd, cwd=cwd)
    if res.returncode != 0:
        print(f"[ERROR] Command failed with exit code {res.returncode}")
        sys.exit(res.returncode)

def verify_elf(path: Path) -> dict:
    data = path.read_bytes()
    if len(data) < 64:
        raise ValueError("ELF file too small")
    if data[0:4] != b"\x7fELF":
        raise ValueError(f"Invalid ELF magic: {data[0:4]}")
    elf_class = data[4] # 2 = 64-bit
    data_enc = data[5]  # 1 = Little Endian
    machine = struct.unpack_from("<H", data, 18)[0] # 62 = x86_64
    entry = struct.unpack_from("<Q", data, 24)[0]
    phoff = struct.unpack_from("<Q", data, 32)[0]
    phentsize = struct.unpack_from("<H", data, 54)[0]
    phnum = struct.unpack_from("<H", data, 56)[0]

    return {
        "format": "ELF64",
        "size": len(data),
        "class": "64-bit" if elf_class == 2 else f"unknown ({elf_class})",
        "machine": "x86_64" if machine == 62 else f"unknown ({machine})",
        "entry": hex(entry),
        "phnum": phnum,
        "valid": (elf_class == 2 and machine == 62)
    }

def verify_pe(path: Path) -> dict:
    data = path.read_bytes()
    if len(data) < 64:
        raise ValueError("PE file too small for DOS header")
    dos_magic = struct.unpack_from("<H", data, 0)[0]
    if dos_magic != 0x5A4D: # "MZ"
        raise ValueError(f"Invalid DOS magic: 0x{dos_magic:04X}")
    e_lfanew = struct.unpack_from("<I", data, 60)[0]
    if e_lfanew + 264 > len(data):
        raise ValueError("Invalid e_lfanew offset")
    pe_sig = struct.unpack_from("<I", data, e_lfanew)[0]
    if pe_sig != 0x00004550: # "PE\0\0"
        raise ValueError(f"Invalid PE signature: 0x{pe_sig:08X}")
    
    file_hdr = e_lfanew + 4
    machine = struct.unpack_from("<H", data, file_hdr)[0] # 0x8664
    num_sections = struct.unpack_from("<H", data, file_hdr + 2)[0]
    opt_hdr = file_hdr + 20
    opt_magic = struct.unpack_from("<H", data, opt_hdr)[0] # 0x020B (PE32+)
    entry_rva = struct.unpack_from("<I", data, opt_hdr + 16)[0]
    image_base = struct.unpack_from("<Q", data, opt_hdr + 24)[0]

    is_pe64 = (machine == 0x8664 and opt_magic == 0x020B)
    return {
        "format": "PE64 / PE32+",
        "size": len(data),
        "machine": "x86_64 (0x8664)" if machine == 0x8664 else f"unknown (0x{machine:04X})",
        "opt_magic": "PE32+ (0x020B)" if opt_magic == 0x020B else f"unknown (0x{opt_magic:04X})",
        "entry_rva": hex(entry_rva),
        "image_base": hex(image_base),
        "sections": num_sections,
        "valid": is_pe64
    }

def main():
    print("======================================================================")
    print("      HybridOS User-Space Build Orchestrator (Ring 3 Ecosystem)      ")
    print("======================================================================")

    # 1. Clean and prepare dist directories
    DIST_BIN.mkdir(parents=True, exist_ok=True)
    DIST_SYS32.mkdir(parents=True, exist_ok=True)

    # 2. Build Linux ELF64 User-space targets
    print("\n--- [Phase 1/3] Building Linux ELF64 User-space Binaries ---")
    run_cmd([
        "cargo", "build",
        "--package", "hybrid_sh",
        "--package", "compositor_gui",
        "--target", TARGET_ELF,
        "--release"
    ])

    elf_src = USERLAND_DIR / "target" / TARGET_ELF / "release"
    shutil.copy2(elf_src / "hybrid_sh", DIST_BIN / "hybrid_sh.elf")
    shutil.copy2(elf_src / "hybrid_desktop", DIST_BIN / "hybrid_desktop.elf")
    print("  -> Staged hybrid_sh.elf and hybrid_desktop.elf into userland/dist/bin/")

    # 3. Build Windows PE64 User-space targets & DLLs
    print("\n--- [Phase 2/3] Building Windows NT PE64 Binaries & Core DLLs ---")
    run_cmd([
        "cargo", "build",
        "--package", "ntdll",
        "--package", "kernel32",
        "--package", "user32",
        "--package", "gdi32",
        "--package", "win_sysbench",
        "--package", "hybrid_3d_demo",
        "--target", TARGET_PE,
        "--release"
    ])

    pe_src = USERLAND_DIR / "target" / TARGET_PE / "release"
    pe_binaries = ["win_sysbench.exe", "hybrid_3d_demo.exe"]
    pe_dlls = ["ntdll.dll", "kernel32.dll", "user32.dll", "gdi32.dll"]

    for b in pe_binaries:
        shutil.copy2(pe_src / b, DIST_BIN / b)
        print(f"  -> Staged {b} into userland/dist/bin/")

    for d in pe_dlls:
        shutil.copy2(pe_src / d, DIST_BIN / d)
        shutil.copy2(pe_src / d, DIST_SYS32 / d)
        print(f"  -> Staged {d} into userland/dist/bin/ and userland/dist/system32/")

    # 4. Verify all generated artifacts against HybridOS kernel specifications
    print("\n--- [Phase 3/3] Validating Binary Formats & Kernel Loader ABI ---")
    artifacts = [
        ("hybrid_sh.elf", "ELF"),
        ("hybrid_desktop.elf", "ELF"),
        ("win_sysbench.exe", "PE"),
        ("hybrid_3d_demo.exe", "PE"),
        ("ntdll.dll", "PE"),
        ("kernel32.dll", "PE"),
        ("user32.dll", "PE"),
        ("gdi32.dll", "PE"),
    ]

    print("\n" + "=" * 80)
    print(f"{'Artifact':<22} | {'Format':<12} | {'Size':<10} | {'Entry / RVA':<18} | {'Status'}")
    print("-" * 80)

    all_passed = True
    for fname, ftype in artifacts:
        fpath = DIST_BIN / fname
        if not fpath.exists():
            print(f"{fname:<22} | MISSING!")
            all_passed = False
            continue

        if ftype == "ELF":
            info = verify_elf(fpath)
            status = "PASS (Valid ELF64)" if info["valid"] else "FAIL"
            print(f"{fname:<22} | {info['format']:<12} | {info['size']:>8} B | {info['entry']:<18} | {status}")
            if not info["valid"]:
                all_passed = False
        else:
            info = verify_pe(fpath)
            status = "PASS (Valid PE64)" if info["valid"] else "FAIL"
            print(f"{fname:<22} | {info['format']:<12} | {info['size']:>8} B | {info['entry_rva']:<18} | {status}")
            if not info["valid"]:
                all_passed = False

    print("=" * 80)
    if all_passed:
        print("\n[SUCCESS] 100% of User-Space Ring 3 Artifacts Compiled & Verified!")
        print(f"Artifacts ready for deployment in: {DIST_DIR}")
    else:
        print("\n[ERROR] One or more artifacts failed validation!")
        sys.exit(1)

if __name__ == "__main__":
    main()
