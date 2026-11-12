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
$temporaryRoot = [IO.Path]::TrimEndingDirectorySeparator([IO.Path]::GetFullPath($env:TEMP))
$dataDirectory = [IO.Path]::GetFullPath((Join-Path $temporaryRoot "alcomd-m2-data-$token"))
function Assert-TestDataDirectory {
    if (![string]::Equals([IO.Path]::GetDirectoryName($dataDirectory), $temporaryRoot, [StringComparison]::OrdinalIgnoreCase) -or
        [IO.Path]::GetFileName($dataDirectory) -ne "alcomd-m2-data-$token") {
        throw "CLI fixture data directory is outside its verified temporary root."
    }
    if ((Test-Path -LiteralPath $dataDirectory) -and
        ((Get-Item -LiteralPath $dataDirectory -Force).Attributes -band [IO.FileAttributes]::ReparsePoint)) {
        throw "CLI fixture data directory must not be a reparse point."
    }
}
function Get-TestDaemons {
    Get-CimInstance Win32_Process -Filter "Name='alcomd.exe'" |
        Where-Object { $_.ExecutablePath -eq $daemonPath -and $_.CommandLine -match [regex]::Escape($dataDirectory) } |
        ForEach-Object { Get-Process -Id $_.ProcessId -ErrorAction SilentlyContinue }
}
Assert-TestDataDirectory
$outputs = @(
    (Join-Path $env:TEMP "alcomd-m1-$token-1.out"),
    (Join-Path $env:TEMP "alcomd-m1-$token-1.err"),
    (Join-Path $env:TEMP "alcomd-m1-$token-2.out"),
    (Join-Path $env:TEMP "alcomd-m1-$token-2.err")
)

try {
    $first = Start-Process -FilePath $cliPath `
        -ArgumentList @("--data-dir", $dataDirectory, "--json", "system", "status") `
        -RedirectStandardOutput $outputs[0] `
        -RedirectStandardError $outputs[1] `
        -PassThru `
        -WindowStyle Hidden
    $second = Start-Process -FilePath $cliPath `
        -ArgumentList @("--data-dir", $dataDirectory, "--json", "system", "status") `
        -RedirectStandardOutput $outputs[2] `
        -RedirectStandardError $outputs[3] `
        -PassThru `
        -WindowStyle Hidden
    $first.WaitForExit()
    $second.WaitForExit()
    if ($first.ExitCode -ne 0 -or $second.ExitCode -ne 0) {
        throw "Concurrent CLI startup failed: $($first.ExitCode), $($second.ExitCode)."
    }

    $daemons = @(Get-TestDaemons)
    if ($daemons.Count -ne 1) {
        throw "Expected one authoritative daemon; found $($daemons.Count)."
    }
    $firstStatus = Get-Content -Raw -LiteralPath $outputs[0] | ConvertFrom-Json
    $secondStatus = Get-Content -Raw -LiteralPath $outputs[2] | ConvertFrom-Json
    if (
        $firstStatus.type -ne "result" -or
        $firstStatus.command -ne "system status" -or
        $firstStatus.result.state -ne "ready" -or
        $secondStatus.type -ne "result" -or
        $secondStatus.command -ne "system status" -or
        $secondStatus.result.state -ne "ready"
    ) {
        throw "One or more concurrent clients did not receive ready status."
    }
    Write-Host "M1 concurrent daemon auto-start passed; authoritative daemon count: 1."
}
finally {
    $ownedDaemons = @(Get-TestDaemons)
    $ownedDaemons | Stop-Process -Force -ErrorAction SilentlyContinue
    foreach ($process in $ownedDaemons) {
        $process.WaitForExit(5000) | Out-Null
        $process.Dispose()
    }
    foreach ($process in @($first, $second)) {
        if ($null -ne $process) {
            $process.Dispose()
        }
    }
    foreach ($path in $outputs) {
        for ($attempt = 0; $attempt -lt 50; $attempt++) {
            if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
                break
            }
            Remove-Item -LiteralPath $path -Force -ErrorAction SilentlyContinue
            Start-Sleep -Milliseconds 10
        }
        if (Test-Path -LiteralPath $path -PathType Leaf) {
            throw "Failed to remove isolated CLI output '$path'."
        }
    }
    if (Test-Path -LiteralPath $dataDirectory -PathType Container) {
        Assert-TestDataDirectory
        Remove-Item -LiteralPath $dataDirectory -Recurse -Force
    }
}
