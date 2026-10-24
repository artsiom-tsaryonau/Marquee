import type { LibraryApi, MusicAlbum, MusicTrack } from './api/contracts';
import { asError } from './api/core';

type AudioOutput = Pick<HTMLAudioElement, 'src' | 'currentTime' | 'volume' | 'pause' | 'load' | 'play'>;
type QueueTrack = MusicTrack & { album: string; artist: string };

/** Playback belongs to the launcher, not to the mounted game details page. */
export class MusicPlayer {
  queue = $state<QueueTrack[]>([]);
  index = $state(0);
  error = $state('');
  loading = $state(false);
  paused = $state(true);
  pauseOnGame = $state(true);
  note = $state('');
  private output: AudioOutput | null = null;
  private generation = 0;
  get current() { return this.queue[this.index]; }
  attach(output: AudioOutput) { this.output = output; }
  detach() { this.pause(); this.output = null; }
  async play(api: LibraryApi, album: MusicAlbum, trackId: string) {
    const tracks = album.tracks.filter(t => t.available);
    const index = tracks.findIndex(t => t.id === trackId);
    if (index < 0) { this.error = 'Track unavailable. Add its audio files again.'; return; }
    this.queue = tracks.map(t => ({ ...t, album: album.title, artist: album.artist }));
    this.index = index;
    await this.load(api);
  }
  async next(api: LibraryApi, direction = 1) {
    const next = this.index + direction;
    if (next < 0 || next >= this.queue.length) { this.pause(); return; }
    this.index = next; await this.load(api);
  }
  private async load(api: LibraryApi) {
    const ticket = ++this.generation;
    const output = this.output;
    if (!output || !this.current) return;
    output.pause(); this.loading = true; this.error = ''; this.note = '';
    try {
      const url = await api.musicUrl(this.current.id);
      if (ticket !== this.generation) return;
      output.src = url; output.load();
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
