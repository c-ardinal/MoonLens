# Lens pipeline evaluation

Detector: **winrt-only**

## Quality (residual fragments after inpainting)

| Input | Lines | Residual chars | Residual texts |
|-------|------:|---------------:|----------------|
| 04ea5e4b9713f84142b7460bbbe18162_original.png | 28 | 15 | `UNW21N16SMVIODL` |
| 51fdac0975494a6f0b5412e3e8b3c19b_original.png | 7 | 0 | (none) |
| english0001.png | 3 | 0 | (none) |
| english0002.png | 4 | 0 | (none) |
| english0003.png | 5 | 2 | `EA` |
| english0004.png | 1 | 1 | `0` |
| synth_de_paragraph.png | 1 | 0 | (none) |
| synth_fr_paragraph.png | 1 | 0 | (none) |
| synth_ja_decorative.png | 0 | 0 | (none) |
| synth_ja_paragraph.png | 0 | 0 | (none) |
| synth_ja_vertical.png | 0 | 0 | (none) |
| synth_ko_paragraph.png | 1 | 9 | `OI`, `L`, `「 H`, `Clf` |
| synth_long_paragraph.png | 1 | 0 | (none) |
| synth_mixed_jp_en.png | 5 | 3 | `0 。` |
| synth_short_lines.png | 6 | 1 | `3` |
| synth_small_body.png | 20 | 17 | `b`, `ⅱ`, `、 3 no`, `h`, `0`, `、 00`, `0`, `i`, `0` |
| synth_title_body.png | 5 | 0 | (none) |
| synth_two_columns.png | 21 | 0 | (none) |
| synth_ui_buttons.png | 3 | 0 | (none) |
| synth_zh_paragraph.png | 0 | 0 | (none) |

## Timings (ms)

| Input | Preproc | WinRT | ocrs | Cards | Translate | Inpaint | Post-OCR | Total |
|-------|--------:|------:|-----:|------:|----------:|--------:|---------:|------:|
| 04ea5e4b9713f84142b7460bbbe18162_original.png | 0 | 0 | 0 | 0.0 | 57977 | 9 | 64 | 58096 |
| 51fdac0975494a6f0b5412e3e8b3c19b_original.png | 0 | 0 | 0 | 0.0 | 6016 | 21 | 71 | 6192 |
| english0001.png | 0 | 0 | 0 | 0.0 | 2950 | 4 | 49 | 3017 |
| english0002.png | 0 | 0 | 0 | 0.0 | 13815 | 8 | 106 | 13971 |
| english0003.png | 0 | 0 | 0 | 0.0 | 28399 | 34 | 138 | 28659 |
| english0004.png | 0 | 0 | 0 | 0.0 | 3205 | 7 | 74 | 3307 |
| synth_de_paragraph.png | 0 | 0 | 0 | 0.0 | 3366 | 4 | 10 | 3399 |
| synth_fr_paragraph.png | 0 | 0 | 0 | 0.0 | 3066 | 3 | 7 | 3096 |
| synth_ja_decorative.png | 0 | 0 | 0 | 0.0 | 263 | 0 | 83 | 362 |
| synth_ja_paragraph.png | 0 | 0 | 0 | 0.0 | 254 | 0 | 76 | 342 |
| synth_ja_vertical.png | 0 | 0 | 0 | 0.0 | 200 | 0 | 72 | 290 |
| synth_ko_paragraph.png | 0 | 0 | 0 | 0.0 | 864 | 3 | 87 | 968 |
| synth_long_paragraph.png | 0 | 0 | 0 | 0.0 | 5131 | 2 | 6 | 5158 |
| synth_mixed_jp_en.png | 0 | 0 | 0 | 0.0 | 5910 | 2 | 85 | 6024 |
| synth_short_lines.png | 0 | 0 | 0 | 0.0 | 5536 | 1 | 75 | 5631 |
| synth_small_body.png | 0 | 0 | 0 | 0.0 | 17464 | 2 | 228 | 17800 |
| synth_title_body.png | 0 | 0 | 0 | 0.0 | 7016 | 2 | 8 | 7052 |
| synth_two_columns.png | 0 | 0 | 0 | 0.0 | 21800 | 2 | 16 | 21852 |
| synth_ui_buttons.png | 0 | 0 | 0 | 0.0 | 3037 | 0 | 4 | 3054 |
| synth_zh_paragraph.png | 0 | 0 | 0 | 0.0 | 313 | 0 | 90 | 420 |

## Translation overflows

### 04ea5e4b9713f84142b7460bbbe18162_original.png

| idx | source | translation | rendered px | card px | ratio |
|----:|--------|-------------|------------:|--------:|------:|
| 10 | `55.3 x 40 x 15.3 mm (
0.2 mm)` | `55.3x40x15.3mm(
0.2mm)` | 34 | 24 | 1.44× |
| 25 | `Up tO 60 days` | `60日後まで<end_of_of_turn>` | 38 | 21 | 1.80× |

### 51fdac0975494a6f0b5412e3e8b3c19b_original.png

| idx | source | translation | rendered px | card px | ratio |
|----:|--------|-------------|------------:|--------:|------:|
| 0 | `UItraNixie` | `UItraNixie<end_of_of_turn>` | 92 | 51 | 1.80× |

### english0002.png

| idx | source | translation | rendered px | card px | ratio |
|----:|--------|-------------|------------:|--------:|------:|
| 1 | `Wikipedia` | `ウィキペディア<end_of_of_turn>` | 27 | 15 | 1.80× |

### english0003.png

| idx | source | translation | rendered px | card px | ratio |
|----:|--------|-------------|------------:|--------:|------:|
| 0 | `The Rainbow Passage` | `レインボー・パッセージ<end_of_of_turn>` | 34 | 19 | 1.80× |

### synth_mixed_jp_en.png

| idx | source | translation | rendered px | card px | ratio |
|----:|--------|-------------|------------:|--------:|------:|
| 1 | `Wikipedia` | `ウィキペディア<end_of_of_turn>` | 19 | 11 | 1.80× |
| 3 | `5n1 and
0 「 m a rainbow. The rainbow iS a division
35 a p` | `5n1と
0「虹。虹は一つの分割` | 12 | 11 | 1.17× |

### synth_short_lines.png

| idx | source | translation | rendered px | card px | ratio |
|----:|--------|-------------|------------:|--------:|------:|
| 0 | `Save your WO 「 k.` | `あなたのWOを保存してください「k.` | 22 | 12 | 1.80× |
| 3 | `Search the document.` | `ドキュメントを検索してください。<end_of_of_turn>` | 17 | 10 | 1.80× |
| 5 | `Quit the application.` | `アプリケーションを終了してください。` | 22 | 12 | 1.80× |

### synth_small_body.png

| idx | source | translation | rendered px | card px | ratio |
|----:|--------|-------------|------------:|--------:|------:|
| 19 | `ompa ⅱ` | `おじいさんII<end_of_of_turn>` | 23 | 8 | 2.70× |

## Aggregate (mean across 20 images)

- Residual chars: **2.4** / image
- Preprocess: 0 ms
- WinRT OCR: 0 ms
- ocrs detect: 0 ms
- Inpaint: 5 ms
- Post-OCR residual check: 67 ms
- **Total: 9435 ms** / image

20 processed, 0 errored. Side-by-side PNGs and per-image JSON live next to this file.
