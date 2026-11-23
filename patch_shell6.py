with open('rust-kernel/src/userland/shell.rs', 'r', encoding='utf-8') as f:
    content = f.read()

# Substituir strings em português por inglês
replacements = {
    'Baixando via HTTP': 'Downloading via HTTP',
    'Baixado': 'Downloaded',
    'Salvo com sucesso': 'Saved successfully',
    'Falha ao baixar': 'Failed to download',
    'Falha ao salvar no VFS': 'Failed to save to VFS',
    'Iniciando download de': 'Downloading',
    'Conectando a': 'Connecting to',
}

for old, new in replacements.items():
    content = content.replace(old, new)

with open('rust-kernel/src/userland/shell.rs', 'w', encoding='utf-8') as f:
    f.write(content)

print('Sucesso: Caracteres não-ASCII substituídos!')