// node --test scripts/portable-zip.test.mjs
import assert from 'node:assert/strict';
import { mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { test } from 'node:test';
import { stage, zipName } from './portable-zip.mjs';

function fixture() {
  const tmp = mkdtempSync(join(tmpdir(), 'portable-'));
  const repoRoot = mkdtempSync(join(tmpdir(), 'portable-repo-'));
  writeFileSync(join(tmp, 'moonlens.exe'), 'MZ');
  writeFileSync(join(tmp, 'moonlens.pdb'), 'debug');
  writeFileSync(join(repoRoot, 'LICENSE'), 'MIT');
  writeFileSync(join(repoRoot, 'THIRD_PARTY_LICENSES.html'), '<html>');
  return { releaseDir: tmp, repoRoot, outDir: join(tmp, 'out') };
}

test('package holds the renamed executable, licences and a readme', () => {
  const f = fixture();
  const files = stage({ ...f, version: '1.2.3' });
  assert.deepEqual(files, ['LICENSE', 'MoonLens.exe', 'README.txt', 'THIRD_PARTY_LICENSES.html']);
  assert.equal(readFileSync(join(f.outDir, 'MoonLens', 'MoonLens.exe'), 'utf8'), 'MZ');
  const readme = readFileSync(join(f.outDir, 'MoonLens', 'README.txt'), 'utf8');
  assert.match(readme, /^﻿MoonLens 1\.2\.3/);
  assert.ok(!/[^\r]\n/.test(readme), 'CRLF line ends only');
});

test('DLLs next to the executable come along', () => {
  const f = fixture();
  writeFileSync(join(f.releaseDir, 'extra.dll'), 'dll');
  assert.ok(stage({ ...f, version: '1.2.3' }).includes('extra.dll'));
});

test('a missing executable or licence file stops the packaging', () => {
  const f = fixture();
  assert.throws(() => stage({ ...f, releaseDir: f.repoRoot, version: '1.2.3' }), /no moonlens\.exe/);
  assert.throws(() => stage({ ...f, repoRoot: f.releaseDir, version: '1.2.3' }), /THIRD_PARTY_LICENSES/);
});

test('zip name carries the version', () => {
  assert.equal(zipName('0.1.0'), 'MoonLens_0.1.0_x64-portable.zip');
});
