import { afterEach, describe, expect, it } from 'vitest';
import { get } from 'svelte/store';
import {
  __seedForTesting,
  currentSettings,
  readSetting,
  restartNeeded,
  resetToBootSnapshot,
  saveStateStore,
  updateSetting
} from './settingsStore';

/**
 * The save side of the store hits Tauri IPC, which `safeInvoke`
 * short-circuits to a thrown error in the test environment. The
 * tests below pre-seed the store via the test-only seeder so they
 * can exercise the read / mutate / restart-diff logic without
 * depending on backend reachability.
 */

afterEach(() => {
  __seedForTesting(null);
});

describe('settingsStore', () => {
  it('readSetting returns the fallback before settings load', () => {
    __seedForTesting(null);
    expect(readSetting('appearance.theme', 'system')).toBe('system');
  });

  it('updateSetting is a no-op before settings load (no crash)', () => {
    __seedForTesting(null);
    updateSetting('appearance.theme', 'dark');
    expect(readSetting('appearance.theme', 'system')).toBe('system');
  });

  it('readSetting walks dotted paths into the loaded tree', () => {
    __seedForTesting({ appearance: { theme: 'dark', density: 'compact' } });
    expect(readSetting('appearance.theme', 'system')).toBe('dark');
    expect(readSetting('appearance.density', 'normal')).toBe('compact');
    expect(readSetting('missing.key', 7)).toBe(7);
  });

  it('updateSetting deep-mutates and marks the store dirty', () => {
    __seedForTesting({ appearance: { theme: 'system' } });
    updateSetting('appearance.theme', 'dark');
    expect(readSetting('appearance.theme', '')).toBe('dark');
    expect(get(saveStateStore)).toBe('dirty');
  });

  it('updateSetting can create missing intermediate objects', () => {
    __seedForTesting({ appearance: { theme: 'system' } });
    updateSetting('storage.retention_days', 30);
    expect(readSetting('storage.retention_days', 0)).toBe(30);
  });

  it('currentSettings reflects mutations live', () => {
    __seedForTesting({ appearance: { theme: 'system' } });
    updateSetting('appearance.theme', 'light');
    const tree = get(currentSettings) as { appearance: { theme: string } };
    expect(tree.appearance.theme).toBe('light');
  });

  it('restartNeeded stays false until a restart-required field changes', () => {
    // Boot-time snapshot is established by `replaceSettings` because
    // the store hasn't been initialised through `ensureSettingsLoaded`
    // in this test. We flip the field via `updateSetting` and assert
    // the diff turns truthy. Other field changes don't trigger it.
    __seedForTesting({
      storage: { encrypt_at_rest: false, retention_days: 90 },
      privacy: { log_level: 'info' }
    });
    expect(get(restartNeeded)).toBe(false);
    updateSetting('storage.retention_days', 30);
    expect(get(restartNeeded)).toBe(false);
    updateSetting('storage.encrypt_at_rest', true);
    expect(get(restartNeeded)).toBe(true);
  });

  it('resetToBootSnapshot clears the restart-needed banner', () => {
    __seedForTesting({
      privacy: { log_level: 'info' },
      storage: { encrypt_at_rest: false }
    });
    updateSetting('privacy.log_level', 'debug');
    expect(get(restartNeeded)).toBe(true);
    resetToBootSnapshot();
    expect(get(restartNeeded)).toBe(false);
    expect(readSetting('privacy.log_level', '')).toBe('info');
  });
});
