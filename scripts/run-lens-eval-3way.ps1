# Run lens_eval three times with different forced tiers so we can
# directly compare translation quality across JA-Std+ (the active
# JA bridge), Edge E2B (Gemma 4 E2B), and Edge+ E4B (Gemma 4 E4B)
# on the same fixture set. Each run's output is snapshot to its
# own subdirectory under
#   tests/runs/lens_eval_3way/<tier>/
# (gitignored scratch). Bless a winning run as the new baseline by
# copying the desired subdirectory over to
#   tests/snapshots/lens_eval/<tier>/.
#
# Usage (from repo root):
#   pwsh -File scripts/run-lens-eval-3way.ps1
#
# Assumes the lens_eval binary has already been built with the
# embedded-llm-vulkan feature:
#   cargo build --example lens_eval --features embedded-llm-vulkan --release
#
# Layout produced:
#   tests/runs/lens_eval_3way/
#     hy-mt2-1.8b-q4_k_m/                 # current JA-Std+ bridge
#       *_eval.png, *_eval.json, _summary.md, ...
#     gemma-4-e2b-it-q4_k_m/              # Standard multi
#       ...
#     gemma-4-e4b-it-q4_k_m/              # Quality multi
#       ...

# NOTE: `$ErrorActionPreference` deliberately stays at the default
# `Continue` here. The lens_eval binary writes its llama.cpp init
# preamble (`ggml_vulkan: Found 1 Vulkan devices:` + per-layer
# logging) to stderr, and Windows PowerShell 5.1 wraps each stderr
# line from a native exe in a NativeCommandError. Setting
# `ErrorActionPreference = Stop` would escalate those benign init
# logs into terminating errors and abort the run BEFORE the binary
# has even finished, leaving us with empty output. We still detect
# real failures by checking `$LASTEXITCODE` directly after each
# invocation.

$repoRoot = Split-Path -Parent $PSScriptRoot
$binary = Join-Path $repoRoot "src-tauri\target\release\examples\lens_eval.exe"
if (-not (Test-Path $binary)) {
    Write-Error "lens_eval binary not found at $binary. Build first with ``cargo build --example lens_eval --features embedded-llm-vulkan --release``."
}

$runRoot = Join-Path $repoRoot "tests\runs\lens_eval_3way"
if (Test-Path $runRoot) {
    Remove-Item -Recurse -Force $runRoot
}
New-Item -ItemType Directory $runRoot | Out-Null

$tiers = @(
    "hy-mt2-1.8b-q4_k_m",
    "gemma-4-e2b-it-q4_k_m",
    "gemma-4-e4b-it-q4_k_m"
)

foreach ($tier in $tiers) {
    Write-Host ""
    Write-Host "=== Running lens_eval with force-tier=$tier ==="

    $tierOut = Join-Path $runRoot $tier
    New-Item -ItemType Directory $tierOut | Out-Null

    $env:MOONLENS_EVAL_FORCE_TIER = $tier
    $env:MOONLENS_LENS_EVAL_OUTPUT_DIR = $tierOut
    # Redirect stderr to $null so the llama.cpp init preamble does
    # not flood the terminal — exit code is what we care about.
    & $binary 2>$null
    if ($LASTEXITCODE -ne 0) {
        throw "lens_eval exited with code $LASTEXITCODE for tier $tier"
    }
    Write-Host "Output saved to $tierOut"
}

Write-Host ""
Write-Host "Done. Three runs ready under $runRoot for side-by-side diff."
Write-Host "Bless a baseline by copying a tier dir into tests\snapshots\lens_eval\."
