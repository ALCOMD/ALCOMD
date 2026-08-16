param(
    [string]$V3Path = "..\ALCOMD3-v3-readonly",
    [string]$VrcGetPath = "..\vrc-get-readonly",
    [string]$OutputPath = "docs\baselines\source-lock.toml",
    [switch]$Check
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repositoryRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
$v3Repository = "https://github.com/ALCOMD/ALCOMD3.git"
$v3RepositoryIdentity = "alcomd/alcomd3"
$vrcGetRepository = "https://github.com/vrc-get/vrc-get.git"
$vrcGetRepositoryIdentity = "vrc-get/vrc-get"
$migrationTag = "v3.4.0"
$migrationVersion = "3.4.0"
$mcpSpecification = "2026-07-28"
$mcpSpecificationUrl = "https://modelcontextprotocol.io/specification/2026-07-28"
$mcpRepository = "https://github.com/modelcontextprotocol/modelcontextprotocol.git"
$mcpRepositoryIdentity = "modelcontextprotocol/modelcontextprotocol"
$mcpCommit = "4df2d6b6e3588efb46e7542d98498e5c630a0a86"
$mcpSchemaPath = "schema/2026-07-28/schema.ts"
$mcpConformancePackage = "@modelcontextprotocol/conformance"
$mcpConformanceVersion = "0.2.0-alpha.9"

function Resolve-RepositoryPath {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path,
        [Parameter(Mandatory = $true)]
        [string]$Name
    )

    $candidate = if ([System.IO.Path]::IsPathRooted($Path)) {
        $Path
    } else {
        Join-Path $repositoryRoot $Path
    }

    if (-not (Test-Path -LiteralPath $candidate -PathType Container)) {
        throw "$Name repository not found: $candidate"
    }

    return (Resolve-Path -LiteralPath $candidate).Path
}

function ConvertTo-LockPath {
    param(
        [Parameter(Mandatory = $true)]
        [string]$ResolvedPath
    )

    $relative = [System.IO.Path]::GetRelativePath($repositoryRoot, $ResolvedPath)
    $value = if ([System.IO.Path]::IsPathRooted($relative)) {
        $ResolvedPath
    } else {
        $relative
    }

    return $value.Replace("\", "/")
}

function Invoke-Git {
    param(
        [Parameter(Mandatory = $true)]
        [string]$RepositoryPath,
        [Parameter(Mandatory = $true)]
        [string[]]$Arguments
    )

    $output = & git -C $RepositoryPath @Arguments 2>&1
    if ($LASTEXITCODE -ne 0) {
        $command = $Arguments -join " "
        $details = ($output | ForEach-Object { $_.ToString() }) -join "`n"
        throw "git -C $RepositoryPath $command failed:`n$details"
    }

    return (($output | ForEach-Object { $_.ToString() }) -join "`n").Trim()
}

function Get-GitHubRepositoryIdentity {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Url
    )

    $trimmed = $Url.Trim().TrimEnd("/")
    $path = $null

    if ($trimmed -match '^git@github\.com:(?<path>.+)$') {
        $path = $Matches.path
    } else {
        $uri = $null
        if ([System.Uri]::TryCreate($trimmed, [System.UriKind]::Absolute, [ref]$uri) -and
            $uri.Host.Equals("github.com", [System.StringComparison]::OrdinalIgnoreCase)) {
            $path = $uri.AbsolutePath.Trim("/")
        }
    }

    if (-not $path) {
        return $null
    }

    $identity = $path -replace '\.git$', ''
    if (($identity -split "/").Count -ne 2) {
        return $null
    }

    return $identity.ToLowerInvariant()
}

function Assert-DeclaredRemote {
    param(
        [Parameter(Mandatory = $true)]
        [string]$RepositoryPath,
        [Parameter(Mandatory = $true)]
        [string]$Name,
        [Parameter(Mandatory = $true)]
        [string]$ExpectedIdentity
    )

    $remoteNames = @(
        (Invoke-Git -RepositoryPath $RepositoryPath -Arguments @("remote")) -split "`n" |
            Where-Object { $_ }
    )

    foreach ($remoteName in $remoteNames) {
        $urls = @(
            (Invoke-Git -RepositoryPath $RepositoryPath -Arguments @(
                "remote",
                "get-url",
                "--all",
                $remoteName
            )) -split "`n" | Where-Object { $_ }
        )
        foreach ($url in $urls) {
            if ((Get-GitHubRepositoryIdentity -Url $url) -eq $ExpectedIdentity) {
                return
            }
        }
    }

    throw "$Name repository does not declare an HTTPS or SSH remote for $ExpectedIdentity"
}

function Assert-CleanCompleteRepository {
    param(
        [Parameter(Mandatory = $true)]
        [string]$RepositoryPath,
        [Parameter(Mandatory = $true)]
        [string]$Name
    )

    $status = Invoke-Git -RepositoryPath $RepositoryPath -Arguments @("status", "--porcelain")
    if ($status) {
        throw "$Name repository has uncommitted changes and cannot be frozen:`n$status"
    }

    $isShallow = Invoke-Git -RepositoryPath $RepositoryPath -Arguments @(
        "rev-parse",
        "--is-shallow-repository"
    )
    if ($isShallow -ne "false") {
        throw "$Name repository is shallow; complete history and tags are required"
    }
}

function Get-RemoteRefs {
    param(
        [Parameter(Mandatory = $true)]
        [string]$RepositoryPath,
        [Parameter(Mandatory = $true)]
        [string]$RepositoryUrl
    )

    $output = Invoke-Git -RepositoryPath $RepositoryPath -Arguments @(
        "ls-remote",
        "--heads",
        "--tags",
        $RepositoryUrl
    )
    $refs = @{}
    foreach ($line in ($output -split "`n" | Where-Object { $_ })) {
        $parts = $line -split "\s+", 2
        if ($parts.Count -ne 2) {
            throw "Unexpected git ls-remote output: $line"
        }
        $refs[$parts[1]] = $parts[0]
    }
    return $refs
}

function Assert-CompleteTags {
    param(
        [Parameter(Mandatory = $true)]
        [string]$RepositoryPath,
        [Parameter(Mandatory = $true)]
        [string]$Name,
        [Parameter(Mandatory = $true)]
        [hashtable]$RemoteRefs
    )

    $localTags = @(
        (Invoke-Git -RepositoryPath $RepositoryPath -Arguments @("tag", "--list")) -split "`n" |
            Where-Object { $_ } |
            Sort-Object -Unique
    )
    $remoteTags = @(
        $RemoteRefs.Keys |
            Where-Object { $_ -match '^refs/tags/.+' -and $_ -notmatch '\^\{\}$' } |
            ForEach-Object { $_.Substring("refs/tags/".Length) } |
            Sort-Object -Unique
    )
    $difference = @(Compare-Object -ReferenceObject $remoteTags -DifferenceObject $localTags)
    if ($difference.Count -ne 0) {
        $details = ($difference | ForEach-Object { "$($_.SideIndicator) $($_.InputObject)" }) -join "`n"
        throw "$Name local tags do not exactly match the remote tag set:`n$details"
    }
}

function Get-RemoteTagCommit {
    param(
        [Parameter(Mandatory = $true)]
        [hashtable]$RemoteRefs,
        [Parameter(Mandatory = $true)]
        [string]$Tag
    )

    $tagRef = "refs/tags/$Tag"
    $peeledRef = "$tagRef^{}"
    if ($RemoteRefs.ContainsKey($peeledRef)) {
        return $RemoteRefs[$peeledRef]
    }
    if ($RemoteRefs.ContainsKey($tagRef)) {
        return $RemoteRefs[$tagRef]
    }
    throw "Remote tag not found: $Tag"
}

function Get-RemoteBranchForCommit {
    param(
        [Parameter(Mandatory = $true)]
        [hashtable]$RemoteRefs,
        [Parameter(Mandatory = $true)]
        [string]$Commit,
        [string]$PreferredRef
    )

    if ($PreferredRef -and
        $RemoteRefs.ContainsKey($PreferredRef) -and
        $RemoteRefs[$PreferredRef] -eq $Commit) {
        return $PreferredRef
    }

    $matches = @(
        $RemoteRefs.GetEnumerator() |
            Where-Object { $_.Key.StartsWith("refs/heads/") -and $_.Value -eq $Commit } |
            ForEach-Object { $_.Key } |
            Sort-Object
    )
    if ($matches.Count -eq 0) {
        throw "Commit $Commit is not the tip of any branch in the declared remote"
    }
    return $matches[0]
}

function Get-FileTextAtRef {
    param(
        [Parameter(Mandatory = $true)]
        [string]$RepositoryPath,
        [Parameter(Mandatory = $true)]
        [string]$Ref,
        [Parameter(Mandatory = $true)]
        [string]$FilePath
    )

    return Invoke-Git -RepositoryPath $RepositoryPath -Arguments @(
        "show",
        "${Ref}:$FilePath"
    )
}

function Get-BlobIdAtRef {
    param(
        [Parameter(Mandatory = $true)]
        [string]$RepositoryPath,
        [Parameter(Mandatory = $true)]
        [string]$Ref,
        [Parameter(Mandatory = $true)]
        [string]$FilePath
    )

    return Invoke-Git -RepositoryPath $RepositoryPath -Arguments @(
        "rev-parse",
        "${Ref}:$FilePath"
    )
}

function Get-VersionAtRef {
    param(
        [Parameter(Mandatory = $true)]
        [string]$RepositoryPath,
        [Parameter(Mandatory = $true)]
        [string]$Ref,
        [Parameter(Mandatory = $true)]
        [string]$ManifestPath
    )

    $manifest = Get-FileTextAtRef -RepositoryPath $RepositoryPath -Ref $Ref -FilePath $ManifestPath
    $match = [regex]::Match($manifest, '(?m)^version\s*=\s*"([^"]+)"')
    if (-not $match.Success) {
        throw "No version declaration found in ${Ref}:$ManifestPath"
    }

    return $match.Groups[1].Value
}

function Get-Sha256Hex {
    param(
        [Parameter(Mandatory = $true)]
        [byte[]]$Bytes
    )

    $algorithm = [System.Security.Cryptography.SHA256]::Create()
    try {
        $hash = $algorithm.ComputeHash($Bytes)
        return ([System.BitConverter]::ToString($hash) -replace '-', '').ToLowerInvariant()
    } finally {
        $algorithm.Dispose()
    }
}

function Get-FrozenReleaseAsset {
    param(
        [Parameter(Mandatory = $true)]
        [object]$Release,
        [Parameter(Mandatory = $true)]
        [string]$Name
    )

    $matches = @($Release.assets | Where-Object { $_.name -eq $Name })
    if ($matches.Count -ne 1) {
        throw "Expected exactly one release asset named $Name, found $($matches.Count)"
    }
    $asset = $matches[0]
    if ($asset.state -ne "uploaded") {
        throw "Release asset is not uploaded: $Name"
    }
    if ($asset.id -le 0 -or $asset.size -le 0) {
        throw "Release asset has invalid identity or size: $Name"
    }
    if ($asset.digest -notmatch '^sha256:(?<hash>[0-9a-f]{64})$') {
        throw "Release asset has no valid SHA-256 digest: $Name"
    }

    return [pscustomobject]@{
        Id = [long]$asset.id
        Name = [string]$asset.name
        Size = [long]$asset.size
        Sha256 = [string]$Matches.hash
        Url = [string]$asset.browser_download_url
    }
}

function ConvertTo-TomlString {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Value
    )

    return $Value.Replace("\", "\\").Replace('"', '\"')
}

function ConvertTo-TomlStringArray {
    param(
        [Parameter(Mandatory = $true)]
        [AllowEmptyCollection()]
        [string[]]$Values
    )

    $encoded = @($Values | ForEach-Object { '"' + (ConvertTo-TomlString $_) + '"' })
    return "[" + ($encoded -join ", ") + "]"
}

function Format-InvariantInteger {
    param(
        [Parameter(Mandatory = $true)]
        [long]$Value
    )

    return $Value.ToString([System.Globalization.CultureInfo]::InvariantCulture)
}

$resolvedV3Path = Resolve-RepositoryPath -Path $V3Path -Name "ALCOMD3 v3"
$resolvedVrcGetPath = Resolve-RepositoryPath -Path $VrcGetPath -Name "vrc-get"
$lockedV3Path = ConvertTo-LockPath -ResolvedPath $resolvedV3Path
$lockedVrcGetPath = ConvertTo-LockPath -ResolvedPath $resolvedVrcGetPath

Assert-CleanCompleteRepository -RepositoryPath $resolvedV3Path -Name "ALCOMD3 v3"
Assert-CleanCompleteRepository -RepositoryPath $resolvedVrcGetPath -Name "vrc-get"
Assert-DeclaredRemote -RepositoryPath $resolvedV3Path -Name "ALCOMD3 v3" -ExpectedIdentity $v3RepositoryIdentity
Assert-DeclaredRemote -RepositoryPath $resolvedVrcGetPath -Name "vrc-get" -ExpectedIdentity $vrcGetRepositoryIdentity

$v3RemoteRefs = Get-RemoteRefs -RepositoryPath $resolvedV3Path -RepositoryUrl $v3Repository
$vrcGetRemoteRefs = Get-RemoteRefs -RepositoryPath $resolvedVrcGetPath -RepositoryUrl $vrcGetRepository
Assert-CompleteTags -RepositoryPath $resolvedV3Path -Name "ALCOMD3 v3" -RemoteRefs $v3RemoteRefs
Assert-CompleteTags -RepositoryPath $resolvedVrcGetPath -Name "vrc-get" -RemoteRefs $vrcGetRemoteRefs

$v3AuditCommit = Invoke-Git -RepositoryPath $resolvedV3Path -Arguments @("rev-parse", "HEAD^{commit}")
$migrationCommit = Invoke-Git -RepositoryPath $resolvedV3Path -Arguments @(
    "rev-parse",
    "refs/tags/$migrationTag^{commit}"
)
$remoteMigrationCommit = Get-RemoteTagCommit -RemoteRefs $v3RemoteRefs -Tag $migrationTag
if ($migrationCommit -ne $remoteMigrationCommit) {
    throw "Local $migrationTag commit $migrationCommit does not match remote commit $remoteMigrationCommit"
}
if ($v3AuditCommit -ne $migrationCommit) {
    throw "ALCOMD3 audit HEAD must be the $migrationTag migration entry commit"
}
$v3AuditTags = @(
    (Invoke-Git -RepositoryPath $resolvedV3Path -Arguments @("tag", "--points-at", "HEAD")) -split "`n" |
        Where-Object { $_ } |
        Sort-Object -Unique
)
if ($migrationTag -notin $v3AuditTags) {
    throw "ALCOMD3 audit HEAD is not tagged $migrationTag"
}
$v3RemoteRef = Get-RemoteBranchForCommit -RemoteRefs $v3RemoteRefs -Commit $v3AuditCommit -PreferredRef "refs/heads/main"
$v3AuditVersion = Get-VersionAtRef -RepositoryPath $resolvedV3Path -Ref $v3AuditCommit -ManifestPath "Cargo.toml"
$lockedMigrationVersion = Get-VersionAtRef -RepositoryPath $resolvedV3Path -Ref $migrationCommit -ManifestPath "Cargo.toml"
if ($v3AuditVersion -ne $migrationVersion -or $lockedMigrationVersion -ne $migrationVersion) {
    throw "$migrationTag must declare version $migrationVersion"
}
$v3ManifestBlob = Get-BlobIdAtRef -RepositoryPath $resolvedV3Path -Ref $migrationCommit -FilePath "Cargo.toml"

$configPath = "alcomd3.config.json"
$configText = Get-FileTextAtRef -RepositoryPath $resolvedV3Path -Ref $migrationCommit -FilePath $configPath
$config = $configText | ConvertFrom-Json
$configBlob = Get-BlobIdAtRef -RepositoryPath $resolvedV3Path -Ref $migrationCommit -FilePath $configPath
$updateBaseUri = [System.Uri]::new([string]$config.updaterApi.baseUrl, [System.UriKind]::Absolute)
if ($updateBaseUri.Scheme -ne "https") {
    throw "ALCOMD3 updaterApi.baseUrl must use HTTPS"
}
$stableUpdateApi = [System.Uri]::new($updateBaseUri, [string]$config.updaterApi.stablePath).AbsoluteUri
$betaUpdateApi = [System.Uri]::new($updateBaseUri, [string]$config.updaterApi.betaPath).AbsoluteUri

$publicKeyPath = "vrc-get-gui/src/updater-public-key.txt"
$encodedPublicKey = (Get-FileTextAtRef -RepositoryPath $resolvedV3Path -Ref $migrationCommit -FilePath $publicKeyPath).Trim()
$publicKeyBlob = Get-BlobIdAtRef -RepositoryPath $resolvedV3Path -Ref $migrationCommit -FilePath $publicKeyPath
$publicKeyFingerprint = Get-Sha256Hex -Bytes ([System.Text.Encoding]::UTF8.GetBytes($encodedPublicKey))
try {
    $decodedPublicKey = [System.Text.Encoding]::UTF8.GetString(
        [System.Convert]::FromBase64String($encodedPublicKey)
    )
} catch {
    throw "Updater public key is not valid base64: $publicKeyPath"
}
$keyIdMatch = [regex]::Match($decodedPublicKey, '(?m)^untrusted comment: minisign public key: ([0-9A-Fa-f]{16})$')
if (-not $keyIdMatch.Success) {
    throw "Updater public key does not contain a minisign key ID"
}
$publicKeyId = $keyIdMatch.Groups[1].Value.ToUpperInvariant()

$githubHeaders = @{
    Accept = "application/vnd.github+json"
    "X-GitHub-Api-Version" = "2026-03-10"
    "User-Agent" = "ALCOMD-baseline-freezer"
}
$mcpCommitApiUrl = "https://api.github.com/repos/$mcpRepositoryIdentity/commits/$mcpCommit"
$mcpCommitMetadata = Invoke-RestMethod -Headers $githubHeaders -Uri $mcpCommitApiUrl
if ($mcpCommitMetadata.sha -ne $mcpCommit) {
    throw "MCP specification commit identity mismatch"
}
$mcpSchemaApiUrl = "https://api.github.com/repos/$mcpRepositoryIdentity/contents/$mcpSchemaPath`?ref=$mcpCommit"
$mcpSchemaMetadata = Invoke-RestMethod -Headers $githubHeaders -Uri $mcpSchemaApiUrl
if ($mcpSchemaMetadata.type -ne "file" -or $mcpSchemaMetadata.sha -notmatch '^[0-9a-f]{40}$') {
    throw "MCP schema metadata is invalid"
}
$mcpSchemaBytes = [System.Convert]::FromBase64String(
    ([string]$mcpSchemaMetadata.content).Replace("`n", "")
)
$mcpSchemaBlobSha1 = [string]$mcpSchemaMetadata.sha
$mcpSchemaSha256 = Get-Sha256Hex -Bytes $mcpSchemaBytes

$mcpConformanceRegistryUrl = "https://registry.npmjs.org/@modelcontextprotocol%2fconformance/$mcpConformanceVersion"
$mcpConformanceMetadata = Invoke-RestMethod -Uri $mcpConformanceRegistryUrl
if ($mcpConformanceMetadata.name -ne $mcpConformancePackage -or
    $mcpConformanceMetadata.version -ne $mcpConformanceVersion -or
    $mcpConformanceMetadata.dist.shasum -notmatch '^[0-9a-f]{40}$' -or
    $mcpConformanceMetadata.dist.integrity -notmatch '^sha512-[A-Za-z0-9+/]+={0,2}$') {
    throw "MCP conformance package metadata is invalid"
}
$mcpConformanceTarballUrl = [string]$mcpConformanceMetadata.dist.tarball
$mcpConformanceTarballSha1 = [string]$mcpConformanceMetadata.dist.shasum
$mcpConformanceTarballIntegrity = [string]$mcpConformanceMetadata.dist.integrity

$releaseApiUrl = "https://api.github.com/repos/ALCOMD/ALCOMD3/releases/tags/$migrationTag"
$release = Invoke-RestMethod -Headers $githubHeaders -Uri $releaseApiUrl
if ($release.tag_name -ne $migrationTag -or $release.draft -or $release.prerelease) {
    throw "GitHub release must be a published stable $migrationTag release"
}
if ($release.id -le 0 -or -not $release.html_url) {
    throw "GitHub release has invalid identity metadata"
}
if ($null -eq $release.immutable) {
    throw "GitHub release API did not provide the immutable field"
}

$expectedAssetNames = [System.Collections.Generic.HashSet[string]]::new(
    [System.StringComparer]::Ordinal
)
$updaterAssets = @()
$installerAssets = @()
foreach ($platformProperty in $config.releasePlatforms.PSObject.Properties) {
    $platformName = [string]$platformProperty.Name
    $platform = $platformProperty.Value
    $updaterName = ([string]$platform.updater.assetPattern).Replace("{version}", $migrationVersion)
    $signatureName = "$updaterName.sig"
    if (-not $expectedAssetNames.Add($updaterName) -or -not $expectedAssetNames.Add($signatureName)) {
        throw "Duplicate updater asset mapping in $configPath for $platformName"
    }
    $updaterAssets += [pscustomobject]@{
        Platform = $platformName
        Asset = Get-FrozenReleaseAsset -Release $release -Name $updaterName
        Signature = Get-FrozenReleaseAsset -Release $release -Name $signatureName
    }

    foreach ($download in @($platform.downloads)) {
        $installerName = ([string]$download.assetPattern).Replace("{version}", $migrationVersion)
        if (-not $expectedAssetNames.Add($installerName)) {
            throw "Duplicate installer asset mapping in ${configPath}: $installerName"
        }
        $installerAssets += [pscustomobject]@{
            Platform = $platformName
            ConfigId = [string]$download.id
            Format = [string]$download.format
            Primary = [bool]$download.primary
            Asset = Get-FrozenReleaseAsset -Release $release -Name $installerName
        }
    }
}
$actualAssetNames = @($release.assets | ForEach-Object { [string]$_.name } | Sort-Object)
$expectedNames = @($expectedAssetNames | Sort-Object)
$assetDifference = @(Compare-Object -ReferenceObject $expectedNames -DifferenceObject $actualAssetNames)
if ($assetDifference.Count -ne 0) {
    $details = ($assetDifference | ForEach-Object { "$($_.SideIndicator) $($_.InputObject)" }) -join "`n"
    throw "GitHub release assets do not exactly match frozen $configPath patterns:`n$details"
}
$updaterAssets = @($updaterAssets | Sort-Object Platform)
$installerAssets = @($installerAssets | Sort-Object Platform, ConfigId, { $_.Asset.Name })

$vrcGetCommit = Invoke-Git -RepositoryPath $resolvedVrcGetPath -Arguments @("rev-parse", "HEAD^{commit}")
$vrcGetRemoteRef = Get-RemoteBranchForCommit -RemoteRefs $vrcGetRemoteRefs -Commit $vrcGetCommit -PreferredRef "refs/heads/master"
$vrcGetExactTags = @(
    (Invoke-Git -RepositoryPath $resolvedVrcGetPath -Arguments @("tag", "--points-at", "HEAD")) -split "`n" |
        Where-Object { $_ } |
        Sort-Object -Unique
)
$vrcGetCliDescribe = Invoke-Git -RepositoryPath $resolvedVrcGetPath -Arguments @(
    "describe",
    "--tags",
    "--match",
    "v[0-9]*",
    "--long",
    "HEAD"
)
$vrcGetVpmDescribe = Invoke-Git -RepositoryPath $resolvedVrcGetPath -Arguments @(
    "describe",
    "--tags",
    "--match",
    "vpm-v*",
    "--long",
    "HEAD"
)
$vrcGetCliVersion = Get-VersionAtRef -RepositoryPath $resolvedVrcGetPath -Ref $vrcGetCommit -ManifestPath "vrc-get/Cargo.toml"
$vrcGetGuiVersion = Get-VersionAtRef -RepositoryPath $resolvedVrcGetPath -Ref $vrcGetCommit -ManifestPath "vrc-get-gui/Cargo.toml"
$vrcGetVpmVersion = Get-VersionAtRef -RepositoryPath $resolvedVrcGetPath -Ref $vrcGetCommit -ManifestPath "vrc-get-vpm/Cargo.toml"

$publishedAt = ([datetime]$release.published_at).ToUniversalTime().ToString(
    "yyyy-MM-ddTHH:mm:ssZ",
    [System.Globalization.CultureInfo]::InvariantCulture
)
$releaseImmutable = if ([bool]$release.immutable) { "true" } else { "false" }
$vrcExactTag = if ($vrcGetExactTags.Count -gt 0) { "true" } else { "false" }

$lines = [System.Collections.Generic.List[string]]::new()
foreach ($line in @(
    "# Generated by scripts/freeze-baselines.ps1. Do not edit manually.",
    "# frozen = true covers source, specification, and release-asset snapshots only; it does not mean M-1 is complete.",
    "schema = 4",
    "frozen = true",
    "",
    "[alcomd3_v3_audit_source]",
    "repository = `"$(ConvertTo-TomlString $v3Repository)`"",
    "commit = `"$(ConvertTo-TomlString $v3AuditCommit)`"",
    "remote_ref = `"$(ConvertTo-TomlString $v3RemoteRef)`"",
    "tag = `"$migrationTag`"",
    "tag_commit = `"$(ConvertTo-TomlString $remoteMigrationCommit)`"",
    "version = `"$(ConvertTo-TomlString $v3AuditVersion)`"",
    "version_manifest = `"Cargo.toml`"",
    "version_manifest_blob_sha1 = `"$(ConvertTo-TomlString $v3ManifestBlob)`"",
    "local_readonly_path = `"$(ConvertTo-TomlString $lockedV3Path)`"",
    "remote_verified = true",
    "tags_complete = true",
    "shallow = false",
    "status = `"frozen`"",
    "",
    "[alcomd3_v3_migration_entry_release]",
    "repository = `"$(ConvertTo-TomlString $v3Repository)`"",
    "commit = `"$(ConvertTo-TomlString $migrationCommit)`"",
    "tag = `"$migrationTag`"",
    "tag_commit = `"$(ConvertTo-TomlString $remoteMigrationCommit)`"",
    "version = `"$migrationVersion`"",
    "config_path = `"$configPath`"",
    "config_blob_sha1 = `"$(ConvertTo-TomlString $configBlob)`"",
    "stable_update_api = `"$(ConvertTo-TomlString $stableUpdateApi)`"",
    "beta_update_api = `"$(ConvertTo-TomlString $betaUpdateApi)`"",
    "status = `"frozen`"",
    "",
    "[alcomd3_v3_migration_assets]",
    "repository = `"$(ConvertTo-TomlString $v3Repository)`"",
    "release_id = $(Format-InvariantInteger ([long]$release.id))",
    "release_tag = `"$migrationTag`"",
    "release_commit = `"$(ConvertTo-TomlString $remoteMigrationCommit)`"",
    "release_url = `"$(ConvertTo-TomlString ([string]$release.html_url))`"",
    "release_api_url = `"$(ConvertTo-TomlString $releaseApiUrl)`"",
    "release_published_at = `"$publishedAt`"",
    "release_immutable = $releaseImmutable",
    "release_asset_count = $(Format-InvariantInteger ([long]$release.assets.Count))",
    "updater_public_key_path = `"$publicKeyPath`"",
    "updater_public_key_blob_sha1 = `"$(ConvertTo-TomlString $publicKeyBlob)`"",
    "updater_public_key_fingerprint = `"sha256:$publicKeyFingerprint`"",
    "updater_public_key_fingerprint_format = `"sha256-utf8-trimmed-encoded-key`"",
    "updater_public_key_minisign_id = `"$publicKeyId`"",
    "digest_source = `"github-release-api`"",
    "status = `"frozen`""
)) {
    $lines.Add($line)
}

foreach ($entry in $updaterAssets) {
    $lines.Add("")
    $lines.Add("[[alcomd3_v3_migration_assets.updater_assets]]")
    $lines.Add("platform = `"$(ConvertTo-TomlString $entry.Platform)`"")
    $lines.Add("asset_id = $(Format-InvariantInteger $entry.Asset.Id)")
    $lines.Add("asset_name = `"$(ConvertTo-TomlString $entry.Asset.Name)`"")
    $lines.Add("asset_size = $(Format-InvariantInteger $entry.Asset.Size)")
    $lines.Add("asset_sha256 = `"$(ConvertTo-TomlString $entry.Asset.Sha256)`"")
    $lines.Add("asset_url = `"$(ConvertTo-TomlString $entry.Asset.Url)`"")
    $lines.Add("signature_asset_id = $(Format-InvariantInteger $entry.Signature.Id)")
    $lines.Add("signature_asset_name = `"$(ConvertTo-TomlString $entry.Signature.Name)`"")
    $lines.Add("signature_asset_size = $(Format-InvariantInteger $entry.Signature.Size)")
    $lines.Add("signature_asset_sha256 = `"$(ConvertTo-TomlString $entry.Signature.Sha256)`"")
    $lines.Add("signature_asset_url = `"$(ConvertTo-TomlString $entry.Signature.Url)`"")
}

foreach ($entry in $installerAssets) {
    $lines.Add("")
    $lines.Add("[[alcomd3_v3_migration_assets.installer_assets]]")
    $lines.Add("platform = `"$(ConvertTo-TomlString $entry.Platform)`"")
    $lines.Add("config_id = `"$(ConvertTo-TomlString $entry.ConfigId)`"")
    $lines.Add("format = `"$(ConvertTo-TomlString $entry.Format)`"")
    $primary = if ($entry.Primary) { "true" } else { "false" }
    $lines.Add("primary = $primary")
    $lines.Add("asset_id = $(Format-InvariantInteger $entry.Asset.Id)")
    $lines.Add("asset_name = `"$(ConvertTo-TomlString $entry.Asset.Name)`"")
    $lines.Add("asset_size = $(Format-InvariantInteger $entry.Asset.Size)")
    $lines.Add("asset_sha256 = `"$(ConvertTo-TomlString $entry.Asset.Sha256)`"")
    $lines.Add("asset_url = `"$(ConvertTo-TomlString $entry.Asset.Url)`"")
}

foreach ($line in @(
    "",
    "[vrc_get_function_behavior]",
    "repository = `"$(ConvertTo-TomlString $vrcGetRepository)`"",
    "commit = `"$(ConvertTo-TomlString $vrcGetCommit)`"",
    "remote_ref = `"$(ConvertTo-TomlString $vrcGetRemoteRef)`"",
    "exact_tag = $vrcExactTag",
    "exact_tags = $(ConvertTo-TomlStringArray $vrcGetExactTags)",
    "cli_describe = `"$(ConvertTo-TomlString $vrcGetCliDescribe)`"",
    "vpm_describe = `"$(ConvertTo-TomlString $vrcGetVpmDescribe)`"",
    "cli_version = `"$(ConvertTo-TomlString $vrcGetCliVersion)`"",
    "gui_version = `"$(ConvertTo-TomlString $vrcGetGuiVersion)`"",
    "vpm_version = `"$(ConvertTo-TomlString $vrcGetVpmVersion)`"",
    "local_readonly_path = `"$(ConvertTo-TomlString $lockedVrcGetPath)`"",
    "remote_verified = true",
    "tags_complete = true",
    "shallow = false",
    "usage = `"function-and-behavior-audit-only`"",
    "scopes = [`"functionality`", `"security`", `"cli`", `"error-handling`"]",
    "source_reuse = false",
    "status = `"frozen`"",
    "",
    "[mcp]",
    "specification = `"$mcpSpecification`"",
    "url = `"$mcpSpecificationUrl`"",
    "repository = `"$(ConvertTo-TomlString $mcpRepository)`"",
    "commit = `"$(ConvertTo-TomlString $mcpCommit)`"",
    "commit_api_url = `"$(ConvertTo-TomlString $mcpCommitApiUrl)`"",
    "schema_path = `"$(ConvertTo-TomlString $mcpSchemaPath)`"",
    "schema_blob_sha1 = `"$(ConvertTo-TomlString $mcpSchemaBlobSha1)`"",
    "schema_sha256 = `"$(ConvertTo-TomlString $mcpSchemaSha256)`"",
    "conformance_package = `"$(ConvertTo-TomlString $mcpConformancePackage)`"",
    "conformance_version = `"$(ConvertTo-TomlString $mcpConformanceVersion)`"",
    "conformance_registry_url = `"$(ConvertTo-TomlString $mcpConformanceRegistryUrl)`"",
    "conformance_tarball_url = `"$(ConvertTo-TomlString $mcpConformanceTarballUrl)`"",
    "conformance_tarball_sha1 = `"$(ConvertTo-TomlString $mcpConformanceTarballSha1)`"",
    "conformance_tarball_integrity = `"$(ConvertTo-TomlString $mcpConformanceTarballIntegrity)`"",
    "status = `"frozen`""
)) {
    $lines.Add($line)
}

$expectedContent = ($lines -join "`n") + "`n"
$resolvedOutputPath = if ([System.IO.Path]::IsPathRooted($OutputPath)) {
    [System.IO.Path]::GetFullPath($OutputPath)
} else {
    [System.IO.Path]::GetFullPath((Join-Path $repositoryRoot $OutputPath))
}

if ($Check) {
    if (-not (Test-Path -LiteralPath $resolvedOutputPath -PathType Leaf)) {
        throw "Baseline lock file not found: $resolvedOutputPath"
    }

    $actualContent = [System.IO.File]::ReadAllText($resolvedOutputPath)
    if (-not [string]::Equals($actualContent, $expectedContent, [System.StringComparison]::Ordinal)) {
        throw "Baseline lock file is stale. Run scripts/freeze-baselines.ps1 to regenerate it."
    }

    Write-Host "Baseline lock verified: $resolvedOutputPath"
    exit 0
}

$outputDirectory = Split-Path -Parent $resolvedOutputPath
if (-not (Test-Path -LiteralPath $outputDirectory -PathType Container)) {
    throw "Output directory not found: $outputDirectory"
}

$temporaryPath = Join-Path $outputDirectory (
    ".{0}.{1}.tmp" -f ([System.IO.Path]::GetFileName($resolvedOutputPath)), ([guid]::NewGuid())
)
$utf8WithoutBom = New-Object System.Text.UTF8Encoding($false)

try {
    [System.IO.File]::WriteAllText($temporaryPath, $expectedContent, $utf8WithoutBom)
    [System.IO.File]::Move($temporaryPath, $resolvedOutputPath, $true)
} finally {
    if (Test-Path -LiteralPath $temporaryPath) {
        Remove-Item -LiteralPath $temporaryPath -Force
    }
}

Write-Host "Baseline lock generated: $resolvedOutputPath"
