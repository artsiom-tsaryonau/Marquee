import type {
  AppError,
  ArtworkReport,
  KeyInfo,
  LibraryApi,
  LibraryRoot,
  LibrarySnapshot,
  PortInfo,
  Runner,
  ScanReport,
  Session,
} from './contracts';

export type Call = <T>(cmd: string, args?: Record<string, unknown>) => Promise<T>;

export function asError(err: unknown): AppError {
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

/** The commands are identical on every surface; only the transport (`call`) differs. */
export function commands(call: Call): Omit<LibraryApi, 'kind' | 'pickRoot' | 'subscribe' | 'whenReady' | 'webUrl'> {
  return {
    snapshot: () => call<LibrarySnapshot>('library_snapshot'),
    addRoot: (path) => call<LibraryRoot>('add_library_root', { path }),
    scan: (rootId) => call<ScanReport>('scan_library_root', { root_id: rootId ?? null }),
    listRunners: () => call<Runner[]>('list_runners'),
    selectRunner: (runnerId) => call<void>('select_runner', { runner_id: runnerId }),
    openSetup: (runnerId) => call<void>('open_runner_setup', { runner_id: runnerId }),
    launch: (editionId) => call<Session>('launch_game', { edition_id: editionId }),
    setFavourite: (editionId, favorite) => call<void>('set_favourite', { edition_id: editionId, favorite }),
    setDisplayTitle: (editionId, title) => call<void>('set_display_title', { edition_id: editionId, title }),
    installPcsx2: (channel) => call<Runner[]>('install_pcsx2', { channel: channel ?? null }),
    refreshArtwork: (editionId) => call<ArtworkReport>('refresh_artwork', { edition_id: editionId ?? null }),
    listKeys: () => call<KeyInfo[]>('list_keys'),
    setKey: (id, value) => call<KeyInfo[]>('set_key', { id, value }),
    clearKey: (id) => call<KeyInfo[]>('clear_key', { id }),
    listPorts: () => call<PortInfo[]>('list_ports'),
    checkPortUpdates: () => call<PortInfo[]>('check_port_updates'),
    installPort: (portId) => call<PortInfo[]>('install_port', { port_id: portId }),
    launchPort: (portId) => call<void>('launch_port', { port_id: portId }),
    openPortFolder: (portId) => call<void>('open_port_folder', { port_id: portId }),
  };
}
