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

/** Drop rows left completely blank (an added row never filled in). */
export function compactGlossary(list: GlossaryEntry[] | null | undefined): GlossaryEntry[] {
  return (list ?? []).filter((e) => e.source.trim() !== '' || e.target.trim() !== '');
}
