with open('rust-kernel/src/userland/shell.rs', 'r') as f:
    content = f.read()

start = content.find('pub fn cmd_download')
if start == -1:
    print('pub fn cmd_download não encontrado')
else:
    # Encontrar o próximo "pub fn " após start
    end = content.find('\n    pub fn ', start + 1)
    if end == -1:
        end = len(content)
    
    new_block = '''    /// Command `download`: Não implementado nesta versão.
    /// O HybridOS não suporta download via HTTP neste momento.
    /// Use 'run <arquivo>' para executar binários já presentes no VFS.
    pub fn cmd_download(&mut self, _args: &[String]) -> Result<String, ShellError> {
        Ok("Download não disponível nesta versão.\n\
          Use 'run <arquivo>' para executar binários já presentes no VFS.\n\
          Use 'apps' para listar aplicativos nativos.\n\
          ".to_string())
    }'''

    content = content[:start] + new_block + content[end:]
    
    with open('rust-kernel/src/userland/shell.rs', 'w') as f:
        f.write(content)
    print('Sucesso: cmd_download substituído por versão simples')