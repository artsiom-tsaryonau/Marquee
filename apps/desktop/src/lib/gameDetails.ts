export const gameDetailTabs = ['overview', 'ost', 'metadata'] as const;
export type GameDetailTab = typeof gameDetailTabs[number];

export function nextGameDetailTab(current: GameDetailTab, key: string): GameDetailTab {
  if (key === 'Home') return gameDetailTabs[0];
  if (key === 'End') return gameDetailTabs[gameDetailTabs.length - 1];
  const offset = key === 'ArrowRight' ? 1 : key === 'ArrowLeft' ? -1 : 0;
  return gameDetailTabs[(gameDetailTabs.indexOf(current) + offset + gameDetailTabs.length) % gameDetailTabs.length];
}
