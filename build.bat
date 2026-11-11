@echo off
setlocal enabledelayedexpansion

echo ================================================================
echo Building HybridOS Kernel Core (x64)
echo ================================================================

call "C:\Program Files (x86)\Microsoft Visual Studio\18\BuildTools\VC\Auxiliary\Build\vcvarsall.bat" x64

cd /d "C:\Users\Vinicius\Documents\HybridOS-Kernel"

if not exist bin mkdir bin

cl.exe /nologo /W3 /O2 /I include ^
    kernel\main.c ^
    kernel\process.c ^
    arch\x86_64\syscall_dispatcher.c ^
    subsystems\win_subsystem.c ^
    subsystems\linux_subsystem.c ^
    loaders\pe_loader.c ^
    loaders\elf_loader.c ^
    drivers\vdm_bridge.c ^
    /Fe:bin\hybridos_core.exe /Fo:bin\

if %ERRORLEVEL% equ 0 (
    echo.
    echo ================================================================
    echo Build SUCCESS! Output executable: bin\hybridos_core.exe
    echo ================================================================
) else (
    echo.
    echo Build FAILED with code %ERRORLEVEL%
)
