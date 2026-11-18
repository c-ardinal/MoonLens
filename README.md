# MoonLens

PC常駐の Google Lens 風ツール。1つのグローバルホットキーで

- 画面領域の OCR + ローカル LLM 翻訳 + オーバーレイ表示
- QR コード認識
- 外国語動画のループバック音声からライブ字幕
- ローカル LLM による要約・説明・対話

を統合する。翻訳・要約・説明はアプリに内蔵した llama.cpp エンジンで端末内だけで
完結し（Ollama などの外部ランタイムは不要）、クラウドは明示的に有効化した場合のみ使う。音楽認識は運用コストの観点から本体には含めず、
プラグインとして実装できる余地を確保している。

## スタック

- **アプリシェル**: Tauri 2.x (Rust)
- **UI**: Svelte 5 (runes) + Vite + Tailwind CSS v4
- **OCR**: [`ocrs`](https://github.com/robertknight/ocrs) crate（追加の OCR エンジンはプラグインで差し替え可）
- **QR**: `rqrr` crate
- **翻訳 (local)**: 内蔵 llama.cpp（`llama-cpp-2`、静的リンク）で GGUF を実行。GPU は Windows / Linux が Vulkan（NVIDIA・AMD・Intel）、macOS が Metal。GPU がない環境では CPU で動く。翻訳の既定は PLaMo 3 Translate 2B（0.8B も選択可）、説明・要約は Gemma 4 E4B。モデルはアプリのホーム画面からダウンロード（SHA-256 検証・中断再開）。モデル選定の根拠は [docs/translation-models.md](docs/translation-models.md)
- **翻訳 (cloud / Ollama)**: DeepL / Gemini API、Ollama は任意のオプション（既定チェーンには入っていない）
- **ASR**: `whisper.cpp` HTTP サーバ（別途起動が必要。未接続時はライブ字幕の開始時に案内）
- **画面キャプチャ**: `xcap` 0.9、**音声ループバック**: `cpal`
- **履歴**: `rusqlite`（SQLite、bundled）
- **プラグイン**: WebAssembly Component Model (`wasmtime`) と JSON-RPC サイドカー
- **プラグイン署名**: Ed25519 (`ed25519-dalek`) + SHA-256

## ディレクトリ

```
MoonLens/
├─ src-tauri/          # Rust バックエンド（Tauri）
│  └─ src/
│     ├─ settings/     # 階層型設定、schemars による JSON Schema 自動生成
│     ├─ providers/    # OCR / Translator / ASR / QR / Search 実装
│     ├─ plugins/      # WASM + sidecar ホスト、WIT 定義
│     ├─ pipeline.rs   # チェーン実行、フォールバック、stickiness
│     └─ ...
├─ src/                # Svelte 5 + SvelteKit フロント
│  ├─ routes/          # overlay / subtitle / settings / plugins / history
│  └─ lib/
│     ├─ design/       # デザイントークン、モーション
│     └─ components/   # SchemaForm, CommandPalette, PrivacyBadge, ActionRibbon
├─ plugin-sdk/         # プラグイン開発キット（WIT + Rust + 例）
└─ .github/workflows/  # Win/Mac/Linux 3系列ビルド
```

## カスタマイズ思想

MoonLens はハードコードされた挙動を極力持たない。UI/挙動/プロバイダに関わる
すべての定数は `config.toml`（Linux: `~/.config/io.moonlens.app/`、macOS: `~/Library/Application Support/io.moonlens.app/`、Windows: `%APPDATA%\io.moonlens.app\`）
から制御可能で、変更は保存で即時反映される。`config.toml` には既定値と異なる値だけが
保存され、既定値の一覧は同じフォルダーの `config.default.toml`（参照用）にある。
各段階（導入〜削除）の挙動は [docs/lifecycle.md](docs/lifecycle.md) を参照。

### プロバイダチェーン

タスク（`translate` / `ocr` / `asr` / `explain` / `summarize` / ...）ごとに
独立した順序付きチェーンとフォールバック条件を定義する：

```toml
[providers.translate]
chain = ["llamacpp", "plugin.my_translator"]
parallel_race = false
fallback_on = ["timeout", "error", "offline"]
timeout_ms = 15000
allow_cloud = false
allow_network = false
sticky_session_ms = 30000
```

プロバイダごとの設定は `[providers.configs."<id>"]` に任意キーで書ける。
知らないキーはそのまま保存されるため、プラグインは独自スキーマで
`[providers.configs."plugin.my_translator"]` を参照できる。

## プラグイン

### 2 形態

1. **WebAssembly Component プラグイン**: `wasmtime` サンドボックス内で動作。
   任意の言語（Rust / Go / TypeScript / Python / MoonBit...）で WIT
   インターフェース実装すれば可。権限はマニフェストで許可制。
2. **サイドカープラグイン**: 子プロセスが JSON-RPC 2.0 over stdio で
   MoonLens と通信。OS 権限・重い ML・GPL 依存などに。

詳細: [`plugin-sdk/docs/README.md`](plugin-sdk/docs/README.md)

### 拡張ポイント

`translator` / `ocr` / `asr` / `explain` / `summarize` /
`reverse_image_search` / `qr_handler` / `intent_classifier` /
`action` / `pre_processor` / `post_processor` / `overlay_renderer` /
`hotkey_handler` / `history_indexer`

## 開発セットアップ

### 前提

- Rust stable (1.80+)
- Node.js 20+
- Tauri 2 系列のプラットフォーム依存（`npm run tauri info` で確認）
- CMake と C/C++ コンパイラ、libclang（内蔵 llama.cpp のビルド用。Windows は
  Visual Studio Build Tools + LLVM）
- Vulkan SDK（内蔵 llama.cpp を GPU で動かすため。Windows は LunarG の
  Vulkan SDK を入れて環境変数 `VULKAN_SDK` を通す。Linux は `libvulkan-dev`、
  `glslc`、`spirv-headers`。macOS は Metal を使うので不要）。Windows では
  llama.cpp の入れ子のビルドでパスが 260 文字を超えて失敗するため、
  `$env:CARGO_TARGET_DIR = "C:\mlt"` のように短いビルド先を指定する
- Windows では一度だけ `git config --global core.longpaths true` を実行する
  （llama.cpp を git から取得しており、パスが 260 文字を超えるため）
- 翻訳モデルはアプリ起動後、ホーム画面の「翻訳モデル」からダウンロードする
  （保存先: `<データディレクトリ>/moonlens/models/llm`）

- （字幕用）`whisper.cpp` サーバを `http://127.0.0.1:9002` で待機させる

### 初回

```bash
npm install
npm run tauri:dev
```

### ビルド

```bash
npm run tauri:build
```

### CI

`.github/workflows/build.yml` で Win / macOS / Linux の 3 系列ビルド、
`cargo fmt` / `cargo test` / `cargo clippy` / Tauri installer 生成を実行。

## 設計ドキュメント

実装方針の全体像は Plan ファイルに集約してある：
`docs/plan.md`（このリポジトリのスナップショット版）

## ライセンス

Apache License 2.0
