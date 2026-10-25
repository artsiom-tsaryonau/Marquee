import type { Runner } from './api/contracts';

/** One row per runner. The default row clears a per-game override. */
export function emulatorChoices(runners: Runner[], defaultId?: string) {
  const seen = new Set<string>();
  return runners.filter(r => !seen.has(r.id) && !!seen.add(r.id)).map(r => ({
    id: r.id,
    value: r.id === defaultId ? '' : r.id,
    label: `${r.display_name}${r.id === defaultId ? ' (default)' : ''}${r.available ? '' : ' · unavailable'}`,
    available: r.available,
  }));
}
