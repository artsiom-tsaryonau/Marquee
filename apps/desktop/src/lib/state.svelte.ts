import { downloads as seedDownloads, games, ports } from './mock';
import type { Download, GameEntry, PortEntry } from './types';

export type Route = 'home' | 'library' | 'game' | 'ports' | 'downloads' | 'settings';

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
  history: Route[] = [];
  gameId = $state<string | null>(null);
  /** The entry whose art fills the backdrop. */
  heroId = $state<string>(games[0].id);

  mode = $state<'desktop' | 'big'>(load('mode', 'desktop'));
  theme = $state<'dark' | 'light'>(load('theme', 'dark'));
  reduceMotion = $state<boolean>(load('reduceMotion', false));
  sounds = $state<boolean>(load('sounds', false));

  games = $state<GameEntry[]>(games);
  ports = $state<PortEntry[]>(ports);
  downloads = $state<Download[]>(seedDownloads);

  launching = $state<GameEntry | null>(null);
  running = $state<GameEntry | null>(null);
  toast = $state<string | null>(null);
  lastInput = $state<string>('—');

  hero = $derived(this.games.find((g) => g.id === this.heroId) ?? this.games[0]);
  current = $derived(this.games.find((g) => g.id === this.gameId) ?? null);

  constructor() {
    $effect.root(() => {
      $effect(() => save('mode', this.mode));
      $effect(() => save('theme', this.theme));
      $effect(() => save('reduceMotion', this.reduceMotion));
      $effect(() => save('sounds', this.sounds));
    });
  }

  go(route: Route) {
    if (route === this.route) return;
    this.history.push(this.route);
    this.route = route;
  }

  private previewTimer: ReturnType<typeof setTimeout> | undefined;

  /** Hover/focus swaps the backdrop after a short pause, so sweeping across a grid doesn't stack fades. */
  preview(id: string) {
    clearTimeout(this.previewTimer);
    this.previewTimer = setTimeout(() => (this.heroId = id), 120);
  }

  open(game: GameEntry) {
    this.gameId = game.id;
    this.heroId = game.id;
    this.go('game');
  }

  back() {
    if (this.running) return this.stop();
    const prev = this.history.pop();
    if (prev) this.route = prev;
  }

  play(game: GameEntry) {
    this.heroId = game.id;
    this.launching = game;
    setTimeout(() => {
      if (this.launching?.id !== game.id) return;
      this.launching = null;
      this.running = game;
    }, 2200);
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
    if (entry) entry.favorite = !entry.favorite;
  }

  install(name: string) {
    if (this.downloads.some((d) => d.name === name && d.state !== 'done')) return;
    this.downloads.unshift({ id: crypto.randomUUID(), name, progress: 0, speed: '0 MB/s', state: 'downloading' });
    this.notify(`${name} added to downloads`);
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

// Fake download progress so the Downloads screen moves.
setInterval(() => {
  let active = 0;
  for (const d of app.downloads) {
    if (d.state !== 'downloading') continue;
    active++;
    d.progress = Math.min(1, d.progress + 0.01 + Math.random() * 0.02);
    d.speed = `${(12 + Math.random() * 14).toFixed(1)} MB/s`;
    if (d.progress >= 1) {
      d.state = 'done';
      d.speed = '';
      const port = app.ports.find((p) => d.name.startsWith(p.name));
      if (port) port.status = 'ready';
      app.notify(`${d.name} installed`);
    }
  }
  if (active === 0) {
    const next = app.downloads.find((d) => d.state === 'queued');
    if (next) next.state = 'downloading';
  }
}, 400);
