import assert from 'node:assert/strict';
import { after, beforeEach, mock, test } from 'node:test';

mock.timers.enable({ apis: ['setTimeout', 'setInterval'] });
const { app } = await import('../src/lib/state.svelte');
const { installOption, installables } = await import('../src/lib/install');
const { artFromSeed } = await import('../src/lib/art');
const fixtureGame = (id, over = {}) => ({
  id, title: id, tagline: '', platform: 'ps2', source: 'rom', installed: true, genre: '', year: 0, playtimeMin: 0,
  lastPlayed: null, addedAt: '', favorite: false, ratings: { critic: null, criticCount: 0, user: null, steamReviews: null },
  timeToBeat: null, achievements: null, art: artFromSeed(id), launchMethod: 'emulation', ...over,
});
const fixturePort = (id, over = {}) => ({ id, name: id, game: id, family: 'Native', platform: 'pc-linux', upstream: 'upstream', version: '1', status: 'ready', requirement: '', art: artFromSeed(id), ...over });
const initialGames = [fixtureGame('disc-a'), fixtureGame('disc-b'), fixtureGame('disc-c')];
const initialPorts = [fixturePort('opengoal')];

beforeEach(() => {
  app.games = structuredClone(initialGames);
  app.ports = structuredClone(initialPorts);
  app.portDialogId = null;
  app.portErrors = {};
  app.portBusy = {};
  app.intakeOpen=false; app.intakeBusy=false; app.intakePaths=[]; app.intakeResults=[]; app.intakeDelete=false; app.intakeScanError=null;
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

const { editionToGame, pickRunner, gamesFromSnapshot } = await import('../src/lib/api/map');
const { metadataMatches, versions } = await import('../src/lib/metadata');
function catalogueFixture() {
  return { id:'vimm:123',game_id:'123',system:'ps2',title:'Source title',regions:'Europe | USA',languages:'en fr',version:'1.0',rating:'10',url:'https://vimm.net/vault/123',has_detail:true,
    media:[{media_id:'456',filename:'game.iso',version:'1.0',regions:['Europe'],languages:['en','fr'],size:'1 MB',serial:'',crc32:'',md5:'',sha1:''}],
    players:1,year:null,ratings:{graphics:10,sound:10,gameplay:10,overall:10},rating_votes:2,verified_at:'2026-10-18',
    cover:{data_url:'data:image/png;base64,test',credit:'source',source_url:'https://github.com/libretro-thumbnails/'},exported_at:'export date',imported_at:'import date' };
}
test('catalogue discoveries cannot launch and have attributed metadata rather than invented critic scores', () => {
  const snap=discSnapshot(); snap.catalogue={records:[catalogueFixture()],systems:[],links:[]};
  const game=gamesFromSnapshot(snap).find(g=>g.source==='catalogue');
  assert.equal(game.installed,false); assert.equal(game.setupState,undefined); assert.equal(game.ratings.critic,null);
  assert.equal(game.metadata.rating_votes,2); assert.equal(game.players,1); assert.equal(game.year,0); assert.ok(game.coverUrl.startsWith('data:'));
  const api=app.api; let launches=0;
  try { app.api={...api,launch:()=>{launches++;}}; app.play(game); assert.equal(launches,0); assert.equal(app.launching,null); }
  finally {app.api=api;}
});
test('metadata links enrich only the chosen installation and preserve identity, title, history and launch setup', () => {
  const snap=discSnapshot(); snap.editions[1].playtime_seconds=120; snap.editions[1].favorite=true;
  snap.catalogue={records:[catalogueFixture()],systems:[],links:[{edition_id:'e2',record_id:'vimm:123',media_id:'456',method:'manual',valid:true}]};
  const games=gamesFromSnapshot(snap), local=games.find(g=>g.id==='e2');
  assert.equal(local.title,'Vice City'); assert.equal(local.source,'rom'); assert.equal(local.playtimeMin,2); assert.equal(local.favorite,true);
  assert.equal(local.runnerId,'pcsx2'); assert.equal(local.revision,'1.0'); assert.equal(local.region,'Europe');
  assert.equal(games.find(g=>g.id==='e1').metadata,undefined);
  snap.catalogue.links[0].valid=false;
  assert.equal(gamesFromSnapshot(snap).find(g=>g.id==='e2').metadata,undefined);
});
test('region language revision and player filters match one concrete entry with unknowns excluded', () => {
  const snap=discSnapshot(); snap.catalogue={records:[catalogueFixture()],systems:[],links:[]};
  const game=gamesFromSnapshot(snap).find(g=>g.source==='catalogue');
  assert.ok(metadataMatches(game,'Europe','fr','1.0','single'));
  assert.equal(metadataMatches(game,'Japan','fr','1.0','single'),false);
  assert.equal(metadataMatches(game,'Europe','de','1.0','single'),false);
  assert.equal(metadataMatches(game,'Europe','fr','2.0','single'),false);
  assert.equal(metadataMatches(game,'Europe','fr','1.0','multi'),false);
  assert.equal(metadataMatches(fixtureGame('unknown'),'Europe','','',''),false);
  assert.deepEqual(versions(game),['1.0']);
  game.metadata.media.push({media_id:'789',version:'2.0',regions:['USA'],languages:['en']});
  assert.equal(metadataMatches(game,'Europe','fr','2.0','single'),false);
  assert.ok(metadataMatches(game,'USA','en','2.0','single'));
});
test('linked variants without locale evidence keep local identity instead of catalogue-wide labels', () => {
  const snap=discSnapshot(); snap.editions[1].region='USA';
  const record=catalogueFixture(); record.media[0].regions=[]; record.media[0].languages=[];
  snap.catalogue={records:[record],systems:[],links:[{edition_id:'e2',record_id:record.id,media_id:'456',method:'manual',valid:true}]};
  const local=gamesFromSnapshot(snap).find(g=>g.id==='e2');
  assert.equal(local.region,'USA'); assert.equal(local.languages,undefined);
  assert.equal(metadataMatches(local,'Europe','','',''),false);
  const discovery=gamesFromSnapshot(snap).find(g=>g.source==='catalogue');
  assert.equal(discovery.region,'Europe | USA');
});
test('canonical pce catalogue identity displays on the TurboGrafx platform', () => {
  const snap=discSnapshot(); const record=catalogueFixture(); record.system='pce';
  snap.catalogue={records:[record],systems:[],links:[]};
  assert.equal(gamesFromSnapshot(snap).find(g=>g.source==='catalogue').platform,'tg16');
});
function discSnapshot() {
  return { roots: [], media: [{ id: 'm1', path: '/roms/ps1/game.cue', available: true, skip_reason: null }, { id: 'm2', path: '/roms/ps2/game.iso', available: true, skip_reason: null }], sessions: [],
    editions: [{ id: 'e1', system: 'ps1', title: 'Metal Gear Solid', media_id: 'm1', missing: false, cover_path: null, playtime_seconds: 0 }, { id: 'e2', system: 'ps2', title: 'Vice City', media_id: 'm2', missing: false, cover_path: null, playtime_seconds: 0 }],
    runners: [{ id: 'pcsx2', kind: 'pcsx2', display_name: 'PCSX2', available: true, bios_state: 'unverified' }], settings: { selected_runner_id: 'pcsx2' } };
}
test('mixed disc library keeps PS1 categories and setup separate from PS2', () => {
  const [ps1, ps2] = gamesFromSnapshot(discSnapshot());
  assert.equal(ps1.platform, 'ps1'); assert.equal(ps1.genre, ''); assert.equal(ps1.setupState, 'runner_missing'); assert.equal(ps1.runnerId, undefined);
  assert.equal(ps2.platform, 'ps2'); assert.equal(ps2.runnerId, 'pcsx2');
});
test('DS and Neo Geo entries use their own platforms and runner readiness', () => {
  const snap=discSnapshot();
  snap.editions[0].system='ds'; snap.editions[1].system='neogeo';
  snap.editions[1].missing=true; snap.media[1].skip_reason='Legacy split set needs parent ROMs';
  snap.runners=[{id:'runner.melonds.marquee',kind:'emulator',display_name:'melonDS',available:true,bios_state:'unverified'},
    {id:'runner.fbneo.marquee',kind:'emulator',display_name:'FinalBurn Neo',available:true,bios_state:'unverified'}];
  const [ds,neo]=gamesFromSnapshot(snap);
  assert.equal(ds.platform,'ds'); assert.equal(ds.runnerName,'melonDS'); assert.equal(ds.setupState,'unverified');
  assert.equal(neo.platform,'neogeo'); assert.equal(neo.runnerName,'FinalBurn Neo'); assert.equal(neo.installed,false);
  assert.equal(neo.mediaIssue,'Legacy split set needs parent ROMs');
});
test('runner defaults are platform scoped and unavailable explicit pins stay visible', () => {
  const snap = discSnapshot(); snap.runners.push({ id: 'duck', kind: 'duckstation', display_name: 'DuckStation', available: true, bios_state: 'unverified' });
  snap.settings.selected_runner_ids = { ps1: 'duck', ps2: 'pcsx2' }; snap.settings.selected_runner_id = 'duck';
  assert.equal(pickRunner(snap, 'ps2').id, 'pcsx2'); assert.equal(pickRunner(snap, 'ps1').id, 'duck');
  snap.runners.push({ id: 'dead', kind: 'pcsx2', available: false }); snap.settings.selected_runner_ids.ps2 = 'dead';
  assert.equal(pickRunner(snap, 'ps2').id, 'dead');
  assert.equal(pickRunner(snap, 'ps2').available, false);
});
test('missing companion diagnostics reach cards without unrelated offline folders', () => {
  const snap = discSnapshot(); snap.media[0].available = false; snap.media[0].skip_reason = 'Missing CUE track: audio.bin'; snap.editions[0].missing = true;
  snap.roots = [{ system: 'ps2', path: '/other', status: 'offline' }];
  const game = editionToGame(snap.editions[0], snap);
  assert.equal(game.installed, false); assert.equal(game.mediaIssue, 'Missing CUE track: audio.bin'); assert.equal(game.rootOffline, false);
});

test('unrelated tool events never change PCSX2 installation state', () => {
  app.installingPcsx2 = false;
  app.installProgress = 'PCSX2 progress';
  app.onEvent({ type: 'tool_install_started', tool_id: 'purple', channel: 'stable' });
  app.onEvent({ type: 'tool_install_progress', tool_id: 'purple', message: 'Setting up Purple' });
  app.onEvent({ type: 'tool_install_finished', tool_id: 'purple', runner_id: null });
  app.onEvent({ type: 'tool_install_failed', tool_id: 'purple', message: 'Purple failed' });
  assert.equal(app.installingPcsx2, false);
  assert.equal(app.installProgress, 'PCSX2 progress');
});

test('Purple uses the shared command transport', async () => {
  const { commands } = await import('../src/lib/api/core');
  const calls = [];
  const api = commands(async (cmd, args) => { calls.push([cmd, args]); return {}; });
  await api.purpleStatus(); await api.installPurple(); await api.cancelPurple(); await api.openPurple();
  assert.deepEqual(calls.map(([cmd]) => cmd), ['purple_status', 'install_purple', 'cancel_purple', 'open_purple']);
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
  app.launching = app.games[0];
  app.back();
  assert.equal(app.launching, null);
  assert.equal(app.running, null);
  assert.equal(app.route, 'library');
});

test('install dropdown options are executable actions', () => {
  const pcsx2 = installables.find((tool) => tool.id === 'pcsx2');
  assert.equal(pcsx2.options.length, 1);
  assert.equal(installOption('pcsx2', 'auto').run.channel, 'auto');
  assert.equal(installOption('pcsx2', 'flatpak'), undefined);
  assert.equal(installOption('pcsx2', 'appimage'), undefined);
  assert.equal(installOption('pcsx2', 'setup'), undefined);
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
const jakFixture = () => enrichIdentity(fixtureGame('jak-ps2', { serial: 'SCUS-97124', title: 'Jak and Daxter: The Precursor Legacy' }));
test('explicit game identity groups PS2 and native options without merging equal titles', () => {
  const jak = jakFixture();
  const native = nativePortGame(initialPorts.find((p) => p.id === 'opengoal'));
  const other = { ...jak, id: 'another-disc', canonicalId: undefined, editionKey: undefined };
  const grouped = groupGames([jak, native, other]);
  assert.equal(grouped.length, 2);
  assert.equal(grouped[0].entries.length, 2);
  assert.equal(grouped[1].game.id, 'another-disc');
});
test('platform and method filters must match the same installation', () => {
  const ps2 = jakFixture();
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
  const original = jakFixture();
  // Synthetic model fixture, not an advertised Jak edition.
  const anniversary = { ...original, id: 'edition-fixture', editionKey: 'anniversary-fixture', editionLabel: 'Anniversary' };
  const [group] = groupGames([original, anniversary]);
  assert.equal(group.entries.length, 2);
  assert.notEqual(group.entries[0].editionKey, group.entries[1].editionKey);
});

test('library invalidation refreshes both disc entries and installed native ports', async () => {
  const oldApi = app.api;
  const snap = { roots: [], editions: [], media: [], runners: [], sessions: [], settings: { selected_runner_id: null } };
  let snapshots = 0; let portLists = 0;
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
  } finally { app.api = oldApi }
});

test('remembered variant stays inside the filtered choices and changes no history', () => {
  app.variantPreferences = {};
  const ps2 = jakFixture();
  const native = nativePortGame(initialPorts[0]);
  const unrelated = fixtureGame('unrelated');
  app.games = [ps2, native, unrelated];
  app.open(ps2);
  const history = app.history.length;
  app.chooseVariant(native.id);
  assert.equal(app.current.id, native.id); assert.equal(app.heroId, native.id);
  assert.equal(app.history.length, history);
  assert.equal(groupGames([ps2, native], app.variantPreferences)[0].game.id, native.id);
  assert.equal(groupGames([ps2], app.variantPreferences)[0].game.id, ps2.id);
  app.chooseVariant(unrelated.id);
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
  const api = app.api; let lutrisCalls = 0; let discCalls = 0;
  app.api = { ...api, openLutrisGame: async (id) => { assert.equal(id, lutrisFixture.id); lutrisCalls++; }, launch: async () => { discCalls++; } };
  try { app.play(lutrisToGame(lutrisFixture)); await Promise.resolve(); assert.equal(lutrisCalls, 1); assert.equal(discCalls, 0); assert.equal(app.running, null); assert.equal(app.launching, null); }
  finally { app.api = api; }
});
test('explicit legacy connection adds store games and disconnect preserves other libraries', async () => {
  const api = app.api;
  const config = { enabled: true, include_owned: true, database_path: null, launcher_id: 'detected.lutris' };
  const snapshot = { config, games: [lutrisFixture], detected_paths: ['/test/pga.db'], database_path: '/test/pga.db', cached_by_service: { egs: 1 }, configured_count: 0, installed_count: 0, warnings: [] };
  const before = app.games.map((g) => g.id);
  app.api = { ...api, lutrisLibrary: async () => snapshot, configureLutris: async (cfg) => ({ ...snapshot, config: cfg, games: [] }) };
  try {
    await app.loadLutris(); assert.equal(app.games.filter((g) => g.lutris).length, 1);
    assert.deepEqual(app.games.filter((g) => !g.lutris).map((g) => g.id), before);
    app.open(app.games.find((g) => g.lutris));
    await app.configureLutris({ ...config, enabled: false });
    assert.equal(app.games.some((g) => g.lutris), false); assert.equal(app.route, 'library');
    assert.deepEqual(app.games.map((g) => g.id), before);
  } finally { app.api = api; }
});

test('emulator install shows progress, clears it afterwards, and removal asks for saves explicitly', async () => {
  const api = app.api, emulators = app.emulators;
  const calls = [];
  const info = (status) => [{ id: 'duckstation', name: 'DuckStation', systems: ['ps1'], integrated: true, requirement: 'Needs a BIOS', status, installed_version: status === 'ready' ? 'latest-1' : null }];
  const snap = { roots: [], media: [], sessions: [], editions: [], runners: [], settings: {} };
  try {
    app.api = { ...api, snapshot: async () => snap,
      installEmulator: async (id) => { calls.push(['install', id]); assert.ok(id in app.emulatorBusy); app.onEvent({ type: 'tool_install_progress', tool_id: id, message: 'Downloading…' }); assert.equal(app.emulatorBusy[id], 'Downloading…'); return info('ready'); },
      uninstallEmulator: async (id, purge) => { calls.push(['uninstall', id, purge]); return info('not_installed'); } };
    const pending = app.installEmulator(info('not_installed')[0]);
    await pending;
    assert.equal(app.emulators[0].status, 'ready'); assert.deepEqual(app.emulatorBusy, {});
    await app.uninstallEmulator(app.emulators[0], false);
    await app.uninstallEmulator(info('ready')[0], true);
    assert.deepEqual(calls, [['install', 'duckstation'], ['uninstall', 'duckstation', false], ['uninstall', 'duckstation', true]]);
    assert.deepEqual(app.emulatorBusy, {});
  } finally { app.api = api; app.emulators = emulators; }
});

test('a failed emulator install reports the error and does not stay busy', async () => {
  const api = app.api;
  try {
    app.api = { ...api, installEmulator: async () => { throw { code: 'NETWORK', message: 'offline' }; } };
    await app.installEmulator({ id: 'flycast', name: 'Flycast', requirement: '' });
    assert.equal(app.toast, 'offline'); assert.deepEqual(app.emulatorBusy, {});
  } finally { app.api = api; }
});

test('source build starts a backend job without claiming an installed emulator or game session', async () => {
  const api = app.api, emulators = app.emulators;
  const calls = [];
  const emulator = { id: 'mgba', name: 'mGBA', source_build: true, status: 'not_installed' };
  try {
    app.emulators = [emulator];
    app.api = { ...api, buildEmulator: async (id) => { calls.push(id); }, installEmulator: async () => { throw new Error('wrong installation method'); } };
    await app.installEmulator(emulator);
    assert.deepEqual(calls, ['mgba']);
    assert.equal(app.emulators[0].status, 'not_installed');
    assert.equal(app.running, null);
    assert.deepEqual(app.emulatorBusy, {});
  } finally { app.api = api; app.emulators = emulators; }
});

test('source build status, cancellation and rollback use the shared transport', async () => {
  const { commands } = await import('../src/lib/api/core');
  const calls = [];
  const api = commands(async (cmd, args) => { calls.push([cmd, args]); return null; });
  await api.buildEmulator('mgba');
  await api.emulatorBuildStatus('mgba');
  await api.cancelEmulatorBuild('mgba');
  await api.rollbackEmulator('mgba');
  assert.deepEqual(calls.map(([cmd]) => cmd), ['build_emulator', 'emulator_build_status', 'cancel_emulator_build', 'rollback_emulator']);
  assert.ok(calls.every(([, args]) => args.id === 'mgba'));
});

test('a completed source build refreshes Tools even after its dialog closes', async () => {
  const api = app.api, emulators = app.emulators;
  try {
    app.emulators = [{ id: 'mgba', source_build: true, status: 'not_installed' }];
    app.emulatorDialogId = null;
    app.api = { ...api, listEmulators: async () => [{ id: 'mgba', source_build: true, status: 'ready' }] };
    app.onEvent({ type: 'tool_install_finished', tool_id: 'mgba', runner_id: null });
    await Promise.resolve();
    assert.equal(app.emulators[0].status, 'ready');
    assert.equal(app.running, null);
  } finally { app.api = api; app.emulators = emulators; }
});

test('reduced motion combines the application and operating-system preferences', () => {
  const reduce = app.reduceMotion, system = app.systemReduceMotion;
  try {
    app.reduceMotion = false; app.systemReduceMotion = false;
    assert.equal(app.motionReduced, false);
    app.systemReduceMotion = true;
    assert.equal(app.motionReduced, true);
    app.systemReduceMotion = false; app.reduceMotion = true;
    assert.equal(app.motionReduced, true);
  } finally { app.reduceMotion = reduce; app.systemReduceMotion = system; }
});

test('remounting the desktop shell replaces its subscription and cleanup releases it', async () => {
  const api = app.api;
  let subscriptions = 0, releases = 0;
  try {
    app.api = { ...api, snapshot: async () => discSnapshot(), subscribe: () => { subscriptions++; return () => { releases++; }; } };
    await app.boot(); await app.boot();
    assert.equal(subscriptions, 2);
    assert.equal(releases, 1);
    app.disconnect(); app.disconnect();
    assert.equal(releases, 2);
  } finally { app.disconnect(); app.api = api; }
});


test('Home never promotes catalogue discoveries to the personal default hero', () => {
  app.games = [fixtureGame('discovery', { source: 'catalogue', installed: false, lastPlayed: '2099-01-01', favorite: true, coverUrl: '/cover.jpg' }), fixtureGame('personal')];
  assert.equal(app.defaultHero().id, 'personal');
  app.games = app.games.slice(0, 1);
  assert.equal(app.defaultHero(), null);
});

test('installed port engines retain unchecked game-data state after refresh', async () => {
  const api = app.api;
  try {
    app.api = { ...api, listPorts: async () => [{ id: 'shipwright', name: 'Ship of Harkinian', game: 'Ocarina of Time', family: 'Native', platform: 'n64', upstream: 'HarbourMasters/Shipwright', installed_version: '1', latest_version: null, status: 'ready', requirement: 'Supply your data', folder: '/test' }], checkPortUpdates: async () => app.api.listPorts() };
    await app.loadPorts();
    assert.equal(app.ports[0].status, 'installed');
    assert.equal(app.games.find((g) => g.id === 'port:shipwright').portStatus, 'installed');
  } finally { app.api = api; }
});


test('exact catalogue matching is offered only for available raw ISO media', () => {
  const snap = discSnapshot();
  snap.media[0].kind = 'ps1_cue'; snap.media[1].kind = 'ps2_iso';
  let entries = gamesFromSnapshot(snap);
  assert.equal(entries.find((g) => g.id === 'e1').catalogueMatchSupported, false);
  assert.equal(entries.find((g) => g.id === 'e2').catalogueMatchSupported, true);
  snap.media[1].available = false;
  entries = gamesFromSnapshot(snap);
  assert.equal(entries.find((g) => g.id === 'e2').catalogueMatchSupported, false);
});


test('emulator profiles use the shared desktop/browser command contract', async () => {
  const { commands } = await import('../src/lib/api/core');
  const calls = []; const api = commands(async (name, args) => { calls.push({name,args}); return {}; });
  const config = { mode:'custom', custom:{renderer:'Automatic',resolution_scale:2,vsync:true} };
  await api.emulatorProfile('duckstation'); await api.configureEmulatorProfile('duckstation',config);
  assert.deepEqual(calls,[{name:'emulator_profile',args:{id:'duckstation'}},{name:'configure_emulator_profile',args:{id:'duckstation',config}}]);
});


test('port dialog closes before navigation and retains the underlying Tools page', () => {
  app.route='tools'; app.portDialogId='shipwright';
  app.history=[{route:'home',gameId:null,heroId:null}];
  app.back(); assert.equal(app.portDialogId,null); assert.equal(app.route,'tools'); assert.equal(app.history.length,1);
});

test('shared port actions dispatch by availability without inventing a game session', async () => {
  const api=app.api; const calls=[];
  try {
    app.api={...api,openPortFolder:async id=>calls.push(['folder',id]),launchPort:async id=>calls.push(['launch',id]),installPort:async id=>{calls.push(['install',id]);return [];}};
    await app.runPortAction(fixturePort('missing-data',{status:'needs-data'}));
    await app.runPortAction(fixturePort('missing-rom',{status:'needs-rom'}));
    await app.runPortAction(fixturePort('installed',{status:'installed'}));
    await app.runPortAction(fixturePort('new',{status:'not-installed'}));
    assert.deepEqual(calls,[['folder','missing-data'],['folder','missing-rom'],['launch','installed'],['install','new']]);
    assert.equal(app.launching,null); assert.equal(app.running,null);
  } finally {app.api=api;}
});

test('port installation errors remain visible after busy clears and successful retry clears the error', async () => {
  const api=app.api; const port=fixturePort('shipwright',{status:'not-installed'}); let fail=true;
  try {
    app.api={...api,installPort:async()=>{if(fail)throw new Error('Download interrupted');return [];}};
    await app.runPortAction(port); assert.equal(app.portErrors.shipwright,'Download interrupted'); assert.equal(app.portBusy.shipwright,undefined);
    fail=false; await app.runPortAction(port); assert.equal(app.portErrors.shipwright,undefined);
  } finally {app.api=api;}
});

const intakeReport = (path, outcome = 'imported') => ({ archive:path, outcome, system:'nes',dest:'/games/nes',files:['Game.nes'],message:'Imported',source_removed:false });
const scanReport = () => ({scanned:1,added:1,updated:0,skipped_unsupported:0,skipped_by_kind:[],unresolved_serial:0,missing:0});

test('archive batches deduplicate paths, continue after failures and scan once after extraction', async () => {
  const api=app.api; let scans=0; const calls=[];
  try {
    app.api={...api,intakeArchive:async(path,remove)=>{calls.push([path,remove]);if(path.endsWith('bad.zip'))throw new Error('Unsafe archive');return intakeReport(path);},scan:async()=>{scans++;return scanReport();},snapshot:async()=>discSnapshot()};
    await app.importArchives(['/archives/bad.zip','/archives/game.zip','/archives/game.zip']);
    assert.deepEqual(calls,[['/archives/bad.zip',false],['/archives/game.zip',false]]);
    assert.equal(scans,1); assert.equal(app.intakeResults.length,2); assert.equal(app.intakeResults[0].error,'Unsafe archive');
    assert.deepEqual(app.intakePaths,['/archives/bad.zip']); assert.equal(app.intakeBusy,false);
  } finally {app.api=api;}
});

test('existing archive files are rescanned while skipped archives and invalid host paths are not', async () => {
  const api=app.api; let scans=0,imports=0;
  try {
    app.api={...api,intakeArchive:async(path)=>{imports++;return intakeReport(path,'already_present');},scan:async()=>{scans++;return scanReport();},snapshot:async()=>discSnapshot()};
    await app.importArchives(['/archives/old.7z']); assert.equal(scans,1);
    await app.importArchives(['relative.zip','/archives/no.tar']); assert.equal(imports,1); assert.equal(scans,1);
    await app.importArchives(['/archives/game.rar']); assert.equal(imports,2); assert.equal(scans,2);
    app.api={...app.api,intakeArchive:async(path)=>intakeReport(path,'skipped')};
    await app.importArchives(['/archives/docs.zip']); assert.equal(scans,2);
  } finally {app.api=api;}
});

test('failed post-import scan is distinct from extraction and can be retried without reimporting', async () => {
  const api=app.api; let imports=0,fail=true;
  try {
    app.api={...api,intakeArchive:async(path)=>{imports++;return intakeReport(path);},scan:async()=>{if(fail)throw new Error('Drive offline');return scanReport();},snapshot:async()=>discSnapshot()};
    await app.importArchives(['/archives/game.zip']);
    assert.equal(app.intakeResults[0].report.outcome,'imported'); assert.equal(app.intakeScanError,'Drive offline');
    fail=false; await app.refreshImportedGames(); assert.equal(app.intakeScanError,null); assert.equal(imports,1);
  } finally {app.api=api;}
});

test('a pending batch blocks duplicate execution and queueing does not imply extraction', async () => {
  const api=app.api; let resolve,imports=0;
  try {
    app.intakePaths=[]; app.queueArchives(['/archives/game.zip']); assert.equal(imports,0); assert.equal(app.intakeDelete,false);
    app.api={...api,intakeArchive:async()=>{imports++;return new Promise(done=>{resolve=done;});}};
    const pending=app.importArchives(); await app.importArchives(); assert.equal(imports,1);
    resolve(intakeReport('/archives/game.zip','skipped')); await pending;
  } finally {app.api=api;}
});

test('source removal requires an explicit batch choice and resets before the next batch', async () => {
  const api=app.api; const deletes=[];
  try {
    app.api={...api,intakeArchive:async(path,remove)=>{deletes.push(remove);return intakeReport(path,'skipped');}};
    app.intakeDelete=true; await app.importArchives(['/archives/game.zip'],true); assert.equal(app.intakeDelete,false);
    await app.importArchives(['/archives/next.zip']); assert.deepEqual(deletes,[true,false]);
  } finally {app.api=api;}
});

test('archive transport defaults to retaining source files on both surfaces', async () => {
  const { commands }=await import('../src/lib/api/core'); const calls=[];
  const api=commands(async(name,args)=>{calls.push({name,args});});
  await api.intakeArchive('/archives/game.zip'); await api.intakeArchive('/archives/game.7z',true);
  assert.deepEqual(calls,[{name:'intake_archive',args:{path:'/archives/game.zip',delete_source:false}},{name:'intake_archive',args:{path:'/archives/game.7z',delete_source:true}}]);
});

test('transfer persistence errors stay visible and clear after a successful refresh', async () => {
  const api=app.api, settings=app.runnerSettings;
  try {
    app.runnerSettings={...settings,torrents:{enabled:true,seeding:false,upload_kbps:0,download_kbps:0}};
    app.api={...api,transferStatus:async()=>{throw new Error('Saved transfer state is corrupt; preserved');}};
    await app.refreshTransfers(); assert.match(app.transferError,/corrupt/);
    app.api={...api,transferStatus:async()=>({enabled:true,seeding:false,items:[]})};
    await app.refreshTransfers(); assert.equal(app.transferError,''); assert.deepEqual(app.transfers.items,[]);
  } finally {app.api=api;app.runnerSettings=settings;app.transferError='';}
});

test('preparation commands keep review and execution separate and preserve exact job identity', async () => {
  const { commands } = await import('../src/lib/api/core');
  const calls = [];
  const api = commands(async (name, args) => { calls.push({name,args}); return []; });
  await api.preparationSystems();
  await api.inspectPreparation('/archives/game.zip', 'genesis');
  await api.preparationJobs();
  assert.equal(calls.some(c => c.name === 'start_preparation'), false);
  await api.startPreparation('stable-job', 'review-fingerprint');
  await api.cancelPreparation('other-job');
  assert.deepEqual(calls, [
    {name:'preparation_systems',args:undefined},
    {name:'inspect_preparation',args:{path:'/archives/game.zip',system:'genesis'}},
    {name:'preparation_jobs',args:undefined},
    {name:'start_preparation',args:{id:'stable-job',review_token:'review-fingerprint'}},
    {name:'cancel_preparation',args:{id:'other-job'}},
  ]);
});

test('manager controls share the desktop/browser transport and do not launch games', async () => {
  const { commands } = await import('../src/lib/api/core');
  const calls=[];
  const api=commands(async(name,args)=>{calls.push([name,args]);return {};});
  await api.managerStatus(); await api.startManager(); await api.stopManager();
  assert.deepEqual(calls,[['manager_status',undefined],['start_manager',undefined],['stop_manager',undefined]]);
});

test('legacy Ports navigation opens tagged Games and clears incompatible discovery filters', async () => {
  const {libraryView}=await import('../src/lib/libraryView.svelte');
  const view=libraryView('all');Object.assign(view,{discovery:true,query:'old',platform:'ps2',sourceFilter:'rom',region:'USA',favOnly:true,installedOnly:true});
  app.go('ports');
  assert.equal(app.route,'library');assert.equal(view.sourceFilter,'port');assert.equal(view.discovery,false);
  assert.equal(view.platform,'');assert.equal(view.query,'');assert.equal(view.region,'');assert.equal(view.favOnly,false);assert.equal(view.installedOnly,false);
});
