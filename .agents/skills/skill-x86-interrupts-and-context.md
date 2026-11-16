# Skill: Interrupções x86_64, IDT e Troca Real de Contexto no HybridOS

## Mecânica da CPU em Interrupções no x86_64
Ao receber uma interrupção ou exceção, o hardware da CPU x86_64 executa as seguintes etapas atômicas:
1. Se houve mudança de privilégio (Ring 3 -> Ring 0):
   - Lê `RSP0` da TSS (ou `IST` especificado no descritor de gate da IDT).
   - Salva o `SS` e `RSP` do usuário na nova pilha de kernel.
2. Empurra na pilha: `RFLAGS`, `CS`, `RIP`.
3. Para exceções com código de erro (`#DF=8`, `#TS=10`, `#NP=11`, `#SS=12`, `#GP=13`, `#PF=14`, `#AC=17`, `#CP=21`), a CPU empurra um `u64` com o Error Code.
4. Para vetores normais (sem código de erro), o stub Assembly deve empurrar um dummy `0` para uniformizar o frame na pilha antes de chamar o despachante Rust.
5. Limpa a flag `IF` (se for Interrupt Gate).

## Layout do Stack Frame de Interrupção
```rust
#[repr(C)]
pub struct InterruptContext {
    // Registradores gerais salvos pelo stub (ordem inversa de push)
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub r11: u64,
    pub r10: u64,
    pub r9: u64,
    pub r8: u64,
    pub rbp: u64,
    pub rdi: u64,
    pub rsi: u64,
    pub rdx: u64,
    pub rcx: u64,
    pub rbx: u64,
    pub rax: u64,
    // Vetor e Código de erro
    pub vector: u64,
    pub error_code: u64,
    // Hardware Stack Frame empurrado automaticamente pela CPU
    pub rip: u64,
    pub cs: u64,
    pub rflags: u64,
    pub rsp: u64,
    pub ss: u64,
}
```

## Regra de Ouro da IDT: `iretq` Obrigatório
- Todo tratador de interrupção x86_64 DEVE terminar com a instrução `iretq`.
- Nunca usar funções Rust `extern "C"` diretamente na IDT sem um stub Assembly naked, pois `extern "C"` retorna com `ret` (que consome `RIP` incorretamente e não restaura `CS`, `SS` e `RFLAGS`).
- No `#PF` (Vetor 14), o registrador `CR2` deve ser lido imediatamente antes de qualquer outra operação.

## Troca de Contexto Real (`switch_to`)
A troca cooperativa ou preemptiva de threads entre `prev` e `next` exige:
1. Salvar na pilha da thread atual: `rbx`, `rbp`, `r12`, `r13`, `r14`, `r15`, `rflags`.
2. Salvar o estado matemático/SSE da CPU (`fxsave` / `fxrstor`).
3. Salvar o `RSP` atual no descritor de thread `prev`.
4. Carregar o `RSP` do descritor de thread `next`.
5. Se `prev.cr3 != next.cr3`, recarregar o registrador `CR3` para comutar o espaço de endereçamento.
6. Atualizar `TSS.RSP0` com o topo da pilha de Ring 0 da thread `next`.
7. Atualizar registradores MSR de TLS:
   - `IA32_FS_BASE` (para threads Linux).
   - `IA32_KERNEL_GS_BASE` (para threads Windows, comutada com `swapgs` em user mode).
8. Restaurar `r15..rbx`, `rflags` e executar `ret` para saltar para o endereço salvo na pilha de `next`.
