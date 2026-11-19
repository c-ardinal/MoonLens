# changelog.d — 変更履歴の断片ファイル

`CHANGELOG.md` の直接編集は禁止。PR 単位で本ディレクトリに断片ファイルを追加し、
リリース時に `CHANGELOG.md` へ一括結合する。並行 PR 同士が同一行を編集しないため、
競合は発生しない。

## 断片ファイルの追加

```bash
node scripts/changelog.mjs new fixed lens-capture-exclusion
# → changelog.d/lens-capture-exclusion.fixed.md を生成、本文を記入
```

- ファイル名：`<slug>.<section>.md`
  - `slug`：英小文字・数字・`-` のみ。ブランチの主題など、他 PR と重複しない名称。
  - `section`：`added` / `changed` / `deprecated` / `removed` / `performance` / `fixed` / `security`
- 本文：`- ` 始まりの Markdown 箇条書き（英語、既存 CHANGELOG と同一の書式）。見出し不可。
  1 ファイルに複数項目も可。
- 利用者に見えない変更（リファクタリング、CI、テストのみ等）は断片不要。
- 既存断片の修正は同一ファイルの編集で対応（他 PR と競合しない限り自由）。

確認：`node scripts/changelog.mjs preview`（次回リリース分の出力）。

## CI

`.github/workflows/changelog.yml` が全 PR で以下を検査。

- 断片の名称・本文の書式
- `CHANGELOG.md` への項目の直接追加（断片削除を伴う結合 PR のみ許可）。
  旧版の誤記修正など意図的な編集は PR に `changelog-edit` ラベルを付与。

## リリース時

```bash
node scripts/changelog.mjs release v0.2.0-rc.1   # 断片を結合・削除
# CHANGELOG.md を確認・推敲後、コミットしてタグ付与
```

`scripts/check-release.mjs <tag>`（リリース CI の事前検査）は、該当版の節が
`CHANGELOG.md` に存在しない場合、または断片が残存する場合に失敗。
リリース下書きの本文には該当版の節を自動挿入。手順詳細は `docs/RELEASING.md`。
