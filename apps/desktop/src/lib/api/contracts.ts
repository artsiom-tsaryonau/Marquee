export interface AppError {
  code: string;
  message: string;
  context?: unknown;
}

export interface LibraryRoot {
  id: string;
  path: string;
  kind: 'folder';
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
  kind: 'pcsx2' | 'fake';
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
}

export interface Settings {
  last_root: string | null;
  last_collection: string | null;
  last_focus: string | null;
  selected_runner_id: string | null;
}

export interface LibrarySnapshot {
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

export interface LibraryApi {
  /** `desktop` = native window, `web` = browser attached to a running Marquee, `demo` = invented data. */
  kind: 'demo' | 'desktop' | 'web';
  snapshot(): Promise<LibrarySnapshot>;
  addRoot(path: string): Promise<LibraryRoot>;
  pickRoot(): Promise<string | null>;
  scan(rootId?: string): Promise<ScanReport>;
  listRunners(): Promise<Runner[]>;
  selectRunner(id: string): Promise<void>;
  openSetup(id: string): Promise<void>;
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
  listPorts(): Promise<PortInfo[]>;
  checkPortUpdates(): Promise<PortInfo[]>;
  installPort(portId: string): Promise<PortInfo[]>;
  launchPort(portId: string): Promise<void>;
  openPortFolder(portId: string): Promise<void>;
}

export interface ArtworkReport {
  fetched: number;
  skipped: number;
  failed: number;
}
