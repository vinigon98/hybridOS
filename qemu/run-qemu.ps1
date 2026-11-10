# HybridOS QEMU Launcher (PowerShell)
# Launches HybridOS Kernel with full hardware emulation (NVMe, AHCI SATA, VirtIO-GPU, VirtIO-Net)

$ErrorActionPreference = "Stop"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
Set-Location $ScriptDir

$QemuBin = Join-Path $ScriptDir "bin\qemu-system-x86_64.exe"
$QemuShare = Join-Path $ScriptDir "bin\share"

if (-not (Test-Path $QemuBin)) {
    Write-Error "QEMU executable not found at: $QemuBin"
    exit 1
}

Write-Host "================================================================" -ForegroundColor Cyan
Write-Host "  HybridOS Kernel - Launching on QEMU x86_64" -ForegroundColor Cyan
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host "CPU: max (x86_64 Long Mode, 4 vCPUs)"
Write-Host "RAM: 2048 MB"
Write-Host "Display: VirtIO-GPU PCI (1024x768x32) + VGA Standard"
Write-Host "Storage 1: Boot Disk (hybridos_system.img) on AHCI Port 0 [Boot Priority 1]"
Write-Host "Storage 2: SATA AHCI Drive (hybridos_sata.img) on AHCI Port 1"
Write-Host "Storage 3: NVMe 1.4 PCI Controller (hybridos_nvme.img)"
Write-Host "Network: VirtIO-Net Gigabit Adapter (MAC 52:54:00:12:34:56)"
Write-Host "Serial: COM1 mapped to stdio (115200 baud)"
Write-Host "----------------------------------------------------------------"
Write-Host "Tip: Type 'help' in the shell for command list."
Write-Host "Tip: Type 'powershell', 'cmd', 'bash', or 'hybrid' to switch flavors."
Write-Host "Tip: Type 'exit' to shut down the virtual machine.`n" -ForegroundColor Yellow

$qemuArgs = @(
    "-L", $QemuShare,
    "-machine", "q35,accel=tcg",
    "-cpu", "max",
    "-m", "2048M",
    "-smp", "4",
    "-vga", "std",
    "-device", "virtio-gpu-pci",
    "-serial", "stdio",
    "-device", "ahci,id=ahci0",
    "-drive", "file=hybridos_system.img,format=raw,if=none,id=bootdisk",
    "-device", "ide-hd,drive=bootdisk,bus=ahci0.0,bootindex=1",
    "-drive", "file=hybridos_sata.img,format=raw,if=none,id=sata0",
    "-device", "ide-hd,drive=sata0,bus=ahci0.1,bootindex=2",
    "-drive", "file=hybridos_nvme.img,format=raw,if=none,id=nvm0",
    "-device", "nvme,serial=HYBRIDOS_NVME0,drive=nvm0",
    "-device", "virtio-net-pci,netdev=net0,mac=52:54:00:12:34:56",
    "-netdev", "user,id=net0"
)

& $QemuBin @qemuArgs
