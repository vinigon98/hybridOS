with open('rust-kernel/src/userland/shell.rs', 'r') as f:
    content = f.read()

# Encontrar a última aspa dupla não fechada antes da posição 123065
pos = 123065
in_string = False
escape = False
last_open = -1
for i in range(pos):
    if escape:
        escape = False
        continue
    if content[i] == '\\':
        escape = True
        continue
    if content[i] == '"':
        if not in_string:
            in_string = True
            last_open = i
        else:
            in_string = False

if in_string:
    print(f'ASPAS DUPLAS NÃO FECHADAS na posição {last_open}')
    print(f'Contexto: {content[max(0,last_open-50):last_open+50]}')
else:
    print('Todas as aspas estão fechadas antes da posição 123065')