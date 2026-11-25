import type { Runner, SystemSummary } from './api/contracts';

/** Catalog emulator id, with the old hardcoded table only when a snapshot has no systems. */
export function catalogDefaultEmulator(
  systems: Pick<SystemSummary, 'id' | 'emulator'>[] | undefined,
  system: string,
  fallback: Record<string, string>,
): string | undefined {
  return systems?.find((item) => item.id === system)?.emulator ?? fallback[system];
}

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
