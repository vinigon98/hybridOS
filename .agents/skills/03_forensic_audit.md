# Skill 03 — Auditoria Forense e Revisão Adversarial

## Objetivo
Atuar como um Auditor de Kernel rigoroso e implacável após a passagem do teste funcional, inspecionando o código antes que ele seja considerado concluído e registrado em `PROGRESS.md`.

---

## Procedimento de Auditoria Forense

### 1. Inspeção do Git Diff
Analise todas as modificações introduzidas na iteração atual:
```bash
git diff
```
Verifique linha por linha buscando as seguintes falhas:
- **Código Incompleto ou Mocks:**
  - Presença de macros proibidas: `todo!()`, `unimplemented!()`, `panic!()` em caminhos normais.
  - Estruturas falsas ou dados mockados (ex: listas fixas de arquivos ou processos artificiais).
- **Segurança de Memória e Blocos Unsafe:**
  - Cada `unsafe { ... }` possui um comentário `// SAFETY:` robusto e verdadeiro?
  - Há ponteiros não alinhados, possíveis estouros de buffer ou casts perigosos?
- **Concorrência e Interrupções:**
  - Há algum `SpinLock` adquirido com interrupções ativas que possa causar deadlock com tratador de IRQ?
- **Estilo e Advertências:**
  - Presença de números mágicos sem constantes nomeadas (`const` ou `bitflags`).
  - Avisos de compilador (`cargo clippy -- -D warnings`).

### 2. Execução Oficial de Validação do Loop
O auditor deve obrigatoriamente executar o validador oficial do loop:
```powershell
python qemu/verify_loop.py
```
Critérios de aprovação do auditor:
1. `cargo build --release --target x86_64-unknown-none` compilou com código de saída 0.
2. `python qemu/build_disk.py` gerou a imagem de boot sem erros.
3. O QEMU completou o boot headless dentro do timeout de 15 segundos.
4. O marcador `[GUI] HybridOS Desktop Environment & PS/2 Mouse active.` apareceu no log serial.
5. O token serial `[TEST_OK] <NOME_DA_TAREFA>` foi emitido e confirmado no log.
6. Zero panics, triple faults ou exceções não tratadas.

### 3. Conclusão e Registro
Apenas após todas as condições acima serem satisfeitas:
1. Atualize `PROGRESS.md`, marcando a micro-tarefa com `[x]`.
2. Se novas estruturas públicas foram criadas, atualize `SPEC.md`.
3. Registre novos aprendizados ou armadilhas encontradas em `.agents/GUARDRAILS.md`.
4. Faça commit limpo no repositório:
   ```bash
   git add -A && git commit -m "feat(<modulo>): <descricao da tarefa> [verified & audited]"
   ```
