param(
    [string]$V3Path = "..\ALCOMD3-v3-readonly",
    [string]$VrcGetPath = "..\vrc-get-readonly"
)

$ErrorActionPreference = "Stop"
Set-Location (Resolve-Path (Join-Path $PSScriptRoot ".."))

foreach ($entry in @(
    @{ Name = "ALCOMD3 v3"; Path = $V3Path },
    @{ Name = "vrc-get"; Path = $VrcGetPath }
)) {
    if (-not (Test-Path $entry.Path)) {
        throw "$($entry.Name) repository not found: $($entry.Path)"
    }

    $commit = git -C $entry.Path rev-parse HEAD
    $tag = git -C $entry.Path describe --tags --exact-match 2>$null
    Write-Host "$($entry.Name):"
    Write-Host "  commit = $commit"
    Write-Host "  tag    = $tag"
}

Write-Host "Review the values, then update docs/baselines/source-lock.toml manually."
