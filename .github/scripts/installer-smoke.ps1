#Requires -Version 7.2

[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [string] $CurrentInstaller,

    [string] $PreviousInstaller,

    [Parameter(Mandatory)]
    [string] $ExpectedVersion,

    [Parameter(Mandatory)]
    [string] $InstallDir
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

# The MCP config and listener are created after the renderer reports ready; hosted runners can have cold WebView startup.
$mcpObservationSeconds = 30
$associationObservationSeconds = 10
$settingsObservationSeconds = 10
$uninstallCleanupSeconds = 20
$checkedProcessTimeoutSeconds = 600
$script:InstallationStarted = $false

function Assert-GitHubHostedWindowsRunner {
    if ($env:GITHUB_ACTIONS -ne 'true') {
        throw 'installer-smoke.ps1 may only run when GITHUB_ACTIONS=true.'
    }

    if ($env:RUNNER_ENVIRONMENT -ne 'github-hosted') {
        throw 'installer-smoke.ps1 requires an ephemeral GitHub-hosted runner.'
    }

    if ($env:RUNNER_OS -ne 'Windows' -or $env:OS -ne 'Windows_NT') {
        throw 'installer-smoke.ps1 requires a Windows GitHub-hosted runner.'
    }

    if ([string]::IsNullOrWhiteSpace($env:RUNNER_TEMP)) {
        throw 'RUNNER_TEMP is required to isolate the smoke installation.'
    }
}

function Resolve-ExistingInstaller {
    param(
        [Parameter(Mandatory)]
        [string] $Path,

        [Parameter(Mandatory)]
        [string] $Label
    )

    $resolved = Resolve-Path -LiteralPath $Path -ErrorAction Stop
    if ($resolved.Provider.Name -ne 'FileSystem') {
        throw "$Label must be a filesystem path: $Path"
    }

    $item = Get-Item -LiteralPath $resolved.ProviderPath -Force
    if ($item.PSIsContainer -or $item.Extension -ine '.exe') {
        throw "$Label must be an existing .exe file: $($item.FullName)"
    }

    return $item.FullName
}

function Resolve-IsolatedInstallDirectory {
    param(
        [Parameter(Mandatory)]
        [string] $Path
    )

    $runnerTemp = [System.IO.Path]::GetFullPath($env:RUNNER_TEMP).TrimEnd(
        [System.IO.Path]::DirectorySeparatorChar,
        [System.IO.Path]::AltDirectorySeparatorChar
    )
    $candidate = [System.IO.Path]::GetFullPath($Path).TrimEnd(
        [System.IO.Path]::DirectorySeparatorChar,
        [System.IO.Path]::AltDirectorySeparatorChar
    )
    $runnerPrefix = $runnerTemp + [System.IO.Path]::DirectorySeparatorChar

    if (-not $candidate.StartsWith($runnerPrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "InstallDir must be a child of RUNNER_TEMP ($runnerTemp): $candidate"
    }

    if (Test-Path -LiteralPath $candidate) {
        throw "InstallDir must not exist before the smoke test: $candidate"
    }

    return $candidate
}

function Invoke-CheckedProcess {
    param(
        [Parameter(Mandatory)]
        [string] $FilePath,

        [Parameter(Mandatory)]
        [string[]] $ArgumentList,

        [Parameter(Mandatory)]
        [string] $Description
    )

    Write-Information "::group::$Description" -InformationAction Continue
    try {
        $startInfo = [System.Diagnostics.ProcessStartInfo]::new()
        $startInfo.FileName = $FilePath
        $startInfo.UseShellExecute = $false
        $startInfo.CreateNoWindow = $true
        foreach ($argument in $ArgumentList) {
            [void] $startInfo.ArgumentList.Add($argument)
        }

        $process = [System.Diagnostics.Process]::Start($startInfo)
        if ($null -eq $process) {
            throw "Failed to start $Description."
        }

        try {
            if (-not $process.WaitForExit($checkedProcessTimeoutSeconds * 1000)) {
                $taskKill = Join-Path $env:SystemRoot 'System32\taskkill.exe'
                & $taskKill /PID $process.Id /T /F 2>&1 | Write-Information -InformationAction Continue
                $process.WaitForExit(30000) | Out-Null
                throw "$Description exceeded the $checkedProcessTimeoutSeconds second timeout."
            }
            if ($process.ExitCode -ne 0) {
                throw "$Description failed with exit code $($process.ExitCode)."
            }
        }
        finally {
            $process.Dispose()
        }
    }
    finally {
        Write-Information '::endgroup::' -InformationAction Continue
    }
}

function Invoke-Installer {
    param(
        [Parameter(Mandatory)]
        [string] $Installer,

        [Parameter(Mandatory)]
        [string] $Destination,

        [Parameter(Mandatory)]
        [string] $LogPath,

        [Parameter(Mandatory)]
        [string] $Description,

        [switch] $CreateDesktopIcon
    )

    $arguments = @(
        '/SP-'
        '/VERYSILENT'
        '/SUPPRESSMSGBOXES'
        '/NORESTART'
        '/NOICONS'
        '/CURRENTUSER'
        "/DIR=$Destination"
        "/LOG=$LogPath"
    )
    if ($CreateDesktopIcon) {
        $arguments += '/TASKS=desktopicon'
    }

    Invoke-CheckedProcess -FilePath $Installer -ArgumentList $arguments -Description $Description
}

function New-TestShortcut {
    param(
        [Parameter(Mandatory)]
        [string] $Path,

        [Parameter(Mandatory)]
        [string] $Target
    )

    $shell = New-Object -ComObject WScript.Shell
    $shortcut = $null
    try {
        $shortcut = $shell.CreateShortcut($Path)
        $shortcut.TargetPath = $Target
        $shortcut.WorkingDirectory = [System.IO.Path]::GetDirectoryName($Target)
        $shortcut.Save()
    }
    finally {
        if ($null -ne $shortcut) {
            [void] [System.Runtime.InteropServices.Marshal]::ReleaseComObject($shortcut)
        }
        [void] [System.Runtime.InteropServices.Marshal]::ReleaseComObject($shell)
    }
}

function Assert-ShortcutTarget {
    param(
        [Parameter(Mandatory)]
        [string] $Path,

        [Parameter(Mandatory)]
        [string] $ExpectedTarget
    )

    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "Expected shortcut is missing: $Path"
    }

    $shell = New-Object -ComObject WScript.Shell
    $shortcut = $null
    try {
        $shortcut = $shell.CreateShortcut($Path)
        $actualTarget = [System.IO.Path]::GetFullPath($shortcut.TargetPath)
    }
    finally {
        if ($null -ne $shortcut) {
            [void] [System.Runtime.InteropServices.Marshal]::ReleaseComObject($shortcut)
        }
        [void] [System.Runtime.InteropServices.Marshal]::ReleaseComObject($shell)
    }

    $expected = [System.IO.Path]::GetFullPath($ExpectedTarget)
    if ($actualTarget -ine $expected) {
        throw "Shortcut target mismatch for ${Path}: expected $expected, got $actualTarget."
    }
}

function Get-ShortcutAppUserModelId {
    param(
        [Parameter(Mandatory)]
        [string] $Path
    )

    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "Expected shortcut is missing: $Path"
    }

    $shell = New-Object -ComObject Shell.Application
    $folder = $null
    $item = $null
    try {
        $folder = $shell.Namespace([System.IO.Path]::GetDirectoryName($Path))
        if ($null -eq $folder) {
            throw "Unable to open shortcut directory: $Path"
        }
        $item = $folder.ParseName([System.IO.Path]::GetFileName($Path))
        if ($null -eq $item) {
            throw "Unable to inspect shortcut: $Path"
        }
        return [string] $item.ExtendedProperty('System.AppUserModel.ID')
    }
    finally {
        foreach ($value in @($item, $folder, $shell)) {
            if ($null -ne $value -and [System.Runtime.InteropServices.Marshal]::IsComObject($value)) {
                [void] [System.Runtime.InteropServices.Marshal]::ReleaseComObject($value)
            }
        }
    }
}

function Assert-ShortcutAppUserModelId {
    param(
        [Parameter(Mandatory)]
        [string] $Path,

        [Parameter(Mandatory)]
        [string] $ExpectedAppUserModelId
    )

    $actual = Get-ShortcutAppUserModelId -Path $Path
    if ($actual -cne $ExpectedAppUserModelId) {
        throw "Shortcut AppUserModelID mismatch for ${Path}: expected $ExpectedAppUserModelId, got $actual."
    }
}

function Assert-ShortcutRemoved {
    param(
        [Parameter(Mandatory)]
        [string] $Path
    )

    if (Test-Path -LiteralPath $Path) {
        throw "Shortcut remains after cleanup: $Path"
    }
}

function Get-InstalledProductRegistration {
    param(
        [Parameter(Mandatory)]
        [string] $Destination,

        [Parameter(Mandatory)]
        [string] $ProductName
    )

    $uninstallRoot = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall'
    if (-not (Test-Path -LiteralPath $uninstallRoot)) {
        return @()
    }

    return @(
        Get-ChildItem -LiteralPath $uninstallRoot -ErrorAction Stop |
            ForEach-Object {
                $registration = Get-ItemProperty -LiteralPath $_.PSPath -ErrorAction Stop
                $displayName = $registration.PSObject.Properties['DisplayName']
                $installLocation = $registration.PSObject.Properties['InstallLocation']
                $localizedDisplayNamePrefix = "$ProductName "
                if (
                    $null -ne $displayName -and
                    $null -ne $installLocation -and
                    (
                        $displayName.Value -ceq $ProductName -or
                        ([string] $displayName.Value).StartsWith(
                            $localizedDisplayNamePrefix,
                            [System.StringComparison]::Ordinal
                        )
                    ) -and
                    -not [string]::IsNullOrWhiteSpace($installLocation.Value) -and
                    [System.IO.Path]::GetFullPath($installLocation.Value).TrimEnd('\', '/') -ieq $Destination
                ) {
                    $registration
                }
            }
    )
}

function Test-AppIdRegistration {
    param(
        [Parameter(Mandatory)]
        [string] $AppId,

        [Parameter(Mandatory)]
        [Microsoft.Win32.RegistryHive] $Hive,

        [Parameter(Mandatory)]
        [Microsoft.Win32.RegistryView] $View
    )

    $subkey = "Software\Microsoft\Windows\CurrentVersion\Uninstall\$($AppId)_is1"
    $baseKey = [Microsoft.Win32.RegistryKey]::OpenBaseKey($Hive, $View)
    try {
        $registration = $baseKey.OpenSubKey($subkey)
        if ($null -eq $registration) {
            return $false
        }
        $registration.Dispose()
        return $true
    }
    finally {
        $baseKey.Dispose()
    }
}

function Assert-LegacyAppIdRemoved {
    param(
        [Parameter(Mandatory)]
        [pscustomobject] $Config
    )

    $locations = @(
        [pscustomobject]@{
            Hive = [Microsoft.Win32.RegistryHive]::CurrentUser
            View = [Microsoft.Win32.RegistryView]::Default
            Name = 'HKCU'
        }
        [pscustomobject]@{
            Hive = [Microsoft.Win32.RegistryHive]::LocalMachine
            View = [Microsoft.Win32.RegistryView]::Registry64
            Name = 'HKLM64'
        }
        [pscustomobject]@{
            Hive = [Microsoft.Win32.RegistryHive]::LocalMachine
            View = [Microsoft.Win32.RegistryView]::Registry32
            Name = 'HKLM32'
        }
    )
    foreach ($location in $locations) {
        if (Test-AppIdRegistration -AppId $Config.legacyWindowsAppId -Hive $location.Hive -View $location.View) {
            throw "Legacy AppId registration remains in $($location.Name): $($Config.legacyWindowsAppId)"
        }
    }
}

function Assert-TemplateAssociationInstalled {
    param(
        [Parameter(Mandatory)]
        [string] $Destination,

        [Parameter(Mandatory)]
        [pscustomobject] $Config,

        [switch] $AssertCurrentIdentity
    )

    $extensionKey = "HKCU:\Software\Classes\$($Config.templateAssociation.extension)\OpenWithProgids"
    $associationKey = "HKCU:\Software\Classes\$($Config.templateAssociation.key)"
    $commandKey = Join-Path $associationKey 'shell\open\command'
    foreach ($key in @($extensionKey, $associationKey, $commandKey)) {
        if (-not (Test-Path -LiteralPath $key)) {
            throw "Installed template association key is missing: $key"
        }
    }

    $extension = Get-ItemProperty -LiteralPath $extensionKey -ErrorAction Stop
    if ($null -eq $extension.PSObject.Properties[$Config.templateAssociation.key]) {
        throw "Installed template association does not register $($Config.templateAssociation.key)."
    }

    $expectedCommand = '"' + (Join-Path $Destination "$($Config.mainBinaryName).exe") + '" "%1"'
    $actualCommand = (Get-Item -LiteralPath $commandKey -ErrorAction Stop).GetValue('')
    if ($actualCommand -cne $expectedCommand) {
        throw "Installed template association command mismatch: expected $expectedCommand, got $actualCommand."
    }
    if ($AssertCurrentIdentity) {
        $actualAppUserModelId = (Get-Item -LiteralPath $associationKey -ErrorAction Stop).GetValue('AppUserModelID')
        if ($actualAppUserModelId -cne $Config.windowsAumid) {
            throw "Installed template association AppUserModelID mismatch: expected $($Config.windowsAumid), got $actualAppUserModelId."
        }
    }
}

function Assert-VccAssociationInstalled {
    param(
        [Parameter(Mandatory)]
        [string] $Destination,

        [Parameter(Mandatory)]
        [pscustomobject] $Config,

        [Parameter(Mandatory)]
        [System.Diagnostics.Process] $Process
    )

    $associationKey = 'HKCU:\Software\Classes\vcc'
    $commandKey = Join-Path $associationKey 'shell\open\command'
    $expectedCommand = '"' + (Join-Path $Destination "$($Config.mainBinaryName).exe") + '" link "%1"'
    $actualCommand = $null
    $deadline = [DateTime]::UtcNow.AddSeconds($associationObservationSeconds)
    do {
        $Process.Refresh()
        if ($Process.HasExited) {
            throw "Installed GUI exited before registering vcc:// with code $($Process.ExitCode)."
        }
        if (Test-Path -LiteralPath $commandKey) {
            $actualCommand = (Get-Item -LiteralPath $commandKey -ErrorAction Stop).GetValue('')
            if ($actualCommand -ceq $expectedCommand) {
                $actualAppUserModelId = (Get-Item -LiteralPath $associationKey -ErrorAction Stop).GetValue('AppUserModelID')
                if ($actualAppUserModelId -cne $Config.windowsAumid) {
                    throw "Installed vcc:// AppUserModelID mismatch: expected $($Config.windowsAumid), got $actualAppUserModelId."
                }
                return
            }
        }
        Start-Sleep -Milliseconds 250
    } while ([DateTime]::UtcNow -lt $deadline)

    if ($null -eq $actualCommand) {
        throw "Installed GUI did not register its vcc:// protocol command: $commandKey"
    }
    throw "Installed vcc:// protocol command mismatch: expected $expectedCommand, got $actualCommand."
}

function Assert-TemplateAssociationRemoved {
    param(
        [Parameter(Mandatory)]
        [pscustomobject] $Config
    )

    $extensionKey = "HKCU:\Software\Classes\$($Config.templateAssociation.extension)\OpenWithProgids"
    if (Test-Path -LiteralPath $extensionKey) {
        $extension = Get-ItemProperty -LiteralPath $extensionKey -ErrorAction Stop
        if ($null -ne $extension.PSObject.Properties[$Config.templateAssociation.key]) {
            throw "Uninstall left the template association value at $extensionKey."
        }
    }

    $associationKey = "HKCU:\Software\Classes\$($Config.templateAssociation.key)"
    if (Test-Path -LiteralPath $associationKey) {
        throw "Uninstall left the template association key: $associationKey"
    }
}

function Assert-CompatibleExecutableVersion {
    param(
        [Parameter(Mandatory)]
        [string] $Executable,

        [Parameter(Mandatory)]
        [string] $Version
    )

    $versionInfo = (Get-Item -LiteralPath $Executable).VersionInfo
    $resourceVersion = $versionInfo.ProductVersion
    if ([string]::IsNullOrWhiteSpace($resourceVersion)) {
        $resourceVersion = $versionInfo.FileVersion
    }
    if ([string]::IsNullOrWhiteSpace($resourceVersion)) {
        throw "Installed GUI has no version resource: $Executable"
    }

    $expectedCore = $Version.Split('-', 2)[0].Split('+', 2)[0]
    $acceptedVersions = @($Version, $expectedCore, "$expectedCore.0")
    if ($resourceVersion -notin $acceptedVersions) {
        throw "Installed GUI version resource mismatch: expected $Version (core $expectedCore), got $resourceVersion."
    }
}

function Assert-Installation {
    param(
        [Parameter(Mandatory)]
        [string] $Destination,

        [Parameter(Mandatory)]
        [pscustomobject] $Config,

        [Parameter(Mandatory)]
        [ValidateSet('Baseline', 'Current')]
        [string] $Phase,

        [string] $Version
    )

    $mainExecutable = Join-Path $Destination "$($Config.mainBinaryName).exe"
    foreach ($executable in @($mainExecutable)) {
        if (-not (Test-Path -LiteralPath $executable -PathType Leaf)) {
            throw "Expected installed executable is missing: $executable"
        }
        if ((Get-Item -LiteralPath $executable).Length -le 0) {
            throw "Installed executable is empty: $executable"
        }
    }

    $registrations = @(Get-InstalledProductRegistration -Destination $Destination -ProductName $Config.productName)
    if ($registrations.Count -ne 1) {
        throw "Expected exactly one uninstall registration for $($Config.productName) at $Destination, found $($registrations.Count)."
    }

    if ([string]::IsNullOrWhiteSpace($registrations[0].DisplayVersion)) {
        throw 'Installed product registration has no DisplayVersion.'
    }

    if ($Phase -ceq 'Baseline' -and -not [string]::IsNullOrWhiteSpace($Version)) {
        throw 'Baseline installation validation must not use the current Version contract.'
    }
    if ($Phase -ceq 'Current') {
        if ([string]::IsNullOrWhiteSpace($Version)) {
            throw 'Current installation validation requires Version.'
        }
        if (-not (Test-AppIdRegistration `
            -AppId $Config.windowsAppId `
            -Hive ([Microsoft.Win32.RegistryHive]::CurrentUser) `
            -View ([Microsoft.Win32.RegistryView]::Default))) {
            throw "Current AppId is not registered in HKCU: $($Config.windowsAppId)"
        }
        if ($registrations[0].DisplayName -cne $Config.productName) {
            throw "Installed application display name mismatch: expected $($Config.productName), got $($registrations[0].DisplayName)."
        }
        if ($registrations[0].DisplayVersion -cne $Version) {
            throw "Installed version mismatch: expected $Version, got $($registrations[0].DisplayVersion)."
        }
        Assert-CompatibleExecutableVersion -Executable $mainExecutable -Version $Version

        $legacyMcpExecutable = Join-Path $Destination 'alcomd3-mcp.exe'
        if (Test-Path -LiteralPath $legacyMcpExecutable) {
            throw "Current installation still contains the removed MCP helper: $legacyMcpExecutable"
        }
    }

    Assert-TemplateAssociationInstalled `
        -Destination $Destination `
        -Config $Config `
        -AssertCurrentIdentity:($Phase -ceq 'Current')
}

function Stop-InstalledProcess {
    [CmdletBinding(SupportsShouldProcess)]
    param(
        [Parameter(Mandatory)]
        [string] $Destination,

        [Parameter(Mandatory)]
        [pscustomobject] $Config
    )

    $executableNames = @(
        "$($Config.mainBinaryName).exe"
        'alcomd3-mcp.exe'
        $Config.legacyWindowsExecutableName
    )

    foreach ($name in $executableNames) {
        $escapedName = $name.Replace("'", "''")
        $processes = @(Get-CimInstance -ClassName Win32_Process -Filter "Name = '$escapedName'" -ErrorAction Stop)
        foreach ($process in $processes) {
            if ([string]::IsNullOrWhiteSpace($process.ExecutablePath)) {
                continue
            }

            $processPath = [System.IO.Path]::GetFullPath($process.ExecutablePath)
            $destinationPrefix = $Destination.TrimEnd('\', '/') + [System.IO.Path]::DirectorySeparatorChar
            if ($processPath.StartsWith($destinationPrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
                if ($PSCmdlet.ShouldProcess($processPath, "Stop process $($process.ProcessId)")) {
                    Write-Information "Stopping installed process $($process.ProcessId): $processPath" -InformationAction Continue
                    try {
                        Stop-Process -Id $process.ProcessId -Force -ErrorAction Stop
                    }
                    catch {
                        $stillRunning = Get-Process `
                            -Id $process.ProcessId `
                            -ErrorAction SilentlyContinue
                        if ($null -ne $stillRunning) {
                            throw
                        }

                        Write-Information `
                            "Installed process $($process.ProcessId) exited before it could be stopped." `
                            -InformationAction Continue
                    }
                    Wait-Process -Id $process.ProcessId -Timeout 10 -ErrorAction SilentlyContinue
                }
            }
        }
    }
}

function Invoke-McpHttpRequest {
    param(
        [Parameter(Mandatory)]
        [pscustomobject] $Endpoint,

        [Parameter(Mandatory)]
        [hashtable] $Request,

        [AllowNull()]
        [string] $Token,

        [string] $SessionId
    )

    $headers = @{
        Accept = 'application/json, text/event-stream'
    }
    if ($null -ne $Token) {
        $headers.Authorization = "Bearer $Token"
    }
    if (-not [string]::IsNullOrWhiteSpace($SessionId)) {
        $headers['Mcp-Session-Id'] = $SessionId
    }

    $response = Invoke-WebRequest `
        -Uri "http://127.0.0.1:$($Endpoint.port)/mcp" `
        -Method Post `
        -Headers $headers `
        -ContentType 'application/json' `
        -Body ($Request | ConvertTo-Json -Compress -Depth 8) `
        -TimeoutSec 5 `
        -SkipHttpErrorCheck

    return [pscustomobject]@{
        StatusCode = [int] $response.StatusCode
        Content = [string] $response.Content
        SessionId = [string] $response.Headers['Mcp-Session-Id']
    }
}

function Assert-McpAccessDisabledByDefault {
    param(
        [Parameter(Mandatory)]
        [string] $Destination,

        [Parameter(Mandatory)]
        [pscustomobject] $Config,

        [Parameter(Mandatory)]
        [string] $GuiConfigFile,

        [switch] $AssertGuiOwnsListener,

        [switch] $AssertVccAssociation,

        [string] $ExpectedSettingsFile
    )

    Stop-InstalledProcess -Destination $Destination -Config $Config

    $mainExecutable = Join-Path $Destination "$($Config.mainBinaryName).exe"
    $startInfo = [System.Diagnostics.ProcessStartInfo]::new()
    $startInfo.FileName = $mainExecutable
    $startInfo.UseShellExecute = $false
    $process = [System.Diagnostics.Process]::Start($startInfo)
    if ($null -eq $process) {
        throw "Failed to launch installed GUI: $mainExecutable"
    }

    try {
        $endpoint = $null
        for ($elapsed = 0; $elapsed -lt $mcpObservationSeconds; $elapsed++) {
            $process.Refresh()
            if ($process.HasExited) {
                throw "Installed GUI exited unexpectedly with code $($process.ExitCode)."
            }
            if (Test-Path -LiteralPath $GuiConfigFile -PathType Leaf) {
                try {
                    $guiConfig = Get-Content -LiteralPath $GuiConfigFile -Raw | ConvertFrom-Json
                    $candidate = [pscustomobject]@{
                        port = [int] $guiConfig.mcpHttpPort
                        token = [string] $guiConfig.mcpHttpToken
                    }
                    if (
                        $candidate.port -ge 1 -and
                        $candidate.port -le 65535 -and
                        $candidate.token -cmatch '^[0-9a-f]{32}$'
                    ) {
                        $probe = Invoke-McpHttpRequest `
                            -Endpoint $candidate `
                            -Request @{ jsonrpc = '2.0'; id = 'probe'; method = 'initialize'; params = @{} }
                        if ($probe.StatusCode -eq 401) {
                            $endpoint = $candidate
                            break
                        }
                    }
                }
                catch {
                    Write-Verbose "Waiting for MCP HTTP startup: $($_.Exception.Message)"
                }
            }
            Start-Sleep -Seconds 1
        }

        if ($null -eq $endpoint) {
            throw "Installed GUI did not start MCP HTTP from its configuration: $GuiConfigFile"
        }

        $listeners = @(
            Get-NetTCPConnection -ErrorAction Stop |
                Where-Object {
                    $_.State -eq 'Listen' -and
                    $_.LocalPort -eq $endpoint.port
                }
        )
        $unexpectedListeners = @(
            $listeners | Where-Object { $_.LocalAddress -cne '127.0.0.1' }
        )
        if ($unexpectedListeners.Count -gt 0) {
            $unexpectedAddresses = @($unexpectedListeners.LocalAddress) -join ', '
            throw "GUI PID $($process.Id) exposes non-loopback MCP listener(s) on port $($endpoint.port): $unexpectedAddresses"
        }
        $loopbackListeners = @(
            $listeners | Where-Object { $_.LocalAddress -ceq '127.0.0.1' }
        )
        if ($loopbackListeners.Count -lt 1) {
            throw "MCP HTTP listener 127.0.0.1:$($endpoint.port) was not found."
        }
        if (
            $AssertGuiOwnsListener -and
            @($loopbackListeners | Where-Object { $_.OwningProcess -eq $process.Id }).Count -lt 1
        ) {
            $owners = @($loopbackListeners.OwningProcess) -join ', '
            throw "MCP HTTP port $($endpoint.port) is not owned by GUI PID $($process.Id); owners: $owners"
        }
        if ($AssertVccAssociation) {
            Assert-VccAssociationInstalled `
                -Destination $Destination `
                -Config $Config `
                -Process $process
        }
        if (-not [string]::IsNullOrWhiteSpace($ExpectedSettingsFile)) {
            $settingsCreated = $false
            $settingsDeadline = [DateTime]::UtcNow.AddSeconds($settingsObservationSeconds)
            do {
                $process.Refresh()
                if ($process.HasExited) {
                    throw "Installed GUI exited while waiting for its settings file with code $($process.ExitCode)."
                }
                if (Test-Path -LiteralPath $ExpectedSettingsFile -PathType Leaf) {
                    $settingsCreated = $true
                    break
                }
                Start-Sleep -Milliseconds 250
            } while ([DateTime]::UtcNow -lt $settingsDeadline)

            if (-not $settingsCreated) {
                throw "Installed GUI did not create its settings file: $ExpectedSettingsFile"
            }
        }

        $unauthenticated = Invoke-McpHttpRequest `
            -Endpoint $endpoint `
            -Request @{ jsonrpc = '2.0'; id = 'no-token'; method = 'initialize'; params = @{} }
        if ($unauthenticated.StatusCode -ne 401) {
            throw "MCP HTTP request without a token returned $($unauthenticated.StatusCode), expected 401."
        }

        do {
            $wrongToken = [guid]::NewGuid().ToString('N')
        } while ($wrongToken -ceq $endpoint.token)
        $unauthorized = Invoke-McpHttpRequest `
            -Endpoint $endpoint `
            -Token $wrongToken `
            -Request @{ jsonrpc = '2.0'; id = 'wrong-token'; method = 'initialize'; params = @{} }
        if ($unauthorized.StatusCode -ne 401) {
            throw "MCP HTTP request with the wrong token returned $($unauthorized.StatusCode), expected 401."
        }

        $initialize = Invoke-McpHttpRequest `
            -Endpoint $endpoint `
            -Token $endpoint.token `
            -Request @{
                jsonrpc = '2.0'
                id = 'initialize'
                method = 'initialize'
                params = @{
                    protocolVersion = '2025-11-25'
                    capabilities = @{}
                    clientInfo = @{ name = 'alcomd3-installer-smoke'; version = '1' }
                }
            }
        if (
            $initialize.StatusCode -ne 200 -or
            [string]::IsNullOrWhiteSpace($initialize.SessionId)
        ) {
            throw "MCP initialize failed: HTTP $($initialize.StatusCode), $($initialize.Content)"
        }

        $initialized = Invoke-McpHttpRequest `
            -Endpoint $endpoint `
            -Token $endpoint.token `
            -SessionId $initialize.SessionId `
            -Request @{ jsonrpc = '2.0'; method = 'notifications/initialized' }
        if ($initialized.StatusCode -notin @(200, 202)) {
            throw "MCP initialized notification failed: HTTP $($initialized.StatusCode), $($initialized.Content)"
        }

        $response = Invoke-McpHttpRequest `
            -Endpoint $endpoint `
            -Token $endpoint.token `
            -SessionId $initialize.SessionId `
            -Request @{
                jsonrpc = '2.0'
                id = 'list-projects'
                method = 'tools/call'
                params = @{ name = 'list_projects'; arguments = @{} }
            }
        if ($response.StatusCode -ne 200 -or $response.Content -cnotmatch 'mcp_disabled') {
            throw "MCP access was not disabled by default: HTTP $($response.StatusCode), $($response.Content)"
        }
    }
    finally {
        if (-not $process.HasExited) {
            Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
            Wait-Process -Id $process.Id -Timeout 10 -ErrorAction SilentlyContinue
        }
        Stop-InstalledProcess -Destination $Destination -Config $Config
    }
}

function Get-Uninstaller {
    param(
        [Parameter(Mandatory)]
        [string] $Destination
    )

    $uninstallers = @(Get-ChildItem -LiteralPath $Destination -Filter 'unins*.exe' -File -ErrorAction Stop)
    if ($uninstallers.Count -ne 1) {
        throw "Expected exactly one Inno Setup uninstaller in $Destination, found $($uninstallers.Count)."
    }

    return $uninstallers[0].FullName
}

function Invoke-Uninstall {
    param(
        [Parameter(Mandatory)]
        [string] $Destination,

        [Parameter(Mandatory)]
        [pscustomobject] $Config,

        [Parameter(Mandatory)]
        [string] $RegistrationPath
    )

    Stop-InstalledProcess -Destination $Destination -Config $Config
    $uninstaller = Get-Uninstaller -Destination $Destination
    Invoke-CheckedProcess -FilePath $uninstaller -ArgumentList @(
        '/VERYSILENT'
        '/SUPPRESSMSGBOXES'
        '/NORESTART'
    ) -Description 'Uninstall current version'

    $deadline = [DateTime]::UtcNow.AddSeconds($uninstallCleanupSeconds)
    while ((Test-Path -LiteralPath $Destination) -and [DateTime]::UtcNow -lt $deadline) {
        Start-Sleep -Milliseconds 500
    }

    $remainingRegistrations = @(Get-InstalledProductRegistration -Destination $Destination -ProductName $Config.productName)
    if ($remainingRegistrations.Count -ne 0) {
        throw "Uninstall registration still exists for $Destination."
    }
    if (Test-Path -LiteralPath $RegistrationPath) {
        throw "The exact uninstall registration still exists: $RegistrationPath"
    }

    Assert-TemplateAssociationRemoved -Config $Config

    $vccCommandKey = 'HKCU:\Software\Classes\vcc\shell\open\command'
    if (Test-Path -LiteralPath $vccCommandKey) {
        $vccCommand = (Get-Item -LiteralPath $vccCommandKey -ErrorAction Stop).GetValue('')
        $installedExecutable = Join-Path $Destination "$($Config.mainBinaryName).exe"
        if ($vccCommand -like "*$installedExecutable*") {
            throw "Uninstall left a stale vcc protocol command: $vccCommand"
        }
    }

    if (Test-Path -LiteralPath $Destination) {
        $remaining = @(Get-ChildItem -LiteralPath $Destination -Force -ErrorAction Stop)
        throw "Install directory still exists after uninstall with $($remaining.Count) item(s): $Destination"
    }
}

Assert-GitHubHostedWindowsRunner

if ($ExpectedVersion -cnotmatch '^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(?:-[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?$') {
    throw "ExpectedVersion must be a semantic version: $ExpectedVersion"
}

$repositoryRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
$configPath = Join-Path $repositoryRoot 'alcomd3.config.json'
$config = Get-Content -LiteralPath $configPath -Raw | ConvertFrom-Json

foreach ($property in @(
    'productName',
    'mainBinaryName',
    'tauriIdentifier',
    'legacyTauriIdentifier',
    'windowsAppId',
    'windowsAumid',
    'legacyWindowsAppId',
    'legacyWindowsMigrationReleaseTag',
    'legacyWindowsExecutableName'
)) {
    if ([string]::IsNullOrWhiteSpace($config.$property)) {
        throw "alcomd3.config.json is missing required property: $property"
    }
}
foreach ($property in @('extension', 'key')) {
    if ([string]::IsNullOrWhiteSpace($config.templateAssociation.$property)) {
        throw "alcomd3.config.json is missing required templateAssociation property: $property"
    }
}

$currentInstallerPath = Resolve-ExistingInstaller -Path $CurrentInstaller -Label 'CurrentInstaller'
$previousInstallerPath = $null
if (-not [string]::IsNullOrWhiteSpace($PreviousInstaller)) {
    $previousInstallerPath = Resolve-ExistingInstaller -Path $PreviousInstaller -Label 'PreviousInstaller'
    if ($previousInstallerPath -ieq $currentInstallerPath) {
        throw 'PreviousInstaller and CurrentInstaller must be different files.'
    }
}

$installDirectory = Resolve-IsolatedInstallDirectory -Path $InstallDir
$localApplicationData = [Environment]::GetFolderPath([Environment+SpecialFolder]::LocalApplicationData)
$applicationDataDirectory = Join-Path $localApplicationData $config.productName
$guiConfigFile = Join-Path $applicationDataDirectory 'config\gui-config.json'
$currentTauriDataDirectory = Join-Path $localApplicationData $config.tauriIdentifier
$legacyTauriDataDirectory = Join-Path $localApplicationData $config.legacyTauriIdentifier
if (Test-Path -LiteralPath $applicationDataDirectory) {
    throw "Ephemeral runner already contains product data: $applicationDataDirectory"
}
if (Test-Path -LiteralPath $currentTauriDataDirectory) {
    Remove-Item `
        -LiteralPath $currentTauriDataDirectory `
        -Recurse `
        -Force `
        -ErrorAction Stop
}
if (Test-Path -LiteralPath $currentTauriDataDirectory) {
    throw "Failed to reset current Tauri data before installer smoke: $currentTauriDataDirectory"
}
$upgradeSentinel = $null
$legacyExecutable = Join-Path $installDirectory $config.legacyWindowsExecutableName
$legacyMcpExecutable = Join-Path $installDirectory 'alcomd3-mcp.exe'
$currentInstallLog = Join-Path $env:RUNNER_TEMP "alcomd3-installer-smoke-$PID-current.log"
$technicalLogsDirectory = Join-Path $applicationDataDirectory 'technical-logs'
$technicalLogsDiagnosticDirectory = Join-Path $env:RUNNER_TEMP "alcomd3-installer-smoke-$PID-technical-logs"
$userDesktop = [Environment]::GetFolderPath([Environment+SpecialFolder]::DesktopDirectory)
$userPrograms = [Environment]::GetFolderPath([Environment+SpecialFolder]::Programs)
$currentDesktopShortcut = Join-Path $userDesktop "$($config.productName).lnk"
$currentProgramsShortcut = Join-Path $userPrograms "$($config.productName).lnk"
$legacyDesktopShortcut = Join-Path $userDesktop 'ALCOM.lnk'
$legacyProgramsShortcut = Join-Path $userPrograms 'ALCOM.lnk'
$unrelatedShortcutTarget = Join-Path $env:SystemRoot 'System32\notepad.exe'
$smokeShortcuts = @(
    $currentDesktopShortcut
    $currentProgramsShortcut
    $legacyDesktopShortcut
    $legacyProgramsShortcut
)
foreach ($shortcut in $smokeShortcuts) {
    if (Test-Path -LiteralPath $shortcut) {
        throw "Ephemeral runner already contains installer smoke shortcut: $shortcut"
    }
}

try {
    if ($null -ne $previousInstallerPath) {
        $previousInstallLog = Join-Path $env:RUNNER_TEMP "alcomd3-installer-smoke-$PID-previous.log"
        $script:InstallationStarted = $true
        Invoke-Installer `
            -Installer $previousInstallerPath `
            -Destination $installDirectory `
            -LogPath $previousInstallLog `
            -Description 'Install previous version' `
            -CreateDesktopIcon
        Assert-Installation `
            -Destination $installDirectory `
            -Config $config `
            -Phase Baseline
        $previousExecutable = Join-Path $installDirectory "$($config.mainBinaryName).exe"
        Assert-ShortcutTarget -Path $currentDesktopShortcut -ExpectedTarget $previousExecutable
        Assert-ShortcutTarget -Path $currentProgramsShortcut -ExpectedTarget $previousExecutable
        $previousRegistrations = @(
            Get-InstalledProductRegistration `
                -Destination $installDirectory `
                -ProductName $config.productName
        )
        if ($previousRegistrations[0].DisplayVersion -ceq $ExpectedVersion) {
            throw "PreviousInstaller already installs the expected current version $ExpectedVersion."
        }
        $previousSettings = Join-Path $applicationDataDirectory 'settings.json'
        Assert-McpAccessDisabledByDefault `
            -Destination $installDirectory `
            -Config $config `
            -GuiConfigFile $guiConfigFile `
            -ExpectedSettingsFile $previousSettings
        $upgradeSentinel = Join-Path $applicationDataDirectory "installer-smoke-upgrade-$PID.txt"
        Set-Content -LiteralPath $upgradeSentinel -Value 'preserve across upgrade' -NoNewline
    }
    else {
        New-Item -ItemType Directory -Path $installDirectory -ErrorAction Stop | Out-Null
        $script:InstallationStarted = $true
    }

    $vccAssociationKey = 'HKCU:\Software\Classes\vcc'
    if (Test-Path -LiteralPath $vccAssociationKey) {
        Remove-Item -LiteralPath $vccAssociationKey -Recurse -Force -ErrorAction Stop
    }

    Set-Content -LiteralPath $legacyExecutable -Value 'installer smoke legacy sentinel' -NoNewline
    if (-not (Test-Path -LiteralPath $legacyMcpExecutable -PathType Leaf)) {
        Set-Content `
            -LiteralPath $legacyMcpExecutable `
            -Value 'installer smoke legacy MCP helper sentinel' `
            -NoNewline
    }
    $legacyWebViewDirectory = Join-Path $legacyTauriDataDirectory 'EBWebView'
    New-Item -ItemType Directory -Path $legacyWebViewDirectory -Force -ErrorAction Stop | Out-Null
    Set-Content `
        -LiteralPath (Join-Path $legacyWebViewDirectory 'installer-smoke-legacy.txt') `
        -Value 'remove with legacy Tauri identifier' `
        -NoNewline
    if ($null -ne $previousInstallerPath) {
        New-TestShortcut -Path $legacyDesktopShortcut -Target $legacyExecutable
        New-TestShortcut -Path $legacyProgramsShortcut -Target $unrelatedShortcutTarget
    }

    Invoke-Installer `
        -Installer $currentInstallerPath `
        -Destination $installDirectory `
        -LogPath $currentInstallLog `
        -Description $(if ($null -ne $previousInstallerPath) { 'Upgrade to current version' } else { 'Install current version' })

    Assert-Installation `
        -Destination $installDirectory `
        -Config $config `
        -Phase Current `
        -Version $ExpectedVersion
    Assert-LegacyAppIdRemoved -Config $config
    if (Test-Path -LiteralPath $legacyTauriDataDirectory) {
        throw "Legacy Tauri data directory was not removed: $legacyTauriDataDirectory"
    }
    $currentRegistrations = @(
        Get-InstalledProductRegistration `
            -Destination $installDirectory `
            -ProductName $config.productName
    )
    $currentRegistrationPath = $currentRegistrations[0].PSPath
    if (Test-Path -LiteralPath $legacyExecutable) {
        throw "Legacy executable was not removed by current installer: $legacyExecutable"
    }
    if (Test-Path -LiteralPath $legacyMcpExecutable) {
        throw "Legacy MCP helper was not removed by current installer: $legacyMcpExecutable"
    }
    $currentExecutable = Join-Path $installDirectory "$($config.mainBinaryName).exe"
    Assert-ShortcutTarget -Path $currentProgramsShortcut -ExpectedTarget $currentExecutable
    Assert-ShortcutAppUserModelId `
        -Path $currentProgramsShortcut `
        -ExpectedAppUserModelId $config.windowsAumid
    if ($null -ne $previousInstallerPath) {
        Assert-ShortcutTarget -Path $currentDesktopShortcut -ExpectedTarget $currentExecutable
        Assert-ShortcutAppUserModelId `
            -Path $currentDesktopShortcut `
            -ExpectedAppUserModelId $config.windowsAumid
        Assert-ShortcutRemoved -Path $legacyDesktopShortcut
        Assert-ShortcutTarget `
            -Path $legacyProgramsShortcut `
            -ExpectedTarget $unrelatedShortcutTarget
        Remove-Item -LiteralPath $legacyProgramsShortcut -Force -ErrorAction Stop
    }

    Assert-McpAccessDisabledByDefault `
        -Destination $installDirectory `
        -Config $config `
        -GuiConfigFile $guiConfigFile `
        -AssertGuiOwnsListener `
        -AssertVccAssociation
    if (-not (Test-Path -LiteralPath $currentTauriDataDirectory -PathType Container)) {
        throw "Current Tauri data directory was not created: $currentTauriDataDirectory"
    }
    if (
        $null -ne $upgradeSentinel -and
        (
            -not (Test-Path -LiteralPath $upgradeSentinel -PathType Leaf) -or
            (Get-Content -LiteralPath $upgradeSentinel -Raw) -cne 'preserve across upgrade'
        )
    ) {
        throw "Upgrade did not preserve the previous version's user data sentinel: $upgradeSentinel"
    }
    Invoke-Uninstall `
        -Destination $installDirectory `
        -Config $config `
        -RegistrationPath $currentRegistrationPath
    if (-not (Test-Path -LiteralPath $applicationDataDirectory -PathType Container)) {
        throw "Silent uninstall removed product data without an explicit choice: $applicationDataDirectory"
    }
    if (-not (Test-Path -LiteralPath $currentTauriDataDirectory -PathType Container)) {
        throw "Silent uninstall removed Tauri data without an explicit choice: $currentTauriDataDirectory"
    }
    foreach ($shortcut in $smokeShortcuts) {
        Assert-ShortcutRemoved -Path $shortcut
    }
    $script:InstallationStarted = $false

    Write-Information "Installer smoke test passed for $($config.productName) $ExpectedVersion." -InformationAction Continue
}
finally {
    if (Test-Path -LiteralPath $technicalLogsDirectory -PathType Container) {
        try {
            Copy-Item `
                -LiteralPath $technicalLogsDirectory `
                -Destination $technicalLogsDiagnosticDirectory `
                -Recurse `
                -Force `
                -ErrorAction Stop
        }
        catch {
            Write-Warning "Failed to collect technical logs for installer smoke diagnostics: $($_.Exception.Message)"
        }
    }

    if ($script:InstallationStarted -and (Test-Path -LiteralPath $installDirectory)) {
        try {
            Stop-InstalledProcess -Destination $installDirectory -Config $config
            $uninstallers = @(Get-ChildItem -LiteralPath $installDirectory -Filter 'unins*.exe' -File -ErrorAction SilentlyContinue)
            if ($uninstallers.Count -eq 1) {
                Invoke-CheckedProcess -FilePath $uninstallers[0].FullName -ArgumentList @(
                    '/VERYSILENT'
                    '/SUPPRESSMSGBOXES'
                    '/NORESTART'
                ) -Description 'Best-effort cleanup uninstall'
            }
        }
        catch {
            Write-Warning "Best-effort smoke cleanup failed: $($_.Exception.Message)"
        }
    }

    if ($null -ne $upgradeSentinel -and (Test-Path -LiteralPath $upgradeSentinel)) {
        Remove-Item -LiteralPath $upgradeSentinel -Force -ErrorAction SilentlyContinue
    }
    if (Test-Path -LiteralPath $legacyTauriDataDirectory) {
        Remove-Item -LiteralPath $legacyTauriDataDirectory -Recurse -Force -ErrorAction SilentlyContinue
    }
    if (Test-Path -LiteralPath $currentTauriDataDirectory) {
        Remove-Item -LiteralPath $currentTauriDataDirectory -Recurse -Force -ErrorAction SilentlyContinue
    }
    if (Test-Path -LiteralPath $applicationDataDirectory) {
        Remove-Item -LiteralPath $applicationDataDirectory -Recurse -Force -ErrorAction SilentlyContinue
    }
    foreach ($shortcut in $smokeShortcuts) {
        if (Test-Path -LiteralPath $shortcut) {
            Remove-Item -LiteralPath $shortcut -Force -ErrorAction SilentlyContinue
        }
    }
}
