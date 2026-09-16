# Candidate DTO shape and test-reference validation only; semantic tests are not run here.
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$repoRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$schemaPath = Join-Path $repoRoot 'specs/rpc/m7-package-candidates.proposal.schema.json'
$vectorPath = Join-Path $repoRoot 'specs/rpc/m7-package-candidates.contract-vectors.json'
$schema = Get-Content -LiteralPath $schemaPath -Raw | ConvertFrom-Json -AsHashtable
$vectors = Get-Content -LiteralPath $vectorPath -Raw | ConvertFrom-Json -AsHashtable
if ($schema['x-alcomd-approval'] -ne 'owner-approved') {
    throw 'Candidate contract must retain owner approval metadata.'
}
$count = 0
$ids = [System.Collections.Generic.HashSet[string]]::new()
foreach ($case in $vectors.shapeCases) {
    if (-not $ids.Add($case.id)) { throw "Duplicate vector: $($case.id)" }
    if (-not $schema['$defs'].ContainsKey($case.definition)) { throw 'Unknown schema definition' }
    $wrapper = @{
        '$schema' = $schema['$schema']
        '$defs' = $schema['$defs']
        '$ref' = '#/$defs/' + $case.definition
    } | ConvertTo-Json -Depth 100 -Compress
    $value = $case.value | ConvertTo-Json -Depth 100 -Compress
    $valid = Test-Json -Json $value -Schema $wrapper -ErrorAction SilentlyContinue
    if ($valid -ne $case.valid) { throw "Unexpected schema verdict: $($case.id)" }
    $count++
}
$expectedSemanticIds = @(
    'semver-order', 'build-only', 'same-source-build', 'source-priority', 'equal-priority',
    'repo-user', 'source-pin', 'stable', 'prerelease-off', 'missing-classification',
    'unity-fallback', 'unity-unknown-min', 'unity-unknown-no-min', 'yanked-only',
    'installed-unknown', 'uninstalled', 'installed-newer', 'hidden', 'user-snapshot',
    'unrelated-incomplete', 'relevant-incomplete', 'legacy-metadata', 'snapshot-source-add',
    'snapshot-settings', 'cursor-query-change', 'limits', 'permission', 'zero-effects',
    'dependency-conflict', 'stale-plan', 'selection-search', 'selection-mixed-invalid',
    'bulk-remove', 'bulk-reinstall', 'update-all', 'bulk-limit', 'no-ui-probe',
    'screen-reader', 'bulk-remove-incomplete'
)
if ($vectors.semanticVectors.Count -ne $expectedSemanticIds.Count) {
    throw 'The 39 frozen semantic vectors must not be removed or silently replaced.'
}
$referenceCount = 0
$partialCount = 0
$limitCount = 0
$sourceCache = @{}
foreach ($vector in $vectors.semanticVectors) {
    if (-not $ids.Add($vector.id)) { throw "Duplicate vector: $($vector.id)" }
    if ($vector.id -notin $expectedSemanticIds -or -not $vector.given -or -not $vector.expected) {
        throw "Missing or unexpected semantic vector: $($vector.id)"
    }
    if ($vector.status -notin @('implemented-test-reference', 'partial-test-reference', 'evidence-limit')) {
        throw "Invalid test-reference status: $($vector.id)"
    }
    if ($vector.status -eq 'partial-test-reference') {
        if (-not $vector['coverageGap']) { throw "Missing coverage gap: $($vector.id)" }
        $partialCount++
    }
    if ($vector.status -eq 'evidence-limit') {
        if (-not $vector['evidenceLimit']) { throw "Missing evidence limitation: $($vector.id)" }
        $limitCount++
    }
    if ($vector.id -eq 'screen-reader' -and $vector.status -ne 'evidence-limit') {
        throw 'Host disabled/click checks cannot close screen-reader announcement evidence.'
    }
    if (-not $vector['testReferences'] -or $vector.testReferences.Count -eq 0) {
        throw "Missing executable test references: $($vector.id)"
    }
    foreach ($reference in $vector.testReferences) {
        $relative = [string]$reference.file
        if (-not $relative -or [System.IO.Path]::IsPathRooted($relative) -or $relative -match '(^|[/\\])\.\.([/\\]|$)') {
            throw "Test reference must remain inside the repository: $relative"
        }
        $sourcePath = [System.IO.Path]::GetFullPath((Join-Path $repoRoot $relative))
        if (-not $sourcePath.StartsWith($repoRoot + [System.IO.Path]::DirectorySeparatorChar, [System.StringComparison]::OrdinalIgnoreCase) -or -not (Test-Path -LiteralPath $sourcePath -PathType Leaf)) {
            throw "Referenced test file does not exist: $relative"
        }
        if (-not $sourceCache.ContainsKey($sourcePath)) {
            $sourceCache[$sourcePath] = Get-Content -LiteralPath $sourcePath -Raw
        }
        if (-not $reference.symbol) { throw "Missing test symbol in $relative" }
        $symbol = [regex]::Escape([string]$reference.symbol)
        $pattern = switch ($reference.kind) {
            'rust-test' { '(?m)#\[(?:tokio::)?test(?:\([^\]]*\))?\]\s*(?:async\s+)?fn\s+' + $symbol + '\s*\(' }
            'playwright-test' { '(?m)\btest\(\s*["'']' + $symbol + '["'']\s*,' }
            default { throw "Unknown test-reference kind: $($reference.kind)" }
        }
        if ($sourceCache[$sourcePath] -notmatch $pattern) {
            throw "Referenced test declaration not found: $relative :: $($reference.symbol)"
        }
        $referenceCount++
    }
}
Write-Output "Candidate shape validation PASS: $count positive/negative cases."
Write-Output "Semantic reference inventory PASS: $($vectors.semanticVectors.Count) vectors, $referenceCount test references; $partialCount partial-coverage items; $limitCount evidence-limit item."
Write-Output 'This script did NOT execute semantic tests or establish their PASS status. Referenced test runs and manual evidence remain separate.'
