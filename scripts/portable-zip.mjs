#!/usr/bin/env node
// Windows portable package: the release executable plus licence files in
// one `MoonLens/` folder, zipped. MoonLens ships no Windows installer;
// users unpack the zip anywhere and start `MoonLens.exe`.
//
// Usage: node scripts/portable-zip.mjs <cargo release dir> [out dir]
//   e.g. node scripts/portable-zip.mjs D:/t/release dist
// Run after `tauri build --no-bundle` and `scripts/third-party-licenses.mjs`.
// Prints the zip's path.

import { execFileSync } from 'node:child_process';
import { copyFileSync, existsSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');

/** Folder inside the zip, and the executable's name in it. */
export const FOLDER = 'MoonLens';
export const EXE = 'MoonLens.exe';

export function zipName(version) {
  return `MoonLens_${version}_x64-portable.zip`;
}

/** `README.txt` of the package (CRLF, for Notepad). */
export function readme(version) {
  return [
    `MoonLens ${version}（Windows ポータブル版）`,
    '',
    '■ 起動',
    `  本フォルダーを任意の場所へ配置し、${EXE} を実行。インストール不要。`,
    '  未署名のため、SmartScreen 表示時は「詳細情報 → 実行」を選択。',
    '  Microsoft Edge WebView2 ランタイムが必要（Windows 11 は標準搭載）。',
    '',
    '■ 更新',
    '  アプリ終了後、新版のフォルダーと置換。設定・履歴・モデルは継続利用。',
    '',
    '■ データ保存先',
    '  設定・履歴・プラグイン：%APPDATA%\\io.moonlens.app',
    '  モデル・ログ：%LOCALAPPDATA%\\io.moonlens.app',
    '',
    '■ 削除',
    '  アプリ終了後、本フォルダーと上記 2 フォルダーを削除。レジストリは不使用。',
    '',
    '■ ライセンス',
    '  本体：LICENSE（MIT）',
    '  同梱ライブラリ・モデル：THIRD_PARTY_LICENSES.html',
    '',
    'https://github.com/c-ardinal/MoonLens',
    '',
  ].join('\r\n');
}

/**
 * Lay the package out under `<outDir>/MoonLens/`. Returns the staged
 * file names. The executable is `moonlens.exe` (cargo package name) in
 * the release dir; DLLs next to it, if a dependency ever adds one, come
 * along.
 */
export function stage({ releaseDir, outDir, version, repoRoot = root }) {
  const exe = ['moonlens.exe', EXE].map((n) => join(releaseDir, n)).find(existsSync);
  if (!exe) throw new Error(`no moonlens.exe in ${releaseDir} (run \`tauri build --no-bundle\` first)`);
  const licences = join(repoRoot, 'THIRD_PARTY_LICENSES.html');
  if (!existsSync(licences)) {
    throw new Error('THIRD_PARTY_LICENSES.html missing (run scripts/third-party-licenses.mjs first)');
  }
  const dir = join(outDir, FOLDER);
  rmSync(dir, { recursive: true, force: true });
  mkdirSync(dir, { recursive: true });
  copyFileSync(exe, join(dir, EXE));
  for (const dll of readdirSync(releaseDir).filter((n) => /\.dll$/i.test(n))) {
    copyFileSync(join(releaseDir, dll), join(dir, dll));
  }
  copyFileSync(join(repoRoot, 'LICENSE'), join(dir, 'LICENSE'));
  copyFileSync(licences, join(dir, 'THIRD_PARTY_LICENSES.html'));
  // BOM: Notepad on Windows 10 otherwise reads the file as ANSI.
  writeFileSync(join(dir, 'README.txt'), '\uFEFF' + readme(version));
  return readdirSync(dir).sort();
}

function zip(outDir, name) {
  const out = join(outDir, name);
  rmSync(out, { force: true });
  if (process.platform === 'win32') {
    execFileSync('7z', ['a', '-tzip', '-mx=9', name, FOLDER], { cwd: outDir, stdio: 'inherit' });
  } else {
    execFileSync('zip', ['-r', '-9', '-q', name, FOLDER], { cwd: outDir, stdio: 'inherit' });
  }
  return out;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const [releaseDir, outDir = 'dist'] = process.argv.slice(2);
  if (!releaseDir) {
    console.error('usage: node scripts/portable-zip.mjs <cargo release dir> [out dir]');
    process.exit(2);
  }
  const { version } = JSON.parse(readFileSync(join(root, 'src-tauri/tauri.conf.json'), 'utf8'));
  const files = stage({ releaseDir: resolve(releaseDir), outDir: resolve(outDir), version });
  console.error(`staged ${FOLDER}/: ${files.join(', ')}`);
  console.log(zip(resolve(outDir), zipName(version)));
}
