# MoonLens Plugin SDK

Plugins extend MoonLens with new providers (translators, OCR engines,
intent classifiers, custom actions) or add entirely new features (for
example a music recognizer).

Two plugin flavors are supported.

## 1. WebAssembly Component plugins (recommended)

Implement the `moonlens:plugin/provider` world defined in
[`wit/moonlens.wit`](../wit/moonlens.wit). Any language that can target the
WASM Component Model works (Rust, Go, TypeScript via `jco`, Python via
`componentize-py`, MoonBit, ...). Plugins run in a `wasmtime` sandbox and
cannot reach the host OS unless permissions are explicitly granted in the
manifest.

### Skeleton (Rust)

```rust
wit_bindgen::generate!({
    world: "plugin",
    path: "../../wit/moonlens.wit"
});

struct MyPlugin;

impl exports::moonlens::plugin::provider::Guest for MyPlugin {
    fn list_capabilities() -> Vec<Capability> { vec![Capability::Translate] }
    fn run(input: TaskInput) -> Result<TaskOutput, ProviderError> { todo!() }
}

export!(MyPlugin);
```

Compile with `cargo component build --release`, then zip the `.wasm` with
`plugin.toml` into a `.moonlens-plugin` archive.

## 2. Sidecar plugins

A child process started by MoonLens that speaks newline-delimited
JSON-RPC 2.0 on stdio. Use when you need OS privileges (microphone, GPU),
heavy native dependencies, or a language without WASM Component support.

The Rust SDK ships a `run_sidecar(provider)` helper; see
[`examples/hello-translator/`](../examples/hello-translator/) for a minimal
end-to-end example.

## Manifest (`plugin.toml`)

See [`examples/hello-translator/plugin.toml`](../examples/hello-translator/plugin.toml)
for all required and optional fields. The `[permissions]` block is a
capability allowlist enforced by MoonLens; anything not listed is denied.

## Settings

Plugins declare a JSON Schema at `settings.schema`; MoonLens renders the
corresponding form in the Plugins → Settings pane automatically. Defaults
live under `[settings.defaults]` in TOML.

## Distribution

For now plugins are installed by dropping a `.moonlens-plugin` archive onto
the Plugins page. A future release will add a signed registry; manifests
can already declare an `update_url` for opt-in update checks.

## Extension points

A plugin advertises one or more of the following in `capabilities.provides`:

- `translator`, `ocr`, `asr`, `explain`, `summarize`
- `reverse_image_search`, `qr_handler`, `intent_classifier`
- `action` (adds a button to the result action ribbon)
- `pre_processor` / `post_processor` (image/text hooks in the pipeline)
- `overlay_renderer` (alternate overlay UI)
- `hotkey_handler` (custom bound action)
- `history_indexer` (adds metadata to history entries)

MoonLens resolves `plugin.<id>.<capability>` in provider chains so users can
order plugin providers freely relative to built-ins.
