#!/usr/bin/env node
// Release preflight. Fails fast on mistakes that otherwise only show up
// 20 minutes into the release build:
//
//   * the app version differs between package.json, src-tauri/Cargo.toml
//     and src-tauri/tauri.conf.json;
//   * a release tag (argument, e.g. `v0.1.0-rc.1`) does not match that
//     version;
//   * an @tauri-apps/* npm package is on a different major.minor than its
//     Rust crate in Cargo.lock (`tauri build` refuses to bundle then);
//   * with a tag: CHANGELOG.md has no `## <tag>` section, or changelog.d/
//     still holds fragments (run `node scripts/changelog.mjs release <tag>`).
//
// Usage: node scripts/check-release.mjs [tag]

import { readFileSync, existsSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { extractSection, readFragments } from './changelog.mjs';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const read = (p) => readFileSync(join(root, p), 'utf8');
const errors = [];

// --- version consistency ----------------------------------------------
const pkg = JSON.parse(read('package.json'));
const conf = JSON.parse(read('src-tauri/tauri.conf.json'));
const cargoToml = read('src-tauri/Cargo.toml');
const cargoVersion = cargoToml.match(/^\[package\][\s\S]*?^version\s*=\s*"([^"]+)"/m)?.[1];

const versions = {
  'package.json': pkg.version,
  'src-tauri/Cargo.toml': cargoVersion,
  'src-tauri/tauri.conf.json': conf.version,
};
const distinct = new Set(Object.values(versions));
if (distinct.size !== 1) {
  errors.push(
    'app version differs between files:\n' +
      Object.entries(versions)
        .map(([f, v]) => `    ${f}: ${v}`)
        .join('\n'),
  );
}
const version = conf.version;

// --- tag ↔ version ----------------------------------------------------
const tag = process.argv[2];
if (tag) {
  const m = tag.match(/^v(\d+\.\d+\.\d+)(?:-((?:alpha|beta|rc)\.\d+))?$/);
  if (!m) {
    errors.push(`tag "${tag}" is not vX.Y.Z or vX.Y.Z-(alpha|beta|rc).N`);
  } else if (m[1] !== version) {
    errors.push(`tag ${tag} does not match app version ${version}`);
  }
}

// --- changelog folded for this tag -------------------------------------
if (tag) {
  if (extractSection(read('CHANGELOG.md'), tag) === null) {
    errors.push(`CHANGELOG.md has no "## ${tag}" section (node scripts/changelog.mjs release ${tag})`);
  }
  const { fragments } = readFragments(root);
  if (fragments.length) {
    errors.push(
      `changelog.d/ still has ${fragments.length} fragment(s); fold them before tagging ` +
        `(node scripts/changelog.mjs release ${tag})`,
    );
  }
}

// --- @tauri-apps npm ↔ crate major.minor -----------------------------
const lock = read('src-tauri/Cargo.lock');
const crateVersion = (name) =>
  lock.match(new RegExp(`\\[\\[package\\]\\]\\nname = "${name}"\\nversion = "([^"]+)"`))?.[1];
const majorMinor = (v) => v?.split('.').slice(0, 2).join('.');

const npmToCrate = {
  '@tauri-apps/api': 'tauri',
};
for (const dep of Object.keys({ ...pkg.dependencies, ...pkg.devDependencies })) {
  const m = dep.match(/^@tauri-apps\/plugin-(.+)$/);
  if (m) npmToCrate[dep] = `tauri-plugin-${m[1]}`;
}
for (const [npmName, crate] of Object.entries(npmToCrate)) {
  const installed = join(root, 'node_modules', npmName, 'package.json');
  const npmVersion = existsSync(installed)
    ? JSON.parse(readFileSync(installed, 'utf8')).version
    : (pkg.dependencies?.[npmName] ?? '').replace(/^[^\d]*/, '');
  const rustVersion = crateVersion(crate);
  if (!rustVersion) continue; // npm-only helper, no crate
  if (majorMinor(npmVersion) !== majorMinor(rustVersion)) {
    errors.push(
      `${npmName} ${npmVersion} vs ${crate} ${rustVersion}: major.minor must match ` +
        `(npm install ${npmName}@^${rustVersion})`,
    );
  }
}

if (errors.length) {
  for (const e of errors) console.error(`::error title=release preflight::${e}`);
  process.exit(1);
}
console.log(`release preflight ok: version ${version}${tag ? `, tag ${tag}` : ''}`);
