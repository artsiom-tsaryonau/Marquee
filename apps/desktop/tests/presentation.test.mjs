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

test('game detail tabs include Metadata and keyboard navigation wraps across all three', async () => {
  const {gameDetailTabs,nextGameDetailTab}=await import('../src/lib/gameDetails');
  assert.deepEqual(gameDetailTabs,['overview','ost','metadata']);
  assert.equal(nextGameDetailTab('overview','ArrowLeft'),'metadata');
  assert.equal(nextGameDetailTab('metadata','ArrowRight'),'overview');
  assert.equal(nextGameDetailTab('ost','ArrowRight'),'metadata');
  assert.equal(nextGameDetailTab('ost','Home'),'overview');
  assert.equal(nextGameDetailTab('overview','End'),'metadata');
  assert.equal(nextGameDetailTab('ost','Enter'),'ost');
});
