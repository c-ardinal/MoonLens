# Generate a per-line side-by-side comparison of the three tier
# snapshots produced by `run-lens-eval-3way.ps1`. For every image,
# walk its _eval.json `cards[]` array across the three tiers and
# emit a markdown table whose columns are:
#   | source | JA-Std+ | Edge E2B | Edge+ E4B |
#
# Usage (from repo root):
#   pwsh -File scripts/compare-lens-eval-3way.ps1
#
# Output: tests/output_3way/_compare.md

$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $PSScriptRoot
$snapRoot = Join-Path $repoRoot "tests\output_3way"
if (-not (Test-Path $snapRoot)) {
    Write-Error "Snapshot directory $snapRoot not found. Run run-lens-eval-3way.ps1 first."
}

$tierDirs = [ordered]@{
    "JA-Std+ (llm-jp 3.1 1.8B)" = Join-Path $snapRoot "llm-jp-3.1-1.8b-instruct4-q4_k_m"
    "Edge E2B (Gemma 4 E2B)" = Join-Path $snapRoot "gemma-4-e2b-it-q4_k_m"
    "Edge+ E4B (Gemma 4 E4B)" = Join-Path $snapRoot "gemma-4-e4b-it-q4_k_m"
}

# Collect the union of fixture names across tiers (each tier's
# output has the same set, but we discover defensively). Sorted +
# deduped via `Sort-Object -Unique` because PS 5.1's generic
# constructor `New-Object System.Collections.Generic.SortedSet[string]`
# rejects the unambiguous overload at runtime ("Cannot find an
# overload for Add and the argument count: 2"), even though the
# .NET type accepts it. Keeping the implementation plain-array
# also makes the script trivially shell-grep-able.
$fixtureList = @()
foreach ($dir in $tierDirs.Values) {
    foreach ($f in Get-ChildItem $dir -Filter "*_eval.json") {
        $fixtureList += ($f.BaseName -replace '_eval$', '')
    }
}
$fixtures = $fixtureList | Sort-Object -Unique

$out = New-Object System.Text.StringBuilder
[void]$out.AppendLine("# Lens eval 3-way tier comparison")
[void]$out.AppendLine()
[void]$out.AppendLine("Tiers compared (configured via `MOONLENS_EVAL_FORCE_TIER`):")
[void]$out.AppendLine()
foreach ($label in $tierDirs.Keys) {
    [void]$out.AppendLine("- **$label**: ``$($tierDirs[$label] | Split-Path -Leaf)``")
}
[void]$out.AppendLine()
[void]$out.AppendLine("Each row is one OCR-detected card from the fixture; columns hold the translation each tier produced for the same source line. Cells are escaped (newlines → ` ⏎ `) so the table stays single-row per card.")
[void]$out.AppendLine()

function EscapeCell([string]$s) {
    if ($null -eq $s) { return "" }
    return ($s -replace "`r`n", " ⏎ " -replace "`n", " ⏎ " -replace "\|", '\|')
}

foreach ($fixture in $fixtures) {
    [void]$out.AppendLine("## ``$fixture``")
    [void]$out.AppendLine()
    [void]$out.AppendLine("| # | source | $(($tierDirs.Keys -join ' | ')) |")
    [void]$out.AppendLine("|--:|--------|$('---|' * $tierDirs.Keys.Count)")

    # Load each tier's cards into parallel arrays. We key by line
    # index — the underlying pipeline is deterministic per image
    # under fixed OCR, so card indices align across tiers.
    $perTier = @{}
    foreach ($label in $tierDirs.Keys) {
        $jsonPath = Join-Path $tierDirs[$label] "${fixture}_eval.json"
        if (Test-Path $jsonPath) {
            # Force UTF-8 read explicitly. PS 5.1 on Japanese
            # Windows defaults to CP932 (Shift-JIS) for files
            # without a BOM, which mangles every JA character in
            # the JSON ("ウルトラ" → "繧ｦ繝ｫ繝医Λ") and then breaks
            # ConvertFrom-Json on the malformed bytes.
            $raw = [System.IO.File]::ReadAllText($jsonPath, [System.Text.UTF8Encoding]::new($false))
            $perTier[$label] = ($raw | ConvertFrom-Json).cards
        } else {
            $perTier[$label] = @()
        }
    }

    $maxCards = ($perTier.Values | ForEach-Object { @($_).Count } | Measure-Object -Maximum).Maximum
    for ($i = 0; $i -lt $maxCards; $i++) {
        $sourceText = $null
        foreach ($label in $tierDirs.Keys) {
            $cards = $perTier[$label]
            if ($i -lt $cards.Count) {
                $sourceText = $cards[$i].text
                break
            }
        }
        $cells = foreach ($label in $tierDirs.Keys) {
            $cards = $perTier[$label]
            if ($i -lt $cards.Count) {
                EscapeCell($cards[$i].translation)
            } else {
                ""
            }
        }
        [void]$out.AppendLine("| $i | $(EscapeCell($sourceText)) | $($cells -join ' | ') |")
    }
    [void]$out.AppendLine()
}

$outPath = Join-Path $snapRoot "_compare.md"
[System.IO.File]::WriteAllText($outPath, $out.ToString(), [System.Text.UTF8Encoding]::new($false))
Write-Host "Wrote $outPath"
