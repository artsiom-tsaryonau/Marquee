import { convertFileSrc } from '@tauri-apps/api/core';
import { enrichIdentity } from '../unified';
import { artFromSeed } from '../art';
import type { GameEntry } from '../types';
import type { Edition, LibraryRoot, LibrarySnapshot, Runner } from './contracts';
import { isDesktopRuntime } from './desktop';
import { webToken } from './http';

function pickRunner(snap: Pick<LibrarySnapshot, 'runners' | 'settings'>, system = 'ps2'): Runner | null {
  const kind = system === 'ps1' ? 'duckstation' : 'pcsx2';
  const selectedId = snap.settings.selected_runner_ids?.[system] ?? snap.settings.selected_runner_id;
  const matches = (r: Runner) => r.kind === kind || r.kind === 'fake';
  return snap.runners.find((r) => r.id === selectedId && r.available && matches(r))
    ?? snap.runners.find((r) => r.kind === kind && r.id === 'runner.pcsx2.unpacked' && r.available)
    ?? snap.runners.find((r) => r.kind === kind && r.available)
    ?? snap.runners.find((r) => r.kind === 'fake' && r.available)
    ?? null;
}

export function editionToGame(edition: Edition, snap: LibrarySnapshot): GameEntry {
  const runner = pickRunner(snap, edition.system);
  const platform = edition.system === 'ps1' ? 'ps1' : edition.system === 'ps2' ? 'ps2' : 'unknown';
  const media = snap.media.find((m) => m.id === edition.media_id);
  const rootOffline = snap.roots.some((r: LibraryRoot) => r.system === edition.system && r.status === 'offline' && !!media && (media.path === r.path || media.path.startsWith(`${r.path}/`))) && !media?.available;
  let setupState: GameEntry['setupState'] = 'runner_missing';
  if (runner?.available) setupState = runner.bios_state;
  else if (runner) setupState = 'runner_missing';
  return {
    id: edition.id,
    title: edition.title,
    tagline: edition.serial
      ? `${edition.serial}${edition.region ? ` · ${edition.region}` : ''}`
      : `${platform === 'ps1' ? 'PlayStation' : platform === 'ps2' ? 'PlayStation 2' : 'Unknown platform'} disc`,
    platform,
    source: 'rom',
    installed: !edition.missing,
    genre: edition.system.toUpperCase(),
    year: 0,
    playtimeMin: edition.playtime_seconds / 60,
    lastPlayed: edition.last_played_at,
    addedAt: edition.imported_at,
    favorite: edition.favorite,
    serial: edition.serial ?? undefined,
    region: edition.region ?? undefined,
    mediaMissing: edition.missing,
    rootOffline,
    setupState,
    runnerId: runner?.id,
    runnerName: runner?.display_name,
    mediaIssue: media?.skip_reason ?? undefined,
    ratings: { critic: null, criticCount: 0, user: null, steamReviews: null },
    timeToBeat: null,
    achievements: null,
    art: artFromSeed(`${edition.id}:${edition.title}`),
    coverUrl: coverSrc(edition.cover_path),
  };
}

function coverSrc(path: string | null): string | undefined {
  if (!path) return undefined;
  if (isDesktopRuntime()) {
    try {
      return convertFileSrc(path);
    } catch {
      return undefined;
    }
  }
  // Browser: covers are served by Marquee itself, by file name, with the token.
  const token = webToken();
  const name = path.split('/').pop();
  return token && name ? `/art/${encodeURIComponent(name)}?token=${encodeURIComponent(token)}` : undefined;
}

export function gamesFromSnapshot(snap: LibrarySnapshot): GameEntry[] {
  return snap.editions.map((edition) => enrichIdentity(editionToGame(edition, snap)));
}

export { pickRunner };
