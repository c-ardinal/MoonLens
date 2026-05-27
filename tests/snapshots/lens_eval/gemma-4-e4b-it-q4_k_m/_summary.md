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
| 04ea5e4b9713f84142b7460bbbe18162_original.png | 0 | 0 | 0 | 0.0 | 26347 | 13 | 37 | 26444 |
| 51fdac0975494a6f0b5412e3e8b3c19b_original.png | 0 | 0 | 0 | 0.0 | 13009 | 17 | 57 | 13147 |
| english0001.png | 0 | 0 | 0 | 0.0 | 5612 | 4 | 4 | 5634 |
| english0002.png | 0 | 0 | 0 | 0.0 | 8930 | 6 | 41 | 8995 |
| english0003.png | 0 | 0 | 0 | 0.0 | 41937 | 17 | 42 | 42045 |
| english0004.png | 0 | 0 | 0 | 0.0 | 5026 | 3 | 48 | 5090 |
| synth_long_paragraph.png | 0 | 0 | 0 | 0.0 | 4086 | 3 | 5 | 4106 |
| synth_mixed_jp_en.png | 0 | 0 | 0 | 0.0 | 5392 | 1 | 5 | 5409 |
| synth_short_lines.png | 0 | 0 | 0 | 0.0 | 4454 | 1 | 4 | 4467 |
| synth_small_body.png | 0 | 0 | 0 | 0.0 | 15552 | 2 | 43 | 15610 |
| synth_title_body.png | 0 | 0 | 0 | 0.0 | 4874 | 1 | 4 | 4888 |
| synth_two_columns.png | 0 | 0 | 0 | 0.0 | 18701 | 2 | 9 | 18727 |
| synth_ui_buttons.png | 0 | 0 | 0 | 0.0 | 2035 | 0 | 3 | 2044 |

## Translation overflows

### 04ea5e4b9713f84142b7460bbbe18162_original.png

| idx | source | translation | rendered px | card px | ratio |
|----:|--------|-------------|------------:|--------:|------:|
| 0 | `UltraNixie` | `UltraNixie` | 20 | 16 | 1.25× |
| 1 | `Brand` | `ブランド` | 20 | 16 | 1.25× |
| 2 | `ModeI :` | `モデル：` | 20 | 16 | 1.25× |
| 3 | `Case MateriaI:` | `MateriaIのケースを翻します。` | 42 | 17 | 2.50× |
| 4 | `Zinc—AIuminum AIIOY` | `Zinc—AIuminumAIIOY` | 27 | 22 | 1.25× |
| 5 | `Manufacturing Process .` | `製造プロセス` | 28 | 22 | 1.25× |
| 6 | `Precision Die Casting &` | `精密なダイキャストと` | 27 | 22 | 1.25× |
| 7 | `CNC Machining` | `CNC機械加工` | 27 | 22 | 1.25× |
| 8 | `Tempered Protective GIass` | `耐熱保護ガラス` | 27 | 21 | 1.25× |
| 9 | `GIass .` | `ガーンドです。` | 39 | 16 | 2.50× |
| 11 | `Case DimenSIons .` | `サイズ情報。` | 22 | 17 | 1.25× |
| 12 | `Weight .` | `体重です。` | 28 | 22 | 1.25× |
| 13 | `85g` | `85グラム` | 52 | 21 | 2.50× |
| 14 | `DispIay` | `表示します。` | 56 | 22 | 2.50× |
| 15 | `Authentic Soviet-era` | `ソビет時代の本物のもの` | 20 | 16 | 1.25× |
| 16 | `16 Nixie Tube` | `16NixieTube
16キースチュー` | 41 | 16 | 2.50× |
| 17 | `Battery Capacity.` | `バッテリーップル` | 28 | 22 | 1.25× |
| 18 | `90 mAh` | `90mAhを英語で翻すると"90milliamperehours"です。` | 78 | 16 | 5.00× |
| 19 | `Charging Method :` | `電池充電方法：` | 28 | 22 | 1.25× |
| 20 | `Qi WireIess Charging` | `Qiータの無線充電` | 27 | 22 | 1.25× |
| 21 | `Battery Life .` | `バッテリー寿命` | 28 | 22 | 1.25× |
| 22 | `Up tO 5 days with regular` | `5日間の定期的な` | 27 | 21 | 1.25× |
| 23 | `use (Approx. 100 activations per day)` | `1日で100回の使用が推されます。` | 32 | 25 | 1.25× |
| 24 | `StandbyTime .` | `待機時間.` | 28 | 22 | 1.25× |
| 25 | `Up tO 60 days` | `60日以内にまで届きます。` | 52 | 21 | 2.50× |
| 26 | `lnteraction .` | `相互作用です。` | 22 | 17 | 1.25× |
| 27 | `PhysicaI Buttons` | `物理的なボタンを押すね。` | 54 | 22 | 2.50× |

### 51fdac0975494a6f0b5412e3e8b3c19b_original.png

| idx | source | translation | rendered px | card px | ratio |
|----:|--------|-------------|------------:|--------:|------:|
| 0 | `UItraNixie` | `イーーックス` | 64 | 51 | 1.25× |
| 1 | `Nixie Tube Watch` | `ニコール・タブロwatch` | 52 | 42 | 1.25× |
| 4 | `stability, and battery life.` | `安定性とバッテリーイフです。` | 33 | 27 | 1.25× |
| 5 | `This p 「 OJect is built on the t 「 ust Of ou 「
p 「 evious supporters
and this new` | `この「OJectは、前の支持者の「ustで作られています。」そしてこの新しい` | 60 | 57 | 1.06× |
| 6 | `ve 「 sion iS ou 「 way Of honoring it.` | `「それは、その儀式を尊重する方法です。」` | 34 | 27 | 1.25× |

### english0001.png

| idx | source | translation | rendered px | card px | ratio |
|----:|--------|-------------|------------:|--------:|------:|
| 0 | `The Elicitation Paragraph (Please CaIl StelIa)` | `ステラに電話してください。` | 23 | 19 | 1.25× |

### english0002.png

| idx | source | translation | rendered px | card px | ratio |
|----:|--------|-------------|------------:|--------:|------:|
| 0 | `The North Wind and the Sun (` | `北風と太陽(` | 23 | 18 | 1.25× |
| 1 | `Wikipedia` | `ウィキペディア` | 38 | 15 | 2.50× |
| 3 | `and at last the North Wind gave up the attempt. Then the Sun shined out warmly, and immediatelythe travelertook Off his cloak.
And SO the NO 「 th Wind was obliged tO confess that the Sun was the stronger Ofthe two.` | `そして、最後に北風は試みを放棄しました。その代わり、太陽は暖かく光ってきました。そして旅人は包を脱ぎました。
そして故に、「北風」は太陽が二つの中で強くであることを認めざるを得ませんでした。` | 56 | 44 | 1.28× |

### english0003.png

| idx | source | translation | rendered px | card px | ratio |
|----:|--------|-------------|------------:|--------:|------:|
| 0 | `The Rainbow Passage` | `rainbowpassage` | 23 | 19 | 1.25× |
| 1 | `lnte 「 national Dialects of English Archive (IDEA)` | `日本語で翻すると、以下のようになります：

「国際英語方言アーイブ（IDEA）」です。` | 59 | 16 | 3.75× |
| 2 | `Comma Gets a Cure` | `カンマが治療法を見つけた` | 14 | 11 | 1.25× |
| 4 | `blue.` | `blue. 
青いです。 
青（あお）です。 
青色の文字を日本語に翻します。 

青（あお）です。 
青いです。 
青の文字を日本語に翻します。 
青（あお）です。 
青いです。 
青の文字を日本語に翻します。 
青（あお）です。 
青いです。 
青の文字を日本語に翻します。 
青（あお）です。 
青いです。 
青の文字を日本語に翻します。 
青（あお）です。 
青いです。 
青の文字を日本語に翻します。 
青（あお）です。 
青いです。 
青の文字を日本語に翻します。 
青（あお）です。 
青いです。 
青の文字を日本語に翻します。 
青（あお）です。 
青いです。 
青の文字を日本語に翻します。 
青（あお）です。 
青いです。 
青の文字を日本語に翻します。 
青（あお）です。 
青いです。 
青の文字を日本語に翻します。 
青（あお）です。 
青いです。 
青の文字を日本語に翻します。 
青（あお）です。 
青いです。 
青の文字を日本語に翻します。 
青（あお）です。 
青いです。 
青の文字を日本語に翻します。 
青（あお）です。 
青いです。 
青の文字を日本語に翻します。 
青（あお）です。 
青いです。 
青の文字を日本語に翻します。 
青（あお）です。 
青いです。 
青の文字を日本語に翻します。 
青（あお）です。 
青いです。 
青の文字を日本語に翻します。 
青（あお）です。 
青いです。 
青の文字を日本語に翻します。 
青（あお）です。 
青いです。 
青` | 3135 | 13 | 247.50× |

### synth_mixed_jp_en.png

| idx | source | translation | rendered px | card px | ratio |
|----:|--------|-------------|------------:|--------:|------:|
| 0 | `The Rainbow Passage (` | `レインボース(` | 21 | 17 | 1.25× |
| 1 | `Wikipedia` | `ウィキペディア` | 27 | 11 | 2.50× |
| 2 | `When the sunlight strikes raindrops in the air, they act` | `日光が空にある雨滴に当たると、彼らは行動します。` | 14 | 11 | 1.25× |
| 3 | `5n1 and
0 「 m a rainbow. The rainbow iS a division
35 a p` | `5n1と
0「りの虹です。りの虹は分割の35ap` | 17 | 11 | 1.62× |
| 4 | `ofwhite light into many beautifulcolours.` | `白い光が多くの美しい色に変化する。` | 15 | 12 | 1.25× |

### synth_short_lines.png

| idx | source | translation | rendered px | card px | ratio |
|----:|--------|-------------|------------:|--------:|------:|
| 0 | `Save your WO 「 k.` | `保存して、WO「k」` | 15 | 12 | 1.25× |
| 1 | `Open a new` | `新しいを開きます。` | 28 | 11 | 2.50× |
| 2 | `05e` | `05e` | 9 | 7 | 1.25× |
| 3 | `Search the document.` | `該当する文書を探します。` | 12 | 10 | 1.25× |
| 4 | `Show settings panel.` | `表示設定パネルを表示してください。` | 31 | 12 | 2.50× |
| 5 | `Quit the application.` | `アプリを終了してください。` | 30 | 12 | 2.50× |

### synth_small_body.png

| idx | source | translation | rendered px | card px | ratio |
|----:|--------|-------------|------------:|--------:|------:|
| 0 | `0 im` | `0個のものをお願いします。` | 33 | 7 | 5.00× |
| 1 | `03 d` | `03番です。` | 8 | 7 | 1.25× |
| 2 | `hOfbe( f. W35` | `hOfbe(f.W35` | 10 | 8 | 1.25× |
| 3 | `5. was` | `5.は` | 8 | 6 | 1.25× |
| 4 | `0 n ( du` | `0つ(です)` | 8 | 7 | 1.25× |
| 5 | `O n 5.` | `5個あります。` | 9 | 7 | 1.25× |
| 6 | `W35` | `W35` | 7 | 5 | 1.25× |
| 7 | `gof .` | `行って。` | 11 | 9 | 1.25× |
| 8 | `erofd pair. 、 had` | `erofdpair。、had` | 10 | 8 | 1.25× |
| 9 | `ing` | `イングです。` | 65 | 9 | 7.50× |
| 10 | `bef` | `前の文を日本語に翻します：

彼女はこれらを赤い袋3つにめてくれるでしょう。水曜日にで待ち合わせましょう。

日本語:あなたはこれらのものを3つの赤い袋に入れてください。水曜日にで待っているとしましょう。` | 446 | 7 | 63.75× |
| 11 | `gbefO` | `gbefO` | 11 | 9 | 1.25× |
| 12 | `had no` | `無かった` | 8 | 7 | 1.25× |
| 13 | `goingdi (
w 03` | `行く(
w03` | 17 | 9 | 2.00× |
| 14 | `ⅱ t>dw 50 %` | `50パーント` | 9 | 7 | 1.25× |
| 15 | `goi d ⅳ
ep p iOd. (` | `行きましょう。えいきょくです。(` | 32 | 24 | 1.32× |
| 16 | `i ⅲ 53 d00` | `53で0です。` | 8 | 7 | 1.25× |
| 17 | `its i 0 d. ⅸ g02` | `その中で、0.g02です。` | 11 | 9 | 1.25× |
| 18 | `ompa ⅱ` | `オマパ` | 10 | 8 | 1.25× |

### synth_title_body.png

| idx | source | translation | rendered px | card px | ratio |
|----:|--------|-------------|------------:|--------:|------:|
| 0 | `The Quick Brown FOX` | `Thequickbrownfox` | 24 | 19 | 1.25× |
| 1 | `The quick brown fox jumps 0V0` | `速いブラウンfoxが0V0で跳びます。` | 15 | 12 | 1.25× |
| 2 | `dog. The` | `犬。` | 15 | 12 | 1.25× |
| 4 | `liquorjugs. Sphinxofblackquartz,judge myvow.` | `ライスジョールス。シプサウトブラッククォー、私の誓いを判断する。` | 31 | 12 | 2.50× |

### synth_two_columns.png

| idx | source | translation | rendered px | card px | ratio |
|----:|--------|-------------|------------:|--------:|------:|
| 0 | `Windand the Sunwere` | `風と太陽は` | 11 | 9 | 1.25× |
| 1 | `Wind blew 35 hard` | `風が35強めに吹きつきました。` | 22 | 9 | 2.50× |
| 2 | `The No` | `名の` | 10 | 8 | 1.25× |
| 3 | `Then the No` | `それから、そして、No` | 21 | 8 | 2.50× |
| 4 | `disputing which W35 stronger when` | `W35がどの程度強くか議論していました。` | 14 | 11 | 1.25× |
| 5 | `he could, butthe more he blew` | `彼は吹くことができたが、もっと吹くと更に大変だった。` | 25 | 10 | 2.50× |
| 6 | `3 traveler came along wrapped in` | `3人の光客が包まれて現れた。` | 14 | 11 | 1.25× |
| 7 | `the more closely did the traveler` | `旅人の彼が接近するほど、` | 14 | 11 | 1.25× |
| 8 | `the` | `彼女はこれらのものを赤い袋3つにめてくれるでしょう。水曜日にで待ち合わせましょう。` | 228 | 9 | 26.25× |
| 9 | `his cloak around him; and at` | `彼はそのcloakを身につけている。そしてatでは何が起こったのか。` | 25 | 10 | 2.50× |
| 10 | `a warm cloak. They` | `暖かいコートです。彼らは…` | 27 | 11 | 2.50× |
| 12 | `st the North Wind gave up the` | `北風は北風がめました。` | 14 | 11 | 1.25× |
| 13 | `the traveler take his cloak 0` | `旅人はcloakを取ります。` | 11 | 9 | 1.25× |
| 14 | `n shined 0` | `shine0` | 11 | 9 | 1.25× |
| 15 | `attempt. Then` | `試みた後、` | 13 | 11 | 1.25× |
| 16 | `shou` | `束ねて、青い袋3つにめてください。水曜日にで待ち合わせましょう。` | 124 | 9 | 13.75× |
| 17 | `considered stronger than` | `比較して強いです。` | 14 | 11 | 1.25× |
| 18 | `warmly, and the traveter t00k 0` | `暖かく、そして旅人は帽子を取った。` | 14 | 11 | 1.25× |
| 19 | `his cloak.` | `彼のcloak.` | 11 | 9 | 1.25× |
| 20 | `the other.` | `それの他のものです。` | 22 | 9 | 2.50× |

### synth_ui_buttons.png

| idx | source | translation | rendered px | card px | ratio |
|----:|--------|-------------|------------:|--------:|------:|
| 0 | `File Edit View ⅲ se` | `FileEditView` | 11 | 9 | 1.25× |
| 1 | `Tools Help` | `工具が役立ちます。` | 28 | 11 | 2.50× |
| 2 | `Fo 「 m` | `「メロン」をください。` | 30 | 8 | 3.75× |

## Aggregate (mean across 13 images)

- Residual chars: **0.5** / image
- Preprocess: 0 ms
- WinRT OCR: 0 ms
- ocrs detect: 0 ms
- Inpaint: 5 ms
- Post-OCR residual check: 23 ms
- **Total: 12046 ms** / image

13 processed, 0 errored. Side-by-side PNGs and per-image JSON live next to this file.
