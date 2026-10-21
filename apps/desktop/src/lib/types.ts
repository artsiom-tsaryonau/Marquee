// Mirrors the unified game entry in docs/ROADMAP.md (Architecture → Unified game entry).

export type Source = 'steam' | 'gog' | 'epic' | 'lutris' | 'port' | 'rom';

export type Platform =
  | 'unknown'
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

export type SetupState = 'runner_missing' | 'missing' | 'unverified' | 'launch_tested';

export interface GameEntry {
  id: string;
  /** Explicit catalogue identity, independent of a concrete installation ID. */
  canonicalId?: string;
  editionKey?: string;
  editionLabel?: string;
  launchMethod?: import('./unified').LaunchMethod;
  heroUrl?: string;
  screenshots?: { url: string; caption: string }[];
  mediaCredit?: string;
  mediaSource?: string;
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
  serial?: string;
  region?: string;
  mediaMissing?: boolean;
  rootOffline?: boolean;
  setupState?: SetupState;
  runnerId?: string;
  runnerName?: string;
  mediaIssue?: string;
  ratings: Ratings;
  /** Hours: main story, main + extras, completionist (IGDB). */
  timeToBeat: [number, number, number] | null;
  achievements: { unlocked: number; total: number } | null;
  /** Steam games: PICS app info + Web API (see roadmap, "Steam data the way SteamDB gets it"). */
  steam?: SteamInfo;
  lutris?: { service: string; storeId: string | null; runner: string | null };
  /** Ports only: requirement state shown on the card. */
  portStatus?: PortStatus;
  art: Art;
  /** convertFileSrc URL for a cached cover, if one was fetched. */
  coverUrl?: string;
}

export interface SteamInfo {
  playersNow: number;
  updatedDaysAgo: number;
  patchTitle: string;
  deck: 'verified' | 'playable' | 'unsupported' | 'unknown';
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

