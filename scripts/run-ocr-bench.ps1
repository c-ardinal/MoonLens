# OCR engine bench runner.
#
# Builds and runs `cargo run --release --example ocr_bench` from the
# src-tauri crate. Output dir defaults to `tests/runs/ocr_bench/`
# (gitignored scratch) with one JSON per (engine,image) plus an
# aggregated summary.{json,md}. Override the output dir with
# `-OutputDir <path>` — e.g. point at `tests/snapshots/ocr_bench/`
# to refresh the committed baseline after reviewing the diff.
#
# Compares all built-in OCR engines (`ocrs`, PP-OCRv5 mobile,
# PP-OCRv5 oar / manga-hybrid where available) against the
# ground truth under `tests/ocr_gt/`.

param(
    [switch]$Clean,
    [string]$OutputDir = ""
)

$projectRoot = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Definition)
$srcTauri    = Join-Path $projectRoot "src-tauri"

if ([string]::IsNullOrEmpty($OutputDir)) {
    $outDir = Join-Path $projectRoot "tests\runs\ocr_bench"
} elseif ([System.IO.Path]::IsPathRooted($OutputDir)) {
    $outDir = $OutputDir
} else {
    $outDir = Join-Path $projectRoot $OutputDir
}

if ($Clean -and (Test-Path $outDir)) {
    Write-Host "wiping previous results at $outDir"
    Remove-Item -Recurse -Force $outDir
}

$env:MOONLENS_OCR_BENCH_OUTPUT_DIR = $outDir

Push-Location $srcTauri
try {
    Write-Host "building + running ocr_bench (release) -> $outDir" -ForegroundColor Cyan
    cargo run --release --example ocr_bench
    if ($LASTEXITCODE -ne 0) {
        throw "ocr_bench exited $LASTEXITCODE"
    }
} finally {
    Pop-Location
}

Write-Host ""
Write-Host "summary written to:" -ForegroundColor Green
Write-Host "  $outDir\summary.json"
Write-Host "  $outDir\summary.md"
