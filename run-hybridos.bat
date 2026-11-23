@echo off
setlocal
title HybridOS - QEMU Virtual Machine

:: Diretório raiz do projeto
set PROJECT_ROOT=%~dp0
cd /d "%PROJECT_ROOT%"

set QEMU_BIN=.\qemu\bin\qemu-system-x86_64.exe
set QEMU_SHARE=.\qemu\bin\share
set DISK_IMG=.\qemu\hybridos_system.img
set SATA_IMG=.\qemu\hybridos_sata.img
set NVME_IMG=.\qemu\hybridos_nvme.img
set SERIAL_LOG=.\hybrid_serial.log

echo ================================================================
echo   Launching HybridOS Kernel on QEMU x86_64
echo ================================================================

if not exist "%QEMU_BIN%" (
    echo [ERROR] QEMU executable not found at %QEMU_BIN%
    pause
    exit /b 1
)

if not exist "%DISK_IMG%" (
    echo [ERROR] Disk image not found at %DISK_IMG%
    echo         Run: python qemu\build_disk.py
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
    -display gtk ^
    -serial file:%SERIAL_LOG% ^
    -device ahci,id=ahci0 ^
    -drive file=%DISK_IMG%,format=raw,if=none,id=bootdisk ^
    -device ide-hd,drive=bootdisk,bus=ahci0.0,bootindex=1 ^
    -drive file=%SATA_IMG%,format=raw,if=none,id=sata0 ^
    -device ide-hd,drive=sata0,bus=ahci0.1,bootindex=2 ^
    -drive file=%NVME_IMG%,format=raw,if=none,id=nvm0 ^
    -device nvme,serial=HYBRIDOS_NVME0,drive=nvm0 ^
    -device virtio-net-pci,netdev=net0,mac=52:54:00:12:34:56 ^
    -netdev user,id=net0

echo.
echo [HybridOS] QEMU session terminated.
pause
