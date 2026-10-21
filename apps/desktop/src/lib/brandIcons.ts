/** Bundled launcher / store marks under `public/brands/`. See `public/brands/README.md`. */
export type BrandId = 'steam' | 'lutris' | 'heroic' | 'legendary' | 'epic' | 'gog' | 'purple';

const FILES: Record<BrandId, string> = {
  steam: 'steam.png',
  lutris: 'lutris.png',
  heroic: 'heroic.png',
  legendary: 'legendary.png',
  epic: 'epic.png',
  gog: 'gog.png',
  purple: 'purple.png',
};

/** Light silhouettes on dark card art; full-color marks stay untouched. */
const TONE: Record<BrandId, 'color' | 'light'> = {
  steam: 'light',
  lutris: 'color',
  heroic: 'color',
  legendary: 'color',
  epic: 'light',
  gog: 'light',
  purple: 'color',
};

export type BrandIcon = { src: string; tone: 'color' | 'light' };

export function brandIcon(id: BrandId): BrandIcon {
  const base = import.meta.env?.BASE_URL ?? '/';
  return { src: `${base}brands/${FILES[id]}`, tone: TONE[id] };
}

function matchBrand(text: string): BrandId | undefined {
  const t = text.toLowerCase();
  if (t.includes('steam')) return 'steam';
  if (t.includes('heroic')) return 'heroic';
  if (t.includes('legendary')) return 'legendary';
  if (t.includes('lutris')) return 'lutris';
  if (t.includes('purple')) return 'purple';
  if (t.includes('opengoal')) return undefined;
  if (t.includes('epic')) return 'epic';
  if (t.includes('gog')) return 'gog';
  return undefined;
}

/** Icon for a persisted or detected external launcher shortcut. */
export function launcherBrandIcon(info: { id: string; name: string; executable: string }): BrandIcon | undefined {
  const id = matchBrand(info.id) ?? matchBrand(info.name) ?? matchBrand(info.executable);
  return id ? brandIcon(id) : undefined;
}

export function sourceBrandIcon(src: 'purple' | 'lutris'): BrandIcon {
  return brandIcon(src === 'purple' ? 'purple' : 'lutris');
}
