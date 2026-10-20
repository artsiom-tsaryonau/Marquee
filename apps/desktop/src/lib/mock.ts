// Sample data only. Library titles are invented; port names are the planned
// catalogue entries, with made-up versions and statuses.
import { jakExample, enrichIdentity, nativePortGame } from './unified';
import type { Art, Download, GameEntry, Motif, Platform, PortEntry, Ratings, Source } from './types';
export { platformLabel, sourceLabel } from './labels';

const motifs: Motif[] = ['sun', 'rings', 'peaks', 'grid', 'waves', 'shards', 'orbit', 'monolith'];
const logos: Art['logo'][] = ['wide', 'serif', 'mono', 'heavy'];

function rng(seed: number) {
  return () => {
    seed = (seed * 1664525 + 1013904223) % 4294967296;
    return seed / 4294967296;
  };
}

const raw: [string, string, Source, Platform, string, number][] = [
  ['Ashfall Meridian', 'The sky burned. You kept walking.', 'steam', 'pc-windows', 'Action RPG', 2024],
  ['Neon Cartographer', 'Map a city that redraws itself every night.', 'steam', 'pc-linux', 'Exploration', 2025],
  ['Hollow Lantern', 'A lighthouse keeper and a sea that whispers back.', 'gog', 'pc-windows', 'Adventure', 2019],
  ['Tidebreaker', 'Command the last fleet against a rising ocean.', 'epic', 'pc-windows', 'Strategy', 2023],
  ['Glasswing', 'Fly between shattered islands on borrowed wings.', 'steam', 'pc-linux', 'Platformer', 2022],
  ['Ironvale Chronicles', 'A kingdom of clockwork and old debts.', 'gog', 'pc-windows', 'CRPG', 2017],
  ['Starward Drift', 'Salvage, trade and vanish into the deep belt.', 'steam', 'pc-windows', 'Space sim', 2024],
  ['Quiet Harbor', 'Fix boats. Make tea. Don’t ask about the fog.', 'steam', 'pc-linux', 'Cozy', 2025],
  ['Crimson Relay', 'Twelve runners. One message. No second chances.', 'epic', 'pc-windows', 'Racing', 2021],
  ['The Last Archive', 'Every book you save rewrites history.', 'gog', 'pc-linux', 'Puzzle', 2020],
  ['Emberforge', 'Smith legendary weapons for doomed heroes.', 'steam', 'pc-windows', 'Crafting', 2023],
  ['Wavelength', 'A rhythm duel across radio frequencies.', 'steam', 'pc-linux', 'Rhythm', 2022],
  ['Northbound', 'A sled, six dogs and three thousand miles.', 'gog', 'pc-windows', 'Survival', 2018],
  ['Paper Moons', 'Fold the world to find your way home.', 'epic', 'pc-windows', 'Puzzle', 2024],
  ['Sunken Crown', 'Dive for the treasure of a drowned empire.', 'lutris', 'pc-windows', 'Adventure', 2016],
  ['Velocity Bloom', 'Zero-gravity racing through living coral.', 'steam', 'pc-windows', 'Racing', 2025],
  ['Obsidian Choir', 'Gothic horror sung in four voices.', 'gog', 'pc-windows', 'Horror', 2021],
  ['Field of Signals', 'Decode the transmissions before they find you.', 'steam', 'pc-linux', 'Thriller', 2023],
  ['Copperline', 'Build the railway that unites a continent.', 'steam', 'pc-windows', 'Tycoon', 2020],
  ['Moth & Mirror', 'Two souls, one reflection, a hundred rooms.', 'epic', 'pc-windows', 'Narrative', 2022],
  ['Skyward Atelier', 'Paint floating castles into existence.', 'steam', 'pc-linux', 'Creative', 2024],
  ['Rune Courier', 'Deliver spells across a war-torn valley.', 'lutris', 'pc-windows', 'Action', 2015],
  ['Deepcore', 'Drill. Defend. Don’t wake what’s below.', 'steam', 'pc-windows', 'Co-op', 2023],
  ['Lumen Garden', 'Grow light in a world that forgot the sun.', 'gog', 'pc-linux', 'Cozy', 2025],
  ['Pixel Knight DX', 'A pocket-sized legend, remastered.', 'rom', 'gba', 'Action', 2003],
  ['Turbo Comet', 'Blast-processing classic.', 'rom', 'snes', 'Racing', 1994],
  ['Moonlit Cavern', 'A dungeon crawler in sixteen colours.', 'rom', 'nes', 'Adventure', 1988],
  ['Cascade Valley 64', 'Three-dimensional wonder, in your hands.', 'rom', 'n64', 'Platformer', 1998],
  ['Phantom Arcade', 'Memory card not required.', 'rom', 'ps1', 'Arcade', 1999],
  ['Aurora Tactics', 'Grid battles under the northern lights.', 'rom', 'gamecube', 'Tactics', 2004],
];

let seq = 0;
function art(r: () => number): Art {
  const hue = Math.floor(r() * 360);
  return {
    hue,
    hue2: (hue + 40 + Math.floor(r() * 140)) % 360,
    motif: motifs[seq++ % motifs.length],
    logo: logos[Math.floor(r() * logos.length)],
  };
}

function ratings(r: () => number, source: Source): Ratings {
  const critic = r() > 0.12 ? Math.round(62 + r() * 34) : null;
  return {
    critic,
    criticCount: critic ? Math.floor(8 + r() * 90) : 0,
    user: Math.round(58 + r() * 38),
    steamReviews: source === 'steam' ? { percent: Math.round(70 + r() * 28), count: Math.floor(800 + r() * 150000) } : null,
  };
}

function timeToBeat(r: () => number): [number, number, number] {
  const main = Math.round(2 + r() * 40);
  return [main, Math.round(main * (1.4 + r() * 0.6)), Math.round(main * (2.2 + r() * 1.8))];
}

const patches = ['Hotfix 1.4.2', 'Winter update', 'Performance patch', 'Patch 2.0: new region', 'Balance changes', 'Linux build fixes'];
const deck = ['verified', 'playable', 'unsupported', 'unknown'] as const;

const today = new Date('2026-10-20T20:00:00');
const daysAgo = (d: number) => new Date(today.getTime() - d * 86400000).toISOString();

export const games: GameEntry[] = raw.map(([title, tagline, source, platform, genre, year], i) => {
  const r = rng(i * 7919 + 17);
  const played = r() > 0.35;
  const slug = title.toLowerCase().replace(/[^a-z0-9]+/g, '-');
  return {
    id: `${source}:${slug}`,
    title,
    tagline,
    platform,
    source,
    installed: source === 'rom' || r() > 0.4,
    genre,
    year,
    playtimeMin: played ? Math.floor(r() * 9000) + 30 : 0,
    lastPlayed: played ? daysAgo(Math.floor(r() * 120)) : null,
    addedAt: daysAgo(Math.floor(r() * 400)),
    favorite: r() > 0.75,
    ratings: ratings(r, source),
    timeToBeat: r() > 0.1 ? timeToBeat(r) : null,
    achievements: source === 'steam' || source === 'rom' ? { unlocked: played ? Math.floor(r() * 40) : 0, total: 40 + Math.floor(r() * 30) } : null,
    steam:
      source === 'steam'
        ? {
            playersNow: Math.floor(r() * r() * 40000) + 12,
            updatedDaysAgo: Math.floor(r() * 60),
            patchTitle: patches[Math.floor(r() * patches.length)],
            deck: deck[Math.floor(r() * 3.4)],
          }
        : undefined,
    art: art(r),
  };
});

export const ports: PortEntry[] = (
  [
    ['shipwright', 'Ship of Harkinian', 'Ocarina of Time', 'HarbourMasters', 'n64', 'HarbourMasters/Shipwright', '9.0.2', 'ready', 'ROM extracted'],
    ['2ship', '2Ship2Harkinian', 'Majora’s Mask', 'HarbourMasters', 'n64', 'HarbourMasters/2ship2harkinian', '2.0.1', 'needs-rom', 'Needs your N64 ROM (US 1.0)'],
    ['zelda64recomp', 'Zelda64Recomp', 'Majora’s Mask', 'N64 recomps', 'n64', 'Zelda64Recomp/Zelda64Recomp', '1.2.0', 'update', 'Update 1.2.0 → 1.3.0'],
    ['sm64coopdx', 'sm64coopdx', 'Super Mario 64', 'SM64 ports', 'n64', 'coop-deluxe/sm64coopdx', '1.3', 'ready', 'ROM extracted'],
    ['fallout2-ce', 'Fallout 2 CE', 'Fallout 2', 'Engine rewrites', 'pc-windows', 'alexbatalov/fallout2-ce', '1.3.0', 'needs-data', 'Needs your GOG or Steam install'],
    ['opengoal', 'OpenGOAL', 'Jak and Daxter', 'OpenGOAL', 'ps2', 'open-goal/jak-project', '0.2.x', 'not-installed', 'Launcher extracts your disc image'],
    ['uzdoom', 'UZDoom', 'Doom / Doom II', 'Engine rewrites', 'pc-windows', 'UZDoom/UZDoom', '4.14', 'ready', 'IWAD found'],
    ['unleashed', 'UnleashedRecomp', 'Sonic Unleashed', 'Xbox 360 recomps', 'pc-windows', 'hedge-dev/UnleashedRecomp', '1.0.3', 'not-installed', 'Installer copies your game files'],
    ['openmw', 'OpenMW', 'Morrowind', 'Engine rewrites', 'pc-windows', 'OpenMW/openmw', '0.49', 'ready', 'GOG install found'],
  ] as const
).map(([id, name, game, family, platform, upstream, version, status, requirement], i) => ({
  id,
  name,
  game,
  family,
  platform,
  upstream,
  version,
  status,
  requirement,
  art: art(rng(i * 104729 + 3)),
}));

export function portGame(p: PortEntry): GameEntry {
  return enrichIdentity({
    id: `port:${p.id}`,
    title: p.name,
    tagline: `Native port of ${p.game}.`,
    platform: 'pc-linux',
    source: 'port',
    installed: true,
    genre: p.family,
    year: 2025,
    playtimeMin: 240 + p.name.length * 37,
    lastPlayed: daysAgo(p.name.length),
    addedAt: daysAgo(p.name.length * 3),
    favorite: false,
    ratings: { critic: 90 + (p.name.length % 8), criticCount: 20, user: 85 + (p.name.length % 12), steamReviews: null },
    timeToBeat: [12 + p.name.length, 20 + p.name.length, 40 + p.name.length],
    achievements: null,
    portStatus: p.status,
    art: p.art,
  });
}

// Ready ports also show up in the library, like any other entry.
for (const p of ports.filter((p) => p.status === 'ready' || p.status === 'update')) {
  games.push(portGame(p));
}

games.unshift(jakExample, nativePortGame(ports.find((p) => p.id === 'opengoal')!));

export const downloads: Download[] = [
  { id: 'd1', name: 'Zelda64Recomp 1.3.0', progress: 0.64, speed: '18.2 MB/s', state: 'downloading', target: { kind: 'port', id: 'zelda64recomp' } },
  { id: 'd2', name: 'GE-Proton 10-21', progress: 0.12, speed: '—', state: 'queued' },
  { id: 'd3', name: 'Starward Drift', progress: 0.38, speed: '—', state: 'paused', target: { kind: 'game', id: 'steam:starward-drift' } },
  { id: 'd4', name: 'OpenMW 0.49', progress: 1, speed: '', state: 'done' },
];

