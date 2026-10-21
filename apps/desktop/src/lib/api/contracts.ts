export interface AppError {
  code: string;
  message: string;
  context?: unknown;
}

export interface PurpleStatus {
  installed: boolean;
  ready: boolean;
  busy: boolean;
  running: boolean;
  message: string;
  error: string | null;
  prefix: string;
  log_path: string;
  runtime: string | null;
  aion2_verified: boolean;
}

export interface LibraryRoot {
  id: string;
  path: string;
  kind: 'folder';
  system: string;
  label: string | null;
  status: 'online' | 'offline';
  last_scan_at: string | null;
}

export interface Edition {
  id: string;
  system: string;
  title: string;
  scan_title: string;
  display_title: string | null;
  favorite: boolean;
  media_id: string | null;
  last_played_at: string | null;
  playtime_seconds: number;
  play_count: number;
  imported_at: string;
  missing: boolean;
  serial: string | null;
  region: string | null;
  cover_path: string | null;
}

export interface MediaFile {
  id: string;
  edition_id: string;
  kind: string;
  path: string;
  size_bytes: number;
  mtime: number;
  serial: string | null;
  serial_raw: string | null;
  region: string | null;
  available: boolean;
  skip_reason: string | null;
}

export interface Runner {
  id: string;
  kind: 'pcsx2' | 'duckstation' | 'fake';
  display_name: string;
  command: string[];
  bios_state: 'missing' | 'unverified' | 'launch_tested';
  bios_hint: string | null;
  source: string;
  available: boolean;
}

export interface Session {
  id: string;
  edition_id: string;
  runner_id: string;
  started_at: string;
  ended_at: string | null;
  duration_seconds: number | null;
  exit_code: number | null;
  state: 'running' | 'ended' | 'failed';
  error: string | null;
}

export interface SkipCount {
  kind: string;
  count: number;
}

export interface ScanReport {
  scanned: number;
  added: number;
  updated: number;
  skipped_unsupported: number;
  skipped_by_kind: SkipCount[];
  unresolved_serial: number;
  missing: number;
  warnings?: string[];
}

export interface Settings {
  last_root: string | null;
  last_collection: string | null;
  last_focus: string | null;
  selected_runner_id: string | null;
  selected_runner_ids?: Record<string, string>;
  display_mode?: 'window' | 'fullscreen';
}

export interface LibrarySnapshot {
  catalogue?: CatalogueSnapshot;
  roots: LibraryRoot[];
  editions: Edition[];
  media: MediaFile[];
  runners: Runner[];
  sessions: Session[];
  settings: Settings;
}

export type CoreEvent =
  | { type: 'scan_started'; root_id: string }
  | { type: 'scan_progress'; scanned: number; added: number }
  | { type: 'scan_finished'; report: ScanReport }
  | { type: 'session_started'; session: Session }
  | { type: 'session_ended'; session: Session }
  | { type: 'session_failed'; session: Session }
  | { type: 'library_changed' }
  | { type: 'launchers_changed' }
  | { type: 'lutris_changed' }
  | { type: 'tool_install_started'; tool_id: string; channel: string }
  | { type: 'tool_install_progress'; tool_id: string; message: string }
  | { type: 'tool_install_finished'; tool_id: string; runner_id: string | null }
  | { type: 'tool_install_failed'; tool_id: string; message: string }
  | { type: 'port_progress'; port_id: string; message: string }
  | { type: 'artwork_finished'; fetched: number; skipped: number; failed: number };

export interface PortInfo {
  id: string;
  name: string;
  game: string;
  family: string;
  platform: string;
  upstream: string;
  requirement: string;
  installed_version: string | null;
  latest_version: string | null;
  status: 'not_installed' | 'ready' | 'update';
  folder: string | null;
}

/** One configurable key. The UI never receives the secret itself, only a masked preview. */
export interface KeyInfo {
  id: string;
  env: string;
  label: string;
  description: string;
  help: string;
  used_by: string;
  secret: boolean;
  /** Where the active value comes from: saved in Marquee, the environment, a .env file, or nowhere. */
  source: 'app' | 'env' | 'dot_env' | 'none';
  set: boolean;
  preview: string | null;
}

export interface LutrisConfig {
  enabled: boolean;
  include_owned: boolean;
  database_path: string | null;
  launcher_id: string;
}
export interface LutrisGame {
  id: string; name: string; service: string; store_id: string | null;
  lutris_id: number | null; runner: string | null; platform: string;
  installed: boolean; available: boolean; playtime_minutes: number;
}
export interface LutrisSnapshot {
  config: LutrisConfig;
  detected_paths: string[];
  database_path: string | null;
  games: LutrisGame[];
  cached_by_service: Record<string, number>;
  configured_count: number;
  installed_count: number;
  warnings: string[];
}

export interface LauncherConfig {
  id: string;
  name: string;
  executable: string;
  args: string[];
  working_directory: string | null;
  pinned: boolean;
}
export interface LauncherInfo extends LauncherConfig {
  configured: boolean;
  available: boolean;
  status: string;
}


export interface FirmwareStatus { name: string; required: boolean; present: boolean; path: string | null; note: string }

export interface EmulatorInfo {
  firmware?: FirmwareStatus[];
  launch_presentation?: string;
  source_build?: boolean;
  source_receipt?: { commit: string; image: string; recipe: string; validation: string } | null;
  can_rollback?: boolean;
  id: string;
  name: string;
  systems: string[];
  /** True when library games launch through it; otherwise it can only be installed and opened. */
  integrated: boolean;
  upstream: string;
  requirement: string;
  installed_version: string | null;
  latest_version: string | null;
  status: 'not_installed' | 'ready' | 'update';
  program_dir: string | null;
  data_dir: string;
  bios_dir: string | null;
  icon: string | null;
  args: string[];
}

export interface BiosFile { system: string; file: string; region: string; version: string }
export interface EmulatorBuildStatus {
  state: 'running' | 'succeeded' | 'failed' | 'cancelled' | 'interrupted';
  phase: string;
  message: string;
  commit: string;
  log_path: string;
  log_tail: string;
}
export interface BiosImport { added: BiosFile[]; ignored: number }

export interface LibraryApi {
  importCatalogue(snapshot: unknown): Promise<CatalogueSnapshot>;
  linkCatalogue(editionId: string, recordId: string, mediaId: string): Promise<void>;
  unlinkCatalogue(editionId: string): Promise<void>;
  matchCatalogue(editionId: string): Promise<CatalogueLink>;
  clearCatalogue(): Promise<void>;
  openCataloguePage(recordId: string): Promise<void>;
  /** `desktop` = native window, `web` = browser attached to a running Marquee. */
  kind: 'desktop' | 'web';
  snapshot(): Promise<LibrarySnapshot>;
  addRoot(path: string, system?: string): Promise<LibraryRoot>;
  pickRoot(): Promise<string | null>;
  scan(rootId?: string): Promise<ScanReport>;
  listRunners(): Promise<Runner[]>;
  selectRunner(id: string): Promise<void>;
  setDisplayMode(mode: 'window' | 'fullscreen'): Promise<void>;
  launch(editionId: string): Promise<Session>;
  setFavourite(editionId: string, favorite: boolean): Promise<void>;
  setDisplayTitle(editionId: string, title: string): Promise<void>;
  subscribe(handler: (event: CoreEvent) => void): () => void;
  whenReady(): Promise<void>;
  /** Address that opens this same library in a browser (desktop only). */
  webUrl(): Promise<string | null>;
  installPcsx2(channel?: string): Promise<Runner[]>;
  refreshArtwork(editionId?: string): Promise<ArtworkReport>;
  listKeys(): Promise<KeyInfo[]>;
  setKey(id: string, value: string): Promise<KeyInfo[]>;
  clearKey(id: string): Promise<KeyInfo[]>;
  lutrisLibrary(): Promise<LutrisSnapshot>;
  configureLutris(config: LutrisConfig): Promise<LutrisSnapshot>;
  openLutrisGame(id: string): Promise<void>;
  listLaunchers(): Promise<LauncherInfo[]>;
  saveLauncher(launcher: LauncherConfig): Promise<LauncherInfo[]>;
  removeLauncher(id: string): Promise<LauncherInfo[]>;
  openLauncher(id: string): Promise<void>;
  purpleStatus(): Promise<PurpleStatus>;
  installPurple(): Promise<PurpleStatus>;
  cancelPurple(): Promise<PurpleStatus>;
  openPurple(): Promise<void>;
  pickExecutable(): Promise<string | null>;
  listPorts(): Promise<PortInfo[]>;
  checkPortUpdates(): Promise<PortInfo[]>;
  installPort(portId: string): Promise<PortInfo[]>;
  launchPort(portId: string): Promise<void>;
  openPortFolder(portId: string): Promise<void>;
  listEmulators(): Promise<EmulatorInfo[]>;
  checkEmulatorUpdates(): Promise<EmulatorInfo[]>;
  installEmulator(id: string): Promise<EmulatorInfo[]>;
  buildEmulator(id: string): Promise<void>;
  emulatorBuildStatus(id: string): Promise<EmulatorBuildStatus | null>;
  cancelEmulatorBuild(id: string): Promise<void>;
  rollbackEmulator(id: string): Promise<EmulatorInfo[]>;
  uninstallEmulator(id: string, purgeData: boolean): Promise<EmulatorInfo[]>;
  openEmulator(id: string): Promise<void>;
  configureEmulator(id: string, args: string[]): Promise<EmulatorInfo[]>;
  listBios(): Promise<BiosFile[]>;
  importBios(path: string): Promise<BiosImport>;
  removeBios(system: string): Promise<number>;
}

export interface ArtworkReport {
  fetched: number;
  skipped: number;
  failed: number;
}

export interface CatalogueVariant {
  regions?: string[]; languages?: string[]; verified_at?: string | null;
  media_id: string; filename: string; version: string; size: string; serial: string;
  crc32: string; md5: string; sha1: string;
}
export interface CatalogueRecord {
  id: string; game_id: string; system: string; title: string; regions: string; languages: string;
  version: string; rating: string; url: string; has_detail: boolean; media: CatalogueVariant[];
  players: number | null; year: number | null;
  ratings: { graphics: number | null; sound: number | null; gameplay: number | null; overall: number | null };
  rating_votes: number | null; verified_at: string | null;
  cover: { data_url: string; credit: string; source_url: string } | null;
  exported_at: string; imported_at: string;
}
export interface CatalogueLink { edition_id: string; record_id: string; media_id: string; method: string; valid: boolean }
export interface CatalogueSnapshot {
  records: CatalogueRecord[];
  systems: { slug: string; name: string; category: string; launch_year: string; url: string }[];
  links: CatalogueLink[];
}
