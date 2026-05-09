# MoonLens

> PC 版 Google Lens。1 つのグローバルホットキーで、画面上の外国語テキストを
> **その場で** OCR + 翻訳し続け、QR・画像検索・ライブ字幕まで 1 箇所にまとめる
> プライバシーファーストの常駐ツール。

> **English summary**: MoonLens is a privacy-first, keyboard-driven desktop
> companion that streams the contents of any screen region through OCR +
> local LLM translation, overlaying the result in place. Beyond OCR it
> also handles QR codes, reverse image search, and loopback-audio live
> subtitles. Truly standalone: ships with an embedded LLM runtime
> (`embedded.llm`, currently driving Qwen 2.5 GGUFs) so translation
> works without Ollama or any external daemon. Cloud APIs (DeepL /
> Gemini) and a local Ollama daemon are supported as opt-in fallbacks.

- 📷 **Lens ストリーミング**: Lens ウィンドウを対象領域に重ねると、
  継続的に OCR + 翻訳を in-place で描画し続ける
- ⚡ **ワンショット領域キャプチャ**: `Ctrl+Alt+Space` → ドラッグ選択 →
  オーバーレイで結果表示（QR / テキスト / 写真を自動分岐）
- 🔒 **スタンドアロン (ローカル LLM 同梱)**: 既定では `embedded.llm`
  プロバイダがアプリ内で Qwen 2.5 (Apache 2.0) の GGUF を読み込んで翻訳。
  Ollama などの外部サーバを必要としない。クラウドは明示的な許可時のみ
  DeepL / Gemini にフォールバック
- 🎙 **ライブ字幕**: システム音声ループバック → whisper.cpp → 翻訳 → 半透明字幕
- 🔎 **リバース画像検索**: Google Images / Bing / Yandex をブラウザで、
  SerpAPI キー設定時はアプリ内で結果表示
- 🧩 **拡張機構**: WASM Component と JSON-RPC サイドカーの 2 形態、
  Ed25519 でマニフェスト署名検証

## プロジェクトステータス

| 項目 | 状態 |
|---|---|
| 設計仕様 | [`docs/plan.md`](docs/plan.md) に集約 |
| MVP (S0.5) | ✅ 完了（2026-04-21、ユーザ承認済） |
| S1 — window chrome + sidebar + schema form 改修 | ✅ 完了 |
| S2 — DXGI + WDA + Windows OCR 差替 | 予定 |
| v0.1.0 stable | S5 での golden path 6 シナリオ全 pass 後に tag |

現在のブランチは `claude/moonbit-lens-tool-2FkOu`。release ladder は
`v0.1.0-beta.1 → beta.2 → beta.3 → rc.1 → v0.1.0` の 5 タグ段階。
詳細は [`docs/plan.md`](docs/plan.md) の "Release Process" を参照。

## Platform matrix

| OS | v0.1 対応 | 備考 |
|---|---|---|
| Windows 11 | **Full support** | DXGI + `WDA_EXCLUDEFROMCAPTURE` で Lens 完全対応（S2） |
| Linux | Degraded | `xcap` polling ベース、Lens のブリンク残存。残り機能は動作 |
| macOS | v0.2 以降 | Apple Developer + notarization 要、v0.1 スコープ外 |

## スタック

- **アプリシェル**: Tauri 2.x (Rust stable 1.80+)
- **UI**: Svelte 5 (runes) + Vite + Tailwind CSS v4、Inter + JetBrains Mono
- **OCR**: [`ocrs`](https://github.com/robertknight/ocrs)（S2 で Windows OCR API への差替検討）
- **QR**: `rqrr`
- **翻訳 (embedded local)**: [`mistral.rs`](https://github.com/EricLBuehler/mistral.rs)
  経由で Qwen 2.5 の GGUF をアプリ内ロード。既定モデル
  `qwen2.5-1.5b-instruct-q4_k_m.gguf` (~1.1GB, **Apache 2.0**)。
  `embedded-llm` cargo feature を有効化したビルドで利用可能、
  初回起動時に HuggingFace から `<app-data>/moonlens/models/` へ自動DL。
  制限環境向けに `.moonlens-model` パッケージファイルとしてエクスポート/
  インポートも可能 (1人がDL → ファイルサーバ経由で社内展開)
- **翻訳 (Ollama 連携、オプション)**: Ollama HTTP ストリーミング、ユーザーが
  別途 `ollama serve` を起動した場合のみ
- **翻訳 (cloud, オプション)**: DeepL / Gemini API（オプションフォールバック）
- **ASR**: `whisper.cpp` サイドカー
- **画面キャプチャ**: `xcap` 0.9（S2 で DXGI Desktop Duplication 併用へ）
- **音声ループバック**: `cpal`
- **履歴**: `rusqlite` (bundled SQLite)
- **プラグイン**: WebAssembly Component Model (`wasmtime`) + JSON-RPC サイドカー
- **プラグイン署名**: Ed25519 (`ed25519-dalek`) + SHA-256

## ドキュメント

| ファイル | 用途 |
|---|---|
| [`docs/plan.md`](docs/plan.md) | 全ての設計判断・セッション引継ぎ protocol・release ladder |
| [`docs/USAGE.md`](docs/USAGE.md) | エンドユーザ向け操作ガイド |
| [`docs/DEV.md`](docs/DEV.md) | 開発者向けセットアップ・ビルド・テスト |
| [`docs/KNOWN_ISSUES.md`](docs/KNOWN_ISSUES.md) | v0.1 の既知制約一覧 |
| [`docs/RELEASING.md`](docs/RELEASING.md) | リリースワークフロー |
| [`docs/benchmarks/`](docs/benchmarks/) | パフォーマンス計測結果の履歴 |
| [`CHANGELOG.md`](CHANGELOG.md) | 変更履歴（Keep a Changelog） |
| [`plugin-sdk/docs/README.md`](plugin-sdk/docs/README.md) | プラグイン開発ガイド |

## クイックスタート

### 前提

- Rust stable 1.80+
- Node.js 20+

**翻訳エンジンは外部依存なしで動きます。** 初回起動時に「ローカル翻訳モデル
を準備します」モーダルが表示され、`Qwen 2.5 1.5B-Instruct Q4_K_M`
(~1.1GB, **Apache 2.0**) を HuggingFace から `<app-data>/moonlens/models/`
へダウンロード。以降は完全オフラインで翻訳できます。

**インターネット制限環境での運用** — 1人がオフラインで `.moonlens-model`
パッケージファイルにエクスポートし、社内ファイルサーバ経由で他のPCへ
配布する運用が可能です。受け取り側は設定 → プロバイダ → 「パッケージから
取込」ボタン (sha256 自動検証) で完了。

オプションで Ollama を使いたい場合のみ:

```bash
ollama pull gemma4:e2b
ollama serve
```

その上で 設定 → プロバイダ → 翻訳 のチェーンに `ollama.gemma4` を追加。

字幕機能を使う場合は `whisper.cpp` サーバを `http://127.0.0.1:9002`
で起動しておく（[`docs/USAGE.md`](docs/USAGE.md) の字幕セクション参照）。

### 開発モードのビルド要件 (組み込み LLM)

`embedded-llm` feature (= 既定) は `llama-cpp-2` 経由で llama.cpp を
**ソースコンパイル**します。以下が **開発者の PC** に必要です
(エンドユーザーの配布バイナリには不要):

| OS | 必要なもの |
|---|---|
| Windows | Visual Studio 2022 Build Tools (C++ workload) + LLVM/libclang (`winget install LLVM.LLVM`) + CMake (Build Tools 同梱で OK) |
| Linux | gcc/clang + cmake + libclang-dev |
| macOS | Xcode CLI Tools (`xcode-select --install`) |

Windows では `LIBCLANG_PATH=C:\Program Files\LLVM\bin` を環境変数に
通し、CMake を PATH に通した状態で `cargo build` を起動してください。

```bash
npm install
# 既定 features に embedded-llm が含まれているため通常起動で翻訳が動きます。
# 初回ビルドのみ llama.cpp のコンパイルに数分〜十数分かかります。
npm run tauri:dev
```

### リリースビルド

```bash
# 通常はこれだけで OK (embedded-llm 含む)。
npm run tauri:build
# CI 等で C++ ビルドを skip して type-only check をしたい場合:
#   cargo check --no-default-features
# 旧 mistral.rs ランタイムで A/B 比較ビルドする場合 (非推奨):
#   cargo build --no-default-features --features audio,ocrs-engine,embedded-llm-mistralrs
```

Windows 自己署名は v0.2 以降。現状 Win 初回起動時は SmartScreen 警告が出る
（回避手順は [`docs/USAGE.md`](docs/USAGE.md) 参照）。

## ディレクトリ

```
MoonLens/
├─ src-tauri/                # Rust バックエンド
│  └─ src/
│     ├─ settings/           # 階層型設定、schemars による JSON Schema
│     ├─ providers/          # OCR / Translator / ASR / QR / Search 実装
│     ├─ plugins/            # WASM + サイドカーホスト、WIT 定義
│     ├─ pipeline.rs         # チェーン実行、フォールバック、stickiness
│     ├─ lens_watcher.rs     # Lens streaming 差分検出エンジン
│     └─ lens.rs             # Lens ウィンドウのライフサイクル
├─ src/                      # Svelte 5 + SvelteKit フロント
│  ├─ routes/
│  │  ├─ +layout.svelte      # 常駐シェル (WindowChrome + AppSidebar)
│  │  ├─ +page.svelte        # ホーム（ダッシュボード）
│  │  ├─ settings/           # 自動生成フォーム (auto-save)
│  │  ├─ plugins/  history/  subtitle/
│  │  ├─ lens/               # Lens result overlay
│  │  ├─ overlay/            # one-shot 結果
│  │  └─ select/             # 領域選択
│  └─ lib/
│     ├─ components/         # WindowChrome, AppSidebar, SchemaForm, HotkeyInput, ...
│     ├─ design/tokens.css   # CSS custom properties
│     └─ ipc.ts              # 型付き Tauri invoke / listen ヘルパ
├─ plugin-sdk/               # プラグイン開発キット (WIT + Rust + 例)
├─ docs/                     # 設計・運用ドキュメント
└─ .github/workflows/        # CI (Linux 中心)
```

## カスタマイズ思想

MoonLens はハードコードされた挙動を極力持たない。UI / 挙動 / プロバイダに
関わる全定数は `$XDG_CONFIG_HOME/moonlens/config.toml`（およびプロファイル）
から制御可能で、カテゴリごとの戦略で即時反映 or 次サイクル反映される
([`docs/plan.md`](docs/plan.md) § Hot-reload strategy)。設定画面は
`schemars` が吐く JSON Schema から自動生成されるため、Rust 側で新しい
provider や plugin を追加してもフォームの手書き更新は不要。

### プロバイダチェーン

タスク（`translate` / `ocr` / `asr` / `explain` / `summarize` / …）ごとに
独立した順序付きチェーンとフォールバック条件を定義する。**既定**は
組み込み LLM のみで完結し、外部サーバ不要:

```toml
[providers.translate]
chain = ["embedded.llm"]
parallel_race = false
fallback_on = ["timeout", "error", "offline"]
timeout_ms = 30000
allow_cloud = false
allow_network = false
sticky_session_ms = 30000

[providers.configs."embedded.llm"]
# `model_path` を省略するとアプリの app-data ディレクトリ配下に
# 自動展開される。HuggingFace 上のリポジトリ + ファイル名は
# `repo` / `filename_remote` で上書き可能。
repo = "Qwen/Qwen2.5-1.5B-Instruct-GGUF"
filename_remote = "qwen2.5-1.5b-instruct-q4_k_m.gguf"
license = "Apache-2.0"
stream = true
temperature = 0.0
num_ctx = 4096
num_predict = 2048
```

オプションで Ollama を併用する場合の例:

```toml
[providers.translate]
chain = ["embedded.llm", "ollama.gemma4", "deepl"]
allow_cloud = true
allow_network = true

[providers.configs."ollama.gemma4"]
endpoint = "http://127.0.0.1:11434"
model = "gemma4:e2b"
think = false
num_ctx = 8192
stream = true
```

プロバイダごとの詳細は `[providers.configs."<id>"]` に任意キーで書ける。
知らないキーはそのまま保存されるため、プラグインは独自スキーマで
`[providers.configs."plugin.my_translator"]` を参照できる。

### プラグイン拡張ポイント (v0.1)

`translator` / `ocr` / `asr` / `explain` / `summarize` /
`reverse_image_search` / `qr_handler` / `intent_classifier` /
`action` / `pre_processor` / `post_processor` / `overlay_renderer` /
`hotkey_handler` / `history_indexer`

v0.2 以降追加予定:
`frame_stream_processor` / `change_detector` / `lens_overlay_renderer` /
`translate_stream`。

## ライセンス

Apache License 2.0
