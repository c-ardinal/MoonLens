# PP-OCRv4-det ONNX model downloader for the lens_eval benchmark.
#
# Downloads `ch_PP-OCRv4_det_infer.onnx` from the RapidAI release mirror
# and places it next to this script. The evaluator picks it up
# automatically when present.

$ErrorActionPreference = "Stop"
$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
$target = Join-Path $scriptDir "ch_PP-OCRv4_det_infer.onnx"

if (Test-Path $target) {
    Write-Host "Model already present at $target"
    exit 0
}

$url = "https://github.com/RapidAI/RapidOCR/releases/download/v1.3.0/ch_PP-OCRv4_det_infer.onnx"
Write-Host "Downloading PP-OCRv4 detection model from $url ..."
Invoke-WebRequest -Uri $url -OutFile $target
$size = (Get-Item $target).Length
Write-Host "Saved $target ($([math]::Round($size/1024/1024, 1)) MB)"
