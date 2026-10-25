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
  runner_override?: string | null;
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
  info?: GameInfo | null;
  /** `wishlist` or `owned` for a game that has no files yet. */
  wanted?: 'wishlist' | 'owned' | null;
}

/** Genre, year, developer, publisher, players and description, from libretro and TheGamesDB. */
export interface GameInfo {
  genre: string | null;
  year: number | null;
  developer: string | null;
  publisher: string | null;
  players: number | null;
  description: string | null;
  source: string;
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

export interface HeadlessStatus {
  supported: boolean; ready: boolean; reason: string | null; busy: boolean;
  error: AppError | null; host_running: boolean; game_running: boolean;
  active_edition_id: string | null; app_id: string | null; host_error: string | null;
}

export interface Runner {
  id: string;
  kind: 'pcsx2' | 'duckstation' | 'emulator' | 'fake';
  systems?: string[];
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

export interface IntakeReport {
  archive: string;
  outcome: 'imported' | 'kept_archive' | 'stored_no_emulator' | 'already_present' | 'skipped';
  system: string | null;
  dest: string | null;
  files: string[];
  message: string;
  source_removed: boolean;
}
export interface IntakeResult { path: string; report: IntakeReport | null; error: string | null }

export type PreparationState = 'awaiting_review' | 'preparing' | 'interrupted' | 'awaiting_library' | 'registered' | 'failed' | 'cancelled';
export interface PreparationJob {
  torrent_source?: { info_hash: string; name: string; relative_path: string; download_directory: string; artifact: { filename: string; bytes: number; sha256: string; content_verified: boolean }; piece_verified: boolean };
  browser_source?: { browser_job_id: string; selection: { provider: string; title: string }; artifact: { filename: string; bytes: number; sha256: string; content_verified: boolean } };
  id: string;
  source: string;
  source_size: number;
  source_sha256: string;
  destination: string;
  plan: { system: string; files: { path: string; size: number }[]; keep_archive: boolean; required_bytes: number };
  review_token: string;
  state: PreparationState;
  attempt: number;
  error: string | null;
  error_code: string | null;
  receipt: { id: string; recipe: string; content_verified: boolean } | null;
  edition_ids: string[];
  created_at: string;
  updated_at: string;
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
  torrents?: TorrentSettings;
}

/** Optional device-to-device library transfer. Off by default. */
export interface TorrentSettings {
  enabled: boolean;
  trackers?: boolean;
  dht?: boolean;
  /** Let other devices download from this one. */
  seeding: boolean;
  /** KiB/s, 0 = unlimited. */
  upload_kbps: number;
  download_kbps: number;
  /** Loopback SOCKS5 proxy for public torrents (e.g. a VPN container); null = direct. */
  proxy?: string | null;
}

export interface TransferItem {
  info_hash: string;
  name: string;
  kind: 'share' | 'get';
  edition_id: string | null;
  state: 'preparing' | 'checking' | 'downloading' | 'sharing' | 'seeding' | 'source_missing' | 'complete' | 'paused' | 'error' | 'starting';
  files_present?: boolean | null;
  directory?: string;
  public?: boolean;
  downloaded: number;
  total: number;
  uploaded: number;
  /** Paste this on another device to get the game from this one. */
  link: string | null;
  error: string | null;
}

export interface TransferStatus {
  enabled: boolean;
  seeding: boolean;
  items: TransferItem[];
}

export interface TorrentArtifact { path: string; size: number; supported: boolean; reason: string | null; preparation_id: string | null }
export interface TorrentDownload { info_hash: string; name: string; directory: string; available: boolean; error: string | null; files: TorrentArtifact[] }
export interface TorrentReview {
  preparation_files?: { path: string; size: number; supported: boolean; reason: string | null }[];
  info_hash: string;
  name: string;
  total: number;
  files: { path: string; size: number }[];
  tracker_count: number;
  private: boolean;
  review_token: string;
  directory: string;
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
  job?: { state: string; message: string; error_code: string | null } | null;
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

export interface EmulatorProfileOptions { renderer: 'Automatic' | 'Vulkan' | 'OpenGL' | 'Software'; resolution_scale: number; vsync: boolean }
export interface EmulatorProfileConfig { mode: 'recommended' | 'custom' | 'disabled'; custom: EmulatorProfileOptions }
export interface EmulatorProfileStatus {
  version: number;
  source: string;
  host: { gpus: { vendor: string; device: string; driver: string }[]; displays: { name: string; width: number; height: number; refresh_hz: number; source: string }[]; session: string; notes: string[] };
  recommended: EmulatorProfileOptions;
  config: EmulatorProfileConfig;
  fields: { section: string; key: string; desired: string; current: string | null; managed: boolean; preserved: boolean }[];
  settings_path: string;
}

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

export interface ManagerStatus { running: boolean; pid: number | null; stopping: boolean; protocol_version: number; snapshot_polling: boolean; event_replay: boolean; active_cancel: boolean }

export interface StreamingConnection { data_dir: string; socket: string }
export interface StreamingApp { id: string; title: string; support_hdr: boolean; start_virtual_compositor: boolean; start_audio_server: boolean }
export interface StreamingSession { session_id: string; app_id: string | null; client_ip: string; video_width: number; video_height: number; video_refresh_rate: number; audio_channel_count: number }
export interface StreamingState {
  headless?: HeadlessStatus;
  connection: StreamingConnection;
  host: { apps: StreamingApp[]; clients: { client_id: string }[]; sessions: StreamingSession[] } | null;
  host_error: AppError | null;
  pending: { client_ip: string }[];
  viewer: { running: boolean; app_id?: string; connection_verified?: boolean };
  viewer_error: AppError | null;
  receiver_installed: boolean;
  receiver_error: AppError | null;
  operation: 'viewer' | 'install' | 'pair' | null;
  last_error: AppError | null;
}

export interface MusicTrack { id: string; title: string; available: boolean; duration_seconds?: number | null }
export interface MusicAlbum { id: string; title: string; artist: string; source_url: string; cover_data_url?: string | null; cover_source?: string | null; game_keys?: string[]; tracks: MusicTrack[] }
export interface MusicDraft { game_key: string; album_id: string | null; title: string; artist: string; source_url: string }

export interface Ps3FirmwareStatus {
  job: {
    id: string; state: 'reviewed' | 'running' | 'promoting' | 'installed' | 'failed' | 'interrupted';
    message: string; review_token: string;
    package: {source: string; version: string; bytes: number; sha256: string};
    runner_version: string; program_sha256: string;
    output: {version: string; files: number; bytes: number; sha256: string} | null;
    exit_code: number | null; log_path: string | null;
  } | null;
  installed_version: string | null;
  managed_receipt: boolean;
}

export interface Ps3Inspection {
  source: string;
  source_kind: string;
  source_bytes: number | null;
  archive_unpacked_bytes: number | null;
  inspected_entries: number;
  truncated: boolean;
  playable: false;
  items: {
    path: string; kind: string; bytes: number | null;
    metadata: {title: string; title_id: string; category: string; app_version: string | null; system_version: string | null} | null;
    boot_file_present: boolean; artwork_present: boolean;
    status: string; next_action: string;
  }[];
  diagnostics: string[];
}

export interface MediaAsset { id:string; title:string; kind:'image'|'video'; bytes:number; source:string; spoiler:boolean; position:number; available:boolean; }
export interface MediaCollection { id:string; title:string; game_keys:string[]; assets:MediaAsset[]; }
export interface MediaDisc { path:string; source_bytes:number; signature:string; items:{path:string; bytes:number; kind:'image'|'video'; supported:boolean}[]; }
export interface AppListEntry { id:string; source:string; name:string; project:string|null; display_name:string; repository:string; provider:string; folder_name:string; release_asset_filter:string|null; tags:string[]; icon_url:string|null; recipe_id:string|null; status:'supported_recipe'|'needs_recipe'|'unsupported'; change:'add'|'already_present'|'update'; diagnostics:string[]; original:Record<string,unknown>; }
export interface AppListReview { token:string; source:string; sha256:string; diagnostics:string[]; entries:AppListEntry[]; }
export interface YoutubeSource { id:string; title:string; uploader:string; url:string; kind:'playlist'|'video'; duration:number|null; }
export interface YoutubePlaylist { title:string; uploader:string; url:string; tracks:YoutubeSource[]; truncated:boolean; }
export interface MetadataReview {token:string;provider:string;candidates:{id:number;title:string;system?:string;facts?:{year?:number;genre?:string;players?:number;description?:string}}[];}
export interface LibraryApi {
  previewAppList(source:string,json:string):Promise<AppListReview>;
  approveAppList(token:string,ids:string[]):Promise<number>;
  appListEntries():Promise<AppListEntry[]>;
  importPs3Disc(path:string):Promise<string>;
  searchGameMetadata(editionId:string,query:string):Promise<MetadataReview>;
  searchGameFacts(editionId:string,query:string):Promise<MetadataReview>;
  applyGameFacts(editionId:string,token:string,id:number):Promise<void>;
  applyMetadataMatch(editionId:string,token:string,candidateId:number):Promise<void>;

  mediaCollections(gameKey?:string):Promise<MediaCollection[]>;
  mediaCreate(title:string,gameKey?:string):Promise<string>;
  mediaLink(id:string,gameKey:string,linked:boolean):Promise<void>;
  mediaImport(id:string,paths:string[]):Promise<void>;
  mediaInspectDisc(path:string):Promise<MediaDisc>;
  mediaImportDisc(id:string,path:string,signature:string,member:string):Promise<void>;
  mediaPosition(id:string,position:number):Promise<void>;
  mediaUpdate(id:string,title:string,spoiler:boolean):Promise<void>;
  mediaRemove(id:string):Promise<void>;
  mediaUrl(id:string):Promise<string>;
  pickMedia(kind?:'image'|'video'):Promise<string[]>;
  pickMediaDisc():Promise<string|null>;

  musicSearchYoutube(query:string,kind:'playlist'|'video'):Promise<YoutubeSource[]>;
  musicInspectYoutube(url:string):Promise<YoutubePlaylist>;
  musicOpenYoutube(url:string):Promise<void>;
  musicLinkYoutube(gameKey:string,url:string,title:string):Promise<string>;
  musicAlbums(gameKey?: string): Promise<MusicAlbum[]>;
  musicFind(title: string): Promise<void>;
  musicOpenSource(albumId: string): Promise<void>;
  musicSaveAlbum(draft: MusicDraft): Promise<string>;
  musicLink(gameKey: string, albumId: string, linked: boolean): Promise<void>;
  musicImport(albumId: string, paths: string[]): Promise<number>;
  musicRemoveTrack(albumId: string, trackId: string): Promise<void>;
  musicUrl(trackId: string): Promise<string>;
  pickMusic(): Promise<string[]>;
  pickMusicCover(): Promise<string | null>;
  musicSetCover(albumId: string, path: string | null): Promise<void>;
  headlessStatus(editionId?: string): Promise<HeadlessStatus>;
  headlessLaunch(editionId: string): Promise<void>;
  headlessStop(): Promise<void>;
  streamingState(): Promise<StreamingState>;
  streamingSaveConnection(connection: StreamingConnection): Promise<void>;
  streamingOpenViewer(appId: string): Promise<void>;
  streamingPairViewer(): Promise<void>;
  streamingInstallViewer(): Promise<void>;
  streamingDisconnectViewer(): Promise<unknown>;
  streamingPair(clientIp: string, pin: string): Promise<void>;
  streamingRevoke(clientId: string): Promise<void>;
  streamingEndGame(sessionId: string): Promise<void>;
  torrentDownloads(): Promise<TorrentDownload[]>;
  inspectTorrentPreparation(infoHash: string, path: string, system: string): Promise<PreparationJob>;
  browserJobs(): Promise<BrowserJob[]>;
  inspectBrowserPreparation(id: string, system: string): Promise<PreparationJob>;
  cancelBrowserJob(id: string): Promise<BrowserJob>;
  managerStatus(): Promise<ManagerStatus>;
  startManager(): Promise<ManagerStatus>;
  stopManager(): Promise<ManagerStatus>;
  preparationSystems(): Promise<{ id: string; name: string }[]>;
  preparationJobs(): Promise<PreparationJob[]>;
  inspectPreparation(path: string, system: string): Promise<PreparationJob>;
  startPreparation(id: string, reviewToken: string): Promise<PreparationJob>;
  cancelPreparation(id: string): Promise<PreparationJob>;
  intakeArchive(path: string, deleteSource?: boolean): Promise<IntakeReport>;
  pickArchives(): Promise<string[]>;
  pickConsoleContent(): Promise<string | null>;
  inspectConsole(path:string, system: 'ps4'|'ps5'|'xbox'|'xbox360'): Promise<ConsoleInspection>;
  pickPs3Content(): Promise<string | null>;
  pickPs3Firmware(): Promise<string | null>;
  ps3FirmwareStatus(): Promise<Ps3FirmwareStatus>;
  reviewPs3Firmware(path: string): Promise<Ps3FirmwareStatus>;
  installPs3Firmware(reviewToken: string): Promise<Ps3FirmwareStatus>;
  inspectPs3(path: string): Promise<Ps3Inspection>;
  pickTorrent(): Promise<string | null>;
  onArchiveDrop(handler: (paths: string[]) => void): Promise<() => void>;
  emulatorProfile(id: string): Promise<EmulatorProfileStatus>;
  configureEmulatorProfile(id: string, config: EmulatorProfileConfig): Promise<EmulatorProfileStatus>;
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
  selectSystemRunner(system: string, id: string): Promise<void>;
  setGameRunner(editionId: string, id: string | null): Promise<void>;
  setDisplayMode(mode: 'window' | 'fullscreen'): Promise<void>;
  setTorrentSettings(settings: TorrentSettings): Promise<void>;
  transferStatus(): Promise<TransferStatus>;
  transferShare(editionId: string): Promise<void>;
  transferGet(link: string): Promise<void>;
  transferReview(path: string): Promise<TorrentReview>;
  transferAddFile(path: string, reviewToken: string, peers?: string[]): Promise<void>;
  transferPause(infoHash: string): Promise<void>;
  transferResume(infoHash: string): Promise<void>;
  transferStop(infoHash: string): Promise<void>;
  launch(editionId: string): Promise<Session>;
  setFavourite(editionId: string, favorite: boolean): Promise<void>;
  setDisplayTitle(editionId: string, title: string): Promise<void>;
  /** A game without files: Play stays disabled until a matching file is scanned. Returns its edition id. */
  addWantedGame(system: string, title: string, status: 'wishlist' | 'owned'): Promise<string>;
  setWantedStatus(editionId: string, status: 'wishlist' | 'owned'): Promise<void>;
  removeWantedGame(editionId: string): Promise<void>;
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
  cancelPortInstall(portId: string): Promise<PortInfo[]>;
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

export interface BrowserJob {
  job_id: string;
  selection: { provider: string; title: string; source_url: string; link_type?: string };
  state: 'waiting' | 'downloading' | 'completed' | 'interrupted' | 'cancelled';
  bytes: number;
  total: number | null;
  cancel_requested: boolean;
  error: string | null;
  result: { filename: string; bytes: number; sha256: string; content_verified: boolean } | null;
  preparation_id?: string;
  source_available?: boolean;
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
  download_url?: string;
}
export interface CatalogueRecord {
  provenance?: Record<string, { source_url: string; exported_at: string; imported_at: string; method: string }>;
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

export interface ConsoleInspection {
 xbox360?: {title_id:string;media_id:string;version:number;disc_number:number;disc_count:number}|null;
 source: string; system: string; kind: string; bytes: number; content_id: string|null; content_type:number|null; content_flags:number|null;
 metadata: {title:string;title_id:string;category:string;app_version:string|null}|null;
 role:string; archive: {entries:number;files:number;unpacked_bytes:number;candidates:{path:string;bytes:number;role:string}[];sample:string[];truncated:boolean}|null;
 diagnostics:string[];playable:boolean;
}
