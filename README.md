# MoonLens

PC 常駐型の Google Lens 風ツール。単一のグローバルホットキーで

- ウィンドウ翻訳：窓の背面を OCR + ローカル LLM で常時翻訳し、原文位置へ訳文を重畳表示。
  窓は複数・任意サイズ・任意位置で配置可能（最大化で全画面翻訳）。字幕・チャット等の変化へ追従。
  既定は透過モード（窓上のクリックは背面へ）、操作モードで訳文の選択・コピー
- QR コード認識
- 音声翻訳：外国語動画のループバック音声から字幕表示
- ローカル LLM による要約・説明・対話

を統合。OCR・翻訳・要約・説明はアプリ内蔵エンジンにより端末内で完結し
（Ollama 等の外部ランタイムは不要）、画像・テキストの外部送信なし。
通信はユーザー操作によるモデル取得時のみ。音楽認識は運用コストの観点から本体に含めず、
プラグインによる実装の余地を確保。

## 導入

[Releases](https://github.com/c-ardinal/MoonLens/releases) から OS 対応のファイルを取得。

| OS | ファイル | 備考 |
| --- | --- | --- |
| Windows 10/11 (x64) | `MoonLens_<版>_x64-setup.exe` または `.msi` | 未署名のため SmartScreen を表示。「詳細情報 → 実行」で起動 |
| Linux (x64) | `.AppImage` / `.deb` / `.rpm` | AppImage は `chmod +x` 後に実行。Wayland ではポータル経由で画面取得 |
| macOS 11 以降（Apple シリコン） | `MoonLens_<版>_aarch64.dmg` | 未署名。初回は「システム設定 → プライバシーとセキュリティ → このまま開く」。画面収録・マイクの許可が必要 |

### 初回起動

1. 起動時にホーム画面を表示（タスクトレイにも常駐。`--hidden` 指定時はトレイのみ）。
2. ホーム画面の「モデル準備」で OCR モデル・翻訳モデルを取得。既定の翻訳モデルは
   PLaMo 3 Translate 2B（約 1.8 GB）。低速な PC では 0.8B 版を推奨。取得は中断・再開可能で、
   SHA-256 で検証。
3. `Ctrl+Shift+Space`（macOS は `⌘⇧Space`）でマウス位置へウィンドウ翻訳の窓を表示。
   窓を翻訳対象へ移動・サイズ変更すると、背面の文字を常時翻訳し原文位置へ訳文を表示。最大化で全画面翻訳。

主な既定ショートカット（設定画面で変更可）:

| 操作 | キー |
| --- | --- |
| ウィンドウ翻訳（窓の追加） | `Ctrl+Shift+Space` |
| ウィンドウ翻訳 表示／全終了 | `Ctrl+Shift+P` |
| ウィンドウ翻訳 透過／操作（全窓） | `Ctrl+Shift+E` |
| 履歴 | `Ctrl+Shift+H` |
| コマンドパレット | `Ctrl+K` |
| 音声翻訳 | `Ctrl+Shift+S`（初回は設定で音声認識モデルを導入） |

### アンインストール

OS の標準手順で削除。Windows のインストーラー（setup.exe）で「アプリデータ削除」を選択すると
設定・履歴・モデルも一括削除。その他（.msi、deb／rpm／AppImage）は次のフォルダーが残存するため、
不要なら手動で削除。実パスと使用容量は設定画面の「データ」に表示
（詳細は [docs/lifecycle.md](docs/lifecycle.md)）。

- Linux: `~/.config/io.moonlens.app/`、`~/.local/share/io.moonlens.app/`
- Windows: `%APPDATA%\io.moonlens.app\`、`%LOCALAPPDATA%\io.moonlens.app\`

## スタック

- **アプリシェル**: Tauri 2.x (Rust)
- **UI**: Svelte 5 (runes) + Vite + Tailwind CSS v4
- **OCR**: PP-OCRv5 mobile（`rten` で実行、日中英）を既定とし、ラテン文字専用の [`ocrs`](https://github.com/robertknight/ocrs) をフォールバックに使う（追加の OCR エンジンはプラグインで差し替え可）
- **QR**: `rqrr` crate
- **翻訳 (local)**: 内蔵 llama.cpp（`llama-cpp-2`、静的リンク）で GGUF を実行。GPU は Windows / Linux が Vulkan（NVIDIA・AMD・Intel）、macOS が Metal。GPU がない環境では CPU で動く。翻訳の既定は PLaMo 3 Translate 2B（0.8B も選択可）、説明・要約は Gemma 4 E4B。モデルはアプリのホーム画面からダウンロード（SHA-256 検証・中断再開）。モデル選定の根拠は [docs/translation-models.md](docs/translation-models.md)
- **翻訳 (cloud / Ollama)**: DeepL / Gemini API、Ollama は任意のオプション（既定チェーンには入っていない）
- **ASR**: 内蔵 whisper.cpp（`src-tauri/vendor/whisper.cpp`、llama.cpp と ggml・GPU バックエンドを共有して静的リンク）。既定モデルは Whisper large-v3-turbo（small も選択可）。モデルは設定の「モデル」からダウンロード（SHA-256 検証・中断再開）。選定の根拠は [docs/asr-models.md](docs/asr-models.md)。外部の whisper.cpp サーバーも任意のプロバイダーとして利用可
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
│  ├─ routes/          # overlay / subtitle / settings / history
│  └─ lib/
│     ├─ design/       # デザイントークン、モーション
│     └─ components/   # SchemaForm, CommandPalette, PrivacyBadge, ActionRibbon
├─ plugin-sdk/         # プラグイン開発キット（WIT + Rust + 例）
└─ .github/workflows/  # CI（Linux 既定）とリリースビルド
```

## カスタマイズ思想

MoonLens はハードコードされた挙動を極力排除。UI/挙動/プロバイダに関する
全定数は `config.toml`（Linux: `~/.config/io.moonlens.app/`、macOS: `~/Library/Application Support/io.moonlens.app/`、Windows: `%APPDATA%\io.moonlens.app\`）
で制御可能で、変更は保存時に即時反映。`config.toml` には既定値と異なる値のみを
保存し、既定値の一覧は同一フォルダーの `config.default.toml`（参照用）に記載。
各段階（導入〜削除）の挙動は [docs/lifecycle.md](docs/lifecycle.md) を参照。

### プロバイダチェーン

タスク（`translate` / `ocr` / `asr` / `explain` / `summarize` / ...）単位で
独立した順序付きチェーンとフォールバック条件を定義：

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

プロバイダ単位の設定は `[providers.configs."<id>"]` に任意キーで記述可能。
未知のキーも保存対象のため、プラグインは独自スキーマで
`[providers.configs."plugin.my_translator"]` を参照可能。

## プラグイン

### 2 形態

1. **WebAssembly Component プラグイン**: `wasmtime` サンドボックス内で動作。
   任意の言語（Rust / Go / TypeScript / Python / MoonBit...）で WIT
   インターフェースを実装。権限はマニフェストで許可制。
2. **サイドカープラグイン**: 子プロセスが JSON-RPC 2.0 over stdio で
   MoonLens と通信。OS 権限・高負荷 ML・GPL 依存等の用途向け。

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
- Vulkan SDK（内蔵 llama.cpp の GPU 実行用。Windows は LunarG の
  Vulkan SDK を導入し環境変数 `VULKAN_SDK` を設定。Linux は `libvulkan-dev`、
  `glslc`、`spirv-headers`。macOS は Metal 使用のため不要）。Windows では
  llama.cpp の入れ子ビルドでパスが 260 文字を超過し失敗するため、
  `$env:CARGO_TARGET_DIR = "C:\mlt"` 等の短いビルド先を指定
- Windows では初回のみ `git config --global core.longpaths true` を実行
  （llama.cpp を git から取得し、パスが 260 文字を超過するため）
- 翻訳モデルはアプリ起動後、ホーム画面の「モデル準備」でダウンロード
  （保存先: `<ローカルデータ>/io.moonlens.app/models/llm`）
- （字幕用）音声認識モデルも設定の「モデル」からダウンロードする
  （保存先: `<ローカルデータ>/io.moonlens.app/models/whisper`）

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

`.github/workflows/build.yml` が PR 単位で Linux 上の svelte-check / vitest / vite build と
`cargo fmt` / `clippy` / `cargo test` を実行。ネイティブコード変更を含む PR は `windows.yml` で
Windows でも検証。インストーラーはタグ push 時に `release.yml` が生成。
方針は [docs/ci-and-release-policy.md](docs/ci-and-release-policy.md)、
リリース手順は [docs/RELEASING.md](docs/RELEASING.md)。

## 設計ドキュメント

実装方針の全体像は Plan ファイルに集約：
`docs/plan.md`（本リポジトリのスナップショット版）

## ライセンス

MoonLens 本体は [MIT License](LICENSE)。

同梱ライブラリ（Rust crate、npm パッケージ、llama.cpp）のライセンス全文は、リリース単位で
`THIRD_PARTY_LICENSES.html` としてインストーラーに同梱し、Releases にも添付
（生成: `node scripts/third-party-licenses.mjs`、`cargo install cargo-about` が必要）。

同梱書体 DotGothic16 は SIL Open Font License 1.1（全文：`static/fonts/DotGothic16-OFL.txt`）。

アプリが取得するモデルは各ライセンスに準拠。

| モデル | ライセンス |
| --- | --- |
| PLaMo 3 Translate 2B / 0.8B | [PLaMo Community License](https://plamo.preferredai.jp/info/plamo-community-license-en) |
| Gemma 4 E4B / E2B | [Apache-2.0](https://ai.google.dev/gemma/apache_2) |
| PP-OCRv5 mobile | [Apache-2.0](https://github.com/PaddlePaddle/PaddleOCR/blob/main/LICENSE) |

Built with PLaMo.
