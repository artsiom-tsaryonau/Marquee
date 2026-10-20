import type { LibraryApi } from './contracts';
import { desktopApi, isDesktopRuntime } from './desktop';
import { demoApi } from './demo';

export type { LibraryApi } from './contracts';
export { isDesktopRuntime } from './desktop';

export function createApi(): LibraryApi {
  return isDesktopRuntime() ? desktopApi : demoApi;
}
