import type { LibraryApi, StreamingState } from './api/contracts';
import { asError } from './api/core';

/** One request at a time; navigation stops polling, never a game or viewer. */
export class StreamingControls {
  state = $state<StreamingState | null>(null);
  error = $state('');
  refreshError = $state('');
  busy = $state(false);
  confirmation = $state('');
  private disposed = false;
  constructor(private api: LibraryApi) {}
  dispose() { this.disposed = true; }
  async refresh() {
    if (this.busy || this.disposed) return;
    this.busy = true;
    try {
      const next = await this.api.streamingState();
      if (!this.disposed) { this.state = next; this.refreshError = ''; }
    } catch (err) { if (!this.disposed) this.refreshError = asError(err).message; }
    finally { this.busy = false; }
  }
  async act(action: () => Promise<unknown>) {
    if (this.busy || this.disposed) return;
    this.busy = true; this.error = ''; this.confirmation = '';
    try { await action(); }
    catch (err) { if (!this.disposed) this.error = asError(err).message; }
    finally { this.busy = false; }
    // Preserve the action error even when refreshing the host succeeds.
    const error = this.error;
    await this.refresh();
    if (!this.disposed && error) this.error = error;
  }
  async confirm(kind: 'end' | 'revoke' | 'headless', id: string) {
    const key = `${kind}:${id}`;
    if (this.busy || this.disposed) return;
    if (this.confirmation !== key) { this.confirmation = key; return; }
    await this.act(() => kind === 'end' ? this.api.streamingEndGame(id) : kind === 'headless' ? this.api.headlessStop() : this.api.streamingRevoke(id));
  }
}
