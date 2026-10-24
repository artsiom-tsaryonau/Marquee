import { createApi, type LibraryApi } from './api';
import { gamesFromSnapshot, pickRunner } from './api/map';
import type { BiosFile, CoreEvent, IntakeResult, TorrentSettings, TransferItem, TransferStatus, EmulatorInfo, KeyInfo, LauncherConfig, LauncherInfo, LutrisConfig, LutrisSnapshot, LibraryRoot, LibrarySnapshot, PortInfo, Runner, ScanReport, Session } from './api/contracts';
import { nativePortGame, canonicalId } from './unified';
import { lutrisToGame } from './lutris';
import { artFromSeed } from './art';
import { libraryView } from './libraryView.svelte';
import { music } from './music.svelte';
import { musicAccent } from './musicAppearance';
import { portAction } from './presentation';
import { describeScan } from './format';
import { installOption } from './install';
import type { GameEntry, PortEntry } from './types';

export type Route = 'home' | 'library' | 'music' | 'game' | 'ports' | 'settings' | 'ps2' | 'tools' | 'streaming';
export type LibraryStatus = 'loading' | 'empty' | 'loaded' | 'failed';

function load<T>(key: string, fallback: T): T {
  try {
    const v = localStorage.getItem(`marquee.${key}`);
    return v === null ? fallback : (JSON.parse(v) as T);
  } catch {
    return fallback;
  }
}

function save(key: string, value: unknown) {
  try {
    localStorage.setItem(`marquee.${key}`, JSON.stringify(value));
  } catch {
    // Storage can be unavailable (private window); preferences just won't stick.
  }
}

class AppState {
  catalogue = $state<import('./api/contracts').CatalogueSnapshot>({ records: [], systems: [], links: [] });
  catalogueBusy = $state(false);
  catalogueError = $state<string | null>(null);
  route = $state<Route>('home');
  history: { route: Route; gameId: string | null; heroId: string | null }[] = [];
  gameId = $state<string | null>(null);
  /** The entry whose art fills the backdrop. */
  heroId = $state<string | null>(null);

  mode = $state<'desktop' | 'big'>(load('mode', 'desktop'));
  showLegacyIntegrations = $state(load('showLegacyIntegrations', false));
  sidebarExpanded = $state(load('sidebarExpanded', false));
  theme = $state<'dark' | 'light'>(load('theme', 'dark'));
  visualStyle = $state<'aircloud' | 'classic'>(load<string>('visualStyle', 'aircloud') === 'classic' ? 'classic' : 'aircloud');
  libraryLayout = $state<'platform' | 'grid'>(load<string>('libraryLayout', 'platform') === 'grid' ? 'grid' : 'platform');
  musicAccent = $state(musicAccent(load<unknown>('musicAccent', '#63e6be')));
  reduceMotion = $state<boolean>(load('reduceMotion', false));
  visualEffects = $state<'balanced' | 'full'>(load<string>('visualEffects', 'balanced') === 'full' ? 'full' : 'balanced');
  systemReduceMotion = $state(typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches);
  motionReduced = $derived(this.reduceMotion || this.systemReduceMotion);
  variantPreferences = $state<Record<string, string>>(load('variantPreferences', {}));
  sounds = $state<boolean>(load('sounds', false));

  api: LibraryApi = createApi();
  webUrl = $state<string | null>(null);
  keys = $state<KeyInfo[]>([]);
  emulators = $state<EmulatorInfo[]>([]);
  bios = $state<BiosFile[]>([]);
  /** Emulator id → what it is doing right now (install/update progress or a short action). */
  emulatorBusy = $state<Record<string, string>>({});
  biosBusy = $state(false);
  /** Emulator whose settings dialog is open. */
  intakeOpen = $state(false);
  intakeBusy = $state(false);
  intakePaths = $state<string[]>([]);
  intakeDelete = $state(false);
  intakeResults = $state<IntakeResult[]>([]);
  intakeProgress = $state('');
  intakeScanError = $state<string | null>(null);
  portDialogId = $state<string | null>(null);
  portErrors = $state<Record<string, string>>({});
  emulatorDialogId = $state<string | null>(null);
  launchers = $state<LauncherInfo[]>([]);
  lutrisSource = $state<LutrisSnapshot | null>(null);
  lutrisError = $state<string | null>(null);
  lutrisLoading = $state(false);
  private lutrisRequest = 0;
  launcherError = $state<string | null>(null);
  launchersLoading = $state(false);
  closeLauncherEditor: (() => void) | null = null;
  private launcherRequest = 0;
  openingLaunchers = $state<string[]>([]);
  libraryStatus = $state<LibraryStatus>('loading');
  loadError = $state<string | null>(null);
  roots = $state<LibraryRoot[]>([]);
  runners = $state<Runner[]>([]);
  selectedRunner = $state<Runner | null>(null);
  runnerSettings = $state<LibrarySnapshot['settings']>({ last_root: null, last_collection: null, last_focus: null, selected_runner_id: null });
  runnerFor(system: string) { return pickRunner({ runners: this.runners, settings: this.runnerSettings }, system); }
  lastScan = $state<ScanReport | null>(null);
  scanning = $state(false);
  installingPcsx2 = $state(false);
  installProgress = $state<string | null>(null);
  installToolId = $state<string | null>(null);
  installOptionId = $state<string | null>(null);
  fetchingArt = $state(false);
  overlayDismissed = $state(false);

  games = $state<GameEntry[]>([]);
  ports = $state<PortEntry[]>([]);
  /** Port id → latest progress line while it installs (desktop only). */
  portBusy = $state<Record<string, string>>({});

  launching = $state<GameEntry | null>(null);
  running = $state<GameEntry | null>(null);
  toast = $state<string | null>(null);
  lastInput = $state<string>('—');

  hero = $derived(this.games.find((g) => g.id === this.heroId) ?? this.defaultHero());

  /** What the home page opens on: what you played last, else a favourite, else a game with a cover. */
  defaultHero(): GameEntry | null {
    const games = this.games.filter((g) => g.source !== 'catalogue');
    const played = games.filter((g) => g.lastPlayed).sort((a, b) => b.lastPlayed!.localeCompare(a.lastPlayed!));
    return played[0] ?? games.find((g) => g.favorite) ?? games.find((g) => g.coverUrl) ?? games[0] ?? null;
  }
  current = $derived(this.games.find((g) => g.id === this.gameId) ?? null);

  constructor() {
    $effect.root(() => {
      $effect(() => save('mode', this.mode));
      $effect(() => save('theme', this.theme));
      $effect(() => save('visualStyle', this.visualStyle));
      $effect(() => save('libraryLayout', this.libraryLayout));
      $effect(() => save('musicAccent', musicAccent(this.musicAccent)));
      $effect(() => save('sidebarExpanded', this.sidebarExpanded));
      $effect(() => save('showLegacyIntegrations', this.showLegacyIntegrations));
      $effect(() => save('reduceMotion', this.reduceMotion));
      $effect(() => save('visualEffects', this.visualEffects));
      $effect(() => save('sounds', this.sounds));
      $effect(() => save('variantPreferences', this.variantPreferences));
    });
  }

  async boot() {
    this.disconnect();
    this.unlisten = this.api.subscribe((event) => this.onEvent(event));
    try {
      await this.api.whenReady();
      const snap = await this.api.snapshot();
      this.applySnapshot(snap);
      await Promise.all([this.loadPorts(), this.loadLaunchers(), this.loadLutris(), this.loadEmulators(), this.loadBios()]);
      this.webUrl = await this.api.webUrl().catch(() => null);
    } catch (err) {
      this.libraryStatus = 'failed';
      this.loadError = err instanceof Error ? err.message : String((err as { message?: string }).message ?? err);
    }
  }

  private unlisten: (() => void) | undefined;
  disconnect() {
    this.unlisten?.();
    this.unlisten = undefined;
  }

  applySnapshot(snap: LibrarySnapshot) {
    this.catalogue = snap.catalogue ?? { records: [], systems: [], links: [] };
    this.roots = snap.roots;
    this.runners = snap.runners;
    this.runnerSettings = snap.settings;
    this.selectedRunner = pickRunner(snap);
    this.games = [...gamesFromSnapshot(snap), ...this.games.filter((g) => g.source === 'port' || !!g.lutris)];
    this.libraryStatus = this.games.length ? 'loaded' : 'empty';
    this.loadError = null;
    if (this.heroId && !this.games.some((g) => g.id === this.heroId)) {
      this.heroId = this.defaultHero()?.id ?? null;
    } else if (!this.heroId) {
      this.heroId = this.defaultHero()?.id ?? null;
    }
    const active = snap.sessions.find((s: Session) => s.state === 'running');
    if (active) {
      const game = this.games.find((g) => g.id === active.edition_id) ?? null;
      this.launching = null;
      if (!this.overlayDismissed) this.running = game;
    } else if (!this.launching) {
      this.running = null;
      this.overlayDismissed = false;
    }
  }

  onEvent(event: CoreEvent) {
    if (event.type === 'lutris_changed') void this.loadLutris();
    if (event.type === 'launchers_changed') void this.loadLaunchers();
    if (event.type === 'scan_finished') this.lastScan = event.report;
    if (event.type === 'session_started') {
      const game = this.games.find((g) => g.id === event.session.edition_id);
      this.launching = null;
      this.running = game ?? this.running;
    }
    if (event.type === 'session_ended' || event.type === 'session_failed') {
      const ended = event.session;
      this.launching = null;
      this.running = null;
      this.overlayDismissed = false;
      void this.api.snapshot().then((snap) => {
        this.applySnapshot(snap);
        const game = this.games.find((g) => g.id === ended.edition_id);
        if (game) {
          this.gameId = game.id;
          this.heroId = game.id;
          this.route = 'game';
        }
        const seconds = ended.duration_seconds ?? 0;
        if (event.type === 'session_ended') {
          this.notify(`${game?.title ?? 'Game'} closed · ${Math.max(seconds, 0)}s played`);
        } else {
          this.notify(ended.error ?? 'Launch failed');
        }
      });
    }
    if (event.type === 'library_changed' || event.type === 'scan_finished') {
      void this.api.snapshot().then((snap) => this.applySnapshot(snap)).catch(() => {});
      void this.api.listPorts().then((list) => this.applyPorts(list)).catch(() => {});
    }
    if (event.type === 'tool_install_started' && event.tool_id === 'pcsx2') {
      this.installingPcsx2 = true;
      this.installProgress = `Installing PCSX2 (${event.channel})…`;
    }
    if (event.type === 'tool_install_progress' && event.tool_id === 'pcsx2') {
      this.installProgress = event.message;
    }
    if (event.type === 'tool_install_finished' && event.tool_id === 'pcsx2') {
      this.installingPcsx2 = false;
      this.installProgress = 'PCSX2 is installed. Open setup to add your BIOS files.';
      this.notify('PCSX2 is installed. Open setup to add your BIOS files — Marquee will not download them.');
      void this.api.snapshot().then((snap) => this.applySnapshot(snap));
    }
    if (event.type === 'tool_install_failed' && event.tool_id === 'pcsx2') {
      this.installingPcsx2 = false;
      this.installProgress = null;
      this.notify(event.message);
    }
    if (event.type === 'tool_install_progress' && event.tool_id in this.emulatorBusy) {
      this.emulatorBusy[event.tool_id] = event.message;
    }
    if ((event.type === 'tool_install_finished' || event.type === 'tool_install_failed')
      && this.emulators.some((e) => e.id === event.tool_id && e.source_build)) {
      void this.api.listEmulators().then((list) => (this.emulators = list)).catch(() => {});
      this.notify(event.type === 'tool_install_failed' ? event.message : 'Source build installed. Open it from Tools.');
    }
    if (event.type === 'port_progress') {
      if (event.port_id in this.portBusy) this.portBusy[event.port_id] = event.message;
    }
    if (event.type === 'artwork_finished') {
      this.fetchingArt = false;
      this.notify(`Covers: ${event.fetched} fetched · ${event.skipped} skipped · ${event.failed} failed`);
      void this.api.snapshot().then((snap) => this.applySnapshot(snap));
    }
  }

  go(route: Route, gameId: string | null = route === 'game' ? this.gameId : null) {
    if (route === 'ports') {
      Object.assign(libraryView('all'), { discovery: false, sourceFilter: 'port', platform: '', query: '', region: '', language: '', revision: '', playerMode: '', installedOnly: false, noFiles: false, favOnly: false });
      route = 'library'; gameId = null;
    }
    if (route === this.route && gameId === this.gameId) return;
    this.history.push({ route: this.route, gameId: this.gameId, heroId: this.heroId });
    this.route = route;
    this.gameId = gameId;
  }

  private previewTimer: ReturnType<typeof setTimeout> | undefined;

  /** Open the supported system picker from any screen. */
  addGames() {
    libraryView('all').discovery = false;
    libraryView('all').addMenuOpen = true;
    this.go('library');
  }

  /** Big Screen focus previews the selected game after a short pause. */
  preview(id: string) {
    clearTimeout(this.previewTimer);
    if (id === this.heroId) return;
    this.previewTimer = setTimeout(() => (this.heroId = id), 120);
  }

  open(game: GameEntry) {
    clearTimeout(this.previewTimer);
    this.go('game', game.id);
    this.heroId = game.id;
  }

  chooseVariant(id: string) {
    const chosen = this.games.find((g) => g.id === id);
    if (!chosen || !this.current || canonicalId(chosen) !== canonicalId(this.current)) return;
    this.variantPreferences = { ...this.variantPreferences, [canonicalId(chosen)]: id };
    this.gameId = id;
    this.heroId = id;
  }

  openInstall(toolId: string, optionId?: string) {
    this.installToolId = toolId;
    this.installOptionId = optionId ?? null;
  }

  closeInstall() {
    this.installToolId = null;
    if (!this.installingPcsx2) this.installOptionId = null;
  }

  async runInstall(toolId: string, optionId: string) {
    const option = installOption(toolId, optionId);
    if (!option) return;
    this.installToolId = toolId;
    this.installOptionId = optionId;
    await this.installPcsx2(option.run.channel);
  }

  back() {
    if (this.intakeOpen) { this.intakeOpen = false; return; }
    if (this.portDialogId) { this.portDialogId = null; return; }
    if (this.emulatorDialogId) { this.emulatorDialogId = null; return; }
    if (this.closeLauncherEditor) { this.closeLauncherEditor(); return; }
    if (this.installToolId) {
      this.closeInstall();
      return;
    }
    if (this.running || this.launching) {
      return this.dismissOverlay();
    }
    const prev = this.history.pop();
    if (prev) {
      clearTimeout(this.previewTimer);
      this.route = prev.route;
      this.gameId = prev.gameId;
      this.heroId = prev.heroId;
    }
  }

  play(game: GameEntry) {
    music.pauseForGame();
    if (game.source === 'catalogue') { this.notify('Catalogue entry only. Open its source page or link an existing installation.'); return; }
    this.heroId = game.id;
    if (game.lutris) { void this.openLutrisGame(game); return; }
    if (game.source === 'port') {
      const port = this.ports.find((p) => `port:${p.id}` === game.id);
      if (port) void this.launchPort(port);
      return;
    }
    void this.playDesktop(game);
  }

  private async playDesktop(game: GameEntry) {
    this.overlayDismissed = false;
    this.launching = game;
    try {
      await this.api.launch(game.id);
    } catch (err) {
      this.launching = null;
      this.running = null;
      const message = (err as { message?: string }).message ?? 'Launch failed';
      this.notify(message);
    }
  }

  dismissOverlay() {
    this.overlayDismissed = true;
    this.launching = null;
    this.running = null;
  }


  toggleFavorite(game: GameEntry) {
    const entry = this.games.find((x) => x.id === game.id);
    if (!entry) return;
    if (entry.source === 'port' || entry.source === 'catalogue' || entry.lutris) return;
    entry.favorite = !entry.favorite;
    void this.api.setFavourite(entry.id, entry.favorite).catch((err) => {
      entry.favorite = !entry.favorite;
      this.notify((err as { message?: string }).message ?? 'Could not save favourite');
    });
  }

  async rename(game: GameEntry, title: string) {
    const entry = this.games.find((x) => x.id === game.id);
    if (!entry) return;
    const previous = entry.title;
    entry.title = title;
    try {
      await this.api.setDisplayTitle(entry.id, title);
    } catch (err) {
      entry.title = previous;
      this.notify((err as { message?: string }).message ?? 'Could not rename');
    }
  }

  /** Add a game that has no files yet; its cover and facts are looked up by name. */
  async addWantedGame(system: string, title: string, status: 'wishlist' | 'owned') {
    try {
      const id = await this.api.addWantedGame(system, title, status);
      this.applySnapshot(await this.api.snapshot());
      void this.refreshArtwork(id);
      return id;
    } catch (err) {
      this.notify((err as { message?: string }).message ?? 'Could not add the game');
      return null;
    }
  }

  async setWantedStatus(game: GameEntry, status: 'wishlist' | 'owned') {
    try {
      await this.api.setWantedStatus(game.id, status);
      this.applySnapshot(await this.api.snapshot());
    } catch (err) {
      this.notify((err as { message?: string }).message ?? 'Could not change the status');
    }
  }

  async removeWantedGame(game: GameEntry) {
    try {
      await this.api.removeWantedGame(game.id);
      this.applySnapshot(await this.api.snapshot());
      this.back();
    } catch (err) {
      this.notify((err as { message?: string }).message ?? 'Could not remove the game');
    }
  }

  async addPs2Folder() {
    return this.addSystemFolder('ps2');
  }

  /** Pick a folder and import it for a supported system. */
  async addSystemFolder(system: string) {
    const path = await this.api.pickRoot();
    if (!path) return;
    this.scanning = true;
    try {
      const root = await this.api.addRoot(path, system);
      const report = await this.api.scan(root.id);
      this.lastScan = report;
      this.applySnapshot(await this.api.snapshot());
      this.notify(describeScan(report));
      if (this.libraryStatus === 'empty' && report.skipped_unsupported && !report.added) {
        this.notify(
          system === 'ds' ? 'Nothing imported. Extract .nds cartridges from their ZIP/7z/RAR archives first.'
          : system === 'neogeo' ? 'Nothing imported. Keep Neo Geo ROM sets zipped; place neogeo.zip and required parent sets alongside them.'
          : system === 'ps1'
            ? 'Nothing imported. PS1 supports .iso, .chd and .cue with its referenced tracks. Standalone BIN files need a CUE sheet.'
            : 'Nothing imported. CHD/CUE/BIN/GZ/CSO are not supported yet — add a folder of .iso files.'
        );
      }
      void this.refreshArtwork();
    } catch (err) {
      this.notify((err as { message?: string }).message ?? 'Could not add folder');
    } finally {
      this.scanning = false;
    }
  }

  async rescan() {
    this.scanning = true;
    try {
      const report = await this.api.scan();
      this.lastScan = report;
      this.applySnapshot(await this.api.snapshot());
      this.notify(describeScan(report));
      void this.refreshArtwork();
    } catch (err) {
      this.notify((err as { message?: string }).message ?? 'Scan failed');
    } finally {
      this.scanning = false;
    }
  }

  queueArchives(paths: string[]) {
    this.intakeOpen = true;
    if (this.intakeBusy) { this.notify('An archive import is already running. Add more files after it finishes.'); return; }
    const merged = [...new Set([...this.intakePaths, ...paths.map((p) => p.trim()).filter(Boolean)])];
    if (merged.length > 50) { this.notify('Choose at most 50 archives per batch. No new paths were added.'); return; }
    this.intakePaths = merged;
    this.intakeDelete = false;
  }

  async importArchives(paths = this.intakePaths, deleteSource = false) {
    if (this.intakeBusy) return;
    const selected = [...new Set(paths.map((p) => p.trim()).filter(Boolean))];
    if (!selected.length || selected.length > 50) { this.notify('Choose between 1 and 50 ZIP/7z/RAR archives.'); return; }
    this.intakeBusy = true; this.intakeResults = []; this.intakeScanError = null;
    let scan = false;
    try {
      for (const [index, path] of selected.entries()) {
        this.intakeProgress = `Importing archive ${index + 1} of ${selected.length}`;
        if (!path.startsWith('/') || path.includes('\0') || path.length > 4096 || !/\.(zip|7z|rar)$/i.test(path)) {
          this.intakeResults.push({ path, report:null, error:'Choose an absolute ZIP, 7z or RAR archive path on the Marquee computer.' });
          continue;
        }
        try {
          const report = await this.api.intakeArchive(path, deleteSource);
          this.intakeResults.push({ path, report, error:null });
          scan ||= ['imported', 'kept_archive', 'already_present'].includes(report.outcome);
        } catch (err) {
          this.intakeResults.push({ path, report:null, error:(err as {message?:string}).message ?? 'Archive import failed' });
        }
      }
      if (scan) {
        this.intakeProgress = 'Scanning imported game files…';
        await this.refreshImportedGames();
      }
      this.intakePaths = this.intakeResults.filter((r) => !!r.error).map((r) => r.path);
      const failed = this.intakeResults.filter((r) => r.error).length;
      this.intakeProgress = failed ? `Import finished · ${failed} ${failed === 1 ? 'archive needs' : 'archives need'} attention` : 'Archive import finished';
      this.notify(this.intakeProgress);
    } finally { this.intakeBusy = false; this.intakeDelete = false; }
  }

  async refreshImportedGames() {
    const wasBusy = this.intakeBusy;
    this.intakeBusy = true; this.intakeScanError = null;
    try {
      this.lastScan = await this.api.scan();
      this.applySnapshot(await this.api.snapshot());
    } catch (err) { this.intakeScanError = (err as {message?:string}).message ?? 'Library refresh failed. Rescan folders to retry.'; }
    finally { this.intakeBusy = wasBusy; }
  }

  // ---- Share games with your own other devices (optional, off by default) ----
  transfers = $state<TransferStatus | null>(null);
  transferError = $state('');
  torrentSettingsBusy = $state(false);
  private transferWatchers = 0;
  private transferTimer: ReturnType<typeof setInterval> | undefined;

  get torrentsOn() { return !!this.runnerSettings.torrents?.enabled; }
  get torrentSettings(): TorrentSettings {
    return { enabled: false, seeding: true, trackers: false, dht: false, upload_kbps: 0, download_kbps: 0, ...this.runnerSettings.torrents };
  }
  transferFor(editionId: string): TransferItem | undefined {
    return this.transfers?.items.find((i) => i.edition_id === editionId);
  }

  async refreshTransfers() {
    if (!this.torrentsOn) { this.transfers = null; return; }
    try { this.transfers = await this.api.transferStatus(); this.transferError = ''; }
    catch (err) { this.transferError = (err as { message?: string }).message ?? 'Could not read saved transfers'; }
  }

  /** Poll while a screen that shows transfers is open. Returns the cleanup. */
  watchTransfers() {
    this.transferWatchers++;
    void this.refreshTransfers();
    this.transferTimer ??= setInterval(() => void this.refreshTransfers(), 3000);
    return () => {
      this.transferWatchers = Math.max(0, this.transferWatchers - 1);
      if (this.transferWatchers === 0 && this.transferTimer) { clearInterval(this.transferTimer); this.transferTimer = undefined; }
    };
  }

  async setTorrents(patch: Partial<TorrentSettings>) {
    if (this.torrentSettingsBusy) return;
    this.torrentSettingsBusy = true;
    try {
      await this.api.setTorrentSettings({ ...this.torrentSettings, ...patch });
      this.applySnapshot(await this.api.snapshot());
      await this.refreshTransfers();
    } catch (err) {
      this.notify((err as { message?: string }).message ?? 'Could not change sharing settings');
    } finally { this.torrentSettingsBusy = false; }
  }

  async shareGame(game: GameEntry) {
    try {
      await this.api.transferShare(game.id);
      await this.refreshTransfers();
    } catch (err) {
      this.notify((err as { message?: string }).message ?? 'Could not share this game');
    }
  }

  async getFromLink(link: string) {
    try {
      await this.api.transferGet(link.trim());
      this.notify('Download queued. Progress is under Settings → Torrents and devices.');
      await this.refreshTransfers();
    } catch (err) {
      this.notify((err as { message?: string }).message ?? 'Could not fetch that game');
    }
  }

  async stopTransfer(item: TransferItem) {
    try {
      await this.api.transferStop(item.info_hash);
      await this.refreshTransfers();
      this.notify('Stopped. Your game files were not touched.');
    } catch (err) {
      this.notify((err as { message?: string }).message ?? 'Could not stop');
    }
  }

  async pauseTransfer(item: TransferItem, paused: boolean) {
    try {
      if (paused) await this.api.transferPause(item.info_hash);
      else await this.api.transferResume(item.info_hash);
      await this.refreshTransfers();
    } catch (err) { this.notify((err as { message?: string }).message ?? 'Could not change transfer'); }
  }

  async copyTransferLink(item: TransferItem) {
    if (!item.link) return;
    try {
      await navigator.clipboard.writeText(item.link);
      this.notify('Link copied. Paste it into Marquee on your other device.');
    } catch {
      this.notify(item.link);
    }
  }

  async setDisplayMode(mode: 'window' | 'fullscreen') {
    try {
      await this.api.setDisplayMode(mode);
      this.applySnapshot(await this.api.snapshot());
    } catch (err) {
      this.notify((err as { message?: string }).message ?? 'Could not change how games open');
    }
  }

  async chooseRunner(id: string) {
    try {
      await this.api.selectRunner(id);
      this.applySnapshot(await this.api.snapshot());
    } catch (err) {
      this.notify((err as { message?: string }).message ?? 'Could not select runner');
    }
  }

  async refreshRunners() {
    try { await this.api.listRunners(); this.applySnapshot(await this.api.snapshot()); }
    catch (err) { this.notify((err as { message?: string }).message ?? 'Could not refresh emulators'); }
  }

  /** Marquee configures emulators itself; the only thing it cannot supply is the user's own BIOS. */
  explainBios() {
    this.notify('Put your BIOS files (or a zip of them) in Marquee’s bios-inbox folder, or use Tools → BIOS import. They are picked up on the next launch.');
  }

  async installPcsx2(channel = 'auto') {
    if (this.installingPcsx2) return;
    this.installingPcsx2 = true;
    this.installProgress = 'Starting PCSX2 install…';
    try {
      await this.api.installPcsx2(channel);
      // The finished event normally clears this, but a dropped event must not leave it stuck.
      this.installingPcsx2 = false;
      this.applySnapshot(await this.api.snapshot());
    } catch (err) {
      this.installingPcsx2 = false;
      this.installProgress = null;
      this.notify((err as { message?: string }).message ?? 'Could not install PCSX2');
    }
  }

  async refreshArtwork(editionId?: string) {
    if (this.fetchingArt) return;
    this.fetchingArt = true;
    try {
      await this.api.refreshArtwork(editionId);
    } catch (err) {
      this.fetchingArt = false;
      this.notify((err as { message?: string }).message ?? 'Could not fetch covers');
    }
  }

  private applyPorts(list: PortInfo[]) {
    this.ports = list.map((p) => ({
      id: p.id,
      name: p.name,
      game: p.game,
      family: p.family,
      platform: p.platform as PortEntry['platform'],
      upstream: p.upstream,
      version: p.installed_version ?? p.latest_version ?? '',
      status: p.status === 'not_installed' ? 'not-installed' : p.status === 'ready' ? 'installed' : p.status,
      requirement:
        p.status === 'update' && p.latest_version
          ? `Update ${p.installed_version} → ${p.latest_version}. ${p.requirement}`
          : p.requirement,
      art: artFromSeed(`port:${p.id}`),
    }));
    this.games = [...this.games.filter((g) => g.source !== 'port'), ...this.ports.filter((p) => p.status !== 'not-installed').map(nativePortGame)];
    this.libraryStatus = this.games.length ? 'loaded' : 'empty';
  }

  private applyLutris(snapshot: LutrisSnapshot) {
    this.lutrisSource = snapshot;
    this.games = [...this.games.filter((g) => !g.lutris), ...snapshot.games.map(lutrisToGame)];
    if (this.gameId?.startsWith('lutris:') && !this.games.some((g) => g.id === this.gameId)) {
      this.gameId = null;
      if (this.route === 'game') this.route = 'library';
    }
    if (!this.games.some((g) => g.id === this.heroId)) this.heroId = this.games[0]?.id ?? null;
    this.libraryStatus = this.games.length ? 'loaded' : 'empty';
  }
  async loadLutris() {
    const request = ++this.lutrisRequest;
    this.lutrisLoading = true; this.lutrisError = null;
    try { const snapshot = await this.api.lutrisLibrary(); if (request === this.lutrisRequest) this.applyLutris(snapshot); }
    catch (err) { if (request === this.lutrisRequest) this.lutrisError = (err as { message?: string }).message ?? 'Could not read Lutris library'; }
    finally { if (request === this.lutrisRequest) this.lutrisLoading = false; }
  }
  async configureLutris(config: LutrisConfig) {
    const snapshot = await this.api.configureLutris(config);
    ++this.lutrisRequest; this.lutrisLoading = false; this.lutrisError = null;
    this.applyLutris(snapshot);
  }
  async openLutrisGame(game: GameEntry) {
    try { await this.api.openLutrisGame(game.id); this.notify(game.installed ? `Handed ${game.title} to Lutris` : `Opened Lutris installer for ${game.title}`); }
    catch (err) { this.notify((err as { message?: string }).message ?? 'Could not open Lutris game'); }
  }

  async loadLaunchers() {
    const request = ++this.launcherRequest;
    this.launchersLoading = true;
    this.launcherError = null;
    try { const list = await this.api.listLaunchers(); if (request === this.launcherRequest) this.launchers = list; }
    catch (err) { if (request === this.launcherRequest) this.launcherError = (err as { message?: string }).message ?? 'Could not load launchers'; }
    finally { if (request === this.launcherRequest) this.launchersLoading = false; }
  }

  async saveLauncher(config: LauncherConfig): Promise<boolean> {
    try {
      const list = await this.api.saveLauncher(config);
      ++this.launcherRequest;
      this.launchersLoading = false;
      this.launcherError = null;
      this.launchers = list;
      this.notify('Launcher configuration saved');
      return true;
    } catch (err) { throw new Error((err as { message?: string }).message ?? 'Could not save launcher'); }
  }

  async removeLauncher(id: string) {
    try { const list = await this.api.removeLauncher(id); ++this.launcherRequest; this.launchersLoading = false; this.launchers = list; this.notify('Launcher configuration removed'); }
    catch (err) { this.notify((err as { message?: string }).message ?? 'Could not remove launcher'); }
  }

  async openLauncher(id: string) {
    if (this.openingLaunchers.includes(id)) return;
    this.openingLaunchers = [...this.openingLaunchers, id];
    try {
      await this.api.openLauncher(id);
      this.notify(`Opened ${this.launchers.find((l) => l.id === id)?.name ?? 'launcher'}`);
    } catch (err) { this.notify((err as { message?: string }).message ?? 'Could not open launcher'); }
    finally { this.openingLaunchers = this.openingLaunchers.filter((x) => x !== id); }
  }

  async loadKeys() {
    try {
      this.keys = await this.api.listKeys();
    } catch (err) {
      this.notify((err as { message?: string }).message ?? 'Could not load keys');
    }
  }

  async saveKey(id: string, value: string) {
    try {
      this.keys = await this.api.setKey(id, value);
      this.notify('Saved');
    } catch (err) {
      this.notify((err as { message?: string }).message ?? 'Could not save the key');
    }
  }

  async clearKey(id: string) {
    try {
      this.keys = await this.api.clearKey(id);
      this.notify('Removed from Marquee');
    } catch (err) {
      this.notify((err as { message?: string }).message ?? 'Could not remove the key');
    }
  }

  async copyWebUrl() {
    if (!this.webUrl) return;
    try {
      await navigator.clipboard.writeText(this.webUrl);
      this.notify('Address copied. Paste it into a browser on this computer.');
    } catch {
      this.notify(this.webUrl);
    }
  }

  async loadEmulators() {
    try {
      this.emulators = await this.api.listEmulators();
      // A few GitHub calls for installed emulators only; failures keep the offline state.
      void this.api.checkEmulatorUpdates().then((list) => (this.emulators = list)).catch(() => {});
    } catch (err) {
      this.notify((err as { message?: string }).message ?? 'Could not load emulators');
    }
  }

  async loadBios() {
    try {
      this.bios = await this.api.listBios();
    } catch (err) {
      this.notify((err as { message?: string }).message ?? 'Could not read BIOS files');
    }
  }

  private async emulatorAction(id: string, label: string, run: () => Promise<void>) {
    if (id in this.emulatorBusy) return;
    this.emulatorBusy[id] = label;
    try {
      await run();
    } catch (err) {
      this.notify((err as { message?: string }).message ?? `Could not ${label.toLowerCase()}`);
    } finally {
      delete this.emulatorBusy[id];
    }
  }

  /** Emulators the library folders need that are not installed yet (link downloads only). */
  get neededEmulators(): EmulatorInfo[] {
    const systems = new Set(this.roots.map((r) => r.system));
    return this.emulators.filter((e) => e.integrated && !e.source_build && e.status === 'not_installed' && e.systems.some((s) => systems.has(s)));
  }

  async installNeededEmulators() {
    for (const emulator of this.neededEmulators) await this.installEmulator(emulator);
  }

  async installEmulator(emulator: EmulatorInfo) {
    if (emulator.source_build) { await this.buildEmulator(emulator); return; }
    await this.emulatorAction(emulator.id, `Installing ${emulator.name}…`, async () => {
      this.emulators = await this.api.installEmulator(emulator.id);
      await this.api.snapshot().then((snap) => this.applySnapshot(snap)).catch(() => {});
      this.notify(`${emulator.name} is installed. ${emulator.requirement}`);
    });
  }

  async uninstallEmulator(emulator: EmulatorInfo, purgeData: boolean) {
    await this.emulatorAction(emulator.id, `Removing ${emulator.name}…`, async () => {
      this.emulators = await this.api.uninstallEmulator(emulator.id, purgeData);
      await this.api.snapshot().then((snap) => this.applySnapshot(snap)).catch(() => {});
      this.notify(purgeData ? `${emulator.name} and its saves were removed` : `${emulator.name} was removed; its settings and saves are kept`);
    });
  }

  async buildEmulator(emulator: EmulatorInfo) {
    await this.emulatorAction(emulator.id, 'Starting source build…', async () => {
      await this.api.buildEmulator(emulator.id);
      this.notify(`${emulator.name} source build started. Progress and logs are in its Tools dialog.`);
    });
  }

  async rollbackEmulator(emulator: EmulatorInfo) {
    await this.emulatorAction(emulator.id, 'Restoring previous version…', async () => {
      this.emulators = await this.api.rollbackEmulator(emulator.id);
      this.notify(`${emulator.name}: restored the previous program. Saves were kept.`);
    });
  }

  async openEmulator(emulator: EmulatorInfo) {
    await this.emulatorAction(emulator.id, `Opening ${emulator.name}…`, async () => {
      await this.api.openEmulator(emulator.id);
      this.notify(`Opened ${emulator.name}`);
    });
  }

  async configureEmulator(emulator: EmulatorInfo, args: string[]): Promise<boolean> {
    try {
      this.emulators = await this.api.configureEmulator(emulator.id, args);
      this.notify(`${emulator.name} settings saved`);
      return true;
    } catch (err) {
      throw new Error((err as { message?: string }).message ?? 'Could not save the settings');
    }
  }

  async importBios() {
    const path = await this.api.pickRoot();
    if (!path || this.biosBusy) return;
    this.biosBusy = true;
    try {
      const report = await this.api.importBios(path);
      await this.loadBios();
      const added = report.added.length;
      this.notify(added ? `Added ${added} BIOS file${added === 1 ? '' : 's'}` : 'No new BIOS files found in that folder');
      void this.api.snapshot().then((snap) => this.applySnapshot(snap)).catch(() => {});
    } catch (err) {
      this.notify((err as { message?: string }).message ?? 'Could not import BIOS files');
    } finally {
      this.biosBusy = false;
    }
  }

  async removeBios(system: string) {
    if (this.biosBusy) return;
    this.biosBusy = true;
    try {
      const removed = await this.api.removeBios(system);
      await this.loadBios();
      this.notify(`Removed ${removed} ${system.toUpperCase()} BIOS file${removed === 1 ? '' : 's'}`);
      void this.api.snapshot().then((snap) => this.applySnapshot(snap)).catch(() => {});
    } catch (err) {
      this.notify((err as { message?: string }).message ?? 'Could not remove BIOS files');
    } finally {
      this.biosBusy = false;
    }
  }

  async loadPorts() {
    try {
      this.applyPorts(await this.api.listPorts());
      // Update check is a handful of GitHub calls; failures keep the offline state.
      void this.api
        .checkPortUpdates()
        .then((list) => this.applyPorts(list))
        .catch(() => {});
    } catch (err) {
      this.notify((err as { message?: string }).message ?? 'Could not load ports');
    }
  }

  async runPortAction(port: PortEntry) {
    if (port.id in this.portBusy) return;
    const operation = portAction(port).operation;
    if (operation === 'install') await this.installPort(port);
    else if (operation === 'folder') await this.openPortFolder(port);
    else await this.launchPort(port);
  }

  async installPort(port: PortEntry) {
    if (port.id in this.portBusy) return;
    delete this.portErrors[port.id];
    this.portBusy[port.id] = `Installing ${port.name}…`;
    try {
      this.applyPorts(await this.api.installPort(port.id));
      this.notify(`${port.name} is installed. ${port.requirement}`);
    } catch (err) {
      this.portErrors[port.id] = (err as { message?: string }).message ?? `Could not install ${port.name}`;
      this.notify(this.portErrors[port.id]);
    } finally {
      delete this.portBusy[port.id];
    }
  }

  async launchPort(port: PortEntry) {
    music.pauseForGame();
    delete this.portErrors[port.id];
    try {
      await this.api.launchPort(port.id);
      this.notify(`Starting ${port.name}…`);
    } catch (err) {
      this.portErrors[port.id] = (err as { message?: string }).message ?? `Could not start ${port.name}`;
      this.notify(this.portErrors[port.id]);
    }
  }

  async openPortFolder(port: PortEntry) {
    delete this.portErrors[port.id];
    try {
      await this.api.openPortFolder(port.id);
    } catch (err) {
      this.portErrors[port.id] = (err as { message?: string }).message ?? 'Could not open the folder';
      this.notify(this.portErrors[port.id]);
    }
  }

  async catalogueAction(action: () => Promise<unknown>, message: string) {
    if (this.catalogueBusy) return false;
    this.catalogueBusy = true; this.catalogueError = null;
    try {
      await action();
      this.applySnapshot(await this.api.snapshot());
      this.notify(message);
      return true;
    } catch (err) {
      this.catalogueError = (err as { message?: string }).message ?? 'Catalogue operation failed';
      this.notify(this.catalogueError);
      return false;
    } finally { this.catalogueBusy = false; }
  }

  async importCatalogueFile(file: File) {
    return this.catalogueAction(async () => {
      if (file.size > 900_000) throw new Error('Snapshot exceeds 900 KB. Import a smaller catalogue batch.');
      await this.api.importCatalogue(JSON.parse(await file.text()));
    }, 'Catalogue metadata imported');
  }

  async openCataloguePage(recordId: string) {
    try { await this.api.openCataloguePage(recordId); }
    catch (err) { this.notify((err as { message?: string }).message ?? 'Could not open the browser'); }
  }

  notify(msg: string) {
    this.toast = msg;
    const shown = msg;
    setTimeout(() => {
      if (this.toast === shown) this.toast = null;
    }, 2800);
  }
}

export const app = new AppState();
