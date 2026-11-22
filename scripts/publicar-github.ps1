$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")
if (-not (Get-Command gh -ErrorAction SilentlyContinue)) { throw "GitHub CLI requerido" }
if (-not (Get-Command git -ErrorAction SilentlyContinue)) { throw "Git requerido" }
gh auth status
if ($LASTEXITCODE -ne 0) { throw "Autentique GitHub CLI" }
if (-not (Test-Path .git)) { git init -b main }
if (-not (git remote)) { git remote add origin https://github.com/ChicoFigueiredo/Piano-Tutor.git }
git fetch origin main
git pull --ff-only origin main
Write-Host "Repositório sincronizado. Revise as alterações antes de fazer push."
git status --short
