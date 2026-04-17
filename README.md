# MoonLens

PC常駐の Google Lens 風ツール。1つのグローバルホットキーで

- 画面領域の OCR + ローカル LLM 翻訳 + オーバーレイ表示
- QR コード認識
- リバース画像検索
- 外国語動画のループバック音声からライブ字幕
- ローカル LLM による要約・説明・対話

を統合する。ローカル LLM（Ollama + Gemma4）を第一級に扱い、クラウドは明示的な
フォールバックとして利用する。音楽認識は運用コストの観点から本体には含めず、
プラグインとして実装できる余地を確保している。

## スタック

- **アプリシェル**: Tauri 2.x (Rust)
- **UI**: Svelte 5 (runes) + Vite + Tailwind CSS v4
- **OCR**: [`ocrs`](https://github.com/robertknight/ocrs) crate（将来 PaddleOCR サイドカーへルーティング）
- **QR**: `rqrr` crate
- **翻訳 (local)**: Ollama HTTP, デフォルトモデル `gemma4:7b-instruct-q4_K_M`
- **翻訳 (cloud)**: DeepL / Gemini API（オプション）
- **ASR**: `whisper.cpp` server（サイドカー起動）
- **画面キャプチャ**: `xcap`、**音声ループバック**: `cpal`
- **履歴**: `rusqlite` + `tantivy`
- **プラグイン**: WebAssembly Component Model (`wasmtime`) と JSON-RPC サイドカー

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
すべての定数は `$XDG_CONFIG_HOME/moonlens/config.toml`（およびプロファイル）
から制御可能で、変更は保存で即時反映される。設定画面は `schemars` が吐く
JSON Schema から自動生成されるため、新しい provider や plugin を追加して
もフォームの手書き更新は不要。

### プロバイダチェーン

タスク（`translate` / `ocr` / `asr` / `explain` / `summarize` / ...）ごとに
独立した順序付きチェーンとフォールバック条件を定義する：

```toml
[providers.translate]
chain = ["ollama.gemma4", "deepl", "gemini", "plugin.my_translator"]
parallel_race = false
fallback_on = ["timeout", "error", "offline"]
timeout_ms = 3000
allow_cloud = true
allow_network = true
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
- （翻訳用）Ollama + `gemma4` モデル

```bash
ollama pull gemma4:7b-instruct-q4_K_M
ollama serve
```

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
