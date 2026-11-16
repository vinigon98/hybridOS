# Skill: MMU e Gerenciamento de Memória Física/Virtual no HybridOS

## Invariantes Fundamentais de Hardware x86_64
1. **Unidade de Alocação da MMU**: A granularidade padrão da MMU x86_64 é de 4096 bytes (4 KiB), alinhada em fronteira de 4 KiB (`addr & 0xFFF == 0`).
2. **Endereçamento Canônico de 48 bits**:
   - Lower-Half (Espaço de Usuário Ring 3): `0x0000_0000_0000_0000` até `0x0000_7FFF_FFFF_FFFF`.
   - Buraco Não-Canônico: `0x0000_8000_0000_0000` até `0xFFFF_7FFF_FFFF_FFFF` (dispara `#GP` se acessado).
   - Higher-Half (Espaço de Kernel Ring 0): `0xFFFF_8000_0000_0000` até `0xFFFF_FFFF_FFFF_FFFF`.
3. **Higher-Half Direct Map (HHDM)**:
   - Fornecido pelo protocolo Limine (`HHDM_REQUEST`).
   - Permite que o kernel acesse qualquer frame de memória física `phys` através de:
     `virt_addr = phys_addr + hhdm_offset`
   - O HHDM deve ser mapeado na metade superior de todas as PML4s ativas do sistema.

## Estrutura da PML4 Física (4 Níveis)
- **Tamanho**: Exatamente 4096 bytes (512 entradas de 64 bits = 8 bytes cada).
- **Entradas**:
  - `PML4` (Level 4): Bits `[47:39]` do endereço virtual (cobre 512 GiB por entrada).
  - `PDPT` (Level 3): Bits `[38:30]` do endereço virtual (cobre 1 GiB por entrada).
  - `PD`   (Level 2): Bits `[29:21]` do endereço virtual (cobre 2 MiB por entrada).
  - `PT`   (Level 1): Bits `[20:12]` do endereço virtual (cobre 4 KiB por entrada).
- **Bits de Controle nas Entradas**:
  - Bit 0 (`P`): Present. Se 0, qualquer acesso dispara `#PF`.
  - Bit 1 (`R/W`): Read/Write. Se 0, escrita dispara `#PF`.
  - Bit 2 (`U/S`): User/Supervisor. Se 1, acessível em Ring 3. Se 0, restrito a Ring 0.
  - Bit 3 (`PWT`): Page-level Write-Through.
  - Bit 4 (`PCD`): Page-level Cache Disable (para MMIO e buffers de dispositivos).
  - Bit 5 (`A`): Accessed (setado pelo hardware ao ler/escrever).
  - Bit 6 (`D`): Dirty (setado pelo hardware ao escrever na página de 4K).
  - Bit 7 (`PAT` / `PS`): Page Size (1 = 2MB/1GB large page em PD/PDPT) ou PAT bit em PT.
  - Bit 8 (`G`): Global (não invalida TLB em recarga de CR3; usado para o Kernel Higher-Half).
  - Bits `[51:12]`: Endereço físico do próximo nível ou da página de dados (máscara `0x000F_FFFF_FFFF_F000`).
  - Bit 63 (`NX`): No-Execute (se bit `EFER.NXE == 1`, execução nesta página dispara `#PF` com código `INSTRUCTION`).

## Ativação de CR3 e Invalidação de TLB
- **Carga de CR3**:
  ```rust
  unsafe {
      core::arch::asm!("mov cr3, {}", in(reg) phys_pml4_addr, options(nostack, preserves_flags));
  }
  ```
- **Invalidação de TLB**:
  Ao criar, modificar permissões ou remover qualquer mapeamento de página virtual `va`:
  ```rust
  unsafe {
      core::arch::asm!("invlpg [{}]", in(reg) va, options(nostack, preserves_flags));
  }
  ```
- **Proibição Absoluta**: Nunca usar estruturas de alto nível como `BTreeMap` ou arrays em heap para simular a tabela de páginas. Todas as tabelas devem ser frames físicos reais alocados no Physical Frame Allocator e acessados via HHDM.
