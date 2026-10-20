import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { open } from '@tauri-apps/plugin-dialog';
import type { CoreEvent, LibraryApi } from './contracts';
import { asError, commands, type Call } from './core';

const call: Call = async <T>(cmd: string, args?: Record<string, unknown>) => {
  try {
    return await invoke<T>('core_call', { cmd, args: args ?? null });
  } catch (err) {
    throw asError(err);
  }
};

let listenReady: Promise<void> = Promise.resolve();

export const desktopApi: LibraryApi = {
  kind: 'desktop',
  ...commands(call),
  async pickRoot() {
    const selected = await open({ directory: true, multiple: false, title: 'Choose a PS2 ISO folder' });
    return typeof selected === 'string' ? selected : null;
  },
  async pickExecutable() {
    const selected = await open({ directory: false, multiple: false, title: 'Choose a launcher executable or AppImage' });
    return typeof selected === 'string' ? selected : null;
  },
  webUrl: () => invoke<string | null>('web_url'),
  subscribe(handler) {
    let stop = () => {};
    listenReady = listen<CoreEvent>('core-event', (event) => handler(event.payload)).then((unlisten) => {
      stop = unlisten;
    });
    return () => stop();
  },
  whenReady() {
    return listenReady;
  },
};

export function isDesktopRuntime(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}
