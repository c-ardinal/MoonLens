import { defineConfig } from 'vitest/config';

/**
 * Vitest isolates from the SvelteKit Vite config so runs stay fast and
 * do not require the sveltekit adapter. Tests live next to the helpers
 * they cover under `src/lib/**` and compile with the same tsconfig.
 */
export default defineConfig({
  resolve: {
    alias: {
      $lib: new URL('./src/lib', import.meta.url).pathname
    }
  },
  test: {
    include: ['src/lib/**/*.test.ts'],
    environment: 'node',
    reporters: ['default']
  }
});
