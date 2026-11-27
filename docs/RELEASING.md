# Releasing MoonLens

MoonLens ships through a **two-branch, tag-driven** release flow:

| Branch    | Purpose                                           | Tag shape                              |
| --------- | ------------------------------------------------- | -------------------------------------- |
| `release` | Integration branch for the **next** version.     | `vX.Y.Z-rc.N` / `-beta.N` / `-alpha.N` |
| `main`    | Code that has already shipped as a full release. | `vX.Y.Z` (no suffix)                   |

Feature work merges into `release` first. Release candidates are tagged
on `release`; once an RC is approved, `release` is merged into `main`
and the matching full-release tag is cut there. No feature branch is
ever merged directly into `main`.

Why split the two: it keeps `main` a ready-to-ship snapshot of the
latest published binary, while `release` can accumulate work that
produces rc builds for dog-fooding without disturbing the official
line.

## First release (v0.1.0-rc.1) checklist

No tag has been pushed yet; `v0.1.0-rc.1` is the first one.

1. Merge the open PRs that should ship into `release` and wait for a
   green `build` run on `release`.
2. `node scripts/check-release.mjs v0.1.0-rc.1` — versions, tag shape
   and Tauri npm/crate pairing (CI runs the same check).
3. `node scripts/changelog.mjs release v0.1.0-rc.1` folds the
   fragments in `changelog.d/` into a new `## v0.1.0-rc.1` section of
   `CHANGELOG.md` and deletes them. Review and polish the section.
4. Tag and push (see below). `release.yml` builds the Linux, Windows
   and macOS bundles and creates a **draft** release with all attached.
5. Install from the draft's assets on a clean machine and walk the QA
   list below (model download, capture → translate, pin, uninstall).
6. Press **Publish release** on the draft.

The repository is public: a published (pre-)release is visible to
everyone.

## Version scheme

[Semantic Versioning](https://semver.org):

| Tag              | Branch    | Published as       | Meaning                                                  |
| ---------------- | --------- | ------------------ | -------------------------------------------------------- |
| `v0.2.0-alpha.1` | `release` | GitHub pre-release | Internal / dog-food. Surfaces may change without notice. |
| `v0.2.0-beta.3`  | `release` | GitHub pre-release | Feature-complete. External testers welcome.             |
| `v0.2.0-rc.1`    | `release` | GitHub pre-release | No known regressions. Last call for showstoppers.       |
| `v0.2.0`         | `main`    | GitHub release     | Shipped. No further changes until the next tag.         |
| `v0.2.1`         | `main`    | GitHub release     | Bug-fix on top of `v0.2.0`. No schema migrations.       |

A tag whose name contains a `-` publishes as a GitHub pre-release
automatically. The release workflow also logs a warning if a
pre-release tag is cut outside `release`, or a full-release tag
outside `main` — advisory only so emergency tags still work.

## Changelog fragments

Feature PRs never edit `CHANGELOG.md`: each adds a file
`changelog.d/<slug>.<section>.md` (`node scripts/changelog.mjs new
<section> <slug>`), so parallel PRs cannot conflict on it. The
`changelog` workflow checks every PR. Format: `changelog.d/README.md`.

## Day-to-day flow

```text
feature branch
     │  PR
     ▼
  release  ── tag vX.Y.Z-rc.N ──▶  release.yml ──▶  pre-release
     │                                                   │
     │ QA / dog-food                                     ▼
     │                                            feedback issues
     │  PR (fast-forward when green)
     ▼
    main   ── tag vX.Y.Z      ──▶  release.yml ──▶  full release
```

## Cutting a release candidate

```bash
# 1. Make sure the release branch is green
git checkout release
git pull --ff-only
gh run watch                          # wait for the latest CI on release

# 2. Fold changelog.d/ fragments into CHANGELOG.md, then curate by hand
node scripts/changelog.mjs release v0.2.0-rc.1
$EDITOR CHANGELOG.md                  # add an intro line, merge duplicates
git add -A CHANGELOG.md changelog.d && git commit -m "docs(changelog): v0.2.0-rc.1"
git push

# 3. Tag + push
git tag -s v0.2.0-rc.1 -m "v0.2.0-rc.1"
git push origin v0.2.0-rc.1
```

Pushing the tag fires `release.yml`:

1. The `guard` job checks the tag sits on `release` (warns otherwise).
1b. The same job runs `scripts/check-release.mjs <tag>` and stops the
   release when the tag does not match the app version, when
   `CHANGELOG.md` has no section for the tag, or when `changelog.d/`
   still holds fragments. It then creates the GitHub Release as a
   **draft** (pre-release flag set for `-` tags); the body is
   `.github/release-body.md` followed by that section
   (`scripts/changelog.mjs notes <tag>`).
2. The `build` matrix builds on Linux, Windows and macOS
   (see `docs/ci-and-release-policy.md`) and attaches `.AppImage` /
   `.deb` / `.rpm`, the Windows portable zip
   (`MoonLens_<version>_x64-portable.zip`, no installer), `.dmg`
   (Apple silicon), plus `THIRD_PARTY_LICENSES.html`.
3. Check the draft's assets and notes, then publish it.

Watch progress at
<https://github.com/c-ardinal/MoonLens/actions/workflows/release.yml>.

## QA window

While an rc is live:

- Download the artefact matching your host from the pre-release.
- Walk through the main flows: ⌘⇧Space capture → OCR → translate,
  window-translation windows (several, moved / resized / maximized), live subtitles, plugin install, settings
  hot-reload.
- File any issue against the matching `rc.N` label / milestone.

Feedback lands on the pre-release as GitHub comments or (preferred)
on an issue template.

## Promoting an rc to a full release

When an `rc.N` is declared clean:

```bash
# 1. Fast-forward main from release
git checkout main
git pull --ff-only
git merge --ff-only release
git push origin main

# 2. Tag the new shipped version
git tag -s v0.2.0 -m "v0.2.0"
git push origin v0.2.0
```

`release.yml` builds fresh binaries and publishes a full release with
the new artefacts. `release` and `main` now point at the same commit
and `release` can start accumulating work for the next cycle.

Alternative: if rebuilding is undesirable, `gh release edit
v0.2.0-rc.3 --prerelease=false --latest` flips the flag on the
existing pre-release. Keeps the old artefact names; prefer the tag
route for packages published to package managers (Homebrew,
winget, Flatpak) which expect to discover the release at its tag
URL.

## Hotfixes

For an urgent patch on a shipped release:

```bash
git checkout -b hotfix/v0.2.1 v0.2.0
# commit the fix
git push -u origin hotfix/v0.2.1
```

Open a PR into `main`. Once merged, fast-forward `release` from
`main` so it keeps up:

```bash
git checkout release
git merge --ff-only main
git push origin release
```

Then tag `v0.2.1` on `main`.

## Signing

`release.yml` reads the following repository secrets. All are
optional: missing secrets produce unsigned bundles that still run
locally with the OS's untrusted-developer warning.

- **Tauri updater**: not used. MoonLens has no auto-updater (it makes
  no network calls besides user-started model downloads), so
  `TAURI_SIGNING_PRIVATE_KEY*` are not needed.
- **macOS notarisation**: `APPLE_CERTIFICATE` (base64 .p12),
  `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`,
  `APPLE_ID`, `APPLE_PASSWORD` (app-specific password),
  `APPLE_TEAM_ID`.
- **Windows code-signing**: supply `WINDOWS_CERTIFICATE` /
  `WINDOWS_CERTIFICATE_PASSWORD` and reference them in
  `tauri.conf.json > bundle.windows` once an EV cert is available.

Populate in *Settings ▸ Secrets and variables ▸ Actions*. No code
changes required.

## Licences

Every release bundles `THIRD_PARTY_LICENSES.html` (Rust crates via
`cargo-about`, npm runtime packages, llama.cpp, and the downloadable
models' licences) through `src-tauri/tauri.release.conf.json`.
Generate it locally with:

```bash
cargo install cargo-about --locked --features cli
node scripts/third-party-licenses.mjs
```

`src-tauri/about.toml` lists the accepted crate licences; a dependency
under any other licence fails the release job so it is noticed.

## Rolling back

If a release turns out broken:

1. `gh release edit vX.Y.Z --draft` to hide it from users (and the
   auto-updater, which never serves drafts).
2. Cut `vX.Y.(Z+1)` via the hotfix flow above.
3. Either delete the broken release, or leave it as a draft for
   historical reference.
