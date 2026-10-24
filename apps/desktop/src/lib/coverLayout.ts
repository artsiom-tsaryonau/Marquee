import type { GameEntry, Platform } from './types';

/** Defaults reserve shelf space only; decoded artwork always supplies its ratio. */
export function caseRatio(platform: Platform): number {
  if (['ps1', 'dreamcast', 'ds', '3ds', 'gb', 'gbc', 'gamegear'].includes(platform)) return 1;
  if (['snes', 'n64', 'gba'].includes(platform)) return 1.4;
  return 2 / 3;
}

export function platformShelves(games: GameEntry[]) {
  const groups = new Map<Platform, GameEntry[]>();
  for (const game of games) {
    const list = groups.get(game.platform) ?? [];
    list.push(game); groups.set(game.platform, list);
  }
  return [...groups].sort(([a, x], [b, y]) => a === 'unknown' ? 1 : b === 'unknown' ? -1 : y.length - x.length || a.localeCompare(b));
}
