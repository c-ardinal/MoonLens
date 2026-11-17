# CI & Release Build Policy

This document captures decisions about **what CI builds and what is
built locally**, why the split is shaped the way it is, and how to
revert budget-saving workarounds once constraints lift. It is the
companion to `RELEASING.md` (which covers the tag flow itself) and
sits a layer above `.github/workflows/{build,release}.yml`.

## Context: free-tier minute budget

GitHub Actions free tier allocates **2000 minutes / month** for
private repositories. On a shared account those minutes must cover
MoonLens plus every other project the owner maintains. The budget is
the binding constraint on CI design — correctness is a given; cost
shapes the workflow layout.

## Billing multipliers

Minutes are **not charged at wall-clock 1:1**. OS multipliers on
GitHub-hosted runners:

| Runner             | Multiplier | Typical MoonLens build | Billing minutes |
| ------------------ | :--------: | ---------------------: | --------------: |
| `ubuntu-latest`    | ×1         | ~18 min                | ~18             |
| `windows-latest`   | ×2         | ~30 min                | ~60             |
| `macos-14` (ARM)   | **×10**    | ~7 min                 | **~70**         |

A naive 3-platform matrix therefore costs ~148 billing minutes per
run, **macOS dominates even though its wall-clock is fastest**. One
full run ≈ 7.4% of the monthly free budget. Every merge + tag cycle
that naively rebuilds on all three runners eats ~15%. Under 10 cycles
per month the budget is gone.

This is the single number that drives every decision below.

## CI tier design (`.github/workflows/build.yml`)

### Default: Linux only

Every PR (any base branch) and every push to `main` / `release` runs
on `ubuntu-latest` only. Rationale:

- Linux catches ~95% of regressions. Platform-specific bugs (xcap
  Linux backends, cpal WASAPI vs CoreAudio vs PulseAudio, Windows
  `link.exe` quirks) surface at the **release tag build**, not at
  merge time. Merge-time platform parity was not worth the 7× cost.
- `cargo fmt`, `clippy -D warnings`, the 269-test suite, and the
  WASM Component fixture all run on Linux end-to-end.
- macOS-specific code paths (CoreGraphics permission detection) are
  gated by `#[cfg(target_os = "macos")]` and do not compile on
  Linux; the remaining Mac-only surface is small enough that
  breakage would be caught during local dev on a Mac.

### Triggers: PR-driven, not push-driven

Pushes to feature branches (`claude/**`, `feature/**`, ...) do **not**
trigger CI on their own; the branch's PR does. Previously a push to a
branch with an open PR ran the full suite twice (once for `push`, once
for `pull_request`), doubling cost for the most frequent event. To
check a branch that has no PR yet, run the workflow manually
(`Actions → build → Run workflow`, pick the branch).

### Jobs

Two jobs run in parallel:

- `frontend` (~3 min): `npm ci`, `npm run check` (svelte-check),
  `npm test` (vitest), `npm run build` (production vite build).
  Gives fast feedback on UI changes without waiting for Rust.
- `rust` (~15 min): `cargo fmt --check` on all four crates, `clippy
  --locked -D warnings` and `cargo test --locked` for `src-tauri`,
  the plugin SDK and both SDK examples. `--locked` fails the run if a
  `Cargo.toml` change was pushed without the matching `Cargo.lock`.

The Rust cache (`Swatinem/rust-cache`) is only **written** by runs on
`main` / `release`; PR runs restore from it. This keeps the 10 GB
repo cache quota from being churned by every PR.

### On-demand matrix expansion

`workflow_dispatch` exposes an `all_platforms: boolean` input. When
set true the `rust` job fans out to Linux + Windows + macOS. Use cases:

- Pre-tag smoke test: cross-platform sanity before cutting an `rc`.
- Investigation: a Linux-green PR suspected of platform-specific
  regression.

### Windows on native-code PRs (`.github/workflows/windows.yml`)

The "Run workflow" button only shows for workflows on the default
branch (`main`), which carries no code, so `workflow_dispatch` above is
not reachable from the UI in practice. PRs that touch the bundled
llama.cpp or the Rust dependency graph (`Cargo.toml` / `Cargo.lock`,
`build.rs`, `src-tauri/.cargo/**`, the local LLM provider files) run a
Windows clippy + test job automatically. Other PRs stay Linux-only.

### Concurrency

`cancel-in-progress: true` on every ref **except** `main` / `release`.
Rapid pushes to a feature branch and force-pushes to a PR cancel
superseded runs so minutes aren't burned on stale commits. Runs on
`main` / `release` complete so merge-time history stays queryable.

### `paths-ignore`

`**.md` (includes `CHANGELOG.md`), `docs/**`, `LICENSE` do not trigger CI. The
burst of doc-only commits near a release cycle is free.

## Dependency audit (`.github/workflows/audit.yml`)

`cargo audit` (all four crates) and `npm audit` run every Monday, on
demand, and on PRs that change a lockfile or manifest. No compile, so
a run costs ~2 billing minutes. Vulnerability advisories fail the run;
"unmaintained" / "unsound" notices are warnings. Runtime npm deps fail
on moderate+, dev-only tooling is reported but never fails. The weekly
schedule matters because advisories are published after merge: a
lockfile that was clean last month can be vulnerable today.

GitHub Actions versions are bumped by Dependabot (`.github/dependabot.yml`),
grouped into one monthly PR against `release`, so it costs one CI run.

## Release build split (`.github/workflows/release.yml`)

`workflow_dispatch` (re-running a release for an existing tag) checks
out **the tag**, not the branch the run was started from.

### Current cycle (v0.1.0-rc.1): Linux only in CI

The macOS + Windows matrix entries in `release.yml` are **commented
out** for this release cycle. CI produces only `.deb` / `.AppImage` /
`.rpm` (~25 billing minutes for the whole workflow).

**Trigger**: any `v*` tag push, e.g. `v0.1.0-rc.1`.

**What runs**: `guard` (tag-branch hygiene warning) + `build` (Linux
matrix entry) via `tauri-apps/tauri-action@v0`. No tests, no clippy
— this workflow exists to **bundle**, not to verify. Verification
happens earlier in `build.yml` on the same commit.

### Windows installers: built locally

The primary developer runs Windows. Policy:

- Dev machine builds `.msi` and `.nsis` via `npm run tauri:build`.
- Outputs land under `src-tauri\target\release\bundle\{msi,nsis}\`.
- Attach to the published pre-release: `gh release upload
  v0.1.0-rc.1 <path to .msi> <path to .nsis>`.
- This is **not** a workaround; it stays this way even after budget
  relief, because dogfooding builds on the dev machine catches env
  drift (WebView2 runtime version, MSVC toolchain) that CI misses.

### macOS installers: deferred

No Mac is available to the current developer, and CI at ×10 is the
cost bottleneck. Policy for this cycle:

- Skip `.dmg` generation entirely.
- Announce Linux + Windows coverage in the release notes.
- Re-introduce macOS in the **next** tag (`v0.1.0-rc.2` or `v0.1.0`)
  after the monthly Actions minute reset.

### Reversion path

When budget is healthy (reset, paid plan, self-hosted runners):

```diff
  # .github/workflows/release.yml
  matrix:
    include:
      - { name: linux, os: ubuntu-latest, bundle: deb,appimage,rpm }
-     # - { name: macos, os: macos-14, bundle: dmg }
-     # - { name: windows, os: windows-latest, bundle: msi,nsis }
+     - { name: macos, os: macos-14, bundle: dmg }
+     - { name: windows, os: windows-latest, bundle: msi,nsis }
```

Two-line diff. No other workflow files need to change. Windows-local
build policy can continue in parallel (belt-and-braces) or be
retired.

## Per-cycle cost budget

Targeted cost for a typical release cycle under the current policy:

| Step                                        | Billing min |
| ------------------------------------------- | ----------: |
| PR push × ~5 iterations (PR-only, no dup)   |     ~90    |
| PR merge into `release`                     |      ~18    |
| Tag `vX.Y.Z-rc.N` → Linux-only bundle       |      ~25    |
| PR merge `release → main` + tag `vX.Y.Z`    |      ~43    |
| **Total per cycle**                          | **~176**   |

Compared to the pre-optimization baseline (~300–400 min per cycle
with 3-platform matrix on every push and full release build on every
tag), this is a 2–3× reduction. Headroom for 8–10 full cycles per
month within the free tier.

## Local responsibilities vs CI

A clear split of what must pass on the dev machine before a push
vs. what CI gates:

| Check                                       | Dev machine | CI (Linux) |
| ------------------------------------------- | :---------: | :--------: |
| `cargo fmt --check` (all 4 crates)          |      ✓      |     ✓      |
| `cargo clippy -D warnings` (all 4 crates)   |      ✓      |     ✓      |
| `cargo test --lib`                          |      ✓      |     ✓      |
| `cargo test --tests`                        |      ✓      |     ✓      |
| `npm run check`                             |      ✓      |     ✓      |
| `npm test` (vitest)                         |      ✓      |     ✓      |
| `npm run build` (vite production build)     |      ✓      |     ✓      |
| `npm run tauri:dev` smoke test              |      ✓      |            |
| `npm run tauri:build` (Windows installer)   | ✓ (release) |            |
| `.deb` / `.AppImage` / `.rpm`               |             | ✓ (tag)    |
| `.dmg` (deferred)                           |             |            |

CI is the authoritative Linux gate. Dev machine is the authoritative
Windows gate. macOS has no gate this cycle — accepted risk.

## Skip-CI etiquette

When pushing a commit that is genuinely not worth a CI run (WIP
intermediate, formatting-only if `paths-ignore` doesn't cover it,
branch housekeeping), include `[skip ci]` in the commit subject.
GitHub honors this to avoid spending minutes. Use sparingly and never
on commits that cross a merge boundary.

## When to reconsider this policy

Triggers for revisiting:

- Switch to a paid GitHub plan or self-hosted runner → remove the
  Linux-only caps across the board.
- A platform-specific regression ships → tighten by re-enabling that
  platform in `build.yml` for PRs at least.
- macOS user onboards → make macOS local build the new default (as
  with Windows today) and keep `.dmg` out of CI.
- Contributor count grows beyond solo → PR gating on all 3
  platforms becomes more defensible, since regressions land faster
  than one person can review.

## Appendix: quick math

Derived numbers referenced throughout:

```
macOS   billing = wall × 10
Windows billing = wall × 2
Linux   billing = wall × 1

3-platform matrix at observed MoonLens build times:
   7 × 10 + 30 × 2 + 18 × 1 = 70 + 60 + 18 = 148 min per run

Free tier: 2000 min/month
   148 min  ≈ 7.4 %  per full 3-platform run
   18 min   ≈ 0.9 %  per Linux-only run
```
