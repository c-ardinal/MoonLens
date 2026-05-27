# Lens pipeline evaluation

Detector: **winrt-only**

## Quality (residual fragments after inpainting)

| Input | Lines | Residual chars | Residual texts |
|-------|------:|---------------:|----------------|
| 04ea5e4b9713f84142b7460bbbe18162_original.png | 28 | 0 | (none) |
| 51fdac0975494a6f0b5412e3e8b3c19b_original.png | 7 | 0 | (none) |
| english0001.png | 3 | 0 | (none) |
| english0002.png | 4 | 0 | (none) |
| english0003.png | 5 | 3 | `DEA` |
| english0004.png | 1 | 1 | `0` |
| synth_long_paragraph.png | 1 | 0 | (none) |
| synth_mixed_jp_en.png | 5 | 0 | (none) |
| synth_short_lines.png | 6 | 0 | (none) |
| synth_small_body.png | 19 | 2 | `b`, `ⅱ` |
| synth_title_body.png | 5 | 0 | (none) |
| synth_two_columns.png | 21 | 0 | (none) |
| synth_ui_buttons.png | 3 | 0 | (none) |

## Timings (ms)

| Input | Preproc | WinRT | ocrs | Cards | Translate | Inpaint | Post-OCR | Total |
|-------|--------:|------:|-----:|------:|----------:|--------:|---------:|------:|
| 04ea5e4b9713f84142b7460bbbe18162_original.png | 0 | 0 | 0 | 0.0 | 51785 | 14 | 37 | 51882 |
| 51fdac0975494a6f0b5412e3e8b3c19b_original.png | 0 | 0 | 0 | 0.0 | 48718 | 20 | 58 | 48862 |
| english0001.png | 0 | 0 | 0 | 0.0 | 11999 | 4 | 5 | 12022 |
| english0002.png | 0 | 0 | 0 | 0.0 | 16138 | 5 | 40 | 16201 |
| english0003.png | 0 | 0 | 0 | 0.0 | 58584 | 17 | 44 | 58693 |
| english0004.png | 0 | 0 | 0 | 0.0 | 15459 | 3 | 50 | 15526 |
| synth_long_paragraph.png | 0 | 0 | 0 | 0.0 | 17386 | 2 | 9 | 17411 |
| synth_mixed_jp_en.png | 0 | 0 | 0 | 0.0 | 12780 | 1 | 6 | 12799 |
| synth_short_lines.png | 0 | 0 | 0 | 0.0 | 12392 | 1 | 6 | 12407 |
| synth_small_body.png | 0 | 0 | 0 | 0.0 | 39458 | 2 | 46 | 39518 |
| synth_title_body.png | 0 | 0 | 0 | 0.0 | 13309 | 2 | 6 | 13326 |
| synth_two_columns.png | 0 | 0 | 0 | 0.0 | 76395 | 2 | 11 | 76423 |
| synth_ui_buttons.png | 0 | 0 | 0 | 0.0 | 3413 | 0 | 3 | 3422 |

## Translation overflows

### 04ea5e4b9713f84142b7460bbbe18162_original.png

| idx | source | translation | rendered px | card px | ratio |
|----:|--------|-------------|------------:|--------:|------:|
| 0 | `UltraNixie` | `超Nixie` | 20 | 16 | 1.25× |
| 1 | `Brand` | `ブランド` | 20 | 16 | 1.25× |
| 2 | `ModeI :` | `モードI：` | 20 | 16 | 1.25× |
| 3 | `Case MateriaI:` | `ケース材質：` | 21 | 17 | 1.25× |
| 4 | `Zinc—AIuminum AIIOY` | `ジルシ—アルミニウマイオイ` | 27 | 22 | 1.25× |
| 5 | `Manufacturing Process .` | `製造プロセス。` | 28 | 22 | 1.25× |
| 6 | `Precision Die Casting &` | `精密ダイキャスト` | 27 | 22 | 1.25× |
| 7 | `CNC Machining` | `CNC機械加工。` | 27 | 22 | 1.25× |
| 8 | `Tempered Protective GIass` | `耐久保護衣` | 27 | 21 | 1.25× |
| 9 | `GIass .` | `こんにちは、世界。` | 39 | 16 | 2.50× |
| 11 | `Case DimenSIons .` | `ケースの寸法。` | 22 | 17 | 1.25× |
| 12 | `Weight .` | `質量。` | 28 | 22 | 1.25× |
| 13 | `85g` | `85g` | 26 | 21 | 1.25× |
| 14 | `DispIay` | `表示` | 28 | 22 | 1.25× |
| 15 | `Authentic Soviet-era` | `真実のソビエト時代` | 20 | 16 | 1.25× |
| 16 | `16 Nixie Tube` | `16NixieTube.` | 20 | 16 | 1.25× |
| 17 | `Battery Capacity.` | `バッテリ容量` | 28 | 22 | 1.25× |
| 18 | `90 mAh` | `90mAh` | 20 | 16 | 1.25× |
| 19 | `Charging Method :` | `充電方法：` | 28 | 22 | 1.25× |
| 20 | `Qi WireIess Charging` | `Qiワイヤレス充電` | 27 | 22 | 1.25× |
| 21 | `Battery Life .` | `バッテリ寿命。` | 28 | 22 | 1.25× |
| 22 | `Up tO 5 days with regular` | `私はそれが正しく行われているかどうかわからないです。` | 53 | 21 | 2.50× |
| 23 | `use (Approx. 100 activations per day)` | `一日に100回のアクティブ化を使用します。` | 32 | 25 | 1.25× |
| 24 | `StandbyTime .` | `待機時間。` | 28 | 22 | 1.25× |
| 25 | `Up tO 60 days` | `60日間まで。` | 26 | 21 | 1.25× |
| 26 | `lnteraction .` | `Interaction.` | 22 | 17 | 1.25× |
| 27 | `PhysicaI Buttons` | `物理ボタン` | 27 | 22 | 1.25× |

### 51fdac0975494a6f0b5412e3e8b3c19b_original.png

| idx | source | translation | rendered px | card px | ratio |
|----:|--------|-------------|------------:|--------:|------:|
| 0 | `UItraNixie` | `超Nixie` | 64 | 51 | 1.25× |
| 1 | `Nixie Tube Watch` | `ニコライチューー` | 52 | 42 | 1.25× |
| 4 | `stability, and battery life.` | `安定性とバッテリ寿命。` | 33 | 27 | 1.25× |
| 5 | `This p 「 OJect is built on the t 「 ust Of ou 「
p 「 evious supporters
and this new` | `この「OJect」は、以前の支援者の「ust」に基づいて作られていますし、この新しいものも同様です。` | 60 | 57 | 1.06× |
| 6 | `ve 「 sion iS ou 「 way Of honoring it.` | `おはようございます。` | 34 | 27 | 1.25× |

### english0001.png

| idx | source | translation | rendered px | card px | ratio |
|----:|--------|-------------|------------:|--------:|------:|
| 0 | `The Elicitation Paragraph (Please CaIl StelIa)` | `TheElicitationParagraph(PleaseCallStella)` | 23 | 19 | 1.25× |

### english0002.png

| idx | source | translation | rendered px | card px | ratio |
|----:|--------|-------------|------------:|--------:|------:|
| 0 | `The North Wind and the Sun (` | `北風と太陽。` | 23 | 18 | 1.25× |
| 1 | `Wikipedia` | `ウィキペディア` | 38 | 15 | 2.50× |

### english0003.png

| idx | source | translation | rendered px | card px | ratio |
|----:|--------|-------------|------------:|--------:|------:|
| 0 | `The Rainbow Passage` | `TheRainbowPassage.` | 23 | 19 | 1.25× |
| 1 | `lnte 「 national Dialects of English Archive (IDEA)` | `アイデア：英語の方言アーイブ` | 20 | 16 | 1.25× |
| 2 | `Comma Gets a Cure` | `コンマゲットセキュア。` | 14 | 11 | 1.25× |
| 4 | `blue.` | `青色。` | 16 | 13 | 1.25× |

### synth_mixed_jp_en.png

| idx | source | translation | rendered px | card px | ratio |
|----:|--------|-------------|------------:|--------:|------:|
| 0 | `The Rainbow Passage (` | `TheRainbowPassage.` | 21 | 17 | 1.25× |
| 1 | `Wikipedia` | `ウィキペディア` | 27 | 11 | 2.50× |
| 2 | `When the sunlight strikes raindrops in the air, they act` | `日光が雨dropを空気中に当たると、彼らは` | 14 | 11 | 1.25× |
| 4 | `ofwhite light into many beautifulcolours.` | `白光を様々な美しい色に変換します。` | 15 | 12 | 1.25× |

### synth_short_lines.png

| idx | source | translation | rendered px | card px | ratio |
|----:|--------|-------------|------------:|--------:|------:|
| 0 | `Save your WO 「 k.` | `保存yourWO"k."` | 15 | 12 | 1.25× |
| 1 | `Open a new` | `新しいを開きます` | 28 | 11 | 2.50× |
| 2 | `05e` | `05e` | 9 | 7 | 1.25× |
| 3 | `Search the document.` | `索してください。` | 12 | 10 | 1.25× |
| 4 | `Show settings panel.` | `ディスプレイ設定パネル` | 15 | 12 | 1.25× |
| 5 | `Quit the application.` | `終了するアプリケーションを終了します。` | 30 | 12 | 2.50× |

### synth_small_body.png

| idx | source | translation | rendered px | card px | ratio |
|----:|--------|-------------|------------:|--------:|------:|
| 0 | `0 im` | `0は数字です。` | 17 | 7 | 2.50× |
| 1 | `03 d` | `03rdday.` | 8 | 7 | 1.25× |
| 2 | `hOfbe( f. W35` | `hOfbe(f.W35)` | 10 | 8 | 1.25× |
| 3 | `5. was` | `5.was.` | 8 | 6 | 1.25× |
| 4 | `0 n ( du` | `0n(done)` | 8 | 7 | 1.25× |
| 5 | `O n 5.` | `5時です。` | 9 | 7 | 1.25× |
| 6 | `W35` | `W35.` | 7 | 5 | 1.25× |
| 7 | `gof .` | `go.` | 11 | 9 | 1.25× |
| 8 | `erofd pair. 、 had` | `erofdpair.ここがです。` | 10 | 8 | 1.25× |
| 9 | `ing` | `に` | 11 | 9 | 1.25× |
| 10 | `bef` | `以前に` | 18 | 7 | 2.50× |
| 11 | `gbefO` | `gbefO` | 11 | 9 | 1.25× |
| 12 | `had no` | `私は何もありません。` | 17 | 7 | 2.50× |
| 14 | `ⅱ t>dw 50 %` | `II.50%` | 9 | 7 | 1.25× |
| 16 | `i ⅲ 53 d00` | `I53d00.` | 8 | 7 | 1.25× |
| 17 | `its i 0 d. ⅸ g02` | `It'si0d.g02.` | 11 | 9 | 1.25× |
| 18 | `ompa ⅱ` | `オマピ。` | 10 | 8 | 1.25× |

### synth_title_body.png

| idx | source | translation | rendered px | card px | ratio |
|----:|--------|-------------|------------:|--------:|------:|
| 0 | `The Quick Brown FOX` | `Thequickbrownfox.` | 24 | 19 | 1.25× |
| 1 | `The quick brown fox jumps 0V0` | `Thequickbrownfoxjumpsoverthefence.` | 30 | 12 | 2.50× |
| 2 | `dog. The` | `犬。` | 15 | 12 | 1.25× |
| 4 | `liquorjugs. Sphinxofblackquartz,judge myvow.` | `水晶のliquorjugs、私の誓いを判断する。` | 15 | 12 | 1.25× |

### synth_two_columns.png

| idx | source | translation | rendered px | card px | ratio |
|----:|--------|-------------|------------:|--------:|------:|
| 0 | `Windand the Sunwere` | `風と太陽は。` | 11 | 9 | 1.25× |
| 1 | `Wind blew 35 hard` | `風が強く吹きつきました。` | 11 | 9 | 1.25× |
| 2 | `The No` | `その数は。` | 10 | 8 | 1.25× |
| 3 | `Then the No` | `そして、それ以降` | 10 | 8 | 1.25× |
| 4 | `disputing which W35 stronger when` | `W35の強さを争っています。` | 14 | 11 | 1.25× |
| 5 | `he could, butthe more he blew` | `彼はできても、もっとできれば。` | 12 | 10 | 1.25× |
| 6 | `3 traveler came along wrapped in` | `那人戴着一顶帽子。` | 14 | 11 | 1.25× |
| 7 | `the more closely did the traveler` | `旅行者が目的地に近づいた越え越えていた。` | 28 | 11 | 2.50× |
| 8 | `the` | `申しありませんが、指示は不完全です。もう少し情報を提供していただけますか？また、ご要望を明確に教えていただけますか？` | 314 | 9 | 36.25× |
| 9 | `his cloak around him; and at` | `彼は自分のcloakを身につけました。そして、<end_of_turn>` | 25 | 10 | 2.50× |
| 10 | `a warm cloak. They` | `I'msorry,butIdon'tunderstandyourquestion.` | 27 | 11 | 2.50× |
| 12 | `st the North Wind gave up the` | `北風は任務を達成できなくなりました。` | 28 | 11 | 2.50× |
| 13 | `the traveler take his cloak 0` | `旅行者は自分のcloakを取ります。` | 11 | 9 | 1.25× |
| 14 | `n shined 0` | `私はパーティーに行かなければならないのかどうかわからないです。` | 56 | 9 | 6.25× |
| 15 | `attempt. Then` | `試みた。` | 13 | 11 | 1.25× |
| 16 | `shou` | `こんにちは、世界。` | 34 | 9 | 3.75× |
| 17 | `considered stronger than` | `私はパーティーに行かなければならないのかわからない。` | 28 | 11 | 2.50× |
| 18 | `warmly, and the traveter t00k 0` | `温かく、旅行者は0をTook.` | 14 | 11 | 1.25× |
| 19 | `his cloak.` | `ヘスクラック` | 11 | 9 | 1.25× |
| 20 | `the other.` | `それの。` | 11 | 9 | 1.25× |

### synth_ui_buttons.png

| idx | source | translation | rendered px | card px | ratio |
|----:|--------|-------------|------------:|--------:|------:|
| 0 | `File Edit View ⅲ se` | `ファイル、編集、表示` | 11 | 9 | 1.25× |
| 1 | `Tools Help` | `ツールアシスタント` | 28 | 11 | 2.50× |
| 2 | `Fo 「 m` | `「for」` | 10 | 8 | 1.25× |

## Aggregate (mean across 13 images)

- Residual chars: **0.5** / image
- Preprocess: 0 ms
- WinRT OCR: 0 ms
- ocrs detect: 0 ms
- Inpaint: 6 ms
- Post-OCR residual check: 25 ms
- **Total: 29115 ms** / image

13 processed, 0 errored. Side-by-side PNGs and per-image JSON live next to this file.
