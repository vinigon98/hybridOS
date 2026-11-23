with open('rust-kernel/src/userland/shell.rs', 'rb') as f:
    content = f.read()

# Substituir todos os bytes não-ASCII (> 127) por '?'
new_content = bytearray()
for byte in content:
    if byte > 127:
        new_content.append(ord('?'))  # substituir por '?'
    else:
        new_content.append(byte)

with open('rust-kernel/src/userland/shell.rs', 'wb') as f:
    f.write(new_content)

print('Sucesso: Caracteres não-ASCII substituídos')
print(f'Total de bytes alterados: {sum(1 for byte in content if byte > 127)}')