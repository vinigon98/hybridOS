# PLANO — Instalar Primeiro Programa Externo (.exe) no HybridOS

## Estado Atual (IMPLEMENTADO)

### Kernel (rust-kernel/) — Compilando ✅

| Componente | Arquivo | Status |
|------------|---------|--------|
| Loader PE32+/PE64 | `src/loaders/pe.rs`, `pe64_advanced.rs` | ✅ Existente |
| IAT Binding | `src/loaders/pe_iat.rs` | ✅ Existente |
| Base Relocation | `src/loaders/pe_reloc.rs` | ✅ Existente |
| **Loader PE do VFS** | `src/loaders/pe_from_vfs.rs` | ✅ **NOVO** |
| Integração no boot | `src/main.rs` | ✅ **MODIFICADO** |

### O que `pe_from_vfs.rs` implementa:

1. **`load_pe_from_vfs(path)`** — Lê .exe do VFS via `vfs_read_all()`, valida assinatura MZ/PE, chama `spawn_pe_process()` para executar em Ring 3
2. **`install_program_from_downloads(exe_name, sha256)`** — Copia de /Downloads para /Program Files com validação de hash
3. **`list_installed_programs()`** — Lista .exe em /Program Files

### O que `main.rs` faz agora:

Após o teste de Ring 3 (Linux ELF + Windows PE), tenta carregar automaticamente:
- `/Program Files/hello.exe`
- `/Downloads/hello.exe`

Se encontrar, executa o .exe em Ring 3.

---

## .exe de Teste Gerado ✅

**Arquivo:** `hello_test.exe` (raiz do projeto)

- Formato: PE64 (x86_64 Windows)
- EntryPoint: 0x1000
- ImageBase: 0x400000
- Tamanho: 1024 bytes
- Função: Imprime "Hello from HybridOS!" via syscall, termina com ExitProcess

**Script gerador:** `scripts/make_hello_exe.py`

---

## Próximos Passos para Testar no QEMU

### 1. Adicionar hello.exe ao disco de boot

O disco de boot do QEMU é gerado por `qemu/build_disk.py`. Para testar, copie `hello_test.exe` como `/Program Files/hello.exe` ou `/Downloads/hello.exe` na imagem FAT32.

O kernel já tenta carregar esses caminhos no boot (linhas 235-239 de main.rs).

### 2. Executar QEMU

```bash
python qemu/build_disk.py
python qemu/run-qemu.bat  (ou qemu/run_gui.bat)
```

Verificar na saída serial se aparece:
```
[PE-VFS] Carregando '/Program Files/hello.exe' (1024 bytes) do VFS
[PE-VFS] Processo '/Program Files/hello.exe' spawnado com sucesso
Hello from HybridOS!
```

---

## Fluxo Completo para hyb-pkg (FUTURO)

Quando o `hyb-pkg` for estendido para suporte completo a .exe:

```
hyb-pkg install <url>.exe [sha256]
  1. Baixa via curl para /Downloads/
  2. Valida SHA-256 (se fornecido)
  3. Chama install_program_from_downloads() -> grava em /Program Files/
  4. Kernel carrega automaticamente no próximo boot (ou via syscall)

hyb-pkg run <nome>.exe
  1. Chama syscall para carregar o .exe do VFS
  2. Executa em Ring 3
```

### Syscalls necessárias (futuro):
- `NtLoadImageFromVFS(path)` — carrega e executa .exe do VFS sob demanda
- `NtCreateFile/VFS` — escrita de arquivo do userspace para VFS

---

## Arquivos Modificados/Criados

| Arquivo | Ação | Descrição |
|---------|------|-----------|
| `rust-kernel/src/loaders/pe_from_vfs.rs` | Criado | Loader PE do VFS + install + list |
| `rust-kernel/src/loaders/mod.rs` | Modificado | Exporta novas funções |
| `rust-kernel/src/main.rs` | Modificado | Tenta carregar .exe no boot |
| `scripts/make_hello_exe.py` | Criado | Gera .exe Hello World de teste |
| `hello_test.exe` | Criado | Binário PE64 de teste |
| `PLANO_EXE_INSTALACAO.md` | Criado | Este documento |
