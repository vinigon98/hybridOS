with open('src/main.rs', 'r') as f:
    lines = f.readlines()

# Encontrar e corrigir o problema na linha 399 (índice 398)
# O problema é um '}' sobrando na linha 399
new_lines = []
skip_line = False
for i, line in enumerate(lines):
    if i == 398:  # Linha 399 (índice 398) - remover esta linha que tem '}' sobrando
        print(f"Removendo linha {i+1}: {line.strip()}")
        continue
    new_lines.append(line)

with open('src/main.rs', 'w') as f:
    f.writelines(new_lines)

print("Correção aplicada com sucesso!")