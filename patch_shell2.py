with open('rust-kernel/src/userland/shell.rs', 'r') as f:
    content = f.read()

# Encontrar o bloco do impl Display e substituir inteiramente
old = '''impl fmt::Display for ShellError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyCommand => write!(f, ""),
            Self::UnknownCommand(cmd) => {
                write!(f, "command not found: '{}'. Type 'help' for available commands.", cmd)
            }
            Self::MissingArgument(arg) => write!(f, "missing required argument: <{}>", arg),
            Self::InvalidArgument(msg) => write!(f, "invalid argument: {}", msg),
            Self::VfsError(err) => write!(f, "vfs error: {}", err),
            Self::BinaryExecutionError(err) => write!(f, "binary execution error: {}", err),
                        Self::IoError(err) => write!(f, "i/o error: {}", err),
            Self::DownloadError(err) => write!(f, "download error: {}", err),
        }
    }

    impl std::error::Error for ShellError {}'''

new = '''impl fmt::Display for ShellError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyCommand => write!(f, ""),
            Self::UnknownCommand(cmd) => {
                write!(f, "command not found: '{}'. Type 'help' for available commands.", cmd)
            }
            Self::MissingArgument(arg) => write!(f, "missing required argument: <{}>", arg),
            Self::InvalidArgument(msg) => write!(f, "invalid argument: {}", msg),
            Self::VfsError(err) => write!(f, "vfs error: {}", err),
            Self::BinaryExecutionError(err) => write!(f, "binary execution error: {}", err),
            Self::IoError(err) => write!(f, "i/o error: {}", err),
            Self::DownloadError(err) => write!(f, "download error: {}", err),
        }
    }

    impl std::error::Error for ShellError {}'''

if old in content:
    content = content.replace(old, new, 1)
    with open('rust-kernel/src/userland/shell.rs', 'w') as f:
        f.write(content)
    print('Sucesso: DownloadError adicionado corretamente!')
else:
    print('ERRO: Texto não encontrado')