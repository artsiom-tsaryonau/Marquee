// Mirrors the unified game entry in docs/ROADMAP.md (Architecture → Unified game entry).

export type Source = 'steam' | 'gog' | 'epic' | 'lutris' | 'port' | 'rom';

export type Platform =
  | 'pc-linux'
  | 'pc-windows'
  | 'nes'
  | 'snes'
  | 'n64'
  | 'gamecube'
  | 'ps1'
  | 'ps2'
  | 'gba';

export type Motif = 'sun' | 'rings' | 'peaks' | 'grid' | 'waves' | 'shards' | 'orbit' | 'monolith';

/** Procedural artwork parameters; stand-in for downloaded cover/hero/logo images. */
export interface Art {
  hue: number;
  hue2: number;
  motif: Motif;
  /** Logo lettering style. */
  logo: 'wide' | 'serif' | 'mono' | 'heavy';
}

export interface GameEntry {
  id: string;
  title: string;
  tagline: string;
  platform: Platform;
  source: Source;
  installed: boolean;
  genre: string;
  year: number;
  playtimeMin: number;
  lastPlayed: string | null;
  addedAt: string;
  favorite: boolean;
  ratings: Ratings;
  /** Hours: main story, main + extras, completionist (IGDB). */
  timeToBeat: [number, number, number] | null;
  achievements: { unlocked: number; total: number } | null;
  /** Ports only: requirement state shown on the card. */
  portStatus?: PortStatus;
  art: Art;
}

/** 0–100 per source; null when that site has no score. */
export interface Ratings {
  critic: number | null;
  criticCount: number;
  user: number | null;
  steamReviews: { percent: number; count: number } | null;
}

export type PortStatus = 'ready' | 'needs-rom' | 'needs-data' | 'update' | 'not-installed';

export interface PortEntry {
  id: string;
  name: string;
  game: string;
  family: string;
  platform: Platform;
  upstream: string;
  version: string;
  status: PortStatus;
  requirement: string;
  art: Art;
}

export interface Download {
  id: string;
  name: string;
  progress: number;
  speed: string;
  state: 'downloading' | 'queued' | 'paused' | 'done';
}
