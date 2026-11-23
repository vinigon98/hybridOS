with open('rust-kernel/src/userland/shell.rs', 'r') as f:
    content = f.read()

# Verificar aspas duplas não fechadas
in_string = False
escape = False
for i, ch in enumerate(content):
    if escape:
        escape = False
        continue
    if ch == '\\':
        escape = True
        continue
    if ch == '"':
        in_string = not in_string

if in_string:
    print('ASPAS DUPLAS NÃO FECHADAS!')
else:
    print('Aspas duplas estão balanceadas')

# Verificar comentários de bloco
in_block_comment = False
for i in range(len(content) - 1):
    if content[i:i+2] == '/*':
        in_block_comment = True
    elif content[i:i+2] == '*/':
        in_block_comment = False

if in_block_comment:
    print('COMENTÁRIO DE BLOCO NÃO FECHADO!')
else:
    print('Comentários de bloco estão balanceados')