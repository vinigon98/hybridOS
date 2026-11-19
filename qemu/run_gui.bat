@echo off
setlocal
title HybridOS - QEMU Graphical Machine
cd /d "%~dp0"

set QEMU_BIN=.\bin\qemu-system-x86_64.exe
set QEMU_SHARE=.\bin\share

if not exist "%QEMU_BIN%" (
    echo [ERROR] QEMU executable not found at %QEMU_BIN%
    pause
    exit /b 1
)

"%QEMU_BIN%" ^
    -L "%QEMU_SHARE%" ^
    -machine pc,accel=tcg ^
    -cpu max ^
    -m 1024M ^
    -drive file=hybridos_system.img,format=raw,if=ide ^
    -drive file=hybridos_nvme.img,format=raw,if=none,id=nvm0 ^
    -device nvme,serial=HYBRIDOS_NVME0,drive=nvm0 ^
    -netdev user,id=net0 ^
    -device e1000,netdev=net0,mac=52:54:00:12:34:56 ^
    -vga std ^
    -display gtk

pause
