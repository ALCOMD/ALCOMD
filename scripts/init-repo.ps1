param(
    [switch]$CreateInitialCommit
)

$ErrorActionPreference = "Stop"
Set-Location (Resolve-Path (Join-Path $PSScriptRoot ".."))

if (-not (Test-Path ".git")) {
    git init -b main
}

git status --short

if ($CreateInitialCommit) {
    git add --all
    git commit -m "chore: initialize ALCOMD v4 repository"
}
