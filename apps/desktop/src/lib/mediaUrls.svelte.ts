import { asError } from './api/core';
import type { MediaAsset } from './api/contracts';

/** Resolve only the displayed collection. A failed item never hides its peers. */
export class MediaUrls {
  urls = $state<Record<string, string>>({});
  errors = $state<Record<string, string>>({});
  pending = $state<Record<string, boolean>>({});
  private requests = new Map<string, Promise<void>>();
  private disposed = false;

  constructor(private resolve: (id: string) => Promise<string>) {}

  load(assets: MediaAsset[]): Promise<void[]> {
    return Promise.all(assets.filter(asset => asset.available).map(asset => this.loadOne(asset.id)));
  }

  private loadOne(id: string): Promise<void> {
    if (this.disposed || this.urls[id] || this.errors[id]) return Promise.resolve();
    const pending = this.requests.get(id);
    if (pending) return pending;
    this.pending = { ...this.pending, [id]: true };
    const request = Promise.resolve().then(() => this.resolve(id)).then(url => {
      if (!this.disposed) this.urls = { ...this.urls, [id]: url };
    }).catch(error => {
      if (!this.disposed) this.fail(id, asError(error).message);
    }).finally(() => {
      this.requests.delete(id);
      if (!this.disposed) this.pending = { ...this.pending, [id]: false };
    });
    this.requests.set(id, request);
    return request;
  }

  fail(id: string, message = 'This image could not load. Try again.') {
    const { [id]: removed, ...urls } = this.urls;
    this.urls = urls;
    this.errors = { ...this.errors, [id]: message };
  }

  retry(id: string): Promise<void> {
    const { [id]: removed, ...errors } = this.errors;
    this.errors = errors;
    return this.loadOne(id);
  }

  dispose() { this.disposed = true; }
}
