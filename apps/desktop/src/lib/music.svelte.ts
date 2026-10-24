import type { LibraryApi, MusicAlbum, MusicTrack } from './api/contracts';
import { asError } from './api/core';

type AudioOutput = Pick<HTMLAudioElement, 'src' | 'currentTime' | 'duration' | 'volume' | 'muted' | 'pause' | 'load' | 'play'>;
type QueueTrack = MusicTrack & { albumId: string; album: string; artist: string };

/** Hide an import's redundant ordinal only when it matches this track's position. */
export function trackTitle(title: string, position: number): string {
  const match = title.match(/^(\d{1,3})[ ._-]+(.+)$/);
  return match && Number(match[1]) === position ? match[2] : title;
}
export function musicTime(seconds: number): string {
  const value = Number.isFinite(seconds) ? Math.max(0, Math.floor(seconds)) : 0;
  return `${Math.floor(value / 60)}:${String(value % 60).padStart(2, '0')}`;
}

/** Playback belongs to the launcher, not to the mounted game details page. */
export class MusicPlayer {
  queue = $state<QueueTrack[]>([]);
  index = $state(0);
  error = $state('');
  loading = $state(false);
  paused = $state(true);
  pauseOnGame = $state(true);
  note = $state('');
  position = $state(0);
  duration = $state(0);
  volume = $state(0.65);
  muted = $state(false);
  private output: AudioOutput | null = null;
  private generation = 0;
  private loadedTrackId: string | null = null;
  private lastAudibleVolume = 0.65;
  get current() { return this.queue[this.index]; }
  attach(output: AudioOutput) { this.output = output; output.volume = this.volume; output.muted = this.muted; if (this.volume > 0) this.lastAudibleVolume = this.volume; }
  detach() { this.pause(); this.output = null; this.loadedTrackId = null; }
  async play(api: LibraryApi, album: MusicAlbum, trackId: string) {
    const tracks = album.tracks.filter(t => t.available);
    const index = tracks.findIndex(t => t.id === trackId);
    if (index < 0) { this.error = 'Track unavailable. Add its audio files again.'; return; }
    this.queue = tracks.map(t => ({ ...t, title: trackTitle(t.title, album.tracks.findIndex(original => original.id === t.id) + 1), albumId: album.id, album: album.title, artist: album.artist }));
    this.index = index;
    await this.load(api);
  }
  async next(api: LibraryApi, direction = 1) {
    const next = this.index + direction;
    if (next < 0 || next >= this.queue.length) { this.pause(); return; }
    this.index = next; await this.load(api);
  }
  async select(api: LibraryApi, album: MusicAlbum, trackId: string) {
    if (this.current?.id === trackId && this.current.albumId === album.id) await this.toggle(api);
    else await this.play(api, album, trackId);
  }
  async toggle(api: LibraryApi) {
    if (this.loading || !this.paused) { this.pause(); return; }
    if (!this.output || !this.current) return;
    if (this.error || !this.output.src || this.loadedTrackId !== this.current.id) { await this.load(api); return; }
    const ticket = ++this.generation;
    this.note = ''; this.error = '';
    try {
      if (this.duration > 0 && this.output.currentTime >= this.duration) this.seek(0);
      await this.output.play();
      if (ticket === this.generation) this.paused = false;
    } catch (err) { if (ticket === this.generation) this.error = `Playback could not resume. ${asError(err).message}`; }
  }
  async previous(api: LibraryApi) {
    if (this.position > 3 || this.index === 0) this.seek(0);
    else await this.next(api, -1);
  }
  syncTime() {
    if (!this.output) return;
    this.duration = Number.isFinite(this.output.duration) ? Math.max(0, this.output.duration) : 0;
    this.position = Number.isFinite(this.output.currentTime) ? Math.max(0, this.output.currentTime) : 0;
  }
  seek(seconds: number) {
    if (!this.output || !Number.isFinite(seconds) || this.duration <= 0) return;
    this.output.currentTime = Math.max(0, Math.min(seconds, this.duration));
    this.position = this.output.currentTime;
  }
  setVolume(value: number) {
    if (!Number.isFinite(value)) return;
    this.volume = Math.min(1, Math.max(0, value));
    if (this.volume > 0) { this.muted = false; this.lastAudibleVolume = this.volume; }
    if (this.output) { this.output.volume = this.volume; this.output.muted = this.muted; }
  }
  toggleMute() {
    if (this.muted || this.volume === 0) { this.muted = false; if (this.volume === 0) this.volume = this.lastAudibleVolume; }
    else this.muted = true;
    if (this.output) { this.output.muted = this.muted; this.output.volume = this.volume; }
  }
  private async load(api: LibraryApi) {
    const ticket = ++this.generation;
    const output = this.output;
    if (!output || !this.current) return;
    const trackId = this.current.id;
    output.pause(); this.paused = true; this.position = 0; this.duration = 0; this.loading = true; this.error = ''; this.note = '';
    try {
      const url = await api.musicUrl(trackId);
      if (ticket !== this.generation) return;
      output.src = url; this.loadedTrackId = trackId; output.load();
      await output.play();
      if (ticket === this.generation) this.paused = false;
    } catch (err) {
      if (ticket === this.generation) { this.paused = true; this.error = `Playback could not start. ${asError(err).message}`; }
    } finally { if (ticket === this.generation) this.loading = false; }
  }
  pause() { this.generation++; this.output?.pause(); this.paused = true; this.loading = false; }
  pauseForGame() {
    if (!this.pauseOnGame) return;
    this.pause();
    if (this.current) this.note = 'Paused for gameplay. Resume when you want to listen.';
  }
  removed(trackId: string) {
    if (this.current?.id === trackId) { this.pause(); this.queue = []; return; }
    const current = this.current?.id;
    this.queue = this.queue.filter(t => t.id !== trackId);
    this.index = Math.max(0, this.queue.findIndex(t => t.id === current));
  }
}

export const music = new MusicPlayer();
