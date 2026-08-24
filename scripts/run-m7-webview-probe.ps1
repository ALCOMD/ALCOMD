$ErrorActionPreference = 'Stop'

$repo = Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $repo
cargo build --locked -p alcomd-gui --example m7_isolation_probe
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

$executable = Join-Path $repo 'target\debug\examples\m7_isolation_probe.exe'
python scripts/run-m7-webview-probe.py `
    --executable $executable `
    --platform windows `
    --engine WebView2 `
    --output target/m7-webview-evidence/windows.json
exit $LASTEXITCODE
