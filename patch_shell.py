with open('rust-kernel/src/userland/shell.rs', 'r') as f:
    content = f.read()

# Encontrar o ponto exato de inserção
start = content.find('Self::IoError(err) => write!(f, "i/o error: {}", err),')
end = content.find('impl std::error::Error for ShellError {}')

if start != -1 and end != -1:
    end = end + len('impl std::error::Error for ShellError {}')
    
    # O que queremos substituir
    old_segment = content[start:end]
    print(f'Encontrado segmento de {len(old_segment)} caracteres')
    
    new_segment = '''            Self::IoError(err) => write!(f, "i/o error: {}", err),
            Self::DownloadError(err) => write!(f, "download error: {}", err),
        }
    }

    impl std::error::Error for ShellError {}'''
    
    content = content[:start] + new_segment + content[end:]
    
    with open('rust-kernel/src/userland/shell.rs', 'w') as f:
        f.write(content)
    print('Sucesso: DownloadError adicionado!')
else:
    print('ERRO: Não foi possível encontrar os pontos de inserção')
    print(f'start={start}, end={end}')