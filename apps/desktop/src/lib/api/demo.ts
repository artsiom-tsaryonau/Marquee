import { games } from '../mock';
import type { LibraryApi, LibrarySnapshot, ScanReport } from './contracts';

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
};
