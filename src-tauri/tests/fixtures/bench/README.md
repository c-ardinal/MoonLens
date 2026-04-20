# bench fixtures

Fixture PNGs for `cargo run --example bench_speed` live here. They are
**gitignored** because the first run captures the developer's primary
monitor contents — those screenshots may contain personal data.

## Regenerate

Delete any existing `.png` files, then run the benchmark:

```bash
cd src-tauri
cargo run --example bench_speed --release -- --iters 10 --warmup 3 --write-report
```

The harness will capture the current primary monitor and crop three
reference rects (`small.png`, `medium.png`, `large.png`) into this
directory. Subsequent runs reuse the saved fixtures, so results stay
comparable until you regenerate.

## Bring-your-own fixtures

You can drop any PNG files named `small.png`, `medium.png`, `large.png`
into this directory before running the bench. The harness detects
pre-existing fixtures and skips the screen capture step. Recommended
content: a screen-shot of a subtitle line (`small`), a paragraph of
text (`medium`), and a full PDF page (`large`).
