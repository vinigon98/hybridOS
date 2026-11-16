# Skill: VFS Bilíngue Nativo no HybridOS

## Arquitetura do VFS Unificado
O Virtual File System (VFS) do HybridOS suporta simultaneamente duas semânticas de caminhos de arquivo no mesmo namespace:
1. **Sintaxe Windows / DOS**: Letras de unidade (`C:\`, `D:\`), barras invertidas `\`, insensibilidade a maiúsculas/minúsculas opcional por montagem.
2. **Sintaxe Unix / POSIX**: Raiz unificada `/`, separadores `/`, sensibilidade a maiúsculas/minúsculas.

## Resolução e Mapeamento Bilíngue
- A unidade raiz `C:\` é montada como o volume de sistema principal.
- No namespace POSIX, a raiz `/` espelha o volume de sistema `C:\`:
  - `/bin` mapeia para `C:\bin`
  - `/usr` mapeia para `C:\usr`
  - `/etc` mapeia para `C:\etc`
  - `/proc` e `/sys` são sistemas de arquivos virtuais sintéticos do kernel.
- Se um processo com personalidade Windows chamar `NtCreateFile` com `\??\C:\Windows\notepad.exe`, o resolvedor localiza o inode correspondente.
- Se um processo com personalidade Linux chamar `sys_open` com `/bin/sh`, o mesmo resolvedor VFS traduz para o mesmo nó correspondente.

## Operações Reais de I/O de Arquivo
- **Buffers Binários Seguros**:
  Syscalls como `sys_read`, `sys_write`, `NtReadFile` e `NtWriteFile` devem receber buffers de bytes (`*const u8`, `*mut u8`) acompanhados do tamanho explícito em bytes (`length: usize`).
  Nunca converter buffers arbitrários para `CStr` ou assumir terminação nula.
- **Handles e Descritores de Arquivo**:
  A tabela de handles do processo gerencia instâncias abertas de arquivos conectadas aos Inodes reais do VFS, mantendo o offset atual de leitura/escrita e as permissões de acesso concedidas (`GENERIC_READ`, `GENERIC_WRITE`, `O_RDONLY`, `O_WRONLY`, `O_RDWR`).
