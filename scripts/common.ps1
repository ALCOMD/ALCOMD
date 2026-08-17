$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

function Require-Command([string]$Name) {
    if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
        throw "Required command '$Name' was not found."
    }
}

function Resolve-RepositoryPython {
    $launcher = Get-Command "py" -CommandType Application -ErrorAction SilentlyContinue
    if ($launcher) {
        return [pscustomobject]@{
            Executable = $launcher.Source
            PrefixArguments = @("-3")
        }
    }

    if ($env:LOCALAPPDATA) {
        $pythonRoot = Join-Path $env:LOCALAPPDATA "Programs\Python"
        $installations = Get-ChildItem -LiteralPath $pythonRoot -Directory -Filter "Python*" -ErrorAction SilentlyContinue |
            Sort-Object Name -Descending
        foreach ($installation in $installations) {
            $executable = Join-Path $installation.FullName "python.exe"
            if (Test-Path -LiteralPath $executable -PathType Leaf) {
                return [pscustomobject]@{
                    Executable = $executable
                    PrefixArguments = @()
                }
            }
        }
    }

    foreach ($name in @("python3", "python")) {
        $command = Get-Command $name -CommandType Application -ErrorAction SilentlyContinue
        if ($command -and $command.Source -notmatch '\\WindowsApps\\') {
            return [pscustomobject]@{
                Executable = $command.Source
                PrefixArguments = @()
            }
        }
    }

    throw "Python 3.11 or newer was not found. Add Python to PATH or install a per-user Python runtime."
}

function Assert-RepositoryToolchain {
    foreach ($command in @("rustup", "rustc", "cargo", "rustfmt", "clippy-driver", "node", "npm", "git", "pwsh")) {
        Require-Command $command
    }

    $rustVersion = ((rustc --version).Split(" ")[1]).Trim()
    if ($rustVersion -ne "1.97.1") {
        throw "Rust 1.97.1 is required; found $rustVersion."
    }

    $nodeMajor = (node --version).TrimStart("v").Split(".")[0]
    if ($nodeMajor -ne "24") {
        throw "Node.js 24 LTS is required; found major version $nodeMajor."
    }

    $powerShellMajor = [int](& pwsh -NoLogo -NoProfile -Command '$PSVersionTable.PSVersion.Major')
    if ($powerShellMajor -lt 7) {
        throw "PowerShell 7 or newer is required; found major version $powerShellMajor."
    }

    $script:RepositoryPython = Resolve-RepositoryPython
    $pythonVersion = (& $script:RepositoryPython.Executable @($script:RepositoryPython.PrefixArguments) --version 2>&1 | Out-String).Trim()
    if ($pythonVersion -notmatch '^Python 3\.(\d+)\.') {
        throw "Unable to determine the Python version from '$pythonVersion'."
    }
    if ([int]$Matches[1] -lt 11) {
        throw "Python 3.11 or newer is required for tomllib; found $pythonVersion."
    }

    Write-Host "Toolchain: Rust $rustVersion; Node $(node --version); npm $(npm --version); PowerShell $powerShellMajor; $pythonVersion"
}

function Assert-WindowsTauriPrerequisites {
    if (-not $IsWindows) {
        Write-Host "PowerShell native prerequisite detection is defined for Windows; use scripts/setup.sh on Linux or macOS."
        return
    }

    $visualStudioFound = $false
    if (Get-Command "cl.exe" -ErrorAction SilentlyContinue) {
        $visualStudioFound = $true
    }
    else {
        $vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
        if (Test-Path -LiteralPath $vswhere -PathType Leaf) {
            $installation = (& $vswhere -latest -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath | Out-String).Trim()
            $visualStudioFound = -not [string]::IsNullOrWhiteSpace($installation)
        }
    }
    if (-not $visualStudioFound) {
        throw "Tauri requires MSVC C++ Build Tools with the x86/x64 compiler component."
    }

    $webViewRoots = @(
        (Join-Path ${env:ProgramFiles(x86)} "Microsoft\EdgeWebView\Application"),
        (Join-Path $env:ProgramFiles "Microsoft\EdgeWebView\Application"),
        (Join-Path $env:LOCALAPPDATA "Microsoft\EdgeWebView\Application")
    )
    $webViewFound = $false
    foreach ($root in $webViewRoots) {
        if (-not (Test-Path -LiteralPath $root -PathType Container)) {
            continue
        }
        $runtime = Get-ChildItem -LiteralPath $root -Directory -ErrorAction SilentlyContinue |
            Where-Object { Test-Path -LiteralPath (Join-Path $_.FullName "msedgewebview2.exe") -PathType Leaf } |
            Select-Object -First 1
        if ($runtime) {
            $webViewFound = $true
            break
        }
    }
    if (-not $webViewFound) {
        throw "Tauri requires the Microsoft Edge WebView2 Evergreen Runtime."
    }

    Write-Host "Windows Tauri prerequisites: MSVC C++ Build Tools and WebView2 Runtime found."
}

function Get-LockFileSnapshot {
    $lockFiles = @(
        "Cargo.lock",
        "package-lock.json",
        "extensions/first-party/alcomd-extension-discord/backend/Cargo.lock"
    )
    foreach ($relative in $lockFiles) {
        if (-not (Test-Path -LiteralPath $relative -PathType Leaf)) {
            throw "Required lock file '$relative' is missing."
        }
    }
    return (($lockFiles | ForEach-Object { git hash-object -- $_ }) -join "|")
}

function Assert-LockFileSnapshot([string]$Before) {
    $after = Get-LockFileSnapshot
    if ($after -ne $Before) {
        throw "One or more of the three required lock files changed while running the command."
    }
}

function Invoke-MetadataValidator {
    if (-not $script:RepositoryPython) {
        $script:RepositoryPython = Resolve-RepositoryPython
    }
    & $script:RepositoryPython.Executable @($script:RepositoryPython.PrefixArguments) ".\scripts\validate-metadata.py"
}
