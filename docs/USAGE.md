# MoonLens ユーザガイド

このドキュメントは MoonLens をエンドユーザとして使うための操作ガイドです。
開発者向けのセットアップは [`DEV.md`](DEV.md)、プロジェクトの設計判断は
[`plan.md`](plan.md) を参照してください。

## 目次

- [1. セットアップ](#1-セットアップ)
- [2. 初回キャプチャ](#2-初回キャプチャ)
- [3. Lens モード](#3-lens-モード)
- [4. ライブ字幕](#4-ライブ字幕)
- [5. キーバインド一覧](#5-キーバインド一覧)
- [6. 設定項目](#6-設定項目)
- [7. トラブルシュート](#7-トラブルシュート)
- [8. プライバシーと権限](#8-プライバシーと権限)

## 1. セットアップ

### 1.1 Ollama を用意する

MoonLens はローカル LLM を第一級に扱うため、既定では [Ollama](https://ollama.com/)
経由で `gemma4:e2b` モデルを呼び出します。

```bash
# Ollama 本体をインストール（OS ごとの手順はオリジンサイトを参照）
ollama pull gemma4:e2b
ollama serve
```

`gemma4:e2b` を選んでいるのは翻訳向けの first-token レイテンシが
もっとも低いためです（詳細: [`docs/benchmarks/2026-04-20-speed-feasibility.md`](benchmarks/2026-04-20-speed-feasibility.md)）。
別のモデルを使う場合は設定画面 `providers.translate.ollama.gemma4.model` を
変更してください。

> ⚠️ **重要**: Gemma 4 系は reasoning モデルのため、`think: false` を
> 指定しないと first-token が 3.7 秒以上遅延します。MoonLens のデフォルト設定は
> この対策済みです。

### 1.2 OCR モデルをダウンロード

初回起動時、ホーム画面の「OCR モデル」セクションで **モデル取得** ボタンを押すと
ocrs 用 text-detection / text-recognition モデル（合計 ≈100 MB）が
`$APPDATA/moonlens/models/` にダウンロードされます。

### 1.3 （任意）クラウド翻訳のフォールバック

DeepL / Gemini をフォールバックに使う場合は、設定画面で API キー参照を
登録します。v0.1 では `api_key_env` による環境変数方式（`DEEPL_API_KEY`
など）が主流、v0.2 で OS keychain 連携に統合予定です。

クラウド利用ポリシー `privacy.lens.allow_cloud_mode` は既定で
`confirm_per_session` — Lens セッション起動時に毎回確認トーストが出ます。

## 2. 初回キャプチャ

1. ホットキー `Ctrl+Alt+Space` を押す（既定値）
2. 半透明の選択ウィンドウが画面全体を覆うので、翻訳したい矩形をドラッグ選択
3. オーバーレイウィンドウが前面に出て、OCR → ストリーミング翻訳の結果を表示

結果パネルの **アクションリボン** から原文 / 訳文コピー、画像検索、
LLM への追加説明要求、Lens 化（その領域を Lens ウィンドウに昇格）等を
ワンクリックで呼び出せます。

## 3. Lens モード

Lens ウィンドウは「ドラッグ可能な透明枠」をスクリーンの上に常駐させ、
枠内のテキストを継続的に OCR + 翻訳し続けるモードです。動画字幕や
ゲーム UI、読み進めるドキュメントに最適です。

### 3.1 起動と操作

| アクション | 操作 |
|---|---|
| Lens ウィンドウを開く | ホーム画面「Lens ウィンドウ」、またはコマンドパレットから |
| 枠の移動 | タイトルバー（hover で出現）をドラッグ |
| リサイズ | ウィンドウ端をドラッグ |
| 閉じる | タイトルバー右端の `×` |
| 強制再翻訳 | （S3 で追加予定） |

### 3.2 動作の仕組み

- 枠の移動 / リサイズをトリガに 1 回キャプチャ → OCR → 翻訳ストリーミング
- 行ごとに bbox を検出、背景色（2px 外周の平均 RGB）をサンプリングして
  カードを塗り潰し、Google Lens のような **in-place 置き換え** 体験
- 隣接行のカードは **中点** で接し、原文が覗かないように tile
- 背景輝度に応じて黒 / 白フォントを自動選択（WCAG AA 以上のコントラスト）
- 静止中は一切再計算しない（イベント駆動）ため、以前あったブリンクは消滅

### 3.3 既知の制限（S2/S3 で改善予定）

- 段落まとまり翻訳はまだ未実装（行単位独立翻訳のため、文が行を
  またぐと文脈ロス）
- フォントサイズは `clamp` ベースの簡易 shrink（S3 で 2 行折返し + ellipsis 対応）
- Click-through + hover toolbar は S3 で本格化

## 4. ライブ字幕

1. ホーム画面「ライブ字幕」セクション、または `Ctrl+Alt+S`
2. システム音声（ループバック）が whisper.cpp で書き起こし → 翻訳 → 画面下の
   半透明オーバーレイに表示

起動前に whisper.cpp サーバを立てておく必要があります:

```bash
# 例: whisper.cpp ビルド後
./build/bin/whisper-server -m models/ggml-large-v3-turbo.bin -p 9002
```

`settings.subtitle.*` で chunk 秒数 / overlap 秒数 / 言語 / フォントサイズが
調整できます。

## 5. キーバインド一覧

既定値（設定画面の `hotkeys.*` で変更可）:

| アクション | Keybind | スコープ |
|---|---|---|
| 領域キャプチャ | `Ctrl+Alt+Space` | Global |
| ウィンドウキャプチャ | `Ctrl+Alt+Enter` | Global |
| Lens ウィンドウを開く | `Ctrl+Alt+L` | Global |
| ライブ字幕トグル | `Ctrl+Alt+S` | Global |
| コマンドパレット | `Ctrl+Shift+P` (または `Ctrl+K`) | Global |
| 履歴 | `Ctrl+Shift+H` | Global |
| リバース画像検索 | `Ctrl+Alt+G` | Global |
| 全停止（panic） | `Ctrl+Alt+Escape` | Global |
| Lens edit mode toggle | `Alt+E` | Lens focus |
| オーバーレイを閉じる | `Escape` | Overlay focus |
| Lens を閉じる | `Alt+W` | Lens focus |
| Side panel 切替 | `Alt+P` | Lens focus |

設定画面では HotkeyInput widget に対してキー組み合わせを直接押すことで
再バインドできます。Esc でキャンセル、クリアボタンで未割当化。

## 6. 設定項目

設定画面（サイドバーの「設定」）は Rust 側の `schemars` 定義から **自動生成**
されるフォームです。主要セクション:

| セクション | 内容 |
|---|---|
| `appearance` | テーマ (light/dark/system), 密度, アクセントカラー |
| `providers.translate` | 翻訳プロバイダチェーン、timeout、allow_cloud |
| `providers.ocr` | OCR エンジン、言語ヒント、前処理 |
| `providers.asr` | Whisper モデル、VAD 感度 |
| `hotkeys` | グローバル・コンテクスト両方のホットキー |
| `privacy` | cloud 許可ポリシー、rate limit、blocklist |
| `storage` | 履歴件数上限、画像保持方針 |
| `performance` | 同時翻訳数、Lens 上限、アイドル停止 |
| `subtitle` | chunk 秒、overlap 秒、フォント、言語 |

保存は **自動**（500ms debounce）— 右上のインジケータが「未保存 → 保存中 → 保存済」を示します。
「既定に戻す」はそのセッションで開いた値に巻き戻します（全体リセットは v0.2 で拡張予定）。

## 7. トラブルシュート

| 症状 | 対処 |
|---|---|
| Lens で翻訳が来ない | Ollama が起動しているか確認 (`ollama serve`), `gemma4:e2b` が pull 済みか `ollama list` |
| 翻訳が極端に遅い | 設定で `providers.translate.ollama.gemma4.think = false` になっているか確認 |
| Lens 枠が真っ黒 | `capabilities/default.json` が存在するか再ビルド（開発者向け）、`Ctrl+Alt+Escape` で stop_all → 再起動 |
| ホットキーが反応しない | 他アプリとの競合。設定画面 `hotkeys` で別の組み合わせに再バインド |
| OCR が日本語を認識しない | `ocrs` は英数特化、v0.1 では CJK サポートは弱い。S2/S3 で PaddleOCR 差替検討 |
| SmartScreen 警告 (Windows) | v0.1 は未署名配布。README 記載の回避手順、または自己ビルド推奨 |

問題を報告する際は、設定画面の「問題を報告」（v0.1 簡易版）または
`$APPDATA/moonlens/logs/error.log` の末尾を
[GitHub Issues](https://github.com/cardinal-fan/MoonLens/issues) に添付してください。

## 8. プライバシーと権限

- 既定では全処理がローカル完結（Ollama + ocrs + whisper.cpp）
- クラウド送信は `privacy.lens.allow_cloud_mode` で明示的に許可した場合のみ
  — `never` / `confirm_per_session`（default） / `always` の 3 択
- `offline_mode = true` にすると全クラウドプロバイダを一括無効化
- `respect_wda_excludefromcapture` で、他アプリが `WDA_EXCLUDEFROMCAPTURE` を
  設定している window を Lens が覆った場合に自動一時停止
- 履歴 DB (`rusqlite`) は `$APPDATA/moonlens/moonlens.db` に保存、
  `storage.image_retention` で画像保持方針を制御（default: thumbnail のみ）
- ログは `$APPDATA/moonlens/logs/error.log`、30 日 rotate、API キーや
  Authorization ヘッダは `tracing-subscriber` の redact filter で自動マスク

詳細な設計判断は [`plan.md` § Privacy & Security](plan.md) を参照してください。
