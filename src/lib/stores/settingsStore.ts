/**
 * Global settings store.
 *
 * The settings page and several "ambient" surfaces (layout shell for
 * theme + accent, sidebar engine status, audio model display, …) all
 * need the current settings tree. Loading them per-component leads to
 * three problems we hit in earlier builds:
 *
 *   1. Theme / accent / density changes didn't reflect immediately
 *      because the layout only read settings on its initial mount.
 *   2. The sidebar engine card showed stale provider names after a
 *      save.
 *   3. The "this field needs a restart" UX had no boot-time baseline
 *      to diff against — every refresh would reset the diff.
 *
 * This module owns the lifecycle: it loads once, exposes a writable
 * snapshot, and remembers the boot-time copy so callers can answer
 * "did the user touch a restart-required field this session?".
 */

import { writable, derived, get, type Readable } from 'svelte/store';
import { getSettings, setSettings, type Settings } from '$lib/ipc';

// eslint-disable-next-line @typescript-eslint/no-explicit-any
type Tree = any;

interface State {
  current: Tree | null;
  /** Snapshot taken on first load — used by the restart-needed diff. */
  bootSnapshot: Tree | null;
  loading: boolean;
  /** Mirrors the SaveIndicator's accepted states. We start as
   *  `'saved'` because no edits have been made yet; the load process
   *  itself does not count as a dirty state. */
  saveState: 'dirty' | 'saving' | 'saved' | 'error';
  errorMessage: string | null;
}

const initialState: State = {
  current: null,
  bootSnapshot: null,
  loading: false,
  saveState: 'saved',
  errorMessage: null
};

const state = writable<State>(initialState);

let saveTimer: ReturnType<typeof setTimeout> | undefined;
let initialized = false;

/** Idempotent loader. Layout calls this on mount; subsequent calls
 *  are no-ops so settings are loaded exactly once per session. */
export async function ensureSettingsLoaded(): Promise<void> {
  if (initialized) return;
  initialized = true;
  state.update((s) => ({ ...s, loading: true }));
  try {
    const current = (await getSettings()) as Settings;
    state.update((s) => ({
      ...s,
      current: deepClone(current),
      bootSnapshot: deepClone(current),
      loading: false,
      saveState: 'saved',
      errorMessage: null
    }));
  } catch (e) {
    state.update((s) => ({
      ...s,
      loading: false,
      saveState: 'error',
      errorMessage: String(e)
    }));
    initialized = false;
  }
}

/** Read-only view of the current settings tree. */
export const currentSettings: Readable<Tree | null> = derived(
  state,
  ($s) => $s.current
);

export const settingsLoading: Readable<boolean> = derived(
  state,
  ($s) => $s.loading
);

export const saveStateStore: Readable<State['saveState']> = derived(
  state,
  ($s) => $s.saveState
);

export const saveErrorMessage: Readable<string | null> = derived(
  state,
  ($s) => $s.errorMessage
);

/** Path-based reader. Returns the fallback when settings are not yet
 *  loaded or any intermediate node is missing. Designed to be called
 *  inline from templates — Svelte's reactive system tracks reads on
 *  the store so the result re-evaluates on changes. */
export function readSetting<T>(path: string, fallback: T): T {
  const s = get(state).current;
  if (!s) return fallback;
  const parts = path.split('.');
  let node: unknown = s;
  for (const key of parts) {
    if (node === null || node === undefined || typeof node !== 'object') {
      return fallback;
    }
    node = (node as Record<string, unknown>)[key];
  }
  return (node === undefined || node === null ? fallback : node) as T;
}

/** Deep mutator + debounced save. Walks the existing tree so Svelte
 *  reactivity is preserved even for deeply-nested writes, then defers
 *  the IPC by `delayMs` so rapid drags on a slider coalesce. */
export function updateSetting(path: string, value: unknown, delayMs = 300) {
  state.update((s) => {
    if (!s.current) return s;
    const next = deepClone(s.current);
    const parts = path.split('.');
    let node = next;
    for (let i = 0; i < parts.length - 1; i++) {
      const key = parts[i];
      if (
        node[key] === undefined ||
        node[key] === null ||
        typeof node[key] !== 'object'
      ) {
        node[key] = {};
      }
      node = node[key];
    }
    node[parts[parts.length - 1]] = value;
    return { ...s, current: next, saveState: 'dirty' };
  });
  if (saveTimer) clearTimeout(saveTimer);
  saveTimer = setTimeout(saveNow, delayMs);
}

/** Replace the whole settings tree (used when applying a preset). */
export function replaceSettings(next: Tree, delayMs = 0) {
  state.update((s) => ({
    ...s,
    current: deepClone(next),
    saveState: 'dirty'
  }));
  if (saveTimer) clearTimeout(saveTimer);
  saveTimer = setTimeout(saveNow, delayMs);
}

/**
 * Test-only seeder. Bypasses the IPC-backed loader so tests can put
 * the store in a known state without booting the Tauri runtime.
 * Resets the `initialized` flag so a real `ensureSettingsLoaded` can
 * run again in production (this hook is unused outside tests).
 */
export function __seedForTesting(tree: Tree | null) {
  state.set({
    current: tree === null ? null : deepClone(tree),
    bootSnapshot: tree === null ? null : deepClone(tree),
    loading: false,
    saveState: 'saved',
    errorMessage: null
  });
  if (saveTimer) clearTimeout(saveTimer);
  initialized = true;
}

export async function saveNow(): Promise<void> {
  const snapshot = get(state).current;
  if (!snapshot) return;
  state.update((s) => ({ ...s, saveState: 'saving', errorMessage: null }));
  try {
    await setSettings(deepClone(snapshot));
    state.update((s) => ({
      ...s,
      saveState: 'saved'
    }));
  } catch (e) {
    state.update((s) => ({
      ...s,
      saveState: 'error',
      errorMessage: String(e)
    }));
  }
}

/** Roll the in-memory tree back to the boot-time snapshot so the
 *  restart banner clears, then save. */
export function resetToBootSnapshot() {
  state.update((s) => {
    if (!s.bootSnapshot) return s;
    return {
      ...s,
      current: deepClone(s.bootSnapshot),
      saveState: 'dirty'
    };
  });
  void saveNow();
}

/** Settings keys (dotted paths) whose effect requires a full restart
 *  because the value is consumed once at boot. The settings page
 *  surfaces a banner when any of these has been changed since the
 *  current process started. */
export const RESTART_REQUIRED_PATHS: string[] = [
  'storage.encrypt_at_rest',
  'privacy.log_level',
  'appearance.font_family_ui',
  'appearance.font_family_mono'
];

/** Reactive boolean: did the user change any restart-required field
 *  since the process booted? */
export const restartNeeded: Readable<boolean> = derived(state, ($s) => {
  if (!$s.current || !$s.bootSnapshot) return false;
  for (const path of RESTART_REQUIRED_PATHS) {
    const a = pluck($s.current, path);
    const b = pluck($s.bootSnapshot, path);
    if (!shallowEqual(a, b)) return true;
  }
  return false;
});

function pluck(tree: Tree, path: string): unknown {
  const parts = path.split('.');
  let node: unknown = tree;
  for (const key of parts) {
    if (node === null || node === undefined || typeof node !== 'object') {
      return undefined;
    }
    node = (node as Record<string, unknown>)[key];
  }
  return node;
}

function shallowEqual(a: unknown, b: unknown): boolean {
  if (a === b) return true;
  if (typeof a !== typeof b) return false;
  if (a === null || b === null) return false;
  if (typeof a === 'object') return JSON.stringify(a) === JSON.stringify(b);
  return false;
}

function deepClone<T>(value: T): T {
  // Plain JSON deep clone: settings are serialisable by definition
  // (they round-trip through TOML/JSON for persistence) and this
  // sidesteps Svelte $state proxy quirks that bit `structuredClone`
  // in earlier builds.
  return JSON.parse(JSON.stringify(value)) as T;
}
