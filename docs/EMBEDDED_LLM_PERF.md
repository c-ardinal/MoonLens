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

| ティア | モデル | サイズ (Q4_K_M) | **llama.cpp 期待値 (現行ランタイム)** | mistral.rs (旧ランタイム参考値) |
|---|---|---|---|---|
| Compact | Qwen 2.5 0.5B | ~400 MB | **50-100 tok/s** | 5-15 tok/s |
| Compact+ | Qwen 3 0.6B | ~460 MB | **45-90 tok/s** | n/a |
| Standard | Qwen 2.5 1.5B | ~1.1 GB | **20-30 tok/s** | 4-10 tok/s |
| Standard- | SmolLM 2 1.7B | ~1.1 GB | **22-35 tok/s** | n/a |
| Standard+ | Qwen 3 1.7B | ~1.2 GB | **18-28 tok/s** | n/a |
| Edge (Gemma) | Gemma 4 e2b | ~1.5 GB | **30-50 tok/s** | n/a |
| Edge+ (Gemma) | Gemma 4 e4b | ~2.5 GB | **15-25 tok/s** | n/a |
| Quality- | Qwen 3 4B | ~2.7 GB | **8-14 tok/s** | n/a |
| Quality | Qwen 2.5 7B (split) | ~4.7 GB | **3-7 tok/s** | 0.5-2 tok/s |

**Gemma nano (e2b/e4b) の補足**: スパース活性化で 1 トークン生成あたりの実効パラメータが小さいので、デコード速度は同じ「総 GB ファイル」のデンスモデルより速いことがあります (e2b は ~1B 相当、e4b は ~2B 相当)。

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

実際のユーザー実機でのベンチ結果を時系列で記録 (期待値の校正に使用):

| 日付 | CPU | RAM | ティア | decode tok/s (mean) | wall tok/s (mean) | コメント |
|---|---|---|---|---|---|---|
| 2026-05-09 | i5-8400 (Coffee Lake, 6c6t, 2017) | 64 GB | Compact 0.5B Q4_K_M | **49** | 11.4 | 期待値レンジ内。短文では wall が大きく低下するのは仕様 (= プロンプト評価が支配的)。 |

i5-8400 のような 5+ 年前の CPU でも decode 49 tok/s は十分実用速度。Lens で 1 行翻訳すると wall ~10 tok/s に見えるが、これは「1 行ごとに context 再構築」しているため。複数行連続翻訳では decode 側に近づく。

## 改訂履歴

- **2026-05-09 (午前)**: 初版。期待値ベースラインと現状のランタイム評価を記載。
- **2026-05-09 (午後)**: ランタイムを mistral.rs → llama-cpp-2 に差し替え。期待値表を「現行 / 旧」併記に変更。
- **2026-05-09 (夜)**: ベンチを decode tok/s と per-line wall-clock tok/s に分離。判定は decode 側で行うように変更 (= 上流 `llama-bench tg` 互換指標)。i5-8400 実機ベースラインを追加。
- **2026-05-10**: Phase 20.7 — `EmbeddedLlmTranslator` にワーカースレッド + context 再利用導入。per-line wall-clock のオーバーヘッド (~700ms/サンプル) を初回 1 回に圧縮。i5-8400 / Compact 0.5B Q4_K_M で wall 10.5 → 14.1 tok/s (+34%)、decode 45.2 → 51.1 tok/s (+13%)。Standard / Quality も全ティア 70%+ 緑判定。
