#!/usr/bin/env bash
# scripts/audit_no_stubs.sh — Auditoria Estática Anti-Stub e Validação de Conformidade
set -euo pipefail

TARGET_DIR="${1:-.}"
echo "======================================================================"
echo "    AUDITORIA ESTÁTICA ANTI-STUB & DEFINITION OF DONE: ${TARGET_DIR}   "
echo "======================================================================"

FAILURES=0

# 1. Verificar arquivos vazios ou com menos de 20 linhas úteis (exceto mod.rs)
echo "[1/3] Verificando tamanho mínimo e ausência de arquivos vazios..."
while IFS= read -r file; do
    fname=$(basename "$file")
    if [[ "$fname" == "mod.rs" ]]; then
        continue
    fi
    # Contar linhas que não sejam vazias nem apenas comentários de barra dupla
    useful_lines=$(grep -v '^[[:space:]]*$' "$file" | grep -v '^[[:space:]]*//' | wc -l)
    if [[ "$useful_lines" -lt 20 ]]; then
        echo "  [ERRO] Arquivo com poucas linhas úteis ($useful_lines < 20): $file"
        FAILURES=$((FAILURES + 1))
    fi
done < <(find "$TARGET_DIR" -type f -name "*.rs" ! -path "*/target/*")

# 2. Verificar ocorrências de TODO, FIXME, unimplemented!, todo!, placeholder, stub, mock_data, fake_
echo "[2/3] Verificando proibição absoluta de stubs, placeholders e TODOs..."
FORBIDDEN_PATTERN='(TODO|FIXME|unimplemented!|todo!|placeholder|stub|mock_data|fake_)'
MATCHES=$(find "$TARGET_DIR" -type f \( -name "*.rs" -o -name "*.toml" \) ! -path "*/target/*" -exec grep -HnEI "$FORBIDDEN_PATTERN" {} + || true)

if [[ -n "$MATCHES" ]]; then
    echo "  [ERRO] Ocorrências proibidas detectadas no código de produção:"
    echo "$MATCHES" | head -n 30
    FAILURES=$((FAILURES + 1))
else
    echo "  [OK] Zero ocorrências de termos proibidos."
fi

# 3. Verificar presença de binários compilados
echo "[3/3] Verificando executáveis compilados..."
EXPECTED_BINARIES=(
    "hyb-compositor"
    "hyb-shell"
    "hyb-files"
    "hyb-edit"
    "hyb-calc"
    "hyb-capture"
    "hyb-clip"
    "hyb-chars"
    "hyb-fonts"
    "hyb-paint"
    "hyb-rdp"
    "hyb-vault"
    "hyb-cert-user"
    "hyb-cert-sys"
    "hyb-focus"
    "hyb-pdf"
    "hyb-zoom"
)

BIN_FAIL=0
for bin in "${EXPECTED_BINARIES[@]}"; do
    FOUND=0
    if [[ -f "target/release/${bin}" || -f "target/release/${bin}.exe" || -f "target/x86_64-unknown-linux-musl/release/${bin}" ]]; then
        FOUND=1
    fi
    if [[ $FOUND -eq 0 ]]; then
        echo "  [AVISO/ERRO] Binário esperado não encontrado: ${bin}"
        BIN_FAIL=$((BIN_FAIL + 1))
    fi
done

if [[ $BIN_FAIL -gt 0 ]]; then
    echo "  [INFO] $BIN_FAIL binários ainda não compilados."
    FAILURES=$((FAILURES + BIN_FAIL))
else
    echo "  [OK] Todos os 17 binários compilados estão presentes."
fi

echo "======================================================================"
if [[ $FAILURES -gt 0 ]]; then
    echo "  [RESULTADO: FALHA] Foram encontradas $FAILURES violações estáticas."
    exit 1
else
    echo "  [RESULTADO: SUCESSO] Auditoria estática concluída com 0 violações."
    exit 0
fi
