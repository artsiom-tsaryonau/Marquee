import assert from 'node:assert/strict';
import { test } from 'node:test';
import { StreamingControls } from '../src/lib/streaming.svelte';
import { commands } from '../src/lib/api/core';

test('destructive actions require a second click on the same item', async () => {
  const calls = [];
  const controls = new StreamingControls({ streamingState: async () => ({}), streamingEndGame: async id => calls.push(id), streamingRevoke: async id => calls.push(id) });
  await controls.confirm('end', 'one');
  await controls.confirm('revoke', 'two');
  assert.deepEqual(calls, []);
  await controls.confirm('revoke', 'two');
  assert.deepEqual(calls, ['two']);
  assert.equal(controls.confirmation, '');
});
test('action failure persists through successful polling and retry clears it', async () => {
  const controls = new StreamingControls({ streamingState: async () => ({ operation: null }) });
  await controls.act(async () => { throw { code: 'OFFLINE', message: 'Host unavailable' }; });
  await controls.refresh();
  assert.equal(controls.error, 'Host unavailable');
  await controls.act(async () => {});
  assert.equal(controls.error, '');
});
test('polling never overlaps a command and disposal ignores late results', async () => {
  let finish;
  let count = 0;
  const controls = new StreamingControls({ streamingState: () => { count++; return new Promise(resolve => finish = resolve); } });
  const request = controls.refresh();
  await controls.refresh();
  await controls.act(async () => { throw new Error('must not execute'); });
  assert.equal(count, 1);
  controls.dispose(); finish({ host: {} }); await request;
  assert.equal(controls.state, null);
  await controls.refresh(); assert.equal(count, 1);
});
test('shared commands keep viewer disconnect distinct from stopping a game', async () => {
  const calls = [];
  const api = commands(async (cmd, args) => calls.push([cmd, args]));
  await api.streamingDisconnectViewer(); await api.streamingEndGame('s'); await api.streamingRevoke('c'); await api.streamingPairViewer();
  assert.deepEqual(calls, [['streaming_disconnect_viewer', undefined], ['streaming_end_game', {session_id:'s', confirmed:true}], ['streaming_revoke', {client_id:'c', confirmed:true}], ['streaming_pair_viewer', undefined]]);
});
