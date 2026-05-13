# Run lens_eval three times with different forced tiers so we can
# directly compare translation quality across JA-Std+ (llm-jp 3.1
# 1.8B), Edge E2B (Gemma 4 E2B), and Edge+ E4B (Gemma 4 E4B) on
# the same fixture set. Each run's output is snapshot to its own
# directory under tests/output_3way/<tier>/ before the next run
# overwrites tests/output/.
#
# Usage (from repo root):
#   pwsh -File scripts/run-lens-eval-3way.ps1
#
# Assumes the lens_eval binary has already been built with the
# embedded-llm-vulkan feature:
#   cargo build --example lens_eval --features embedded-llm-vulkan --release
#
# Output directory layout:
#   tests/output_3way/
#     llm-jp-3.1-1.8b-instruct4-q4_k_m/   # JA-Std+ (current bridge)
#       *_eval.png, *_eval.json, _summary.md, ...
#     gemma-4-e2b-it-q4_k_m/              # Standard multi
#       ...
#     gemma-4-e4b-it-q4_k_m/              # Quality multi (Ollama-equivalent)
#       ...

# NOTE: `$ErrorActionPreference` deliberately stays at the default
# `Continue` here. The lens_eval binary writes its llama.cpp init
# preamble (`ggml_vulkan: Found 1 Vulkan devices:` + per-layer
# logging) to stderr, and Windows PowerShell 5.1 wraps each stderr
# line from a native exe in a NativeCommandError. Setting
# `ErrorActionPreference = Stop` would escalate those benign init
# logs into terminating errors and abort the run BEFORE the binary
# has even finished, leaving us with empty `tests/output/`. We
# still detect real failures by checking `$LASTEXITCODE` directly
# after each invocation.

$repoRoot = Split-Path -Parent $PSScriptRoot
$binary = Join-Path $repoRoot "src-tauri\target\release\examples\lens_eval.exe"
if (-not (Test-Path $binary)) {
    Write-Error "lens_eval binary not found at $binary. Build first with `cargo build --example lens_eval --features embedded-llm-vulkan --release`."
}

$outputDir = Join-Path $repoRoot "tests\output"
$snapshotRoot = Join-Path $repoRoot "tests\output_3way"
if (Test-Path $snapshotRoot) {
    Remove-Item -Recurse -Force $snapshotRoot
}
New-Item -ItemType Directory $snapshotRoot | Out-Null

$tiers = @(
    "llm-jp-3.1-1.8b-instruct4-q4_k_m",
    "gemma-4-e2b-it-q4_k_m",
    "gemma-4-e4b-it-q4_k_m"
)

foreach ($tier in $tiers) {
    Write-Host ""
    Write-Host "=== Running lens_eval with force-tier=$tier ==="
    # Wipe the eval output before each run so we don't mix tiers.
    Remove-Item -Recurse -Force $outputDir -ErrorAction SilentlyContinue
    New-Item -ItemType Directory $outputDir | Out-Null

    $env:MOONLENS_EVAL_FORCE_TIER = $tier
    # Redirect stderr to $null so the llama.cpp init preamble does
    # not flood the terminal — exit code is what we care about.
    & $binary 2>$null
    if ($LASTEXITCODE -ne 0) {
        throw "lens_eval exited with code $LASTEXITCODE for tier $tier"
    }

    $tierSnap = Join-Path $snapshotRoot $tier
    Move-Item $outputDir $tierSnap
    New-Item -ItemType Directory $outputDir | Out-Null
    Write-Host "Snapshot saved to $tierSnap"
}

# Remove the empty trailing tests/output (the last loop iteration
# leaves an empty dir behind for tier isolation).
Remove-Item -Recurse -Force $outputDir -ErrorAction SilentlyContinue

Write-Host ""
Write-Host "Done. Three snapshots ready under $snapshotRoot for side-by-side diff."
