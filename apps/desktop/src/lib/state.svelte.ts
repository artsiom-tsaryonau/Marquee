import { createApi, type LibraryApi } from './api';
import { gamesFromSnapshot, pickRunner } from './api/map';
import type { CoreEvent, LibraryRoot, LibrarySnapshot, PortInfo, Runner, ScanReport, Session } from './api/contracts';
import { artFromSeed } from './art';
import { describeScan } from './format';
import { installOption } from './install';
import { downloads as seedDownloads, games, portGame, ports } from './mock';
import type { Download, DownloadTarget, GameEntry, PortEntry } from './types';

export type Route = 'home' | 'library' | 'game' | 'ports' | 'downloads' | 'settings' | 'ps2';
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
  sounds = $state<boolean>(load('sounds', false));

  api: LibraryApi = createApi();
  demo = this.api.kind === 'demo';
  webUrl = $state<string | null>(null);
  libraryStatus = $state<LibraryStatus>(this.api.kind === 'demo' ? 'demo' : 'loading');
  loadError = $state<string | null>(null);
  roots = $state<LibraryRoot[]>([]);
  runners = $state<Runner[]>([]);
  selectedRunner = $state<Runner | null>(null);
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
      $effect(() => save('sounds', this.sounds));
    });
  }

  async boot() {
    if (this.demo) return;
    this.unlisten = this.api.subscribe((event) => this.onEvent(event));
    try {
      await this.api.whenReady();
      const snap = await this.api.snapshot();
      this.applySnapshot(snap);
      await this.loadPorts();
      this.webUrl = await this.api.webUrl().catch(() => null);
    } catch (err) {
      this.libraryStatus = 'failed';
      this.loadError = err instanceof Error ? err.message : String((err as { message?: string }).message ?? err);
    }
  }

  private unlisten: (() => void) | undefined;

  applySnapshot(snap: LibrarySnapshot) {
    this.roots = snap.roots;
    this.runners = snap.runners;
    this.selectedRunner = pickRunner(snap);
    this.games = gamesFromSnapshot(snap);
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
      void this.api.snapshot().then((snap) => this.applySnapshot(snap));
    }
    if (event.type === 'tool_install_started') {
      this.installingPcsx2 = true;
      this.installProgress = `Installing PCSX2 (${event.channel})…`;
    }
    if (event.type === 'tool_install_progress') {
      this.installProgress = event.message;
    }
    if (event.type === 'tool_install_finished') {
      this.installingPcsx2 = false;
      this.installProgress = 'PCSX2 is installed. Open setup to add your BIOS files.';
      this.notify('PCSX2 is installed. Open setup to add your BIOS files — Marquee will not download them.');
      void this.api.snapshot().then((snap) => this.applySnapshot(snap));
    }
    if (event.type === 'tool_install_failed') {
      this.installingPcsx2 = false;
      this.installProgress = null;
      this.notify(event.message);
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
    this.previewTimer = setTimeout(() => (this.heroId = id), 120);
  }

  open(game: GameEntry) {
    clearTimeout(this.previewTimer);
    this.go('game', game.id);
    this.heroId = game.id;
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
    if (this.demo) {
      this.notify('Add a PS2 folder in the desktop app');
      return;
    }
    const path = await this.api.pickRoot();
    if (!path) return;
    this.scanning = true;
    try {
      const root = await this.api.addRoot(path);
      const report = await this.api.scan(root.id);
      this.lastScan = report;
      this.applySnapshot(await this.api.snapshot());
      this.notify(describeScan(report));
      if (this.libraryStatus === 'empty' && report.skipped_unsupported && !report.added) {
        this.notify('Nothing imported. CHD/CUE/BIN/GZ/CSO are not supported yet — add a folder of .iso files.');
      }
      if (report.added > 0) {
        void this.refreshArtwork();
      }
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
      if (report.added > 0) {
        void this.refreshArtwork();
      }
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

  async openSetup(id?: string) {
    if (this.demo) return;
    const runnerId = id ?? this.selectedRunner?.id;
    if (!runnerId) {
      this.notify('Install PCSX2 from Settings, then add your own BIOS files.');
      return;
    }
    try {
      await this.api.openSetup(runnerId);
    } catch (err) {
      this.notify((err as { message?: string }).message ?? 'Could not open PCSX2 setup');
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
