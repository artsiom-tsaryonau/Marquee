import { artFromSeed } from '../art';
import type { GameEntry } from '../types';
import type { Edition, LibraryRoot, LibrarySnapshot, Runner } from './contracts';

function pickRunner(snap: LibrarySnapshot): Runner | null {
  const selected = snap.settings.selected_runner_id
    ? snap.runners.find((r) => r.id === snap.settings.selected_runner_id)
    : undefined;
  return (
    selected ??
    snap.runners.find((r) => r.kind === 'pcsx2' && r.available) ??
    snap.runners.find((r) => r.available) ??
    snap.runners[0] ??
    null
  );
}

export function editionToGame(edition: Edition, snap: LibrarySnapshot): GameEntry {
  const runner = pickRunner(snap);
  const media = snap.media.find((m) => m.id === edition.media_id);
  const rootOffline = snap.roots.some((r: LibraryRoot) => r.status === 'offline') && !media?.available;
  let setupState: GameEntry['setupState'] = 'runner_missing';
  if (runner?.available) setupState = runner.bios_state;
  else if (runner) setupState = 'runner_missing';
  return {
    id: edition.id,
    title: edition.title,
    tagline: edition.serial
      ? `${edition.serial}${edition.region ? ` · ${edition.region}` : ''}`
      : 'PlayStation 2 disc',
    platform: 'ps2',
    source: 'rom',
    installed: !edition.missing,
    genre: 'PS2',
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
    ratings: { critic: null, criticCount: 0, user: null, steamReviews: null },
    timeToBeat: null,
    achievements: null,
    art: artFromSeed(`${edition.id}:${edition.title}`),
  };
}

export function gamesFromSnapshot(snap: LibrarySnapshot): GameEntry[] {
  return snap.editions.map((edition) => editionToGame(edition, snap));
}

export { pickRunner };
