with open('rust-kernel/src/userland/shell.rs', 'r') as f:
    content = f.read()

# Verificar comentários de bloco não fechados
in_block_comment = False
start_pos = -1
for i in range(len(content) - 1):
    if content[i:i+2] == '/*':
        if not in_block_comment:
            in_block_comment = True
            start_pos = i
        else:
            # Aninhado - isso pode ser um problema
            pass
    elif content[i:i+2] == '*/':
        if in_block_comment:
            in_block_comment = False

if in_block_comment:
    print(f'COMENTÁRIO DE BLOCO NÃO FECHADO na posição {start_pos}')
    print(f'Contexto: {content[max(0,start_pos-50):start_pos+50]}')
else:
    print('Comentários de bloco estão balanceados')