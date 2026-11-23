# HybridOS System Updater — Guia do Usuário e IA

Este documento explica como usar o sistema de atualização do HybridOS (estilo Windows Update) para:
1. Criar pacotes de atualização (`.hybupdate`)
2. Instalar atualizações
3. Criar pontos de restauração
4. Fazer rollback se algo quebrar
5. Manter dados do usuário preservados

---

## 1. Visão Geral

O HybridOS agora possui um sistema de atualização inspirado no Windows Update, com os seguintes componentes:

| Componente | Caminho | Função |
|------------|---------|--------|
| **hyb-update** | `userland/apps/hyb-update/` | CLI de gerenciamento de atualizações |
| **updater (kernel)** | `rust-kernel/src/updater.rs` | Detecção e aplicação no boot |
| **Pacotes** | `*.hybupdate` | Arquivo JSON com binários e arquivos VFS |
| **Pontos de restauração** | `/update/restore/` | Snapshots automáticos antes de atualizar |
| **Atualização pendente** | `/update/pending/` | Pacote aguardando reboot |

---

## 2. Formato do Pacote de Atualização (.hybupdate)

Um pacote de atualização é um arquivo **JSON** com a seguinte estrutura:

```json
{
  "version": "1.0.1",
  "description": "Atualização de segurança e correção de bugs",
  "kernel": "/caminho/para/novo_kernel.bin",
  "files": {
    "/etc/hybridos/system.toml": "BASE64_CONTEUDO_AQUI",
    "/C:\\Windows\\System32\\driver.sys": "BASE64_CONTEUDO_AQUI",
    "/usr/bin/novo_app": "BASE64_CONTEUDO_AQUI"
  },
  "reboot_required": true
}
```

### Campos:

| Campo | Tipo | Obrigatório | Descrição |
|-------|------|-------------|-----------|
| `version` | string | Sim | Versão da atualização (ex: "1.0.1") |
| `description` | string | Sim | Descrição legível do que a atualização faz |
| `kernel` | string | Não | Caminho para um novo binário do kernel (PE64/ELF64) |
| `files` | object | Não | Mapeamento de caminho VFS → conteúdo Base64 |
| `reboot_required` | boolean | Não | Se `true`, o sistema solicitará reboot após instalar |

### Como codificar arquivos em Base64

Você pode usar qualquer ferramenta que gere Base64. Exemplos:

**Linux/macOS:**
```bash
base64 -w 0 meu_arquivo.bin
```

**Python:**
```python
import base64
with open("meu_arquivo.bin", "rb") as f:
    print(base64.b64encode(f.read()).decode())
```

**PowerShell (Windows):**
```powershell
[Convert]::ToBase64String([IO.File]::ReadAllBytes("meu_arquivo.bin"))
```

---

## 3. Fluxo de Atualização Completo

### Passo 1: O usuário (ou IA) cria o pacote

Exemplo de cenário: o usuário quer adicionar suporte a um novo driver de rede.

1. O agente de IA modifica o código do kernel em `rust-kernel/src/drivers/meu_driver.rs`
2. O agente recompila o kernel: `cargo build --release --target x86_64-unknown-none`
3. O agente cria o pacote `.hybupdate` com o novo kernel e eventuais arquivos de configuração

**Exemplo de pacote completo:**
```json
{
  "version": "1.1.0",
  "description": "Adiciona suporte a driver de rede Intel I210 e corrige bug de memória",
  "kernel": "/build/rust-kernel/target/x86_64-unknown-none/release/hybridos-kernel",
  "files": {
    "/etc/hybridos/system.toml": "Y29ubmVjdGlvbl9kb21haW49InRlbWVsZS5sb2NhbCIK",
    "/drivers/net/e1000_ext.sys": "UEsDBBQAAAAIA..."
  },
  "reboot_required": true
}
```

### Passo 2: Instalar o pacote

No terminal do HybridOS:

```bash
hyb-update install /caminho/para/minha_atualizacao.hybupdate
```

O que acontece:
1. O sistema lê o pacote `.hybupdate`
2. Cria automaticamente um **ponto de restauração** (cópia do kernel atual e VFS)
3. Escreve os novos arquivos no VFS
4. Se houver um novo kernel, grava em `/boot/hybridos-kernel`
5. Marca a atualização como pendente no `/update/pending/`

### Passo 3: Reiniciar o sistema

```bash
reboot
# ou
shutdown
```

Durante o boot, o kernel detecta a atualização pendente e:
1. Aplica o novo kernel e arquivos
2. Se o novo kernel iniciar com sucesso → sistema continua normalmente
3. Se houver kernel panic ou falha → **rollback automático** para o ponto de restauração anterior

---

## 4. Comandos Disponíveis

| Comando | Descrição |
|---------|-----------|
| `hyb-update check` | Verificar se há atualizações pendentes |
| `hyb-update install <arquivo.hybupdate>` | Instalar um pacote de atualização |
| `hyb-update rollback` | Voltar para o último ponto de restauração |
| `hyb-update list` | Listar pontos de restauração disponíveis |
| `hyb-update status` | Mostrar status atual (pendente ou não) |
| `hyb-update create-point` | Criar ponto de restauração manual |
| `hyb-update help` | Mostrar ajuda |

---

## 5. Estrutura de Diretórios do Sistema de Atualização

```
/update/
├── pending/
│   └── package.hybupdate    ← Atualização aguardando reboot
├── restore/
│   ├── restore_001/         ← Ponto de restauração 1
│   │   ├── kernel.bin
│   │   └── vfs_snapshot/
│   ├── restore_002/         ← Ponto de restauração 2
│   │   ├── kernel.bin
│   │   └── vfs_snapshot/
│   └── history.json         → Histórico de atualizações aplicadas
/boot/
├── hybridos-kernel          ← Kernel atual (ativo)
└── hybridos-kernel.backup   ← Último ponto de restauração (kernel)
```

---

## 6. Exemplo Prático Completo

### Cenário: Atualizar o kernel e adicionar um novo aplicativo

**Passo 1: Criar o binário do kernel**
```bash
cd rust-kernel
cargo build --release --target x86_64-unknown-none
cp target/x86_64-unknown-none/release/hybridos-kernel /tmp/novo_kernel.bin
```

**Passo 2: Criar o arquivo JSON do pacote**

Salve como `minha_atualizacao.hybupdate`:
```json
{
  "version": "1.2.0",
  "description": "Nova versão do kernel com suporte a NVMe e correção de bug gráfico",
  "kernel": "/tmp/novo_kernel.bin",
  "files": {
    "/etc/hybridos/system.toml": "Y29ubmVjdGlvbl9kb21haW49InRlbWVsZS5sb2NhbCIKdGVtcF9kaXI9Ii90bXAiCg==",
    "/usr/bin/nova_utilidade": "UEsDBBQAAAAIA..."
  },
  "reboot_required": true
}
```

**Passo 3: Instalar no HybridOS**
```bash
hyb-update install /tmp/minha_atualizacao.hybupdate
```

Saída esperada:
```
============================================================
  hybridOS Update Installer
============================================================

[PACOTE] Versão: 1.2.0
[PACOTE] Descrição: Nova versão do kernel com suporte a NVMe...
[PACOTE] Arquivos: 2
[PACOTE] Kernel: /tmp/novo_kernel.bin

[RESTORE] Criando ponto de restauração...
[RESTORE] Ponto de restauração criado.

[APLICANDO] Gravando arquivos...
  Gravando: /etc/hybridos/system.toml
    [OK] Gravado com sucesso.
  Gravando: /usr/bin/nova_utilidade
    [OK] Gravado com sucesso.

[KERNEL] Gravando novo kernel em /boot/hybridos-kernel...
[KERNEL] Kernel gravado com sucesso.

[PENDENTE] Marcando atualização para o próximo boot...
[PENDENTE] Atualização marcada com sucesso.

[CONCLUSÃO] Pacote instalado com sucesso!
            Reinicie o sistema para aplicar a atualização.
            Use: hyb-update rollback para desfazer.
```

**Passo 4: Reiniciar**
```bash
reboot
```

Durante o boot, o kernel aplicará automaticamente o novo kernel e arquivos.

---

## 7. Rollback (Desfazer Atualização)

Se algo der errado após a atualização:

```bash
hyb-update rollback
```

Isso restaurará o kernel do backup em `/boot/hybridos-kernel.backup` e desfazerá as alterações nos arquivos VFS.

Após o rollback, reinicie o sistema:
```bash
reboot
```

---

## 8. Regras Importantes

### O que NÃO é sobrescrito durante atualização:
- `/home/` — arquivos pessoais do usuário
- `/Users/` — dados do usuário Windows-style
- `/etc/hybridos/system.toml` — **somente se estiver no pacote** (o usuário escolhe)
- Arquivos em `/tmp/` — são temporários

### O que PODE ser atualizado:
- `/boot/hybridos-kernel` — kernel do sistema
- `/etc/hybridos/` — configurações do sistema (se especificado no pacote)
- `/bin/`, `/sbin/`, `/usr/bin/` — aplicativos do sistema
- Drivers de hardware
- Qualquer arquivo especificado no pacote `.hybupdate`

---

## 9. Instruções para o Agente de IA (Opencode, Gemini, Claude Code, etc.)

Se você está usando um agente de IA para criar atualizações, siga este fluxo:

### Passo 1: Ler o código existente
```
Leia todo o código-fonte em:
- rust-kernel/src/ (kernel)
- userland/apps/ (aplicativos)
- etc/hybridos/system.toml (configurações)
```

### Passo 2: Fazer as alterações necessárias
Edite os arquivos necessários para implementar a funcionalidade solicitada.

### Passo 3: Compilar o kernel (se alterou kernel)
```bash
cd rust-kernel
cargo build --release --target x86_64-unknown-none
```

### Passo 4: Criar o pacote .hybupdate
1. Leia o novo binário do kernel e converta para Base64
2. Leia quaisquer arquivos de configuração modificados e converta para Base64
3. Crie o arquivo JSON seguindo o formato descrito acima
4. Salve com extensão `.hybupdate`

### Passo 5: Testar no HybridOS
```bash
hyb-update install seu_pacote.hybupdate
reboot
```

### Passo 6: Verificar se funcionou
Após o reinício, verifique se a nova funcionalidade está ativa. Se não estiver:
```bash
hyb-update rollback
reboot
```

---

## 10. Exemplo de Fluxo Completo com IA

**Usuário:** "Adicione suporte a leitura de arquivos ZIP no kernel"

**IA faz:**
1. Lê `rust-kernel/src/fs/mod.rs` e `rust-kernel/src/vfs/`
2. Adiciona suporte a descompactar ZIP em `rust-kernel/src/fs/zip_reader.rs`
3. Compila: `cargo build --release --target x86_64-unknown-none`
4. Cria pacote:
   ```json
   {
     "version": "1.3.0",
     "description": "Adiciona suporte a leitura de arquivos ZIP no VFS",
     "kernel": "/build/kernel.bin",
     "files": {},
     "reboot_required": true
   }
   ```
5. Salva como `zip_support.hybupdate`

**Usuário executa:**
```bash
hyb-update install zip_support.hybupdate
reboot
```

**Resultado:** O sistema reinicia com suporte a ZIP. Se algo der errado:
```bash
hyb-update rollback
reboot
```

---

## 11. Solução de Problemas

| Problema | Solução |
|----------|---------|
| `hyb-update install` diz "Não foi possível ler o arquivo" | Verifique se o caminho está correto e o arquivo existe |
| `[ERRO] Não foi possível parsear o pacote` | Verifique se o JSON está válido e as aspas estão corretas |
| `[ERRO] Não foi ler o kernel do pacote` | Verifique se o caminho do kernel no JSON está correto |
| Após reboot, o sistema não inicia | Use `hyb-update rollback` em outro sistema ou reinicie e espere o rollback automático |
| Arquivos do usuário sumiram | Os arquivos em `/home/` e `/Users/` NUNCA são tocados pelo updater |

---

## 12. Notas de Segurança

- **Sempre crie um ponto de restauração antes de atualizar** (o sistema faz isso automaticamente)
- **Não modifique arquivos em `/home/` ou `/Users/`** via pacotes de atualização
- **Teste em máquina virtual (QEMU) antes de instalar em hardware real**
- **Mantenha backups externos de dados importantes**

---

Fim do guia.
