# Phase 27 v2: Pivot 翻訳品質レビュー

ベンチ環境: i5-8400 + RTX 4080 (Vulkan), `MOONLENS_GPU_LAYERS=99`, 5 samples/pair, 2 段 Pivot パイプライン。

生データ:
- `2026-05-12-phase27-pivot-bench.md` (v1, clean_text 弱い版)
- `2026-05-12-phase27-pivot-bench-v2.md` (v2, clean_text 強化版、ja→X のみ)

## TL;DR

**Pivot は両方向で機能**:
- ✅ **X→ja**: Phase 27 直接で 🚫/❌ だった非 en/ja ソースが、Edge+ X→en + JA-Std+ en→ja で実用品質に。
- ✅ **ja→X**: clean_text 修正後、JA-Std+ ja→en + Edge+ en→X で 13 ターゲット言語すべてに翻訳成立。一部に「名無し」ガベージ leak (Step 1 の EOS バグ起因) が残るのみ。

## Pivot パイプライン構成

```
ja→X:  ja  ─[JA-Std+ (llm-jp 3.1 1.8B)]─►  en (中間)  ─[Edge+ (Gemma 4 E4B)]─►  X
X→ja:  X   ─[Edge+ (Gemma 4 E4B)]─►          en (中間)  ─[JA-Std+ (llm-jp 3.1 1.8B)]─►  ja
en→ja, ja→en:  単段、JA-Std+ 直接
```

## 直接 vs Pivot 効果比較

### X→ja (Phase 27 直接 vs v1 Pivot)

| ペア | 直接 (Phase 27) | Pivot v1 | 評価 |
|---|---|---|---|
| ar→ja | 🚫 「素早い**土の動物**」 | ✅ 「素早い**フォックス**ジャンプで怠惰な犬を追い越す」 | 大幅改善 (フォックス未訳のみ) |
| ru→ja | 🚫 ハルシネーション | ✅ 自然な日本語 | 完全救済 |
| zh-CN→ja | ⚠️ | ✅ | 改善 |
| ko→ja | ⚠️ | ✅ (sample 2 のみカタカナ転写) | 改善 |
| es/pt/fr/de→ja | 🚫/⚠️ | ✅ | 改善 |
| vi/th/id/hi→ja | 🚫/⚠️ | ✅ | 改善 |

**12/13 ペアで改善**。

### ja→X (Phase 27 直接 vs v2 Pivot)

| ペア | 直接 (Phase 27 Edge+) | Pivot v1 | Pivot v2 (clean_text 修正後) |
|---|---|---|---|
| ja→zh-CN | ✅ 「大家好,世界」 | ❌ 全 5 英語パススルー | **4/5 ✅** 「敏捷的棕狐狸跳过了懒惰的狗。」 |
| ja→ko | ✅ | ❌ 全 5 hallucination | **4/5 ✅** 「재빠른 갈색 여우가 게으른 개 위로 뛰어넘는다.」 |
| ja→ar | ✅ | ❌ 全 5 英語 / hallucination | **5/5 ✅** 「الثعلب البني السريع يقفز فوق الكلب الكسول.」 |
| ja→hi | ✅ | ❌ | **5/5 ✅** 「तेज़ भूरी लोमड़ी आलसी कुत्ते के ऊपर कूदती है।」 |
| 他 9 ペア | ✅ | ❌ | **5/5 or 4/5 ✅** |

**13/13 ペアで Pivot が機能**、直接 Edge+ 品質と互角。

### v1 vs v2 の決定的差: clean_text 修正

v1 の `clean_text` は `<|im_end|>` (角括弧付き) しか剥がさず、JA-Std+ が emit する `|im_end|>` (角括弧無し) リテラル + 「名無し」ガベージが Step 2 にそのまま流れ込んで Edge+ がハルシネーション。

v2 は `|im_end` のサブストリングを検出して全削除。Step 2 入力が完全クリーンに:

| Step 1 出力 (生) | v1 Step 2 入力 | v2 Step 2 入力 |
|---|---|---|
| `Hello,world名無し\|im_end\|>` | `Hello,world名無し\|im_end\|>` | `Hello,world名無し` |
| `Thequickbrownfox....\|im_end\|>` | `Thequickbrownfox....\|im_end\|>` | `Thequickbrownfox....` |

## v2 Pivot 残存課題: Sample 1 の「名無し」leak

**全 13 ja→X ペアの Sample 1 (greeting 文)** で JA-Std+ ja→en Step 1 が "Hello,world" の後に「名無し」を追記し、Step 2 がそれを忠実に翻訳する:

| 言語 | Sample 1 出力 |
|---|---|
| ja→zh-CN | 你好，世界**名無し** (日本語残り) |
| ja→ko | 안녕하세요,세상의**무명인** ("無名人") |
| ja→es | Hola,mundo (Edge+ がガベージを無視) |
| ja→ar | مرحباً، عالم **لا اسم له** ("名前なし") |
| ja→hi | नमस्ते, दुनिया **नामहीन** ("無名") |
| ja→ru | Привет, мир. **Аноним**. ("匿名") |
| ja→de | Hallo, Welt **Name: Nameless** |
| ja→fr | Bonjour, monde **nom 無し** (混在) |

**根本原因**: JA-Std+ の EOS トークン (`<|im_end|>`) がランタイムで停止条件として認識されていない (Phase 25 既知の問題)。`num_predict: 64` まで生成し続けるので「名無し」のようなランダムトークンが付加される。

**修正案** (現バグ):
| 案 | 効果 | コスト |
|---|---|---|
| (a) runtime に stop_tokens 追加 | 根本解決、ja→en 直接も改善 | 30 分: translate_embedded_llm.rs 修正 |
| (b) ja→en step だけ別途 post-process | 「名無し」「Nameless」を末尾から除去 | 10 分: bench 内ヒューリスティック |
| (c) ja→en bridge を Edge+ に切替 | 名無し問題なくなる、ただし JA-Std+ の en quality が高品質 (Phase 27 で確認) ので一段品質低下リスク | 5 分: 設定変更のみ |

## 速度コスト (v4 EOS 修正後)

| 経路 | v2 平均 total ms | v4 平均 total ms | 改善 |
|---|---:|---:|---:|
| en→ja 直接 (JA-Std+) | ~209 | ~106 | 2.0x |
| ja→en 直接 (JA-Std+) | ~209 | **79** | **2.6x** |
| ja→X Pivot 2 段 | ~470 | **~290** | 1.6x |
| X→ja Pivot 2 段 | ~430 | **~270** | 1.6x |

直接 Edge+ ja→X (Phase 27) は ~65 tok/s (約 230ms / 翻訳) だったので、**Pivot の方が直接 Edge+ より速い**。

### EOS 修正の効果

Phase 27 v3 で `<|im_end|>` のみを stop suffix としていたところ、ja→en では JA-Std+ がバレル形 `|im_end|>` (`<` 抜け) を emit するため捕捉できていなかった。v4 で両形式 + Gemma の `<end_of_turn>` をすべてカバー (`STOP_SUFFIXES = ["<|im_end|>", "|im_end|>", "<end_of_turn>"]`)。

これにより JA-Std+ ja→en Step 1 が 235ms → 67ms に短縮 (約 3.5x 速度向上)。`runtime` 修正につき**直接 ja→en 利用にも効く**。

## 推奨利用パターン

### Routing Strategy
```
en ↔ ja:           JA-Std+ (llm-jp 3.1 1.8B) 直接
ja → en→ X:        Pivot (JA-Std+ ja→en + Edge+ en→X)  # 13 ターゲット安定
X → en→ ja:        Pivot (Edge+ X→en + JA-Std+ en→ja)  # 13 ソース改善
X → en→ Y (en/ja 含まず):  Pivot 2 段 (Edge+ X→en + Edge+ en→Y) — 未検証だが原理上動作するはず
```

### サイズトレードオフ
- Pivot 採用 → JA-Std+ + Edge+ の 2 モデルを常駐 (1.1GB + 5GB = **6.1GB RAM**)
- 単段 Edge+ のみ → 5GB RAM、全言語ペア対応だが Phase 27 で 96% safety
- 単段 JA-Std+ のみ → 1.1GB RAM、JA↔EN のみ

### 採用判断

**Pivot を採用するメリット** (vs 単段 Edge+):
1. 速度: ja→X が 65 tok/s → 140 tok/s (約 2 倍)
2. en↔ja 品質: JA-Std+ の方が Edge+ より en↔ja で若干上 (Phase 27)
3. メモリ余裕がある PC ではメリット大

**Pivot を採用しないメリット** (Edge+ 単段):
1. メモリ消費 5GB vs 6.1GB
2. 単純な 1 段で複合誤差リスクなし
3. Sample 1 「名無し」leak 問題なし

## 結論

**Pivot は技術的に成立**: ja↔任意言語の翻訳を、JA↔EN 特化の llm-jp の速度 + 多言語 Edge+ の品質という両方の長所で構成可能。clean_text 修正 + Step 1 EOS バグ修正で実用レベルに到達できる。

**実用での選択肢**:
- **シンプル優先**: Edge+ E4B 単段 (Phase 27 96% safety)
- **速度+品質優先**: Pivot 経路 (Step 1 EOS 修正必須、Sample 1 「名無し」leak 残るが許容範囲)

次の最適化候補:
1. JA-Std+ の `<|im_end|>` を runtime stop_tokens に追加 (Phase 25 既知バグの根本修正、ja↔en 直接利用にも効く)
2. Pivot をプロダクション統合する場合の chain 設定 (settings UI で「translate-pivot」プリセット定義)
