with open('rust-kernel/src/userland/shell.rs', 'rb') as f:
    content = f.read()

# Verificar bytes não-ASCII
for i, byte in enumerate(content):
    if byte > 127:
        print(f'Byte não-ASCII na posição {i}: 0x{byte:02x}')
        # Mostrar contexto
        start = max(0, i-20)
        end = min(len(content), i+20)
        print(f'Contexto: {content[start:end]}')
        break
else:
    print('Todos os bytes são ASCII (0-127)')