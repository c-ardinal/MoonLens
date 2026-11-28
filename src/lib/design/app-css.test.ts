/**
 * Tailwind のクラス名走査の範囲の検査。
 *
 * 既定の走査対象：作業ディレクトリ全体のうち .gitignore 対象外の全ファイル。開発者の作業
 * ディレクトリ内に未追跡の大容量ファイル（llama.cpp の複製、別名のビルド出力等）が有る場合、
 * 開発起動時に全て読込み、本体ウィンドウの初回表示が数分遅延（実例：約 3 分）。
 * `src/app.css` の `source('.')` で走査対象を `src/` へ限定。
 */
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import postcss from 'postcss';
import tailwind from '@tailwindcss/postcss';
import { afterAll, describe, expect, it } from 'vitest';

const root = fileURLToPath(new URL('../../..', import.meta.url));
const appCss = join(root, 'src/app.css');
// src/ 外・未追跡のファイルへのみ記述したクラス名（任意値のため src/ 側との偶然の一致は無し）。
// 本ファイルも走査対象のため、クラス名を分割して記述。
const OUTSIDE = ['translate', 'x', '[1234.5px]'].join('-');
const probeDir = mkdtempSync(join(root, 'tailwind-scan-probe-'));
writeFileSync(join(probeDir, 'notes.txt'), `<div class="${OUTSIDE}"></div>\n`);

afterAll(() => rmSync(probeDir, { recursive: true, force: true }));

describe('app.css：クラス名の走査範囲', () => {
  it('src/ 外のファイルは走査対象外、src/ 内は対象', async () => {
    const out = await postcss([tailwind({ base: root })]).process(readFileSync(appCss, 'utf8'), { from: appCss });
    expect(out.css).not.toContain('1234\\.5px');
    // src/ 内で使用中のクラス（AppNav 等）は生成
    expect(out.css).toMatch(/\.inline-flex\s*\{/);
  }, 60_000);
});
