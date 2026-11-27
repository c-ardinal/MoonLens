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

## 製品方針

- 完全ローカル動作。翻訳・OCR の外部送信機能は追加しない。
