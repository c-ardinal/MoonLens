# Music Recognizer — MoonLens plugin

Sidecar plugin that recognises music playing on the user's PC by
sending a 16 kHz mono PCM16 chunk (supplied by MoonLens's loopback
capture) to [AudD](https://audd.io/). The track result is formatted as
`<artist> — <title> (<album>)` and returned as a `TaskOutput::Text`.

## Setup

1. Build the binary:

   ```bash
   cargo build --release
   ```

2. Copy `target/release/music-recognizer` and `plugin.toml` into a
   sibling directory on disk, or zip the two into a
   `.moonlens-plugin` archive and install via the MoonLens
   "プラグイン ▸ ファイル追加" button.

3. Set `AUDD_API_TOKEN` in the environment before launching
   MoonLens (the plugin reads it once at startup). An empty token
   falls back to the AudD free tier's shared rate limits.

4. In MoonLens's settings, enable `com.example.music-recognizer` and
   add `plugin.com.example.music-recognizer.asr` to the
   `providers.asr.chain` list if you want the recognizer to be
   reachable from the ⌘⇧S subtitle flow; otherwise leave it off and
   invoke it explicitly from the command palette (future work).

## Implementation notes

- Audio arrives as `Vec<u8>` via the `Asr` task-input variant. The
  plugin wraps it in a minimal PCM16 WAV header (no external deps)
  and posts it as a multipart request to `https://api.audd.io/`.
- The WAV encoder and byte-to-i16 decoder are unit-tested
  (`cargo test`) with twelve cases covering endianness, extremes,
  empty/odd-length inputs, and unicode track metadata.
- Error mapping: DNS/network errors surface as
  `ProviderError::Offline` so MoonLens's fallback chain can skip to
  another ASR provider without spinning.

## Regenerating the Cargo.lock

The lockfile is committed so the plugin builds identically on every
CI runner; delete it and run `cargo generate-lockfile` only when
you bump a dependency on purpose.
