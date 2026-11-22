$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")
if (-not (Get-Command gh -ErrorAction SilentlyContinue)) { throw "GitHub CLI requerido" }
Get-ChildItem backlog/fases/F*.md | Sort-Object Name | ForEach-Object {
  $title = ((Get-Content $_.FullName -TotalCount 1) -replace '^# ', '')
  $titles = gh issue list --state all --search "$title in:title" --json title --jq '.[].title'
  if ($titles -contains $title) { Write-Host "Já existe: $title" } else { gh issue create --title $title --body-file $_.FullName }
}
