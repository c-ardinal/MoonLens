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
| 04ea5e4b9713f84142b7460bbbe18162_original.png | 0 | 0 | 0 | 0.0 | 23794 | 14 | 39 | 23891 |
| 51fdac0975494a6f0b5412e3e8b3c19b_original.png | 0 | 0 | 0 | 0.0 | 12475 | 17 | 56 | 12612 |
| english0001.png | 0 | 0 | 0 | 0.0 | 5602 | 4 | 4 | 5625 |
| english0002.png | 0 | 0 | 0 | 0.0 | 10225 | 6 | 41 | 10289 |
| english0003.png | 0 | 0 | 0 | 0.0 | 40506 | 17 | 42 | 40615 |
| english0004.png | 0 | 0 | 0 | 0.0 | 5549 | 3 | 48 | 5614 |
| synth_long_paragraph.png | 0 | 0 | 0 | 0.0 | 4673 | 2 | 5 | 4693 |
| synth_mixed_jp_en.png | 0 | 0 | 0 | 0.0 | 6377 | 1 | 5 | 6397 |
| synth_short_lines.png | 0 | 0 | 0 | 0.0 | 4870 | 1 | 4 | 4883 |
| synth_small_body.png | 0 | 0 | 0 | 0.0 | 21364 | 2 | 44 | 21424 |
| synth_title_body.png | 0 | 0 | 0 | 0.0 | 4657 | 1 | 4 | 4670 |
| synth_two_columns.png | 0 | 0 | 0 | 0.0 | 20545 | 2 | 10 | 20572 |
| synth_ui_buttons.png | 0 | 0 | 0 | 0.0 | 2917 | 0 | 3 | 2925 |

## Translation overflows

### 04ea5e4b9713f84142b7460bbbe18162_original.png

| idx | source | translation | rendered px | card px | ratio |
|----:|--------|-------------|------------:|--------:|------:|
| 0 | `UltraNixie` | `UltraNixie` | 20 | 16 | 1.25× |
| 1 | `Brand` | `ブランド` | 20 | 16 | 1.25× |
| 2 | `ModeI :` | `モデル：` | 20 | 16 | 1.25× |
| 3 | `Case MateriaI:` | `MateriaIのケース:` | 21 | 17 | 1.25× |
| 4 | `Zinc—AIuminum AIIOY` | `Zinc—AIuminumAIIOY` | 27 | 22 | 1.25× |
| 5 | `Manufacturing Process .` | `製造プロセス` | 28 | 22 | 1.25× |
| 6 | `Precision Die Casting &` | `精密なダイキャストと` | 27 | 22 | 1.25× |
| 7 | `CNC Machining` | `CNC加工` | 27 | 22 | 1.25× |
| 8 | `Tempered Protective GIass` | `強化された保護用ガー` | 27 | 21 | 1.25× |
| 9 | `GIass .` | `こんにちは。` | 20 | 16 | 1.25× |
| 11 | `Case DimenSIons .` | `サイズ情報。` | 22 | 17 | 1.25× |
| 12 | `Weight .` | `体重です。` | 28 | 22 | 1.25× |
| 13 | `85g` | `85グラムです。` | 105 | 21 | 5.00× |
| 14 | `DispIay` | `表示` | 28 | 22 | 1.25× |
| 15 | `Authentic Soviet-era` | `ソビエト時代の真っ最中` | 20 | 16 | 1.25× |
| 16 | `16 Nixie Tube` | `16キスチュー` | 20 | 16 | 1.25× |
| 17 | `Battery Capacity.` | `バッテリースティション` | 28 | 22 | 1.25× |
| 18 | `90 mAh` | `90mAhのバッテリーです。` | 39 | 16 | 2.50× |
| 19 | `Charging Method :` | `電源供給方法：` | 28 | 22 | 1.25× |
| 20 | `Qi WireIess Charging` | `Qichargeless` | 27 | 22 | 1.25× |
| 21 | `Battery Life .` | `バッテリー寿命` | 28 | 22 | 1.25× |
| 22 | `Up tO 5 days with regular` | `5日間の定期的な` | 27 | 21 | 1.25× |
| 23 | `use (Approx. 100 activations per day)` | `1日に100回の使用を約する場合、` | 32 | 25 | 1.25× |
| 24 | `StandbyTime .` | `待機時間` | 28 | 22 | 1.25× |
| 25 | `Up tO 60 days` | `60日以内にまで届くと。` | 52 | 21 | 2.50× |
| 26 | `lnteraction .` | `相互作用です。` | 22 | 17 | 1.25× |
| 27 | `PhysicaI Buttons` | `物理的ボタン` | 27 | 22 | 1.25× |

### 51fdac0975494a6f0b5412e3e8b3c19b_original.png

| idx | source | translation | rendered px | card px | ratio |
|----:|--------|-------------|------------:|--------:|------:|
| 0 | `UItraNixie` | `イーンニシ` | 64 | 51 | 1.25× |
| 1 | `Nixie Tube Watch` | `ニコール・チューの時計です。` | 52 | 42 | 1.25× |
| 4 | `stability, and battery life.` | `安定性とバッテリー寿命を考慮してください。` | 67 | 27 | 2.50× |
| 5 | `This p 「 OJect is built on the t 「 ust Of ou 「
p 「 evious supporters
and this new` | `この「OJ」クライアントは、前の支持者の「t」の「ust」を基盤にしています。そしてこの新しい` | 60 | 57 | 1.06× |
| 6 | `ve 「 sion iS ou 「 way Of honoring it.` | `「それは、どのように敬意を示すのか。」` | 34 | 27 | 1.25× |

### english0001.png

| idx | source | translation | rendered px | card px | ratio |
|----:|--------|-------------|------------:|--------:|------:|
| 0 | `The Elicitation Paragraph (Please CaIl StelIa)` | `ステラに電話してください。` | 23 | 19 | 1.25× |
| 1 | `The Speech Accent A 「 chive (SAA)
SAA` | `「chive」は英語で「スライスしたハー」という意味です。` | 58 | 40 | 1.44× |

### english0002.png

| idx | source | translation | rendered px | card px | ratio |
|----:|--------|-------------|------------:|--------:|------:|
| 0 | `The North Wind and the Sun (` | `北風と太陽は、雪を吹き飛ばし、日光を浴びて寒さから身を守る。` | 46 | 18 | 2.50× |
| 1 | `Wikipedia` | `Wikipediaを翻します。` | 38 | 15 | 2.50× |
| 2 | `The North Wind and the Sun were disputing which was the stronger, when a t 「 aveler came along wrapped in a warm cloak. They
agreed that the one wh0 first succeeded in making the t 「 aveler take his cloak 0ff should be conside 「 ed stronge 「 than the 0ther.
Then the North Wind blew as hard as he could, but the more he blew the more closely did the traveler fo
his cloak a 「 ound him;` | `北風と太陽が、どちらが強ければ強いかを争っていたとき、一匹の「avelerが暖かいコートをまとい、彼らに会った。彼は「avelerがコートを取り外すことができた方から強ければ強いと信じる。
次に北風は最大限の力で吹き、しかし、吹くほどコートが近づいてきた。` | 84 | 72 | 1.17× |
| 3 | `and at last the North Wind gave up the attempt. Then the Sun shined out warmly, and immediatelythe travelertook Off his cloak.
And SO the NO 「 th Wind was obliged tO confess that the Sun was the stronger Ofthe two.` | `そして、最後に北風は試みを放棄しました。その代わり、太陽は暖かく輝き、旅行者はすぐに帽子を取り外しました。
そして、北風は「二つめの太陽がどちらも強い」と認めざるを得ませんでした。` | 56 | 44 | 1.28× |

### english0003.png

| idx | source | translation | rendered px | card px | ratio |
|----:|--------|-------------|------------:|--------:|------:|
| 0 | `The Rainbow Passage` | `虹の道` | 23 | 19 | 1.25× |
| 1 | `lnte 「 national Dialects of English Archive (IDEA)` | `日本語で翻すると以下のようになります：

「国家の英語方言アーイブ（IDEA）」` | 59 | 16 | 3.75× |
| 2 | `Comma Gets a Cure` | `カンマが治されました。` | 14 | 11 | 1.25× |
| 4 | `blue.` | `blue. 
青いです。 
青色です。 
青でした。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。 
青いです。` | 3246 | 13 | 256.25× |

### synth_long_paragraph.png

| idx | source | translation | rendered px | card px | ratio |
|----:|--------|-------------|------------:|--------:|------:|
| 0 | `the senses, sight must be the most delightful.
Few things give us greater pleasure than tO be able tO see again
afterwe have been prevented from doing so f0 「 a length oftime.
Yet though sight is 50 precious a possession, comparatively few
people make full use 0f it. They may indeed glance at trees and
animals and clouds, but they
most never observe them.` | `感覚の一つ目は最も楽しいものです。
多くのことは、視覚が再び使用できるかどうかを阻止されたときに
私たちに与えるより大きな喜びを与えてくれます。
しかし、視覚は50貴重な財産でありながら、比較的少ない人々が
それを完全に利用することはできません。彼らは確かに木々や動物や雲をglanceするかもしれませんが、それ以上に視覚を見ることはほとんどありません。` | 144 | 132 | 1.09× |

### synth_mixed_jp_en.png

| idx | source | translation | rendered px | card px | ratio |
|----:|--------|-------------|------------:|--------:|------:|
| 0 | `The Rainbow Passage (` | `色の通り道(` | 21 | 17 | 1.25× |
| 1 | `Wikipedia` | `ウィキペディア` | 27 | 11 | 2.50× |
| 2 | `When the sunlight strikes raindrops in the air, they act` | `日光が空に降った雨滴を照らすとき、彼らは行動します。` | 14 | 11 | 1.25× |
| 3 | `5n1 and
0 「 m a rainbow. The rainbow iS a division
35 a p` | `5n1と
0「りの虹。りが分かれています。」虹は35分の2です。` | 17 | 11 | 1.62× |
| 4 | `ofwhite light into many beautifulcolours.` | `白い光から美しい色々が生まれる。` | 15 | 12 | 1.25× |

### synth_short_lines.png

| idx | source | translation | rendered px | card px | ratio |
|----:|--------|-------------|------------:|--------:|------:|
| 0 | `Save your WO 「 k.` | `あなたのWO「k.」を保存してください。` | 31 | 12 | 2.50× |
| 1 | `Open a new` | `新しいを開く` | 14 | 11 | 1.25× |
| 2 | `05e` | `05e` | 9 | 7 | 1.25× |
| 3 | `Search the document.` | `テキストを索します。` | 12 | 10 | 1.25× |
| 4 | `Show settings panel.` | `設定パネルを表示します。` | 15 | 12 | 1.25× |
| 5 | `Quit the application.` | `アプリを終了してください。` | 30 | 12 | 2.50× |

### synth_small_body.png

| idx | source | translation | rendered px | card px | ratio |
|----:|--------|-------------|------------:|--------:|------:|
| 0 | `0 im` | `0個の物をtranslateyourtextintojapanese.0개의것을번역하세요.0itemsto` | 75 | 7 | 11.25× |
| 1 | `03 d` | `03番です。` | 8 | 7 | 1.25× |
| 2 | `hOfbe( f. W35` | `hOfbe(f.W35-"W35"isacodeoridentifier,likelyrepresentingsomekindofproductoriteminthiscontext.Withoutmoreinformation,it'sdifficulttotranslatethefullphraseaccuratelyintoJapanese.However,Icanprovidearoughtranslation:"W35"(水曜日)-"Wednesday"inEnglish.Withoutcontextabouttheproductoritemrepresentedby"hOfbe",Ican'tprovideamoredetailedtranslation.` | 180 | 8 | 22.50× |
| 3 | `5. was` | `5.は` | 8 | 6 | 1.25× |
| 4 | `0 n ( du` | `0つ(の)` | 8 | 7 | 1.25× |
| 5 | `O n 5.` | `5個あります。` | 9 | 7 | 1.25× |
| 6 | `W35` | `W35を翻します。` | 20 | 5 | 3.75× |
| 7 | `gof .` | `行ってきなさい。` | 22 | 9 | 2.50× |
| 8 | `erofd pair. 、 had` | `エロフドペア。、` | 10 | 8 | 1.25× |
| 9 | `ing` | `ing` | 11 | 9 | 1.25× |
| 10 | `bef` | `前の文を日本語に翻します：

前の文を日本語に翻します：` | 131 | 7 | 18.75× |
| 11 | `gbefO` | `gbefO` | 11 | 9 | 1.25× |
| 12 | `had no` | `何もなかった。` | 17 | 7 | 2.50× |
| 13 | `goingdi (
w 03` | `行く(
w03` | 17 | 9 | 2.00× |
| 14 | `ⅱ t>dw 50 %` | `50パーント` | 9 | 7 | 1.25× |
| 15 | `goi d ⅳ
ep p iOd. (` | `行きましょう。エピ・イドさんへです。(` | 32 | 24 | 1.32× |
| 16 | `i ⅲ 53 d00` | `53日後のi、の数字です。` | 17 | 7 | 2.50× |
| 17 | `its i 0 d. ⅸ g02` | `そのうちの1つです。g02` | 11 | 9 | 1.25× |
| 18 | `ompa ⅱ` | `オマップ` | 10 | 8 | 1.25× |

### synth_title_body.png

| idx | source | translation | rendered px | card px | ratio |
|----:|--------|-------------|------------:|--------:|------:|
| 0 | `The Quick Brown FOX` | `TheQuickBrownFOX
[` | 48 | 19 | 2.50× |
| 1 | `The quick brown fox jumps 0V0` | `速い青いfoxが0V0と跳びます。` | 15 | 12 | 1.25× |
| 2 | `dog. The` | `犬です。The` | 29 | 12 | 2.50× |
| 3 | `boxing wizardsjump quickly. Pack my bOX with
ve dozen` | `ボクサーのウィザードは速く跳びます。私のBOXをパッキングします。
vedozen` | 24 | 12 | 2.00× |
| 4 | `liquorjugs. Sphinxofblackquartz,judge myvow.` | `いSphinxofblackquartzに私の誓願を判断してください。` | 15 | 12 | 1.25× |

### synth_two_columns.png

| idx | source | translation | rendered px | card px | ratio |
|----:|--------|-------------|------------:|--------:|------:|
| 0 | `Windand the Sunwere` | `風と太陽は` | 11 | 9 | 1.25× |
| 1 | `Wind blew 35 hard` | `風が強く吹きつけていました。` | 22 | 9 | 2.50× |
| 2 | `The No` | `TheNo.30isabeautiful,well-keptbuildingwithanimpressivehistory.Ithasbeenthehomeofmanyimportantfiguresinlocalpoliticsandculture.
TheNo.30isabeautiful,well-keptbuildingwithanimpressivehistory.Ithasbeenthehomeofmanyimportantfiguresinlocalpoliticsandculture.` | 271 | 8 | 32.50× |
| 3 | `Then the No` | `それから、No` | 10 | 8 | 1.25× |
| 4 | `disputing which W35 stronger when` | `W35がどのくらい強くか議論するときです。` | 14 | 11 | 1.25× |
| 5 | `he could, butthe more he blew` | `彼は吹くことができたが、もっと吹くと更に困難だった。` | 25 | 10 | 2.50× |
| 6 | `3 traveler came along wrapped in` | `3名の旅行者が包まれてやってきました。` | 14 | 11 | 1.25× |
| 7 | `the more closely did the traveler` | `旅人とその周のものを見つめ、それらがどれだけ近い距離に存在していた。` | 28 | 11 | 2.50× |
| 8 | `the` | `彼女はこれらを赤い袋3つにめてくれるでしょう。水曜日にで待ち合わせましょう。` | 206 | 9 | 23.75× |
| 9 | `his cloak around him; and at` | `彼はcloakにかれており、そしてatの後ろに` | 25 | 10 | 2.50× |
| 10 | `a warm cloak. They` | `暖かいキャップです。彼らは…` | 27 | 11 | 2.50× |
| 12 | `st the North Wind gave up the` | `北風は北風が降らなかった。` | 14 | 11 | 1.25× |
| 13 | `the traveler take his cloak 0` | `旅人はcloakを持って行きました。` | 11 | 9 | 1.25× |
| 14 | `n shined 0` | `0を輝かせます。` | 22 | 9 | 2.50× |
| 15 | `attempt. Then` | `試みる。その後` | 13 | 11 | 1.25× |
| 16 | `shou` | `シュウ` | 11 | 9 | 1.25× |
| 17 | `considered stronger than` | `比較して強く感じられる` | 14 | 11 | 1.25× |
| 18 | `warmly, and the traveter t00k 0` | `暖かく、そして旅人は袋を取った。` | 14 | 11 | 1.25× |
| 19 | `his cloak.` | `彼のcloak。` | 11 | 9 | 1.25× |
| 20 | `the other.` | `それ以外の。` | 11 | 9 | 1.25× |

### synth_ui_buttons.png

| idx | source | translation | rendered px | card px | ratio |
|----:|--------|-------------|------------:|--------:|------:|
| 0 | `File Edit View ⅲ se` | `FileEditViewを翻します。` | 11 | 9 | 1.25× |
| 1 | `Tools Help` | `工具が役立ちます。` | 28 | 11 | 2.50× |
| 2 | `Fo 「 m` | `「メモ」の文字を日本語に翻すると、「メモ」になります。` | 70 | 8 | 8.75× |

## Aggregate (mean across 13 images)

- Residual chars: **0.5** / image
- Preprocess: 0 ms
- WinRT OCR: 0 ms
- ocrs detect: 0 ms
- Inpaint: 5 ms
- Post-OCR residual check: 24 ms
- **Total: 12631 ms** / image

13 processed, 0 errored. Side-by-side PNGs and per-image JSON live next to this file.
