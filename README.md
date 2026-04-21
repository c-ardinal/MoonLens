# MoonLens

> PC 版 Google Lens。1 つのグローバルホットキーで、画面上の外国語テキストを
> **その場で** OCR + 翻訳し続け、QR・画像検索・ライブ字幕まで 1 箇所にまとめる
> プライバシーファーストの常駐ツール。

> **English summary**: MoonLens is a privacy-first, keyboard-driven desktop
> companion that streams the contents of any screen region through OCR +
> local LLM translation, overlaying the result in place. Beyond OCR it
> also handles QR codes, reverse image search, and loopback-audio live
> subtitles. Local-first via Ollama + Gemma 4, with optional cloud fallback.

- 📷 **Lens ストリーミング**: Lens ウィンドウを対象領域に重ねると、
  継続的に OCR + 翻訳を in-place で描画し続ける
- ⚡ **ワンショット領域キャプチャ**: `Ctrl+Alt+Space` → ドラッグ選択 →
  オーバーレイで結果表示（QR / テキスト / 写真を自動分岐）
- 🔒 **ローカル優先**: 既定では Ollama + Gemma 4 (`gemma4:e2b`) をローカルで実行。
  クラウドは明示的な許可時のみ DeepL / Gemini にフォールバック
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
- **翻訳 (local)**: Ollama HTTP ストリーミング、既定モデル `gemma4:e2b` (`think: false` 必須)
- **翻訳 (cloud)**: DeepL / Gemini API（オプションフォールバック）
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
- [Ollama](https://ollama.com/) + `gemma4:e2b` モデル

```bash
ollama pull gemma4:e2b
ollama serve
```

字幕機能を使う場合は `whisper.cpp` サーバを `http://127.0.0.1:9002`
で起動しておく（[`docs/USAGE.md`](docs/USAGE.md) の字幕セクション参照）。

### 開発モード

```bash
npm install
npm run tauri:dev
```

### リリースビルド

```bash
npm run tauri:build
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
独立した順序付きチェーンとフォールバック条件を定義する:

```toml
[providers.translate]
chain = ["ollama.gemma4", "deepl", "gemini", "plugin.my_translator"]
parallel_race = false
fallback_on = ["timeout", "error", "offline"]
timeout_ms = 3000
allow_cloud = true
allow_network = true
sticky_session_ms = 30000

[providers.translate.ollama.gemma4]
endpoint = "http://127.0.0.1:11434"
model = "gemma4:e2b"
think = false                       # 必須: 10× first-token 差
num_ctx = 4096
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
