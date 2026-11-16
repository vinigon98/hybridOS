#!/usr/bin/env bash
# ==============================================================================
# verify_kernel.sh - Deterministic Verification Pipeline for HybridOS Kernel
# ==============================================================================
# Pipeline:
#   1. Lint & Compilação (cargo check, cargo clippy -- -D warnings, cargo build, build_disk.py)
#   2. Execução Headless no QEMU com Timeout e Captura de Logs
#   3. Validação Automática do Log (PANIC, Triple fault, Exception, [BOOT_STAGE_OK])
# Returns:
#   0 -> Success
#   1 -> Failure
# ==============================================================================

# Find project root directory relative to this script
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

cd "$REPO_ROOT" || exit 1

echo "================================================================="
echo ">>> [STAGE 1/3] Lint & Compilation..."
echo "================================================================="

# 1.1 cargo check
echo "[+] Running cargo check..."
if ! (cd "$REPO_ROOT/rust-kernel" && cargo check); then
    echo "[-] [FAIL] cargo check failed!"
    exit 1
fi

# 1.2 cargo clippy -- -D warnings
echo "[+] Running cargo clippy -- -D warnings..."
if ! (cd "$REPO_ROOT/rust-kernel" && cargo clippy -- -D warnings); then
    echo "[-] [FAIL] cargo clippy failed!"
    exit 1
fi

# 1.3 Compile kernel binary
echo "[+] Building kernel binary..."
if ! (cd "$REPO_ROOT/rust-kernel" && cargo build); then
    echo "[-] [FAIL] cargo build failed!"
    exit 1
fi

# 1.4 Generate bootable disk image
echo "[+] Generating bootable system disk image..."
if ! python "$REPO_ROOT/qemu/build_disk.py"; then
    echo "[-] [FAIL] python qemu/build_disk.py failed!"
    exit 1
fi

echo "================================================================="
echo ">>> [STAGE 2/3] Headless QEMU Execution & Telemetry Capture..."
echo "================================================================="

mkdir -p "$REPO_ROOT/logs"
rm -f "$REPO_ROOT/logs/serial_latest.log"
rm -f "$REPO_ROOT/logs/qemu_cpu.log"

# Locate QEMU binary
QEMU_BIN="$REPO_ROOT/qemu/bin/qemu-system-x86_64.exe"
QEMU_SHARE="$REPO_ROOT/qemu/bin/share"

if [ ! -f "$QEMU_BIN" ]; then
    if command -v qemu-system-x86_64 &> /dev/null; then
        QEMU_BIN="qemu-system-x86_64"
        QEMU_SHARE=""
    else
        echo "[-] [FAIL] qemu-system-x86_64 not found!"
        exit 1
    fi
fi

SYS_IMG="$REPO_ROOT/qemu/hybridos_system.img"
NVME_IMG="$REPO_ROOT/qemu/hybridos_nvme.img"
SATA_IMG="$REPO_ROOT/qemu/hybridos_sata.img"

QEMU_ARGS=(
    -machine pc,accel=tcg
    -cpu max
    -m 1024M
    -drive "file=$SYS_IMG,format=raw,if=ide"
    -drive "file=$NVME_IMG,format=raw,if=none,id=nvm0"
    -device nvme,serial=HYBRIDOS_NVME0,drive=nvm0
    -drive "file=$SATA_IMG,format=raw,if=none,id=sata0"
    -device ahci,id=ahci0
    -device ide-hd,drive=sata0,bus=ahci0.0
    -netdev user,id=net0
    -device e1000,netdev=net0,mac=52:54:00:12:34:56
    -vga std
    -display none
    -serial "file:logs/serial_latest.log"
    -no-reboot
    -d int,cpu_reset
    -D "logs/qemu_cpu.log"
    -device isa-debug-exit,iobase=0xf4,iosize=0x04
)

if [ -n "$QEMU_SHARE" ] && [ -d "$QEMU_SHARE" ]; then
    QEMU_ARGS=(-L "$QEMU_SHARE" "${QEMU_ARGS[@]}")
fi

TIMEOUT_VAL="${KERNEL_TIMEOUT:-25s}"
echo "[*] Spawning QEMU with ${TIMEOUT_VAL} timeout..."
timeout "${TIMEOUT_VAL}" "$QEMU_BIN" "${QEMU_ARGS[@]}" > /dev/null 2>&1 || true

echo "================================================================="
echo ">>> [STAGE 3/3] Telemetry & Log Verification..."
echo "================================================================="

LOG_FILE="$REPO_ROOT/logs/serial_latest.log"
CPU_LOG="$REPO_ROOT/logs/qemu_cpu.log"

if [ ! -f "$LOG_FILE" ]; then
    echo "[-] [FAIL] Log file $LOG_FILE was not generated!"
    exit 1
fi

HAS_PANIC=0
HAS_TRIPLE_FAULT=0
HAS_UNHANDLED_EXCEPTION=0
HAS_BOOT_OK=0

if grep -q "PANIC" "$LOG_FILE"; then
    HAS_PANIC=1
fi

if grep -q "Triple fault" "$LOG_FILE"; then
    HAS_TRIPLE_FAULT=1
fi

if grep -qi "Exception:" "$LOG_FILE"; then
    HAS_UNHANDLED_EXCEPTION=1
fi

if grep -q "\[BOOT_STAGE_OK\]" "$LOG_FILE"; then
    HAS_BOOT_OK=1
fi

if [ "$HAS_PANIC" -eq 1 ] || [ "$HAS_TRIPLE_FAULT" -eq 1 ] || [ "$HAS_UNHANDLED_EXCEPTION" -eq 1 ] || [ "$HAS_BOOT_OK" -eq 0 ]; then
    echo "[-] ============================================================="
    echo "[-] VERIFICATION FAILED:"
    [ "$HAS_PANIC" -eq 1 ] && echo "[-]   -> Kernel PANIC detected!"
    [ "$HAS_TRIPLE_FAULT" -eq 1 ] && echo "[-]   -> CPU Triple fault detected!"
    [ "$HAS_UNHANDLED_EXCEPTION" -eq 1 ] && echo "[-]   -> Unhandled CPU Exception detected!"
    [ "$HAS_BOOT_OK" -eq 0 ] && echo "[-]   -> [BOOT_STAGE_OK] token missing within ${TIMEOUT_VAL} timeout!"
    echo "[-] ============================================================="
    echo "[-] Last 40 lines of serial_latest.log:"
    echo "-----------------------------------------------------------------"
    tail -n 40 "$LOG_FILE"
    echo "-----------------------------------------------------------------"
    if [ -f "$CPU_LOG" ]; then
        echo "[-] Last 40 lines of qemu_cpu.log:"
        echo "-----------------------------------------------------------------"
        tail -n 40 "$CPU_LOG"
        echo "-----------------------------------------------------------------"
    fi
    exit 1
fi

echo "[+] ============================================================="
echo "[+] [BOOT_STAGE_OK] verified successfully!"
echo "[+] Zero kernel panics, zero unhandled exceptions, zero triple faults."
echo "[+] Verification PASSED with exit code 0."
echo "[+] ============================================================="
exit 0
