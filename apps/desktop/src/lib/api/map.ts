import { convertFileSrc } from '@tauri-apps/api/core';
import { enrichIdentity } from '../unified';
import { projectCatalogue } from '../metadata';
import { artFromSeed } from '../art';
import type { GameEntry } from '../types';
import type { Edition, LibraryRoot, LibrarySnapshot, Runner } from './contracts';
import { isDesktopRuntime } from './desktop';
import { webToken } from './http';

/** Cartridge/disc systems run by a catalogue emulator (mirrors `catalog/systems/*.json`). */
const SYSTEM_EMULATOR: Record<string, string> = {
  ps3: 'rpcs3', genesis: 'blastem', ds: 'melonds', neogeo: 'fbneo', dreamcast: 'flycast', nes: 'mesen', snes: 'mesen', gb: 'mesen', gbc: 'mesen', pce: 'mesen', n64: 'rmg',
};
/** Backend system id -> UI platform id. */
const PLATFORM: Record<string, GameEntry['platform']> = {
  ps1: 'ps1', ps2: 'ps2', ps3:'ps3', genesis: 'genesis', ds: 'ds', neogeo: 'neogeo', dreamcast: 'dreamcast', nes: 'nes', snes: 'snes', gb: 'gb', gbc: 'gbc', pce: 'tg16', n64: 'n64',
};

export function runnerSupports(runner: Runner, system: string): boolean {
  if (runner.systems) return runner.systems.includes(system);
  // Older fixtures/snapshots carry no capability list.
  if (SYSTEM_EMULATOR[system]) return runner.id === `runner.${SYSTEM_EMULATOR[system]}.marquee`;
  return ['ps1', 'ps2'].includes(system) && (runner.kind === (system === 'ps1' ? 'duckstation' : 'pcsx2') || runner.kind === 'fake');
}

function pickRunner(snap: Pick<LibrarySnapshot, 'runners' | 'settings'>, system = 'ps2', override?: string | null): Runner | null {
  const selected = override ?? snap.settings.selected_runner_ids?.[system];
  if (selected) return snap.runners.find(r => r.id === selected && runnerSupports(r, system)) ?? null;
  const legacy = snap.runners.find(r => r.id === snap.settings.selected_runner_id && r.available && runnerSupports(r, system));
  if (legacy) return legacy;
  const compatible = snap.runners.filter(r => runnerSupports(r, system) && r.available);
  const emulator = SYSTEM_EMULATOR[system];
  return compatible.find(r => emulator ? r.id === `runner.${emulator}.marquee` : r.source === 'marquee') ?? compatible[0] ?? null;
}

export function editionToGame(edition: Edition, snap: LibrarySnapshot): GameEntry {
  const runner = pickRunner(snap, edition.system, edition.runner_override);
  const platform = PLATFORM[edition.system] ?? 'unknown';
  const media = snap.media.find((m) => m.id === edition.media_id);
  const rootOffline = snap.roots.some((r: LibraryRoot) => r.system === edition.system && r.status === 'offline' && !!media && (media.path === r.path || media.path.startsWith(`${r.path}/`))) && !media?.available;
  let setupState: GameEntry['setupState'] = 'runner_missing';
  if (runner?.available) setupState = runner.bios_state;
  else if (runner) setupState = 'runner_missing';
  return {
    id: edition.id,
    runnerOverride: edition.runner_override ?? undefined,
    title: edition.title,
    tagline: edition.serial
      ? `${edition.serial}${edition.region ? ` · ${edition.region}` : ''}`
      : '',
    platform,
    source: 'rom',
    installed: !edition.missing,
    genre: edition.info?.genre ?? '',
    year: edition.info?.year ?? 0,
    developer: edition.info?.developer ?? undefined,
    publisher: edition.info?.publisher ?? undefined,
    description: edition.info?.description ?? undefined,
    descriptionSource: edition.info?.description ? edition.info.source : undefined,
    players: edition.info?.players ?? undefined,
    playtimeMin: edition.playtime_seconds / 60,
    lastPlayed: edition.last_played_at,
    addedAt: edition.imported_at,
    favorite: edition.favorite,
    serial: edition.serial ?? undefined,
    region: edition.region ?? undefined,
    mediaMissing: edition.missing,
    wanted: edition.wanted ?? undefined,
    rootOffline,
    setupState,
    runnerId: runner?.id,
    runnerName: runner?.display_name,
    mediaIssue: media?.skip_reason ?? undefined,
    catalogueMatchSupported: !!media?.available && (media.kind === 'ps1_iso' || media.kind === 'ps2_iso'),
    ratings: { critic: null, criticCount: 0, user: null, steamReviews: null },
    timeToBeat: null,
    achievements: null,
    art: artFromSeed(`${edition.id}:${edition.title}`),
    coverUrl: coverSrc(edition.cover_path),
  };
}

export function coverSrc(path: string | null): string | undefined {
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
  return projectCatalogue(snap.editions.map((edition) => enrichIdentity(editionToGame(edition, snap))), snap);
}

export { pickRunner };
