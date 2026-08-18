$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

Set-Location (Resolve-Path (Join-Path $PSScriptRoot ".."))
$daemonPath = (Resolve-Path -LiteralPath ".\target\debug\alcomd.exe").Path
$cliPath = (Resolve-Path -LiteralPath ".\target\debug\alcomd-cli.exe").Path
$existing = @(
    Get-Process -Name "alcomd" -ErrorAction SilentlyContinue |
        Where-Object { $_.Path -eq $daemonPath }
)
if ($existing.Count -ne 0) {
    throw "A matching development daemon is already running."
}

$token = [Guid]::NewGuid().ToString("N")
$outputs = @(
    (Join-Path $env:TEMP "alcomd-m1-$token-1.out"),
    (Join-Path $env:TEMP "alcomd-m1-$token-1.err"),
    (Join-Path $env:TEMP "alcomd-m1-$token-2.out"),
    (Join-Path $env:TEMP "alcomd-m1-$token-2.err")
)

try {
    $first = Start-Process -FilePath $cliPath `
        -ArgumentList @("--json", "system", "status") `
        -RedirectStandardOutput $outputs[0] `
        -RedirectStandardError $outputs[1] `
        -PassThru `
        -WindowStyle Hidden
    $second = Start-Process -FilePath $cliPath `
        -ArgumentList @("--json", "system", "status") `
        -RedirectStandardOutput $outputs[2] `
        -RedirectStandardError $outputs[3] `
        -PassThru `
        -WindowStyle Hidden
    $first.WaitForExit()
    $second.WaitForExit()
    if ($first.ExitCode -ne 0 -or $second.ExitCode -ne 0) {
        throw "Concurrent CLI startup failed: $($first.ExitCode), $($second.ExitCode)."
    }

    $daemons = @(
        Get-Process -Name "alcomd" -ErrorAction SilentlyContinue |
            Where-Object { $_.Path -eq $daemonPath }
    )
    if ($daemons.Count -ne 1) {
        throw "Expected one authoritative daemon; found $($daemons.Count)."
    }
    $firstStatus = Get-Content -Raw -LiteralPath $outputs[0] | ConvertFrom-Json
    $secondStatus = Get-Content -Raw -LiteralPath $outputs[2] | ConvertFrom-Json
    if ($firstStatus.state -ne "ready" -or $secondStatus.state -ne "ready") {
        throw "One or more concurrent clients did not receive ready status."
    }
    Write-Host "M1 concurrent daemon auto-start passed; authoritative daemon count: 1."
}
finally {
    @(
        Get-Process -Name "alcomd" -ErrorAction SilentlyContinue |
            Where-Object { $_.Path -eq $daemonPath }
    ) | Stop-Process -Force -ErrorAction SilentlyContinue
    foreach ($path in $outputs) {
        if (Test-Path -LiteralPath $path -PathType Leaf) {
            Remove-Item -LiteralPath $path -Force
        }
    }
}
