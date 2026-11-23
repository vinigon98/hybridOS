#!/usr/bin/env python3
"""
Gera um PE64 (Windows x86_64) mínimo que:
1. Imprime "Hello from HybridOS!" via NtWriteFile (stdout)
2. Termina via NtTerminateProcess(0)

Este .exe roda no kernel HybridOS bare-metal via loader PE + stubs ntdll/kernel32.
"""

import struct
import sys

# ---------- Constantes PE ----------
IMAGE_DOS_SIGNATURE = 0x5A4D          # "MZ"
IMAGE_NT_SIGNATURE  = 0x00004550      # "PE\0\0"
IMAGE_FILE_MACHINE_AMD64 = 0x8664
OPTIONAL_HEADER_MAGIC_PE32_PLUS = 0x20B
IMAGE_SUBSYSTEM_WINDOWS_CUI = 3

IMAGE_SCN_CNT_CODE   = 0x00000020
IMAGE_SCN_MEM_EXECUTE = 0x20000000
IMAGE_SCN_MEM_READ    = 0x40000000

# ---------- Código x86_64 (Hello World via syscall) ----------
# Montagem manual (sem assembler):
#   mov rax, 1          ; sys_write (Linux) ou NtWriteFile (Windows)
#   mov rdi, 1          ; fd = stdout
#   lea rsi, [rel msg]  ; mensagem
#   mov rdx, msglen     ; tamanho
#   syscall
#   mov rax, 60         ; sys_exit (Linux) ou NtTerminateProcess (Windows)
#   xor rdi, rdi        ; código 0
#   syscall
# msg: db "Hello from HybridOS!\n", 0

# Código de máquina x86_64 (apenas instruções simples, sem relocações complexas)
# Vamos usar um truque: colocar a mensagem logo após o código e usar RIP-relative
code = bytes.fromhex(
    "48c7c001000000"      # mov rax, 1
    "48c7c701000000"      # mov rdi, 1
    "488d350e000000"      # lea rsi, [rip + 0x0e]  -> aponta para msg
    "48c7c214000000"      # mov rdx, 0x14 (20 bytes)
    "0f05"                # syscall
    "48c7c03c000000"      # mov rax, 60 (sys_exit)
    "48c7c700000000"      # mov rdi, 0
    "0f05"                # syscall
    "48656c6c6f2066726f6d204879627269644f53210a"  # "Hello from HybridOS!\n"
)
code_len = len(code)
print(f"[+] Código: {code_len} bytes")

# ---------- Estruturas PE ----------
# DOS Header (64 bytes)
dos_header = bytearray(64)
dos_header[0:2] = b"MZ"
dos_header[0x3C:0x40] = struct.pack("<I", 64)  # e_lfanew = 64 (PE header começa no offset 64)

# PE Signature
pe_sig = b"PE\x00\x00"

# COFF Header (20 bytes)
coff_header = bytearray(20)
coff_header[0:2] = struct.pack("<H", IMAGE_FILE_MACHINE_AMD64)  # Machine
coff_header[2:4] = struct.pack("<H", 1)                          # NumberOfSections
coff_header[4:8] = struct.pack("<I", 0)                          # TimeDateStamp
coff_header[8:12] = struct.pack("<I", 0)                         # PointerToSymbolTable
coff_header[12:16] = struct.pack("<I", 0)                        # NumberOfSymbols
coff_header[16:18] = struct.pack("<H", 0)                        # SizeOfOptionalHeader (vamos preencher depois)
coff_header[18:20] = struct.pack("<H", 0x0003)                   # Characteristics (executable image)

# Optional Header PE32+ (pelo menos 112 bytes para ter os campos essenciais)
opt_header = bytearray(112)
opt_header[0:2] = struct.pack("<H", OPTIONAL_HEADER_MAGIC_PE32_PLUS)  # Magic
opt_header[2] = 14                                                    # MajorLinkerVersion
opt_header[3] = 0                                                     # MinorLinkerVersion
opt_header[4:8] = struct.pack("<I", code_len)                         # SizeOfCode
opt_header[8:12] = struct.pack("<I", 0)                               # SizeOfInitializedData
opt_header[12:16] = struct.pack("<I", 0)                              # SizeOfUninitializedData
opt_header[16:20] = struct.pack("<I", 0x1000)                         # AddressOfEntryPoint
opt_header[20:24] = struct.pack("<I", 0x1000)                         # BaseOfCode
opt_header[24:32] = struct.pack("<Q", 0x0000_0000_0040_0000)         # ImageBase (0x400000)
opt_header[32:36] = struct.pack("<I", 0x1000)                         # SectionAlignment
opt_header[36:40] = struct.pack("<I", 0x200)                          # FileAlignment
opt_header[40:44] = struct.pack("<H", 6)                              # MajorOperatingSystemVersion (Windows 7)
opt_header[44:46] = struct.pack("<H", 1)                              # MinorOperatingSystemVersion
opt_header[46:48] = struct.pack("<H", 0)                              # MajorImageVersion
opt_header[48:50] = struct.pack("<H", 0)                              # MinorImageVersion
opt_header[50:52] = struct.pack("<H", 6)                              # MajorSubsystemVersion
opt_header[52:54] = struct.pack("<H", 1)                              # MinorSubsystemVersion
opt_header[54:58] = struct.pack("<I", 0)                              # Win32VersionValue
opt_header[58:62] = struct.pack("<I", code_len + 0x200)               # SizeOfImage (alinhado)
opt_header[62:66] = struct.pack("<I", 0x200)                          # SizeOfHeaders
opt_header[66:70] = struct.pack("<I", 0)                              # CheckSum
opt_header[70:72] = struct.pack("<H", IMAGE_SUBSYSTEM_WINDOWS_CUI)    # Subsystem (CUI)
opt_header[72:74] = struct.pack("<H", 0x0100)                         # DllCharacteristics
opt_header[74:82] = struct.pack("<Q", 0x100000)                       # SizeOfStackReserve
opt_header[82:90] = struct.pack("<Q", 0x1000)                         # SizeOfStackCommit
opt_header[90:98] = struct.pack("<Q", 0x100000)                       # SizeOfHeapReserve
opt_header[98:106] = struct.pack("<Q", 0x1000)                        # SizeOfHeapCommit
opt_header[106:110] = struct.pack("<I", 0)                            # LoaderFlags
opt_header[110:112] = struct.pack("<I", 16)                           # NumberOfRvaAndSizes

# Data Directories (16 entradas de 8 bytes cada = 128 bytes)
# Vamos adicionar apenas o suficiente para o Optional Header ter 112 + 128 = 240 bytes
# Atualizando SizeOfOptionalHeader no COFF Header
coff_header[16:18] = struct.pack("<H", 112 + 128)  # SizeOfOptionalHeader = 240

# Data directories (todas zero exceto as que precisamos)
data_dirs = bytearray(128)
# IMAGE_DIRECTORY_ENTRY_IMPORT (índice 1) - vamos deixar zero por enquanto (sem imports)
# IMAGE_DIRECTORY_ENTRY_BASERELOC (índice 5) - zero

# Montando Optional Header completo
opt_header_full = opt_header + data_dirs

# ---------- Section Table ----------
# Uma seção: .text
section_name = b".text\x00\x00\x00"
section_table = bytearray(40)  # Uma seção = 40 bytes
section_table[0:8] = section_name                                    # Name
section_table[8:12] = struct.pack("<I", code_len)                    # VirtualSize
section_table[12:16] = struct.pack("<I", 0x1000)                     # VirtualAddress
section_table[16:20] = struct.pack("<I", ((code_len + 0xFF) & 0xFFFFFF00))  # SizeOfRawData (alinhado a 0x200)
section_table[20:24] = struct.pack("<I", 0x200)                      # PointerToRawData
section_table[24:28] = struct.pack("<I", 0)                          # PointerToRelocations
section_table[28:32] = struct.pack("<I", 0)                          # PointerToLinenumbers
section_table[32:36] = struct.pack("<H", 0)                          # NumberOfRelocations
section_table[36:40] = struct.pack("<H", 0)                          # NumberOfLinenumbers
section_table[36:40] = struct.pack("<I", IMAGE_SCN_CNT_CODE | IMAGE_SCN_MEM_EXECUTE | IMAGE_SCN_MEM_READ)  # Characteristics

# ---------- Montando o arquivo ----------
# Atualizar SizeOfRawData para ser múltiplo de FileAlignment (0x200)
raw_data_size = ((code_len + 0x1FF) & 0xFFFFFE00)  # alinhar a 0x200
section_table[16:20] = struct.pack("<I", raw_data_size)

# Atualizar SizeOfImage no Optional Header
opt_header_full[58:62] = struct.pack("<I", 0x2000)  # SizeOfImage (duas seções: header + .text)

# Cabeçalho (64 bytes DOS + 4 bytes PE sig + 20 bytes COFF + 240 bytes Optional Header)
header_size = 64 + 4 + 20 + len(opt_header_full)
header = bytearray(header_size)
header[0:64] = dos_header
header[64:68] = pe_sig
header[68:88] = coff_header
header[88:88 + len(opt_header_full)] = opt_header_full

# Seção .text (alinhada a 0x200)
section_data = bytearray(raw_data_size)
section_data[0:code_len] = code

# Montando arquivo final
exe = bytearray()
exe.extend(header)
# Preencher até PointerToRawData (0x200)
while len(exe) < 0x200:
    exe.append(0)
exe.extend(section_data)

# Salvando
output_path = sys.argv[1] if len(sys.argv) > 1 else "hello.exe"
with open(output_path, "wb") as f:
    f.write(exe)

print(f"[+] PE64 gerado: {output_path} ({len(exe)} bytes)")
print(f"[+] EntryPoint: 0x1000")
print(f"[+] ImageBase: 0x400000")
print(f"[+] Código: {code_len} bytes")
print(f"[+] Tamanho do arquivo: {len(exe)} bytes")