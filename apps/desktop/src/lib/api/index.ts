import type { LibraryApi } from './contracts';
import { desktopApi, isDesktopRuntime } from './desktop';
import { demoApi } from './demo';
import { httpApi, webToken } from './http';

export type { LibraryApi } from './contracts';
export { isDesktopRuntime } from './desktop';
export { webToken } from './http';

function forcedDemo(): boolean {
  return typeof window !== 'undefined' && new URLSearchParams(window.location.search).get('demo') === '1';
}

/**
 * Same UI, same data, three ways in:
 * - native window → the core over Tauri IPC
 * - browser holding a token from a running Marquee → the same core over local HTTP
 * - anything else (or `?demo=1`) → the invented demo library, for design work
 */
export function createApi(): LibraryApi {
  if (isDesktopRuntime()) return desktopApi;
  if (!forcedDemo() && webToken()) return httpApi;
  return demoApi;
}
