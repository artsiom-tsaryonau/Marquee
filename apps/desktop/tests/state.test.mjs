import assert from 'node:assert/strict';
import { after, beforeEach, mock, test } from 'node:test';

mock.timers.enable({ apis: ['setTimeout', 'setInterval'] });
const { app } = await import('../src/lib/state.svelte');
const { installOption, installables } = await import('../src/lib/install');
const { games, ports } = await import('../src/lib/mock');
const initialGames = structuredClone(games);
const initialPorts = structuredClone(ports);

beforeEach(() => {
  app.games = structuredClone(initialGames);
  app.ports = structuredClone(initialPorts);
  app.downloads = [];
  app.history = [];
  app.route = 'home';
  app.gameId = null;
  app.heroId = app.games[0].id;
  app.launching = null;
  app.running = null;
  app.installToolId = null;
  app.installOptionId = null;
});
after(() => mock.timers.reset());

function finishNextDownload() {
  app.downloads[0].progress = 0.999;
  mock.timers.tick(400);
  assert.equal(app.downloads[0].state, 'done');
}

test('completed game download changes Install to Play for the correct game ID', () => {
  const game = app.games.find((g) => !g.installed);
  const other = app.games.find((g) => g.id !== game.id);
  other.title = game.title;
  other.installed = false;
  app.install(game.title, { kind: 'game', id: game.id });
  finishNextDownload();
  assert.equal(game.installed, true);
  assert.equal(other.installed, false);
});

test('new port install adds one playable library entry with no invented playtime', () => {
  const port = app.ports.find((p) => p.id === 'opengoal');
  app.install(`${port.name} ${port.version}`, { kind: 'port', id: port.id });
  finishNextDownload();
  const game = app.games.find((g) => g.id === `port:${port.id}`);
  assert.equal(port.status, 'ready');
  assert.equal(game.installed, true);
  assert.equal(game.portStatus, 'ready');
  assert.equal(game.playtimeMin, 0);
  assert.equal(game.lastPlayed, null);
  app.play(game);
  mock.timers.tick(2200);
  assert.equal(app.running?.id, game.id);
});

test('port update synchronizes catalogue and library while preserving user data', () => {
  const game = app.games.find((g) => g.id === 'port:zelda64recomp');
  game.favorite = true;
  const played = game.playtimeMin;
  app.install('Renamed display label', { kind: 'port', id: 'zelda64recomp' });
  finishNextDownload();
  assert.equal(app.ports.find((p) => p.id === 'zelda64recomp').status, 'ready');
  assert.equal(game.portStatus, 'ready');
  assert.equal(game.favorite, true);
  assert.equal(game.playtimeMin, played);
  assert.equal(app.games.filter((g) => g.id === game.id).length, 1);
});

test('active installs are deduplicated by identity across different labels', () => {
  app.install('OpenGOAL', { kind: 'port', id: 'opengoal' });
  app.downloads[0].state = 'paused';
  app.install('OpenGOAL update', { kind: 'port', id: 'opengoal' });
  assert.equal(app.downloads.length, 1);
});

test('tool download labels cannot mark similarly named ports ready', () => {
  app.install('OpenGOAL unrelated helper');
  finishNextDownload();
  assert.equal(app.ports.find((p) => p.id === 'opengoal').status, 'not-installed');
  assert.equal(app.games.find((g) => g.id === 'port:opengoal').installed, false);
  assert.equal(app.games.find((g) => g.id === 'port:opengoal').portStatus, 'not-installed');
});

test('back restores each game and its artwork before returning to Library', () => {
  const [a, b] = app.games;
  app.go('library');
  app.open(a);
  app.open(b);
  app.back();
  assert.equal(app.route, 'game');
  assert.equal(app.gameId, a.id);
  assert.equal(app.heroId, a.id);
  app.back();
  assert.equal(app.route, 'library');
  assert.equal(app.gameId, null);
});

test('reopening the same game does not add a duplicate history entry', () => {
  app.open(app.games[0]);
  app.open(app.games[0]);
  app.back();
  assert.equal(app.route, 'home');
  assert.equal(app.history.length, 0);
});

test('back cancels launching without changing the underlying route', () => {
  app.go('library');
  app.play(app.games[0]);
  app.back();
  mock.timers.tick(2200);
  assert.equal(app.launching, null);
  assert.equal(app.running, null);
  assert.equal(app.route, 'library');
});

test('install dropdown options are executable actions', () => {
  const pcsx2 = installables.find((tool) => tool.id === 'pcsx2');
  assert.ok(pcsx2.options.length >= 3);
  assert.equal(installOption('pcsx2', 'unpacked').run.channel, 'unpacked');
  assert.equal(installOption('pcsx2', 'flatpak').run.channel, 'flatpak');
  assert.equal(installOption('pcsx2', 'appimage').run.channel, 'appimage');
  assert.equal(installOption('pcsx2', 'setup').run.type, 'setup');
});

test('back closes the install dialog before leaving the route', () => {
  app.go('settings');
  app.openInstall('pcsx2', 'auto');
  assert.equal(app.installToolId, 'pcsx2');
  assert.equal(app.installOptionId, 'auto');
  app.back();
  assert.equal(app.installToolId, null);
  assert.equal(app.route, 'settings');
});

const { groupGames, matchesVariant, enrichIdentity, nativePortGame, canonicalId } = await import('../src/lib/unified');
test('explicit game identity groups PS2 and native options without merging equal titles', () => {
  const jak = initialGames.find((g) => g.id === 'demo:jak-ps2');
  const native = nativePortGame(initialPorts.find((p) => p.id === 'opengoal'));
  const other = { ...jak, id: 'another-disc', canonicalId: undefined, editionKey: undefined };
  const grouped = groupGames([jak, native, other]);
  assert.equal(grouped.length, 2);
  assert.equal(grouped[0].entries.length, 2);
  assert.equal(grouped[1].game.id, 'another-disc');
});
test('platform and method filters must match the same installation', () => {
  const ps2 = initialGames.find((g) => g.id === 'demo:jak-ps2');
  const native = nativePortGame(initialPorts.find((p) => p.id === 'opengoal'));
  assert.equal(matchesVariant(ps2, 'ps2', 'emulation'), true);
  assert.equal(matchesVariant(native, 'pc-linux', 'native'), true);
  assert.equal([ps2, native].some((g) => matchesVariant(g, 'ps2', 'native')), false);
});
test('Jak serial enrichment keeps installation identity, custom title, and playtime', () => {
  const game = { ...initialGames[0], id: 'real-disc', serial: 'SCUS-97124', title: 'My Jak copy', playtimeMin: 91 };
  const enriched = enrichIdentity(game);
  assert.equal(enriched.id, 'real-disc'); assert.equal(enriched.title, 'My Jak copy');
  assert.equal(enriched.playtimeMin, 91); assert.equal(canonicalId(enriched), 'jak-and-daxter-tpl');
  assert.equal(enriched.screenshots.length, 3);
});
test('an anniversary edition remains a separate edition inside the same game', () => {
  const original = initialGames.find((g) => g.id === 'demo:jak-ps2');
  // Synthetic model fixture, not an advertised Jak edition.
  const anniversary = { ...original, id: 'edition-fixture', editionKey: 'anniversary-fixture', editionLabel: 'Anniversary' };
  const [group] = groupGames([original, anniversary]);
  assert.equal(group.entries.length, 2);
  assert.notEqual(group.entries[0].editionKey, group.entries[1].editionKey);
});

test('library invalidation refreshes both disc entries and installed native ports', async () => {
  const oldApi = app.api; const oldDemo = app.demo;
  const snap = { roots: [], editions: [], media: [], runners: [], sessions: [], settings: { selected_runner_id: null } };
  let snapshots = 0; let portLists = 0;
  app.demo = false;
  app.api = { ...oldApi, snapshot: async () => { snapshots++; return snap; }, listPorts: async () => {
    portLists++; return [{ id: 'shipwright', name: 'Ship of Harkinian', game: 'Ocarina of Time', family: 'Native', platform: 'n64', upstream: 'HarbourMasters/Shipwright', installed_version: 'test', latest_version: null, status: 'ready', requirement: 'Supply your data', folder: '/test' }];
  }};
  try {
    app.onEvent({ type: 'library_changed' });
    await Promise.resolve(); await Promise.resolve();
    assert.equal(snapshots, 1); assert.equal(portLists, 1);
    assert.equal(app.games.length, 1); assert.equal(app.games[0].id, 'port:shipwright');
    app.applySnapshot(snap);
    assert.equal(app.games[0].id, 'port:shipwright');
  } finally { app.api = oldApi; app.demo = oldDemo; }
});

test('remembered variant stays inside the filtered choices and changes no history', () => {
  app.variantPreferences = {};
  const ps2 = app.games.find((g) => g.id === 'demo:jak-ps2');
  const native = app.games.find((g) => g.id === 'port:opengoal');
  app.open(ps2);
  const history = app.history.length;
  app.chooseVariant(native.id);
  assert.equal(app.current.id, native.id); assert.equal(app.heroId, native.id);
  assert.equal(app.history.length, history);
  assert.equal(groupGames([ps2, native], app.variantPreferences)[0].game.id, native.id);
  assert.equal(groupGames([ps2], app.variantPreferences)[0].game.id, ps2.id);
  app.chooseVariant(app.games.find((g) => g.id !== ps2.id && g.id !== native.id).id);
  assert.equal(app.current.id, native.id);
});

test('editing a launcher draft does not mutate saved arguments', async () => {
  const { launcherDraft } = await import('../src/lib/launchers');
  const original = { id: 'test', name: 'A', executable: '/bin/true', args: ['one'], working_directory: null, pinned: false };
  const draft = launcherDraft(original); draft.args[0] = 'changed'; draft.name = 'B';
  assert.deepEqual(original.args, ['one']); assert.equal(original.name, 'A');
});

test('saving a launcher never opens it and stale refresh cannot replace saved config', async () => {
  const api = app.api; let finish; let opens = 0;
  const saved = { id: 'custom.test', name: 'Test', executable: '/bin/true', args: [], working_directory: null, pinned: true, available: true, configured: true, status: 'Ready' };
  app.api = { ...api, listLaunchers: () => new Promise((resolve) => { finish = resolve; }), saveLauncher: async () => [saved], openLauncher: async () => { opens++; } };
  try {
    const loading = app.loadLaunchers();
    const { available, configured, status, ...config } = saved;
    await app.saveLauncher(config);
    finish([]); await loading;
    assert.equal(app.launchers[0].id, saved.id); assert.equal(opens, 0);
    await app.openLauncher(saved.id); assert.equal(opens, 1);
  } finally { app.api = api; }
});

test('launcher changes refresh through the shared event surface', async () => {
  const api = app.api; let calls = 0;
  app.api = { ...api, listLaunchers: async () => { calls++; return []; } };
  try { app.onEvent({ type: 'launchers_changed' }); await Promise.resolve(); assert.equal(calls, 1); }
  finally { app.api = api; }
});

test('back closes launcher editor before leaving Tools', () => {
  app.route = 'tools'; let closed = false;
  app.closeLauncherEditor = () => { closed = true; app.closeLauncherEditor = null; };
  app.back(); assert.equal(closed, true); assert.equal(app.route, 'tools');
});

const { lutrisToGame } = await import('../src/lib/lutris');
const lutrisFixture = { id: 'lutris:test:owned:egs', name: 'Cached game', service: 'egs', store_id: 'store-id', lutris_id: null, runner: null, platform: 'unknown', installed: false, available: false, playtime_minutes: 0 };
test('cached store library retains store identity and honest installation state', () => {
  const game = lutrisToGame(lutrisFixture);
  assert.equal(game.source, 'epic'); assert.equal(game.installed, false);
  assert.equal(game.platform, 'unknown'); assert.equal(game.lutris.service, 'egs');
  assert.equal(game.lutris.storeId, 'store-id'); assert.equal(game.playtimeMin, 0);
});
test('Lutris handoff does not use PS2 launcher or fabricate a running session', async () => {
  const api = app.api; const demo = app.demo; let lutrisCalls = 0; let discCalls = 0;
  app.demo = false;
  app.api = { ...api, openLutrisGame: async (id) => { assert.equal(id, lutrisFixture.id); lutrisCalls++; }, launch: async () => { discCalls++; } };
  try { app.play(lutrisToGame(lutrisFixture)); await Promise.resolve(); assert.equal(lutrisCalls, 1); assert.equal(discCalls, 0); assert.equal(app.running, null); assert.equal(app.launching, null); }
  finally { app.api = api; app.demo = demo; }
});
test('automatic source discovery adds store games and disconnect preserves other libraries', async () => {
  const api = app.api; const demo = app.demo;
  const config = { enabled: true, include_owned: true, database_path: null, launcher_id: 'detected.lutris' };
  const snapshot = { config, games: [lutrisFixture], detected_paths: ['/test/pga.db'], database_path: '/test/pga.db', cached_by_service: { egs: 1 }, configured_count: 0, installed_count: 0, warnings: [] };
  const before = app.games.map((g) => g.id);
  app.demo = false;
  app.api = { ...api, lutrisLibrary: async () => snapshot, configureLutris: async (cfg) => ({ ...snapshot, config: cfg, games: [] }) };
  try {
    await app.loadLutris(); assert.equal(app.games.filter((g) => g.lutris).length, 1);
    assert.deepEqual(app.games.filter((g) => !g.lutris).map((g) => g.id), before);
    app.open(app.games.find((g) => g.lutris));
    await app.configureLutris({ ...config, enabled: false });
    assert.equal(app.games.some((g) => g.lutris), false); assert.equal(app.route, 'library');
    assert.deepEqual(app.games.map((g) => g.id), before);
  } finally { app.api = api; app.demo = demo; }
});
