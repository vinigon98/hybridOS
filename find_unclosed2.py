with open('rust-kernel/src/userland/shell.rs', 'r') as f:
    content = f.read()

# Encontrar a posição 123064 e mostrar mais contexto
pos = 123064
print(f'Contexto completo ao redor da posição {pos}:')
print(repr(content[pos-200:pos+200]))