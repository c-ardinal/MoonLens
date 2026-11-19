#!/usr/bin/env node
// Changelog fragments. Every PR adds its own file under `changelog.d/`
// instead of editing CHANGELOG.md, so parallel PRs never touch the same
// lines. A release folds the fragments into CHANGELOG.md.
//
// Fragment: changelog.d/<slug>.<section>.md
//   section: added | changed | deprecated | removed | performance | fixed | security
//   body:    one or more Markdown bullets ("- ..."), wrapped as you like.
//
// Usage:
//   node scripts/changelog.mjs new <section> <slug>   create a fragment
//   node scripts/changelog.mjs preview                print the pending section
//   node scripts/changelog.mjs check [--base <ref>]   validate (CI)
//   node scripts/changelog.mjs release <tag> [--date YYYY-MM-DD]
//                                                     fold fragments into CHANGELOG.md
//   node scripts/changelog.mjs notes <tag>            print a released section (release CI)

import { execFileSync } from 'node:child_process';
import { existsSync, readdirSync, readFileSync, unlinkSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

// Keep a Changelog order; "Performance" is this project's addition.
export const SECTIONS = [
  ['added', 'Added'],
  ['changed', 'Changed'],
  ['deprecated', 'Deprecated'],
  ['removed', 'Removed'],
  ['performance', 'Performance'],
  ['fixed', 'Fixed'],
  ['security', 'Security'],
];
const SECTION_KEYS = SECTIONS.map(([k]) => k);
const NAME_RE = /^([a-z0-9][a-z0-9-]*)\.([a-z]+)\.md$/;
// Files in changelog.d/ that are not fragments.
const IGNORED = new Set(['README.md', '.gitkeep']);

const defaultRoot = join(dirname(fileURLToPath(import.meta.url)), '..');

/** Reads and validates every fragment. Returns { fragments, errors }. */
export function readFragments(root = defaultRoot) {
  const dir = join(root, 'changelog.d');
  const fragments = [];
  const errors = [];
  if (!existsSync(dir)) return { fragments, errors };
  for (const file of readdirSync(dir).sort()) {
    if (IGNORED.has(file)) continue;
    const m = file.match(NAME_RE);
    if (!m) {
      errors.push(`changelog.d/${file}: name must be <slug>.<section>.md (lowercase, digits, "-")`);
      continue;
    }
    const [, slug, section] = m;
    if (!SECTION_KEYS.includes(section)) {
      errors.push(`changelog.d/${file}: unknown section "${section}" (use ${SECTION_KEYS.join(' | ')})`);
      continue;
    }
    const body = readFileSync(join(dir, file), 'utf8').replace(/\r\n/g, '\n').trim();
    if (!body) {
      errors.push(`changelog.d/${file}: empty`);
      continue;
    }
    if (!body.startsWith('- ')) {
      errors.push(`changelog.d/${file}: must be Markdown bullets starting with "- "`);
      continue;
    }
    if (/^#/m.test(body)) {
      errors.push(`changelog.d/${file}: no headings; the section comes from the file name`);
      continue;
    }
    fragments.push({ file, slug, section, body });
  }
  return { fragments, errors };
}

/** Renders fragments as `### Section` blocks (no version heading). */
export function render(fragments) {
  const out = [];
  for (const [key, title] of SECTIONS) {
    const items = fragments.filter((f) => f.section === key);
    if (!items.length) continue;
    out.push(`### ${title}`, '', items.map((f) => f.body).join('\n'), '');
  }
  return out.join('\n').trimEnd();
}

/** Inserts a version section above the newest released one. */
export function insertSection(changelog, heading, body) {
  const text = changelog.replace(/\r\n/g, '\n');
  if (new RegExp(`^## ${escape(heading.split(' ')[0])}(\\s|$)`, 'm').test(text)) {
    throw new Error(`CHANGELOG.md already has a "${heading.split(' ')[0]}" section`);
  }
  const block = `## ${heading}\n\n${body}\n\n`;
  const at = text.search(/^## /m);
  if (at < 0) return `${text.trimEnd()}\n\n${block}`.trimEnd() + '\n';
  return text.slice(0, at) + block + text.slice(at);
}

/** Body of `## <tag>` in CHANGELOG.md, or null. */
export function extractSection(changelog, tag) {
  const text = changelog.replace(/\r\n/g, '\n');
  const re = new RegExp(`^## ${escape(tag)}(?:\\s[^\\n]*)?\\n([\\s\\S]*?)(?=^## |(?![\\s\\S]))`, 'm');
  const m = text.match(re);
  return m ? m[1].trim() : null;
}

const escape = (s) => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');

function git(root, ...args) {
  return execFileSync('git', args, { cwd: root, encoding: 'utf8' });
}

/**
 * PR rule: new entries go to changelog.d/. A diff that adds bullets to
 * CHANGELOG.md must also delete fragments (a release fold). Removing or
 * rewording prose is fine; a typo fix inside an old bullet needs the
 * `changelog-edit` PR label (CHANGELOG_ALLOW_EDIT=1).
 */
export function checkDiff(nameStatus, changelogPatch) {
  const folded = nameStatus
    .split('\n')
    .some((l) => /^D\tchangelog\.d\//.test(l));
  const addsBullets = changelogPatch
    .split('\n')
    .some((l) => /^\+\s*- /.test(l));
  if (addsBullets && !folded) {
    return [
      'CHANGELOG.md gained entries directly. Add a fragment instead: ' +
        'node scripts/changelog.mjs new <section> <slug> (see changelog.d/README.md). ' +
        'Only a release fold, which deletes fragments, adds entries to CHANGELOG.md.',
    ];
  }
  return [];
}

function main(argv) {
  const root = defaultRoot;
  const [cmd, ...rest] = argv;
  const opt = (name) => {
    const i = rest.indexOf(name);
    return i >= 0 ? rest[i + 1] : undefined;
  };
  const fail = (msgs) => {
    for (const m of msgs) console.error(`::error title=changelog::${m}`);
    process.exit(1);
  };

  switch (cmd) {
    case 'new': {
      const [section, slug] = rest;
      if (!SECTION_KEYS.includes(section) || !slug || !/^[a-z0-9][a-z0-9-]*$/.test(slug)) {
        fail([`usage: changelog.mjs new <${SECTION_KEYS.join('|')}> <slug>`]);
      }
      const path = join(root, 'changelog.d', `${slug}.${section}.md`);
      if (existsSync(path)) fail([`${path} already exists`]);
      writeFileSync(path, '- \n');
      console.log(`created changelog.d/${slug}.${section}.md`);
      return;
    }
    case 'preview': {
      const { fragments, errors } = readFragments(root);
      if (errors.length) fail(errors);
      console.log(fragments.length ? render(fragments) : '(no fragments)');
      return;
    }
    case 'check': {
      const { fragments, errors } = readFragments(root);
      const base = opt('--base');
      if (base && process.env.CHANGELOG_ALLOW_EDIT !== '1') {
        const range = `${base}...HEAD`;
        errors.push(
          ...checkDiff(
            git(root, 'diff', '--no-renames', '--name-status', range),
            git(root, 'diff', '-U0', range, '--', 'CHANGELOG.md'),
          ),
        );
      }
      if (errors.length) fail(errors);
      console.log(`changelog ok: ${fragments.length} pending fragment(s)`);
      return;
    }
    case 'release': {
      const tag = rest[0];
      if (!tag || !/^v\d+\.\d+\.\d+/.test(tag)) fail(['usage: changelog.mjs release vX.Y.Z[-rc.N] [--date YYYY-MM-DD]']);
      const { fragments, errors } = readFragments(root);
      if (errors.length) fail(errors);
      if (!fragments.length) fail(['no fragments in changelog.d/']);
      const date = opt('--date') ?? new Date().toISOString().slice(0, 10);
      const file = join(root, 'CHANGELOG.md');
      writeFileSync(file, insertSection(readFileSync(file, 'utf8'), `${tag} — ${date}`, render(fragments)));
      for (const f of fragments) unlinkSync(join(root, 'changelog.d', f.file));
      console.log(`folded ${fragments.length} fragment(s) into CHANGELOG.md as ${tag}; review, then commit`);
      return;
    }
    case 'notes': {
      const body = extractSection(readFileSync(join(root, 'CHANGELOG.md'), 'utf8'), rest[0] ?? '');
      if (body === null) fail([`CHANGELOG.md has no "## ${rest[0]}" section`]);
      console.log(body);
      return;
    }
    default:
      console.error('usage: changelog.mjs new|preview|check|release|notes (see header)');
      process.exit(2);
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main(process.argv.slice(2));
}
