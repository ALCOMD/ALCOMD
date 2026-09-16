# Proposal-only JSON Schema/vector validation; no production RPC is invoked.
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$repoRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$schemaPath = Join-Path $repoRoot 'specs/rpc/m7-package-candidates.proposal.schema.json'
$vectorPath = Join-Path $repoRoot 'specs/rpc/m7-package-candidates.contract-vectors.json'
$schema = Get-Content -LiteralPath $schemaPath -Raw | ConvertFrom-Json -AsHashtable
$vectors = Get-Content -LiteralPath $vectorPath -Raw | ConvertFrom-Json -AsHashtable
if ($schema['x-alcomd-publication'] -ne 'proposal-only' -or $schema['x-alcomd-active-rpc-modified']) {
    throw 'This validator is limited to the unapproved proposal.'
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
foreach ($vector in $vectors.semanticVectors) {
    if (-not $ids.Add($vector.id)) { throw "Duplicate vector: $($vector.id)" }
    if ($vector.status -ne 'planned-production-test' -or -not $vector.given -or -not $vector.expected) {
        throw "Incomplete planned semantic vector: $($vector.id)"
    }
}
Write-Output "Proposal shape validation PASS: $count positive/negative cases."
Write-Output "Planned semantic vectors inventoried: $($vectors.semanticVectors.Count); NOT executed production tests."
