// node --test scripts/changelog.test.mjs
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { test } from 'node:test';
import { checkDiff, extractSection, insertSection, readFragments, render } from './changelog.mjs';

function repo(files) {
  const root = mkdtempSync(join(tmpdir(), 'changelog-'));
  mkdirSync(join(root, 'changelog.d'));
  for (const [name, body] of Object.entries(files)) writeFileSync(join(root, 'changelog.d', name), body);
  return root;
}

test('fragments render in Keep a Changelog order, sorted by name', () => {
  const root = repo({
    'README.md': '# not a fragment',
    'b-fix.fixed.md': '- B fix\n',
    'a-fix.fixed.md': '- A fix\r\n  wrapped\r\n',
    'gpu.performance.md': '- Faster\n',
    'page.added.md': '- New page\n',
  });
  const { fragments, errors } = readFragments(root);
  assert.deepEqual(errors, []);
  assert.equal(
    render(fragments),
    '### Added\n\n- New page\n\n### Performance\n\n- Faster\n\n### Fixed\n\n- A fix\n  wrapped\n- B fix',
  );
});

test('bad fragments are reported', () => {
  const root = repo({
    'Upper.fixed.md': '- x',
    'x.bugfix.md': '- x',
    'empty.added.md': '\n',
    'prose.added.md': 'no bullet',
    'head.added.md': '- x\n### Added',
  });
  const { fragments, errors } = readFragments(root);
  assert.equal(fragments.length, 0);
  assert.equal(errors.length, 5);
});

const LOG = '# Changelog\n\nIntro.\n\n## v0.1.0 — 2026-01-01\n\n### Added\n\n- One\n';

test('insertSection puts the new version above the newest one', () => {
  const out = insertSection(LOG, 'v0.2.0-rc.1 — 2026-02-01', '### Fixed\n\n- Two');
  assert.equal(
    out,
    '# Changelog\n\nIntro.\n\n## v0.2.0-rc.1 — 2026-02-01\n\n### Fixed\n\n- Two\n\n## v0.1.0 — 2026-01-01\n\n### Added\n\n- One\n',
  );
  assert.throws(() => insertSection(out, 'v0.2.0-rc.1 — 2026-03-01', '- x'), /already has/);
});

test('extractSection returns one version body', () => {
  const two = insertSection(LOG, 'v0.2.0 — 2026-02-01', '### Fixed\n\n- Two');
  assert.equal(extractSection(two, 'v0.2.0'), '### Fixed\n\n- Two');
  assert.equal(extractSection(two, 'v0.1.0'), '### Added\n\n- One');
  assert.equal(extractSection(two, 'v0.1'), null);
  assert.equal(extractSection(two, 'v9.9.9'), null);
});

test('checkDiff: entries added straight to CHANGELOG.md fail unless folded', () => {
  const added = '+++ b/CHANGELOG.md\n@@ -7,0 +8,2 @@\n+### Fixed\n+- Direct edit\n';
  assert.equal(checkDiff('M\tCHANGELOG.md\nM\tsrc/a.ts', added).length, 1);
  assert.deepEqual(checkDiff('M\tCHANGELOG.md\nD\tchangelog.d/a.fixed.md', added), []);
  // Removing entries / editing prose is allowed.
  assert.deepEqual(checkDiff('M\tCHANGELOG.md', '-- Old\n+Intro text.\n'), []);
  assert.deepEqual(checkDiff('A\tchangelog.d/a.fixed.md', ''), []);
});
