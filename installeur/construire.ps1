# Construit l'installeur Windows d'AG03 MK2 Studio : compilation, signature, installeur, signature.
# Utilisation (depuis le dossier ag03ctl) :  powershell -ExecutionPolicy Bypass -File installeur\construire.ps1
$ErrorActionPreference = "Stop"
Set-Location (Split-Path $PSScriptRoot -Parent)

Write-Host "1/4 Compilation en mode release..." -ForegroundColor Cyan
cargo build --release
if ($LASTEXITCODE -ne 0) { throw "La compilation a échoué." }

# Inno Setup : installé par  winget install JRSoftware.InnoSetup
$iscc = @("$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe", "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe", "$env:ProgramFiles\Inno Setup 6\ISCC.exe") |
    Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $iscc) { throw "Inno Setup introuvable. Installe-le avec :  winget install JRSoftware.InnoSetup" }

Write-Host "2/4 Signature des exécutables..." -ForegroundColor Cyan
& "$PSScriptRoot\signer.ps1" "target\release\ag03studio.exe", "target\release\ag03ctl.exe"

Write-Host "3/4 Création de l'installeur..." -ForegroundColor Cyan
& $iscc "installeur\ag03studio.iss"
if ($LASTEXITCODE -ne 0) { throw "Inno Setup a échoué." }

Write-Host "4/4 Signature de l'installeur..." -ForegroundColor Cyan
$setup = Get-ChildItem "dist\AG03-MK2-Studio-*-installeur.exe" | Sort-Object LastWriteTime | Select-Object -Last 1
& "$PSScriptRoot\signer.ps1" $setup.FullName

Write-Host "Terminé : $($setup.FullName)" -ForegroundColor Green
