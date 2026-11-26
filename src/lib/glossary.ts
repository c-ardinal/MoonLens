/**
 * Glossary entries as stored in `translation.glossary` (see
 * `GlossaryEntry` in src-tauri/src/settings/schema.rs). The backend uses an
 * entry only when its source term occurs in the text and `lang` matches
 * the target language ("" = any). Blank entries are ignored there.
 */
export type GlossaryEntry = { source: string; target: string; lang: string };

export function addEntry(list: GlossaryEntry[], lang: string): GlossaryEntry[] {
  return [...list, { source: '', target: '', lang }];
}

export function updateEntry(list: GlossaryEntry[], index: number, patch: Partial<GlossaryEntry>): GlossaryEntry[] {
  return list.map((e, i) => (i === index ? { ...e, ...patch } : e));
}

export function removeEntry(list: GlossaryEntry[], index: number): GlossaryEntry[] {
  return list.filter((_, i) => i !== index);
}

/** Put `entry` back at `index` (clamped to the list), e.g. to undo a removal. */
export function insertEntry(list: GlossaryEntry[], index: number, entry: GlossaryEntry): GlossaryEntry[] {
  const at = Math.max(0, Math.min(index, list.length));
  return [...list.slice(0, at), entry, ...list.slice(at)];
}

/** A removed row and the position it returns to in the current list. */
export interface RemovedRow {
  readonly entry: GlossaryEntry;
  index: number;
}

/**
 * Removes rows and puts them back in their original order, whichever order
 * the removals are undone in. Rows have no id, so a position recorded at
 * removal goes stale as other rows are removed or restored; the log keeps
 * every position current.
 */
export function createRemovalLog() {
  /** Removed rows, in list order. */
  const log: RemovedRow[] = [];
  return {
    remove(list: GlossaryEntry[], index: number): { list: GlossaryEntry[]; row: RemovedRow } {
      const row: RemovedRow = { entry: list[index], index };
      // Rows removed earlier from before this one stay ahead of it.
      const at = log.findIndex((r) => r.index > index);
      for (const r of log) if (r.index > index) r.index -= 1;
      log.splice(at < 0 ? log.length : at, 0, row);
      return { list: removeEntry(list, index), row };
    },
    /** No-op for a row that was restored already. */
    restore(list: GlossaryEntry[], row: RemovedRow): GlossaryEntry[] {
      const at = log.indexOf(row);
      if (at < 0) return list;
      log.splice(at, 1);
      for (const r of log.slice(at)) r.index += 1;
      return insertEntry(list, row.index, row.entry);
    }
  };
}

/** A row with neither term filled in: nothing is lost by removing it. */
export function isBlankEntry(e: GlossaryEntry): boolean {
  return e.source.trim() === '' && e.target.trim() === '';
}

/** Drop rows left completely blank (an added row never filled in). */
export function compactGlossary(list: GlossaryEntry[] | null | undefined): GlossaryEntry[] {
  return (list ?? []).filter((e) => !isBlankEntry(e));
}
