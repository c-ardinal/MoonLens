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
| 04ea5e4b9713f84142b7460bbbe18162_original.png | 0 | 0 | 0 | 0.0 | 22942 | 14 | 40 | 23041 |
| 51fdac0975494a6f0b5412e3e8b3c19b_original.png | 0 | 0 | 0 | 0.0 | 11050 | 18 | 57 | 11189 |
| english0001.png | 0 | 0 | 0 | 0.0 | 6231 | 4 | 4 | 6252 |
| english0002.png | 0 | 0 | 0 | 0.0 | 9279 | 6 | 41 | 9344 |
| english0003.png | 0 | 0 | 0 | 0.0 | 22643 | 16 | 41 | 22741 |
| english0004.png | 0 | 0 | 0 | 0.0 | 8766 | 3 | 49 | 8832 |
| synth_long_paragraph.png | 0 | 0 | 0 | 0.0 | 5539 | 3 | 5 | 5559 |
| synth_mixed_jp_en.png | 0 | 0 | 0 | 0.0 | 4956 | 1 | 5 | 4972 |
| synth_short_lines.png | 0 | 0 | 0 | 0.0 | 3091 | 1 | 4 | 3103 |
| synth_small_body.png | 0 | 0 | 0 | 0.0 | 11268 | 2 | 46 | 11330 |
| synth_title_body.png | 0 | 0 | 0 | 0.0 | 3922 | 1 | 6 | 3937 |
| synth_two_columns.png | 0 | 0 | 0 | 0.0 | 12574 | 2 | 10 | 12599 |
| synth_ui_buttons.png | 0 | 0 | 0 | 0.0 | 2743 | 0 | 3 | 2751 |

## Translation overflows

### 04ea5e4b9713f84142b7460bbbe18162_original.png

| idx | source | translation | rendered px | card px | ratio |
|----:|--------|-------------|------------:|--------:|------:|
| 0 | `UltraNixie` | `アルファニキ` | 20 | 16 | 1.25× |
| 2 | `ModeI :` | `モードイ：` | 20 | 16 | 1.25× |
| 3 | `Case MateriaI:` | `ケースタリック：` | 21 | 17 | 1.25× |
| 4 | `Zinc—AIuminum AIIOY` | `ジンマール` | 27 | 22 | 1.25× |
| 5 | `Manufacturing Process .` | `製造プロセス` | 28 | 22 | 1.25× |
| 6 | `Precision Die Casting &` | `精密なダイキャスト` | 27 | 22 | 1.25× |
| 7 | `CNC Machining` | `CNCカニクス` | 27 | 22 | 1.25× |
| 8 | `Tempered Protective GIass` | `耐久性のある防護ガー` | 27 | 21 | 1.25× |
| 9 | `GIass .` | `こんにちは。` | 20 | 16 | 1.25× |
| 11 | `Case DimenSIons .` | `サイズについて` | 22 | 17 | 1.25× |
| 12 | `Weight .` | `体重` | 28 | 22 | 1.25× |
| 13 | `85g` | `85グラム` | 52 | 21 | 2.50× |
| 14 | `DispIay` | `表示` | 28 | 22 | 1.25× |
| 15 | `Authentic Soviet-era` | `ソビエト時代の実物` | 20 | 16 | 1.25× |
| 16 | `16 Nixie Tube` | `16キーストー` | 20 | 16 | 1.25× |
| 17 | `Battery Capacity.` | `バッテリーペラ` | 28 | 22 | 1.25× |
| 18 | `90 mAh` | `90mAh` | 20 | 16 | 1.25× |
| 19 | `Charging Method :` | `chargerの方法：` | 28 | 22 | 1.25× |
| 20 | `Qi WireIess Charging` | `Qiーター充電` | 27 | 22 | 1.25× |
| 21 | `Battery Life .` | `バッテリー寿命` | 28 | 22 | 1.25× |
| 22 | `Up tO 5 days with regular` | `5日以内の通常配送` | 27 | 21 | 1.25× |
| 23 | `use (Approx. 100 activations per day)` | `一日に100回使用すること(約)` | 32 | 25 | 1.25× |
| 24 | `StandbyTime .` | `待機時間` | 28 | 22 | 1.25× |
| 25 | `Up tO 60 days` | `60日以内に` | 26 | 21 | 1.25× |
| 26 | `lnteraction .` | `相互作用` | 22 | 17 | 1.25× |
| 27 | `PhysicaI Buttons` | `物理的なボタン` | 27 | 22 | 1.25× |

### 51fdac0975494a6f0b5412e3e8b3c19b_original.png

| idx | source | translation | rendered px | card px | ratio |
|----:|--------|-------------|------------:|--------:|------:|
| 0 | `UItraNixie` | `イーーックス` | 64 | 51 | 1.25× |
| 1 | `Nixie Tube Watch` | `ニキシー・タブーの時計` | 52 | 42 | 1.25× |
| 4 | `stability, and battery life.` | `安定性とバッテリーイフ。` | 33 | 27 | 1.25× |
| 5 | `This p 「 OJect is built on the t 「 ust Of ou 「
p 「 evious supporters
and this new` | `この「オージー」は、以前の支持者たちの「トライアングル」を基に作られています。` | 60 | 57 | 1.06× |
| 6 | `ve 「 sion iS ou 「 way Of honoring it.` | `それは「尊敬するもの」の「道」を示しています。` | 68 | 27 | 2.50× |

### english0001.png

| idx | source | translation | rendered px | card px | ratio |
|----:|--------|-------------|------------:|--------:|------:|
| 0 | `The Elicitation Paragraph (Please CaIl StelIa)` | `エリクションの段落（ステラに電話してください）` | 23 | 19 | 1.25× |

### english0002.png

| idx | source | translation | rendered px | card px | ratio |
|----:|--------|-------------|------------:|--------:|------:|
| 0 | `The North Wind and the Sun (` | `北風と太陽(北风と太阳)` | 23 | 18 | 1.25× |
| 1 | `Wikipedia` | `ウィキペディア` | 38 | 15 | 2.50× |
| 2 | `The North Wind and the Sun were disputing which was the stronger, when a t 「 aveler came along wrapped in a warm cloak. They
agreed that the one wh0 first succeeded in making the t 「 aveler take his cloak 0ff should be conside 「 ed stronge 「 than the 0ther.
Then the North Wind blew as hard as he could, but the more he blew the more closely did the traveler fo
his cloak a 「 ound him;` | `北風と太陽はどちらが強いか争っていました。ある「avelerが暖かいコートを着て現れたとき、彼らは
最初に「avelerのコートを取り外せる人を選ばれるべきであると決めました。
そして北風が全力で吹き始めましたが、その越え越えてコートを着ている人のほど近づいて来たようでした。` | 84 | 72 | 1.17× |
| 3 | `and at last the North Wind gave up the attempt. Then the Sun shined out warmly, and immediatelythe travelertook Off his cloak.
And SO the NO 「 th Wind was obliged tO confess that the Sun was the stronger Ofthe two.` | `そして最後に北風は試みを放棄しました。すると太陽は暖かく光り、旅行者はすぐに羽りを脱ぎました。そしてその結果、「北風は二つ目の太陽が強かったことを認めざるを得ませんでした」でした。` | 56 | 44 | 1.28× |

### english0003.png

| idx | source | translation | rendered px | card px | ratio |
|----:|--------|-------------|------------:|--------:|------:|
| 0 | `The Rainbow Passage` | `虹の道` | 23 | 19 | 1.25× |
| 1 | `lnte 「 national Dialects of English Archive (IDEA)` | `「国際Dialettiingleeの英語アーイブ(IDEA)」` | 20 | 16 | 1.25× |
| 2 | `Comma Gets a Cure` | `カンマが治す` | 14 | 11 | 1.25× |
| 4 | `blue.` | `青色` | 16 | 13 | 1.25× |

### synth_long_paragraph.png

| idx | source | translation | rendered px | card px | ratio |
|----:|--------|-------------|------------:|--------:|------:|
| 0 | `the senses, sight must be the most delightful.
Few things give us greater pleasure than tO be able tO see again
afterwe have been prevented from doing so f0 「 a length oftime.
Yet though sight is 50 precious a possession, comparatively few
people make full use 0f it. They may indeed glance at trees and
animals and clouds, but they
most never observe them.` | `感覚の一つ目は最も足なものです。
多くのものは、視力を失った後で再び見る事が出来ることを可能にする事によりますと、その喜びは大きくなります。
しかし、目は比較的貴重な財産です。しかしながら、多くの人々は視力を失った後で再び見る事が出来ることを可能にするためのその喜びを最大限に享受しません。彼らは確かに木々や動物、雲などを見ましたが、それらを最もはっきりと見ることはほとんどありません。` | 168 | 132 | 1.27× |

### synth_mixed_jp_en.png

| idx | source | translation | rendered px | card px | ratio |
|----:|--------|-------------|------------:|--------:|------:|
| 0 | `The Rainbow Passage (` | `虹の道（あかひらなわ）` | 21 | 17 | 1.25× |
| 1 | `Wikipedia` | `ウィキペディア` | 27 | 11 | 2.50× |
| 2 | `When the sunlight strikes raindrops in the air, they act` | `日光が空に降った雨滴を照らすとき、彼らは行動します。` | 14 | 11 | 1.25× |
| 3 | `5n1 and
0 「 m a rainbow. The rainbow iS a division
35 a p` | `5n1そして
0「marainbow.TherainbowiSadivision
35ap` | 26 | 11 | 2.44× |
| 4 | `ofwhite light into many beautifulcolours.` | `白い光が様々な美しい色に変化する。` | 15 | 12 | 1.25× |

### synth_short_lines.png

| idx | source | translation | rendered px | card px | ratio |
|----:|--------|-------------|------------:|--------:|------:|
| 0 | `Save your WO 「 k.` | `保存するWO「k.` | 15 | 12 | 1.25× |
| 1 | `Open a new` | `新しいを開きます` | 28 | 11 | 2.50× |
| 2 | `05e` | `05` | 9 | 7 | 1.25× |
| 3 | `Search the document.` | `索してください。` | 12 | 10 | 1.25× |
| 4 | `Show settings panel.` | `設定パネルを表示します。` | 15 | 12 | 1.25× |
| 5 | `Quit the application.` | `アプリを終了してください。` | 30 | 12 | 2.50× |

### synth_small_body.png

| idx | source | translation | rendered px | card px | ratio |
|----:|--------|-------------|------------:|--------:|------:|
| 0 | `0 im` | `0個` | 8 | 7 | 1.25× |
| 1 | `03 d` | `03日` | 8 | 7 | 1.25× |
| 2 | `hOfbe( f. W35` | `hOfbe(f.W35` | 10 | 8 | 1.25× |
| 3 | `5. was` | `5.は` | 8 | 6 | 1.25× |
| 4 | `0 n ( du` | `0つ(の)` | 8 | 7 | 1.25× |
| 5 | `O n 5.` | `5日目` | 9 | 7 | 1.25× |
| 6 | `W35` | `W35` | 7 | 5 | 1.25× |
| 7 | `gof .` | `行こう。` | 11 | 9 | 1.25× |
| 8 | `erofd pair. 、 had` | `エロフィールドー、それについて。` | 21 | 8 | 2.50× |
| 9 | `ing` | `ing` | 11 | 9 | 1.25× |
| 10 | `bef` | `彼女はこれらを赤い袋3つにめてくれるでしょう。水曜日にで待ち合わせましょう。` | 166 | 7 | 23.75× |
| 11 | `gbefO` | `ええと、お前が何かを書くのを手伝ってくれるか？` | 90 | 9 | 10.00× |
| 12 | `had no` | `Non` | 8 | 7 | 1.25× |
| 14 | `ⅱ t>dw 50 %` | `10%` | 9 | 7 | 1.25× |
| 16 | `i ⅲ 53 d00` | `53d00を翻します。` | 8 | 7 | 1.25× |
| 17 | `its i 0 d. ⅸ g02` | `それは何ですか。` | 11 | 9 | 1.25× |
| 18 | `ompa ⅱ` | `オマパ` | 10 | 8 | 1.25× |

### synth_title_body.png

| idx | source | translation | rendered px | card px | ratio |
|----:|--------|-------------|------------:|--------:|------:|
| 0 | `The Quick Brown FOX` | `Thequickbrownfox.` | 24 | 19 | 1.25× |
| 1 | `The quick brown fox jumps 0V0` | `quickbrownfoxjumpsoverthelazydog` | 15 | 12 | 1.25× |
| 2 | `dog. The` | `犬。` | 15 | 12 | 1.25× |
| 3 | `boxing wizardsjump quickly. Pack my bOX with
ve dozen` | `ボクシングWizardsは速く跳びます。私のBOXをめましょう、12個のもの。` | 24 | 12 | 2.00× |
| 4 | `liquorjugs. Sphinxofblackquartz,judge myvow.` | `酒瓶。いSphinx、私の誓いを判断します。` | 15 | 12 | 1.25× |

### synth_two_columns.png

| idx | source | translation | rendered px | card px | ratio |
|----:|--------|-------------|------------:|--------:|------:|
| 0 | `Windand the Sunwere` | `風と太陽は` | 11 | 9 | 1.25× |
| 1 | `Wind blew 35 hard` | `風が35メートルで吹きました。` | 22 | 9 | 2.50× |
| 2 | `The No` | `TheNo` | 10 | 8 | 1.25× |
| 3 | `Then the No` | `その後、No` | 10 | 8 | 1.25× |
| 4 | `disputing which W35 stronger when` | `W35がどのくらい強いかを争っています。` | 14 | 11 | 1.25× |
| 5 | `he could, butthe more he blew` | `彼は吹くことができたが、もっと吹くと。` | 12 | 10 | 1.25× |
| 6 | `3 traveler came along wrapped in` | `3名の旅人があたふつと包まれて来ました` | 14 | 11 | 1.25× |
| 7 | `the more closely did the traveler` | `旅行者があなたの要求にどのように近づいたか` | 28 | 11 | 2.50× |
| 8 | `the` | `th` | 11 | 9 | 1.25× |
| 9 | `his cloak around him; and at` | `彼のcloakを身につけている；そして` | 12 | 10 | 1.25× |
| 10 | `a warm cloak. They` | `暖かいコート。` | 13 | 11 | 1.25× |
| 12 | `st the North Wind gave up the` | `北風は雪を止める` | 14 | 11 | 1.25× |
| 13 | `the traveler take his cloak 0` | `旅人はcloakを取ります` | 11 | 9 | 1.25× |
| 14 | `n shined 0` | `0を輝かせろ` | 11 | 9 | 1.25× |
| 15 | `attempt. Then` | `試みる。` | 13 | 11 | 1.25× |
| 16 | `shou` | `申しありません、その翻は適切ではありません。` | 90 | 9 | 10.00× |
| 17 | `considered stronger than` | `より強くregarded` | 14 | 11 | 1.25× |
| 18 | `warmly, and the traveter t00k 0` | `暖かく、そして旅人は帽子を取った。` | 14 | 11 | 1.25× |
| 19 | `his cloak.` | `彼のcloak.` | 11 | 9 | 1.25× |
| 20 | `the other.` | `それ以外の` | 11 | 9 | 1.25× |

### synth_ui_buttons.png

| idx | source | translation | rendered px | card px | ratio |
|----:|--------|-------------|------------:|--------:|------:|
| 0 | `File Edit View ⅲ se` | `ファイルィレクションュー` | 11 | 9 | 1.25× |
| 1 | `Tools Help` | `工具が役立ちます。` | 28 | 11 | 2.50× |
| 2 | `Fo 「 m` | `彼女はこれらを赤い袋3つにめてくれるでしょう。水曜日にで待ち合わせましょう。` | 100 | 8 | 12.50× |

## Aggregate (mean across 13 images)

- Residual chars: **0.5** / image
- Preprocess: 0 ms
- WinRT OCR: 0 ms
- ocrs detect: 0 ms
- Inpaint: 5 ms
- Post-OCR residual check: 24 ms
- **Total: 9665 ms** / image

13 processed, 0 errored. Side-by-side PNGs and per-image JSON live next to this file.
