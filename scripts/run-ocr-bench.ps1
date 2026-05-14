# OCR engine bench runner.
#
# Builds and runs `cargo run --release --example ocr_bench` from the
# src-tauri crate. Outputs land under tests/output_ocr_bench/ with one
# JSON per (engine,image) plus an aggregated summary.{json,md}.
#
# Iteration 1 compares `ocrs` (current production OCR) against
# PP-OCRv5 mobile (via pure-onnx-ocr / tract-onnx). See ocr_bench.rs
# header for the iteration-2 roadmap (manga-ocr ONNX).

param(
    [switch]$Clean
)

$projectRoot = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Definition)
$srcTauri    = Join-Path $projectRoot "src-tauri"
$outDir      = Join-Path $projectRoot "tests\output_ocr_bench"

if ($Clean -and (Test-Path $outDir)) {
    Write-Host "wiping previous results at $outDir"
    Remove-Item -Recurse -Force $outDir
}

Push-Location $srcTauri
try {
    Write-Host "building + running ocr_bench (release)..." -ForegroundColor Cyan
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
