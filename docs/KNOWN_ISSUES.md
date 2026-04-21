# Known Issues — v0.1

v0.1 系で把握済みの制約・既知不具合を一箇所にまとめたもの。release ladder
（`v0.1.0-beta.1` 〜 `v0.1.0`）の途中で解消する項目は "解消予定" 欄に
対象セッション / タグを記入しています。

## プラットフォーム / ディストリビューション

| 項目 | 現状 | 解消予定 |
|---|---|---|
| macOS サポート | **v0.1 スコープ外**。Apple Developer 登録 + notarization が未整備のため、ビルドは通ってもインストール時に警告で止まる | v0.2+ |
| Linux Lens ブリンク | Wayland / X11 では DXGI 相当がなく、`xcap` polling でハイド / ショーしている間 Lens 枠が瞬間的にブリンクする | v0.2+（Wayland portal の成熟待ち） |
| Windows SmartScreen 警告 | 未署名配布のため、初回起動時に「不明な発行元」警告が出る。ユーザが明示的に「詳細 → 実行」を選ぶ必要あり | v0.2 で自己署名、v1.0 で EV Code Signing |
| 自動更新 | 未実装。Release ページから手動 DL | v0.2 で Tauri updater plugin + signed manifest |
| CSP | `tauri.conf.json` で `"csp": null`（dev 速度優先） | v0.2 で `default-src 'self' tauri://localhost; connect-src 'self' http://127.0.0.1:*` |

## Lens streaming / OCR / 翻訳

| 項目 | 現状 | 解消予定 |
|---|---|---|
| 速度 SLO | E2E small = 684ms (P50) — category **beta** 判定。α (<500ms) には DXGI + Windows OCR 差替が必要 | S2 `v0.1.0-beta.2` |
| DXGI Desktop Duplication | 未導入。現状は `xcap` 単発キャプチャ | S2 |
| `WDA_EXCLUDEFROMCAPTURE` | 未導入。自己ウィンドウを capture から除外できないため、S2 まで Lens にはハイド / ショーが必要 | S2 |
| OCR エンジン | `ocrs` は英数に強いが CJK が弱い。日本語ドキュメントの認識精度は限定的 | S2 で Windows OCR API (`Windows.Media.Ocr`) 差替候補を評価、ラストリゾート PaddleOCR サイドカー |
| Font auto-shrink / 2 行折返し / ellipsis | MVP では `clamp(7px, min(85cqh, 90cqw/len), 40px)` で簡易 shrink のみ | S3 |
| WCAG コントラスト最適化 | luma 140 閾値で黒 / 白 flip の簡易ロジック | S3 で自動最適化 |
| 段落まとまり翻訳 | 行単位独立翻訳のため、文が行をまたぐと文脈がロス（例: "very interesting." が行頭で分断されると "interesting." が単独文扱い） | S3 |
| Cloud fallback | 行単位並列 per-line 翻訳の cloud fallback 未実装、現状は Ollama 直叩きのみ | S3 |
| Click-through + hover toolbar | per-pixel hit-testing が未実装、現状は hover-only header で代替 | S3 |
| Lens 台数制限 | `settings.performance.max_active_lenses = 4` で guard 済だが、超過時のトーストが簡素 | S3 |
| Lens idle 停止 | `settings.performance.lens_idle_timeout_min = 10` で指定可能だが、cursor hover での再開 UI が未整備 | S3 |

## 設定画面

| 項目 | 現状 | 解消予定 |
|---|---|---|
| "既定に戻す" の挙動 | セッション開始時の snapshot に戻るだけ。真のスキーマデフォルトにリセットする backend API 未実装 | v0.1.0 までに backend `default_settings` コマンド追加、それと合わせて 3 粒度（field / section / 全体）対応 |
| 変更フィールドの bullet (`•`) ハイライト | plan.md の仕様（保存前のドットマーカー）は S1 で一旦省略。SaveIndicator のみ | v0.1.0-beta.2 |
| Diff modal | "Diff" ボタン（変更一覧 modal）は v0.2 予定 | v0.2 |
| 外部編集コンフリクト検出 | `config.toml` を外部エディタで変更された場合の再読み込み dialog が未実装 | v0.1.0-beta.3 |
| Raw TOML エディタ (Monaco) | 未実装 | v0.2 |

## プライバシー / セキュリティ

| 項目 | 現状 | 解消予定 |
|---|---|---|
| API キー保存 | `api_key_env` (環境変数参照) のみ。OS keychain / AES-GCM fallback が未実装 | S4 `v0.1.0-rc.1` |
| Plugin 署名 | Ed25519 検証は実装済みだが、UI への警告表示がまだ簡素。"未署名プラグインをインストールしてもいいか？" 確認 dialog が弱い | S4 |
| Plugin sandbox | WASM は WASI preview2 の自然な sandbox、Sidecar は "ユーザへの情報提供" のみ。AppContainer 等の runtime enforcement は v0.2 以降 | v0.2+ |
| Rate limit / daily budget | 設定フィールドは存在するが、enforcement ロジックが簡略（toast 表示のみ、hard block は未実装） | S4 |

## ホットキー

| 項目 | 現状 | 解消予定 |
|---|---|---|
| HotkeyInput widget | S1 で実装済 — ただし競合検出（同一 accelerator の重複割当）は schemars 側ではなく保存時チェックに依存 | v0.1.0-beta.2 |
| `stop_all` panic button | ✅ 実装済、`Ctrl+Alt+Escape` + サイドバー赤ボタン + コマンドパレット。内部は Lens watchers `stop_all()` + 全 Lens close + subtitle stop + overlay hide | - |
| Linux Wayland での global-hotkey | 動作はするが、compositor によっては一部 accelerator が取れないケース | v0.2+ |

## 履歴 / ストレージ

| 項目 | 現状 | 解消予定 |
|---|---|---|
| Session 単位集約 | One-shot は 1 件 1 履歴、Lens session は 1 件集約で時系列チャンク保持 — 実装済 | - |
| tantivy 全文検索 | plan.md では採用予定だが、v0.1 実装は `LIKE` クエリの simple text search に留まる | v0.1.0-rc.1 |
| Export / Import | JSON Lines + PNG zip での export は未実装 | v0.2 |
| 削除 undo | 5 秒 undo toast は未実装 | v0.1.0-beta.3 |

## テスト / CI

| 項目 | 現状 | 解消予定 |
|---|---|---|
| Visual regression | Playwright 15 shot の CI 統合は未整備。ローカル `gstack` / `browser-use` での手動確認のみ | S4 |
| E2E | `axe-core` + Playwright による AA コントラスト検証は未実装 | S4 |
| Criterion benchmark | 手動実行で `docs/benchmarks/` に記録、CI 統合は scope 外 | - |
| Windows / macOS CI | CI budget 節約のため Linux のみ、Win / mac は local build | v0.2+ で self-hosted runner 検討 |

## ドキュメント / オンボーディング

| 項目 | 現状 | 解消予定 |
|---|---|---|
| Onboarding wizard | `MacOnboarding.svelte` が現役、汎用的な `Onboarding.svelte` への rename + 5 ステップ化は未着手 | S4 |
| CONTRIBUTING.md | 未作成 | S0.5 繰延、次セッションで整備 |
| `.github/ISSUE_TEMPLATE/` | 未作成 | S0.5 繰延 |
| i18n | 日本語 hardcode、`src/i18n/` 基盤は v0.2 | v0.2 |
| Plugin SDK README | `plugin-sdk/docs/README.md` あり、v0.1 版の整備は進行中 | S4 |

## その他 / 未決

- Lens window 位置メモリ（閉じて再度開いた時の復元）
- マルチモニタで Lens が跨る時の挙動
- DPI スケーリングの edge case
- RTL 言語サポート
- 詳細なスクリーンリーダー対応

これらは該当機能の実装タイミングで個別議論する（[`plan.md`](plan.md)
§ 未決 topic 参照）。

## 解消済み（参考）

- ✅ **Lens 32ms blink** — Event-driven capture + card mask で解消（2026-04-21）
- ✅ **Gemma 4 thinking mode 10× first-token 遅延** — `think: false` デフォルト化で解消
- ✅ **Hotkey 登録 panic** — best-effort 登録 + `HotkeyRegistrationReport` で解消
- ✅ **selector 真っ黒** — `capabilities/default.json` 整備で解消
- ✅ **SchemaForm の JSON 羅列表示** — S1 で $ref 解決 + 警告表示に整理
- ✅ **Window 移動不可** — `WindowChrome.svelte` + `data-tauri-drag-region` で解消（S1）
