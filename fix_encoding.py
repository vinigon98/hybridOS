with open('rust-kernel/src/userland/shell.rs', 'rb') as f:
    content = f.read()

# Substituir caracteres não-ASCII comuns por versões em inglês
replacements = {
    b'\xe7': b'c',    # ç
    b'\xe1': b'a',    # á
    b'\xe9': b'e',     # é
    b'\xed': b'i',     # í
    b'\xf3': b'o',     # ó
    b'\xfa': b'u',     # ú
    b'\xe7': b'c',     # ç
    b'\xe0': b'a',     # à
    b'\xe2': b'a',     # â
    b'\xe4': b'a',     # ä
    b'\xe6': b'ae',    # æ
    b'\xe7': b'c',     # ç
    b'\xe8': b'e',     # è
    b'\xe9': b'e',     # é
    b'\xea': b'e',     # ê
    b'\xeb': b'e',     # ë
    b'\xec': b'i',     # ì
    b'\xed': b'i',     # í
    b'\xee': b'i',     # î
    b'\xef': b'i',     # ï
    b'\xf1': b'n',     # ñ
    b'\xf2': b'o',     # ò
    b'\xf3': b'o',     # ó
    b'\xf4': b'o',     # ô
    b'\xf5': b'o',     # õ
    b'\xf6': b'o',     # ö
    b'\xf8': b'o',     # ø
    b'\xf9': b'u',     # ù
    b'\xfa': b'u',     # ú
    b'\xfb': b'u',     # û
    b'\xfc': b'u',     # ü
    b'\xfd': b'y',     # ý
    b'\xff': b'y',     # ÿ
}

new_content = bytearray()
for b in content:
    new_content.append(replacements.get(b, b)[0])

with open('rust-kernel/src/userland/shell.rs', 'wb') as f:
    f.write(new_content)

print('Sucesso: Caracteres não-ASCII substituídos')