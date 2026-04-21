# MoonLens 開発者ガイド

このドキュメントは MoonLens のコードに手を入れる開発者向けです。
ユーザ向けの使い方は [`USAGE.md`](USAGE.md)、設計判断は [`plan.md`](plan.md)
にあります。

## 目次

- [1. 開発環境のセットアップ](#1-開発環境のセットアップ)
- [2. 主要コマンド](#2-主要コマンド)
- [3. コード構成](#3-コード構成)
- [4. アーキテクチャ概要](#4-アーキテクチャ概要)
- [5. テスト](#5-テスト)
- [6. リリースフロー](#6-リリースフロー)
- [7. プラグイン開発](#7-プラグイン開発)
- [8. デバッグ](#8-デバッグ)

## 1. 開発環境のセットアップ

### 必要なツール

- **Rust** stable 1.80 以降（`rustup update stable`）
- **Node.js** 20 以降 + npm
- **Tauri 2 系プラットフォーム依存**:
  - Windows: WebView2 Runtime（Win11 は同梱済み）
  - Linux: `webkit2gtk-4.1`, `libssl`, `libayatana-appindicator`, etc.
    具体名は `npm run tauri info` で確認
- **Ollama** + `gemma4:e2b` モデル（翻訳機能を動かす場合）
- **whisper.cpp** HTTP サーバ（字幕機能を動かす場合、`http://127.0.0.1:9002`）

### クローン後

```bash
npm install
npm run tauri info   # 環境チェック
```

開発用ホットキー・プライバシー設定・ログ等は `$APPDATA/moonlens/`
（Windows）または `$XDG_CONFIG_HOME/moonlens/`（Linux）に保存されます。

## 2. 主要コマンド

| コマンド | 用途 |
|---|---|
| `npm run dev` | Vite 単体（`http://localhost:1420`）。Tauri IPC 呼び出しは `try/catch` で degrade、UI 描画の純粋な目視確認向け |
| `npm run tauri:dev` | Tauri ネイティブウィンドウで起動。実際の UX 検証はこちらで |
| `npm run build` | SvelteKit をビルド (`build/`) |
| `npm run tauri:build` | 配布用インストーラを生成。Win は `.msi`/`.nsis`、Linux は `.AppImage`/`.deb`/`.rpm`、macOS は `.dmg` |
| `npm run check` | `svelte-kit sync` + `svelte-check` — TypeScript / Svelte の静的チェック |
| `npm test` | vitest（unit + DOM）。現状 9 スイート / 67 件 |
| `cd src-tauri && cargo test` | Rust side unit + integration |
| `cargo test --test lens_pipeline_live -- --ignored` | Lens pipeline の live 統合テスト（OCR + 実 Ollama） |
| `cargo fmt --check` / `cargo clippy --all-targets` | 整形 / lint 強制 |

## 3. コード構成

```
src-tauri/src/
├─ main.rs                # Tauri エントリ、setup hook
├─ app.rs                 # AppState (settings, pipeline, watchers), tray
├─ ipc.rs                 # #[tauri::command] 定義、IPC surface
├─ pipeline.rs            # チェーン実行、preload、fallback state machine
├─ providers/
│  ├─ mod.rs              # Provider trait + Task / Intent / ChunkSink
│  ├─ ocr_ocrs.rs         # ocrs + 背景色サンプリング
│  ├─ translate_ollama.rs # think: false / stream: true, JSON line parser
│  ├─ translate_deepl.rs  # DeepL REST client
│  ├─ translate_gemini.rs # Gemini REST client
│  ├─ asr_whisper.rs      # whisper-server HTTP client
│  ├─ qr_rqrr.rs          # rqrr
│  └─ search_serpapi.rs
├─ lens.rs                # Lens ウィンドウのライフサイクル
├─ lens_watcher.rs        # 差分検出エンジン (move/resize trigger + position poll)
├─ hotkey.rs              # global-hotkey, best-effort 登録 + 失敗レポート
├─ capture.rs             # xcap ラッパ
├─ audio.rs               # cpal loopback
├─ intent.rs              # QR/テキスト/画像判別
├─ storage.rs             # rusqlite (session 単位集約)
├─ settings/
│  ├─ mod.rs              # 階層 Settings struct (schemars + serde)
│  ├─ schema.rs           # JSON Schema 自動生成
│  └─ store.rs            # default.toml → user.toml merge
├─ plugins/
│  ├─ mod.rs              # PluginHost
│  ├─ manifest.rs         # plugin.toml parser
│  ├─ wasm_host.rs        # wasmtime Component Model
│  ├─ sidecar_host.rs     # jsonrpsee over stdio
│  └─ signature.rs        # ed25519_dalek
└─ lib.rs                 # pub mod 公開

src/
├─ app.css                # Tailwind entry + tokens import
├─ app.html               # SvelteKit shell
├─ routes/
│  ├─ +layout.svelte      # WindowChrome + AppSidebar シェル（浮動 window は bypass）
│  ├─ +layout.ts          # SvelteKit SSR 無効化
│  ├─ +page.svelte        # ホーム（ダッシュボード）
│  ├─ settings/+page.svelte   # auto-save + SaveIndicator
│  ├─ history/+page.svelte    # rusqlite 検索 UI
│  ├─ plugins/+page.svelte    # plugin list + install + toggle
│  ├─ subtitle/+page.svelte   # 字幕ウィンドウ描画
│  ├─ overlay/+page.svelte    # one-shot 結果 overlay
│  ├─ lens/+page.svelte       # Lens in-place card renderer
│  └─ select/+page.svelte     # 領域選択 overlay
├─ lib/
│  ├─ design/tokens.css       # CSS custom properties (light/dark/density/motion)
│  ├─ components/
│  │  ├─ WindowChrome.svelte      # カスタムタイトルバー
│  │  ├─ AppSidebar.svelte        # 常駐 nav
│  │  ├─ AppHeader.svelte         # サブページヘッダ（back ボタンは opt-in）
│  │  ├─ SchemaForm.svelte        # $ref 解決 + map + hotkey widget + 警告
│  │  ├─ HotkeyInput.svelte       # accelerator recorder widget
│  │  ├─ SaveIndicator.svelte     # saved/dirty/saving/error
│  │  ├─ CommandPalette.svelte    # ⌘K
│  │  ├─ ActionRibbon.svelte      # intent-aware action row
│  │  ├─ PrivacyBadge.svelte      # Local/Cloud/Rate-limited/Blocked
│  │  ├─ HotkeyFailureToast.svelte
│  │  └─ MacOnboarding.svelte     # macOS 権限オンボ (v0.2 で再編)
│  ├─ ipc.ts                      # 型付き Tauri invoke / listen ヘルパ
│  ├─ hotkey.ts                   # accelerator → symbol 変換 + validation
│  ├─ schema-filter.ts            # settings 検索フィルタ
│  └─ result.ts                   # TaskOutput 判別ヘルパ
└─ ...

plugin-sdk/                # プラグイン開発キット
├─ wit/                    # WIT interface 定義
├─ rust/                   # Rust 用 macro + runtime
├─ examples/               # hello-translator, music-recognizer stub, custom-action
├─ cli/                    # moonlens plugin new
└─ docs/
```

## 4. アーキテクチャ概要

```
Tauri 2 Shell
 ├─ global-hotkey / tray-icon / window management
 ├─ xcap (one-shot capture)
 ├─ DXGI Desktop Duplication (S2 で Windows streaming 用)
 ├─ WDA_EXCLUDEFROMCAPTURE (S2 で Windows 自己除外)
 ├─ cpal (audio loopback)
 └─ WebView (overlay / settings / history / sidebar)
──────────────────────────────
Rust Core（同一プロセス、IPC 無し）
 ├─ Pipeline orchestrator (capture → detect → dispatch)
 ├─ Lens streaming engine (stream → pixel diff → OCR → translate)
 ├─ Provider trait 群 (Ocr / Translator / Asr / QR / Search)
 ├─ Intent classifier (QR / テキスト / 画像を自動判別)
 ├─ Cache / history (rusqlite)
 └─ reqwest で Ollama / 外部 API へ
──────────────────────────────
External / サイドカー
 ├─ OCR: ocrs (pure Rust、S2 で Win OCR API 差替候補)
 ├─ QR: rqrr
 ├─ Translate: Ollama + DeepL/Gemini fallback
 ├─ ASR: whisper.cpp HTTP server
 └─ 画像検索: ブラウザ起動 or SerpAPI
```

### Lens streaming のデータフロー

```
window move/resize event ──┐
position polling (250ms)  ─┴─► trigger
                               │
                               ▼
                         one-shot capture (xcap)
                               │
                               ▼
                         ocrs: lines + bboxes + bg color sampling
                               │
                       ┌──────┴──────┐
                       ▼             ▼
                   unchanged     changed lines
                   (diff)        │
                       │         ▼
                       │   parallel translate (max_concurrent_translations=4)
                       │         │
                       ▼         ▼
                    reuse   streaming chunks
                       │         │
                       └──► in-place overlay render
```

詳細: [`plan.md` § Lens streaming データフロー](plan.md)。

## 5. テスト

### テスト戦略（Q14）

- **Unit（全 OS、高速）**: `cargo test` の通常路、`npm test` (vitest)
  — trait-level mock、fake impl 流し込み
- **Integration（Linux CI 可）**: real `ocrs` + `MockOllamaServer`
  （`/api/generate` チャンクシミュレート）
- **Live integration (opt-in)**: `cargo test --test lens_pipeline_live -- --ignored`
  — 実 ocrs モデル + 実 Ollama を叩く、Tauri 起動不要
- **Manual acceptance (release 前 gate)**: 実 Windows + 実 Ollama、人手 verify

### Coverage

- Rust: ≈80% 相当（168 + integration）、新規 lens pipeline は commit 前 75% 以上
- Svelte: 67 件（schema-filter 11 / result 13 / hotkey 11 / 各 component 5〜8）

### 新規テストの追加

- Rust unit: モジュール内 `#[cfg(test)]`、table-driven を推奨
- 統合: `src-tauri/tests/` に `*.rs` を置く
- Svelte: コンポーネントと同階層に `*.svelte.test.ts`、`@testing-library/svelte`
  の `render` + `fireEvent`

## 6. リリースフロー

[`RELEASING.md`](RELEASING.md) を参照。要点のみ:

| tag | 主な成果物 | セッション |
|---|---|---|
| `v0.1.0-beta.1` | WindowChrome + サイドバー + SchemaForm 改修 + auto-save | S1 |
| `v0.1.0-beta.2` | DXGI stream + `WDA_EXCLUDEFROMCAPTURE` + OCR engine 決定 | S2 |
| `v0.1.0-beta.3` | Lens 再設計本体（hover + in-place + streaming + session 履歴） | S3 |
| `v0.1.0-rc.1` | Privacy / Security / Onboarding / Perf tune / E2E 整備 | S4 |
| `v0.1.0` | Golden Path 6 シナリオ verify + bug fix + docs finalize | S5 |

`main` に直接 push せず、feature branch → PR → review → merge → tag の
流れ。CI は budget 節約のため Linux only。

## 7. プラグイン開発

[`plugin-sdk/docs/README.md`](../plugin-sdk/docs/README.md) を参照。
v0.1 の API version は `1-experimental`（互換性を破る変更の可能性あり）、
v1.0 で `1` へ昇格。

### 2 形態

| | WASM Component | Sidecar |
|---|---|---|
| ランタイム | `wasmtime` + Component Model (WIT) | 子プロセス + JSON-RPC 2.0 |
| 言語 | Rust / Go / TS(jco) / Python / MoonBit | 任意 |
| サンドボックス | WASI preview2 | OS レベル (v0.2 で AppContainer 等) |
| 配布 | `.wasm` + `plugin.toml` zip | 実行ファイル + セットアップスクリプト |
| 用途 | 純粋計算 | OS API / 重い ML / GPL 依存 |

### 拡張ポイント（v0.1 公開範囲）

`translator` / `ocr` / `asr` / `reverse_image_search` / `qr_handler` /
`intent_classifier` / `action` / `pre_processor` / `post_processor` /
`overlay_renderer` / `hotkey_handler` / `history_indexer`

v0.2 以降追加予定: `frame_stream_processor` / `change_detector` /
`lens_overlay_renderer` / `translate_stream`。

## 8. デバッグ

### `MOONLENS_DEBUG_DUMP_FRAMES=1`

環境変数を設定して起動すると、DXGI フレーム PNG 連番 + OCR JSON が
`$APPDATA/moonlens/debug/frames/` に dump される（S2 で DXGI 化後）。

### In-app debug overlay

`Ctrl+Alt+D` で toggle。fps / OCR 遅延 / 翻訳遅延 / fallback 発動回数を
live 表示。`settings.performance.show_debug_overlay = true` で永続化も可能。

### ログ

- `$APPDATA/moonlens/logs/error.log` に `tracing` の warning 以上が append
- `RUST_LOG=moonlens=debug npm run tauri:dev` で debug 出力
- `authorization` / `api_key` / `Bearer` は自動 redact

### 失敗時の再現しやすい手順

1. `cargo test --test lens_pipeline_live -- --ignored --nocapture` で OCR + 翻訳部分を
   Tauri 抜きで再現
2. `npm run dev` + `http://localhost:1420/<route>/` でルート単体の UI を確認
   （IPC は `try/catch` で degrade、ビジュアルのみ）
3. Tauri 実機確認は `npm run tauri:dev`、WebView DevTools は
   `Ctrl+Shift+I`（`capabilities/default.json` で許可済み）

## コントリビュート

[`CONTRIBUTING.md`](../CONTRIBUTING.md)（S0.5 繰延、近日整備）を参照。
PR は branch 命名 `claude/` or `feat/*` / `fix/*`、commit は
Conventional Commits + 日本語本文可。
