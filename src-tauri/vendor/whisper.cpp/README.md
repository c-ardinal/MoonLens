# whisper.cpp (vendored)

Upstream: https://github.com/ggml-org/whisper.cpp, tag `v1.9.5`
(commit `d1be6fde11ac6e0407606b4e42fe72d34add8037`), MIT License (see `LICENSE`).

Only the library sources are kept, with one local change (below):

- `include/whisper.h`
- `src/whisper.cpp`, `src/whisper-arch.h`
- `src/aneforge/whisper-aneforge.h` (header only; MoonLens links the stub in
  `native/aneforge_stub.cpp` instead of the upstream loader)

whisper.cpp is compiled by `build.rs` against the ggml that `llama-cpp-sys-2`
already builds for the translation engine, so the app carries one ggml (and one
set of GPU backends) for both. When the llama.cpp revision moves, check that
this whisper.cpp still compiles against its ggml headers; if not, update to the
whisper.cpp release whose `ggml` sync is closest to it.

## Local change

`moonlens.patch` (already applied to `src/whisper.cpp`; re-apply it after an
upstream update with `patch -p1 < moonlens.patch`):

- `whisper_full` with `language = "auto"` used to run the encoder twice on the
  first window: once to detect the language, once to decode. The decode now
  reuses the encoding from the detection. Live subtitles detect the language of
  every chunk (speech can switch language at any time), so this halves the
  encoder work per chunk; the output is unchanged.
