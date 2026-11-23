with open('rust-kernel/src/userland/shell.rs', 'r') as f:
    content = f.read()

# Encontrar colchetes não fechados
depth = 0
for i, ch in enumerate(content):
    if ch == '[':
        depth += 1
    elif ch == ']':
        depth -= 1
    if depth < 0:
        print(f'Colchete fechado sem abertura na posição {i}')
        print(f'Contexto: {content[max(0,i-50):i+50]}')
        break

print(f'Profundidade final: {depth}')

# Se depth > 0, encontrar as últimas aberturas não fechadas
if depth > 0:
    # Encontrar todas as posições de colchetes abertos
    open_positions = []
    for i, ch in enumerate(content):
        if ch == '[':
            open_positions.append(i)
        elif ch == ']':
            if open_positions:
                open_positions.pop()
    
    print(f'Colchetes abertos não fechados nas posições: {open_positions}')
    for pos in open_positions:
        print(f'  Posição {pos}: {repr(content[max(0,pos-30):pos+30])}')