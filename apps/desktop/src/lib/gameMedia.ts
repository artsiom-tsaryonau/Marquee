import type { GameEntry } from './types';

export function gameMedia(game: GameEntry) {
  const items: { url: string; caption: string; kind: 'hero' | 'screenshot' | 'cover' }[] = [];
  if (game.heroUrl) items.push({ url: game.heroUrl, caption: `${game.title} · featured artwork`, kind: 'hero' });
  for (const shot of game.screenshots ?? []) if (!items.some(i => i.url === shot.url)) items.push({ ...shot, kind: 'screenshot' });
  if (game.coverUrl && !items.some(i => i.url === game.coverUrl)) items.push({ url: game.coverUrl, caption: `${game.title} · box cover`, kind: 'cover' });
  return items;
}
