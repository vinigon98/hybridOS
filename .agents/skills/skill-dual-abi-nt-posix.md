# Skill: Dupla Personalidade Nativa (Windows NT & Linux POSIX) no HybridOS

## Filosofia: Tradução Direta para Primitivas do Kernel
O HybridOS não é um emulador e não usa o kernel Linux ou o código do Windows NT.
O kernel implementa nativamente as interfaces binárias (ABIs) oficiais de ambos os ecossistemas, despachando chamadas diretamente para o subsistema nativo do HybridOS.

## ABI Windows NT x64
1. **Convenção de Chamada Microsoft x64**:
   - Registradores de argumento: `RCX`, `RDX`, `R8`, `R9`.
   - Na instrução `SYSCALL`: O registrador `RCX` é sobrescrito pela CPU com o endereço de retorno `RIP`. Portanto, o Windows utiliza `R10` para passar o primeiro parâmetro, `RDX` para o segundo, `R8` para o terceiro, `R9` para o quarto.
   - Parâmetros adicionais (5º, 6º, ...) são passados na pilha de usuário:
     - `[rsp + 0x20]`: 5º parâmetro.
     - `[rsp + 0x28]`: 6º parâmetro.
     - Nota: O chamador aloca 32 bytes de "shadow space" (`[rsp + 0x00]` a `[rsp + 0x18]`).
   - Retorno: `RAX` contendo `NTSTATUS` (ex: `STATUS_SUCCESS = 0x00000000`, `STATUS_UNSUCCESSFUL = 0xC0000001`).
2. **Estruturas de Dados do Processo e Thread Windows**:
   - **`TEB` (Thread Environment Block)**:
     - Endereçada via registrador de segmento `GS` em Ring 3.
     - `GS:[0x30]`: Ponteiro `Self` da `TEB`.
     - `GS:[0x60]`: Ponteiro para a `PEB`.
   - **`PEB` (Process Environment Block)**:
     - Contém `ImageBaseAddress`, `ProcessParameters` (linha de comando, variáveis de ambiente).
   - **`KUSER_SHARED_DATA`**:
     - Mapeada em página somente-leitura em `0x7FFE0000` em todo processo Windows NT.
     - Fornece timestamp de alta precisão (`InterruptTime`, `SystemTime`) sem necessidade de syscall.

## ABI Linux POSIX System V AMD64
1. **Convenção de Chamada de Syscall Linux**:
   - Número da syscall: `RAX`.
   - Argumentos: `RDI` (1º), `RSI` (2º), `RDX` (3º), `R10` (4º - `RCX` destruído pelo hardware), `R8` (5º), `R9` (6º).
   - Retorno: `RAX` (valores negativos entre `-1` e `-4095` representam `-errno`).
2. **Inicialização da Pilha de Usuário (System V AMD64 ABI)**:
   - Ao iniciar um processo ELF64, a pilha deve conter:
     - `[rsp + 0]`: `argc` (`u64`).
     - `[rsp + 8 ..]`: `argv[0]`, `argv[1]`, ..., `NULL`.
     - `[rsp + env_offset ..]`: `envp[0]`, `envp[1]`, ..., `NULL`.
     - `[rsp + auxv_offset ..]`: Entradas `Auxv` (`a_type`, `a_val`) terminando com `AT_NULL (0, 0)`.
   - O alinhamento do `RSP` antes do salto para a primeira instrução deve respeitar `(RSP + 8) % 16 == 0` se simular um `call`, ou `RSP % 16 == 0` no ponto de entrada do processo.
3. **Thread-Local Storage (TLS)**:
   - Apontado pelo registrador `FSBase` (`IA32_FS_BASE` MSR).

## Isolamento e Validação de Memória de Usuário
- O kernel NUNCA deve desreferenciar ponteiros passados por syscall diretamente sem validação prévia.
- Funções obrigatórias:
  - `copy_from_user<T>(src_user_ptr: u64, dst_kernel: *mut T) -> Result<(), KernelError>`
  - `copy_to_user<T>(dst_user_ptr: u64, src_kernel: *const T) -> Result<(), KernelError>`
- Verificações de segurança:
  1. `ptr + size <= 0x0000_7FFF_FFFF_FFFF` (não pode apontar para a metade superior do kernel).
  2. As páginas virtuais correspondentes devem estar marcadas como presentes e com o bit `USER_ACCESSIBLE` na PML4 do processo.
