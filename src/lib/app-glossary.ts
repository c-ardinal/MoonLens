/**
 * Glossaries of single applications, as stored in
 * `translation.app_glossaries` (see `AppGlossary` in
 * src-tauri/src/settings/schema.rs). The backend adds an application's
 * terms to the common glossary for text translated from that application.
 */
import { compactGlossary, isBlankEntry, type GlossaryEntry } from './glossary';

export type AppGlossary = { app: string; name: string; glossary: GlossaryEntry[] };

/** Base name of an executable (a file name or a path) without `.exe`, as written. */
export function appName(executable: string): string {
  const base = (executable.trim().split(/[/\\]/).pop() ?? '').trim();
  return base.replace(/\.exe$/i, '').trim();
}

/** Key of an application: same rule as `app_identity::normalize_key`. */
export function appKey(executable: string): string {
  return appName(executable).toLowerCase();
}

/**
 * Add the application whose executable is `executable`. `key` is null when
 * the name names nothing; an application listed already is not added twice.
 */
export function addApp(list: AppGlossary[], executable: string): { list: AppGlossary[]; key: string | null } {
  const key = appKey(executable);
  if (!key) return { list, key: null };
  if (list.some((a) => a.app === key)) return { list, key };
  return { list: [...list, { app: key, name: appName(executable), glossary: [] }], key };
}

export function removeApp(list: AppGlossary[], key: string): AppGlossary[] {
  return list.filter((a) => a.app !== key);
}

/** Put `app` back at `index` (clamped), unless it is listed again already. */
export function insertApp(list: AppGlossary[], index: number, app: AppGlossary): AppGlossary[] {
  if (list.some((a) => a.app === app.app)) return list;
  const at = Math.max(0, Math.min(index, list.length));
  return [...list.slice(0, at), app, ...list.slice(at)];
}

/** The entries of application `key`; undefined when it is not listed. */
export function glossaryOf(list: AppGlossary[], key: string): GlossaryEntry[] | undefined {
  return list.find((a) => a.app === key)?.glossary;
}

export function setGlossary(list: AppGlossary[], key: string, glossary: GlossaryEntry[]): AppGlossary[] {
  return list.map((a) => (a.app === key ? { ...a, glossary } : a));
}

/** An application without a filled-in term: nothing is lost by removing it. */
export function isBlankApp(app: AppGlossary): boolean {
  return app.glossary.every(isBlankEntry);
}

/** Drop blank rows from every application (the applications stay listed). */
export function compactAppGlossaries(list: AppGlossary[] | null | undefined): AppGlossary[] {
  return (list ?? []).map((a) => ({ ...a, glossary: compactGlossary(a.glossary) }));
}
