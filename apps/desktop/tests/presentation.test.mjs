import assert from 'node:assert/strict';
import { test } from 'node:test';
import { canPlay, gameStatus, portAction, emulatorName } from '../src/lib/presentation';

const game = (overrides = {}) => ({ source: 'rom', platform: 'ps2', installed: true, ...overrides });

test('files on disk do not imply a playable installation', () => {
  for (const overrides of [
    { source: 'catalogue' }, { wanted: 'owned' }, { wanted: 'wishlist' }, { rootOffline: true },
    { mediaMissing: true }, { setupState: 'missing' }, { setupState: 'runner_missing' },
    { source: 'port', portStatus: 'needs-rom' }, { source: 'port', portStatus: 'needs-data' },
    { source: 'port', portStatus: 'installed' }, { source: 'port', portStatus: 'update' }, { installed: false },
  ]) assert.equal(canPlay(game(overrides)), false, JSON.stringify(overrides));
  assert.equal(canPlay(game({ setupState: 'unverified' })), true, 'optional verification never gates launch');
  assert.equal(canPlay(game({ source: 'port', portStatus: 'ready' })), true);
});

test('an offline drive and a missing file retain different recovery states', () => {
  assert.equal(gameStatus(game({ rootOffline: true, mediaMissing: true })).label, 'Drive unavailable');
  assert.equal(gameStatus(game({ mediaMissing: true })).label, 'Files unavailable');
  assert.equal(gameStatus(game({ installed: false })).label, 'Not installed');
  assert.equal(gameStatus(game({ source: 'catalogue', installed: false })).label, 'Catalogue only');
});

test('missing port data never dispatches to engine installation', () => {
  for (const status of ['needs-rom', 'needs-data']) {
    assert.equal(portAction({ status }).operation, 'folder');
    assert.match(portAction({ status }).label, /folder/);
  }
  assert.equal(portAction({ status: 'not-installed' }).operation, 'install');
  assert.equal(portAction({ status: 'update' }).operation, 'install');
  assert.deepEqual(portAction({ status: 'installed' }), { label: 'Open port', operation: 'launch' });
  assert.deepEqual(portAction({ status: 'ready' }), { label: 'Play', operation: 'launch' });
});

test('launch names follow the selected emulator rather than defaulting every ROM to PCSX2', () => {
  assert.equal(emulatorName(game({ platform: 'ps1' })), 'DuckStation');
  assert.equal(emulatorName(game({ platform: 'ds' })), 'melonDS');
  assert.equal(emulatorName(game({ platform: 'ps2', runnerName: 'Custom PCSX2 (Marquee)' })), 'Custom PCSX2');
});

test('game media gallery orders real assets and deduplicates URLs without inventing screenshots', async () => {
  const {gameMedia}=await import('../src/lib/gameMedia');
  assert.deepEqual(gameMedia({title:'Sonic 2'}),[]);
  const cover=gameMedia({title:'Sonic 2',coverUrl:'/cover.png'});
  assert.equal(cover.length,1);assert.equal(cover[0].kind,'cover');
  const items=gameMedia({title:'Game',heroUrl:'/hero.jpg',coverUrl:'/cover.jpg',screenshots:[{url:'/hero.jpg',caption:'duplicate'},{url:'/shot.jpg',caption:'Actual gameplay'}]});
  assert.deepEqual(items.map(i=>i.kind),['hero','screenshot','cover']);
  assert.equal(items[1].caption,'Actual gameplay');
});

test('platform shelves preserve filtered order inside each system and keep every game once', async () => {
  const {caseRatio,platformShelves}=await import('../src/lib/coverLayout');
  const games=[{id:'unknown',platform:'unknown'},{id:'ps2-b',platform:'ps2'},{id:'ps1-a',platform:'ps1'},{id:'ps2-a',platform:'ps2'}];
  const shelves=platformShelves(games);
  assert.deepEqual(shelves.map(([p])=>p),['ps2','ps1','unknown']);
  assert.deepEqual(shelves[0][1].map(g=>g.id),['ps2-b','ps2-a']);
  assert.equal(new Set(shelves.flatMap(([,list])=>list.map(g=>g.id))).size,games.length);
  assert.deepEqual(games.map(g=>g.id),['unknown','ps2-b','ps1-a','ps2-a']);
  assert.equal(caseRatio('ps1'),1);assert.equal(caseRatio('ps2'),2/3);assert.ok(caseRatio('snes')>1);
});

test('game detail tabs include Media and Metadata; keyboard navigation wraps across all three', async () => {
  const {gameDetailTabs,nextGameDetailTab}=await import('../src/lib/gameDetails');
  assert.deepEqual(gameDetailTabs,['overview','media','metadata']);
  assert.equal(nextGameDetailTab('overview','ArrowLeft'),'metadata');
  assert.equal(nextGameDetailTab('metadata','ArrowRight'),'overview');
  assert.equal(nextGameDetailTab('overview','ArrowRight'),'media');
  assert.equal(nextGameDetailTab('media','Home'),'overview');
  assert.equal(nextGameDetailTab('overview','End'),'metadata');
  assert.equal(nextGameDetailTab('media','Enter'),'media');
});

test('console icon profiles distinguish common systems and missing artwork initials stay decorative', async () => {
  const {platformIconStyle,gameInitials}=await import('../src/lib/platformIcons');
  assert.equal(platformIconStyle('gbc').hardware,'handheld');
  assert.notEqual(platformIconStyle('gbc').shell,platformIconStyle('gb').shell);
  assert.equal(platformIconStyle('nes').hardware,'nes');assert.equal(platformIconStyle('genesis').hardware,'genesis');
  assert.equal(platformIconStyle('ps1').hardware,'ps1');assert.equal(platformIconStyle('ps2').hardware,'ps2');
  assert.equal(platformIconStyle('virtualboy').hardware,'headset');
  assert.equal(platformIconStyle('constructor').hardware,'console');assert.equal(platformIconStyle('unlisted').hardware,'console');
  assert.equal(gameInitials('The Legend of Zelda (USA)'),'LZ');assert.equal(gameInitials('2048'),'20');
  assert.equal(gameInitials('Sonic the Hedgehog 2'),'SH');assert.equal(gameInitials(''),'?' );
});

test('GB defaults and per-game overrides use compatible engines without hiding a missing selection', async () => {
  const { pickRunner, editionToGame } = await import('../src/lib/api/map');
  const runner = (id, systems, available = true) => ({id:`runner.${id}.marquee`,systems,kind:'emulator',source:'marquee',display_name:id,available,bios_state:'unverified'});
  const mesen = runner('mesen',['gb','gbc','nes']);
  const mgba = runner('mgba',['gb','gbc']);
  const snap = {runners:[mgba,mesen],settings:{},media:[],roots:[]};
  assert.equal(pickRunner(snap,'gb').id,mesen.id);
  snap.settings.selected_runner_ids = {gb:mgba.id,gbc:mesen.id};
  assert.equal(pickRunner(snap,'gb').id,mgba.id);
  assert.equal(pickRunner(snap,'gbc').id,mesen.id);
  assert.equal(pickRunner(snap,'gb',mesen.id).id,mesen.id);
  assert.equal(pickRunner(snap,'nes',mgba.id),null);
  mgba.available = false;
  const game = editionToGame({id:'game',system:'gb',title:'Test',playtime_seconds:0},snap);
  assert.equal(game.runnerId,mgba.id);
  assert.equal(game.setupState,'runner_missing');
  snap.runners = [mesen];
  assert.equal(pickRunner(snap,'gb'),null);
  assert.equal(pickRunner(snap,'gb',mesen.id).id,mesen.id);
});

 test('PS3 discs require an RPCS3 runner and never claim PCSX2 readiness',async()=>{
 const {editionToGame}=await import('../src/lib/api/map');
 const snap={runners:[],settings:{},roots:[],media:[{id:'disc',kind:'ps3_iso',path:'/game.iso',available:true}]};
 const entry=editionToGame({id:'gow3',system:'ps3',title:'God of War III',media_id:'disc',missing:false,playtime_seconds:0},snap);
 assert.equal(entry.platform,'ps3');assert.equal(entry.installed,true);assert.equal(canPlay(entry),false);assert.match(gameStatus(entry).label,/install|setup|emulator/i);assert.equal(emulatorName(entry),'RPCS3');
 const ready=editionToGame({id:'gow3',system:'ps3',title:'God of War III',media_id:'disc',missing:false,playtime_seconds:0},{...snap,runners:[{id:'runner.rpcs3.marquee',kind:'emulator',systems:['ps3'],available:true,bios_state:'unverified',source:'marquee'}]});
 assert.equal(canPlay(ready),true);assert.equal(ready.runnerId,'runner.rpcs3.marquee');
 });
