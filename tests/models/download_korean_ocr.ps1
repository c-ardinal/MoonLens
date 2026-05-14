# Downloads Korean PP-OCR rec.onnx + dict.txt for the ocr_bench.
#
# Source: monkt/paddleocr-onnx (Apache 2.0). Files mirror the
# PaddleOCR Korean PP-OCR mobile recogniser. The detector (det.onnx)
# is shared with the multilingual PP-OCRv5 model already downloaded
# under tests/models/ppocrv5/ — only the recogniser swap matters.

$ErrorActionPreference = "Stop"
$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
$target = Join-Path $scriptDir "ppocr_ko"
New-Item -ItemType Directory -Force $target | Out-Null

function Fetch($url, $dest) {
    if (Test-Path $dest) {
        $sz = (Get-Item $dest).Length
        Write-Host "  cached: $dest ($([math]::Round($sz/1MB, 2)) MB)" -ForegroundColor DarkGray
        return
    }
    Write-Host "  downloading: $dest" -ForegroundColor Cyan
    Invoke-WebRequest -Uri $url -OutFile $dest -UseBasicParsing
    $sz = (Get-Item $dest).Length
    Write-Host "    saved $([math]::Round($sz/1MB, 2)) MB" -ForegroundColor Green
}

$base = "https://huggingface.co/monkt/paddleocr-onnx/resolve/main/languages/korean"
Fetch "$base/rec.onnx"  (Join-Path $target "rec.onnx")
Fetch "$base/dict.txt"  (Join-Path $target "dict.txt")
Fetch "$base/config.json" (Join-Path $target "config.json")

Write-Host ""
Write-Host "Korean PP-OCR model ready at $target" -ForegroundColor Green
