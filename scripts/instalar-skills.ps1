$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")
if (-not (Get-Command npx -ErrorAction SilentlyContinue)) { throw "Instale Node.js/npm para usar npx" }
$upstream = "mattpocock/skills@abeeaf95660ba9e8b7c753eb38f441ca707a8823"
Write-Host "Revise o código de $upstream antes de executar ferramentas externas."
npx --yes skills@latest add $upstream -a codex -a claude-code --copy -y --skill setup-matt-pocock-skills grill-with-docs to-spec to-tickets implement implement-spec tdd code-review prototype research diagnosing-bugs domain-modeling retro pr
if ($LASTEXITCODE -ne 0) { throw "Falha na instalação das skills" }
git status --short
