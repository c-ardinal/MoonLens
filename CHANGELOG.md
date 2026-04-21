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
