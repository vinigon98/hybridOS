with open('rust-kernel/src/userland/shell.rs', 'r') as f:
    lines = f.readlines()

# Encontrar e corrigir o bloco
new_lines = []
i = 0
while i < len(lines):
    line = lines[i]
    
    # Detectar o início do bloco impl fmt::Display
    if 'impl fmt::Display for ShellError {' in line:
        new_lines.append(line)
        i += 1
        # Adicionar fn fmt
        if i < len(lines):
            new_lines.append(lines[i])
            i += 1
        # Adicionar match self {
        if i < len(lines):
            new_lines.append(lines[i])
            i += 1
        # Adicionar todos os casos, corrigindo a indentação
        while i < len(lines) and '}' not in lines[i]:
            line = lines[i]
            if 'Self::IoError(err)' in line:
                # Corrigir indentação
                new_lines.append('            Self::IoError(err) => write!(f, "i/o error: {}", err),\n')
            elif 'Self::DownloadError(err)' in line:
                new_lines.append(line)
            else:
                new_lines.append(line)
            i += 1
        # Adicionar o fechamento do match
        if i < len(lines):
            new_lines.append(lines[i])
            i += 1
        # Adicionar fechamento do fn fmt
        if i < len(lines):
            new_lines.append(lines[i])
            i += 1
        # Adicionar impl std::error::Error
        if i < len(lines):
            new_lines.append(lines[i])
            i += 1
    else:
        new_lines.append(lines[i])
        i += 1

with open('rust-kernel/src/userland/shell.rs', 'w') as f:
    f.writelines(new_lines)

print('Sucesso: Arquivo corrigido!')