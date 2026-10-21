import type { CatalogueRecord, CatalogueVariant, LibrarySnapshot } from './api/contracts';
import type { GameEntry, Platform } from './types';
import { artFromSeed } from './art';
import { platformLabel } from './labels';

export const regionValues = (value?: string): string[] => value?.split(/\s*[,|]\s*/).filter(Boolean) ?? [];
export const languageValues = (value: string): string[] => value.split(/[\s,|]+/).filter((s) => !!s && s !== '-');
export function providerScore(record?: CatalogueRecord): number | null {
  if (!record) return null;
  if (record.ratings.overall !== null) return record.ratings.overall;
  const score = /^\d+(\.\d+)?$/.test(record.rating) ? Number(record.rating) : NaN;
  return Number.isFinite(score) && score >= 0 && score <= 10 ? score : null;
}
export function versions(game: GameEntry): string[] {
  return game.revision ? [game.revision] : [...new Set([...(game.metadata?.media.map((m) => m.version) ?? []), game.metadata?.version ?? ''].filter((s) => !!s && s !== '-'))];
}
export function metadataMatches(game: GameEntry, region: string, language: string, revision: string, players: string): boolean {
  if (players && String(game.players ?? '') !== players) return false;
  if (game.source === 'catalogue' && game.metadata?.media.length) {
    return game.metadata.media.some((media) =>
      (!region || (media.regions?.length ? media.regions : regionValues(game.region)).includes(region))
      && (!language || (media.languages?.length ? media.languages : game.languages ?? []).includes(language))
      && (!revision || media.version === revision));
  }
  return (!region || regionValues(game.region).includes(region)) && (!language || !!game.languages?.includes(language))
    && (!revision || versions(game).includes(revision)) && (!players || String(game.players ?? '') === players);
}
export function cataloguePlatform(system: string): Platform {
  if (system === 'pce') return 'tg16';
  return Object.hasOwn(platformLabel, system) ? system as Platform : 'unknown';
}
export function withMetadata(game: GameEntry, record: CatalogueRecord, media?: CatalogueVariant, method?: string): GameEntry {
  return { ...game, metadata: record, metadataMediaId: media?.media_id, metadataMatch: method,
    region: media?.regions?.length ? media.regions.join(' | ') : media ? game.region : [...new Set([...regionValues(record.regions), ...record.media.flatMap(m => m.regions ?? [])])].join(' | ') || game.region,
    languages: media?.languages?.length ? media.languages : media ? game.languages : [...new Set([...languageValues(record.languages), ...record.media.flatMap(m => m.languages ?? [])])], revision: media?.version,
    players: record.players ?? undefined, year: game.year || record.year || 0,
    coverUrl: game.coverUrl ?? record.cover?.data_url,
  };
}
export function projectCatalogue(games: GameEntry[], snap: LibrarySnapshot): GameEntry[] {
  const catalogue = snap.catalogue;
  if (!catalogue) return games;
  const local = games.map((game) => {
    const link = catalogue.links.find((l) => l.edition_id === game.id && l.valid);
    const record = link && catalogue.records.find((r) => r.id === link.record_id);
    const media = record?.media.find((m) => m.media_id === link?.media_id);
    return record && media ? withMetadata(game, record, media, link?.method) : game;
  });
  // Keep source discoveries separate; an explicit metadata link doesn't merge
  // installations, change launch profiles, or move history between editions.
  const discoveries = catalogue.records.map((record): GameEntry => withMetadata({
    id: `catalogue:${record.id}`, title: record.title, tagline: `${record.system.toUpperCase()} · Saved catalogue entry`,
    platform: cataloguePlatform(record.system), source: 'catalogue', installed: false, genre: '', year: record.year ?? 0,
    playtimeMin: 0, lastPlayed: null, addedAt: record.imported_at, favorite: false,
    ratings: { critic: null, criticCount: 0, user: null, steamReviews: null }, timeToBeat: null, achievements: null,
    art: artFromSeed(record.id),
  }, record));
  return [...local, ...discoveries];
}
