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
| 04ea5e4b9713f84142b7460bbbe18162_original.png | 0 | 0 | 0 | 0.0 | 138348 | 9 | 118 | 138562 |
| 51fdac0975494a6f0b5412e3e8b3c19b_original.png | 0 | 0 | 0 | 0.0 | 58493 | 63 | 137 | 58870 |
| english0001.png | 0 | 0 | 0 | 0.0 | 36204 | 50 | 188 | 36538 |
| english0002.png | 0 | 0 | 0 | 0.0 | 55213 | 15 | 159 | 55431 |
| english0003.png | 0 | 0 | 0 | 0.0 | 145872 | 52 | 255 | 146363 |
| english0004.png | 0 | 0 | 0 | 0.0 | 28616 | 10 | 225 | 28949 |
| synth_de_paragraph.png | 0 | 0 | 0 | 0.0 | 28616 | 17 | 44 | 28731 |
| synth_fr_paragraph.png | 0 | 0 | 0 | 0.0 | 26101 | 34 | 203 | 26680 |
| synth_ja_decorative.png | 0 | 0 | 0 | 0.0 | 870 | 0 | 162 | 1071 |
| synth_ja_paragraph.png | 0 | 0 | 0 | 0.0 | 658 | 0 | 320 | 1013 |
| synth_ja_vertical.png | 0 | 0 | 0 | 0.0 | 378 | 7 | 173 | 577 |
| synth_ko_paragraph.png | 0 | 0 | 0 | 0.0 | 6657 | 0 | 315 | 7082 |
| synth_long_paragraph.png | 0 | 0 | 0 | 0.0 | 25461 | 4 | 12 | 25545 |
| synth_mixed_jp_en.png | 0 | 0 | 0 | 0.0 | 38581 | 8 | 188 | 38848 |
| synth_short_lines.png | 0 | 0 | 0 | 0.0 | 40684 | 1 | 144 | 40856 |
| synth_small_body.png | 0 | 0 | 0 | 0.0 | 118239 | 5 | 197 | 118482 |
| synth_title_body.png | 0 | 0 | 0 | 0.0 | 29141 | 2 | 17 | 29188 |
| synth_two_columns.png | 0 | 0 | 0 | 0.0 | 91288 | 15 | 62 | 91474 |
| synth_ui_buttons.png | 0 | 0 | 0 | 0.0 | 14402 | 1 | 15 | 14452 |
| synth_zh_paragraph.png | 0 | 0 | 0 | 0.0 | 582 | 1 | 236 | 863 |

## Translation overflows

### 04ea5e4b9713f84142b7460bbbe18162_original.png

| idx | source | translation | rendered px | card px | ratio |
|----:|--------|-------------|------------:|--------:|------:|
| 10 | `55.3 x 40 x 15.3 mm (
0.2 mm)` | `55.3x40x15.3mm(
0.2mm)` | 34 | 24 | 1.44× |

### synth_mixed_jp_en.png

| idx | source | translation | rendered px | card px | ratio |
|----:|--------|-------------|------------:|--------:|------:|
| 3 | `5n1 and
0 「 m a rainbow. The rainbow iS a division
35 a p` | `5n1and
0「marainbow.TherainbowiSadivision
35ap` | 19 | 11 | 1.75× |

### synth_short_lines.png

| idx | source | translation | rendered px | card px | ratio |
|----:|--------|-------------|------------:|--------:|------:|
| 0 | `Save your WO 「 k.` | `WOを保存してください。「k。` | 22 | 12 | 1.80× |
| 5 | `Quit the application.` | `アプリケーションを終了してください。` | 22 | 12 | 1.80× |

### synth_small_body.png

| idx | source | translation | rendered px | card px | ratio |
|----:|--------|-------------|------------:|--------:|------:|
| 12 | `had no` | `持っていなかった` | 12 | 7 | 1.80× |

### synth_title_body.png

| idx | source | translation | rendered px | card px | ratio |
|----:|--------|-------------|------------:|--------:|------:|
| 3 | `boxing wizardsjump quickly. Pack my bOX with
ve dozen` | `ボクシングの魔法使いは素早くジャンプする。私のbOXを
12個詰めてください` | 17 | 12 | 1.44× |

### synth_two_columns.png

| idx | source | translation | rendered px | card px | ratio |
|----:|--------|-------------|------------:|--------:|------:|
| 2 | `The No` | `「TheNo」を日本語に翻訳してください：` | 23 | 8 | 2.70× |
| 8 | `the` | `次のテキストを日本語に翻訳してください：` | 55 | 9 | 6.30× |

## Aggregate (mean across 20 images)

- Residual chars: **2.4** / image
- Preprocess: 0 ms
- WinRT OCR: 0 ms
- ocrs detect: 0 ms
- Inpaint: 15 ms
- Post-OCR residual check: 158 ms
- **Total: 44479 ms** / image

20 processed, 0 errored. Side-by-side PNGs and per-image JSON live next to this file.
