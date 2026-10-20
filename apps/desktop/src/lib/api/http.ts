import type { AppError, CoreEvent, LibraryApi } from './contracts';
import { asError, commands, type Call } from './core';

const TOKEN_KEY = 'marquee.token';

/**
 * The browser surface of a running Marquee. Open the address the desktop app (or
 * `marquee serve`) prints once: it ends in `#token=…`. The token is kept for next
 * time and removed from the address bar.
 */
export function webToken(): string | null {
  if (typeof window === 'undefined') return null;
  const fromHash = new URLSearchParams(window.location.hash.replace(/^#/, '')).get('token');
  try {
    if (fromHash) {
      localStorage.setItem(TOKEN_KEY, fromHash);
      history.replaceState(null, '', window.location.pathname + window.location.search);
      return fromHash;
    }
    return localStorage.getItem(TOKEN_KEY);
  } catch {
    return fromHash;
  }
}

export function forgetWebToken() {
  try {
    localStorage.removeItem(TOKEN_KEY);
  } catch {
    /* nothing to forget */
  }
}

const token = () => webToken() ?? '';

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  let res: Response;
  try {
    res = await fetch(path, {
      ...init,
      headers: { Authorization: `Bearer ${token()}`, 'Content-Type': 'application/json' },
    });
  } catch {
    throw {
      code: 'UNREACHABLE',
      message: 'Marquee is not running. Start the desktop app (or `marquee serve`), or add ?demo=1 for the demo.',
    } satisfies AppError;
  }
  const body = (await res.json().catch(() => null)) as unknown;
  if (!res.ok) {
    if (res.status === 401) forgetWebToken();
    throw asError(body ?? { code: 'NETWORK', message: `HTTP ${res.status}` });
  }
  return body as T;
}

const call: Call = <T>(cmd: string, args?: Record<string, unknown>) =>
  request<T>(`/api/${cmd}`, { method: 'POST', body: JSON.stringify(args ?? {}) });

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

export const httpApi: LibraryApi = {
  kind: 'web',
  ...commands(call),
  async pickRoot() {
    // A browser cannot open a native folder dialog; the path is on the machine running Marquee.
    const path = window.prompt('Folder on the Marquee machine that holds your PS2 .iso files');
    return path && path.trim() ? path.trim() : null;
  },
  async pickExecutable() {
    return window.prompt('Absolute executable path on the Marquee machine');
  },
  webUrl: async () => null,
  subscribe(handler) {
    let stopped = false;
    void (async () => {
      let after: number | null = null;
      while (!stopped) {
        try {
          const query: string = after === null ? '' : `?after=${after}`;
          const res: { seq: number; events: CoreEvent[] } = await request(`/api/events${query}`);
          after = res.seq;
          for (const event of res.events) handler(event);
        } catch {
          await sleep(2000);
        }
      }
    })();
    return () => {
      stopped = true;
    };
  },
  async whenReady() {},
};
