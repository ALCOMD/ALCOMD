$ErrorActionPreference = "Stop"
Set-Location (Resolve-Path (Join-Path $PSScriptRoot ".."))

function Require-Command([string]$Name) {
    if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
        throw "Required command '$Name' was not found."
    }
}

Require-Command "rustup"
Require-Command "cargo"
Require-Command "node"
Require-Command "npm"
Require-Command "git"

$nodeVersion = (node --version).TrimStart("v").Split(".")[0]
if ($nodeVersion -ne "24") {
    throw "Node.js 24 LTS is required by .node-version; found major version $nodeVersion."
}

rustup show
npm install

Write-Host "Setup complete. Run .\scripts\check.ps1 -SkipGuiRust"
