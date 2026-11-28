/**
 * 開発サーバー（`npm run dev` / `tauri dev`）が言語ファイルを配信できるかの検査。
 *
 * 言語ファイルはリポジトリ直下の `locales/` に有り、SvelteKit 既定の配信許可
 * （`src/`・`.svelte-kit/`・`node_modules/` 等）の範囲外。許可漏れの場合、
 * 開発起動時に `/locales/*.json` が 404 となり、画面の文言が全て欠落する。
 * 本番ビルドは埋込みのため影響無し（`npm run build` 側で別途検査）。
 */
import { createServer, type ViteDevServer } from 'vite';
import { fileURLToPath } from 'node:url';
import { afterAll, beforeAll, describe, expect, it } from 'vitest';

const root = fileURLToPath(new URL('../../..', import.meta.url));
let server: ViteDevServer;
let base: string;

beforeAll(async () => {
  server = await createServer({
    root,
    configFile: `${root}/vite.config.ts`,
    logLevel: 'silent',
    server: { port: 0, strictPort: false, host: '127.0.0.1', hmr: false }
  });
  await server.listen();
  const addr = server.httpServer?.address();
  if (!addr || typeof addr === 'string') throw new Error('開発サーバーの待受先が不明');
  base = `http://127.0.0.1:${addr.port}`;
}, 60_000);

afterAll(async () => {
  await server?.close();
});

describe('開発サーバー：言語ファイルの配信', () => {
  it('i18n モジュールが参照する言語ファイルを全て配信', async () => {
    const res = await fetch(`${base}/src/lib/i18n/index.ts`);
    expect(res.status).toBe(200);
    const code = await res.text();
    const urls = [...code.matchAll(/from\s+["']([^"']*\/locales\/[^"']+\.json[^"']*)["']/g)].map((m) => m[1]);
    // ja.json（基準）と en.json（代替）は必須
    expect(urls.some((u) => u.includes('/ja.json'))).toBe(true);
    expect(urls.some((u) => u.includes('/en.json'))).toBe(true);
    for (const url of urls) {
      const r = await fetch(new URL(url, base));
      expect(r.status, `${url} の配信`).toBe(200);
    }
  });
});
