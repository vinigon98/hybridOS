with open('rust-kernel/src/userland/shell.rs', 'r') as f:
    content = f.read()

in_string = False
escape = False
string_start = -1
for i, ch in enumerate(content):
    if escape:
        escape = False
        continue
    if ch == '\\':
        escape = True
        continue
    if ch == '"':
        if not in_string:
            in_string = True
            string_start = i
        else:
            in_string = False

if in_string:
    print(f'ASPAS DUPLAS NÃO FECHADAS na posição {string_start}')
    print(f'Contexto: {content[max(0,string_start-50):string_start+50]}')
else:
    print('Aspas duplas estão balanceadas')