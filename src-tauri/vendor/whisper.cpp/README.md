# whisper.cpp (vendored)

Upstream: https://github.com/ggml-org/whisper.cpp, tag `v1.9.5`
(commit `d1be6fde11ac6e0407606b4e42fe72d34add8037`), MIT License (see `LICENSE`).

Only the library sources are kept, unmodified:

- `include/whisper.h`
- `src/whisper.cpp`, `src/whisper-arch.h`
- `src/aneforge/whisper-aneforge.h` (header only; MoonLens links the stub in
  `native/aneforge_stub.cpp` instead of the upstream loader)

whisper.cpp is compiled by `build.rs` against the ggml that `llama-cpp-sys-2`
already builds for the translation engine, so the app carries one ggml (and one
set of GPU backends) for both. When the llama.cpp revision moves, check that
this whisper.cpp still compiles against its ggml headers; if not, update to the
whisper.cpp release whose `ggml` sync is closest to it.
