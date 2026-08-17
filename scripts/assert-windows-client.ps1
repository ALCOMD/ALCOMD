param(
    [Parameter(Mandatory = $true)]
    [ValidateSet("Windows10", "Windows11")]
    [string]$ExpectedClient
)

$ErrorActionPreference = "Stop"
$operatingSystem = Get-CimInstance -ClassName Win32_OperatingSystem
$currentVersion = Get-ItemProperty -LiteralPath "HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion"
$build = [int]$currentVersion.CurrentBuildNumber

if ([int]$operatingSystem.ProductType -ne 1) {
    throw "A Windows client installation is required; ProductType was $($operatingSystem.ProductType)."
}
if ($operatingSystem.OSArchitecture -notmatch "64") {
    throw "A 64-bit Windows client installation is required; found '$($operatingSystem.OSArchitecture)'."
}

switch ($ExpectedClient) {
    "Windows10" {
        if ($build -ne 19045) {
            throw "Windows 10 22H2 build 19045 is required; found build $build."
        }
    }
    "Windows11" {
        if ($build -lt 22000) {
            throw "Windows 11 build 22000 or newer is required; found build $build."
        }
    }
}

Write-Host "Windows client validation: expected=$ExpectedClient; ProductType=$($operatingSystem.ProductType); build=$build; UBR=$($currentVersion.UBR); architecture=$($operatingSystem.OSArchitecture)"
