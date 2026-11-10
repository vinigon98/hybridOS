@echo off
setlocal
title HybridOS - QEMU Virtual Machine
cd /d "%~dp0"

echo ================================================================
echo   Launching HybridOS Kernel on QEMU x86_64
echo ================================================================

set QEMU_BIN=.\bin\qemu-system-x86_64.exe
set QEMU_SHARE=.\bin\share

if not exist "%QEMU_BIN%" (
    echo [ERROR] QEMU executable not found at %QEMU_BIN%
    pause
    exit /b 1
)

"%QEMU_BIN%" ^
    -L "%QEMU_SHARE%" ^
    -machine q35,accel=tcg ^
    -cpu max ^
    -m 2048M ^
    -smp 4 ^
    -vga std ^
    -device virtio-gpu-pci ^
    -serial stdio ^
    -device ahci,id=ahci0 ^
    -drive file=hybridos_system.img,format=raw,if=none,id=bootdisk ^
    -device ide-hd,drive=bootdisk,bus=ahci0.0,bootindex=1 ^
    -drive file=hybridos_sata.img,format=raw,if=none,id=sata0 ^
    -device ide-hd,drive=sata0,bus=ahci0.1,bootindex=2 ^
    -drive file=hybridos_nvme.img,format=raw,if=none,id=nvm0 ^
    -device nvme,serial=HYBRIDOS_NVME0,drive=nvm0 ^
    -device virtio-net-pci,netdev=net0,mac=52:54:00:12:34:56 ^
    -netdev user,id=net0

echo.
echo [HybridOS] QEMU session terminated.
pause
