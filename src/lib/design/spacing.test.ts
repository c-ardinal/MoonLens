import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';

/**
 * 余白の 4px 格子：メインウィンドウの余白（padding・margin・gap）は
 * 余白トークン（--ml-sp-N／--ml-page-*）で指定。px の直接指定は、文字サイズ設定の
 * 拡縮後に 4px の倍数から外れるため禁止。
 */
const FILES = [
  'src/lib/design/pixel.css',
  'src/lib/components/AppNav.svelte',
  'src/lib/components/SettingField.svelte',
  'src/lib/components/SettingRow.svelte',
  'src/routes/+page.svelte',
  'src/routes/settings/+page.svelte',
  'src/routes/setup/+page.svelte'
];
const DECL = /^\s*((?:padding|margin|gap|row-gap|column-gap)(?:-[a-z]+)*)\s*:\s*([^;]+);/gm;

/** 検査対象の CSS：Svelte は style 部、pixel.css は余白トークン定義より後。 */
function css(file: string): string {
  const src = readFileSync(file, 'utf8');
  if (file.endsWith('.svelte')) return /<style[^>]*>([\s\S]*?)<\/style>/.exec(src)?.[1] ?? '';
  return src.slice(src.indexOf('/* ── 共通部品 ── */'));
}

describe('余白の 4px 格子', () => {
  it.each(FILES)('%s：余白は px 直接指定なし', (file) => {
    const raw = [...css(file).matchAll(DECL)]
      // 部品内部の位置合せ（calc 内の寸法差）は対象外
      .filter(([, , value]) => /(?<![\w.-])-?\d*\.?\d+px/.test(value.replace(/calc\(\([^)]*\)[^)]*\)/g, '')))
      .map(([, prop, value]) => `${prop}: ${value}`);
    expect(raw).toEqual([]);
  });

  it('余白トークンは 1〜16 段（4〜64px）を定義', () => {
    const src = readFileSync('src/lib/design/pixel.css', 'utf8');
    for (let n = 1; n <= 16; n++)
      expect(src).toContain(`--ml-sp-${n}: calc(max(4px, round(${n * 4}px * var(--ml-z), 4px)) / var(--ml-z));`);
  });
});
