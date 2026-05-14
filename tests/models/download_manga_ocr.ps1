# Downloads manga-ocr + comic-text-detector ONNX models for ocr_bench (iter3).
#
# Source: HuggingFace mirrors maintained by `mayocream` (Apache 2.0).
# Total ~555 MB; cached locally so the script is idempotent.
#
# Models exported from:
#   - kha-white/manga-ocr-base (vision encoder-decoder, JP recognition)
#   - dmMaze/comic-text-detector (YOLO-based, manga text detection)

$ErrorActionPreference = "Stop"
$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Definition

$mangaDir = Join-Path $scriptDir "manga_ocr"
$ctdDir = Join-Path $scriptDir "comic_text_detector"
New-Item -ItemType Directory -Force $mangaDir | Out-Null
New-Item -ItemType Directory -Force $ctdDir | Out-Null

function Fetch($url, $target) {
    if (Test-Path $target) {
        $sz = (Get-Item $target).Length
        Write-Host "  cached: $target ($([math]::Round($sz/1MB, 1)) MB)" -ForegroundColor DarkGray
        return
    }
    Write-Host "  downloading: $target" -ForegroundColor Cyan
    Invoke-WebRequest -Uri $url -OutFile $target -UseBasicParsing
    $sz = (Get-Item $target).Length
    Write-Host "    saved $([math]::Round($sz/1MB, 1)) MB" -ForegroundColor Green
}

$mangaBase = "https://huggingface.co/mayocream/manga-ocr-onnx/resolve/main"
Write-Host "manga-ocr-onnx ->" $mangaDir
Fetch "$mangaBase/encoder_model.onnx"        (Join-Path $mangaDir "encoder_model.onnx")
Fetch "$mangaBase/decoder_model.onnx"        (Join-Path $mangaDir "decoder_model.onnx")
Fetch "$mangaBase/vocab.txt"                 (Join-Path $mangaDir "vocab.txt")
Fetch "$mangaBase/config.json"               (Join-Path $mangaDir "config.json")
Fetch "$mangaBase/generation_config.json"    (Join-Path $mangaDir "generation_config.json")
Fetch "$mangaBase/preprocessor_config.json"  (Join-Path $mangaDir "preprocessor_config.json")
Fetch "$mangaBase/special_tokens_map.json"   (Join-Path $mangaDir "special_tokens_map.json")
Fetch "$mangaBase/tokenizer_config.json"     (Join-Path $mangaDir "tokenizer_config.json")

$ctdBase = "https://huggingface.co/mayocream/comic-text-detector-onnx/resolve/main"
Write-Host "comic-text-detector-onnx ->" $ctdDir
Fetch "$ctdBase/comic-text-detector.onnx"    (Join-Path $ctdDir "comic-text-detector.onnx")

Write-Host ""
Write-Host "all done." -ForegroundColor Green
