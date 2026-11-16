# Skill 01 — Pesquisa e Planejamento Técnico de Micro-Tarefas

## Objetivo
Orientar o agente orquestrador ou planejador a selecionar o próximo item pendente em `PROGRESS.md`, inspecionar o código-fonte atual do kernel e estruturar um plano de implementação robusto antes de qualquer alteração de código.

---

## Procedimento Passo a Passo

### 1. Localização e Leitura de Estado
1. Abra `PROGRESS.md` na raiz (`C:\Users\Vinicius\Documents\HybridOS-Kernel\PROGRESS.md`).
2. Localize a primeira micro-tarefa pendente marcada com `[ ]` na fase atual.
3. Leia `.agents/GUARDRAILS.md` para relembrar as invariantes de hardware e regras de caminho absoluto.
4. Execute a Skill `00_research_opensource_rust.md` para consultar as referências de código (Redox, Asterinas, Aero OS, `uutils`, etc.) adequadas para o subsistema da tarefa.

### 2. Inspeção Detalhada do Código Existente
Navegue até o código do kernel em:
`C:\Users\Vinicius\Documents\HybridOS-Kernel\rust-kernel\src\`
- Inspecione os módulos relacionados (ex: `drivers/desktop.rs`, `arch/`, `mm/`, `userland/shell.rs`, `fs/`).
- Mapeie as estruturas de dados (`struct`, `enum`), traços (`trait`) e assinaturas de funções já existentes para manter 100% de consistência.
- Verifique se a tarefa requer alocação de memória no Heap (`alloc::vec::Vec`, `alloc::string::String`) ou tipos atômicos/sincronizados (`SpinLock`).

### 3. Definição do Token de Auto-Teste
- Cada tarefa DEVE definir um token serial de auto-teste determinístico (ex: `[TEST_OK] GFX_CRYSTAL_GLASS_SHADER`, `[TEST_OK] APP_EXPLORER_REAL_VFS`).
- O plano deve prever onde e como esse token será impresso na serial COM1 durante o boot do kernel antes de atingir a inicialização operacional completa.

### 4. Critérios de Conclusão do Planejamento
- Plano de arquivos a serem criados/modificados com separação clara de responsabilidades.
- Confirmação de que nenhum dado mock/estático será inserido (respeitando a REGRA 2 de `GUARDRAILS.md`).
- Passagem do plano para a etapa de implementação (`02_parallel_implement.md`).
