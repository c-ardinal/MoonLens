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
| 04ea5e4b9713f84142b7460bbbe18162_original.png | 0 | 0 | 0 | 0.0 | 33686 | 19 | 87 | 33881 |
| 51fdac0975494a6f0b5412e3e8b3c19b_original.png | 0 | 0 | 0 | 0.0 | 7963 | 31 | 124 | 8231 |
| english0001.png | 0 | 0 | 0 | 0.0 | 3642 | 4 | 61 | 3734 |
| english0002.png | 0 | 0 | 0 | 0.0 | 4870 | 7 | 71 | 4983 |
| english0003.png | 0 | 0 | 0 | 0.0 | 12417 | 31 | 95 | 12609 |
| english0004.png | 0 | 0 | 0 | 0.0 | 3816 | 4 | 77 | 3920 |
| synth_de_paragraph.png | 0 | 0 | 0 | 0.0 | 2152 | 3 | 6 | 2174 |
| synth_fr_paragraph.png | 0 | 0 | 0 | 0.0 | 2071 | 2 | 6 | 2095 |
| synth_ja_decorative.png | 0 | 0 | 0 | 0.0 | 232 | 0 | 58 | 300 |
| synth_ja_paragraph.png | 0 | 0 | 0 | 0.0 | 234 | 0 | 70 | 317 |
| synth_ja_vertical.png | 0 | 0 | 0 | 0.0 | 118 | 0 | 54 | 181 |
| synth_ko_paragraph.png | 0 | 0 | 0 | 0.0 | 624 | 0 | 89 | 723 |
| synth_long_paragraph.png | 0 | 0 | 0 | 0.0 | 3230 | 4 | 6 | 3269 |
| synth_mixed_jp_en.png | 0 | 0 | 0 | 0.0 | 2530 | 1 | 48 | 2596 |
| synth_short_lines.png | 0 | 0 | 0 | 0.0 | 1993 | 1 | 51 | 2055 |
| synth_small_body.png | 0 | 0 | 0 | 0.0 | 7635 | 1 | 78 | 7730 |
| synth_title_body.png | 0 | 0 | 0 | 0.0 | 2598 | 1 | 6 | 2618 |
| synth_two_columns.png | 0 | 0 | 0 | 0.0 | 5233 | 2 | 10 | 5262 |
| synth_ui_buttons.png | 0 | 0 | 0 | 0.0 | 648 | 0 | 4 | 658 |
| synth_zh_paragraph.png | 0 | 0 | 0 | 0.0 | 179 | 0 | 47 | 236 |

## Translation overflows

### 04ea5e4b9713f84142b7460bbbe18162_original.png

| idx | source | translation | rendered px | card px | ratio |
|----:|--------|-------------|------------:|--------:|------:|
| 7 | `CNC Machining` | `CNC加工\|機械加工\|工作機械加工\|工作機械加工業務\|工作機械加工工事\|工作機械加工サース\|機械加工業者\|機械加工会社\|機械加工店\|機械加工工房\|工作機械加工業界\|工作機械加工市場\|工作機械加工技術\|工作機械加工設備\|工作機械加工サース業\|工作機械加工業界の企業\|工作機械加工市場の争\|工作機械加工技術の向上\|工作機械加工設備の更新\|工作機械加工サース業の発展\|工作機械加工市場の成長\|工作機械加工技術の普及\|工作機械加工サース業の成功\|工作機械加工業界の発展\|工作機械加工市場の争力\|工作機械加工技術の向上と普及\|工作機械加工サース業の成功と発展\|工作機械加工市場の成長と争力\|工作機械加工技術の向上と普及の効果\|工作機械加工サース業の成功と発展の効果\|工作機械加工市場の成長と争力の効果\|工作機械加工技術の向上と普及の効果と成功\|工作機械加工サース業の成功と発展の効果と成長の効果\|工作機械加工市場の争力と成長の効果\|工作機械加工技術の向上と普及の効果と成功の効果\|工作機械加工サース業の成功と発展の効果と成長の効果と争力の効果\|工作機械加工市場の争力と成長の効果と成功の効果と争力の効果\|工作機械加工技術の向上と普及の効果と成功の効果と成長の効果と争力の効果と成功の効果\|工作機械加工サース業の成功と発展の効果と成長の効果と争力の効果と成功の効果と争力の効果\|工作機械` | 780 | 22 | 36.00× |

### english0003.png

| idx | source | translation | rendered px | card px | ratio |
|----:|--------|-------------|------------:|--------:|------:|
| 4 | `blue.` | `ブルーーズです。` | 23 | 13 | 1.80× |

### synth_ko_paragraph.png

| idx | source | translation | rendered px | card px | ratio |
|----:|--------|-------------|------------:|--------:|------:|
| 0 | `æ01` | `OK,Iwilldoit.` | 25 | 14 | 1.80× |

### synth_mixed_jp_en.png

| idx | source | translation | rendered px | card px | ratio |
|----:|--------|-------------|------------:|--------:|------:|
| 3 | `5n1 and
0 「 m a rainbow. The rainbow iS a division
35 a p` | `0「marainbow.TherainbowiSadivision
35ap` | 12 | 11 | 1.17× |

### synth_short_lines.png

| idx | source | translation | rendered px | card px | ratio |
|----:|--------|-------------|------------:|--------:|------:|
| 2 | `05e` | `OK,Iwillmakeadecisiononthematter.` | 25 | 7 | 3.60× |

### synth_small_body.png

| idx | source | translation | rendered px | card px | ratio |
|----:|--------|-------------|------------:|--------:|------:|
| 9 | `ing` | `彼女はそれを食べるのではなく、ただ見ているだけです。` | 101 | 9 | 11.70× |
| 11 | `gbefO` | `codeident,modulepath\|filepath\|URL\|ISOtimestamp\|loglevel\|CLIflag\|numericliteral\|hexdigit\|`_`\|`::`\|`/`\|`.`\|`=`\|backticks
```

```
gbefOcodeident,modulepath\|filepath\|URL\|ISOtimestamp\|loglevel\|CLIflag\|numericliteral\|hexdigit\|`_`\|`::`\|`/`\|`.`\|`=`\|backticks
```` | 251 | 9 | 27.90× |
| 12 | `had no` | `持っていなかった` | 12 | 7 | 1.80× |

### synth_two_columns.png

| idx | source | translation | rendered px | card px | ratio |
|----:|--------|-------------|------------:|--------:|------:|
| 2 | `The No` | `No.Idon'tknowwhattosay.Ihaveafeelingthatsomethingiswrongwiththesystem,butit'snotclearwhy.ItseemslikeIcan'tgetanyinformationfromthesystemtoconfirmmytheory.` | 83 | 8 | 9.90× |
| 3 | `Then the No` | `No.Idon'tknowwhattosay.I'mjustasimplemanfromasmalltown.Ihavenoideawhatisgoingonintheworldoutsidemyhouse.Ican'teventellyouexactlywhatkindofflowersarebloomingthere.` | 53 | 8 | 6.30× |
| 16 | `shou` | `レーシング コードID</im_start><im_end>` | 49 | 9 | 5.40× |

### synth_ui_buttons.png

| idx | source | translation | rendered px | card px | ratio |
|----:|--------|-------------|------------:|--------:|------:|
| 2 | `Fo 「 m` | `フォー「m」` | 14 | 8 | 1.80× |

## Aggregate (mean across 20 images)

- Residual chars: **2.4** / image
- Preprocess: 0 ms
- WinRT OCR: 0 ms
- ocrs detect: 0 ms
- Inpaint: 6 ms
- Post-OCR residual check: 52 ms
- **Total: 4879 ms** / image

20 processed, 0 errored. Side-by-side PNGs and per-image JSON live next to this file.
