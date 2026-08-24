$ErrorActionPreference = 'Stop'

$repo = Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $repo
cargo build --locked --release -p alcomd-gui --example m7_isolation_probe --features tauri/custom-protocol
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

$executable = Join-Path $repo 'target\release\examples\m7_isolation_probe.exe'
$manifest = Join-Path $repo 'apps\alcomd-gui\src-tauri\m7-probe.windows.manifest'
$windowsKits = Join-Path ${env:ProgramFiles(x86)} 'Windows Kits\10\bin'
$manifestTool = Get-Command mt.exe -ErrorAction SilentlyContinue
if ($null -eq $manifestTool -and (Test-Path -LiteralPath $windowsKits)) {
    $manifestTool = Get-ChildItem -LiteralPath $windowsKits -Recurse -File -Filter mt.exe |
        Where-Object { $_.FullName -match '[\\/]x64[\\/]mt\.exe$' } |
        Sort-Object -Property FullName -Descending |
        Select-Object -First 1
}
if ($null -eq $manifestTool) {
    throw 'Windows SDK mt.exe is required for the test-only WebView probe'
}
$manifestToolPath = if ($manifestTool.Source) { $manifestTool.Source } else { $manifestTool.FullName }
& $manifestToolPath -nologo -manifest $manifest "-outputresource:$executable;#1"
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

python scripts/run-m7-webview-probe.py `
    --executable $executable `
    --platform windows `
    --engine WebView2 `
    --output target/m7-webview-evidence/windows.json
exit $LASTEXITCODE
