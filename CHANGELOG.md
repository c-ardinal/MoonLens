# Changelog

All notable changes to MoonLens are documented here. The format
follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and
the project adheres to [SemVer](https://semver.org/).

## Unreleased

### Added

- Built-in local LLM engine: translation, explanation and summaries now
  run in-process through llama.cpp (`llama-cpp-2`, statically linked),
  so Ollama is no longer required. New `llamacpp` provider with a model
  catalog (Gemma 4 E4B default, Gemma 4 E2B, PLaMo 3 Translate 2B / 8B)
  downloaded from the home screen; files are SHA-256 pinned, downloads
  resume after interruption. The model stays resident, is preloaded at
  launch, and the KV cache of the shared prompt prefix is reused between
  requests. Metal on macOS; `llm-vulkan` / `llm-cuda` features are
  opt-in.

### Changed

- Default translate / explain / summarize chains are fully local
  (`["llamacpp"]`, cloud and network disabled). Configs that still carry
  the old seeded Ollama + cloud chains are moved to the built-in engine
  automatically; the Ollama, DeepL and Gemini providers remain available
  for anyone who adds them to a chain.

- Japanese / Chinese OCR: new `ppocr` provider runs PP-OCRv5 mobile
  models (detection + recognition) on rten. It reads kana, kanji,
  simplified/traditional Chinese and English, and is now first in the
  default OCR chain (`["ppocr", "ocrs"]`), falling through to ocrs until
  its models are installed. Configs still carrying the old seeded chain
  `["ocrs", "paddle"]` are upgraded automatically. "モデル取得" installs
  both engines; the PP-OCR bundle is verified against a pinned SHA-512.
- `cargo run --example ppocr_bench -- <image>` benchmark.

- Lens-style in-place translation. The capture hotkey now freezes the
  monitor under the cursor, you select on the still image (or click to
  translate the whole screen), and each paragraph's translation is
  painted over the original text with matching background / text
  colours. Toolbar: original ⇄ translation toggle, copy, selectable text
  panel, explain, search, pin. `Esc` closes.
- Translation streams into the paragraph boxes as tokens arrive.
- OCR returns real line bounding boxes; lines are grouped into
  paragraphs (columns and headings stay separate, wrapped lines are
  joined, CJK-aware) and translated in one request so sentences keep
  their context.
- `cargo run --example ocr_bench -- <image>` prints OCR timings, lines
  and paragraph blocks for a screenshot.

### Fixed

- Legacy `.rten` ocrs models could not load after the rten 0.28 upgrade
  (the `rten_format` feature was off).
- Paragraph grouping: a wide paragraph no longer swallows a column to
  its right, and paragraph breaks are detected from the block's own line
  spacing and from short last lines (CJK line spacing no longer splits
  wrapped lines).
- OCR model download: upstream removed the `.rten` model files (403),
  so fresh installs could not OCR at all. Models are now the `.onnx`
  files (ocrs 0.14 / rten 0.28); existing `.rten` installs keep working.
- OCR reloaded both models from disk on every capture; the engine is
  now cached for the life of the process.
- `tauri dev` / debug builds ran OCR unoptimised (~30 s per capture on
  a 820×360 screenshot); the ML crates are now optimised in dev too
  (~0.6 s).
- QR decoding never ran with the default config because `rqrr` had no
  `[providers.configs]` entry. Built-in providers referenced by a chain
  are now registered with defaults.
- Multi-monitor: the selector opens on the monitor under the cursor
  instead of always the primary one.

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
