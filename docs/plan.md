# MoonLens — PC版 Google Lens風ツール（設計プラン）

## Context（なぜこれを作るか）

PC上で外国語コンテンツ（ゲーム画面、Webの画像内文字、PDF、動画字幕の読めない部分、動画音声そのもの、QRコード）に遭遇したとき、用途ごとに別ツール（翻訳サイト、QRアプリ、Whisperスクリプト、画像検索）へ手動で渡すのが面倒。Google Lens + ライブ字幕 + リバース画像検索 を **1つのグローバルホットキー** に統合し、ローカルLLMによるプライバシー重視の翻訳を第一級に扱う常駐ツールが欲しい。

## スタックの結論

- **フルRust + Tauri 2**。MoonBitハイブリッド案はIPC性能・pre-1.0非互換リスク・FFI作業量でコストが見合わず不採用（プロジェクト名「MoonLens」はブランドとして維持）。
- **音楽認識は不採用**：Shazam非公式APIは規約グレー、ACRCloud等は有償で運用コストが合わない。抽象層のpluginポイントだけ残し、個人利用拡張可能な余地を確保。
- 既存OSS調査結論：残り全機能（OCR翻訳／画像検索／QR／動画字幕／ローカルLLM）をクロスプラットフォームで統合した実装は無く、MoonLensの存在意義あり。各サブ機能はRust crate / サイドカーを **composeする** 方針。

## アーキテクチャ

```
┌──────────────────────────────────────────────────────────────┐
│ Tauri 2.x Shell                                               │
│  ├─ global-hotkey / tray-icon / window management            │
│  ├─ xcap (screen capture, マルチモニタ/HiDPI)                 │
│  ├─ cpal (audio loopback: WASAPI/CoreAudio/PulseAudio)       │
│  └─ Webview (overlay + 設定画面 + 履歴)                        │
├──────────────────────────────────────────────────────────────┤
│ Rust Core (同一プロセス、IPC無し)                              │
│  ├─ Pipeline orchestrator (capture→detect→dispatch)          │
│  ├─ Provider trait群 (Ocr / Translator / Asr / Search)       │
│  ├─ Intent classifier (QR/テキスト/画像を自動判別)              │
│  ├─ Cache / history (rusqlite + tantivy)                     │
│  └─ reqwest で Ollama / 外部APIへ                             │
├──────────────────────────────────────────────────────────────┤
│ External / サイドカー                                          │
│  ├─ OCR: ocrs crate (pure Rust) / PaddleOCR サイドカー予備    │
│  ├─ QR: rqrr crate                                           │
│  ├─ Translate: Ollama(gemma4) + DeepL/Gemini APIフォールバック │
│  ├─ ASR: whisper.cpp server (サイドカー、明示起動)             │
│  └─ 画像検索: ブラウザ起動 or SerpAPI                          │
└──────────────────────────────────────────────────────────────┘
```

## UI要件 — Modern / Simple / Refined

参照点：**Raycast / Linear / Arc / Obsidian**。静かで高密度、キーボードファースト。

- **フロントエンド**：Svelte 5（runes）+ Vite。軽量・起動速い・バンドル小。
- **スタイリング**：Tailwind CSS v4 + カスタムデザイントークン。shadcn丸出しにせず自作コンポーネント。
- **モーション**：`motion-one` or Svelte transitions。60fps spring のみ、派手なエフェクト排除。
- **カラー**：モノクロ基調（#0A0A0A / #FAFAFA）+ 単一アクセント（violet-500系）。ダーク/ライト自動切替＋手動。
- **タイポ**：Inter Variable（UI）+ JetBrains Mono（コード）。行間広め、字間タイト。
- **アイコン**：Lucide。線画1.5px統一。
- **ウィンドウクローム**：`decorations: false` + 自作タイトルバー。角丸12px、微細ボーダー（1px, 5%不透明）、影 `0 8px 32px rgba(0,0,0,.12)`。
- **オーバーレイ**：半透明 backdrop-filter（blur 24px, saturate 1.8）。ガラスパネル。
- **結果パネル**：コマンドパレット様式。`⌘K` でインラインアクションメニュー。
- **マイクロコピー**：日本語、体言止め・短文中心、情報密度優先。
- **アクセシビリティ**：全機能キーボード可、AA以上コントラスト。
- **空間設計**：8pxベースグリッド、パディング12/16/24の3種のみ、余白潤沢。

## 機能スコープ（一気通貫で全実装）

フェーズ分けはしない。以下をすべてリリース対象とする：

1. **グローバルホットキー + 領域選択 + OCR + ローカルLLM翻訳 + オーバーレイ表示**
2. **自動コンテンツ判別**：QR／テキスト主体／写真主体 を領域内容から自動分岐。曖昧時のみミニツールバー
3. **結果パネル アクションリボン**：翻訳／原文コピー／訳文コピー／画像検索／LLM説明／ピン留め
4. **ローカル優先 + クラウドフォールバック**：Ollama未起動/タイムアウト時にDeepL/Gemini APIへ自動切替、プライバシー表示ラベル更新
5. **ストリーミング翻訳**：OCR行単位でLLMへ流し、初トークン <500ms で逐次描画
6. **ピン留めオーバーレイ**：ソース領域の変化を監視して自動再翻訳（ゲーム/動画用）
7. **QRコード認識**：URL/テキスト/Wi-Fi/連絡先を判定、安全な場合のみワンクリック起動
8. **リバース画像検索**：Google Images / Bing / Yandex をブラウザで開く ＋ SerpAPIキーがあればアプリ内結果
9. **外国語動画のライブ字幕**：対象ウィンドウ選択→ループバック音声→whisper.cpp→翻訳→半透明オーバーレイ字幕。別ホットキーでトグル
10. **履歴タイムライン**：rusqlite + tantivy全文検索、画像サムネイル付き
11. **設定画面**：ホットキー、LLMモデル、翻訳元/先言語、APIキー、プライバシー設定、オーバーレイ外観
12. **システムトレイ**：常駐起動、メニューから主要機能呼び出し
13. **LLM対話化**：翻訳結果から「要約」「この画像は？」「このQRのURLは安全？」をワンクリック
14. **クリップボード統合**：翻訳後に訳文/原文の自動コピー選択可能
15. **macOS 権限オンボーディング**：画面収録／アクセシビリティ／マイク許可ガイドを段階表示
16. **プラグイン管理画面**：インストール／有効化／無効化／更新／アンインストール／権限確認／設定編集

## 設定の外出し方針（カスタマイズ性最優先）

「UI・挙動・プロバイダに関わるあらゆる定数を設定ファイルから制御可能にする」を原則とする。ハードコードは禁止、全てSettings経由。工数増は許容。

### 設定保存形式
- **形式**：TOML（可読性）＋JSON Schemaでバリデーション。
- **場所**：OS既定の config dir（`$XDG_CONFIG_HOME/moonlens/config.toml` 等）。
- **レイヤ**：`default.toml`（同梱）→ `user.toml`（上書き）→ プロファイル（複数切替可）。
- **ホットリロード**：保存で即反映、再起動不要（ホットキー再登録等も）。
- **エクスポート/インポート**：JSON/TOMLで書き出し・読み込み、Gist共有用URL対応。

### プロバイダ優先度チェーン（全タスク共通モデル）
各タスク（Translate / OCR / ASR / Explain / Summarize / ReverseImageSearch）ごとに独立した **順序付きチェーン** と **フォールバック条件** を定義可能：

```toml
[providers.translate]
chain = ["ollama.gemma4", "deepl", "gemini", "plugin.my_translator"]
parallel_race = false           # trueで同時呼び出し、最速勝ち
fallback_on = ["timeout", "error", "rate_limit", "offline"]
timeout_ms = 3000
allow_cloud = true              # falseならローカルのみ
allow_network = true
max_cost_per_request = 0.001    # USD想定
sticky_session_ms = 30000       # 一度成功したプロバイダをこの期間優先

[providers.translate.ollama.gemma4]
endpoint = "http://127.0.0.1:11434"
model = "gemma4:7b-instruct-q4"
temperature = 0.2
top_p = 0.9
num_ctx = 8192
stream = true
system_prompt_file = "prompts/translate.txt"

[providers.translate.deepl]
api_key_env = "DEEPL_API_KEY"
formality = "default"
```

### タスク別に上書き可能な項目（例）
- **Translate**：言語ペア既定、敬体/常体、用語集ファイル、禁止ワード、カスタムシステムプロンプト、chainごとの最低文字数閾値
- **OCR**：言語ヒント、エンジン優先度（`ocrs` / `paddle` / plugin）、前処理（二値化・デノイズ・upscale係数）、信頼度閾値、矩形検出モード
- **QR**：URLドメイン allow/deny、プロトコル allow（http/https/wifi/tel/mailto）、警告表示条件
- **ASR**：モデル（`large-v3-turbo`/`medium`/`base`）、VAD感度、言語固定/自動、翻訳パイプの有無、chunk長、overlap
- **ReverseImageSearch**：検索エンジン順（Google/Bing/Yandex/SauceNAO/...）、外部ブラウザ/アプリ内のどちら、SerpAPI使用可否
- **Explain / Summarize / CustomAction**：各々独立したプロバイダチェーンとプロンプト
- **Overlay**：フォント・行高・不透明度・blur強度・最大幅・配置・自動追従方式・消滅条件・影
- **Hotkey**：機能ごとに全て割当可（Capture/PinOverlay/Subtitle開始/終了/コマンドパレット/履歴/...）
- **Clipboard**：コンテンツ種別（text/qr/translation/transcript）ごとに自動コピー on/off
- **Privacy**：プロバイダごとに `allow_cloud` 上書き、ログレベル、履歴保存期間、暗号化 on/off、画像保存 on/off
- **Storage**：履歴件数上限、保存先、暗号化キー導出方式、バックアップ
- **Telemetry**：デフォルトoff、opt-inのみ
- **Appearance**：テーマ（light/dark/system）、アクセントカラー、密度（compact/normal/comfortable）、言語（UI）
- **Performance**：デバウンスms、GPU利用、同時実行数、キャッシュTTL
- **Per-Plugin**：プラグイン側が宣言したschemaに従う

### 設定UI実装
- 設定画面は **自動生成** を基本：各Rust側モジュールが `SettingsSchema`（JSON Schema＋UIヒント）を publish し、Svelte側が自動でフォームを描画。新規プロバイダ追加時にUI手書き不要。
- 詳細編集派のため **Rawエディタ** （Monacoでtoml編集＋即バリデーション）も提供。
- 検索バー（設定項目横断検索）、差分表示、ワンクリックで「既定に戻す」。

## プラグイン機構

ユーザが機能を追加・削除できる仕組み。音楽認識、特殊OCR、独自翻訳プロバイダ、カスタムアクション等を本体から独立して配布可能。

### 2種のプラグイン形態
1. **WASM Component プラグイン**（推奨・デフォルト）
   - Runtime：`wasmtime` + WebAssembly Component Model（WIT interface）
   - 言語自由：Rust / Go / TypeScript(jco) / Python(componentize-py) / MoonBit 等、WASM Componentを出力できれば何でも
   - サンドボックス化：WASI preview2、ファイル/ネット/環境変数アクセスは宣言許可制
   - 配布：単一 `.wasm` + `plugin.toml` manifest を zip したもの（`.moonlens-plugin`）
   - 用途：純粋計算（OCR後処理、翻訳プロバイダ呼び出しラッパ、カスタムアクション、intent分類拡張）

2. **Sidecar プラグイン**
   - プロセス分離、JSON-RPC over stdio（`jsonrpc-core`互換）
   - 言語・ライブラリ自由（Python/Node/シェルスクリプトまで可）
   - OS API・重いML・GPL依存が必要な機能向け（例：音楽認識、PaddleOCR、whisper、OBS連携）
   - 配布：実行ファイル＋`plugin.toml`＋依存セットアップスクリプト
   - 起動制御：onDemand / persistent / keepAlive秒数 を選択

### プラグインマニフェスト `plugin.toml`
```toml
[plugin]
id = "com.example.music-recognizer"
name = "Music Recognizer"
version = "1.0.0"
author = "Example"
license = "MIT"
kind = "sidecar"                     # "wasm" or "sidecar"
entry = "bin/recognizer"             # wasm: path/to/component.wasm
api_version = "1"                    # MoonLens Plugin API バージョン

[capabilities]
# 宣言した能力のみMoonLensが呼び出し対象として認識
provides = ["music_recognizer"]      # or "translator","ocr","asr","action","intent_classifier","overlay_renderer" 等
consumes = ["audio_loopback"]        # MoonLens本体から貰う入力

[permissions]
# 許可制、ユーザがインストール時に確認
network = ["https://api.example.com"]
fs_read = []
fs_write = ["$plugin_data/"]
audio_capture = true
screen_capture = false
clipboard = false
subprocess = false

[settings]
# 自動生成UIに使うschema
schema = "schema.json"

[ui]
# オプション：プラグイン専用の設定タブ
settings_page = "ui/settings.html"
```

### 拡張ポイント（Extension points）
プラグインが提供できる役割：
- `translator` / `ocr` / `asr` / `reverse_image_search` / `qr_handler`
- `intent_classifier`（領域内容の判定を拡張）
- `action`（結果パネルのアクションリボンに項目追加）
- `pre_processor` / `post_processor`（キャプチャ画像の補正、翻訳結果の後処理）
- `overlay_renderer`（特殊な表示モード）
- `hotkey_handler`（任意のカスタムショートカット機能）
- `history_indexer`（履歴の追加メタデータ）

各拡張ポイントは Rust trait として定義、WIT にもミラー。チェーンに「plugin.<id>」として登録可能。

### プラグイン配布・インストール
- **ローカルインストール**：`.moonlens-plugin` zipファイルをドラッグ＆ドロップ
- **レジストリ（将来）**：GitHub Releases ベースの index（mooncakes.io 風）。v1では手動配布のみでOK
- **署名**：ed25519で manifest に署名、レジストリ経由は署名必須、ローカルは警告のみ
- **更新**：manifest に `update_url` 指定時に自動チェック（opt-in）
- **有効化/無効化**：再起動不要、トグル即反映

### プラグイン開発キット
- `plugin-sdk/` に WIT定義、Rust macro（`#[moonlens::plugin]`）、サンプル3種（Hello Translator / Music Recognizer stub / Custom Action）
- `moonlens plugin new` CLI で雛形生成
- デバッグ用：ホットリロード、ログビュー、MoonLens本体API modelビュー
- ドキュメント：extension point ごとの仕様、セキュリティガイド、公開手順

## 採用する既存OSS / crate

| 領域 | 採用 | 備考 |
|---|---|---|
| アプリシェル | Tauri 2.x | Win/Mac/Linux、軽量 |
| UIフレームワーク | Svelte 5 + Vite | runes、軽快 |
| スタイリング | Tailwind CSS v4 | JITで最小バンドル |
| OCR主 | [`ocrs`](https://github.com/robertknight/ocrs) | Pure Rust |
| OCR予備 | PaddleOCR サイドカー | 日中韓精度用 |
| QR | `rqrr` | ZBar相当、軽量 |
| 翻訳(local) | Ollama + `gemma4` | reqwestストリーミング |
| 翻訳(cloud) | DeepL / Gemini API | フォールバック |
| 画像検索 | ブラウザfallback + SerpAPI(任) | 規約上クローラ不可 |
| ASR | [`whisper.cpp`](https://github.com/ggerganov/whisper.cpp) server | サイドカー |
| 音声取得 | `cpal` | WASAPI loopback含む |
| ホットキー | `global-hotkey` (Tauri公式) | |
| 画面キャプチャ | `xcap` | マルチモニタ |
| トレイ | `tray-icon` (Tauri公式) | |
| DB | `rusqlite` + `tantivy` | 履歴＋全文検索 |
| HTTP | `reqwest` (rustls) | ストリーミング対応 |
| ロギング | `tracing` + `tracing-subscriber` | |
| WASMランタイム | `wasmtime` | プラグイン用 Component Model |
| 設定 | `toml` + `schemars` + `jsonschema` | TOML保存・schema生成・検証 |
| JSON-RPC | `jsonrpsee` | sidecarプラグイン通信 |

**参考にするが流用しない**：Translumo（Win限定だがオーバーレイUX）、LunaTranslator（マルチバックエンド抽象）、NormCap（クロスプラットフォームOCR UX）、Koharu（Rust+MLパイプライン）、Raycast（UI density）。

## リスクと対策

- **macOS配布**：Apple公証必須。TCCプロンプトUXを段階的に案内。
- **Webview再描画コスト**：オーバーレイはdiff更新のみ、ストリーミング翻訳チャタリングは50msデバウンス。
- **LLMレイテンシ**：Ollama `/api/generate` `stream:true` 必須、初トークン <500ms目標。
- **whisper.cpp CPU負荷**：常時稼働せず明示トグル、アイドル時停止。デフォルト `large-v3-turbo` 量子化、軽量機向け `base` 自動フォールバック。
- **画像検索のAPI規約**：ブラウザ起動が第一選択、SerpAPIは任意キー設定時のみ。
- **OCR精度**：ocrsが英数に強いがCJK弱い場合 PaddleOCR サイドカーへ言語別ルーティング。
- **Wayland画面キャプチャ**：ポータル経由必須、xcapのWayland対応状況をCI確認。

## リポジトリ構成

```
MoonLens/
├─ src-tauri/
│  ├─ src/
│  │  ├─ main.rs
│  │  ├─ app.rs               # Tauri setup
│  │  ├─ capture.rs           # xcap (screen, region)
│  │  ├─ hotkey.rs            # global-hotkey
│  │  ├─ audio.rs             # cpal loopback
│  │  ├─ pipeline.rs          # orchestration, streaming
│  │  ├─ intent.rs            # QR/テキスト/画像判別
│  │  ├─ providers/
│  │  │  ├─ mod.rs            # trait定義
│  │  │  ├─ ocr_ocrs.rs
│  │  │  ├─ ocr_paddle.rs     # サイドカー
│  │  │  ├─ translate_ollama.rs
│  │  │  ├─ translate_deepl.rs
│  │  │  ├─ translate_gemini.rs
│  │  │  ├─ asr_whisper.rs
│  │  │  ├─ qr_rqrr.rs
│  │  │  └─ search_serpapi.rs
│  │  ├─ storage.rs           # rusqlite + tantivy
│  │  ├─ overlay.rs           # pinned overlay, region watcher
│  │  ├─ settings/            # 階層型設定、schema自動生成
│  │  │  ├─ mod.rs
│  │  │  ├─ schema.rs         # schemars で JSON Schema
│  │  │  ├─ store.rs          # TOML load/save/watch
│  │  │  └─ profiles.rs
│  │  ├─ plugins/             # プラグイン基盤
│  │  │  ├─ mod.rs
│  │  │  ├─ manifest.rs
│  │  │  ├─ wasm_host.rs      # wasmtime Component
│  │  │  ├─ sidecar_host.rs   # JSON-RPC over stdio
│  │  │  ├─ registry.rs       # 有効/無効、権限管理
│  │  │  └─ wit/              # WIT interface 定義
│  │  └─ ipc.rs               # Tauri command定義
│  ├─ Cargo.toml
│  └─ tauri.conf.json
├─ src/                       # Svelte 5 + Vite
│  ├─ routes/
│  │  ├─ overlay/+page.svelte        # キャプチャ結果・翻訳
│  │  ├─ subtitle/+page.svelte       # ライブ字幕オーバーレイ
│  │  ├─ settings/+page.svelte       # schema駆動自動生成フォーム
│  │  ├─ plugins/+page.svelte        # プラグイン管理
│  │  └─ history/+page.svelte
│  ├─ lib/
│  │  ├─ design/
│  │  │  ├─ tokens.css        # color, spacing, radius, shadow
│  │  │  └─ motion.ts         # spring presets
│  │  ├─ components/          # Button, Panel, CommandPalette, ActionRibbon, PrivacyBadge, RegionWatcher
│  │  ├─ stores/              # settings, history state (runes)
│  │  └─ ipc.ts               # invoke wrappers
│  ├─ app.css                 # Tailwind layer
│  └─ app.html
├─ vite.config.ts
├─ tailwind.config.ts
├─ svelte.config.js
├─ package.json
├─ plugin-sdk/                # プラグイン開発キット
│  ├─ wit/                    # WIT interface 定義
│  ├─ rust/                   # Rust マクロ + 型
│  ├─ examples/
│  │  ├─ hello-translator/    # WASM Component 例
│  │  ├─ music-recognizer/    # sidecar 例
│  │  └─ custom-action/
│  ├─ docs/                   # 拡張ポイント仕様、セキュリティ
│  └─ cli/                    # `moonlens plugin new` 雛形生成
├─ .github/workflows/
│  └─ build.yml               # Win/Mac/Linux 3系列 + installer
├─ LICENSE
└─ README.md
```

## 検証方法（完成判定）

1. **ゴールデンパス（Win/Mac/Linux 各OS）**：
   - 英語Web画像テキスト → 日本語オーバーレイ（Ollama接続時 <2秒、初トークン <500ms）
   - QR含む領域 → URL抽出＋「開く」
   - 動画再生中 → ループバック音声→Whisper→翻訳字幕オーバーレイ（遅延 <1.5秒）
   - 写真領域 → アクション選択UI → ブラウザでGoogle Images起動
2. **プロバイダフォールバック**：Ollama停止 → DeepL自動切替 → プライバシー表示「Cloud」へ遷移、トースト通知
3. **ピン留め再翻訳**：原文が変わると自動で訳文更新、変化なしでは再翻訳せず
4. **履歴全文検索**：100件以上の履歴から日本語/英語混在で検索ヒット
5. **CI**：GitHub ActionsでWin/Mac/Linux 3系列ビルド、installer生成
6. **Rust単体テスト**：intent判別・状態遷移・プロバイダ選択・フォールバックロジック（`cargo test`）
7. **E2E**：Playwrightでオーバーレイ/設定画面のインタラクション
8. **性能**：HiDPIマルチモニタでキャプチャずれ無し、常駐メモリ <150MB（LLM除く）、ホットキー反応 <100ms
9. **プライバシー**：ネットワーク全ブロック + Ollama起動環境でローカル翻訳完結
10. **設定ホットリロード**：`config.toml` 保存で即時反映（ホットキー再登録、モデル差替、UI再描画）
11. **プラグイン**：サンプル3種が有効化→動作→無効化で完全に機能停止、権限宣言と実アクセスが一致
12. **プロバイダチェーン**：Translate chain 1番目タイムアウト→2番目成功、UIにフォールバック理由表示

## 初期コミットで作成するファイル

現状LICENSE/READMEのみ。新規作成：
- `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`
- `src-tauri/src/main.rs`, `app.rs`, `hotkey.rs`, `capture.rs`, `audio.rs`, `pipeline.rs`, `intent.rs`, `overlay.rs`, `settings.rs`, `storage.rs`, `ipc.rs`
- `src-tauri/src/providers/` 配下一式
- `package.json`, `vite.config.ts`, `tailwind.config.ts`, `svelte.config.js`
- `src/app.html`, `src/app.css`, `src/lib/design/tokens.css`, `src/lib/design/motion.ts`, `src/lib/ipc.ts`
- `src/routes/overlay/+page.svelte`, `subtitle/+page.svelte`, `settings/+page.svelte`, `history/+page.svelte`
- `src/lib/components/` 配下一式
- `.github/workflows/build.yml`
- `README.md` 更新（前提ツール、Ollama gemma4モデル、whisper.cppモデル配置、開発手順）
- `src-tauri/src/settings/` 配下一式、`default.toml`（全項目のデフォルト）
- `src-tauri/src/plugins/` 配下一式、`wit/moonlens.wit`
- `plugin-sdk/` 配下一式（WIT、Rustマクロ、サンプル3種、CLI雛形生成）
- `src/routes/plugins/+page.svelte`、schema駆動フォームジェネレータ `src/lib/components/SchemaForm.svelte`
