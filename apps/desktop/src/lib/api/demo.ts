import { games } from '../mock';
import type { KeyInfo, LibraryApi, LibrarySnapshot, ScanReport } from './contracts';

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
  async refreshArtwork() {
    return { fetched: 0, skipped: 0, failed: 0 };
  },
};
