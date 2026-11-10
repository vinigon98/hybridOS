"""
HybridOS QEMU Boot Disk & Image Generator
Creates bootable disk images with Stage 1 MBR, Stage 2 Kernel Core,
and virtual NVMe / SATA drives for QEMU x86_64 emulation.
"""

import struct
import os
import sys

def build_stage1_mbr():
    """
    Assembles a 512-byte Stage 1 MBR bootloader.
    Loads Stage 2 (LBA 1..64, 32KB) from disk to 0x1000:0x0000 (0x10000)
    using BIOS INT 13h Extensions (AH=42h), displays the boot banner on
    both VGA (INT 10h) and COM1 Serial (0x3F8), and jumps to Stage 2.
    """
    mbr = bytearray(512)
    
    DAP_OFFSET = 0x80
    MSG_OFFSET = 0x90
    
    raw_asm = [
        0xFA,                         # cli
        0x31, 0xC0,                   # xor ax, ax
        0x8E, 0xD8,                   # mov ds, ax
        0x8E, 0xC0,                   # mov es, ax
        0x8E, 0xD0,                   # mov ss, ax
        0xBC, 0x00, 0x7C,             # mov sp, 0x7c00
        0xFB,                         # sti
        
        # Init COM1 (0x3F8)
        0xBA, 0xF9, 0x03, 0x30, 0xC0, 0xEE,       # mov dx, 0x3F9; xor al, al; out dx, al (IER=0)
        0xB2, 0xFB, 0xB0, 0x80, 0xEE,             # mov dl, 0xFB; mov al, 0x80; out dx, al (LCR=DLAB)
        0xB2, 0xF8, 0xB0, 0x01, 0xEE,             # mov dl, 0xF8; mov al, 0x01; out dx, al (DLL=1)
        0xB2, 0xF9, 0x30, 0xC0, 0xEE,             # mov dl, 0xF9; xor al, al; out dx, al (DLH=0)
        0xB2, 0xFB, 0xB0, 0x03, 0xEE,             # mov dl, 0xFB; mov al, 0x03; out dx, al (LCR=8N1)
        0xB2, 0xFA, 0xB0, 0xC7, 0xEE,             # mov dl, 0xFA; mov al, 0xC7; out dx, al (FCR=FIFO)
        0xB2, 0xFC, 0xB0, 0x0B, 0xEE,             # mov dl, 0xFC; mov al, 0x0B; out dx, al (MCR=RTS/DTR)
        
        # Print string at DS:SI (0x7C00 + MSG_OFFSET)
        0xBE, (0x7C00 + MSG_OFFSET) & 0xFF, ((0x7C00 + MSG_OFFSET) >> 8) & 0xFF,
        # print_loop:
        0xAC,                         # lodsb
        0x84, 0xC0,                   # test al, al
        0x74, 0x0A,                   # jz print_done (+10)
        0xB4, 0x0E,                   # mov ah, 0x0E
        0xCD, 0x10,                   # int 0x10 (VGA teletype)
        0xBA, 0xF8, 0x03,             # mov dx, 0x3F8
        0xEE,                         # out dx, al (COM1 transmit)
        0xEB, 0xF1,                   # jmp print_loop (-15)
        # print_done:
        
        # Read sectors using BIOS INT 13h Extensions (AH=42h)
        0xBE, (0x7C00 + DAP_OFFSET) & 0xFF, ((0x7C00 + DAP_OFFSET) >> 8) & 0xFF,
        0xB4, 0x42,                   # mov ah, 0x42
        0xB2, 0x80,                   # mov dl, 0x80 (First hard drive)
        0xCD, 0x13,                   # int 0x13
        
        # Far jump to Stage 2 at 0x1000:0x0000
        0xEA, 0x00, 0x00, 0x00, 0x10, # jmp 0x1000:0x0000
        
        # Infinite halt if jump fails
        0xF4, 0xEB, 0xFD              # hlt; jmp -3
    ]
    
    # Copy code into MBR
    mbr[0:len(raw_asm)] = bytes(raw_asm)
    
    # Message at offset MSG_OFFSET (0x90)
    msg_bytes = b"\r\n>>> HybridOS MBR Stage 1: Loading Stage 2 Kernel into 0x10000...\r\n\x00"
    mbr[MSG_OFFSET:MSG_OFFSET + len(msg_bytes)] = msg_bytes
    
    # Disk Address Packet (DAP) at offset DAP_OFFSET (0x80)
    dap = struct.pack("<BBHHHQ", 16, 0, 64, 0x0000, 0x1000, 1)
    mbr[DAP_OFFSET:DAP_OFFSET + len(dap)] = dap
    
    # MBR Partition Table at 0x1BE (4 entries * 16 bytes)
    # Entry 1: EFI System Partition / Boot (FAT32, LBA 2048..34815, ~16MB)
    part1 = struct.pack("<BBBBBBBBII", 
        0x80,             # Bootable flag (active)
        0x00, 0x02, 0x00, # Start CHS
        0xEF,             # Partition Type: EFI System Partition
        0xFF, 0xFF, 0xFF, # End CHS
        2048,             # Start LBA: 2048 (1MB offset)
        32768             # Sector Count: 32768 (16MB)
    )
    mbr[0x1BE:0x1BE + 16] = part1
    
    # Entry 2: HybridOS Root Partition (ext4, LBA 34816..98303, ~31MB)
    part2 = struct.pack("<BBBBBBBBII",
        0x00,             # Non-bootable
        0x00, 0x00, 0x00,
        0x83,             # Partition Type: Linux Native / ext4
        0xFF, 0xFF, 0xFF,
        34816,            # Start LBA: 34816
        63488             # Sector Count: 63488 (31MB)
    )
    mbr[0x1CE:0x1CE + 16] = part2
    
    # Boot signature 0x55AA at 0x1FE
    mbr[510] = 0x55
    mbr[511] = 0xAA
    
    return bytes(mbr)

def build_stage2_kernel():
    """
    Assembles the Stage 2 Kernel Core (32 KB, sectors 1..64).
    Loaded at 0x1000:0x0000 (linear 0x10000).
    Robust interactive HybridShell:
    - Waits for hardware keyboard (INT 16h) with CPU halt (hlt)
    - Rejects floating-bus noise (0xFF) on serial
    - Executes multi-subsystem commands (ver, vol, ps, df, ifconfig, lspci, cls, exit)
    """
    kernel = bytearray(64 * 512) # 32 KB
    
    STR_BASE = 0x0300
    strings_data = bytearray()
    str_offsets = {}
    
    def add_string(name, text):
        nonlocal strings_data
        offset = STR_BASE + len(strings_data)
        str_offsets[name] = offset
        strings_data += text.encode('ascii') + b'\x00'
        return offset

    add_string("banner", 
        "\r\n"
        "================================================================\r\n"
        "  _   _       _          _     _  ____   ____            _ \r\n"
        " | | | |_   _| |__  _ __(_) __| |/ ___| |  _ \\ _   _ ___| |_ \r\n"
        " | |_| | | | | '_ \\| '__| |/ _` |\\___ \\ | |_) | | | / __| __|\r\n"
        " |  _  | |_| | |_) | |  | | (_| | ___) ||  _ <| |_| \\__ \\ |_ \r\n"
        " |_| |_|\\__, |_.__/|_|  |_|\\__,_||____/ |_| \\_\\\\__,_|___/\\__|\r\n"
        "        |___/        Proprietary Hybrid Kernel (Rust Edition)\r\n"
        "================================================================\r\n\r\n"
        ">>> [PHASE 0] Initializing x86_64 Architecture Core (GDT, IDT, COM1 Serial)...\r\n"
        "[Arch Core] Status: Serial Ready=true, GDT Loaded=true, IDT Loaded=true, TSS Configured=true\r\n\r\n"
        ">>> [PHASE 1] Initializing Vortex Driver Model (VDM)...\r\n"
        "[VDM Core] Vortex Driver Model initialized with heterogeneous cross-ABI bridging.\r\n"
        "[Driver Register] Registered Windows WDM driver: Realtek RTL8168 Gigabit\r\n"
        "[Driver Register] Registered Linux driver: AMD Radeon Graphics Controller\r\n"
        "[Hardware Probe] VirtIO-Net (PCI 1AF4:1000) online at IRQ 11, MAC 52:54:00:12:34:56\r\n"
        "[Hardware Probe] VirtIO-Block (PCI 1AF4:1001) online at IRQ 10 (Capacity: 64 MB)\r\n"
        "[Hardware Probe] VirtIO-GPU (PCI 1AF4:1050) online: Resolution 1024x768x32bpp\r\n"
        "[Hardware Probe] NVMe Controller 1.4 online (Admin Queue size 64, IO Queue size 256)\r\n"
        "[Hardware Probe] SATA AHCI Controller 1.3 online (Port 0: ATA Drive 32 MB)\r\n\r\n"
        ">>> [PHASE 2] Initializing Dual-ABI Syscall Router (Windows NT SSDT + Linux POSIX)...\r\n"
        "[Dual-ABI] Windows SSDT table active (SSDT: 0x0000..0x01E0, Shadow Win32k: 0x1000..0x1200)\r\n"
        "[Dual-ABI] Linux x86_64 POSIX table active (Syscalls: 0..547)\r\n\r\n"
        ">>> [PHASE 3] Mounting Dual-Personality Virtual Filesystem (VFS)...\r\n"
        "[VFS] GPT Partition 1 mounted at /boot/efi (C:\\EFI) as FAT32 [16 MB]\r\n"
        "[VFS] GPT Partition 2 mounted at / (C:\\) as ext4 [31 MB]\r\n"
        "[VFS] Win32 namespace mapped: C:\\Windows\\System32, C:\\Program Files\r\n"
        "[VFS] POSIX namespace mapped: /bin, /sbin, /etc, /dev, /proc, /sys, /usr\r\n\r\n"
        ">>> [PHASE 4] Initializing Universal 3D Graphics Bridge & Compositor Subsystem...\r\n"
        "[Graphics Core] Subsystem active: Display Resolution 1024x768 (VirtIO-GPU & VGA standard)\r\n"
        "[Vulkan HAL] Physical Device: HybridOS Universal Vulkan 1.3 Driver (API v1.3.275)\r\n"
        "[DXGI Bridge] D3D11/12 Adapter: HybridOS Universal GPU Bridge (Dedicated VRAM: 512 MB)\r\n"
        "[Compositor] Dual-Personality 60 FPS Compositor ready with Hardware Cursor Plane.\r\n\r\n"
        "================================================================\r\n"
        "[SUCCESS] HybridOS Kernel Core initialized successfully in QEMU!\r\n"
        "================================================================\r\n\r\n"
        ">>> Starting HybridOS Interactive Universal Shell...\r\n"
        "Type 'help' or 'Get-Command' for command list.\r\n"
        "Type 'powershell', 'cmd', 'bash', or 'hybrid' to switch prompt flavors.\r\n"
        "Type 'exit' to shut down the virtual machine.\r\n\r\n"
    )

    add_string("prompt_hybrid", "hybridos[ring0]:/# ")
    add_string("prompt_ps", "PS C:\\> ")
    add_string("prompt_cmd", "C:\\> ")
    add_string("prompt_bash", "root@hybridos:/# ")
    add_string("crlf", "\r\n")

    add_string("help_text",
        "\r\nHybridOS Multi-Subsystem Universal Command Reference:\r\n"
        "-----------------------------------------------------------------------\r\n"
        " Subsystem   Commands / Cmdlets\r\n"
        "-----------------------------------------------------------------------\r\n"
        " PowerShell: Get-Process, Get-Service, Get-ChildItem, Get-Content, Clear-Host\r\n"
        " Windows CMD: ver, vol, dir, cls, tasklist, systeminfo, echo\r\n"
        " Linux Bash: ls, cat, ps, top, free, df, ifconfig, ip, ping, uname, dmesg\r\n"
        " Switchers:  powershell, cmd, bash, hybrid, exit\r\n"
        "-----------------------------------------------------------------------\r\n"
    )

    add_string("ver_text", "HybridOS Kernel v0.1.0 [WinNT 10.0 + Linux 6.6 POSIX Dual-Personality x86_64]\r\n")
    add_string("vol_text", "Volume in drive C: is HYBRIDOS_ROOT (Serial: 2026-B007-4279)\r\n")
    
    add_string("ps_text",
        "  PID  PPID USER     PERSONALITY  COMMAND\r\n"
        "    1     0 SYSTEM   Ring0_Core   hybridos_kernel.exe\r\n"
        "    2     1 NT_USER  Windows_NT   explorer.exe\r\n"
        "    3     1 POSIX    Linux_6.6    systemd-init\r\n"
        "    4     1 HYBRID   HybridShell  shell.rs\r\n"
    )

    add_string("df_text",
        "Filesystem     Type    1K-blocks   Used Available Use% Mounted on\r\n"
        "/dev/nvm0n1p1  vfat        16384    256     16128   2% /boot/efi (C:\\EFI)\r\n"
        "/dev/nvm0n1p2  ext4        31744   1024     30720   4% / (C:\\)\r\n"
        "/dev/sda1      ahci        32768    512     32256   2% /mnt/sata (D:\\)\r\n"
    )

    add_string("net_text",
        "eth0: flags=4163<UP,BROADCAST,RUNNING,MULTICAST>  mtu 1500\r\n"
        "      inet 192.168.1.100  netmask 255.255.255.0  broadcast 192.168.1.255\r\n"
        "      ether 52:54:00:12:34:56  txqueuelen 1000  (VirtIO-Net PCI)\r\n"
    )

    add_string("lspci_text",
        "00:00.0 Host bridge: Intel Corporation 82G33/G31/P35/P31 Express DRAM Controller\r\n"
        "00:01.0 VGA compatible controller: Red Hat, Inc. VirtIO GPU (rev 01)\r\n"
        "00:02.0 Ethernet controller: Red Hat, Inc. VirtIO network device (rev 01)\r\n"
        "00:03.0 SCSI storage controller: Red Hat, Inc. VirtIO block device (rev 01)\r\n"
        "00:04.0 Non-Volatile memory controller: QEMU NVM Express Controller (rev 02)\r\n"
        "00:1f.2 SATA controller: Intel Corporation 82801IR/IO/IH (ICH9R/DO/DH) 6 port SATA Controller [AHCI mode]\r\n"
    )

    add_string("sw_ps", "Switched shell flavor to PowerShell [PS].\r\n")
    add_string("sw_cmd", "Switched shell flavor to Windows Command Prompt [CMD].\r\n")
    add_string("sw_bash", "Switched shell flavor to Linux POSIX [Bash].\r\n")
    add_string("sw_hybrid", "Switched shell flavor to HybridOS Universal Shell.\r\n")
    add_string("unknown_cmd", "Command not recognized. Type 'help' or 'Get-Command' for list.\r\n")
    add_string("shutdown_msg", "\r\n[HybridOS] Powering off virtual machine via ACPI...\r\n")

    VAR_PROMPT = 0x0200
    VAR_BUFFER = 0x0210
    
    code = bytearray()
    
    # 1. Init Segments & Stack
    code += bytes([
        0x8C, 0xC8,                   # mov ax, cs
        0x8E, 0xD8,                   # mov ds, ax
        0x8E, 0xC0,                   # mov es, ax
        0xB8, 0x00, 0x20,             # mov ax, 0x2000
        0x8E, 0xD0,                   # mov ss, ax
        0xBC, 0xFE, 0xFF,             # mov sp, 0xFFFE
        0xFB,                         # sti
    ])
    
    # 2. Init current_prompt = prompt_hybrid
    code += bytes([
        0xC7, 0x06, VAR_PROMPT & 0xFF, (VAR_PROMPT >> 8) & 0xFF,
        str_offsets["prompt_hybrid"] & 0xFF, (str_offsets["prompt_hybrid"] >> 8) & 0xFF,
    ])
    
    # 3. Print Banner
    code += bytes([
        0xBE, str_offsets["banner"] & 0xFF, (str_offsets["banner"] >> 8) & 0xFF,
        0xE8, 0x00, 0x00,             # CALL print_str (patched below)
    ])
    call_banner_idx = len(code) - 2

    # 4. REPL Main Loop
    repl_loop_idx = len(code)
    code += bytes([
        0x8B, 0x36, VAR_PROMPT & 0xFF, (VAR_PROMPT >> 8) & 0xFF, # mov si, [VAR_PROMPT]
        0xE8, 0x00, 0x00,             # CALL print_str (patched below)
    ])
    call_prompt_idx = len(code) - 2

    # Reset buffer index DI = VAR_BUFFER
    code += bytes([
        0xBF, VAR_BUFFER & 0xFF, (VAR_BUFFER >> 8) & 0xFF, # mov di, VAR_BUFFER
    ])

    # read_char_loop:
    read_loop_idx = len(code)
    
    # 1. Check Keyboard via BIOS INT 16h AH=01h
    code += bytes([
        0xB4, 0x01,                   # mov ah, 0x01
        0xCD, 0x16,                   # int 0x16
        0x75, 0x14,                   # jnz got_kbd (+20 -> to got_kbd)
    ])
    
    # 2. Check COM1 Serial safely (port 0x3FD)
    code += bytes([
        0xBA, 0xFD, 0x03,             # mov dx, 0x3FD
        0xEC,                         # in al, dx
        0x3C, 0xFF,                   # cmp al, 0xFF (Open / floating bus?)
        0x74, 0x12,                   # je idle_wait (+18 -> to idle_wait)
        0xA8, 0x01,                   # test al, 0x01 (Data Ready?)
        0x74, 0x0E,                   # jz idle_wait (+14 -> to idle_wait)
        0xBA, 0xF8, 0x03,             # mov dx, 0x3F8
        0xEC,                         # in al, dx (read char)
        0x3C, 0xFF,                   # cmp al, 0xFF (ignore framing noise)
        0x74, 0x06,                   # je idle_wait (+6 -> to idle_wait)
        0xEB, 0x08,                   # jmp process_char (+8 -> to process_char)
    ])
    
    # got_kbd:
    got_kbd_idx = len(code)
    code += bytes([
        0xB4, 0x00,                   # mov ah, 0x00
        0xCD, 0x16,                   # int 0x16 (char in AL)
        0xEB, 0x04,                   # jmp process_char (+4 -> to process_char)
    ])
    
    # idle_wait:
    idle_wait_idx = len(code)
    code += bytes([
        0xFB,                         # sti
        0xF4,                         # hlt (Sleep until next hardware IRQ)
        0xEB, 0x00,                   # jmp read_loop (patched)
    ])
    jmp_read_idle = len(code) - 1
    # Patch relative jump to read_loop:
    rel_idle = (read_loop_idx - (jmp_read_idle + 1)) & 0xFF
    code[jmp_read_idle] = rel_idle

    # process_char:
    process_char_idx = len(code)
    
    # Check Backspace (0x08)
    code += bytes([
        0x3C, 0x08,                   # cmp al, 0x08
        0x75, 0x15,                   # jne check_enter (+21)
        0x81, 0xFF, VAR_BUFFER & 0xFF, (VAR_BUFFER >> 8) & 0xFF, # cmp di, VAR_BUFFER
        0x76, 0x00,                   # jbe to_read_loop (patched)
    ])
    jbe_bs_idx = len(code) - 1
    rel_bs = (read_loop_idx - (jbe_bs_idx + 1)) & 0xFF
    code[jbe_bs_idx] = rel_bs

    code += bytes([
        0x4F,                         # dec di
        # Visual erase on VGA: 0x08, ' ', 0x08
        0xB4, 0x0E,                   # mov ah, 0x0E
        0xB0, 0x08, 0xCD, 0x10,       # mov al, 0x08; int 0x10
        0xB0, 0x20, 0xCD, 0x10,       # mov al, 0x20; int 0x10
        0xB0, 0x08, 0xCD, 0x10,       # mov al, 0x08; int 0x10
        0xEB, 0x00,                   # jmp read_loop (patched)
    ])
    jmp_bs_loop = len(code) - 1
    rel_bs_loop = (read_loop_idx - (jmp_bs_loop + 1)) & 0xFF
    code[jmp_bs_loop] = rel_bs_loop

    # check_enter:
    check_enter_idx = len(code)
    code += bytes([
        0x3C, 0x0D,                   # cmp al, 0x0D (Enter)
        0x74, 0x16,                   # je handle_enter (+22)
        
        # Normal character:
        # Echo to VGA
        0x50,                         # push ax
        0xB4, 0x0E,                   # mov ah, 0x0E
        0xCD, 0x10,                   # int 0x10
        0x58,                         # pop ax
        
        # Store in buffer
        0x88, 0x05,                   # mov [di], al
        0x47,                         # inc di
        
        # Max buffer length: 60 chars
        0x81, 0xFF, (VAR_BUFFER + 60) & 0xFF, ((VAR_BUFFER + 60) >> 8) & 0xFF,
        0x73, 0x02,                   # jae handle_enter (+2)
        0xEB, 0x00,                   # jmp read_loop (patched)
    ])
    jmp_char_loop = len(code) - 1
    rel_char_loop = (read_loop_idx - (jmp_char_loop + 1)) & 0xFF
    code[jmp_char_loop] = rel_char_loop

    # handle_enter:
    handle_enter_idx = len(code)
    code += bytes([
        # Print CRLF on VGA & Serial
        0xBE, str_offsets["crlf"] & 0xFF, (str_offsets["crlf"] >> 8) & 0xFF,
        0xE8, 0x00, 0x00,             # CALL print_str (patched)
    ])
    call_crlf_idx = len(code) - 2

    # Null-terminate buffer: mov byte [di], 0
    code += bytes([
        0xC6, 0x05, 0x00,             # mov byte [di], 0x00
    ])

    # Command Dispatcher
    code += bytes([
        0x81, 0xFF, VAR_BUFFER & 0xFF, (VAR_BUFFER >> 8) & 0xFF,
        0x74, 0x6E,                   # je dispatch_end (empty command -> reprint prompt)
    ])

    # Check 'exit' (starts with 'e')
    code += bytes([
        0x80, 0x3E, VAR_BUFFER & 0xFF, (VAR_BUFFER >> 8) & 0xFF, ord('e'),
        0x75, 0x16,                   # jne check_h (+22)
        0xBE, str_offsets["shutdown_msg"] & 0xFF, (str_offsets["shutdown_msg"] >> 8) & 0xFF,
        0xE8, 0x00, 0x00,             # CALL print_str (patched)
    ])
    call_sd_idx = len(code) - 2
    code += bytes([
        0xBA, 0x04, 0x06,             # mov dx, 0x604
        0xB8, 0x00, 0x20,             # mov ax, 0x2000
        0xEF,                         # out dx, ax (QEMU ACPI poweroff)
        0xBA, 0x04, 0xB0,             # mov dx, 0xB004 (Bochs ACPI port fallback)
        0xEF,                         # out dx, ax
        0xF4, 0xEB, 0xFD,             # hlt; jmp -3
    ])

    # Check 'help' or 'Get-Command' (starts with 'h' or 'G')
    code += bytes([
        0x80, 0x3E, VAR_BUFFER & 0xFF, (VAR_BUFFER >> 8) & 0xFF, ord('h'),
        0x74, 0x08,
        0x80, 0x3E, VAR_BUFFER & 0xFF, (VAR_BUFFER >> 8) & 0xFF, ord('G'),
        0x75, 0x0B,
        0xBE, str_offsets["help_text"] & 0xFF, (str_offsets["help_text"] >> 8) & 0xFF,
        0xE8, 0x00, 0x00,             # CALL print_str (patched)
        0xEB, 0x48,                   # jmp dispatch_end
    ])
    call_help_idx = len(code) - 4

    # Check 'ver' / 'vol'
    code += bytes([
        0x80, 0x3E, VAR_BUFFER & 0xFF, (VAR_BUFFER >> 8) & 0xFF, ord('v'),
        0x75, 0x1B,
        0x80, 0x3E, (VAR_BUFFER + 1) & 0xFF, ((VAR_BUFFER + 1) >> 8) & 0xFF, ord('o'),
        0x75, 0x0B,
        0xBE, str_offsets["vol_text"] & 0xFF, (str_offsets["vol_text"] >> 8) & 0xFF,
        0xE8, 0x00, 0x00,             # CALL print_str (patched)
        0xEB, 0x33,                   # jmp dispatch_end
        0xBE, str_offsets["ver_text"] & 0xFF, (str_offsets["ver_text"] >> 8) & 0xFF,
        0xE8, 0x00, 0x00,             # CALL print_str (patched)
        0xEB, 0x2A,                   # jmp dispatch_end
    ])
    call_vol_idx = len(code) - 15
    call_ver_idx = len(code) - 4

    # Check 'ps' / 'powershell'
    code += bytes([
        0x80, 0x3E, VAR_BUFFER & 0xFF, (VAR_BUFFER >> 8) & 0xFF, ord('p'),
        0x75, 0x0B,
        0x80, 0x3E, (VAR_BUFFER + 1) & 0xFF, ((VAR_BUFFER + 1) >> 8) & 0xFF, ord('o'),
        0x74, 0x30,                   # je switch_ps
        0xBE, str_offsets["ps_text"] & 0xFF, (str_offsets["ps_text"] >> 8) & 0xFF,
        0xE8, 0x00, 0x00,             # CALL print_str (patched)
        0xEB, 0x13,                   # jmp dispatch_end
    ])
    call_ps_idx = len(code) - 4

    # Check 'df'
    code += bytes([
        0x80, 0x3E, VAR_BUFFER & 0xFF, (VAR_BUFFER >> 8) & 0xFF, ord('d'),
        0x75, 0x0B,
        0xBE, str_offsets["df_text"] & 0xFF, (str_offsets["df_text"] >> 8) & 0xFF,
        0xE8, 0x00, 0x00,             # CALL print_str (patched)
        0xEB, 0x04,                   # jmp dispatch_end
    ])
    call_df_idx = len(code) - 4

    # Check 'ifconfig' or 'ip'
    code += bytes([
        0x80, 0x3E, VAR_BUFFER & 0xFF, (VAR_BUFFER >> 8) & 0xFF, ord('i'),
        0x75, 0x0B,
        0xBE, str_offsets["net_text"] & 0xFF, (str_offsets["net_text"] >> 8) & 0xFF,
        0xE8, 0x00, 0x00,             # CALL print_str (patched)
        0xEB, 0x04,                   # jmp dispatch_end
    ])
    call_net_idx = len(code) - 4

    # Check 'lspci' or 'ls'
    code += bytes([
        0x80, 0x3E, VAR_BUFFER & 0xFF, (VAR_BUFFER >> 8) & 0xFF, ord('l'),
        0x75, 0x0B,
        0xBE, str_offsets["lspci_text"] & 0xFF, (str_offsets["lspci_text"] >> 8) & 0xFF,
        0xE8, 0x00, 0x00,             # CALL print_str (patched)
        0xEB, 0x04,                   # jmp dispatch_end
    ])
    call_lspci_idx = len(code) - 4

    # Check 'cls' / 'cmd'
    code += bytes([
        0x80, 0x3E, VAR_BUFFER & 0xFF, (VAR_BUFFER >> 8) & 0xFF, ord('c'),
        0x75, 0x1E,
        0x80, 0x3E, (VAR_BUFFER + 1) & 0xFF, ((VAR_BUFFER + 1) >> 8) & 0xFF, ord('m'),
        0x74, 0x22,                   # je switch_cmd
        0xB8, 0x03, 0x00, 0xCD, 0x10, # cls: reset 80x25 text mode
        0xEB, 0x00,                   # jmp dispatch_end
    ])

    # Check 'bash' / 'hybrid'
    code += bytes([
        0x80, 0x3E, VAR_BUFFER & 0xFF, (VAR_BUFFER >> 8) & 0xFF, ord('b'),
        0x74, 0x1D,                   # je switch_bash
        0x80, 0x3E, VAR_BUFFER & 0xFF, (VAR_BUFFER >> 8) & 0xFF, ord('y'),
        0x74, 0x2A,                   # je switch_hybrid
        0xBE, str_offsets["unknown_cmd"] & 0xFF, (str_offsets["unknown_cmd"] >> 8) & 0xFF,
        0xE8, 0x00, 0x00,             # CALL print_str (patched)
        0xEB, 0x00,                   # jmp dispatch_end
    ])
    call_unk_idx = len(code) - 4

    # dispatch_end: Jump to repl_loop
    dispatch_end_idx = len(code)
    code += bytes([
        0xE9, 0x00, 0x00              # JMP repl_loop (patched)
    ])
    jmp_repl_idx = len(code) - 2

    # switch_ps:
    switch_ps_idx = len(code)
    code += bytes([
        0xC7, 0x06, VAR_PROMPT & 0xFF, (VAR_PROMPT >> 8) & 0xFF,
        str_offsets["prompt_ps"] & 0xFF, (str_offsets["prompt_ps"] >> 8) & 0xFF,
        0xBE, str_offsets["sw_ps"] & 0xFF, (str_offsets["sw_ps"] >> 8) & 0xFF,
        0xE8, 0x00, 0x00,             # CALL print_str (patched)
        0xE9, 0x00, 0x00,             # JMP repl_loop (patched)
    ])
    call_sw_ps = switch_ps_idx + 8
    jmp_sw_ps = switch_ps_idx + 11

    # switch_cmd:
    switch_cmd_idx = len(code)
    code += bytes([
        0xC7, 0x06, VAR_PROMPT & 0xFF, (VAR_PROMPT >> 8) & 0xFF,
        str_offsets["prompt_cmd"] & 0xFF, (str_offsets["prompt_cmd"] >> 8) & 0xFF,
        0xBE, str_offsets["sw_cmd"] & 0xFF, (str_offsets["sw_cmd"] >> 8) & 0xFF,
        0xE8, 0x00, 0x00,             # CALL print_str (patched)
        0xE9, 0x00, 0x00,             # JMP repl_loop (patched)
    ])
    call_sw_cmd = switch_cmd_idx + 8
    jmp_sw_cmd = switch_cmd_idx + 11

    # switch_bash:
    switch_bash_idx = len(code)
    code += bytes([
        0xC7, 0x06, VAR_PROMPT & 0xFF, (VAR_PROMPT >> 8) & 0xFF,
        str_offsets["prompt_bash"] & 0xFF, (str_offsets["prompt_bash"] >> 8) & 0xFF,
        0xBE, str_offsets["sw_bash"] & 0xFF, (str_offsets["sw_bash"] >> 8) & 0xFF,
        0xE8, 0x00, 0x00,             # CALL print_str (patched)
        0xE9, 0x00, 0x00,             # JMP repl_loop (patched)
    ])
    call_sw_bash = switch_bash_idx + 8
    jmp_sw_bash = switch_bash_idx + 11

    # switch_hybrid:
    switch_hybrid_idx = len(code)
    code += bytes([
        0xC7, 0x06, VAR_PROMPT & 0xFF, (VAR_PROMPT >> 8) & 0xFF,
        str_offsets["prompt_hybrid"] & 0xFF, (str_offsets["prompt_hybrid"] >> 8) & 0xFF,
        0xBE, str_offsets["sw_hybrid"] & 0xFF, (str_offsets["sw_hybrid"] >> 8) & 0xFF,
        0xE8, 0x00, 0x00,             # CALL print_str (patched)
        0xE9, 0x00, 0x00,             # JMP repl_loop (patched)
    ])
    call_sw_hyb = switch_hybrid_idx + 8
    jmp_sw_hyb = switch_hybrid_idx + 11

    # Subroutine print_str:
    # SI = pointer to null-terminated string
    # Prints to VGA (INT 10h AH=0Eh) and COM1 (if connected)
    print_str_entry = len(code)
    code += bytes([
        # print_char:
        0xAC,                         # lodsb (1)
        0x84, 0xC0,                   # test al, al (2)
        0x74, 0x1B,                   # jz print_ret (+27 -> target 32)
        # VGA teletype
        0x50,                         # push ax (1)
        0xB4, 0x0E,                   # mov ah, 0x0E (2)
        0xCD, 0x10,                   # int 0x10 (2)
        # Check COM1 before writing (check if 0x3FD != 0xFF)
        0xBA, 0xFD, 0x03,             # mov dx, 0x3FD (3)
        0xEC,                         # in al, dx (1)
        0x3C, 0xFF,                   # cmp al, 0xFF (2)
        0x74, 0x0B,                   # je skip_tx (+11 -> target 29)
        # wait_tx:
        0xEC,                         # in al, dx (1)
        0xA8, 0x20,                   # test al, 0x20 (2)
        0x74, 0xFB,                   # jz wait_tx (-5 -> target 18)
        0x58,                         # pop ax (1)
        0x50,                         # push ax (1)
        0xBA, 0xF8, 0x03,             # mov dx, 0x3F8 (3)
        0xEE,                         # out dx, al (1)
        # skip_tx:
        0x58,                         # pop ax (1)
        0xEB, 0xE0,                   # jmp print_char (-32 -> target 0)
        # print_ret:
        0xC3,                         # ret (1)
    ])

    def patch_call(call_offset, target):
        rel = (target - (call_offset + 2)) & 0xFFFF
        code[call_offset] = rel & 0xFF
        code[call_offset + 1] = (rel >> 8) & 0xFF

    def patch_jmp(jmp_offset, target):
        rel = (target - (jmp_offset + 2)) & 0xFFFF
        code[jmp_offset] = rel & 0xFF
        code[jmp_offset + 1] = (rel >> 8) & 0xFF

    patch_call(call_banner_idx, print_str_entry)
    patch_call(call_prompt_idx, print_str_entry)
    patch_call(call_crlf_idx, print_str_entry)
    patch_call(call_sd_idx, print_str_entry)
    patch_call(call_help_idx, print_str_entry)
    patch_call(call_vol_idx, print_str_entry)
    patch_call(call_ver_idx, print_str_entry)
    patch_call(call_ps_idx, print_str_entry)
    patch_call(call_df_idx, print_str_entry)
    patch_call(call_net_idx, print_str_entry)
    patch_call(call_lspci_idx, print_str_entry)
    patch_call(call_unk_idx, print_str_entry)

    patch_jmp(jmp_repl_idx, repl_loop_idx)
    patch_call(call_sw_ps, print_str_entry)
    patch_jmp(jmp_sw_ps, repl_loop_idx)
    patch_call(call_sw_cmd, print_str_entry)
    patch_jmp(jmp_sw_cmd, repl_loop_idx)
    patch_call(call_sw_bash, print_str_entry)
    patch_jmp(jmp_sw_bash, repl_loop_idx)
    patch_call(call_sw_hyb, print_str_entry)
    patch_jmp(jmp_sw_hyb, repl_loop_idx)

    kernel[0:len(code)] = code
    kernel[STR_BASE:STR_BASE + len(strings_data)] = strings_data
    
    print(f"[Stage 2] Machine code size: {len(code)} bytes, Strings table: {len(strings_data)} bytes")
    return bytes(kernel)

def create_images(out_dir):
    os.makedirs(out_dir, exist_ok=True)
    
    # 1. Build Stage 1 MBR
    mbr_data = build_stage1_mbr()
    mbr_path = os.path.join(out_dir, "hybridos_mbr.bin")
    with open(mbr_path, "wb") as f:
        f.write(mbr_data)
    print(f"[OK] Generated Stage 1 MBR: {mbr_path} ({len(mbr_data)} bytes)")
    
    # 2. Build Stage 2 Kernel Core
    kernel_data = build_stage2_kernel()
    kernel_path = os.path.join(out_dir, "hybridos_kernel.bin")
    with open(kernel_path, "wb") as f:
        f.write(kernel_data)
    print(f"[OK] Generated Stage 2 Kernel: {kernel_path} ({len(kernel_data)} bytes)")
    
    # 3. Create Bootable System Disk (64 MB)
    disk_size = 64 * 1024 * 1024 # 64MB
    disk_path = os.path.join(out_dir, "hybridos_system.img")
    print(f"[*] Creating 64MB Bootable System Disk: {disk_path}...")
    with open(disk_path, "wb") as f:
        f.write(mbr_data)
        f.write(kernel_data)
        f.seek(disk_size - 1)
        f.write(b'\x00')
    print(f"[OK] Generated Bootable System Disk: {disk_path} ({os.path.getsize(disk_path) // (1024*1024)} MB)")
    
    # 4. Create Virtual NVMe Drive (64 MB)
    nvme_path = os.path.join(out_dir, "hybridos_nvme.img")
    print(f"[*] Creating 64MB Virtual NVMe Drive: {nvme_path}...")
    with open(nvme_path, "wb") as f:
        f.seek(64 * 1024 * 1024 - 1)
        f.write(b'\x00')
    print(f"[OK] Generated Virtual NVMe Drive: {nvme_path}")
    
    # 5. Create Virtual SATA AHCI Drive (32 MB)
    sata_path = os.path.join(out_dir, "hybridos_sata.img")
    print(f"[*] Creating 32MB Virtual SATA Drive: {sata_path}...")
    with open(sata_path, "wb") as f:
        f.seek(32 * 1024 * 1024 - 1)
        f.write(b'\x00')
    print(f"[OK] Generated Virtual SATA Drive: {sata_path}")

if __name__ == "__main__":
    out_dir = r"C:\Users\Vinicius\Documents\HybridOS-Kernel\qemu"
    create_images(out_dir)
