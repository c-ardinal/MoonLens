# Changelog

All notable changes to MoonLens are documented here. The format
follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and
the project adheres to [SemVer](https://semver.org/).

## Unreleased

### Changed

- **組み込み LLM ランタイムを mistral.rs → llama-cpp-2 (FFI to llama.cpp) へ差し替え**
  (`Cargo.toml`, `src-tauri/src/providers/translate_embedded_llm.rs`,
  `src-tauri/src/services/translation_engine.rs`,
  `docs/EMBEDDED_LLM_PERF.md`)。
  実機ベンチで mistral.rs CPU パスが期待値の 30% 未満 (Compact 0.5B
  Q4 で 5–15 tok/s 観測 vs 50–100 tok/s 期待) と確認されたため、
  llama.cpp の SIMD 最適化 (AVX2 / AVX-512 / Neon / Q4_K_M 量子化
  カーネル) に切り替えて 2〜5 倍の高速化を狙う。Provider trait 表面
  ・ChatML 構築・チェーン dispatch・migration ロジックは無変更で、
  推論ループのみ差し替え。
    - `embedded-llm` feature の中身が `dep:llama-cpp-2` に変更
    - 旧 mistral.rs ランタイムは `embedded-llm-mistralrs` feature の
      裏に保持 (A/B 比較・特殊ビルド向け)
    - ChatML プロンプトを Qwen 2.5 互換形式 (`<|im_start|>` /
      `<|im_end|>` / `assistant\n` プライマー) でプロジェクト側から
      手組み。GGUF メタデータの chat template 不在/破損に依存しない。
    - 推論は `tokio::task::spawn_blocking` で実行し executor を
      ブロックしない。
    - スレッド数は `available_parallelism().min(8)` で自動決定。
    - 単体テスト追加 4 件 (`build_user_prompt_*`, `render_chatml_*`)。
      全 380 件パス (差し替え前 376 件 + 新規 4)。
  - **ビルド要件追加 (開発者のみ・エンドユーザーには影響なし)**:
    Windows: Visual Studio 2022 Build Tools + LLVM/libclang + CMake、
    Linux: gcc/clang + cmake、macOS: Xcode CLI Tools。
    `cargo check --no-default-features` で C++ ビルドを skip 可能。

### Fixed

- **Provider id rename: `embedded.gemma` → `embedded.llm`**
  (model_registry / settings / IPC / 全 frontend / `default.toml`)。
  Gemma → Qwen 2.5 へモデルを切替えても旧名のままだった命名 misnomer
  を解消。型名も `EmbeddedGemmaTranslator` → `EmbeddedLlmTranslator`、
  ファイル名も `translate_embedded_gemma.rs` → `translate_embedded_llm.rs`。
  起動時 migration (`migrate_legacy_embedded_settings`) が:
    - `providers.translate.chain` / `providers.explain.chain` /
      `providers.summarize.chain` 内の `"embedded.gemma"` →
      `"embedded.llm"` へ書換
    - `providers.configs."embedded.gemma"` → `providers.configs."embedded.llm"`
      へ移動
    - 続けて 9.x の synthetic-filename 修正も適用
  既存ユーザーは次回起動で自動修復、再選択不要。テスト 2 件 (rename
  + idempotency) 追加。
- **モデルロードを起動時に preload** (`EmbeddedLlmTranslator::preload`):
  `Provider::preload` の noop default に放置されていた為、初回翻訳
  呼び出し時まで `mistralrs::GgufModelBuilder::build` が遅延し
  「Lens を開いた瞬間に数分フリーズ」する致命的 UX 問題を修正。
  `Pipeline::preload_all` で OCR と並列にバックグラウンドロードが走り、
  ユーザーが設定画面・ホーム画面を見ている間にロードが完了する。
  ダウンロード成功直後・ティア切替直後・パッケージ取込直後にも
  preload を再トリガー。テスト追加。
- **`embedded-llm` cargo feature を default に格上げ** (Cargo.toml):
  通常の `npm run tauri:dev` / `tauri:build` で生成したバイナリに
  mistral.rs が含まれず、すべての翻訳呼び出しが
  「embedded-llm feature disabled at compile time」で失敗していた
  問題を修正。CI 用の `cargo check --no-default-features` パスは
  維持。エラー文言も具体的な復旧手順 (rebuild コマンド) を含むように改善。
- **multi-file GGUF 対応** (`model_registry`, `services::*`,
  `embedded_model_package`): Qwen 2.5 7B Q4_K_M は HF 上で 2 分割
  保存されており、pre-9.x の単一ファイル前提だと URL 組立て段階で
  404 を引き起こしていた。`ModelTier::filenames: &[&str]` に変更し
  全パートを順次 DL + 検証。multi-part パッケージング/取込にも
  対応 (manifest に `parts: Vec<PackageManifestPart>` 追加、v1 形式
  との後方互換あり)。
- **legacy 設定の自動移行** (`model_registry::migrate_legacy_embedded_config`):
  pre-9.x で記録された synthetic な単一ファイル名 (実在しない
  `qwen2.5-7b-instruct-q4_k_m.gguf`) を、起動時に登録ティアの
  multi-part 構成へ書き換え。ユーザーが手動で再選択しなくても
  次回起動で復旧。

### Added

- **DI リファクタ — services/ モジュール新設**:
  - `services::ResolvedConfig` (DRY): config解決のスニペットが5箇所
    重複していたのを単一関数に集約。`filename_remote` 上書き時に
    `model_path` フォールバックが古いファイル名を指していたバグも修正
    (test 6 件)。
  - `services::HardwareProbe` trait + `SysinfoProbe` (production) +
    `FixedProbe` (test): 「2 GB-RAM PC で推奨ティアは何か」が実機なし
    で検証可能に (test 3 件)。
  - `services::ModelStorage` trait + `FileSystemStorage` (production) +
    `InMemoryStorage` (test): `.partial` 経由の atomic rename / path
    separator 拒否 / discard クリーンアップを単体テスト化 (test 8 件)。
  - `services::ModelDownloader` trait + `HttpModelDownloader`
    (production) + `ScriptedDownloader` (test): mid-stream 失敗 /
    HTTP 4xx / 進捗イベント発火を mock で検証可能 (test 5 件)。
  - `services::TranslationEngine` trait + `MistralRsEngine`
    (production, feature-gated) + `ScriptedEngine` (test): 実 GGUF
    なしで翻訳パスをテスト可能。リトライ recovery / chunk 分割 /
    ロード失敗の典型ケースをカバー (test 5 件)。
  - `services::ModelService<D, S, H>`: 全 adapter を集約。
    download/package/import/tier_status/set_tier の業務ロジックを
    保持。新規 mock-based E2E test 9 件 (fresh install → DL →
    status flips の完全フロー含む)。
- **AppState に `models: Arc<ModelService<...>>` を追加**: 起動時に
  production adapters で初期化。IPC コマンドは ModelService への
  薄ラッパ (5-15行/cmd) に縮小。

### Changed

- **IPC コマンドを ModelService 委譲に書き換え**:
  `embedded_model_status` / `download_embedded_model` /
  `package_embedded_model` / `apply_embedded_model_package` /
  `model_tier_status` / `set_model_tier` の6本がサービス委譲に
  移行。総行数約500行 → 約100行に圧縮。Tauri 直叩き (reqwest /
  tokio::fs / sysinfo) を含む業務ロジックは services/ 配下に集約。
- **テスト総数 324 件 → 360 件**: services/ サブモジュールで
  36 件追加、すべて mock-based・実 IO/Network/HW非依存。

- **モデル tier registry** (`src-tauri/src/model_registry.rs`):
  Compact (Qwen 2.5 0.5B, 約400MB, RAM≥4GB) / Standard (Qwen 2.5
  1.5B, 約1.1GB, RAM≥8GB) / Quality (Qwen 2.5 7B, 約4.7GB, RAM≥16GB)
  の 3 ティアを Apache 2.0 縛りで定義。`tier_by_id` /
  `tier_by_coord` / `recommended_tier_for_ram` / `default_tier`
  を提供。9 unit tests (license/uniqueness/order を全て検証)。
- **HW検出 + 推奨ティア** (`sysinfo` crate): `model_tier_status`
  IPC が現在ティア+推奨ティア+RAM/CPUコア数を返す。Onboarding
  モーダルが推奨ティアを表示し、初回起動時に低スペック検出時は
  自動的に Compact ティアへ切替。
- **ティア選択 UI**: 設定 → プロバイダ → ローカル LLM カードに
  ドロップダウン追加。⭐ で推奨ティアを示し、選択で
  `repo`/`filename_remote` が user.toml へ反映。`set_model_tier`
  は DL を強制せず、ユーザがダウンロードボタンで明示的に取得。
- **設計ドキュメント** `docs/MODEL_RUNTIMES.md`: ONNX/encoder-decoder
  ランタイム (Helsinki Opus-MT, MADLAD-400) の設計と実装スコープ
  (~2-3 週間相当) を記述。`embedded.opusmt.<src>-<tgt>` provider id
  を予約。実装は v0.2 以降。

- **モデルパッケージ化機能** (`embedded_model_package.rs`):
  `.moonlens-model` ファイル形式 (zip + manifest.json + sha256) で
  ダウンロード済みモデルをエクスポート/インポート可能に。1人が外部
  ネットでDL → ファイルサーバ経由で制限環境のPCへ展開、という運用を
  サポート。新IPC: `package_embedded_model`,
  `inspect_embedded_model_package`, `apply_embedded_model_package`,
  `pick_and_package_embedded_model`, `pick_and_apply_embedded_model_package`。
  オンボーディングモーダル + 設定画面に「パッケージ化」「パッケージ
  から取込」ボタン。sha256 検証で破損/改竄パッケージは旧モデルを
  上書きせず取込失敗。
- **ライセンス情報の UI 表示**: `EmbeddedModelStatus` に `license` /
  `license_url` / `model_id` を追加。設定画面でライセンス種別を確認
  できるようにし、企業導入のライセンス監査に対応。

- **スタンドアロン化: 組み込みローカル LLM プロバイダ `embedded.llm`**
  (`src-tauri/src/providers/translate_embedded_llm.rs`)。`mistral.rs`
  経由で Gemma の GGUF をアプリ内で読み込んで翻訳する。Ollama などの
  外部デーモンに依存しない。`embedded-llm` cargo feature で有効化。
- IPC `embedded_model_status` / `download_embedded_model` (+ 進捗イベント
  `moonlens://embedded-model-progress`): 設定画面 → プロバイダ にダウンロード
  ボタン + 進捗バー + 「準備済み」バッジを追加。HuggingFace から
  `<app-data>/moonlens/models/<file>.gguf` へ直接書き込み、`.partial` 経由で
  クラッシュ時の半端ファイルを残さない。
- `Provider` trait に `run_streaming` を追加。`OllamaTranslator` /
  `EmbeddedLlmTranslator` がネイティブストリームを実装、それ以外の
  プロバイダは default impl で final Text を 1 chunk として emit。
- `Pipeline::run_with_sink`: 旧 `Pipeline::run` の per-call sink 版。
  Lens の per-line streaming がチェーン経由で動くようになり、
  プロバイダ選択が embedded / Ollama / cloud / plugin で透過。
- 設定画面 → プロバイダ → サジェスト一覧の先頭に `embedded.llm` を追加、
  「ローカルのみ」プリセットがそれを優先する。

### Changed

- **デフォルトモデルを Qwen 2.5 1.5B-Instruct (Apache 2.0) に切替**:
  Gemma 3 1B (Gemma Terms of Use) → Qwen 2.5 1.5B (Apache 2.0)。
  企業向けの再配布ライセンス汚染を回避。アーキテクチャは decoder-only
  のままで `mistral.rs` ランタイムは変更なし。サンプリングを greedy
  (`temperature=0.0`) に変更し低スペックPCでの first-token を高速化。
  デフォルトサイズは ~770MB → ~1.1GB に増加(品質はQwen 2.5の方が日本語
  翻訳で勝るため許容)。
- **rebuild_providers でロード済みエンジンを温存**: `Pipeline` に
  `embedded_cache` を追加。設定保存(rebuild_providers発火)のたびに
  数百MBのGGUFが捨てられて再ロードされる挙動を撲滅。`model_path`
  変更時のみ eviction + reload。
- **モデルロード進捗イベント** `moonlens://embedded-model-loading`:
  `mistralrs::Model::build` の前後で発火、layout shell に
  「ローカル LLM をロード中…」バッジを表示。モデル切替時の沈黙時間が
  可視化される。
- **初回起動オンボーディングモーダル** `EmbeddedModelOnboarding.svelte`:
  チェーンに `embedded.*` あり + GGUF 未配置 + 未提示の3条件で初回起動
  時に表示。「ダウンロード」「パッケージから取込」「後で」の3択。
  `OnboardingStatus.needs_embedded_model_setup` で判定。

- **`lens_watcher.rs` の Ollama ハードコード解除**: Lens 翻訳 3 経路
  (`run_capture_loop` / `translate_paragraph_streaming` /
  `translate_line_streaming`) が `OllamaTranslator::new("ollama.gemma4", ...)`
  を直叩きしていた箇所を削除し、すべて `Pipeline::run_with_sink` 経由
  にした。`providers.translate.chain` の設定がついに Lens にも効く。
- `OllamaLensTranslator` を `ChainBackedLensTranslator` に rename。中身を
  Pipeline 経由のチェーン解決に置き換え、構築時に `Arc<Pipeline>` を取る。
- `default.toml` の `[providers.translate]` chain を `[]` から
  `["embedded.llm"]` に変更。timeout_ms を 3000 → 30000 に拡大
  (cold-start 数秒 + paragraph-block 翻訳の現実的な所要時間に合わせた)。
- README.md: 「Ollama 必須」を撤回し、組み込み Gemma が標準になった旨に更新。
  Ollama / cloud は optional fallback として位置づけ。

#### (これ以降は過去の `Unreleased` バッチ — リリース時にまとめて流す)

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

### Fixed (キャプチャ画面が真っ暗 / Esc キャンセル不能 — 2026-04-23 昼 2)

- **画面が真っ暗になる**: `select/+page.svelte` の `:global(body)`
  透明化に `!important` が抜けていたため、`app.css` の
  `body { background: var(--ml-surface) }` (dark theme で `#0a0a0a`)
  が勝って全画面ほぼ黒に。lens page と同じパターン
  (`!important` 付与) で対処。
- **Esc がキャンセルとして機能しない**:
  - `core:window:allow-hide` / `allow-show` permission を capabilities に
    追加 — 従来は `window.hide()` が silently deny されていた可能性
  - cancel ハンドラを robust 化 (try/catch 別々、
    `capture: true` で bubbling 前に拾う、`preventDefault + stopPropagation`)
  - **backend-side `cancel_region_selection`** を呼ぶフォールバック経路追加。
    webview 側 `window.hide()` が何らかの理由で fail しても Rust 側が
    代わりに閉じる
  - Windows: `SetForegroundWindow` を selector::show 後に明示呼び出し
    (グローバルホットキー直後は Windows の foreground 窃取ルール的に
    allowed)。初回 Esc が source app に吸われる問題を回避
- **代替キャンセル経路を追加**:
  - 右クリック / 中クリックで即キャンセル
  - ヒントテキスト "ESC / 右クリックでキャンセル" に更新
  - 画面右上に**明示的な [× キャンセル (Esc)] ボタン**を常時表示、
    Esc が効かない環境でも UI 操作で抜けられる
- **検証**: Rust 230 / vitest 67 green, cargo check clean。

### Fixed (font-size が原文無視 / カード右上ずれ — 2026-04-23 昼)

- **Font-size を原文 bbox 基準に**: カードの `bbox` は tiling で vertical
  に拡張されるため、CSS `85cqh` (card height) を font size の基準に
  していたフォーミュラは原文のグリフ高を無視して過大になっていた。
  `LensOcrLine` に **`source_bbox`** (pre-tile 原文 bbox) を追加、
  frontend は `source_bbox[3] / imageH` を vh 単位で CSS 変数
  `--source-h-vh` として渡し、font-size 計算の第一優先に使用
  (`calc(--source-h-vh * 0.9 * 1vh)`)。card height と char-width は
  cap としてのみ残る。
- **カードの右上ずれ**: `capture_lens` が `outer_position` +
  `outer_size` を使っていたため、Windows 11 の `decorations: false` +
  `transparent: true` 窓に DWM が拡張する不可視 resize frame / shadow
  を含み、outer 座標で OCR したピクセルを inner サイズの viewport で
  positioning → 全カードが定数分 upper-right にずれていた。
  **`inner_position` + `inner_size`** に切替、webview client area と
  OCR 座標系を一致させて解消。outer と inner の差分が出たら debug ログ
  で記録。

**Rust 230 / vitest 67 green**。

### Fixed (日英混在行・skip 行領域被覆 — 2026-04-23 早朝 2)

- **混在 line の word-level 分割**: WinRT OCR から返される `OcrLine` は
  単一の line bbox しか持たなかったため、`Hello こんにちは world` の
  ような日英混在行は全体を 1 枚カードで覆ってしまい、日本語部分が
  カードに隠されていた。`ocr_windows.rs` で
  `OcrLine::Words()` の結果を **連続する同言語 run に分割**、各 run を
  独立 `OcrLine` として返すように変更。結果: 英語 run だけにカードが出て、
  日本語 run は filter で drop されて素通しで見える。
- **Tiling の skip 行 blocker 化**: skip された行 (target 言語で
  drop された行) を無視して直接の前後カード同士で midpoint 計算して
  いたため、間の日本語行領域もカードに覆われていた。skip 行を
  `ocr_lines` に残したまま `skip_flags` で記録、**tiling は全 OCR 行
  (skip 含む) を neighbor として参照**するように変更。skip 行の bbox が
  "blocker" となり、前後カードは skip 行の手前で midpoint を決める。
- **Paragraph grouping は非-skip subset に対して実行**: `rendered_lines`
  (skip 除外、元の順序を保持) を `group_paragraphs` に渡す。member index が
  `lens_lines.line_index` と一致するよう順序も sequential 化。
- **テスト**: `ocr_windows::classify_word` の 3 ケース (JA 文字列 / Latin /
  混在)。**230 Rust + 67 vitest green**。

### Fixed (target 言語の行も表示 / 英語が翻訳されない — 2026-04-23 早朝)

- **target 言語行は完全に非表示**: 従来は target 一致行でも original
  を emit してカードを描画していた。日本語 target 下で日本語原文に
  カードが載り、"翻訳" が元のまま表示される違和感あり。OCR 結果を
  paragraph 化前に filter、`text_is_already_target` が true の行は
  ocr_lines から drop する方式に変更 — **card も LLM 呼び出しも発生
  しない**、原文そのものが素通しで見える。
- **混在パラグラフの英語が翻訳されない regression 修正**:
  `text_is_already_target` が "any kana present" でただちに true を
  返していたため、日本語 1 行 + 英語 1 行がパラグラフに merge される
  と段落全体が skip され、英語部分も翻訳されなかった。判定を
  **ドミナンス比率ベース**に変更: `jp_chars / (jp_chars + latin_letters)
  >= 0.70` の場合のみ "already target"。純粋 JA (latin=0) や単独漢字
  ラベル (戻/次 等) は従来通り skip、`Hello こんにちは` のような
  50/50 mixed は translate にフォールスルー。
- **paragraph-level skip ロジックを削除**: line-level filter で
  全行が非-target になるため、paragraph レベルの skip 分岐は
  redundant。translate_paragraph_streaming へ直接流す経路のみに簡素化。
- **テスト 3 件追加**: `text_is_already_target_ja_dominant_mix_skips`
  ("長い日本語の文章OK" → skip)、`text_is_already_target_mixed_jp_and_latin_translates`
  に 3 ケース追加 (Hello こんにちは 50/50 → translate, Open the 窓 → translate)。
  **227 Rust + 67 vitest green**。

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
