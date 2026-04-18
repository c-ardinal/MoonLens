# Changelog

All notable changes to MoonLens are documented here. The format
follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and
the project adheres to [SemVer](https://semver.org/).

## Unreleased

### Added

- Tag-driven release workflow (`.github/workflows/release.yml`) that
  produces signed-capable `.dmg` / `.msi` / `.AppImage` / `.deb` /
  `.rpm` bundles for every `v*` tag. `-rc` / `-beta` / `-alpha`
  suffixes publish as pre-releases automatically.
- `docs/RELEASING.md` documenting the end-to-end release flow.

### Changed

- Dropped the unused `network_allow` field in `WasmPlugin` /
  `WasmProvider`. The permission will be re-introduced when WASI p3
  sockets stabilise; until then the field would be persisted without
  being honoured.
- `README.md` updated to reflect the actual implementation
  (removed the stale `tantivy` mention, mentioned Ollama streaming
  and Ed25519 plugin signing).
- `plugin-sdk/examples/music-recognizer/README.md` rewritten from a
  placeholder stub into real install/build instructions now that the
  plugin is implemented.

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
