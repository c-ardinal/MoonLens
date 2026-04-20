/**
 * Thin wrapper around SvelteKit `goto` so components that navigate
 * can be unit-tested without pulling the `$app/navigation` virtual
 * module into vitest's resolver. Tests mock this module; production
 * code keeps the same one-line call.
 */
import { goto as sveltekitGoto } from '$app/navigation';

export function goto(path: string): Promise<void> {
  return sveltekitGoto(path);
}
