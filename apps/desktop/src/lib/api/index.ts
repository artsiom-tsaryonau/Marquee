import type { LibraryApi } from './contracts';
import { desktopApi, isDesktopRuntime } from './desktop';
import { httpApi } from './http';

export type { LibraryApi } from './contracts';
export { isDesktopRuntime } from './desktop';
export { webToken } from './http';

/**
 * Same UI, same data, two ways in:
 * - native window → the core over Tauri IPC
 * - browser holding a token from a running Marquee → the same core over local HTTP
 */
export function createApi(): LibraryApi {
  return isDesktopRuntime() ? desktopApi : httpApi;
}
