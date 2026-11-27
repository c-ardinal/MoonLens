#!/usr/bin/env node
// Writes THIRD_PARTY_LICENSES.html: the licence texts of everything
// compiled into the MoonLens app, plus the licences of the models the
// app downloads on request.
//
//   * Rust crates of src-tauri  — `cargo about generate --format json`
//     (config: src-tauri/about.toml; install with `cargo install cargo-about`)
//   * llama.cpp / ggml          — vendored by llama-cpp-sys-2, its own
//                                 LICENSE file is not a crate licence
//   * npm runtime dependencies  — the non-dev packages in package-lock.json
//                                 (bundled into the webview JS)
//   * downloadable models       — not shipped, listed for reference
//
// Usage: node scripts/third-party-licenses.mjs [output path]

import { execFileSync } from 'node:child_process';
import { readFileSync, writeFileSync, existsSync, readdirSync, mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const out = process.argv[2] ?? join(root, 'THIRD_PARTY_LICENSES.html');
const tauriDir = join(root, 'src-tauri');
const pkg = JSON.parse(readFileSync(join(root, 'package.json'), 'utf8'));

const esc = (s) =>
  String(s).replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' })[c]);

// --- Rust crates -------------------------------------------------------
// Written to a file with `-o`: on Windows cargo-about refuses to write
// to a redirected stdout (it guards against PowerShell re-encoding).
const aboutDir = mkdtempSync(join(tmpdir(), 'moonlens-about-'));
const aboutJson = join(aboutDir, 'about.json');
execFileSync('cargo', ['about', 'generate', '--format', 'json', '-o', aboutJson], {
  cwd: tauriDir,
  stdio: ['ignore', 'inherit', 'inherit'],
});
const about = JSON.parse(readFileSync(aboutJson, 'utf8'));
rmSync(aboutDir, { recursive: true, force: true });
// Group identical licence texts so e.g. one MIT text with its copyright
// line is not repeated hundreds of times.
const rustGroups = new Map();
for (const lic of about.licenses) {
  const key = lic.text.trim();
  const g = rustGroups.get(key) ?? { name: lic.name, text: lic.text, crates: new Set() };
  for (const u of lic.used_by) g.crates.add(`${u.crate.name} ${u.crate.version}`);
  rustGroups.set(key, g);
}

// --- llama.cpp ---------------------------------------------------------
const meta = JSON.parse(
  execFileSync('cargo', ['metadata', '--format-version', '1', '--locked'], {
    cwd: tauriDir,
    encoding: 'utf8',
    maxBuffer: 256 * 1024 * 1024,
  }),
);
const extra = [];
const sys = meta.packages.find((p) => p.name === 'llama-cpp-sys-2');
if (sys) {
  const lic = join(dirname(sys.manifest_path), 'llama.cpp', 'LICENSE');
  if (existsSync(lic)) {
    extra.push({ name: 'llama.cpp / ggml (MIT)', text: readFileSync(lic, 'utf8'), crates: ['llama.cpp'] });
  } else {
    console.error(`warning: ${lic} not found; llama.cpp licence omitted`);
  }
}

// --- npm runtime deps -------------------------------------------------
const lock = JSON.parse(readFileSync(join(root, 'package-lock.json'), 'utf8'));
const npmGroups = new Map();
for (const [path, info] of Object.entries(lock.packages)) {
  if (!path.startsWith('node_modules/') || info.dev || info.devOptional) continue;
  const name = path.slice(path.lastIndexOf('node_modules/') + 'node_modules/'.length);
  const dir = join(root, path);
  if (!existsSync(dir)) continue; // optional dep for another platform
  const file = readdirSync(dir).find((f) => /^(licen[cs]e|copying)(\.|$)/i.test(f));
  const text = file ? readFileSync(join(dir, file), 'utf8') : `License: ${info.license ?? 'unknown'}`;
  const key = text.trim();
  const g = npmGroups.get(key) ?? { name: info.license ?? 'unknown', text, crates: new Set() };
  g.crates.add(`${name} ${info.version}`);
  npmGroups.set(key, g);
}

// --- models -----------------------------------------------------------
const models = [
  {
    name: 'PLaMo 3 Translate 2B / 0.8B / 8B (Preferred Networks)',
    license: 'PLaMo Community License',
    url: 'https://plamo.preferredai.jp/info/plamo-community-license-en',
    note: 'Built with PLaMo',
  },
  { name: 'Gemma 4 E4B / E2B (Google)', license: 'Apache-2.0', url: 'https://ai.google.dev/gemma/apache_2' },
  {
    name: 'PP-OCRv5 mobile (PaddlePaddle, via @rustors/model-ppocrv5-mobile)',
    license: 'Apache-2.0',
    url: 'https://github.com/PaddlePaddle/PaddleOCR/blob/main/LICENSE',
  },
  { name: 'ocrs text detection / recognition models', license: 'see project', url: 'https://github.com/robertknight/ocrs-models' },
];

// --- render ------------------------------------------------------------
const section = (title, groups) => {
  const list = [...groups].sort((a, b) => a.name.localeCompare(b.name));
  return `<h2>${esc(title)}</h2>\n${list
    .map(
      (g) => `<section>
<h3>${esc(g.name)}</h3>
<p class="used">${esc([...g.crates].sort().join(', '))}</p>
<pre>${esc(g.text.trim())}</pre>
</section>`,
    )
    .join('\n')}`;
};

const html = `<!doctype html>
<html lang="ja">
<head>
<meta charset="utf-8">
<title>MoonLens ${esc(pkg.version)} — Third-party licences</title>
<style>
body{font:14px/1.5 system-ui,sans-serif;max-width:60rem;margin:2rem auto;padding:0 1rem;color:#222;background:#fff}
pre{white-space:pre-wrap;background:#f5f5f5;padding:.75rem;border-radius:4px;font-size:12px}
.used{color:#555;font-size:12px}
table{border-collapse:collapse}td,th{border:1px solid #ccc;padding:.25rem .5rem;text-align:left}
@media (prefers-color-scheme:dark){body{color:#ddd;background:#111}pre{background:#1d1d1d}.used{color:#aaa}td,th{border-color:#444}}
</style>
</head>
<body>
<h1>MoonLens ${esc(pkg.version)} — Third-party licences</h1>
<p>MoonLens includes the open-source software listed below. Each entry lists the packages
it covers, followed by the full licence text.</p>
<h2>Models (downloaded on request, not included in the package)</h2>
<table><tr><th>Model</th><th>Licence</th></tr>
${models
  .map(
    (m) =>
      `<tr><td>${esc(m.name)}${m.note ? `<br><strong>${esc(m.note)}</strong>` : ''}</td><td><a href="${esc(m.url)}">${esc(m.license)}</a></td></tr>`,
  )
  .join('\n')}
</table>
${section('Native libraries', extra)}
${section('Rust crates', rustGroups.values())}
${section('JavaScript packages', npmGroups.values())}
</body>
</html>
`;
writeFileSync(out, html);
console.log(
  `wrote ${out}: ${about.crates?.length ?? '?'} crates, ${[...npmGroups.values()].reduce((n, g) => n + g.crates.size, 0)} npm packages`,
);
