import type { MusicAlbum } from './api/contracts';
import type { GameEntry } from './types';
import { canonicalId } from './unified';

export function albumArtwork(album: MusicAlbum, games: GameEntry[]) {
  if (album.cover_data_url) return { url: album.cover_data_url, label: album.cover_source || 'Album artwork', fallback: false };
  const game = games.find(g => g.coverUrl && album.game_keys?.includes(canonicalId(g)));
  return game?.coverUrl ? { url: game.coverUrl, label: `Game artwork · ${game.title}`, fallback: true } : null;
}
