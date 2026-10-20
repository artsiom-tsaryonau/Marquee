import assert from 'node:assert/strict';
import { after, beforeEach, mock, test } from 'node:test';

mock.timers.enable({ apis: ['setTimeout', 'setInterval'] });
const { app } = await import('../src/lib/state.svelte');
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
  assert.equal(app.games.some((g) => g.id === 'port:opengoal'), false);
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
