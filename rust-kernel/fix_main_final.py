with open('src/main.rs', 'r') as f:
    content = f.read()

# Contar chaves
open_count = content.count('{')
close_count = content.count('}')
print(f"Chaves abertas: {open_count}")
print(f"Chaves fechadas: {close_count}")
print(f"Diferença: {open_count - close_count}")

# Se há mais chaves abertas que fechadas, adicionar o necessário
if open_count > close_count:
    diff = open_count - close_count
    print(f"Adicionando {diff} fechaduras '}' no final")
    content = content.rstrip() + '\n' + ('}' * diff)
elif close_count > open_count:
    diff = close_count - open_count
    print(f"Removendo {diff} fechaduras '}' do final")
    # Remover chaves extras do final
    while diff > 0 and content.endswith('}'):
        content = content[:-1]
        diff -= 1
    with open('src/main.rs', 'w') as f:
        f.write(content)
    print('Corrigido!')
else:
    print('Chaves balanceadas!')