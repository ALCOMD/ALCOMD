Set-Location (Resolve-Path (Join-Path $PSScriptRoot ".."))
. (Join-Path $PSScriptRoot "common.ps1")

Assert-RepositoryToolchain
Assert-WindowsTauriPrerequisites
$lockSnapshot = Get-LockFileSnapshot
npm ci
Assert-LockFileSnapshot $lockSnapshot

Write-Host "Setup complete. Run .\scripts\check.ps1"
