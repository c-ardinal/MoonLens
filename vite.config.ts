import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

export default defineConfig(async () => ({
  plugins: [sveltekit()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    // 言語ファイル（リポジトリ直下 `locales/`）は SvelteKit 既定の配信許可の範囲外。
    // 許可漏れの場合、開発起動時に `/locales/*.json` が 404（`src/lib/i18n/dev-server.test.ts`）。
    fs: { allow: ['locales'] },
    watch: { ignored: ['**/src-tauri/**'] }
  },
  envPrefix: ['VITE_', 'TAURI_']
}));
