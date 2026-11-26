import assert from 'node:assert/strict';
import { test } from 'node:test';
import { activityCounts } from '../src/lib/downloads.ts';
import { catalogDefaultEmulator } from '../src/lib/runnerChoices.ts';

test('cover capabilities are separate from the API token', () => {
  const token = 'a'.repeat(64);
  const key = 'b'.repeat(64);
  const path = `/art/${encodeURIComponent('cover.png')}?key=${encodeURIComponent(key)}`;
  assert.equal(path.includes(token), false);
  assert.match(path, /^\/art\/cover\.png\?key=/);
});

test('catalog systems choose the default emulator', () => {
  const fallback = { genesis: 'blastem', n64: 'rmg' };
  assert.equal(catalogDefaultEmulator([{ id: 'n64', emulator: 'rmg' }], 'n64', fallback), 'rmg');
  assert.equal(catalogDefaultEmulator([{ id: 'n64', emulator: 'new-engine' }], 'n64', fallback), 'new-engine');
  assert.equal(catalogDefaultEmulator(undefined, 'genesis', fallback), 'blastem');
});

test('download activity ignores finished work', () => {
  const counts = activityCounts(
    [{ state: 'preparing' }, { state: 'registered' }, { state: 'failed' }],
    [{ state: 'downloading' }, { state: 'complete' }, { state: 'error' }, { state: 'source_missing' }],
    [{ state: 'waiting' }, { state: 'completed' }],
  );
  assert.deepEqual(counts, { preparation: 2, torrents: 3, browser: 1 });
  assert.equal(counts.preparation + counts.torrents + counts.browser, 6);
});
