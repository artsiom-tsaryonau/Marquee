import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { open } from '@tauri-apps/plugin-dialog';
import type {
  AppError,
  ArtworkReport,
  CoreEvent,
  LibraryApi,
  LibraryRoot,
  LibrarySnapshot,
  PortInfo,
  Runner,
  ScanReport,
  Session,
} from './contracts';

function asError(err: unknown): AppError {
  if (err && typeof err === 'object' && 'code' in err && 'message' in err) {
    return err as AppError;
  }
  if (typeof err === 'string') {
    try {
      const parsed = JSON.parse(err) as AppError;
      if (parsed.code) return parsed;
    } catch {
      /* keep string */
    }
    return { code: 'SPAWN_FAILED', message: err };
  }
  return { code: 'SPAWN_FAILED', message: String(err) };
}

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(cmd, args);
  } catch (err) {
    throw asError(err);
  }
}

let listenReady: Promise<void> = Promise.resolve();

export const desktopApi: LibraryApi = {
  kind: 'desktop',
  snapshot: () => call<LibrarySnapshot>('library_snapshot'),
  addRoot: (path) => call<LibraryRoot>('add_library_root', { path }),
  async pickRoot() {
    const selected = await open({ directory: true, multiple: false, title: 'Choose a PS2 ISO folder' });
    return typeof selected === 'string' ? selected : null;
  },
  scan: (rootId) => call<ScanReport>('scan_library_root', { rootId: rootId ?? null }),
  listRunners: () => call<Runner[]>('list_runners'),
  selectRunner: (runnerId) => call<void>('select_runner', { runnerId }),
  openSetup: (runnerId) => call<void>('open_runner_setup', { runnerId }),
  launch: (editionId) => call<Session>('launch_game', { editionId }),
  setFavourite: (editionId, favorite) => call<void>('set_favourite', { editionId, favorite }),
  setDisplayTitle: (editionId, title) => call<void>('set_display_title', { editionId, title }),
  installPcsx2: (channel) => call<Runner[]>('install_pcsx2', { channel: channel ?? null }),
  refreshArtwork: (editionId) => call<ArtworkReport>('refresh_artwork', { editionId: editionId ?? null }),
  listPorts: () => call<PortInfo[]>('list_ports'),
  checkPortUpdates: () => call<PortInfo[]>('check_port_updates'),
  installPort: (portId) => call<PortInfo[]>('install_port', { portId }),
  launchPort: (portId) => call<void>('launch_port', { portId }),
  openPortFolder: (portId) => call<void>('open_port_folder', { portId }),
  subscribe(handler) {
    let stop = () => {};
    listenReady = listen<CoreEvent>('core-event', (event) => handler(event.payload)).then(
      (unlisten) => {
        stop = unlisten;
      },
    );
    return () => stop();
  },
  whenReady() {
    return listenReady;
  },
};

export function isDesktopRuntime(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}
