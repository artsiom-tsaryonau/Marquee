declare const __BUILD_ID__: string;

/** Stamped into the bundle at build time; `dev` under the Vite dev server and in tests. */
export const buildId = typeof __BUILD_ID__ === 'string' ? __BUILD_ID__ : 'dev';

/**
 * True when whatever serves this UI now is a different build than the one running here.
 * Catches a tab or window left open across a rebuild, so the surfaces never silently diverge.
 */
export async function uiIsStale(): Promise<boolean> {
  if (buildId === 'dev') return false;
  try {
    const res = await fetch(`/build-id.txt?t=${Date.now()}`, { cache: 'no-store' });
    if (!res.ok) return false;
    const served = (await res.text()).trim();
    return served.length > 0 && served !== buildId;
  } catch {
    return false;
  }
}
