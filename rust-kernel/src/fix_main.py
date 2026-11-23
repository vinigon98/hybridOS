with open('rust-kernel/src/main.rs', 'r') as f:
    lines = f.readlines()

# Encontrar e corrigir o problema na linha 398 (índice 397)
# O problema é um '}' sobrando na linha 399
# Vamos reconstruir o arquivo sem o '}' extra na linha 399

new_lines = []
skip_next = False
for i, line in enumerate(lines):
    # Linha 398 (índice 397) tem '};' que fecha corretamente
    # Linha 399 (índice 398) tem '}' sobrando que deve ser removida
    if i == 397:  # Linha 398 (índice 397)
        # Esta linha tem '};' que fecha corretamente o bloco
        new_lines.append(line)
        continue
    if i == 398:  # Linha 399 (índice 398) - este é o '}' extra
        print(f"Removendo linha {i+1}: {line.strip()}")
        continue  # Pular esta linha
    new_lines.append(line)

with open('rust-kernel/src/main.rs', 'w') as f:
    f.writelines(new_lines)

print("Correção aplicada com sucesso!")