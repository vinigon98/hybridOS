# Skill 02 — Implementação Modular e Paralela com Subagentes

## Objetivo
Instruir o Agente Orquestrador a coordenar a codificação da micro-tarefa planejada, distribuindo trabalho modularmente para subagentes autônomos ou executando a implementação direta com máxima disciplina.

---

## Regras Obrigatórias de Orquestração

### 1. Injeção Obrigatória do Caminho Absoluto
Ao invocar qualquer subagente ou executar ferramentas, **SEMPRE** forneça no início da instrução:
```text
DIRETÓRIO DO KERNEL: C:\Users\Vinicius\Documents\HybridOS-Kernel\rust-kernel
DIRETÓRIO RAIZ DO PROJETO: C:\Users\Vinicius\Documents\HybridOS-Kernel
```
Nunca confie no diretório corrente padrão da ferramenta, que aponta para o diretório da IDE.

### 2. Isolamento de Arquivos em Edições Paralelas
- Se a tarefa for decomposta em mais de um subagente em paralelo:
  - **Dois subagentes NUNCA devem editar o mesmo arquivo simultaneamente.**
  - Subagente A: responsável exclusivamente pelo módulo X (ex: `drivers/desktop.rs`).
  - Subagente B: responsável exclusivamente pelo módulo Y (ex: `userland/shell.rs`).
  - Subagente C: responsável pelos auto-testes ou ligação em `main.rs`.

### 3. Padrões de Código e Invariantes
- Toda implementação deve seguir o padrão `#![no_std]`, utilizando `alloc::` quando necessário.
- Todo bloco `unsafe` DEVE conter um comentário de segurança `// SAFETY:` justificando as garantias de ponteiros, alinhamento e hardware.
- Nenhum dado estático/simulado pode ser inserido (REGRA 2 do `GUARDRAILS.md`).
- Implementar a função de auto-teste `self_test()` associada à tarefa, emitindo a mensagem `[TEST_OK] <TAG>` na porta serial COM1 (`0x3F8`).

### 4. Ciclo de Validação Imediata
Assim que a codificação for concluída:
1. Compile e teste imediatamente via validador oficial:
   ```powershell
   python qemu/verify_loop.py
   ```
2. Se o retorno for `exit 0`, chame a auditoria (`03_forensic_audit.md`).
3. Se falhar (`exit 1`), acione diagnóstico imediato, corrija a falha e repita o teste.
