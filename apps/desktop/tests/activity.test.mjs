import assert from 'node:assert/strict';
import { afterEach, beforeEach, mock, test } from 'node:test';
import { pollWhileActive } from '../src/lib/activity';

let listeners, stop;
const keys = ['document', 'window'];
const originals = new Map(keys.map((key) => [key, Object.getOwnPropertyDescriptor(globalThis, key)]));
beforeEach(() => {
  mock.timers.enable({ apis: ['setTimeout'] });
  listeners = new Map();
  const events = { addEventListener: (key, handler) => listeners.set(key, handler), removeEventListener: (key) => listeners.delete(key) };
  Object.defineProperty(globalThis, 'window', { value: events, configurable: true });
  Object.defineProperty(globalThis, 'document', { value: { ...events, hidden: false, hasFocus: () => true }, configurable: true });
});
afterEach(() => {
  stop?.(); stop = undefined;
  mock.timers.reset();
  for (const [key, descriptor] of originals) {
    if (descriptor) Object.defineProperty(globalThis, key, descriptor);
    else delete globalThis[key];
  }
});
const flush = async () => { await Promise.resolve(); await Promise.resolve(); };

test('hidden/unfocused windows make no requests and resume immediately', async () => {
  let calls = 0;
  document.hidden = true;
  stop = pollWhileActive(async () => { calls++; }, () => 1500);
  mock.timers.tick(60000);
  assert.equal(calls, 0);
  document.hidden = false;
  listeners.get('visibilitychange')();
  await flush();
  assert.equal(calls, 1);
  mock.timers.tick(1500); await flush();
  assert.equal(calls, 2);
  document.hasFocus = () => false;
  listeners.get('blur')();
  mock.timers.tick(60000); await flush();
  assert.equal(calls, 2);
  document.hasFocus = () => true;
  listeners.get('focus')(); await flush();
  assert.equal(calls, 3);
});

test('slow requests cannot overlap or restart a disposed poller', async () => {
  let calls = 0, finish;
  stop = pollWhileActive(() => { calls++; return new Promise((resolve) => { finish = resolve; }); }, () => 1000);
  mock.timers.tick(60000);
  listeners.get('focus')();
  assert.equal(calls, 1);
  stop();
  finish(); await flush();
  mock.timers.tick(60000);
  assert.equal(calls, 1);
  assert.equal(listeners.size, 0);
});

test('polling slows down after work becomes idle', async () => {
  let calls = 0, busy = true;
  stop = pollWhileActive(async () => { calls++; }, () => busy ? 1500 : 15000);
  await flush();
  busy = false;
  mock.timers.tick(1500); await flush();
  assert.equal(calls, 2);
  mock.timers.tick(14999); await flush();
  assert.equal(calls, 2);
  mock.timers.tick(1); await flush();
  assert.equal(calls, 3);
});
