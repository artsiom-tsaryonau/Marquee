import type { FactField, GameInfo } from './api/contracts';

const labels: Record<FactField, string> = {
  year: 'Release year', genre: 'Genre', developer: 'Developer',
  publisher: 'Publisher', players: 'Players', description: 'Description',
};

export function factRows(current: Partial<GameInfo> | null | undefined, proposed: Partial<GameInfo>) {
  return (Object.keys(labels) as FactField[]).flatMap(field => {
    const after = proposed[field];
    if (after == null || (typeof after === 'string' && !after.trim())) return [];
    const before = current?.[field] ?? null;
    return [{ field, label: labels[field], before, after, changed: before !== after,
      missing: before == null || (typeof before === 'string' && !before.trim()) }];
  });
}
