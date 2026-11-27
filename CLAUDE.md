# MoonLens 開発規約

## ブランチ

- 開発 PR の宛先は `develop`。`release` はリリース専用、`main` は公開済み版。
- 手順詳細：`docs/RELEASING.md`、CI 方針：`docs/ci-and-release-policy.md`。

## 変更履歴（CHANGELOG）

- `CHANGELOG.md` の直接編集は禁止。利用者に見える変更は PR 単位で
  `changelog.d/<slug>.<section>.md` を追加（`node scripts/changelog.mjs new <section> <slug>`）。
- 書式・section 一覧：`changelog.d/README.md`。CI（`changelog.yml`）が検査。
- 結合はリリース時のみ（`node scripts/changelog.mjs release <tag>`）。

## 文体（日本語の出力全般）

- UI 文言・文書・コメント・PR 説明が対象。和語を避け、熟語とカタカナ語で簡潔に記述。
- 助詞・助動詞・接続詞を見直し、UI では体言止めを優先、「〜してください」は不使用。
- 例外：CHANGELOG・英語文書・LLM プロンプト・テストデータ。
- 単独の「窓」「面」は禁止（「ウィンドウ」「欄」「側」等を使用）。
- 検査：`npm run lint:copy`（CI で毎回実行。規則・例外の宣言：`scripts/copy-lint.mjs`）。

## 表示言語（言語ファイル）

- UI 文言は言語ファイル `locales/<言語コード>.json` で一覧管理（1 言語 1 ファイル、キー ID → 文言、
  符号位置順、画面側・Rust 側の共用）。日本語は `ja.json`（基準の言語、文体規則の対象）。
- ソースへ文言を直接記述せず、キー ID（`区分:名称`、例：`settings:display_language`）で参照。
  文言の追加・変更は全言語ファイルへ同一 PR で反映。
  - Svelte のマークアップ：`{t('common:save')}`、`title={t('common:close')}`。
  - 値の埋込み：文言中の `{n}` へ `t('history:count', { n })`。テンプレート文字列での連結は不可。
  - データ表（設定項目・選択肢等）：キー ID を保持し、描画時に `t(row.label)`。
  - 対象外の文字列（LLM プロンプト・判定用の文字集合）：直前行または同一行へ `i18n-ignore`。
    日本語固定の文言（LLM プロンプトへの埋込み等）は `sourceText('区分:名称')`。
- Rust：`tr!("ipc:read_failed", e = e)`（`src-tauri/src/i18n.rs`）。静的な文言は
  `i18n::Text::new("区分:名称")`・`i18n::text("区分:名称")`。
- 言語の追加：`locales/` へ言語ファイルを追加（手順：`locales/README.md`）。ソースの変更は不要。
- 検査：`npm run lint:i18n`（ソースへの日本語の直接記述、未登録・未使用のキー、全言語ファイルの過不足、
  `{name}` と `tr!` の引数。`--fix` で整列と未使用項目の削除）、
  `npm run audit:i18n`（英語表示の全ウィンドウに残存する日本語文言）、
  `npm run audit:overflow:en`（英語表示の画面超過）。CI で毎回実行。

## 製品方針

- 完全ローカル動作。翻訳・OCR の外部送信機能は追加しない。
