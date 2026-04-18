import { fileURLToPath } from 'node:url';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { svelteTesting } from '@testing-library/svelte/vite';
import { defineConfig, type UserConfig } from 'vitest/config';

// Two test projects live side by side:
//
// - logic: pure TS under `src/lib/<glob>.test.ts`, runs in a Node
//   environment for speed.
// - dom:  Svelte component render tests under `src/<glob>.svelte.test.ts`,
//   runs in happy-dom with `@testing-library/svelte` so we can mount
//   components and assert rendered output.
//
// `fileURLToPath` gives us a platform-correct `$lib` alias (no leading
// `/` on Windows), so tests resolve on every CI runner.

const libAlias = fileURLToPath(new URL('./src/lib', import.meta.url));

const domProject: UserConfig = {
  plugins: [svelte({ hot: false }), svelteTesting()],
  resolve: { alias: { $lib: libAlias } },
  test: {
    name: 'dom',
    include: ['src/**/*.svelte.test.ts'],
    environment: 'happy-dom',
    globals: true,
    setupFiles: ['./vitest.setup.ts']
  }
};

const logicProject: UserConfig = {
  resolve: { alias: { $lib: libAlias } },
  test: {
    name: 'logic',
    include: ['src/lib/**/*.test.ts'],
    // Svelte component render tests end in `.svelte.test.ts` and live
    // under the same tree; they belong to the dom project, so keep them
    // out of the node-environment logic runner.
    exclude: ['src/**/*.svelte.test.ts', 'node_modules/**'],
    environment: 'node'
  }
};

export default defineConfig({
  test: {
    projects: [logicProject, domProject],
    reporters: ['default']
  }
});
