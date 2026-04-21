# MoonLens — PC版 Google Lens風ツール（設計プラン）

> **本書のステータス**: 2026-04-20 に Q シリーズ（19 問）の意思決定を受けて全面改訂。v0.1.0 リリースまでのすべての設計判断はここに集約する。新規に発生した判断もここに追記すること。

> **新セッション開始者へ**: いきなり本書を読み始めず、**まず末尾「セッション引継ぎ protocol」を先に読む**こと。そこに preflight 手順、過去の失敗パターン、警告事項が集約されている。それから本「現在地」を読み、次のアクションに進むこと。

---

## 現在地（2026-04-21 時点、**S0.5 MVP 完了 / 意義確認レビュー済み**）

### ステータス: **In-place Lens overlay MVP がユーザ承認を得て成立**

**2026-04-21 のユーザ判定**: 「ひとまず MVP としてはこれで良しとする」。動作確認した機能:
- Lens 窓作成 + ドラッグ可（hover-only chrome）
- 英字段落への自動 OCR 検出 + 行ごと bbox 抽出
- 背景色サンプリング + コントラスト自動文字色
- 行ごと並列ストリーミング翻訳 (gemma4:e2b via Ollama)
- **イベント駆動 capture**（window 移動/リサイズ検出時のみ）→ 静止中は一切 blink 無し
- カードは隣接行の midpoint で tile、重複ゼロ
- ガラス風透過 + 枠なし

**2026-04-20 方針継続**: S0 の docs 整備（README/USAGE/DEV/KNOWN_ISSUES/TECH_CHANGES/CONTRIBUTING/ISSUE_TEMPLATE）は MVP 検証後に再開、商用化議論（embedded Gemma + freemium）は別フェーズで判断。

#### Session 0 / 0.5 / MVP todo 進捗

- [x] plan.md を Q1-Q19 決定で全面改訂
- [x] plan.md に「現在地」と「引継ぎ protocol」追加
- [x] **`plugin-sdk/` 実在確認** — v0.1 scope 維持
- [x] **Speed feasibility benchmark** (2026-04-20): `docs/benchmarks/2026-04-20-speed-feasibility.md`、category **beta**（続行 OK、S2 で DXGI + OCR 差替必須）
- [x] **`think: false` / `gemma4:e2b` デフォルト化** — Gemma 4 thinking mode 10× first-token 遅延問題修正
- [x] **MVP Lens streaming** — per-line bbox + streaming translation + in-place overlay 実装、ユーザ承認済 (2026-04-21)
- [x] **Event-driven capture** — polling 撤廃、window move/resize + position polling backup → blink 完全消失
- [x] **背景色サンプリング** — 2px rim mean RGB + コントラスト文字色自動選択で Lens らしい in-place 置換体験
- [x] **行間 midpoint tiling** — 隣接行のカードが overlap もせず source 漏れもせず厳密に接する
- [x] **Live 統合テスト** — `cargo test --test lens_pipeline_live -- --ignored` で tauri 起動不要の OCR+翻訳回帰チェック
- [x] **Dev build ML crate 最適化** — `[profile.dev.package.*]` opt-level=3 で tauri:dev でも OCR が release 速度
- [x] **S1 UI shell 実装** (2026-04-21 autonomous) — WindowChrome + AppSidebar + +layout 統合、SchemaForm 全面改修（$ref/additionalProperties/hotkey widget/警告表示）、設定画面自動保存 + SaveIndicator、HotkeyInput recorder widget、ホーム画面リデザイン。全 67 vitest green、svelte-check 0 error、vite build 成功、vite dev 8 route 全 200。
- [x] **`stop_all` panic button** (2026-04-21 autonomous) — Lens watchers 全停止 + 全 Lens window close + subtitle stop + overlay hide を 1 コマンドに集約。`Ctrl+Alt+Escape` hotkey + サイドバー赤ボタン + コマンドパレットの 3 経路、`StopAllReport` 返却 + `moonlens://stop-all` イベント配信。Rust 175 unit 全 pass、Hotkeys default に `stop_all` 追加（Q11-D γ 対応）。
- [x] **README 整備 + docs/USAGE.md / docs/DEV.md / docs/KNOWN_ISSUES.md** (2026-04-21 autonomous) — S0.5 繰延のうち 4 ドキュメント整備済
- [ ] docs/TECH_CHANGES.md / CONTRIBUTING.md / .github/ISSUE_TEMPLATE/ — 残存 S0.5 繰延
- [ ] `CHANGELOG.md` 更新（`v0.1.0-beta.1` 〜 `v0.1.0` の予定 entry） — Unreleased セクション更新済
- [ ] MVP 成果を分割 commit / push / PR — ユーザ承認時に実施

### 現在の branch / PR / CI

- **Current branch**: `claude/moonbit-lens-tool-2FkOu`
- **Open PRs**: なし（PR #1, #2 は既にマージ済み）
- **Main branches**: `main`（shipped）, `release`（pre-release tags）— 直接 push 禁止、PR 経由
- **CI budget**: 残 ~115 分 / 2000（Linux-only 方針で節約中、Q10-B γ）
- **Latest commit on branch**: `f31d690 perf(bench): speed feasibility harness + gemma4:e2b default + think:false`
- **前セッション用タグ案**（破棄）: `v0.1.0-rc.1` を直接打つ計画は Q10-E α で一度決めたが Q18-D β に上書き、beta ladder に変更

### 本セッション S1 未コミット変更（autonomous mode、2026-04-21 後半）

**Frontend (`src/`)**
- 🆕 `lib/components/WindowChrome.svelte` — 32px カスタムタイトルバー、`data-tauri-drag-region` + lucide `Minus`/`X` アイコン、最小化/閉じるボタン、赤 hover for close
- 🆕 `lib/components/AppSidebar.svelte` — 240px 常駐 nav（collapse 48px）、localStorage 記憶、`$page` stores で active route ハイライト
- 🆕 `lib/components/HotkeyInput.svelte` — record mode の accelerator 入力 widget、Ctrl/Alt/Shift/Meta 修飾キー必須、Esc でキャンセル、クリアボタン
- 🆕 `lib/components/SaveIndicator.svelte` — saved/dirty/saving/error の 4 状態、a11y `aria-live="polite"`
- 🆕 `lib/components/HotkeyInput.svelte.test.ts` / `SaveIndicator.svelte.test.ts` — 新規テスト計 10 件
- ✏️ `lib/components/SchemaForm.svelte` — `$ref` 解決、`additionalProperties` マップ編集、section `<details>` 折り畳み、`format: "hotkey"` → HotkeyInput、未サポート型は警告表示
- ✏️ `lib/components/SchemaForm.svelte.test.ts` — $ref / unsupported / map の 3 ケース追加
- ✏️ `lib/components/AppHeader.svelte` — `showBack` prop 追加（default false）、サイドバー時代に back ボタンを redundant 化しない
- ✏️ `routes/+layout.svelte` — WindowChrome + AppSidebar のシェル統合、`/overlay` `/subtitle` `/select` `/lens` `/lens-frame` は従来通り裸描画
- ✏️ `routes/+page.svelte` — ダッシュボード風リデザイン、重複 nav 削除、primary action をアイコンカードに、ショートカット一覧追加
- ✏️ `routes/settings/+page.svelte` — 保存ボタン廃止、500ms debounce で `setSettings()`、SaveIndicator 表示、既定に戻す（セッション snapshot restore）

**Rust / capabilities**
- ✏️ `src-tauri/capabilities/default.json` — `core:window:allow-minimize` / `core:window:allow-close` 追加

**Docs**
- ✏️ `CHANGELOG.md` — `## Unreleased` に S1 変更を記載
- ✏️ `docs/plan.md` — 本 section、Session 0.5 / S1 進捗、次 action 更新

### 本セッション S0.5 MVP 変更（commit 済: 32badda〜124bf3c）

**Rust (`src-tauri/`)**
- 🆕 `src/lens_watcher.rs` — event-driven per-Lens watcher（`Notify` trigger、position polling backup 250ms、card mask event、OCR bbox→tiling/color sampling）
- 🆕 `tests/lens_pipeline_live.rs` — live 統合テスト（OCR 模型 + Ollama 実叩き、`#[ignore]` デフォルト、`--ignored --nocapture` で実行）
- ✏️ `src/lens.rs` — `capture_lens` hide 撤去、window 常時表示
- ✏️ `src/lib.rs` — `pub mod lens_watcher` 追加
- ✏️ `src/app.rs` — `AppState.lens_watchers` / chunk_sink 単純化 / preload_all spawn / tray handler に event hook 追加
- ✏️ `src/ipc.rs` — `new_lens_window` で watcher + event hook 配線、`refresh_lens` IPC 追加
- ✏️ `src/pipeline.rs` — `preload_all` 追加
- ✏️ `src/providers/mod.rs` — `Provider::preload` trait method（default noop）
- ✏️ `src/providers/ocr_ocrs.rs` — `Arc<Mutex<Option<OcrEngine>>>` 共有 cache、`preload` 実装、AABB from RotatedRect、ログ拡充
- ✏️ `src/settings/default.toml` — OCR chain `timeout_ms` 5000 → 15000
- ✏️ `src/capabilities/default.json` — `core:window:allow-start-dragging` + `allow-set-ignore-cursor-events` 追加
- ✏️ `Cargo.toml` — `[profile.dev.package.{ocrs,rten,rten-imageproc,rten-tensor,rten-simd,rten-vecmath,image,png}]` opt-level=3（tauri:dev でも ML crate は release 速度）

**Frontend (`src/`)**
- ✏️ `lib/ipc.ts` — `LensOcrLine`（`bg_color`/`fg_color`/`bbox` 追加）、`LensLineChunk`、`LensOcrLines`、`LensFlowDone`、`LensMask`、`onLensOcrLines`/`onLensLineChunk`/`onLensFlowDone`/`onLensMask` ヘルパ
- ✏️ `routes/lens/+page.svelte` — 全面書換: position absolute bbox カード、背景色サンプリング反映、hover-only header、window drag via `startDragging()`、mask 同期、midpoint tiling 描画

**その他**
- 🆕 `tests/input/english0001.png` — ユーザ提供英文字幕 fixture（integration test 用）
- ✏️ `docs/plan.md` — 本 section 更新、次 action を commit + S0.5 docs 再開に改訂

**Session 0.5 当初変更（commit 済: `f31d690 perf(bench): speed feasibility harness ...`）**
- `src-tauri/examples/bench_speed.rs`, `docs/benchmarks/2026-04-20-speed-feasibility.md`, `src-tauri/tests/fixtures/bench/README.md`, `translate_ollama.rs` think:false / gemma4:e2b 対応, settings/default.toml 翻訳モデル更新

### 直近のユーザ指摘事項（過去 2 セッションの矯正点、必ず遵守）

1. **「テスト green」≠ 「user 動作 OK」** — `cargo test` / `npm test` 全 green でも UI 実動作は別物。**browseruse で目視確認しないと "使える" と言うな**
2. **UI 確認は browseruse-cli を使う** — Vite 単体起動 + `browser-use open http://localhost:1420/<route>/` + `screenshot` で各 route を確認。Tauri window 直接覗けなくてもこの手段でカバー可
3. **autonomous 勝手走行禁止** — 実装前に approach 提示 → 承認、UI/視覚変更は特に user が見える姿を具体的に説明
4. **spec 解釈で迷ったら `/grill-me`** — 小手先 symptoms patching は根本原因見逃しとスコープ増大の両害
5. **正直レポート** — 「正直に言うと〜」の前置き禁止、嘘・誇張禁止、gap は最初に明示
6. **速度目標**: Lens streaming は **<0.5s 目標 / <1.0s 最悪**（spec L337 の <1.5s を Q18 で上書き）

### 次の具体的 action — **MVP 承認後の手順**

**直近**（本セッション完了時）:

1. **MVP 成果を分割 commit** — 以下 4 本で割付け、ユーザ承認後に順次実施（push なし）:
   - E: `feat(lens): in-place streaming overlay with per-line bbox cards`
   - F: `perf(ocr): cache engine + bg preload + populate bboxes + bump timeout`
   - G: `test(lens): live integration harness + dev-build ML crate opt-level=3`
   - H: `docs(plan): S0.5 / MVP completion + S1 handoff`

**次セッション以降**（優先度順）:

2. **S0.5 繰延 docs の再開**: README reshape / USAGE / DEV / KNOWN_ISSUES / TECH_CHANGES / CONTRIBUTING / ISSUE_TEMPLATE / CHANGELOG
3. **S1 Sidebar + WindowChrome**: `WindowChrome.svelte` + `AppSidebar.svelte` + SchemaForm 改修 + 保存ボタン廃止
4. **S2 DXGI + Windows OCR API 差替**: MVP のカード mask 方式は残存 blink 解消済みだが、speed SLO <0.5s 達成には DXGI Desktop Duplication + `Windows.Media.Ocr` 差替えが必要（bench β 判定のルート、`Session 0.5` 「所見 3」対応）
5. **商用化議論の再開**: MVP 意義確認を踏まえて embedded Gemma + freemium + Paddle/LemonSqueezy + 日本国 EULA + 特商法表記、plan.md 新規 section で議論
7. **判断 = 撤退 or 再考 なら**:
   - plan.md に retrospective 追記
   - commit して branch 凍結

**S0 docs 整備（README/USAGE/DEV/KNOWN_ISSUES/TECH_CHANGES/CONTRIBUTING/ISSUE_TEMPLATE）は MVP 後に実施**。MVP が「意義確認」目的であり、docs が先行しても検証対象が無い。

### Session 1 の開始ゲート（S0 完了後、別 session で実施）

新しい Claude session が立ち上がったら:
- 本書の「引継ぎ protocol」preflight を実行
- 現在地 section の「Session 0 todo」が全 ✅ になっていることを確認
- 「Session 1 で開始する作業」（下記）に進む

### Session 1 の作業内容（参考、S0 完了後に開始）

目標 tag: `v0.1.0-beta.1` — ウィンドウクローム + サイドバー + 設定画面まともに動く

- 🆕 `src/lib/components/WindowChrome.svelte` — `data-tauri-drag-region` + 最小化 + 閉じる（spec L49）
- 🆕 `src/lib/components/AppSidebar.svelte` — 240px 常駐 nav、collapse で 48px アイコンのみ（Q17-A β）
- ✏️ `src/routes/+layout.svelte` — WindowChrome + Sidebar を統合、AppHeader の back ボタン削除
- ✏️ `src/lib/components/SchemaForm.svelte` — `$ref` 解決、`additionalProperties` 対応、section 折り畳み、未サポート型は警告表示（過去の失敗 #7 解消）
- 🆕 `src/lib/components/HotkeyInput.svelte` — chord 入力 recorder widget、`format: "hotkey"` hint 対応
- ✏️ `src/routes/settings/+page.svelte` — 保存ボタン廃止、自動保存 + 保存インジケータ（Q13-B δ）
- ✏️ `tailwind.config.ts` — design tokens mapping（Q17-C γ）
- 追加テスト: `WindowChrome.svelte.test.ts`, `AppSidebar.svelte.test.ts`, `SchemaForm.svelte.test.ts` 拡充
- 動作確認: `npm run dev` + browseruse で 5 route 全回帰スクショ + click + ESC 試験、`npm run tauri:dev` で実機ドラッグ・最小化動作確認
- Commit → push → PR → マージ → `v0.1.0-beta.1` tag

### 未決 / 保留事項

- ~~`plugin-sdk/` 実在状態~~ → **2026-04-20 確定**（存在、v0.1 scope 維持）
- ~~OCR エンジン決定~~ → **2026-04-20 ベンチ結果**: ocrs は beta 圏、S2 で **Windows OCR API (`Windows.Media.Ocr`)** 差替が第一候補（商用配布制限無し、~80-150ms 想定）、PaddleOCR はラストリゾート
- **MVP 残課題** (S2/S3 に継続):
  - 32ms blink → **解消済** (card mask + event-driven capture)
  - DXGI Desktop Duplication への移行 → **S2** (SLO <0.5s 達成に必要)
  - Click-through + hover toolbar per plan.md Q2 → **S3** (per-pixel hit-testing 要、現状は hover-only header で代替)
  - Font auto-shrink + 2-line wrap + ellipsis per Q3 → **S3** (現状は `clamp(7px, min(85cqh, 90cqw/len), 40px)` で簡易)
  - WCAG 自動コントラスト最適化 → **S3** (現状は luma 140 閾値で黒/白 flip)
  - per-line 並列翻訳の cloud fallback → **S3** (現状は Ollama 直叩き、fallback なし)
  - 段落まとまり翻訳 → **S3** (行ごと独立翻訳で文脈ロス、例: "interesting." が単独文扱い)
- **商用化 (embedded Gemma + freemium)** → MVP 意義確認レビュー通過済。具体化は別フェーズで:
  - embedded llama-cpp-2 + gemma4-e2b (A 案 installer bundle 約 7.2GB)
  - Paddle/LemonSqueezy MoR、14-day trial、日本国法準拠 EULA、特商法表記
- Lens window 位置メモリ、マルチモニタ跨ぎ、RTL 等の edge case → 該当機能実装時に個別議論
- OllamaTranslator で Ollama version 検出 → `think` パラメータは Ollama 0.9+ 必要、fallback 考慮

---

## Context（なぜこれを作るか）

PC上で外国語コンテンツ（ゲーム画面、Webの画像内文字、PDF、動画字幕の読めない部分、動画音声そのもの、QRコード）に遭遇したとき、用途ごとに別ツール（翻訳サイト、QRアプリ、Whisperスクリプト、画像検索）へ手動で渡すのが面倒。Google Lens + ライブ字幕 + リバース画像検索 を **1つのグローバルホットキー** に統合し、ローカルLLMによるプライバシー重視の翻訳を第一級に扱う常駐ツールが欲しい。

目指すのは **「スマホ Google Lens の PC 版」** — カメラの代わりに画面を「ストリーミング」で見続け、検出したテキストを**リアルタイムに**、**元の位置で**、**動的に**翻訳し続ける体験。

## スタックの結論

- **フルRust + Tauri 2**。MoonBitハイブリッド案はIPC性能・pre-1.0非互換リスク・FFI作業量でコストが見合わず不採用（プロジェクト名「MoonLens」はブランドとして維持）。
- **音楽認識は不採用**：Shazam非公式APIは規約グレー、ACRCloud等は有償で運用コストが合わない。抽象層のpluginポイントだけ残し、個人利用拡張可能な余地を確保。
- **UI 言語**: v0.1 は日本語 hardcode、v0.2 で i18n 基盤導入（Q15）。
- 既存OSS調査結論：残り全機能（OCR翻訳／画像検索／QR／動画字幕／ローカルLLM）をクロスプラットフォームで統合した実装は無く、MoonLensの存在意義あり。各サブ機能はRust crate / サイドカーを **composeする** 方針。

## v0.1 スコープと段階的リリース（Q10, Q18, Q19）

### Platform matrix

| OS | v0.1 対応 | 備考 |
|---|---|---|
| Windows | **Full support** | DXGI Desktop Duplication + `WDA_EXCLUDEFROMCAPTURE` で streaming 完全対応 |
| Linux | **Degraded** | DXGI 相当なし、`xcap` polling で動作（lens flicker あり）、残り機能は動く |
| macOS | **Skip in v0.1** | Apple Developer 登録 + notarization 未整備、v0.2 以降 |

### Release ladder

v0.1.0 に向けて 5 本の tag を段階的に切る:

| tag | 主な成果物 | 目安セッション |
|---|---|---|
| `v0.1.0-beta.1` | WindowChrome (drag + min/close) + サイドバー仮 + SchemaForm 全面改修 + capabilities 最小公倍数 | S1 |
| `v0.1.0-beta.2` | DXGI stream 実装 + `WDA_EXCLUDEFROMCAPTURE` + `ocrs` ベンチ + OCR engine 決定 | S2 |
| `v0.1.0-beta.3` | Lens 再設計（hover toolbar + in-place overlay + streaming + session 単位履歴） | S3 |
| `v0.1.0-rc.1` | Privacy policy / rate limit / ed25519 署名検証 / keyring API キー / onboarding wizard / Criterion benchmark / E2E 整備 | S4 |
| `v0.1.0` | Golden Path 6 シナリオ目視検証 + bug fix + docs finalize | S5 |

Beta 以降は手動配布、Windows native binary + Linux AppImage/.deb。自動更新は v0.2 以降。

### セッション 0（整地作業）

実装着手前に、plan.md 全面更新（本書） + README / USAGE / DEV 3 分割作成 + `docs/KNOWN_ISSUES.md` 新規 + `plugin-sdk/` 実在確認 を先行実施。実装コード変更なし。

## アーキテクチャ

```
┌──────────────────────────────────────────────────────────────┐
│ Tauri 2.x Shell                                               │
│  ├─ global-hotkey / tray-icon / window management            │
│  ├─ xcap (one-shot capture, マルチモニタ/HiDPI)               │
│  ├─ DXGI Desktop Duplication (Windows streaming only)        │
│  ├─ WDA_EXCLUDEFROMCAPTURE (Windows 自己除外)                 │
│  ├─ cpal (audio loopback: WASAPI/CoreAudio/PulseAudio)       │
│  └─ Webview (overlay + 設定画面 + 履歴 + サイドバー)           │
├──────────────────────────────────────────────────────────────┤
│ Rust Core (同一プロセス、IPC無し)                              │
│  ├─ Pipeline orchestrator (capture→detect→dispatch)          │
│  ├─ Lens streaming engine (stream→pixel diff→OCR→translate)  │
│  ├─ Provider trait群 (Ocr / Translator / Asr / Search)       │
│  ├─ Intent classifier (QR/テキスト/画像を自動判別)              │
│  ├─ Cache / history (rusqlite + tantivy)                     │
│  └─ reqwest で Ollama / 外部APIへ                             │
├──────────────────────────────────────────────────────────────┤
│ External / サイドカー                                          │
│  ├─ OCR: ocrs crate (pure Rust、v0.1 基本採用)                │
│  │      └─ ベンチ未達時は PaddleOCR サイドカー差替             │
│  ├─ QR: rqrr crate                                           │
│  ├─ Translate: Ollama(gemma4) + DeepL/Gemini APIフォールバック │
│  ├─ ASR: whisper.cpp server (サイドカー、明示起動)             │
│  └─ 画像検索: ブラウザ起動 or SerpAPI                          │
└──────────────────────────────────────────────────────────────┘
```

### Lens streaming データフロー（新規）

```
DXGI Duplication → downsample (GPU) → pixel hash (GPU→CPU 小データ)
                                             │
                                             ▼
                                     change detector
                                   (N フレーム連続安定で trigger)
                                             │
                                             ▼
                     ┌──────────────── OCR (per frame) ──────────────┐
                     │                                                │
                     ▼                                                │
          per-line bbox + text                                        │
                     │                                                │
           ┌─────────┴─────────┐                                      │
           ▼ diff against prev ▼                                      │
       unchanged lines    changed lines → parallel translate chain   │
           │                      │        (N=4 concurrent)           │
           ▼                      ▼                                   │
       reuse previous        streaming chunks                         │
       translation               │                                    │
           │                      ▼                                   │
           └──────────►  in-place overlay render                      │
                        (font-fit + shrink + fill)                    │
```

- **Windows**: DXGI + WDA — 自己ウィンドウを capture から除外、hide/show 不要（flicker 消滅）
- **Linux**: `xcap` polling + hide/show（degraded、flicker あり）
- **macOS (v0.2+)**: `NSWindow.sharingType = .none` + `CGDisplayStream`（未実装）

## UI 要件 — Modern / Simple / Refined（Q17）

参照点：**Raycast / Linear / Arc / Obsidian**。静かで高密度、キーボードファースト。

### レイアウト（Q17-A）
- **常駐サイドバー**：240px（collapse で 48px アイコンのみ）に「ホーム / 設定 / プラグイン / 履歴」nav を固定配置。`AppSidebar.svelte` を `+layout.svelte` に mount。
- **コンテンツペイン**：残り全幅、`max-w-*` で横幅絞らず、必要に応じて内部 grid。
- **背景**：大画面時の空白は subtle gradient で意匠化、空虚感を出さない。
- **WindowChrome**：上端 32px、全幅、`data-tauri-drag-region` ＋ 最小化 / 閉じるアイコン（`decorations: false` 準拠）。

### フロントエンド / スタイリング
- **フロントエンド**：Svelte 5（runes）+ Vite。軽量・起動速い・バンドル小。
- **スタイリング**：Tailwind CSS v4 + カスタムデザイントークン。shadcn丸出しにせず自作コンポーネント。
- **デザイントークン（Q17-C）**: `tailwind.config.ts` で `theme.extend` に mapping。主な token:
  - Color: `ml-ink` (#0A0A0A), `ml-ink-muted`, `ml-ink-faint`, `ml-surface` (#FAFAFA / #0A0A0A), `ml-surface-elevated`, `ml-surface-muted`, `ml-border`, `ml-accent` (violet-500), `ml-accent-soft`, `ml-danger`, `ml-warning`, `ml-success`
  - Spacing: Tailwind allowlist 化、`ml-pad-{sm,md,lg}` = 12/16/24
  - Radius: `ml-radius-sm` 8px, `ml-radius` 12px, `ml-radius-lg` 16px
  - Shadow: `ml-shadow` = `0 8px 32px rgba(0,0,0,.12)`
  - Motion: `ml-ease-spring`、duration tokens
  - Font: `font-ui` = Inter Variable、`font-mono` = JetBrains Mono

### モーション（Q17-B）
- **中庸 spring**：`motion-one` or Svelte transitions。60fps、派手排除。
- **具体粒度**:
  - Page 遷移: fade + slide 200ms
  - Overlay enter/exit: `spring({ stiffness: 300, damping: 30 })`
  - Lens chunk 到着: opacity 0→1 + scale 0.95→1 の 150ms spring
  - Hover: Tailwind `transition-colors`、重要 CTA のみ `scale-[1.02]`
  - Command palette: `backdrop-blur` + slide-down spring

### カラー / タイポ / アイコン
- **カラー**：モノクロ基調（#0A0A0A / #FAFAFA）+ 単一アクセント（violet-500系）。ダーク/ライト自動切替＋手動。
- **タイポ**：Inter Variable（UI）+ JetBrains Mono（コード）。行間広め、字間タイト。
- **アイコン**：Lucide。線画1.5px統一。

### ウィンドウクロームとオーバーレイ
- **ウィンドウクローム**：`decorations: false` + 自作タイトルバー（WindowChrome.svelte）。角丸12px、微細ボーダー（1px, 5%不透明）、影 `0 8px 32px rgba(0,0,0,.12)`。
- **オーバーレイ**：半透明 backdrop-filter（blur 24px, saturate 1.8）。ガラスパネル。

### 結果パネルとコマンドパレット（Q17-D）
- **結果パネル**：コマンドパレット様式。`⌘K`（実キー `Ctrl+Shift+P`）でインラインアクションメニュー。
- **コマンドパレット機能拡張**：ページ遷移だけでなく、機能呼び出し（新規 Lens、キャプチャ、履歴検索、cloud 一時許可、`stop_all`、問題報告）まで包括。

### マイクロコピー / アクセシビリティ / 空間
- **マイクロコピー**：日本語、体言止め・短文中心、情報密度優先。
- **アクセシビリティ**：全機能キーボード可、AA以上コントラスト（`axe-core` を Playwright に組込して CI 検証）。
- **空間設計**：8pxベースグリッド、パディング12/16/24の3種のみ、余白潤沢。

## 機能スコープ（Q5, Q6）

v0.1 はフェーズ分けせず以下すべてを対象。ただし release ladder で段階投入。

1. **One-shot キャプチャ + OCR + LLM翻訳 + オーバーレイ**：グローバルホットキー（`Ctrl+Alt+Space`）→ 領域選択 → overlay card に結果表示
2. **Lens streaming（新・中核機能）**：Lens ウィンドウ（click-through 枠）を対象領域に重ね、継続的にストリーミング OCR + 翻訳 + **in-place overlay**。旧 Pin 機能はこれに統合
3. **自動コンテンツ判別**：QR／テキスト主体／写真主体 を領域内容から自動分岐。曖昧時のみミニツールバー
4. **結果パネル アクションリボン**：翻訳／原文コピー／訳文コピー／画像検索／LLM説明／Lens 化
5. **ローカル優先 + クラウドフォールバック**：Ollama未起動/タイムアウト時に DeepL/Gemini API へ。`lens.allow_cloud_mode` policy に従う
6. **ストリーミング翻訳**：OCR行単位でLLMへ流し、初トークン <300ms で逐次描画
7. **QRコード認識**：URL/テキスト/Wi-Fi/連絡先を判定、安全な場合のみワンクリック起動
8. **リバース画像検索**：Google Images / Bing / Yandex をブラウザで開く ＋ SerpAPIキーがあればアプリ内結果
9. **外国語動画のライブ字幕**：システム音声ループバック→whisper.cpp→翻訳→半透明オーバーレイ字幕（**v0.1 は system-wide loopback、target window 選択は v0.2**、Q6-γ）
10. **履歴タイムライン**：rusqlite + tantivy 全文検索、画像サムネイル付き。**Session 単位集約**（Q8-A γ）
11. **設定画面**：ホットキー、LLMモデル、翻訳元/先言語、APIキー、プライバシー設定、オーバーレイ外観。自動生成 + 自動保存
12. **システムトレイ**：常駐起動、メニューから主要機能呼び出し
13. **LLM対話化**：翻訳結果から「要約」「この画像は？」「このQRのURLは安全？」をワンクリック
14. **クリップボード統合**：翻訳後に訳文/原文の自動コピー選択可能
15. **macOS 権限オンボーディング**：画面収録／アクセシビリティ／マイク許可ガイド（**v0.2 以降**）
16. **プラグイン管理画面**：インストール／有効化／無効化／更新／アンインストール／権限確認／設定編集

## Lens 詳細仕様（Q1-Q5）

### 表示形態 — ハイブリッド（Q1 C）
- **メイン表示**: 検出テキストの bbox 上に訳文を **in-place overlay**（Google Lens 方式）
- **サブ表示**: 別パネル（side panel）に履歴的立ち位置で全翻訳履歴、ソース拡大、設定、コピー等を表示。`⌘K` 相当で開閉

### 操作モデル（Q2 C）— hover-revealed toolbar
- **通常時**: 枠本体は click-through、下のコンテンツ操作可能
- **hover 時**: カーソルが枠内に 200ms 以上滞在すると toolbar がフェードイン（移動グラブ、×、設定、side panel 切替）
- **キーボード導線**: `Alt+E` で edit mode toggle（click-through off、全 border 光らせる、drag/resize 可）、`Esc` で戻る。`Alt+W` で close lens、`Alt+P` で side panel

### テキストフィッティング（Q3 D）— Google Lens 式
1. OCR bbox 周囲 2px rim から背景色を mean sampling
2. bbox を該当色で不透明塗りつぶし
3. 訳文を中央 or 左寄せ、font size 自動 shrink（下限 10px）
4. 下限で収まらなければ 2 行折返し（bbox を必要なら縦に伸張）
5. それでも無理なら ellipsis、hover で full text tooltip
6. 背景サンプル不適切時は `settings.lens.render.background_fallback = 'solid_accent'` で violet 85% を使用可能

可読性:
- サンプリング色と訳文色を WCAG AA コントラスト計算で自動選択
- フォント: Inter Variable（既定）、サイズ: bbox 高さ × 0.8 から開始

### ストリーミング描画（Q4 C+D）
- 初トークン到着時に source 幅から推定 font size を固定
- 以降の chunk は到着のたび即反映（throttle なし）
- overflow 検出時のみ font shrink（200ms debounce）
- 最終 token で 1 回だけ最終 reflow
- 初期 placeholder: source テキストを opacity 0.4 + shimmer、初トークン到着で消える
- エラー時: source 表示に戻す + 右上角 `⚠` 12px アイコン（hover 理由）
- 並列: 行単位並列、同時実行数上限 `settings.performance.max_concurrent_translations = 4`
- 更新順序: bbox y 座標昇順（上から埋まる）

### Pin → Lens 統合（Q5 B）
- 旧 Pin 機能は削除。overlay card の「Pin」ボタンは「Lens 化」に変更
- 押下 → その領域を覆う Lens window を spawn、以降は Lens streaming
- `RegionWatcher` は Lens の差分検出エンジン内部に移植、独立モジュールは削除
- spec 内の「Pin 留めオーバーレイ」記述は「Lens streaming」に統合

### 台数制限とアイドル停止（Q5 副次, Q12 副次）
- `settings.performance.max_active_lenses = 4`：超過時は新規 spawn を拒否、UI でエラートースト
- `settings.performance.lens_idle_timeout_min = 10`：N 分変化なしで一時停止、cursor hover で再開
- `settings.performance.lens_stall_timeout_sec = 60`：60 秒以上何も描画されない状態 → 自動停止 + toast

## 速度 SLO（Q18 訂正、2026-04-20 ベンチ後の実測反映）

### 2026-04-20 speed feasibility benchmark 結果（RTX 4080 / Windows 11）

**判定: category beta（600-1200ms、続行 OK、ただし S2 で DXGI + OCR 差替必須）**

| stage | P50 | P95 | 備考 |
|---|---|---|---|
| capture (600×300 region) + PNG | 99ms | 114ms | xcap、 実用 Lens サイズ |
| hide-sleep (現方式) | 60ms | 60ms | DXGI 化で 0ms 可 |
| OCR small (600×120) | 248ms | 328ms | ocrs、 翻訳用途の実用領域 |
| OCR medium (1000×250) | 368ms | 401ms | ocrs |
| OCR large (1600×500) | 875ms | 953ms | ocrs、 大領域は悪化 |
| LLM first-token (gemma4:e2b, warm, think OFF) | **373ms** | 383ms | Ollama、`think: false` 必須 |
| **E2E small** (hide + OCR + LLM) | **684ms** | 745ms | β 判定 |
| **E2E medium** | 788ms | 849ms | β 判定 |

**α 達成（<600ms）への道筋**:
- DXGI Duplication で hide-sleep 削除: -60ms → 624ms
- Windows OCR API 化（想定 80-150ms）: -120ms → **504ms ✓ 目標到達**
- Ollama `num_ctx` / `num_batch` 最適化で LLM first-token -100ms 期待 → ~400ms

**重要な発見**: Gemma 4 系は `thinking` capability を持つ reasoning モデル。Ollama の `/api/generate` で `think: false` を指定しないと**内部思考で first-token が 3.7 秒以上遅延**する（検証済 10× 差）。

詳細: [`docs/benchmarks/2026-04-20-speed-feasibility.md`](benchmarks/2026-04-20-speed-feasibility.md)

### SLO テーブル

| 項目 | 目標 | 最悪許容 | 2026-04-20 実測 |
|------|------|----------|----------------|
| 初トークン（初回フレーム） | <300ms | <500ms | **373ms** (warm, think OFF) |
| Stable frame → overlay 最初の行 | **<0.5s** | **<1.0s** | **684ms** (β、DXGI+WinOCR で <500ms 想定) |
| 全行描画完了（10 行 region） | <1.5s | <2.5s |
| キャッシュヒット時再描画 | <100ms | <200ms |
| Hotkey 反応 | <100ms | <150ms |
| Idle メモリ | <150MB | <180MB |
| 1 Lens streaming メモリ | <250MB | <350MB |
| 3 Lens 同時メモリ | <400MB | <550MB |
| CPU idle | <1% | <3% |
| CPU 1 Lens streaming | <10% | <20% |
| CPU 3 Lens streaming | <25% | <40% |

### 設計上の必須制約
1. **OCR + Translate 並列化**：`settings.performance.max_concurrent_translations = 4`
2. **OCR キャッシュ**：同一 line の pixel hash 照合、未変化行は旧翻訳流用
3. **OCR engine ベンチ（S2 冒頭実施）**：`ocrs` のレイテンシ実測。未達時 fallback:
   - A. `ocrs` 前処理で最適化（downscale / ROI crop）、engine 維持
   - B. PaddleOCR サイドカー切替 → Linux/macOS scope 更に縮小
   - 優先: A 最大限試行、B はラストリゾート
4. **LLM 初トークン <300ms 制約**：Ollama `gemma4:7b-instruct-q4` 実測、厳しければ `gemma4:2b` or smaller をデフォルト推奨
5. **UI render <16ms/frame**：chunk 毎 font-fit 再計算は不可、Q4 D「初推定固定 + overflow 時のみ shrink」必須
6. **DXGI フレーム転送**：GPU 上でダウンサンプル + hash、CPU 転送は pixel diff のみ。OCR 発火時のみ full resolution を CPU へ

### ベンチ未達時の fallback ポリシー
- α. Release blocker、達成まで延期
- β. spec 譲歩（<1.0s 目標へ緩和）、user 承認必要
- γ. 低スペック機向け "degraded mode" 設定（OCR 頻度 3fps に、キャッシュ積極化）
- **採用**: β + γ 併用。release 前実測で判断

## 設定の外出し方針（カスタマイズ性最優先）

「UI・挙動・プロバイダに関わるあらゆる定数を設定ファイルから制御可能にする」を原則とする。ハードコードは禁止、全てSettings経由。工数増は許容。

### 設定保存形式
- **形式**：TOML（可読性）＋JSON Schemaでバリデーション。
- **場所**：OS既定の config dir（`$XDG_CONFIG_HOME/moonlens/config.toml` 等）。
- **レイヤ**：`default.toml`（同梱）→ `user.toml`（上書き）→ プロファイル（複数切替可）。
- **バージョニング（Q13 副次）**：`Settings.version: u32` フィールド、load 時に `CURRENT_VERSION` と比較し migration chain を apply。新しすぎる場合は refuse + warning
- **エクスポート/インポート**：JSON/TOMLで書き出し・読み込み、Gist共有用URL対応。

### ホットリロード（Q13）— カテゴリ別戦略
spec L85「保存で即反映」の具体化:

| カテゴリ | 反映タイミング | 備考 |
|---|---|---|
| Appearance（theme, font, opacity） | **即座** | 現 overlay を再 render、パイプラインには触れない |
| Hotkey | **即座** | unregister → 新 accel で再登録 |
| Target language | **即座、進行中 abort + 即 re-translate** | 「言語変更中」toast |
| Provider chain 順序 | **次 OCR サイクルから** | 進行中 stream は旧設定で完走 |
| OCR engine | **次 OCR サイクルから** | |
| Translate settings | **次 OCR サイクルから** | |
| Privacy (lens.allow_cloud_mode) | **即座** | session 継続中でも即適用、違反 provider skip |

### 自動保存 + 差分表示（Q13-B δ）
- SchemaForm の「保存」ボタンは廃止
- UI 操作時に**即 in-memory 反映**、500ms debounce 後に disk save
- 右上に保存状態インジケータ（ドット: 未保存 / チェック: 保存済み）
- 変更フィールドに bullet dot (`•`) 表示、hover で旧値 tooltip
- 「Diff」ボタンで変更一覧 modal（次回 v0.2）
- 「既定に戻す」= field 単位 + section 単位 + 全体、3 粒度。全体 reset は確認 dialog 必須

### 外部編集コンフリクト（Q13-C γ）
- `Watcher` が外部 `config.toml` 編集検出
- 現 UI が編集中なら「外部で変更されました、リロード？」dialog
- VSCode の「File changed on disk, reload?」パターン準拠

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
model = "gemma4:e2b"                # 2026-04-20 更新: E2B 系を default に
temperature = 0.2
top_p = 0.9
num_ctx = 4096                      # 8192 から削減（翻訳は長文脈不要、prefill 時間短縮）
num_predict = 256                   # 暴走防止
think = false                       # 必須: thinking mode 抑制で first-token 10× 高速化
stream = true
system_prompt = "You are a translator. Output only the translation, no explanation, no alternatives, no commentary."
# system_prompt_file = "prompts/translate.txt"  # v0.2 で外出し化

[providers.translate.deepl]
api_key_env = "DEEPL_API_KEY"   # v0.1 は keyring 統合（下記 Privacy 節）
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
- **Lens**（新）：`allow_cloud_mode`, `ollama_failure_action`, `render.background_fallback`, `max_active_lenses`, `idle_timeout_min`, `stall_timeout_sec`
- **Hotkey**：機能ごとに全て割当可（`capture_region` / `capture_window` / `create_lens` / `toggle_subtitle` / `open_command_palette` / `open_history` / `reverse_image_search` / `stop_all` / `toggle_lens_edit_mode` / custom map）
- **Clipboard**：コンテンツ種別（text/qr/translation/transcript）ごとに自動コピー on/off
- **Privacy**（後述・拡張）
- **Storage**：履歴件数上限、保存先、暗号化キー導出方式、バックアップ、画像保存ポリシー
- **Telemetry**：デフォルトoff、opt-inのみ
- **Appearance**：テーマ（light/dark/system）、アクセントカラー、密度（compact/normal/comfortable）、言語（UI）
- **Performance**：デバウンスms、GPU利用、同時実行数、キャッシュTTL、`max_concurrent_translations`, `max_active_lenses`, `lens_idle_timeout_min`, `lens_stall_timeout_sec`, `show_debug_overlay`
- **Per-Plugin**：プラグイン側が宣言したschemaに従う

### 設定UI実装
- 設定画面は **自動生成** を基本：各Rust側モジュールが `SettingsSchema`（JSON Schema＋UIヒント）を publish し、Svelte側が自動でフォームを描画。新規プロバイダ追加時にUI手書き不要。
- **SchemaForm 要件**（現 bug 修正後）:
  - `$ref` 解決（`#/definitions/*` を walk）
  - `additionalProperties`（HashMap）対応 → 動的キー編集 UI
  - オブジェクトは `<details>` で折り畳み可、検索時自動展開
  - カスタム widget hint サポート: `format: "hotkey"` → `HotkeyInput.svelte` record UI
  - 未サポート型は警告表示（JSON 羅列しない）
- 詳細編集派のため **Rawエディタ** （Monacoでtoml編集＋即バリデーション）も提供。
- 検索バー（設定項目横断検索）、差分表示、ワンクリックで「既定に戻す」。

## Privacy & Security（Q9, Q16）

### Cloud 利用ポリシー — per-Lens session（Q9-A δ）
```toml
[privacy.lens]
allow_cloud_mode = "confirm_per_session"  # 'never' | 'confirm_per_session' | 'always'
ollama_failure_action = "prompt_cloud"     # 'stop' | 'prompt_cloud' | 'auto_cloud'
```

- **`allow_cloud_mode`**: Lens session 起動時の cloud provider 許可
  - `never`: cloud 禁止、Ollama 必須、未起動なら Lens 起動拒否
  - `confirm_per_session`（default）: 起動時に toast で確認、1 タップで許可
  - `always`: 無確認で cloud OK
- **`ollama_failure_action`**: session 中 Ollama 落ちた時
  - `stop`: 停止 + error toast
  - `prompt_cloud`（default）: toast 確認 → 許可で cloud
  - `auto_cloud`: 無確認で cloud 切替
- 両設定の妥当な組み合わせは UI で guard（例: `allow_cloud_mode = never` + `ollama_failure_action = auto_cloud` は validation error）

### Cloud rate limit & budget（Q9-B δ）
```toml
[privacy]
cloud_rate_limit_per_min = 30
cloud_budget_usd_per_day = 0.50
```
- Rate limit 超過: 強制 local-only でその min 継続、UI toast + Cloud badge 灰色化
- Daily budget 超過: 全 cloud provider skip、翌 UTC 0 時にリセット
- Provider の `max_cost_per_request` と組み合わせて呼び出し前にコスト見積

### 機微情報除外（Q9-C γ 必須 + β プリセット）
```toml
[privacy]
respect_wda_excludefromcapture = true   # 他アプリが capture 除外申告したら Lens も capture しない
screen_capture_blocklist = ["1Password", "Bitwarden", "KeePass", "Keeper"]
```
- Windows `SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)` を他アプリが設定している window を Lens 枠が重ねたら自動一時停止
- `screen_capture_blocklist` に実行ファイル名 or window タイトルが一致したら同じく一時停止

### ログ・監査（Q9-D δ）
- 別ログファイルは作らず、履歴 (Q8) の全文検索に `used_provider` を含めることで cloud 送信物を事後特定可能
- 「cloud 送信したテキスト」は履歴 entry の `source_text` + `provider: deepl` 等のフィールドで明示
- `provider:deepl` で検索 → cloud 送信物の一覧化

### Privacy バッジ拡張（Q9 副次）
- 4 値: `Local`（緑）/ `Cloud (rate-limited)`（黄）/ `Cloud`（青）/ `Blocked`（赤）
- lens / overlay / sub panel すべてで表示

### Privacy プリセット（Q9 副次 onboarding）
onboarding 5 ステップ wizard で初期設定:
- **strict**: `allow_cloud_mode = never`, `offline_mode = true`
- **balanced**（default）: `allow_cloud_mode = confirm_per_session`, rate limit default
- **performance**: `allow_cloud_mode = always`, rate limit 高め

### Offline mode（Q16-E β）
```toml
[privacy]
offline_mode = false
```
- `true` で全 cloud provider を強制 skip、内部的に `allow_cloud = false` 一括適用
- spec L344「ネットワーク全ブロック + Ollama 完結」検証項目の実現手段

### API キー保存（Q16-C δ）
- 第一選択: OS keychain（Windows Credential Manager / macOS Keychain / Linux secret-service）経由、`keyring` crate
- Fallback: MoonLens 内で AES-256-GCM、key derivation は `argon2`、user passphrase を初回設定
- `settings.toml` には `api_key_ref = "deepl"` など参照キーのみ、実値は保存しない
- 既存 `api_key_env` 運用は backward compat で残す（v0.1）、v0.2 で deprecate

### Plugin 署名（Q16-B γ）
- spec L208 準拠: ed25519 で manifest に署名
- v0.1 で `ed25519_dalek` による検証実装、local install 時に警告表示
- レジストリ連携は v0.2

### Plugin sandbox（Q16-D γ）
- **WASM**: WASI preview2 の原理的な sandbox（network は `http-outgoing-handler` 経由のみ、fs は preopened dir のみ）
- **Sidecar**: permissions 宣言は「user への情報提供」、install 時 consent UI で明示
- runtime enforcement（AppContainer 等）は v0.2 以降段階導入
- Install 確認 dialog に permissions を明示: 「このプラグインは以下を要求: network（https://api.example.com）, screen_capture, subprocess」

### Tauri capability（Q16-A）
- v0.1: `capabilities/default.json` で全 window に `core:default` 一括付与（現状維持）
- v0.2: window 別に分割（`lens.json`, `selector.json`, etc.）し最小権限化

### CSP（副次）
- v0.1 `"csp": null`（dev 速度優先、README に明記）
- v0.2 で `"default-src 'self' tauri://localhost; connect-src 'self' http://127.0.0.1:*"` に絞る

### Model download hash verification
- OCR / whisper モデル download 時に SHA256 必須、mismatch なら retry → 3 失敗で error

### Log redact
- `tracing-subscriber` の filter で `authorization`, `api_key`, query string の `key=` などを自動マスク

## Error Handling & Failsafes（Q12）

### 失敗時の user 提示（Q12-A γ 混合）
- **行単位の失敗（OCR/translate）**: 該当行のみ source 表示復帰 + `⚠` 12px アイコン（hover 理由）
- **セッション全体の失敗（DXGI/GPU）**: toast（画面隅）+ Lens session 一時停止、「再開」ボタン
- **復旧不能（model 未 DL, disk full）**: modal で action 要求

### 自動復旧戦略（Q12-B）
- **OCR 失敗（1 行）**: 次フレームまで skip（streaming 自然復旧）
- **Translate 失敗**: provider chain 次候補へ、全枯渇時は原文のまま表示
- **DXGI 失敗**: 5 秒 backoff で 3 回 retry → 失敗なら `xcap` polling にフォールバック（flicker 復活だが継続）→ それも駄目なら toast + 停止

### リソース監視（Q12-D β）
- `sysinfo` crate で自 process の CPU/GPU 使用率計測
- 自 process 80% 超で自動 fps 低下（15fps → 5fps）+ 「負荷軽減中」バッジ
- 復帰は 50% 下で元 fps

### AbortController 機構（Q12 副次）
- Rust 側: 全 async task に `tokio_util::sync::CancellationToken` 持たせる
- Frontend: `AbortSignal` 経由で invoke cancel 支援（Tauri に abort 機構追加実装）
- `stop_all` panic button（Q11-D γ）で全 task 即キャンセル

### Watchdog（Q12 副次）
- Lens session が `lens_stall_timeout_sec`（default 60 秒）以上描画なし → 自動停止 + toast

### SQLite backup（Q12 副次）
- `settings.storage.auto_backup = true`（default）
- 週次で `moonlens.db` → `moonlens.db.bak.{1,2,3}` rotate（世代 3）

### ローカルエラーログ（Q12-E γ）
- `$APPDATA/moonlens/logs/error.log` に append、30 日 rotate
- 「問題を報告」UI: ログ末尾 N KB を clipboard copy + GitHub Issue リンク提示
- Telemetry 送信は opt-in 別系統、v0.1 は実装自体なし（送信先未定）

## Hotkey System（Q11）

### Action リスト（Q11-A β）
| Action | Global/Contextual | Default Keybind |
|---|---|---|
| `capture_region` | Global | `Ctrl+Alt+Space` |
| `capture_window` | Global | `Ctrl+Alt+Enter` |
| `create_lens` | Global | `Ctrl+Alt+L` |
| `toggle_subtitle` | Global | `Ctrl+Alt+S` |
| `open_command_palette` | Global | `Ctrl+Shift+P` |
| `open_history` | Global | `Ctrl+Shift+H` |
| `reverse_image_search` | Global | `Ctrl+Alt+G` |
| `stop_all` | Global | `Ctrl+Alt+Escape` |
| `toggle_lens_edit_mode` | Contextual（lens focus） | `Alt+E` |
| `dismiss_overlay` | Contextual（overlay focus） | `Escape` |
| `close_lens` | Contextual（lens focus） | `Alt+W` |
| `toggle_side_panel` | Contextual（lens focus） | `Alt+P` |
| custom | User-defined | - |

### 原則
- Global hotkey は `Ctrl+Alt+<X>` に寄せる（IME や Windows 標準との衝突最小化）
- 未割当（空文字）可能、UI で「未割当」表示、`register_all` は skip（前セッション実装済み）
- 競合検出: settings save 時に同一 accelerator を複数 action に割当不可

### 管理（Q11-C β）
- Global KeyboardShortcuts store (`$lib/keyboard.ts`) を中央集権化
- 各ページが subscribe、action dispatch
- 設定変更が自動伝搬、hot-reload 整合

### HotkeyInput widget（Q11 副次）
- `HotkeyInput.svelte` 新規、record mode で key event → accelerator string
- JSON Schema に `format: "hotkey"` hint、SchemaForm が widget 選択

### `stop_all` 定義（Q11-D γ）
- 全 Lens window 閉じる + Subtitle 停止 + 実行中 OCR/翻訳 abort + プラグイン処理 kill + モデル DL abort + cloud 呼び出し キャンセル

## 履歴 & ストレージ（Q8）

### 保存粒度 — session 単位集約（Q8-A γ）
- **One-shot 1 件 = 1 履歴**（現状踏襲）
- **Lens session 1 件 = 1 履歴**（新）
  - Session 中は timeline 構造で時系列チャンク保持
  - Session 終了時に「完結」マーク、履歴一覧に表示
- DB schema: `entry_kind = 'oneshot' | 'session'`
- Session 中は WAL mode で安定フレーム毎 commit（クラッシュ耐性）

### Session ライフサイクル
| 状態 | トリガ |
|---|---|
| **開始** | Lens window spawn |
| **in-progress** | session 中、履歴一覧には非表示 |
| **終了** | (a) user が window close、(b) `stop_all`、(c) watchdog timeout（60s 描画なし）、(d) app quit |
| **表示** | 終了後に履歴一覧に出現 |
| **自動命名** | `{target_app_name} {YYYY-MM-DD HH:mm} 開始` |

### 画像保存（Q8-B γ、default thumbnail）
```toml
[storage]
image_retention = "thumbnail"   # 'thumbnail' | 'full_recent' | 'full_all' | 'none'
```
- `thumbnail`（default）: 128x128 WebP のみ、~5KB/枚
- `full_recent`: 最大 300 日 full-res、以降 thumbnail に garbage collect
- `full_all`: 無制限保持（警告表示）
- `none`: プライバシー最優先、画像保存なし

### 検索対象（Q8-C γ）
tantivy インデックス:
- 原文 + 訳文 + プロバイダ名 + intent + session タイトル
- `provider:deepl` `intent:qr` `kind:session` などドリルダウン検索対応

### 削除（Q8 副次）
- 物理削除（GDPR 準拠）、ただし UI で「削除」→ 5 秒 undo toast（誤操作救済）

### Export/Import
- v0.1 scope 外、v0.2 で JSON Lines + PNG zip 対応予定

## プラグイン機構（Q7）

ユーザが機能を追加・削除できる仕組み。音楽認識、特殊OCR、独自翻訳プロバイダ、カスタムアクション等を本体から独立して配布可能。

### 2種のプラグイン形態
1. **WASM Component プラグイン**（推奨・デフォルト）
   - Runtime：`wasmtime` + WebAssembly Component Model（WIT interface）
   - 言語自由：Rust / Go / TypeScript(jco) / Python(componentize-py) / MoonBit 等
   - サンドボックス化：WASI preview2
   - 配布：単一 `.wasm` + `plugin.toml` manifest を zip（`.moonlens-plugin`）
   - 用途：純粋計算

2. **Sidecar プラグイン**
   - プロセス分離、JSON-RPC over stdio
   - OS API・重いML・GPL依存が必要な機能向け
   - 配布：実行ファイル＋`plugin.toml`＋依存セットアップスクリプト

### API バージョニング（Q7-C α）— unstable 明示
```toml
api_version = "1-experimental"    # v1.0 リリース時に "1" へ昇格
```
- v0.x は unstable、plugin 開発者は破壊的変更の可能性を受容
- SDK README で「v0.x での API 破壊」を明記
- 互換性判定: host が `api_version` を parse、`-experimental` サフィックスなら warning

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
api_version = "1-experimental"       # unstable marker

[capabilities]
provides = ["music_recognizer"]
consumes = ["audio_loopback"]

[permissions]
network = ["https://api.example.com"]
fs_read = []
fs_write = ["$plugin_data/"]
audio_capture = true
screen_capture = false
clipboard = false
subprocess = false

[signature]
algorithm = "ed25519"
public_key = "..."       # base64
signature = "..."        # base64 of manifest
```

### 拡張ポイント（v0.1 公開範囲 — Q7-A β）
- `translator` / `ocr` / `asr` / `reverse_image_search` / `qr_handler`
- `intent_classifier`
- `action`（結果パネルのアクションリボン追加）
- `pre_processor` / `post_processor`
- `overlay_renderer`
- `hotkey_handler`
- `history_indexer`

**v0.2 以降追加予定（今は未公開）**:
- `frame_stream_processor`（DXGI フレーム前処理）
- `change_detector`（pixel diff 差し替え）
- `lens_overlay_renderer`（Lens 専用描画）
- `translate_stream`（streaming translator、下記）

### Streaming translator API（Q7-B β）
Rust trait:
```rust
trait TranslateProvider {
    async fn translate(&self, text: &str, opts: TranslateOpts) -> Result<String, ProviderError>;
    
    // default impl: 単発翻訳結果を 1 chunk として emit（streaming 非対応 plugin のフォールバック）
    async fn translate_stream(
        &self,
        text: &str,
        opts: TranslateOpts,
        sink: ChunkSink,
    ) -> Result<(), ProviderError> {
        let result = self.translate(text, opts).await?;
        sink.emit(&result, true).await;  // done=true
        Ok(())
    }
}
```
WIT 側にも optional interface として追加。old plugin は default で動く、新 plugin は streaming 実装可能。

### プラグイン配布・インストール
- **ローカルインストール**：`.moonlens-plugin` zipファイルをドラッグ＆ドロップ、ed25519 署名を検証（警告 only）
- **レジストリ**：v0.2 で設計、GitHub Releases ベースの index
- **更新**：manifest に `update_url` 指定時に自動チェック（opt-in）
- **有効化/無効化**：再起動不要、トグル即反映

### プラグイン開発キット
- `plugin-sdk/` にWIT定義、Rust macro（`#[moonlens::plugin]`）、サンプル3種（Hello Translator / Music Recognizer stub / Custom Action）
- **v0.1 注意**: `plugin-sdk/` 実在は S0 で確認、無ければ v0.1 scope から外す
- `moonlens plugin new` CLI で雛形生成
- デバッグ用：ホットリロード、ログビュー、MoonLens本体API modelビュー
- ドキュメント：extension point ごとの仕様、セキュリティガイド、公開手順

## テスト戦略（Q14）

### Layered 構造（Q14-A γ）
- **Unit（全 OS、高速）**: trait-level mock。`FrameSource` / `OcrProvider` / `TranslateProvider` を trait 化、fake impl を流し込む
- **Integration（Linux CI 可）**: real `ocrs` + `MockOllamaServer`（`/api/generate` をチャンクシミュレート）
- **Manual acceptance（release 前 gate）**: 実 Windows + 実 Ollama、human verify

### Unit test 対象
- Pixel diff detector（proptest で性質テスト）
- Stable frame detector
- OCR bbox overflow / font-fit
- Streaming chunk accumulator
- Provider chain fallback state machine
- Region Watcher → OCR trigger

### Visual regression（Q14-B β）
Playwright で canonical test page × 3 状態 = 5 pages × 3 = 15 shot:
- `/` home
- `/settings` schema 全セクション描画
- `/overlay` processing / result / error
- `/history` 空 / 100件 / 検索結果
- `/plugins` 空 / 有効無効混在

ピクセル diff tolerance 1%、failed artifact は CI で 1 週間保持（release tag 時 30 日）。

### Lens UI test（Q14-C β+γ）
- `/dev/lens-preview/` hidden route で固定 fixtures（fake OCR lines + translated text）描画
- Playwright で in-place overlay screenshot
- `production` build 時に除外（`vite.config.ts`）
- Svelte 単体は vitest + testing-library でコンポーネント粒度

### Performance benchmark（Q14-D β）
- `Criterion` で Rust critical path 測定
- `docs/benchmarks/` に history 記録（git 追跡）
- CI への組込は scope 外、手動実行

### Dev tooling（Q14-E γ）
- `MOONLENS_DEBUG_DUMP_FRAMES=1` 環境変数: DXGI フレーム PNG 連番 dump + OCR JSON 保存
- In-app debug overlay: `Ctrl+Alt+D` で toggle、fps / OCR 遅延 / 翻訳遅延 / fallback 発動回数 を live 表示
- `settings.performance.show_debug_overlay = false` デフォルト、production でも on/off 可

### Coverage
- 現 Rust 168 + integration ≈ 80%+。新規 lens pipeline は最低 75% commit 前

### Mock Ollama server
- `examples/mock_ollama.rs`、`POST /api/generate` で固定プロンプトごとに異なるチャンクパターン返却
- シナリオ: 正常、タイムアウト、error、stream 途中切断
- `cargo test --features mock-ollama-integration` で有効化

## 採用する既存OSS / crate

| 領域 | 採用 | 備考 |
|---|---|---|
| アプリシェル | Tauri 2.x | Win/Mac/Linux、軽量 |
| UIフレームワーク | Svelte 5 + Vite | runes、軽快 |
| スタイリング | Tailwind CSS v4 | JITで最小バンドル |
| OCR主 | [`ocrs`](https://github.com/robertknight/ocrs) | Pure Rust、ベンチ未達時は差替 |
| OCR予備 | PaddleOCR サイドカー | `ocrs` 未達時のラストリゾート |
| QR | `rqrr` | ZBar相当、軽量 |
| 翻訳(local) | Ollama + `gemma4` | reqwestストリーミング |
| 翻訳(cloud) | DeepL / Gemini API | フォールバック |
| 画像検索 | ブラウザfallback + SerpAPI(任) | 規約上クローラ不可 |
| ASR | [`whisper.cpp`](https://github.com/ggerganov/whisper.cpp) server | サイドカー |
| 音声取得 | `cpal` | WASAPI loopback含む |
| ホットキー | `global-hotkey` (Tauri公式) | |
| 画面キャプチャ(ワンショット) | `xcap` | マルチモニタ |
| 画面キャプチャ(streaming) | `windows` crate + DXGI Duplication | Windows のみ、Lens streaming 用 |
| Windows API | `windows` crate | WDA_EXCLUDEFROMCAPTURE 等 |
| トレイ | `tray-icon` (Tauri公式) | |
| DB | `rusqlite` + `tantivy` | 履歴＋全文検索 |
| HTTP | `reqwest` (rustls) | ストリーミング対応 |
| ロギング | `tracing` + `tracing-subscriber` | redact filter 追加 |
| WASMランタイム | `wasmtime` | プラグイン用 Component Model |
| 設定 | `toml` + `schemars` + `jsonschema` | TOML保存・schema生成・検証 |
| JSON-RPC | `jsonrpsee` | sidecarプラグイン通信 |
| 署名 | `ed25519_dalek` | プラグイン署名検証 |
| Keyring | `keyring` crate | OS-native API キー保存 |
| Crypto fallback | `argon2` + `ring::aead` | keyring 未対応環境の AES-GCM |
| Benchmark | `Criterion` | perf regression 検出 |
| Motion | `motion-one` or Svelte transitions | UI アニメーション |
| Accessibility test | `axe-core` + Playwright | AA コントラスト検証 |
| System info | `sysinfo` | CPU/GPU 使用率監視 |

**参考にするが流用しない**：Translumo（Win限定だがオーバーレイUX）、LunaTranslator（マルチバックエンド抽象）、NormCap（クロスプラットフォームOCR UX）、Koharu（Rust+MLパイプライン）、Raycast（UI density）。

## ドキュメント構成（Q15）

### 3 分割（Q15-A γ）
- **`README.md`**: プロジェクト要約、主要機能 GIF（release 候補時点で用意）、DL リンク、v0.1 platform 制限、ライセンス、USAGE/DEV へのリンク。英語 summary section 冒頭併記
- **`docs/USAGE.md`**: セットアップ、Ollama インストール、最初のキャプチャ、Lens モード、キーバインド一覧、設定項目、トラブルシュート
- **`docs/DEV.md`**: 環境セットアップ、`npm run tauri:dev`、ビルド手順、テスト、コード構成、アーキ図、プラグイン開発 → `plugin-sdk/README.md`

### 補助ドキュメント
- **`docs/KNOWN_ISSUES.md`**: v0.1 の既知制約（Linux lens flicker、macOS 未対応、auto-update なし、EV 署名なし）
- **`docs/CHANGELOG.md`**: Keep a Changelog フォーマット、日本語、beta ごとに追記
- **`docs/TECH_CHANGES.md`**: 開発者向け技術変更詳細（内部 API、refactor）
- **`docs/RELEASING.md`**: release 手順（既存）、beta ladder 運用更新
- **`docs/benchmarks/`**: Criterion 結果履歴
- **`CONTRIBUTING.md`**: Issue テンプレ、branch 命名、commit format、PR テンプレ、レビュー期待値
- **`.github/ISSUE_TEMPLATE/`**: 日本語 + 英語両方

### Onboarding wizard（Q15-B β+γ）
`Onboarding.svelte`（`MacOnboarding.svelte` を rename + extend）:
1. **言語選択**: UI 言語（ja / en）、翻訳先言語（ja default）
2. **Cloud policy**: strict / balanced / performance の 3 択 radio + 各挙動 1 行説明
3. **Ollama 接続確認**: `GET http://127.0.0.1:11434/api/tags`、無ければ installer リンク + skip
4. **OCR model DL**: 無ければ「ダウンロード（約 100MB）」ボタン + progress bar
5. **ホットキーテスト**: デフォルト hotkey 表示 → 「今押してみて」インタラクティブチェック（`HotkeyFailureToast` と連携、失敗で rebind UI）

完了状態 `settings.onboarding.completed_version` で persist、後から設定画面「Welcome 再表示」で見返せる。

### In-app help（Q15-C γ）
- `?` 単独ショートカットでキーバインド cheatsheet モーダル表示
- Command palette に「Help」コマンド追加

### i18n（Q15-D）
- **v0.1**: 日本語 hardcode
- **v0.2**: `src/i18n/` 追加、`ja.json` / `en.json`、`t('key')` ヘルパ、言語切替可
- 英語コントリビュータ向け README 冒頭に English summary、`docs/DEV.md` は英語ベース

### Plugin SDK（Q15-E）
- S0 で `plugin-sdk/` 実在確認
- 存在すれば v0.1 で README 整備、なければ v0.1 scope 外し「Plugin API は v0.2」と明記

## リスクと対策

- **性能 SLO 未達（Q18 新）**:  `ocrs` が目標届かない可能性あり。S2 冒頭で実測、fallback 方針（α blocker / β spec 譲歩 / γ degraded mode）を用意
- **OCR エンジン差し替え（Q18 依存）**: PaddleOCR サイドカーに切替える場合 Linux/macOS の scope 更に縮小、Windows 先行
- **macOS 配布**：Apple 公証必須。v0.2 以降、Apple Developer 登録 $99/年 + notarytool セットアップ
- **Webview 再描画コスト**：オーバーレイはdiff更新のみ、ストリーミング翻訳チャタリングは50msデバウンス
- **LLM レイテンシ**：Ollama `/api/generate` `stream:true` 必須、初トークン <300ms 目標（spec 厳格化）
- **whisper.cpp CPU負荷**：常時稼働せず明示トグル、アイドル時停止。デフォルト `large-v3-turbo` 量子化、軽量機向け `base` 自動フォールバック
- **画像検索のAPI規約**：ブラウザ起動が第一選択、SerpAPIは任意キー設定時のみ
- **OCR精度**：ocrsが英数に強いがCJK弱い場合 PaddleOCR サイドカーへ言語別ルーティング
- **Wayland画面キャプチャ**：ポータル経由必須、xcapのWayland対応状況をCI確認
- **DXGI 排他**：他アプリ（OBS 等）が DXGI を排他的に握っている場合 fallback → `xcap` polling に degrade
- **キーチェーン未対応環境（Q16-C）**: AES-GCM + passphrase に fallback、user 負担発生

## リポジトリ構成

```
MoonLens/
├─ src-tauri/
│  ├─ src/
│  │  ├─ main.rs
│  │  ├─ app.rs               # Tauri setup
│  │  ├─ capture.rs           # xcap (one-shot)
│  │  ├─ capture/stream.rs    # DXGI Duplication (Windows streaming)  [NEW]
│  │  ├─ capture/wda.rs       # WDA_EXCLUDEFROMCAPTURE (Windows)     [NEW]
│  │  ├─ hotkey.rs            # global-hotkey
│  │  ├─ audio.rs             # cpal loopback
│  │  ├─ pipeline.rs          # one-shot orchestration
│  │  ├─ lens.rs              # lens window lifecycle (redesigned)
│  │  ├─ lens_watcher.rs      # streaming change detector            [NEW]
│  │  ├─ intent.rs            # QR/テキスト/画像判別
│  │  ├─ providers/
│  │  │  ├─ mod.rs            # trait定義（streaming 追加）
│  │  │  ├─ ocr_ocrs.rs
│  │  │  ├─ ocr_paddle.rs     # サイドカー（fallback 用）
│  │  │  ├─ translate_ollama.rs
│  │  │  ├─ translate_deepl.rs
│  │  │  ├─ translate_gemini.rs
│  │  │  ├─ asr_whisper.rs
│  │  │  ├─ qr_rqrr.rs
│  │  │  └─ search_serpapi.rs
│  │  ├─ storage.rs           # rusqlite + tantivy（session schema）
│  │  ├─ overlay.rs           # overlay window
│  │  ├─ settings/            # 階層型設定、schema自動生成
│  │  │  ├─ mod.rs
│  │  │  ├─ schema.rs
│  │  │  ├─ store.rs
│  │  │  └─ profiles.rs
│  │  ├─ plugins/             # プラグイン基盤
│  │  │  ├─ mod.rs
│  │  │  ├─ manifest.rs
│  │  │  ├─ wasm_host.rs
│  │  │  ├─ sidecar_host.rs
│  │  │  ├─ registry.rs
│  │  │  ├─ signature.rs      # ed25519 検証                          [NEW]
│  │  │  └─ wit/
│  │  ├─ credentials.rs       # keyring + AES fallback                [NEW]
│  │  └─ ipc.rs               # Tauri command
│  ├─ capabilities/
│  │  └─ default.json         # Tauri v2 capabilities
│  ├─ Cargo.toml
│  └─ tauri.conf.json
├─ src/                       # Svelte 5 + Vite
│  ├─ routes/
│  │  ├─ overlay/+page.svelte        # one-shot 結果
│  │  ├─ subtitle/+page.svelte
│  │  ├─ settings/+page.svelte       # 自動生成フォーム
│  │  ├─ plugins/+page.svelte
│  │  ├─ history/+page.svelte
│  │  ├─ lens/+page.svelte           # Lens result panel (side)        [REDESIGN]
│  │  ├─ lens-frame/+page.svelte     # Lens click-through border       [NEW]
│  │  └─ dev/lens-preview/+page.svelte  # visual regression fixture   [NEW]
│  ├─ lib/
│  │  ├─ design/
│  │  │  ├─ tokens.css
│  │  │  └─ motion.ts
│  │  ├─ components/          # Button, Panel, CommandPalette, ActionRibbon, PrivacyBadge
│  │  │  ├─ AppHeader.svelte         # (S1 新設、サイドバー統合後は縮退)
│  │  │  ├─ AppSidebar.svelte        # 常駐 nav                      [NEW]
│  │  │  ├─ WindowChrome.svelte      # drag + min/close              [NEW]
│  │  │  ├─ HotkeyFailureToast.svelte
│  │  │  ├─ HotkeyInput.svelte       # keybind recorder              [NEW]
│  │  │  ├─ Onboarding.svelte        # MacOnboarding リネーム拡張
│  │  │  ├─ SchemaForm.svelte        # $ref 解決対応                 [FIX]
│  │  │  └─ LensOverlayRenderer.svelte  # in-place font fit         [NEW]
│  │  ├─ stores/              # settings, history, keyboard
│  │  │  └─ keyboard.ts              # 中央集権ショートカット store  [NEW]
│  │  ├─ navigation.ts               # SvelteKit goto wrapper
│  │  └─ ipc.ts
│  ├─ app.css
│  └─ app.html
├─ vite.config.ts
├─ tailwind.config.ts
├─ svelte.config.js
├─ package.json
├─ plugin-sdk/                # プラグイン開発キット
│  ├─ wit/
│  ├─ rust/
│  ├─ examples/
│  │  ├─ hello-translator/
│  │  ├─ music-recognizer/
│  │  └─ custom-action/
│  ├─ docs/
│  └─ cli/
├─ docs/
│  ├─ plan.md                 # 本書
│  ├─ USAGE.md                [NEW]
│  ├─ DEV.md                  [NEW]
│  ├─ KNOWN_ISSUES.md         [NEW]
│  ├─ CHANGELOG.md
│  ├─ TECH_CHANGES.md         [NEW]
│  ├─ RELEASING.md
│  └─ benchmarks/             [NEW]
├─ CONTRIBUTING.md            [NEW]
├─ .github/
│  ├─ workflows/
│  │  ├─ build.yml            # Linux only (CI budget 節約)
│  │  └─ release.yml          # Linux only, Windows/macOS は local build
│  └─ ISSUE_TEMPLATE/         [NEW]
├─ LICENSE
└─ README.md                  [RESHAPE]
```

## Release Process（Q10, Q18-D）

### Tag ladder
```
main ◄── release ◄── claude/...（feature branch）
  │         │
  │         ├─ v0.1.0-beta.1  (S1 完了時点)
  │         ├─ v0.1.0-beta.2  (S2)
  │         ├─ v0.1.0-beta.3  (S3)
  │         └─ v0.1.0-rc.1    (S4)
  └─ v0.1.0  (S5)
```

### Gate 基準（Q18-C γ）
v0.1.0 stable 昇格条件:
- **Golden Path 6 シナリオ全 pass**（下記）
- Core 12 項目のうち #1, #2, #3 改訂（Lens streaming）, #4, #9, #10, #12 が Win で green
- #11（plugin）は既存サンプル動作確認まで（SDK 整備は v0.1.x 以降）
- Criterion benchmark が SLO 範囲内、未達時は α/β/γ policy 適用
- E2E visual regression 15 shot が安定
- Known Issues に v0.1.0 時点の制約確定記載
- CHANGELOG 更新、release notes ドラフト完了

### Signing & distribution（Q10-C）
- **Windows**: v0.1 自己署名なし（SmartScreen 警告、README 回避手順）、v0.2 で self-sign、v1.0 で EV Code Signing
- **macOS**: v0.2 以降 Apple Developer + notarization
- **Linux**: AppImage + .deb + .rpm、GPG 署名なし、flatpak/snap は v0.3+

### 自動更新（Q10-D）
- **v0.1**: 手動（user が Release ページから DL）
- **v0.2**: Tauri updater plugin + signed manifest

### CI budget strategy（Q10-B γ）
- Feature branch: Linux only
- PR/release tag: Linux only CI、Windows/macOS は local build + manual upload
- Budget 節約最優先、self-hosted runner は v0.2 検討

## Golden Path（v0.1.0 完成判定、Q18-A α）

Win full で以下 6 シナリオ全 pass:

### 1. Lens streaming（中核、新規）
- 英語動画 / ゲーム画面を Lens 枠で覆う
- **<1.0s（目標 <0.5s）以内に**最初の OCR+翻訳が in-place overlay として描画
- コンテンツ変化時、次の stable frame で **<1.0s（目標 <0.5s）以内に**更新
- hide/show flicker 発生なし（WDA_EXCLUDEFROMCAPTURE 検証）
- 5 分 streaming 中メモリリーク無し、SLO 範囲内

### 2. One-shot キャプチャ
- `Ctrl+Alt+Space` → ドラッグ選択 → overlay card < 2 秒、初トークン <300ms（目標）/<500ms（最悪）
- action ribbon が intent に応じて並び替わる

### 3. QR 領域
- QR 含む領域選択 → URL 抽出 → 「開く」ボタンで default browser 起動

### 4. 写真領域 reverse image search
- 写真領域選択 → intent = photo → アクション選択 UI → ブラウザで Google Images 起動

### 5. ライブ字幕
- システム音声 loopback（target window 指定は v0.2）
- Whisper → 翻訳 → 半透明オーバーレイ字幕
- 遅延 <1.5 秒

### 6. Cloud fallback with consent
- Ollama 停止
- Lens 走行中、`confirm_per_session` policy により toast 確認
- user 許可 → DeepL 自動切替
- Privacy バッジ Cloud に遷移

### 追加自動検証項目
| # | 項目 | 方法 |
|---|---|---|
| 7 | プロバイダ フォールバック | Rust unit + integration（mock Ollama timeout → DeepL 成功） |
| 8 | 履歴 session 全文検索 | 100 session から日本語/英語混在検索ヒット |
| 9 | プライバシー `offline_mode` | `offline_mode = true` + Ollama 起動で cloud 呼び出し発生しないこと |
| 10 | 設定ホットリロード | Theme/Appearance 即反映、Provider chain 次サイクル反映 |
| 11 | プラグイン既存動作 | 既存 WASM + Sidecar サンプルが install / 有効化 / 動作 / 無効化 |
| 12 | Provider chain 設定 UI | 1番目 timeout → 2番目成功、UI に理由表示 |
| 13 | `stop_all` panic button | 全 Lens + Subtitle + 実行中 task の即時キャンセル |
| 14 | 自作タイトルバー drag | Main / Overlay / Lens panel ドラッグ可、min/close 動作 |
| 15 | AA コントラスト | `axe-core` で violation 0 |

## 決定事項変更履歴

### 2026-04-20 — Q シリーズ（19 問）による全面改訂
- **Q1** Lens = hybrid（in-place 主 + side panel 履歴）
- **Q2** Lens 操作 = hover-revealed toolbar
- **Q3** テキスト fit = Google Lens 式（opaque fill + shrink + 2行 + ellipsis）
- **Q4** Streaming = 初トークン時 font 推定固定 + chunk 即反映 + overflow 時のみ shrink
- **Q5** Pin 廃止、Lens 一本化
- **Q6** Subtitle = 別機能維持、target window は v0.2
- **Q7** Plugin API = 既存のみ公開、`1-experimental` サフィックス、streaming translator trait default
- **Q8** 履歴 = session 単位集約、画像サムネ default、検索は origin+trans+provider+intent
- **Q9** Cloud policy = per-lens 確認制、rate limit + daily budget、blocklist、監査は履歴統合
- **Q10** v0.1 = Win full / Linux degraded / mac skip、CI = Linux only、beta 段階配布
- **Q11** Hotkey 再整理（create_lens, stop_all, toggle_lens_edit_mode 追加）、デフォルト `Ctrl+Alt+<X>`
- **Q12** Error = カテゴリ別 UI、Cloud 確認 policy、負荷自動 throttle、ログ + 問題報告 UI
- **Q13** Hot-reload = カテゴリ別戦略、自動保存 + 保存インジケータ、外部編集 = 通知 dialog
- **Q14** Test = Layered、Playwright visual regression、debug overlay、Criterion benchmark
- **Q15** Docs = README/USAGE/DEV 3 分割、onboarding 5 step wizard、i18n v0.2
- **Q16** Capability = v0.1 baseline/v0.2 per-window、ed25519 署名 v0.1、API キー = keyring + AES fallback
- **Q17** レイアウト = サイドバー + accent、モーション = 中庸 spring、tokens = Tailwind plugin、command palette 拡張
- **Q18** Release gate = 核心 + Win、SLO 改訂済み、beta 3 + rc + v0.1.0 の 5 tag
- **Q19** セッション 0 新設（spec + 準備）、矛盾 2 解決・曖昧 3 確定・依存 2 確定

### 未決 topic（発生したら個別議論）
- macOS 特有機能（v0.2 以降の深掘り）
- クリップボード統合（spec #14）詳細
- マルチモニタで Lens が跨る時の挙動
- Lens window 位置メモリ（閉じて再度開いた時の復元）
- DPI スケーリングの edge case
- RTL 言語サポート
- 詳細なスクリーンリーダー対応

---

## セッション引継ぎ protocol

> **この section は新セッションを始める Claude が最初に読むべき場所。** 冒頭に戻って「現在地」を読む前に、まずここの preflight と警告を頭に入れる。

### 新セッション開始時の mandatory preflight

**コード変更に手を付ける前に**、以下 5 ステップを必ず実施:

1. **git state 確認**（bash で並列実行可）:
   ```bash
   git branch --show-current              # 想定: claude/moonbit-lens-tool-2FkOu 系
   git status --short                     # 未コミット変更の一覧
   git log --oneline -8                   # 直近 commit
   gh pr list --state open                # open PR 確認
   ```

2. **`docs/plan.md` 冒頭の「現在地」section を読む** — Session 進捗、未コミット一覧、次のアクション、未決事項を正確に把握

3. **`CHANGELOG.md` の `## Unreleased` を読む** — 最新の変更要約

4. **ユーザに preflight 結果を日本語で報告**:
   ```
   現 branch: X / 未コミット N ファイル (list) / 次のアクション Y (plan.md より)。
   この認識で合っていますか？
   ```
   → **ユーザの承認を得てから作業開始**。違っていれば矯正を受ける

5. **skill / MCP ツール利用方針の確認** — 実装前に
   - UI 確認の要否 → 必要なら `browse` or `browser-use` skill を事前に準備
   - spec 解釈の迷いがあれば `/grill-me` skill を提案
   - テスト必須 → `tdd-guide` subagent or `test-driven-development` skill 起動

### セッション終了時の mandatory wrap-up

セッションを閉じる前に:

1. **`docs/plan.md` の「現在地」section を更新**:
   - 完了した todo に `[x]` マークアップ
   - 新たな未コミットファイルを一覧に追加
   - user から受けた新しい矯正指示があれば「直近のユーザ指摘事項」に追記
   - 次の具体的 action を更新

2. **`CHANGELOG.md` の `## Unreleased` に当セッション成果を箇条書き追記** — ユーザ目線での変化（技術詳細は `docs/TECH_CHANGES.md`）

3. **変更を commit してよいかユーザに明示確認** — 勝手に push しない。コミット案を提示 → 承認 → 実施

4. **「次セッションの Claude が plan.md だけ読んで作業再開できるか」を自問** — 疑問が残れば user に提示して追記を相談

### 新 Claude への警告事項（過去の失敗 8 件、同じ轍を踏まない）

#### W1. 「テスト green = 動作 OK」の誤認
- **痕跡**: セッション B で `cargo test` 168 件 + `npm test` 54 件 green を「v0.1.0-rc.1 出荷可」と判定 → user が実機で: 窓動かない、設定画面 JSON 羅列、Lens 1.5s flicker、selector 真っ黒、即 NG
- **教訓**: 自動テストは code paths の検証に過ぎない。UI 実動作は **browseruse の screenshot + click + key input** で確認する。**manual UI 確認なしに "使える状態" と主張しない**

#### W2. UI 確認ツールの手段欠如思い込み
- **痕跡**: セッション B で「Tauri window を直接覗けないから UI 確認不可」と発言 → user「vite + browseruse-cli で確認できるだろう」と矯正
- **教訓**: Tauri Webview の内容は `npm run dev` で Vite 単体起動 + `browser-use open http://localhost:1420/<route>/` で確認できる。IPC は browser 単体では失敗するが、`try/catch` で degrade している route は描画確認可能。**最低でも `/`, `/settings`, `/plugins`, `/history`, `/overlay` の 5 route を screenshot 必須**。`$B status` が stuck するときは `.gstack/browse.json.lock` 削除 + 別 skill（`browser-use`）を使う

#### W3. autonomous 勝手走行禁止
- **痕跡**: セッション B で user が「進めて」と言った直後も approach 提示を省略、結果として user の要求と乖離した変更を連発
- **教訓**: user が明示 autonomous を指示しない限り、**実装前に 2-3 箇条書きで approach 提示 → 承認待ち**。UI/視覚変更は特に user に見える姿を具体的に説明する

#### W4. 仕様の表面的理解での早回し結論
- **痕跡**: セッション C で Lens 1.5s flicker に「60ms sleep を 16ms に短縮」と矮小化提案 → user「キャプチャ必要？ Google Lens は streaming では？」と根本提起 → アーキ全面見直し
- **教訓**: spec 解釈で迷ったら `/grill-me` で一問一答合意形成。**小手先の symptoms patching は禁物**

#### W5. hotkey 登録の panic 落ち（解消済み、再発防止）
- **痕跡**: セッション A で Ctrl+Shift+Space が IME と競合 → `register_all` 内の `?` で Tauri setup 全体 panic → アプリ起動不能
- **対応**: Global hotkey 登録は best-effort に変更、`HotkeyRegistrationReport` で蓄積、UI トースト通知（`hotkey.rs`）。**セッション B で解決済み、再発させないこと**

#### W6. 真っ黒画面 = capabilities 漏れ（解消済み、再発防止）
- **痕跡**: セッション B で user が selector window を開くと真っ黒 → DevTools に `event.listen not allowed` → `src-tauri/capabilities/*.json` が未作成だった
- **対応**: `capabilities/default.json` 追加、全 window に `core:default` + `core:webview:allow-internal-toggle-devtools`。**Tauri v2 プロジェクトでは capabilities 存在確認を preflight に入れる**

#### W7. 設定画面の JSON 羅列
- **痕跡**: セッション B で capabilities 修正後も設定画面で「JSON テキストが羅列」表示 → `SchemaForm.svelte` が `$ref` を解決せず末尾 fallback の `JSON.stringify` に落ちていた
- **教訓**: SchemaForm 改修時は `schema.definitions` を walk して `$ref` を解決、`additionalProperties` も handle、未サポート型は JSON dump でなく警告表示に。**Session 1 の todo**

#### W8. ウィンドウ移動不可
- **痕跡**: `tauri.conf.json` の `decorations: false` だが `data-tauri-drag-region` がない → 移動できない
- **教訓**: 自作タイトルバーは spec L49 の必須要件。`WindowChrome.svelte` で全 main route をカバー。**Session 1 の todo**

### Session 番号と成果物マッピング

| Session | 主成果 | Tag | PR タイトル例 |
|---|---|---|---|
| **S0** | plan.md + docs/ 整備（本セッション含む） | — | `docs: session 0 — spec & documentation overhaul` |
| **S1** | WindowChrome + Sidebar + SchemaForm 修正 + 保存 indicator | `v0.1.0-beta.1` | `feat: v0.1.0-beta.1 — window chrome, sidebar, schema form` |
| **S2** | DXGI stream 基盤 + WDA + OCR ベンチ + engine 決定 | `v0.1.0-beta.2` | `feat: v0.1.0-beta.2 — lens streaming capture foundation` |
| **S3** | Lens 再設計本体（hover + in-place + streaming + session 履歴） | `v0.1.0-beta.3` | `feat: v0.1.0-beta.3 — lens in-place overlay streaming` |
| **S4** | Privacy / Security / Onboarding wizard / Perf tune / E2E | `v0.1.0-rc.1` | `feat: v0.1.0-rc.1 — privacy, security, ship prep` |
| **S5** | Golden Path 6 シナリオ verify + bug fix + docs finalize | `v0.1.0` | `release: v0.1.0` |

### 環境前提

- **OS**: Windows 11 (user の primary machine)
- **Working directory**: `C:\_Workspace\MoonLens`
- **Shell**: bash (Git Bash)
- **Node**: Node.js（`npm`、`npx`）
- **Rust**: stable toolchain
- **Tauri**: 2.x
- **ハンドオーバー先 OS も Windows 前提**（macOS 作業は v0.2 以降）

### ユーザ（c-ardinal）の期待する姿勢

- **日本語で応答** — 最終レポート、commit message は日本語で書ける場合は日本語
- **最終判断前に approach 提示 → 承認**（W3 遵守）
- **嘘・誇張・希望的観測禁止** — gap は最初に明示（W1 遵守）
- **完了判定は自動テスト + 目視検証の両方**（W1-W2 遵守）
- **過去の失敗を plan.md の警告事項と照合**、同じミスを繰り返さない
- **「正直に言うと〜」等の言い訳語禁止**、事実のみ報告
- **勝手な push / destructive 操作禁止** — 全てユーザ確認

### 最重要の単一質問 — 「迷ったらこれを自問」

> **「今やろうとしている作業は、`docs/plan.md` のどの section の、どの決定に紐づいているか？」**

plan.md に記載のない作業をしようとしている場合:
- まず `/grill-me` or user への提示で議論を起こし、**plan.md に追記してから実装**
- 書いていない事を実装すると必ず後で user に矯正される

### 緊急 escape 手順

セッション中に plan.md と現実が大きく乖離した、user 指摘で全体設計を揺るがすレベルの判断が必要になった場合:

1. 実装を **即停止**
2. user に「plan.md 更新が必要な議論が発生した」と報告
3. `/grill-me` 起動を提案
4. 合意後、**plan.md を先に更新** → commit → 初めて実装着手

plan.md は「正」、コードは「plan.md に従うもの」。逆転させない。
