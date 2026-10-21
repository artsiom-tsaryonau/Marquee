import { createApi, type LibraryApi } from './api';
import { gamesFromSnapshot, pickRunner } from './api/map';
import type { BiosFile, CoreEvent, EmulatorInfo, KeyInfo, LauncherConfig, LauncherInfo, LutrisConfig, LutrisSnapshot, LibraryRoot, LibrarySnapshot, PortInfo, Runner, ScanReport, Session } from './api/contracts';
import { nativePortGame, canonicalId } from './unified';
import { lutrisToGame } from './lutris';
import { artFromSeed } from './art';
import { describeScan } from './format';
import { installOption } from './install';
import { downloads as seedDownloads, games, portGame, ports } from './mock';
import type { Download, DownloadTarget, GameEntry, PortEntry } from './types';

export type Route = 'home' | 'library' | 'game' | 'ports' | 'downloads' | 'settings' | 'ps2' | 'tools';
export type LibraryStatus = 'demo' | 'loading' | 'empty' | 'loaded' | 'failed';

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
  route = $state<Route>('home');
  history: { route: Route; gameId: string | null; heroId: string | null }[] = [];
  gameId = $state<string | null>(null);
  /** The entry whose art fills the backdrop. */
  heroId = $state<string | null>(games[0]?.id ?? null);

  mode = $state<'desktop' | 'big'>(load('mode', 'desktop'));
  theme = $state<'dark' | 'light'>(load('theme', 'dark'));
  reduceMotion = $state<boolean>(load('reduceMotion', false));
  visualEffects = $state<'balanced' | 'full'>(load<string>('visualEffects', 'balanced') === 'full' ? 'full' : 'balanced');
  systemReduceMotion = $state(typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches);
  motionReduced = $derived(this.reduceMotion || this.systemReduceMotion);
  variantPreferences = $state<Record<string, string>>(load('variantPreferences', {}));
  sounds = $state<boolean>(load('sounds', false));

  api: LibraryApi = createApi();
  demo = this.api.kind === 'demo';
  webUrl = $state<string | null>(null);
  keys = $state<KeyInfo[]>([]);
  emulators = $state<EmulatorInfo[]>([]);
  bios = $state<BiosFile[]>([]);
  /** Emulator id → what it is doing right now (install/update progress or a short action). */
  emulatorBusy = $state<Record<string, string>>({});
  biosBusy = $state(false);
  /** Emulator whose settings dialog is open. */
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
  libraryStatus = $state<LibraryStatus>(this.api.kind === 'demo' ? 'demo' : 'loading');
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

  games = $state<GameEntry[]>(this.demo ? games : []);
  ports = $state<PortEntry[]>(this.demo ? ports : []);
  downloads = $state<Download[]>(this.demo ? seedDownloads : []);
  /** Port id → latest progress line while it installs (desktop only). */
  portBusy = $state<Record<string, string>>({});

  launching = $state<GameEntry | null>(null);
  running = $state<GameEntry | null>(null);
  toast = $state<string | null>(null);
  lastInput = $state<string>('—');

  hero = $derived(this.games.find((g) => g.id === this.heroId) ?? this.games[0] ?? null);
  current = $derived(this.games.find((g) => g.id === this.gameId) ?? null);

  constructor() {
    $effect.root(() => {
      $effect(() => save('mode', this.mode));
      $effect(() => save('theme', this.theme));
      $effect(() => save('reduceMotion', this.reduceMotion));
      $effect(() => save('visualEffects', this.visualEffects));
      $effect(() => save('sounds', this.sounds));
      $effect(() => save('variantPreferences', this.variantPreferences));
    });
  }

  async boot() {
    this.disconnect();
    if (this.demo) { await Promise.all([this.loadLaunchers(), this.loadEmulators()]); return; }
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
    this.roots = snap.roots;
    this.runners = snap.runners;
    this.runnerSettings = snap.settings;
    this.selectedRunner = pickRunner(snap);
    this.games = [...gamesFromSnapshot(snap), ...this.games.filter((g) => g.source === 'port' || !!g.lutris)];
    this.libraryStatus = this.games.length ? 'loaded' : 'empty';
    this.loadError = null;
    if (this.heroId && !this.games.some((g) => g.id === this.heroId)) {
      this.heroId = this.games[0]?.id ?? null;
    } else if (!this.heroId) {
      this.heroId = this.games[0]?.id ?? null;
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
    if (route === this.route && gameId === this.gameId) return;
    this.history.push({ route: this.route, gameId: this.gameId, heroId: this.heroId });
    this.route = route;
    this.gameId = gameId;
  }

  private previewTimer: ReturnType<typeof setTimeout> | undefined;

  /** Hover/focus swaps the backdrop after a short pause, so sweeping across a grid doesn't stack fades. */
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
    if (option.run.type === 'setup') {
      await this.openSetup();
      return;
    }
    await this.installPcsx2(option.run.channel);
  }

  back() {
    if (this.emulatorDialogId) { this.emulatorDialogId = null; return; }
    if (this.closeLauncherEditor) { this.closeLauncherEditor(); return; }
    if (this.installToolId) {
      this.closeInstall();
      return;
    }
    if (this.running || this.launching) {
      if (this.demo) return this.stop();
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
    this.heroId = game.id;
    if (!this.demo) {
      if (game.lutris) { void this.openLutrisGame(game); return; }
      if (game.source === 'port') {
        const port = this.ports.find((p) => `port:${p.id}` === game.id);
        if (port) void this.launchPort(port);
        return;
      }
      void this.playDesktop(game);
      return;
    }
    this.launching = game;
    setTimeout(() => {
      if (this.launching?.id !== game.id) return;
      this.launching = null;
      this.running = game;
    }, 2200);
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

  stop() {
    const g = this.running;
    this.running = null;
    this.launching = null;
    if (g) {
      const entry = this.games.find((x) => x.id === g.id);
      if (entry) {
        entry.playtimeMin += 42;
        entry.lastPlayed = new Date().toISOString();
      }
      this.notify(`${g.title} closed · 42 min added`);
    }
  }

  toggleFavorite(game: GameEntry) {
    const entry = this.games.find((x) => x.id === game.id);
    if (!entry) return;
    if (!this.demo && (entry.source === 'port' || entry.lutris)) return;
    entry.favorite = !entry.favorite;
    if (!this.demo) {
      void this.api.setFavourite(entry.id, entry.favorite).catch((err) => {
        entry.favorite = !entry.favorite;
        this.notify((err as { message?: string }).message ?? 'Could not save favourite');
      });
    }
  }

  async rename(game: GameEntry, title: string) {
    const entry = this.games.find((x) => x.id === game.id);
    if (!entry || this.demo) return;
    const previous = entry.title;
    entry.title = title;
    try {
      await this.api.setDisplayTitle(entry.id, title);
    } catch (err) {
      entry.title = previous;
      this.notify((err as { message?: string }).message ?? 'Could not rename');
    }
  }

  async addPs2Folder() {
    return this.addSystemFolder('ps2');
  }

  /** Pick a folder and import it for a specific system (`ps1`/`ps2`). */
  async addSystemFolder(system: string) {
    if (this.demo) {
      this.notify(`Add a ${system.toUpperCase()} folder in the desktop app`);
      return;
    }
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
          system === 'ps1'
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
    if (this.demo) {
      this.notify('Rescanning all sources…');
      return;
    }
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

  async chooseRunner(id: string) {
    if (this.demo) return;
    try {
      await this.api.selectRunner(id);
      this.applySnapshot(await this.api.snapshot());
    } catch (err) {
      this.notify((err as { message?: string }).message ?? 'Could not select runner');
    }
  }

  async refreshRunners() {
    if (this.demo) return;
    try { await this.api.listRunners(); this.applySnapshot(await this.api.snapshot()); }
    catch (err) { this.notify((err as { message?: string }).message ?? 'Could not refresh emulators'); }
  }

  async openSetup(id?: string, system = 'ps2') {
    if (this.demo) return;
    const runnerId = id ?? this.runnerFor(system)?.id;
    if (!runnerId) {
      this.notify(`Set up ${system === 'ps1' ? 'DuckStation' : 'PCSX2'} in Tools, then select your BIOS in the emulator.`);
      return;
    }
    try {
      await this.api.openSetup(runnerId);
    } catch (err) {
      this.notify((err as { message?: string }).message ?? 'Could not open emulator setup');
    }
  }

  async installPcsx2(channel = 'auto') {
    if (this.demo) {
      this.notify('Install PCSX2 in the desktop app');
      return;
    }
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
    if (this.demo) return;
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
      status: p.status === 'not_installed' ? 'not-installed' : p.status,
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
    if (this.demo) return;
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
    if (this.demo) {
      this.emulators = await this.api.listEmulators();
      return;
    }
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
    if (this.demo) {
      this.notify('Manage emulators in the desktop app');
      return;
    }
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
    if (this.demo) {
      this.notify('Configure emulators in the desktop app');
      return false;
    }
    try {
      this.emulators = await this.api.configureEmulator(emulator.id, args);
      this.notify(`${emulator.name} settings saved`);
      return true;
    } catch (err) {
      throw new Error((err as { message?: string }).message ?? 'Could not save the settings');
    }
  }

  async importBios() {
    if (this.demo) {
      this.notify('Import BIOS files in the desktop app');
      return;
    }
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
    if (this.demo || this.biosBusy) return;
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
    if (this.demo) return;
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

  async installPort(port: PortEntry) {
    if (this.demo || port.id in this.portBusy) return;
    this.portBusy[port.id] = `Installing ${port.name}…`;
    try {
      this.applyPorts(await this.api.installPort(port.id));
      this.notify(`${port.name} is installed. ${port.requirement}`);
    } catch (err) {
      this.notify((err as { message?: string }).message ?? `Could not install ${port.name}`);
    } finally {
      delete this.portBusy[port.id];
    }
  }

  async launchPort(port: PortEntry) {
    try {
      await this.api.launchPort(port.id);
      this.notify(`Starting ${port.name}…`);
    } catch (err) {
      this.notify((err as { message?: string }).message ?? `Could not start ${port.name}`);
    }
  }

  async openPortFolder(port: PortEntry) {
    try {
      await this.api.openPortFolder(port.id);
    } catch (err) {
      this.notify((err as { message?: string }).message ?? 'Could not open the folder');
    }
  }

  install(name: string, target?: DownloadTarget) {
    if (!this.demo) {
      this.notify('Downloads are demo-only in this milestone');
      return;
    }
    if (this.downloads.some((d) => d.state !== 'done' && (target
      ? d.target?.kind === target.kind && d.target.id === target.id
      : !d.target && d.name === name))) return;
    this.downloads.unshift({ id: crypto.randomUUID(), name, target, progress: 0, speed: '0 MB/s', state: 'downloading' });
    this.notify(`${name} added to downloads`);
  }

  completeDownload(download: Download) {
    download.progress = 1;
    download.state = 'done';
    download.speed = '';
    const target = download.target;
    if (target?.kind === 'game') {
      const game = this.games.find((g) => g.id === target.id);
      if (game) game.installed = true;
    } else if (target?.kind === 'port') {
      const port = this.ports.find((p) => p.id === target.id);
      if (port) {
        port.status = 'ready';
        const game = this.games.find((g) => g.id === `port:${port.id}`);
        if (game) {
          game.installed = true;
          game.portStatus = 'ready';
        } else {
          const entry = portGame(port);
          this.games.unshift({ ...entry, playtimeMin: 0, lastPlayed: null, addedAt: new Date().toISOString() });
        }
      }
    }
    this.notify(`${download.name} installed`);
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

if (app.demo) {
  // Fake download progress so the Downloads screen moves.
  setInterval(() => {
    let active = 0;
    for (const d of app.downloads) {
      if (d.state !== 'downloading') continue;
      active++;
      d.progress = Math.min(1, d.progress + 0.01 + Math.random() * 0.02);
      d.speed = `${(12 + Math.random() * 14).toFixed(1)} MB/s`;
      if (d.progress >= 1) {
        app.completeDownload(d);
      }
    }
    if (active === 0) {
      const next = app.downloads.find((d) => d.state === 'queued');
      if (next) next.state = 'downloading';
    }
  }, 400);
}
