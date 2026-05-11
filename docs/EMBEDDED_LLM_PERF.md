# Embedded LLM Performance — 期待値とトラブルシュート

このドキュメントは「組み込み LLM が遅すぎる / Lens が timeout する」場面で、

1. **何 tok/s なら正常か**
2. **私のハードウェアでこの数字なら何が原因か**
3. **何を変えれば速くなるか**

を判断するためのリファレンスです。

## 目的の再確認

MoonLens の組み込み LLM は **CPU 単体でも実用速度** で動くことを目標にしています。GPU 加速はあくまで上乗せであり、CPU で動かないなら設計目的の失敗を意味します。

## 期待値ベースライン (CPU のみ・モダンオフィスPC想定)

「モダンオフィスPC」= Intel Core i5/i7 12th-gen 以降 / Ryzen 5 5500U 以降、4-8 物理コア、AVX2 対応、DDR4-3200 以上、SSD ローカル。

サイズは Q4_K_M 量子化での実測値。期待値は llama.cpp ベースの decode tok/s (= `llama-bench tg` 相当)。

| ティア | モデル | サイズ (Q4_K_M) | **decode 期待値** | mistral.rs 参考値 |
|---|---|---:|:---|:---|
| Compact+ | Qwen 3 0.6B (unsloth GGUF) | 379 MB | **45-90 tok/s** | n/a |
| Compact | Qwen 2.5 0.5B | 469 MB | **50-100 tok/s** | 5-15 tok/s |
| Standard+ | Qwen 3 1.7B (unsloth GGUF) | 1007 MB | **18-28 tok/s** | n/a |
| Standard | Qwen 2.5 1.5B | 1066 MB | **20-30 tok/s** | 4-10 tok/s |
| Quality- | Qwen 3 4B Instruct 2507 | 2381 MB | **8-14 tok/s** | n/a |
| Edge (Gemma) | Gemma 4 nano E2B | 2963 MB | **15-22 tok/s** | n/a |
| Quality | Qwen 2.5 7B (split) | 4482 MB | **3-7 tok/s** | 0.5-2 tok/s |
| Edge+ (Gemma) | Gemma 4 nano E4B | 4747 MB | **6-12 tok/s** | n/a |

**Gemma nano (E2B/E4B) の補足**: スパース活性化で 1 トークン生成あたりの実効パラメータが小さい (E2B: effective 2B / total ~5B、E4B: effective 4B / total ~8B)。decode はファイルサイズではなく effective param に近い速度になるため、E2B はサイズ 3GB だが decode 速度は ~1.5B クラスのデンスモデル相当。

**SmolLM 2 1.7B**: 当初候補に含めたが、en→ja 翻訳テストで完全に文字化けする出力 (decode ~4 tok/s でも翻訳結果が「ンンンンにす。」など意味不明) が出たため Phase 21 で TIERS から除外。SmolLM 2 は英語特化トレーニングで日本語データが極端に少なく、本ツールの主要ユースケース (英→日翻訳) に使えない。多言語ファインチューン版が出れば再評価する。

llama.cpp の数値は外部公開ベンチ (`ggml-org/llama.cpp` リポジトリ + ユーザーレポート) からの代表値。mistral.rs 列は Phase 20 の差し替え前に観測した参考値。

> **2026-05-09 ランタイム差し替え済み.** `cargo` の `embedded-llm` feature は既に llama-cpp-2 (FFI to llama.cpp) を使用しています。旧 mistral.rs ランタイムは `embedded-llm-mistralrs` feature の裏に残してあり、A/B 比較用です。

## 設定 → プロバイダ → 「実機ベンチを実行」の使い方

1. 組み込み LLM カードを開く (現在のティアの GGUF が DL 済みであること)
2. 「実機ベンチを実行 (5サンプル翻訳)」ボタンを押す
3. 5 つの代表的な英→日翻訳を直列実行 (1 サンプル ~10〜30 秒、合計 1〜3 分目安)
4. **生成速度 (decode)** と **per-line wall-clock** の両方が表示される

## 2 種類の tok/s メトリクス (重要)

ベンチは 2 つの異なる速度指標を分けて表示します:

| メトリクス | 計算式 | 意味 | 比較対象 |
|---|---|---|---|
| **decode tok/s** | `(output_tokens - 1) / (last_token_at - first_token_at)` | 純粋な生成スループット (プロンプト評価込みではない) | 上流 `llama-bench tg` の値 / 期待値ベースライン |
| **per-line wall-clock tok/s** | `output_tokens / total_ms` | ライン単体の実効スループット (context 構築 + プロンプト評価込み) | Lens で 1 行翻訳した時の体感速度 |

短文翻訳の場合、context 構築 + プロンプト評価が **wall-clock の 70%以上**を占めることがあります。例: 4 トークン生成で wall-clock 800ms のうち、context 設定 70ms + プロンプト評価 (118 トークン) 600ms + 純生成 90ms = wall 5 tok/s だが decode は 44 tok/s — 数値が 9 倍違う。

「期待値の何%」の判定は **decode** 側で行います (`llama-bench tg` と同じ意味)。Lens の体感速度を改善したい場合は per-line wall-clock の数値を見てください — 短文では context overhead が支配的なため、複数行を連続翻訳すれば実効スループットは decode 側に近づきます。

## 結果の読み方

| ベンチ結果 | 意味 | 取るべき対応 |
|---|---|---|
| 期待値の **70% 以上** | 想定通り動作。タイムアウト原因は別 (大量 lines / paragraph translation 等) | ティアを下げるか、翻訳の同時実行を減らす (max_concurrent_tasks=1) |
| 期待値の **30〜70%** | やや遅いがティア相応。古めのCPU・background load等 | より小さいティアを推奨 / 不要な常駐ソフト停止 |
| 期待値の **30% 未満** | llama.cpp が想定通り動いていない (古い CPU / SSE2 のみ / 強い background load 等) | CPU 仕様を確認, 同時実行プロセスを停止, それでも改善しなければ低位ティアへ |

## ランタイム選択の経緯と現状

### 現行: llama-cpp-2 (FFI to llama.cpp) — 2026-05-09 〜

**採用理由:**
- mistral.rs 時代のベンチで CPU 推論が期待値の 30% 未満と確認 (本プロジェクト実機 + 上流 issue 一致)
- llama.cpp は 5+ 年 SIMD を詰めてきた成熟スタック (AVX2 / AVX-512 / Neon / Q4_K_M 最適化)
- 2〜5 倍速くなる見込みでユーザー目的 (CPU で実用) を満たせる

**長所:**
- CPU 速度は llama.cpp 由来 → 上記期待値に近い数字
- KV キャッシュ・prefix cache・量子化 SIMD が成熟
- ChatML をプロジェクト側で組み立てているため GGUF メタデータ依存が小さい

**短所 (運用上注意):**
- ビルドに C++ toolchain 必須 (Windows: Visual Studio 2022 Build Tools + LLVM/libclang + CMake、Linux: gcc/clang + cmake、macOS: Xcode CLI Tools)
- クロスコンパイルは llama.cpp 側の cmake 依存に縛られる
- ただし **エンドユーザーは C++ ツールチェーン不要** — 配布バイナリは静的リンク済み

### A/B 比較用: mistral.rs (Pure Rust) — `embedded-llm-mistralrs` feature

**残しておく理由:**
- ベンチ比較で再現性を確保するため (`cargo build --no-default-features --features embedded-llm-mistralrs` で旧バージョンが再生成可能)
- Pure Rust ビルドが必要な特殊環境向けのフォールバック (ただし速度は妥協する)

**短所:**
- CPU 推論は llama.cpp の **2〜5 倍遅い**
- 7B モデルは CPU では実質非実用 (1 tok/s 前後)

## "use_gpu_if_available = true" について (重要)

`default.toml` の `[performance]` セクションに `use_gpu_if_available = true` がありますが、**現状この設定は何もしません** (dead config)。`Cargo.toml` の `mistralrs = { default-features = false }` で CUDA / Metal feature が OFF にされているため、コードが GPU パスに到達しません。

修正の方向としては:
1. **削除** — 機能していない設定を露出する方が混乱を招くので、UI/schema から外す
2. **実装** — feature flag を追加し、ビルド時に CUDA/Metal/Vulkan を選べるようにする

ランタイム差し替えと同タイミングで一緒に整理する予定です。

## ベンチ結果の報告先

ベンチで「期待値 30% 未満」が出た場合、以下を含めて issue / 開発者へ報告いただけると判断材料になります:

- ベンチ結果の screenshot (p50/p95/mean tok/s)
- CPU の正式名 (Windows: `wmic cpu get name`, Linux: `cat /proc/cpuinfo`, macOS: `sysctl -n machdep.cpu.brand_string`)
- 物理コア数 / 論理コア数
- RAM 容量
- 同時実行している重いプロセス (Chrome タブ多数 / VS Code / Docker 等)

## 実機ベースライン記録

実際のユーザー実機でのベンチ結果を時系列で記録 (期待値の校正に使用)。

### i5-8400 (Coffee Lake, 6c6t, 2017) / 64 GB DDR4 — 2026-05-11 (Phase 21 v3)

`cargo run --example bench_all_tiers --release --features embedded-llm -- --samples 5` で全ティアを連続実行した結果。

| ティア | テンプレ | decode mean | wall mean | 翻訳品質 |
|---|---|---:|---:|---|
| Compact (Qwen 2.5 0.5B) | ChatML | **53.9** | 13.6 | ◯ (時々中国語漏れ) |
| Compact+ (Qwen 3 0.6B) | ChatML+no_think | **48.8** | 16.8 | △ (短文/truncated 多い) |
| Standard (Qwen 2.5 1.5B) | ChatML | **25.0** | 7.9 | ◯ |
| Standard+ (Qwen 3 1.7B) | ChatML+no_think | **21.4** | 6.8 | ◯ |
| Edge (Gemma 4 E2B) | Gemma | **16.8** | 6.9 | △ (一部英語エコー) |
| Quality- (Qwen 3 4B Instruct 2507) | ChatML | **9.8** | 3.1 | ◎ |
| Edge+ (Gemma 4 E4B) | Gemma | **8.7** | 1.9 | ◎ (最高品質クラス) |
| Quality (Qwen 2.5 7B) | ChatML | **5.9** | 1.8 | ◎ |

**観察**:
- Compact 帯 (~400MB): Qwen 2.5 0.5B が速度・品質のバランスで最良
- Standard 帯 (~1GB): Qwen 2.5 1.5B が最速、Qwen 3 1.7B は同等品質でやや遅い
- 中量級 (~2.5GB): **Qwen 3 4B Instruct 2507 を新ティアとして追加** — 7B より 1.7倍速く、品質も高い
- 高品質帯: **Gemma 4 E4B が驚きの結果** — 5GB ファイルだが sparse activation のおかげで 7B (4.5GB) より decode 47% 速い + 翻訳品質も同等以上

**Phase 21 v3 で適用した修正**:
- Qwen 3 系 (0.6B / 1.7B) は thinking mode が暴走するため、ChatML に no_think prime (`<think>\n\n</think>\n\n`) を追加
- Qwen 3 4B Instruct 2507 は "instruct-2507" 系列で thinking が weights レベルで除去済 → 通常 ChatML を使用 (no_think prime を入れると EOS を即時出力する)
- SmolLM 2 1.7B は en→ja で完全文字化けのため除外

## 改訂履歴

- **2026-05-09 (午前)**: 初版。期待値ベースラインと現状のランタイム評価を記載。
- **2026-05-09 (午後)**: ランタイムを mistral.rs → llama-cpp-2 に差し替え。期待値表を「現行 / 旧」併記に変更。
- **2026-05-09 (夜)**: ベンチを decode tok/s と per-line wall-clock tok/s に分離。判定は decode 側で行うように変更 (= 上流 `llama-bench tg` 互換指標)。i5-8400 実機ベースラインを追加。
- **2026-05-10**: Phase 20.7 — `EmbeddedLlmTranslator` にワーカースレッド + context 再利用導入。per-line wall-clock のオーバーヘッド (~700ms/サンプル) を初回 1 回に圧縮。i5-8400 / Compact 0.5B Q4_K_M で wall 10.5 → 14.1 tok/s (+34%)、decode 45.2 → 51.1 tok/s (+13%)。Standard / Quality も全ティア 70%+ 緑判定。
- **2026-05-11**: Phase 22 — Gemma 4 MTP drafter による speculative decoding を実装。drafter ロード → spec decode loop → KV ロールバック → bonus token 全フロー完成。但し**実行ブロック**: drafter GGUF は新 architecture `gemma4_mtp` を使用しており、現行 llama.cpp 未対応 (upstream PR #22738 クローズ済、マージ未定)。drafter ロード失敗時は graceful fallback で従来の autoregressive 経路を使用するため翻訳機能には影響なし。upstream 対応後は llama-cpp-2 のバージョン bump のみで spec decode が自動有効化される。
- **2026-05-11 (Phase 23)**: ナレッジカットオフ後リリースの調査で 2 ティアを採用 + 4 を除外。**llm-jp 3.1 1.8B** (日本政府系) が i5-8400 で decode 21.6 / wall 11.2 tok/s を達成。Compact+ の per-line 速度を 85% 維持しつつ Qwen 2.5 7B 級の JA 翻訳品質を実現 → 「小型・速度・高品質」目標の現実的最良解。**Qwen 3.5 4B** も採用 (Quality--、Qwen 2.5 7B より 60% 高速で同等品質)。Qwen 3.5 0.8B / 2B / RakutenAI 2.0 mini / EuroLLM 1.7B は JA 翻訳品質ムラまたは破綻のため除外。
