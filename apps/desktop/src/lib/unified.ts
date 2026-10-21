import type { GameEntry, Platform, PortEntry } from './types';
import { artFromSeed } from './art';

export type LaunchMethod = 'emulation' | 'native' | 'compatibility' | 'reference';
export const methodLabel: Record<LaunchMethod, string> = {
  emulation: 'Emulation', native: 'Native port', compatibility: 'PC / compatibility', reference: 'Catalogue only',
};
export function launchMethod(game: GameEntry): LaunchMethod {
  if (game.source === 'catalogue') return 'reference';
  return game.launchMethod ?? (game.source === 'rom' ? 'emulation' : game.source === 'port' ? 'native' : 'compatibility');
}
export function canonicalId(game: GameEntry): string { return game.canonicalId ?? game.id; }
export function editionKey(game: GameEntry): string { return game.editionKey ?? game.id; }
export function editionLabel(game: GameEntry): string { return game.editionLabel ?? 'Original'; }
export function groupGames(games: GameEntry[], preferred: Record<string, string> = {}): { game: GameEntry; entries: GameEntry[] }[] {
  const groups = new Map<string, { game: GameEntry; entries: GameEntry[] }>();
  for (const game of games) {
    const key = canonicalId(game);
    const group = groups.get(key);
    if (group) { group.entries.push(game); if (preferred[key] === game.id) group.game = game; }
    else groups.set(key, { game, entries: [game] });
  }
  return [...groups.values()];
}
export function matchesVariant(game: GameEntry, platform: Platform | '', method: LaunchMethod | ''): boolean {
  return (!platform || game.platform === platform) && (!method || launchMethod(game) === method);
}

// Explicit identities only: titles, punctuation, and filenames never merge games.
// SCUS-97124 is the US retail Precursor Legacy disc, also accepted by OpenGOAL.
export function enrichIdentity(game: GameEntry): GameEntry {
  if (game.serial !== 'SCUS-97124' && game.id !== 'port:opengoal') return game;
  return { ...game, canonicalId: 'jak-and-daxter-tpl', editionKey: 'jak-tpl-original', editionLabel: 'Original · The Precursor Legacy',
    coverUrl: game.coverUrl ?? '/reference/jak/cover.jpg', heroUrl: '/reference/jak/hero.webp',
    screenshots: [
      { url: '/reference/jak/misty-island.avif', caption: 'Misty Island · OpenGOAL promotional capture' },
      { url: '/reference/jak/geyser-rock.avif', caption: 'Geyser Rock · OpenGOAL promotional capture' },
      { url: '/reference/jak/subtitles.avif', caption: 'In-game subtitles · OpenGOAL promotional capture' },
    ], mediaCredit: 'PS2 cover: xlenore/ps2-covers. Screenshots: OpenGOAL promotional gallery; they depict the native port.',
    mediaSource: 'https://opengoal.dev/docs/gallery/jak1/promo',
  };
}
export function nativePortGame(port: PortEntry): GameEntry {
  return enrichIdentity({ id: `port:${port.id}`, title: port.id === 'opengoal' ? 'Jak and Daxter: The Precursor Legacy' : port.game,
    tagline: `${port.name} · ${port.requirement}`, platform: 'pc-linux', source: 'port', installed: port.status !== 'not-installed',
    genre: port.family, year: 0, playtimeMin: 0, lastPlayed: null, addedAt: '', favorite: false,
    ratings: { critic: null, criticCount: 0, user: null, steamReviews: null }, timeToBeat: null, achievements: null,
    portStatus: port.status, art: artFromSeed(`port:${port.id}`), launchMethod: 'native',
  });
}
