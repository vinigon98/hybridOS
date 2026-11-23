with open('rust-kernel/src/userland/shell.rs', 'r') as f:
    content = f.read()

# Corrigir o colchete não fechado na posição 118487
old = 'assert!(ver.contains("Windows [Version 10.0.22631"));'
new = 'assert!(ver.contains("Windows [Version 10.0.22631]"));'

if old in content:
    content = content.replace(old, new, 1)
    with open('rust-kernel/src/userland/shell.rs', 'w') as f:
        f.write(content)
    print('Sucesso: Colchete fechado!')
else:
    print('ERRO: Texto não encontrado')
    # Procurar por variações
    if 'Windows [Version 10.0.22631' in content:
        print('Encontrei "Windows [Version 10.0.22631" no arquivo')
        idx = content.find('Windows [Version 10.0.22631')
        print(f'Contexto: {content[idx-30:idx+50]}')