import type { LutrisGame } from './api/contracts';
import type { GameEntry } from './types';
import { artFromSeed } from './art';
export function lutrisToGame(game: LutrisGame): GameEntry {
  const source = ({ gog: 'gog', egs: 'epic', steam: 'steam' } as const)[game.service as 'gog' | 'egs' | 'steam'] ?? 'lutris';
  return {
    id: game.id, title: game.name, tagline: game.installed ? 'Managed by Lutris' : 'Cached store library · install through Lutris',
    source, platform: game.platform as GameEntry['platform'], installed: game.installed,
    genre: game.runner ?? 'Store library', year: 0, playtimeMin: game.playtime_minutes,
    lastPlayed: null, addedAt: '', favorite: false, mediaMissing: game.installed && !game.available,
    launchMethod: game.runner === 'linux' ? 'native' : 'compatibility',
    ratings: { critic: null, criticCount: 0, user: null, steamReviews: null }, timeToBeat: null, achievements: null,
    art: artFromSeed(game.id), lutris: { service: game.service, storeId: game.store_id, runner: game.runner },
  };
}
