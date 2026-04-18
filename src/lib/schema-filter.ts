/**
 * Fuzzy-ish path filter for the auto-generated settings form. Pulled out
 * of SchemaForm.svelte so the "does this path match the user's query?"
 * rule can be changed (e.g. to support scoring, CJK segmentation) with
 * confidence that the UI will keep working.
 */

/**
 * Return true when `path` (a dot-joined settings path like
 * "providers.translate.chain") should be visible under `query`. An empty
 * query shows everything; otherwise matching is case-insensitive and
 * requires every whitespace-separated token to appear somewhere.
 */
export function matchesQuery(path: string, query: string): boolean {
  const q = query.trim().toLowerCase();
  if (!q) return true;
  const haystack = path.toLowerCase();
  // Every token must appear; order does not matter so users can type
  // "translate chain" and find `providers.translate.chain`.
  return q
    .split(/\s+/)
    .every((token) => token.length === 0 || haystack.includes(token));
}

/**
 * Flatten a nested JSON Schema `properties` tree into a list of dotted
 * paths. Used by the settings page to offer keyboard-navigable results.
 */
export function flattenPaths(schema: unknown, prefix = ''): string[] {
  const out: string[] = [];
  if (!schema || typeof schema !== 'object') return out;
  const obj = schema as Record<string, unknown>;
  const props = (obj.properties ?? {}) as Record<string, unknown>;
  for (const [key, child] of Object.entries(props)) {
    const path = prefix ? `${prefix}.${key}` : key;
    out.push(path);
    out.push(...flattenPaths(child, path));
  }
  return out;
}
