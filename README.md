# MoonLens

PC常駐の Google Lens 風ツール。1つのグローバルホットキーで

- 画面領域の OCR + ローカル LLM 翻訳 + オーバーレイ表示
- QR コード認識
- 外国語動画のループバック音声からライブ字幕
- ローカル LLM による要約・説明・対話

を統合する。OCR・翻訳・要約・説明はアプリに内蔵したエンジンで端末内だけで
完結し（Ollama などの外部ランタイムは不要）、画像やテキストを外部へ送信しない。
通信はユーザーが開始したモデル取得時のみ。音楽認識は運用コストの観点から本体には含めず、
プラグインとして実装できる余地を確保している。

## 導入

[Releases](https://github.com/c-ardinal/MoonLens/releases) から OS に合うファイルを取得する。

| OS | ファイル | 備考 |
| --- | --- | --- |
| Windows 10/11 (x64) | `MoonLens_<版>_x64-setup.exe` または `.msi` | 未署名のため SmartScreen が表示される。「詳細情報 → 実行」で起動 |
| Linux (x64) | `.AppImage` / `.deb` / `.rpm` | AppImage は `chmod +x` 後に実行。Wayland ではポータル経由で画面取得 |
| macOS | 未提供 | 次版以降で対応予定 |

### 初回起動

1. 起動するとホーム画面が開く（タスクトレイにも常駐。`--hidden` 付きで起動するとトレイのみ）。
2. ホーム画面の「モデル準備」から OCR モデルと翻訳モデルを取得する。既定の翻訳モデルは
   PLaMo 3 Translate 2B（約 1.8 GB）。低速な PC では 0.8B 版を選ぶ。取得は中断・再開でき、
   SHA-256 で検証される。
3. `Ctrl+Shift+Space`（macOS は `⌘⇧Space`）で範囲を選択すると、その場に訳文が重なって表示される。

主な既定ショートカット（設定画面で変更可）:

| 操作 | キー |
| --- | --- |
| 範囲を翻訳 | `Ctrl+Shift+Space` |
| ウィンドウを翻訳 | `Ctrl+Shift+W` |
| 結果を固定表示 | `Ctrl+Shift+P` |
| 履歴 | `Ctrl+Shift+H` |
| コマンドパレット | `Ctrl+K` |
| ライブ字幕 | `Ctrl+Shift+S`（別途 whisper.cpp サーバーが必要） |

### アンインストール

OS の標準手順で削除する。Windows のインストーラー（setup.exe）で「アプリデータ削除」を選ぶと
設定・履歴・モデルもまとめて削除される。それ以外（.msi、deb／rpm／AppImage）では次のフォルダーが残るので、
不要なら手動で削除する。実際のパスと使用容量は設定画面の「データ」に表示される
（詳細は [docs/lifecycle.md](docs/lifecycle.md)）。

- Linux: `~/.config/io.moonlens.app/`、`~/.local/share/io.moonlens.app/`
- Windows: `%APPDATA%\io.moonlens.app\`、`%LOCALAPPDATA%\io.moonlens.app\`

## スタック

- **アプリシェル**: Tauri 2.x (Rust)
- **UI**: Svelte 5 (runes) + Vite + Tailwind CSS v4
- **OCR**: [`ocrs`](https://github.com/robertknight/ocrs) crate（追加の OCR エンジンはプラグインで差し替え可）
- **QR**: `rqrr` crate
- **翻訳 (local)**: 内蔵 llama.cpp（`llama-cpp-2`、静的リンク）で GGUF を実行。翻訳の既定は PLaMo 3 Translate 2B（0.8B も選択可）、説明・要約は Gemma 4 E4B。モデルはアプリのホーム画面からダウンロード（SHA-256 検証・中断再開）。モデル選定の根拠は [docs/translation-models.md](docs/translation-models.md)
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
└─ .github/workflows/  # CI（Linux 既定）とリリースビルド
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
- Windows では一度だけ `git config --global core.longpaths true` を実行する
  （llama.cpp を git から取得しており、パスが 260 文字を超えるため）
- 翻訳モデルはアプリ起動後、ホーム画面の「翻訳モデル」からダウンロードする
  （保存先: `<ローカルデータ>/io.moonlens.app/models/llm`）

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

`.github/workflows/build.yml` が PR ごとに Linux で svelte-check / vitest / vite build と
`cargo fmt` / `clippy` / `cargo test` を実行する。ネイティブコードを触る PR は `windows.yml` で
Windows でも検証する。インストーラーはタグ push 時に `release.yml` が作る。
方針は [docs/ci-and-release-policy.md](docs/ci-and-release-policy.md)、
リリース手順は [docs/RELEASING.md](docs/RELEASING.md)。

## 設計ドキュメント

実装方針の全体像は Plan ファイルに集約してある：
`docs/plan.md`（このリポジトリのスナップショット版）

## ライセンス

MoonLens 本体は [MIT License](LICENSE)。

同梱するライブラリ（Rust crate、npm パッケージ、llama.cpp）のライセンス全文は、リリースごとに
`THIRD_PARTY_LICENSES.html` としてインストーラーに同梱し、Releases にも添付する
（生成: `node scripts/third-party-licenses.mjs`、`cargo install cargo-about` が必要）。

アプリが取得するモデルはそれぞれのライセンスに従う。

| モデル | ライセンス |
| --- | --- |
| PLaMo 3 Translate 2B / 0.8B | [PLaMo Community License](https://plamo.preferredai.jp/info/plamo-community-license-en) |
| Gemma 4 E4B / E2B | [Apache-2.0](https://ai.google.dev/gemma/apache_2) |
| PP-OCRv5 mobile | [Apache-2.0](https://github.com/PaddlePaddle/PaddleOCR/blob/main/LICENSE) |

Built with PLaMo.
