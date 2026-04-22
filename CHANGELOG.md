# Changelog

All notable changes to MoonLens are documented here. The format
follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and
the project adheres to [SemVer](https://semver.org/).

## Unreleased

### Added

- 常駐サイドバー (`AppSidebar.svelte`): 240px 幅の nav、collapse で
  48px アイコンのみ、現在ルートをアクセントバーでハイライト、
  localStorage に状態保存。
- 自作ウィンドウタイトルバー (`WindowChrome.svelte`): 32px 高さ、
  data-tauri-drag-region によるドラッグ移動、最小化/閉じるアイコン。
  `decorations: false` 設定のメインウィンドウで必須。
- `HotkeyInput.svelte`: JSON Schema の `format: "hotkey"` フィールドを
  record モードで入力可能にする。修飾キー+キーで accelerator 文字列化。
- `SaveIndicator.svelte`: 設定画面の保存状態 (未保存 / 保存中 / 保存済み /
  失敗) を右上に表示、保存失敗時はエラーメッセージを tooltip で提供。

### Changed

- 設定画面の「保存」ボタンを廃止、500ms debounce での**自動保存**に変更。
  右上に保存インジケータを常時表示 (plan.md Q13-B δ)。
- `SchemaForm.svelte` を全面改修: `$ref` 解決、`additionalProperties`
  (HashMap) の動的キー/値編集、セクションの `<details>` 折り畳み、
  `format: "hotkey"` widget hint サポート、未サポート型は JSON 羅列で
  なく警告表示に。
- `+layout.svelte` を WindowChrome + AppSidebar の常駐シェルに統合。
  `/overlay` `/subtitle` `/select` `/lens` `/lens-frame` の浮動ウィンドウは
  従来どおりシェル無しで描画。
- `AppHeader.svelte` の back ボタンを opt-in 化 (default: hidden)。
  サイドバーが永続ナビゲーションを担うため、サブルートには通常不要。
- `capabilities/default.json` に `core:window:allow-minimize` /
  `allow-close` を追加 — WindowChrome のボタンが Tauri IPC 経由で動作。
- ホーム画面 (`/`) をダッシュボード風にリデザイン: 重複していた nav
  グリッドを削除しサイドバーに一本化、キャプチャ/Lens のプライマリ
  アクションをアイコン付きカードで強調、ショートカット一覧を
  フッター代わりに配置。

### Tests

- `HotkeyInput.svelte.test.ts` / `SaveIndicator.svelte.test.ts` 新規
  追加、既存 `SchemaForm.svelte.test.ts` に `$ref` / unsupported /
  additionalProperties の 3 ケース追加。全 67 テスト green。

### Fixed (Windows OCR が効かない問題 — 2026-04-22 深夜)

- **user.toml が windows provider を登録しない問題を解消**: 既存の
  `settings::store::merge` は `*base = user` で user 設定を丸ごと上書き
  していたため、upgrade 後も user.toml に
  `[providers.configs.windows]` が無いと provider 未登録 → chain から
  skip → ocrs フォールバック → Latin gibberish、というループに陥って
  いた。`pipeline::rebuild_providers` 末尾で **Windows target かつ map に
  "windows" 不在なら自動登録** するように修正（config 未指定でも
  `language = "ja"` がデフォルト）。
- **Chain auto-prepend**: user.toml の `providers.ocr.chain` に
  "windows" が無くても、Windows 実行時は OCR 呼び出し直前に先頭
  prepend する。`pipeline::run` 内で cfg gated。
- **診断ログ強化**: `pipeline: Ocr served by 'X' in Yms` で実際に使われた
  provider が一目瞭然に。WinRT OCR 側でも:
  - 起動時に `AvailableRecognizerLanguages` をダンプ (インストール
    済み言語パックを列挙)
  - `IsLanguageSupported('ja')` を明示チェック、未対応時は warning
    ログで "言語パック未インストール" を示唆
  - run 開始/終了/失敗の info ログ
- **言語パック未対応の扱い改善**: `IsLanguageSupported` が false の時
  は profile default に fallback しない (英語パックで日本語を無理やり
  OCR してもゴミになるだけ) — LowConfidence で chain を抜けて ocrs
  に委ねる経路に統一。

### Added (S2 前倒し — Windows Media OCR 導入 — 2026-04-22 夜 2)

- **Windows Media OCR プロバイダ**: `windows` crate で
  `Windows.Media.Ocr` WinRT バインディングを実装。OS 標準の日本語 /
  中国語 / 韓国語言語パックを利用できるため、ocrs が生成していた
  Latin gibberish 問題が根本解決。
- **Chain 先頭化**: `providers.ocr.chain = ["windows", "ocrs", "paddle"]`。
  言語パックが未導入の場合は `LowConfidence` を返して ocrs にフォール
  スルー。ユーザーは `providers.configs.windows.language` で言語 tag
  (`ja` / `zh-Hans` / `ko` 等) を変更可能、デフォルトは `ja`。
- **OS gate**: `#[cfg(target_os = "windows")]` でモジュール・プロバイダ
  登録を分離。非 Windows ビルドでは `"windows"` id が未登録扱いとなり
  chain が自動で次候補にフォールスルー (`"not registered"` reason)。
- **plan.md S2 の OCR 差替タスク前倒し完了**。Windows のみ、macOS は
  v0.2 で Apple Vision、Linux は引き続き ocrs。
- **テスト**: `WindowsOcrProvider::new` の config parse、capabilities、
  preferred_language 反映の 3 件。**226 Rust + 67 vitest green**。

### Changed (OCR gibberish 検知強化 — 2026-04-22 夜)

- **英語 digram 密度 + 頻出語 stopword 検査を追加**: 従来の vowel ratio
  < 15% 単独チェックを通り抜けていた「見た目それっぽい Latin 文字列」
  (ocrs が CJK ソースから生成する典型パターン) を多重シグナルで検出。
  Latin 支配的テキストに対し:
  (1) vowel ratio off (< 20% or > 70%)、
  (2) English digram 密度 < 12% (top-50 digram 表で照合)、
  (3) `the`/`and`/UI 動詞等の stopword 不在 — **3 条件同時成立で reject**。
- **限界を正直に docs 化**: `al` / `hi` 等の common digram を含む
  borderline Latin (例: "aloo oki" 形式) は heuristic を通過する。
  完全対応は plan.md S2 の Windows OCR API / PaddleOCR 差替で達成予定。
  KNOWN_ISSUES.md に明記。
- **5 新規テスト**: 日本語 OCR 生成物を模したパターン、真の英語 / 技術
  用語の false-positive 防止、digram density scorer の sanity check。
  **Rust 223 / vitest 67 green**。

### Fixed (gradient 精度・日本語判定・OCR ノイズ — 2026-04-22 後半 3)

- **Multi-stop gradient + bbox 内部サンプリング**: 従来は rim 2 点のみで
  線形補間 → 非線形グラデや不均一な光源で実色と噛み合わなかった。
  vertical / horizontal 方向が選ばれた場合、**bbox 内部を 5 等分した
  帯でピクセル実サンプリング**し、glyph を local-bg 推定値 (rim の線形
  補間) で filter して除外。得られた 5 色を CSS multi-stop として送信
  (`linear-gradient(...deg, c0 0%, c1 25%, c2 50%, c3 75%, c4 100%)`)。
  対角 gradient は corner サンプルがそもそも位置依存なので 2 stop 維持。
- **Clean-linear-gradient 健全性チェック**: 写真 / 模様 / checker-board
  等の複雑背景で無理に gradient を emit すると「オリジナルと違う色で塗られた」
  不自然な見た目になる。線形グラデの性質 `tl+br ≈ tr+bl` を使い、
  対角和の距離が閾値を超えたら gradient を諦めて solid fill にフォールバック。
- **日本語判定の漏れ補修**:
  - **halfwidth katakana** (U+FF65-FF9F) と **CJK 句読点**
    (U+3000-303F, 「」、。…) を強 JA signal に追加
  - **単独漢字** (`戻` / `次` / `日` 等の UI ラベル) の skip を許可
    (従来は len>=2 要求で translate されていた)
  - "pure kanji + 0 Latin letters" を JA 判定に拡張
- **OCR ノイズ filter 強化**: **4 文字連続非 alnum run** を検知して reject。
  `----` / `::::` / `||||` 等の装飾 UI を ocrs が glyph 誤読した case を
  排除。3 文字の ellipsis (`...`) は通過。

**Rust 218 / vitest 67 green、+9 unit test 追加**。

### Fixed (段落翻訳の改行再発 + gradient 精度改善 — 2026-04-22 後半 2)

- **番号マーカー `[1]` `[2]` で段落翻訳を堅牢化**: 先の改修は system prompt
  に "Preserve line breaks exactly" と書いていたが、Gemma が実際に改行を
  落とすケースが残っていた。対策として LLM への入力を番号付き
  (`[1] 第一行\n[2] 第二行`) に変換、出力も regex でマーカーベースで
  parse するようにした。マーカーが半数以上残っていれば numbered path 成功、
  そうでなければ従来の `\n` split fallback。さらに fallback しても半数以上
  の slot が空なら **per-line 翻訳に automatic retry** (2× LLM call 覚悟で
  品質優先)。
- **Gradient を 4-corner + 斜め 2 方向対応**: 従来の 2 辺 (vertical /
  horizontal) だけでは斜めグラデーション (TL dark → BR light 等の典型的な
  UI 光源方向) を捕捉できず誤方向を選んでいた。4 コーナーの patch
  サンプリングを追加し、vertical / horizontal / ↘ (135°) / ↗ (45°) の
  **4 候補から最大 color delta の方向を採用**。CSS angle に変換して
  `linear-gradient` を emit。edge midpoint も中央 1/3 の範囲に narrower し
  長い bbox で gradient が平均化される問題を解消。
- **番号 parse / 斜め gradient / per-line retry の単体テスト 9 件追加**。
  **209 Rust + 67 vitest green**、svelte-check clean。

### Added (Lens 品質向上 — 2026-04-22 後半)

- **段落翻訳 → 改行 split → per-line 配分**: OCR 出力を保守閾値で
  段落化（縦ギャップ ≤ 0.5 × 行高、横 overlap ≥ 60%、両行 6 文字以上、
  末尾非句点）。段落全体を 1 回の LLM 呼び出しで翻訳し、system prompt の
  "Preserve line breaks exactly" を信頼して返答を `\n` で split、各 OCR 行の
  カードに配分。**レイアウトは 1 カード = 1 OCR 行を維持**しつつ、文脈ロスを
  解消（前行が "Very interesting." を含む場合など）。改行数が一致しない時の
  fallback: 多すぎたら末尾行に吸収、少なすぎたら先頭行にまとめて以降は空欄。
- **Gradient 背景 (Phase 1 inpainting 相当)**: 2 px rim の 4 辺を個別
  サンプリング、vertical / horizontal どちらの color delta が大きいかで
  `linear-gradient` 方向を決定。閾値未満なら従来通り solid。`LensOcrLine.bg_gradient`
  として optional で配信、フロントは `background: linear-gradient(<angle>deg,
  from, to)` で描画。グラデ背景や軽い写真上のテキストで境目が目立たなくなる。
  Phase 2（ONNX inpainter）は v0.2 plugin point として後回し。

### Changed (翻訳品質改善 — 2026-04-22)

- **OCR ノイズフィルタ** — `looks_like_ocr_noise(text)` 導入。3 文字未満
  / 句読点比率 > 60% / Latin 文字中の母音比率 < 15% を「ocrs が解読
  できなかった glyph の残骸」として drop する。カード自体を描画せず、
  LLM 呼び出しも発生させない。これまで CJK ソースで出ていた
  "bcdfghjk" → 謎翻訳ループを止める。
- **system_prompt を自然訳寄りに書き換え** — "translate faithfully" →
  "Render naturally and idiomatically … match the meaning, tone, and
  register a native reader would expect, not the word order of the
  source"。同時に user prompt も「Translate …」ではなく「Rewrite …
  naturally」に変更して Gemma の literal mode trigger を回避。
- **サンプリングパラメータ調整** — `temperature` 0.2 → 0.4、
  `top_p` 0.9 → 0.95。これまでの保守的な設定が直訳を強制していた。
  0.4 は semantic drift を起こさずに idiomatic rephrasing を許容する
  実用レンジ。settings.toml でユーザが上書き可。
- **noise filter 単体テスト 5 件 + sampling defaults テスト 1 件**
  追加。192/192 Rust + 67/67 vitest green。

### Fixed (Lens blink 根絶 — 2026-04-21 後半 3)

- **blink の真因**: OCR 毎に「main に opacity:0 を付与 → 60ms sleep →
  screen capture → opacity:1 に戻す」を繰り返していたため、カードが
  物理的に消失と復帰を繰り返していた。「頻度を落とす」ではなく
  「そもそもマスクしない」が正解。
- **Windows 解決**: `SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE)`
  を lens 窓に適用。xcap の screen capture は lens 窓を **「存在しないもの」**
  として扱い、原文がそのまま撮影される。カードはユーザーから見えたまま。
  マスク不要 → 100% blink 消滅。これは plan.md S2 の目玉機能だったが、
  Win 10 2004+ では API 呼び出し 1 行で実現できるので前倒し実装。
- **非 Windows (Linux) フォールバック**: mask 路は残存、sleep を 60→120ms に
  延長してフェード完了を確実に。`transition-opacity duration-75` も復活し、
  MVP の soft fade が効く。
- **段落グルーピング撤去**: 2026-04-21 に導入した paragraph merging を
  revert。union bbox へリスケールされ、短い翻訳がカード内で小さく浮く
  問題を解消。per-line 翻訳 (MVP 同等) に戻した。cross-line LLM 文脈は
  S3 の `translate_stream` plugin point で再検討。
- **text color を原文色サンプリング**: bbox 内部の「bg から Euclid 距離 60
  以上の pixel」の平均色を fg_color として採用。赤文字のソースは赤文字で
  描画される。bbox 内の 2% 未満しか「text pixel」がない場合は WCAG
  contrast fallback。
- **CSS を MVP 同等に restore**: `whitespace-nowrap`, `85cqh`, max `40px`,
  `line-height: 1`。段落対応で崩していたレイアウト判定ロジックを
  シンプルな 1 行 per カードに戻した。
- **auto-poll 維持**: user 要望通り内容変化の自動追従は継続。Windows では
  マスク不要なので 1500ms tick でも視覚的影響ゼロ。Linux では soft fade で
  pulse が緩和される。

### Fixed (Lens visual & capture 回帰修正 — 2026-04-21 後半 2)

- **無限再翻訳ループ** — `transition-opacity duration-75` 中に 32ms
  sleep で capture していたため、cards が半透明のまま hash 対象に
  混入し毎回 hash が違って "source changed" 誤検知。CSS transition を
  撤去して mask を instant 化、backend sleep を 60ms に延長。
- **Hash threshold 緩和** — ocrs のラスタライズ揺れ + フォント
  antialias で 6-10 ビット程度の jitter が実環境で観測される。
  threshold 8 → 20 に引き上げ、真の change（~32 bits 以上）は
  引き続き検知しつつ再翻訳の自走を停止。
- **blink** — 上記 transition 撤去でフェード in/out が消え、mask 期間
  のみ完全不可視 → 復帰が瞬時に。自動 poll 中でも視覚的に静か。
- **翻訳レイアウト** — `text-center` → `text-left`、font-size max
  40px → 22px、縦 budget 90cqh → 75cqh。短い翻訳が card 中央に
  ポツンと浮く / 原文より極端に大きくなる問題を解消。

### Fixed (Lens pipeline 緊急修正 — 2026-04-21 後半)

- **翻訳が出ない / 部分欠落** — Tauri 2 のチャンネル間イベント順序
  が保証されないため、`lens-flow-done` が最後の `done=true` chunk
  より先に届いて frontend が途中状態をコミットしてしまうケースが
  発生していた。Backend: タスク await 後に 80ms grace → `flow-done`。
  Frontend: atomic swap を廃止し **per-line commit** に変更 — 各行の
  `done=true` 到着時点でその行だけ `visibleLines` へ反映、`flow-done`
  は未完了行の強制 fallback + 旧レイアウトの枯葉カード剪定のみ。
- **段落グルーピングの過剰合併** — 本来別文脈の行を束ねて Gemma に
  渡してしまい翻訳が崩れる問題。閾値を保守化:
  縦ギャップ ≤ 0.5 × 行高 (was 0.8)、横 overlap ≥ 60% (was 30%)、
  両行 6 文字以上という 4 条件すべて満たす時のみ merge。短い UI
  ラベルや見出しは独立のままになる。
- **Gemma system prompt 改良** — "Translate faithfully… Preserve
  line breaks verbatim. Do NOT add commentary, explanations,
  romanizations, alternative translations, or surrounding quotes"
  を追加。不要な注釈や複数案併記を防止。

### Changed (Lens pipeline 品質改善 — 2026-04-21)

- **翻訳確定前の ちらつき廃止** — フロントエンドは chunk を
  `pendingLines` / `pendingChunks` にバッファし、`flow-done` イベントで
  `visibleLines` に atomic swap する。文字ごとタイピング風の
  partial 表示や空 placeholder の閃光が発生せず、前フローの結果が
  そのまま維持された状態から新フローの結果ブロックに一発で置換。
- **段落単位翻訳** — OCR 出力の隣接行を「縦方向ギャップ < 0.8 × 行高 +
  横方向 overlap > 30% + 末尾が句点/ピリオドでない」条件で段落に
  グルーピング。paragraph ごとに 1 回の Gemma 呼び出し + union bbox
  の 1 カード描画で文脈ロス（"very interesting." → "Indeed..." の
  代名詞参照等）を回避。
- **同言語スキップ** — `text_is_already_target(text, target)` で
  CJK / Latin heuristic による言語判定、target と一致すれば LLM 呼び
  出しを完全スキップして原文をそのまま表示（例: ANY → ja で OCR が
  日本語を拾った時）。カナ存在 / 全 CJK / ASCII 文字の 3 種類で
  ja / en / zh / ko をサポート。
- **text color 改善** — BT.601 luma (140 閾値) を廃止し、sRGB → 線形
  変換 → WCAG 相対輝度 (0.179 閾値) ベースに。中間色の背景でも
  可読性の高いコントラストを得る。
- **静止時の自動再翻訳** — `performance.lens_auto_poll_ms`
  (default 1500ms) を新設。Lens 窓が静止していても定期的に trigger
  が送出され、既存の content hash 比較で変化があれば再 OCR + 翻訳。
  動画字幕やチャットメッセージの流動的なテキストが追従される。
  `0` で periodic polling 無効 (event-driven のみ)。
- **font-fit 多行対応** — `--text-len` を longest sub-line のみに、
  `--line-count` を新設し `clamp(6px, min(90cqh/line_count/1.2,
  95cqw/longest_line), 40px)` に書き換え。段落カードが縦横両軸で
  自動収まる。

### Safety

- `stop_all` パニックボタン実装 — 全 Lens ウィンドウ閉鎖・字幕停止・
  Overlay 非表示・実行中 watchers の即時 abort を 1 コマンドに束ねた。
  `Ctrl+Alt+Escape` グローバルホットキー (`settings.hotkeys.stop_all`)、
  サイドバーフッターの赤色 OctagonX ボタン、コマンドパレット
  "全停止" エントリの 3 経路から呼び出し可能。実行結果は
  `StopAllReport` (lens_count / subtitle_was_running / overlay_was_open)
  として返却、`moonlens://stop-all` イベントでも全ウィンドウに配信。

### Docs

- `README.md` を MVP + S1 完了後の姿に reshape — プロジェクト
  ステータス表、platform matrix、docs 一覧テーブル、S2 以降の
  差替計画を明示。
- `docs/USAGE.md` 新規 — Ollama セットアップ、初回キャプチャ、
  Lens モード、ライブ字幕、キーバインド一覧、設定セクション
  ガイド、トラブルシュート、プライバシー解説。
- `docs/DEV.md` 新規 — 必須ツール、主要コマンド、ソースツリー
  注釈、アーキテクチャ概要、Lens streaming データフロー、
  テスト戦略、release ladder、プラグイン開発、デバッグ primitives。
- `docs/KNOWN_ISSUES.md` 新規 — v0.1 の既知制約をプラットフォーム
  / Lens / 設定 / プライバシー / ホットキー / ストレージ / テスト
  / ドキュメントでグループ化、各項目に解消予定タグを付与。

## v0.1.0-rc.1 — first public release candidate

First release-candidate build. All 0.1.0 ship-criteria features are
in and tested; this is the last chance to surface regressions before
`v0.1.0` is tagged on the same commit.

### Added

- Tag-driven release workflow (`.github/workflows/release.yml`) that
  produces `.dmg` / `.msi` / `.nsis` / `.AppImage` / `.deb` / `.rpm`
  bundles for every `v*` tag. `-rc` / `-beta` / `-alpha` suffixes
  publish as GitHub pre-releases automatically.
- `docs/RELEASING.md` documenting the end-to-end release flow.

### Changed

- `bundle.active` is `true` in the committed `tauri.conf.json` so
  `tauri build` produces installer artefacts on any machine — signing
  is optional (missing identities yield unsigned bundles that still
  run with an OS warning).
- Dropped the unused `network_allow` field from `WasmPlugin` /
  `WasmProvider`. Will be re-introduced when WASI p3 sockets stabilise.
- `README.md` synced with the actual implementation: removed the
  stale `tantivy` mention, called out Ollama streaming and Ed25519
  plugin signing.
- `plugin-sdk/examples/music-recognizer/README.md` rewritten from a
  placeholder stub into real install/build instructions.

## 0.1.0 — initial release target

Initial public target; see `docs/plan.md` for the architecture
overview. Ship criteria:

- ⌘⇧Space capture → OCR → Ollama translate → overlay, with intent
  detection (QR / text / photo / mixed) and intent-aware action
  ribbon.
- Lens Window mode with pinned region watcher (average-hash change
  detection + auto re-translate).
- Live subtitle pipeline (cpal loopback → whisper.cpp → translate →
  subtitle window) with configurable chunk/overlap seconds.
- Plugin system: WebAssembly Component Model and JSON-RPC sidecar
  plugins, Ed25519 signature verification, AudD music-recogniser
  example.
- Settings hot-reload via `notify` watcher; schemars-generated
  settings form; provider chains with per-task timeouts, fallback
  conditions, stickiness, and global `allow_cloud` gate.
- macOS permission onboarding with live Screen Recording +
  Accessibility detection via CoreGraphics / ApplicationServices.

Test coverage at 0.1.0 target:

- 163 Rust unit tests + 23 integration tests (sidecar RPC, plugin
  registry discovery, storage persistence, settings file
  round-trip, pipeline chain, WASM host load-time, WASM real `run`).
- 14 plugin SDK tests + 20 music recogniser tests.
- 49 Vitest (logic + Svelte component render).
- Clippy `-D warnings`, `cargo fmt --check`, `npm run check` all
  clean across Linux, macOS and Windows.
