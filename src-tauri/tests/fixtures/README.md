# Test fixtures

## `wasm_hello.wasm` (52 KB, committed)

A minimal `moonlens:plugin/provider` Component used by
`tests/plugin_wasm_real_run.rs`. The Rust source for the fixture lives
in [`wasm-hello/`](./wasm-hello/). We commit the compiled `.wasm` so
`cargo test` works on machines that do not have `cargo-component`
installed — the WASM toolchain is only required when regenerating the
fixture.

### Regenerate

```bash
# One-time setup
cargo install cargo-component --locked
rustup target add wasm32-wasip1

# Build the fixture
cd src-tauri/tests/fixtures/wasm-hello
cargo component build --release
cp target/wasm32-wasip1/release/wasm_hello.wasm ../wasm_hello.wasm
```

The resulting file should stay under ~64 KB; if it grows substantially,
check that `profile.release` still sets `opt-level = "z"` and `lto`.

### What the fixture does

- Advertises the `translator` capability.
- On `Translate` input, returns `[wasm-hello] <text>`.
- On `Summarize` input, returns `sum: <text>`.
- Every other variant returns `LowConfidence`, exercising the error
  marshalling path in the host.

## `lens/` (committed)

Screenshots and synthetic images with OCR ground truth (`*.json`, one
per image) for `tests/lens_images_live.rs`: image in, Japanese
translation out through the real PP-OCR and built-in translator. The
test is `#[ignore]` (needs the downloaded models); see its header for
the command.
