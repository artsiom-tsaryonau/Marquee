import { launcherTemplates } from '../launchers';
import type { LauncherConfig, LauncherInfo, LutrisConfig } from './contracts';
import { games } from '../mock';
import type { EmulatorInfo, KeyInfo, LibraryApi, LibrarySnapshot, ScanReport } from './contracts';

function emptySnapshot(): LibrarySnapshot {
  return {
    roots: [],
    editions: [],
    media: [],
    runners: [],
    sessions: [],
    settings: {
      last_root: null,
      last_collection: null,
      last_focus: null,
      selected_runner_id: null,
    },
  };
}

const demoCatalogue: [string, string, string[], boolean, string][] = [
  ['duckstation', 'DuckStation', ['ps1'], true, 'Needs your own PS1 BIOS (Marquee can import a folder of them)'],
  ['flycast', 'Flycast', ['dreamcast'], false, 'Needs your own Dreamcast BIOS in its data folder'],
  ['cemu', 'Cemu', ['wiiu'], false, 'Needs your own Wii U keys and game files'],
  ['rpcs3', 'RPCS3', ['ps3'], false, "Installs the PS3 firmware from Sony's own file, which you download yourself"],
  ['ppsspp', 'PPSSPP', ['psp'], false, 'No BIOS needed. Bring your own game files'],
  ['xemu', 'xemu', ['xbox'], false, 'Needs your own Xbox BIOS, MCPX boot ROM and hard-disk image'],
  ['azahar', 'Azahar', ['3ds'], false, 'Bring your own decrypted 3DS game files'],
];
function demoEmulators(): EmulatorInfo[] {
  return demoCatalogue.map(([id, name, systems, integrated, requirement]) => ({
    id, name, systems, integrated, upstream: id, requirement,
    installed_version: null, latest_version: null, status: 'not_installed' as const,
    program_dir: null, data_dir: `~/.local/share/marquee/${id}`, bios_dir: null, icon: null, args: [],
    alternatives: id === 'duckstation' ? [{ manager: 'flatpak', command: 'flatpak install --user flathub org.duckstation.DuckStation', note: 'Marquee also finds a Flatpak copy, but keeps its own settings and saves separate from it.' }] : [],
  }));
}

/** Browser demo: sample library stays in the Svelte state layer. */
const demoDefs: Omit<KeyInfo, 'source' | 'set' | 'preview'>[] = [
  { id: 'steamgriddb', env: 'STEAM_GRID_DB_API_KEY', label: 'SteamGridDB API key', description: 'Covers, heroes and logos for Steam, Lutris and port games.', help: 'Free: steamgriddb.com, sign in, Preferences, API.', used_by: 'Artwork', secret: true },
  { id: 'steam_web_api', env: 'STEAM_WEB_API_KEY', label: 'Steam Web API key', description: 'Lists games you own but have not installed, with playtime. Your own key; never share it.', help: 'Free: steamcommunity.com/dev/apikey (personal use).', used_by: 'Steam library', secret: true },
  { id: 'steam_id', env: 'STEAM_ID64', label: 'Your SteamID64', description: 'The 17-digit number that identifies your Steam account. Not a secret.', help: 'From your profile address, or a lookup such as steamid.io.', used_by: 'Steam library, wishlist', secret: false },
];
const demoSaved: Record<string, string> = {};
function demoKeys(): KeyInfo[] {
  return demoDefs.map((d) => {
    const v = demoSaved[d.id];
    const preview = !v ? null : d.secret ? `••••${v.slice(-4)}` : v;
    return { ...d, source: v ? 'app' : 'none', set: !!v, preview };
  });
}

let demoLaunchers: LauncherInfo[] = launcherTemplates.slice(0, 3).map((l) => ({ ...l, id: `detected.${l.executable}`, pinned: false, available: false, configured: false, status: 'Demo preview · connect in the desktop app' }));
export const demoApi: LibraryApi = {
  kind: 'demo',
  async snapshot() {
    const _ = games.length;
    return emptySnapshot();
  },
  async addRoot() {
    throw { code: 'DEMO', message: 'Adding a library folder needs the desktop app' };
  },
  async pickRoot() {
    return null;
  },
  async scan(): Promise<ScanReport> {
    throw { code: 'DEMO', message: 'Scanning needs the desktop app' };
  },
  async listRunners() {
    return [];
  },
  async selectRunner() {},
  async openSetup() {},
  async launch() {
    throw { code: 'DEMO', message: 'Demo launches are simulated in the UI' };
  },
  async setFavourite() {},
  async setDisplayTitle() {},
  subscribe() {
    return () => {};
  },
  async whenReady() {},
  async installPcsx2() {
    throw { code: 'DEMO', message: 'Install PCSX2 in the desktop app' };
  },
  // Same list as the real app, kept in memory, so the Settings screen looks identical in the demo.
  async listKeys() {
    return demoKeys();
  },
  async setKey(id: string, value: string) {
    if (value.trim()) demoSaved[id] = value.trim();
    else delete demoSaved[id];
    return demoKeys();
  },
  async clearKey(id: string) {
    delete demoSaved[id];
    return demoKeys();
  },
  async webUrl() {
    return null;
  },
  async lutrisLibrary() { return { config: { enabled: false, include_owned: false, database_path: null, launcher_id: 'detected.lutris' }, detected_paths: [], database_path: null, games: [], cached_by_service: {}, configured_count: 0, installed_count: 0, warnings: [] }; },
  async configureLutris(_config: LutrisConfig) { throw { code: 'DEMO', message: 'Connect your Lutris library in the desktop app' }; },
  async openLutrisGame() { throw { code: 'DEMO', message: 'Opening Lutris games requires the desktop app' }; },
  async purpleStatus() { return { installed: false, ready: false, busy: false, running: false, message: 'Install Purple in the desktop app.', error: null, prefix: '', log_path: '', runtime: null, aion2_verified: false }; },
  async installPurple() { throw { code: 'DEMO', message: 'Purple setup requires the desktop app.' }; },
  async cancelPurple() { throw { code: 'DEMO', message: 'Purple setup requires the desktop app.' }; },
  async openPurple() { throw { code: 'DEMO', message: 'Opening Purple requires the desktop app.' }; },
  async listLaunchers() { return structuredClone(demoLaunchers); },
  async saveLauncher(launcher: LauncherConfig) {
    const info: LauncherInfo = { ...structuredClone(launcher), available: false, configured: true, status: 'Demo configuration · opening requires the desktop app' };
    demoLaunchers = [...demoLaunchers.filter((l) => l.id !== info.id), info].sort((a,b) => Number(b.pinned)-Number(a.pinned) || a.name.localeCompare(b.name));
    return structuredClone(demoLaunchers);
  },
  async removeLauncher(id: string) {
    demoLaunchers = demoLaunchers.filter((l) => l.id !== id);
    const template = launcherTemplates.slice(0, 3).find((l) => `detected.${l.executable}` === id);
    if (template) demoLaunchers.push({ ...template, id, pinned: false, available: false, configured: false, status: 'Demo preview · connect in the desktop app' });
    return structuredClone(demoLaunchers);
  },
  async openLauncher() { throw { code: 'DEMO', message: 'Opening applications requires the desktop app' }; },
  async pickExecutable() { return null; },
  async listPorts() {
    return [];
  },
  async checkPortUpdates() {
    return [];
  },
  async installPort() {
    throw { code: 'DEMO', message: 'Install ports in the desktop app' };
  },
  async launchPort() {
    throw { code: 'DEMO', message: 'Launch ports in the desktop app' };
  },
  async openPortFolder() {
    throw { code: 'DEMO', message: 'Open port folders in the desktop app' };
  },
  // The same catalogue the app ships, shown as not installed so the Tools screen looks real.
  async listEmulators() {
    return demoEmulators();
  },
  async checkEmulatorUpdates() {
    return demoEmulators();
  },
  async installEmulator() {
    throw { code: 'DEMO', message: 'Install emulators in the desktop app' };
  },
  async uninstallEmulator() {
    throw { code: 'DEMO', message: 'Manage emulators in the desktop app' };
  },
  async configureEmulator() {
    throw { code: 'DEMO', message: 'Configure emulators in the desktop app' };
  },
  async openEmulator() {
    throw { code: 'DEMO', message: 'Open emulators in the desktop app' };
  },
  async listBios() {
    return [];
  },
  async importBios() {
    throw { code: 'DEMO', message: 'Import BIOS files in the desktop app' };
  },
  async removeBios() {
    throw { code: 'DEMO', message: 'Manage BIOS files in the desktop app' };
  },
  async refreshArtwork() {
    return { fetched: 0, skipped: 0, failed: 0 };
  },
};
