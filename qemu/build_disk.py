"""
HybridOS Real Bare-Metal Boot Disk Generator
Builds bootable FAT32/MBR disk image for Limine Bootloader and HybridOS 64-bit Kernel ELF,
plus virtual NVMe and SATA AHCI drives for QEMU x86_64 emulation.
"""

import os
import struct
import subprocess
import sys

def make_fat32_image(disk_path, files):
    disk_size = 64 * 1024 * 1024  # 64 MB
    part_start_lba = 2048
    part_sectors = (disk_size // 512) - part_start_lba
    
    sec_per_clus = 1
    rsvd_sec = 32
    num_fats = 2
    
    data_sectors = part_sectors - rsvd_sec
    est_clusters = data_sectors // sec_per_clus
    fat_size = ((est_clusters + 2) * 4 + 511) // 512
    data_sectors = part_sectors - rsvd_sec - (num_fats * fat_size)
    total_clusters = data_sectors // sec_per_clus
    
    disk = bytearray(disk_size)
    
    # MBR Partition Table
    mbr_part1 = struct.pack('<BBBBBBBBII',
        0x80,             # Bootable flag (active)
        0x00, 0x02, 0x00, # Start CHS
        0x0C,             # Partition Type: FAT32 LBA
        0xFF, 0xFF, 0xFF, # End CHS
        part_start_lba,   # Start LBA: 2048 (1MB offset)
        part_sectors      # Sector Count
    )
    disk[0x1BE:0x1BE+16] = mbr_part1
    disk[510] = 0x55
    disk[511] = 0xAA
    
    # Volume Boot Record (VBR)
    vbr_offset = part_start_lba * 512
    vbr = bytearray(512)
    vbr[0:3] = b'\xeb\x58\x90'
    vbr[3:11] = b'MSWIN4.1'
    struct.pack_into('<H', vbr, 11, 512)           # bytes per sector
    vbr[13] = sec_per_clus                         # sectors per cluster
    struct.pack_into('<H', vbr, 14, rsvd_sec)      # reserved sectors
    vbr[16] = num_fats                             # number of FATs
    vbr[21] = 0xF8                                 # media descriptor
    struct.pack_into('<H', vbr, 24, 63)            # sectors per track
    struct.pack_into('<H', vbr, 26, 255)           # heads
    struct.pack_into('<I', vbr, 28, part_start_lba)# hidden sectors
    struct.pack_into('<I', vbr, 32, part_sectors)  # total sectors
    struct.pack_into('<I', vbr, 36, fat_size)      # FAT size in sectors
    struct.pack_into('<I', vbr, 44, 2)             # root cluster = 2
    struct.pack_into('<H', vbr, 48, 1)             # FSInfo sector = 1
    struct.pack_into('<H', vbr, 50, 6)             # backup boot sector = 6
    vbr[64] = 0x80                                 # drive number
    vbr[66] = 0x29                                 # boot signature
    struct.pack_into('<I', vbr, 67, 0x12345678)    # volume ID
    vbr[71:82] = b'HYBRIDOS   '                    # volume label
    vbr[82:90] = b'FAT32   '                       # filesystem type
    vbr[510] = 0x55
    vbr[511] = 0xAA
    disk[vbr_offset:vbr_offset+512] = vbr
    disk[vbr_offset + 6*512 : vbr_offset + 7*512] = vbr # Backup VBR
    
    # FSInfo Sector
    fsinfo = bytearray(512)
    struct.pack_into('<I', fsinfo, 0, 0x41615252)
    struct.pack_into('<I', fsinfo, 484, 0x61417272)
    struct.pack_into('<I', fsinfo, 488, total_clusters - 1)
    struct.pack_into('<I', fsinfo, 492, 3)
    struct.pack_into('<I', fsinfo, 508, 0xAA550000)
    disk[vbr_offset + 512 : vbr_offset + 1024] = fsinfo
    
    # FAT Structures
    fat1_offset = vbr_offset + rsvd_sec * 512
    fat2_offset = fat1_offset + fat_size * 512
    struct.pack_into('<I', disk, fat1_offset + 0, 0x0FFFFFF8) # Media descriptor
    struct.pack_into('<I', disk, fat1_offset + 4, 0x0FFFFFFF) # EOC marker
    struct.pack_into('<I', disk, fat1_offset + 8, 0x0FFFFFFF) # Root directory EOC
    
    clus_size = sec_per_clus * 512
    data_start = vbr_offset + (rsvd_sec + num_fats * fat_size) * 512
    
    def lfn_checksum(short_name):
        chk = 0
        for b in short_name:
            chk = (((chk & 1) << 7) + (chk >> 1) + b) & 0xFF
        return chk

    def make_lfn_entries(long_name, short_name):
        chk = lfn_checksum(short_name)
        chars = [ord(c) for c in long_name] + [0]
        while len(chars) % 13 != 0:
            chars.append(0xFFFF)
        entries = []
        num_entries = len(chars) // 13
        for i in range(num_entries):
            seq = i + 1
            if i == num_entries - 1:
                seq |= 0x40
            chunk = chars[i*13 : (i+1)*13]
            entry = bytearray(32)
            entry[0] = seq
            for j in range(5):
                struct.pack_into('<H', entry, 1 + j*2, chunk[j])
            entry[11] = 0x0F
            entry[12] = 0x00
            entry[13] = chk
            for j in range(6):
                struct.pack_into('<H', entry, 14 + j*2, chunk[5 + j])
            struct.pack_into('<H', entry, 26, 0)
            for j in range(2):
                struct.pack_into('<H', entry, 28 + j*2, chunk[11 + j])
            entries.append(entry)
        entries.reverse()
        return entries

    next_free_clus = 3
    root_entries = bytearray()
    
    for l_name, s_name_str, f_data in files:
        s_name = s_name_str.encode('ascii')
        needed = (len(f_data) + clus_size - 1) // clus_size
        start_clus = next_free_clus
        
        for i in range(needed):
            c_num = start_clus + i
            nxt = 0x0FFFFFFF if i == needed - 1 else c_num + 1
            struct.pack_into('<I', disk, fat1_offset + c_num * 4, nxt)
            
            chunk = f_data[i * clus_size : (i + 1) * clus_size]
            c_off = data_start + (c_num - 2) * clus_size
            disk[c_off : c_off + len(chunk)] = chunk
            
        next_free_clus += needed
        
        lfns = make_lfn_entries(l_name, s_name)
        for lfn in lfns:
            root_entries += lfn
            
        d_entry = bytearray(32)
        d_entry[0:11] = s_name
        d_entry[11] = 0x20
        struct.pack_into('<H', d_entry, 20, (start_clus >> 16) & 0xFFFF)
        struct.pack_into('<H', d_entry, 26, start_clus & 0xFFFF)
        struct.pack_into('<I', d_entry, 28, len(f_data))
        root_entries += d_entry
        
    root_off = data_start
    disk[root_off : root_off + len(root_entries)] = root_entries
    disk[fat2_offset : fat2_offset + fat_size * 512] = disk[fat1_offset : fat1_offset + fat_size * 512]
    
    with open(disk_path, 'wb') as f:
        f.write(disk)
    print(f"[OK] Written FAT32 image: {disk_path} ({len(disk) // (1024*1024)} MB)")

def create_images(out_dir):
    script_dir = os.path.dirname(os.path.abspath(__file__))
    project_root = os.path.dirname(script_dir)
    os.makedirs(out_dir, exist_ok=True)

    kernel_bin_path = os.path.join(project_root, "rust-kernel", "target", "x86_64-unknown-none", "release", "hybridos-kernel")
    if not os.path.exists(kernel_bin_path):
        # Fallback path if release not built yet
        kernel_bin_path = os.path.join(project_root, "rust-kernel", "target", "x86_64-unknown-none", "debug", "hybridos-kernel")
    assert os.path.exists(kernel_bin_path), f"Kernel binary not found at: {kernel_bin_path}. Run cargo build first!"
    
    with open(kernel_bin_path, "rb") as f:
        kernel_data = f.read()
    print(f"[+] Loaded Kernel ELF64: {kernel_bin_path} ({len(kernel_data)} bytes)")

    limine_bios_sys = os.path.join(project_root, "boot", "limine", "limine-binary", "limine-bios.sys")
    with open(limine_bios_sys, "rb") as f:
        bios_sys_data = f.read()

    limine_bootx64 = os.path.join(project_root, "boot", "limine", "limine-binary", "BOOTX64.EFI")
    with open(limine_bootx64, "rb") as f:
        bootx64_data = f.read()

    limine_conf_data = (
        b"timeout: 0\n"
        b"verbose: yes\n\n"
        b"/HybridOS\n"
        b"    protocol: limine\n"
        b"    kernel_path: boot():/hybridos-kernel\n"
    )

    files_to_install = [
        ('limine-bios.sys', 'LIMINE-BSYS', bios_sys_data),
        ('limine.conf',     'LIMINE  CNF', limine_conf_data),
        ('hybridos-kernel', 'HYBRID~1   ', kernel_data),
        ('BOOTX64.EFI',     'BOOTX64 EFI', bootx64_data),
    ]

    # 1. Build Bootable System Disk (64 MB)
    system_img_path = os.path.join(out_dir, "hybridos_system.img")
    make_fat32_image(system_img_path, files_to_install)

    # 2. Run Limine BIOS installation onto MBR
    limine_exe = os.path.join(project_root, "boot", "limine", "limine-binary", "limine-tool-windows-x86", "limine.exe")
    res = subprocess.run([limine_exe, "bios-install", system_img_path], capture_output=True, text=True)
    if res.returncode == 0:
        print(f"[OK] Limine BIOS MBR Stage 1 & Stage 2 successfully installed to {system_img_path}")
    else:
        print(f"[!] Warning: limine bios-install output: {res.stdout} {res.stderr}")

    # 3. Create Virtual NVMe Drive (64 MB)
    nvme_path = os.path.join(out_dir, "hybridos_nvme.img")
    print(f"[*] Creating 64MB Virtual NVMe Drive: {nvme_path}...")
    with open(nvme_path, "wb") as f:
        f.seek(64 * 1024 * 1024 - 1)
        f.write(b'\x00')
    print(f"[OK] Generated Virtual NVMe Drive: {nvme_path}")

    # 4. Create Virtual SATA AHCI Drive (32 MB)
    sata_path = os.path.join(out_dir, "hybridos_sata.img")
    print(f"[*] Creating 32MB Virtual SATA Drive: {sata_path}...")
    with open(sata_path, "wb") as f:
        f.seek(32 * 1024 * 1024 - 1)
        f.write(b'\x00')
    print(f"[OK] Generated Virtual SATA Drive: {sata_path}")

if __name__ == "__main__":
    out_dir = os.path.dirname(os.path.abspath(__file__))
    create_images(out_dir)
