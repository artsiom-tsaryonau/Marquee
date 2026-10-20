import assert from 'node:assert/strict';
import { afterEach, beforeEach, test } from 'node:test';
import { startFocusEngine } from '../src/lib/focus';

let keydown, frame, pads, clicks, backCalls, stop;
const originals = new Map(['window', 'document', 'navigator', 'requestAnimationFrame', 'cancelAnimationFrame']
  .map((key) => [key, Object.getOwnPropertyDescriptor(globalThis, key)]));

beforeEach(() => {
  clicks = backCalls = 0;
  pads = [];
  const globals = {
    window: { addEventListener: (_, fn) => (keydown = fn), removeEventListener() {} },
    document: { activeElement: { click: () => clicks++ }, querySelectorAll: () => [] },
    navigator: { getGamepads: () => pads },
    requestAnimationFrame: (fn) => (frame = fn, 1),
    cancelAnimationFrame() {},
  };
  for (const [key, value] of Object.entries(globals)) {
    Object.defineProperty(globalThis, key, { value, writable: true, configurable: true });
  }
  stop = startFocusEngine({ back: () => backCalls++, onInput() {} });
});

afterEach(() => {
  stop();
  for (const [key, descriptor] of originals) {
    if (descriptor) Object.defineProperty(globalThis, key, descriptor);
    else delete globalThis[key];
  }
});

function pad(index, pressed = false) {
  return { index, axes: [0, 0], buttons: Array.from({ length: 16 }, (_, i) => ({ pressed: i === 0 && pressed })) };
}

function key(target, key) {
  let prevented = false;
  keydown({ target, key, preventDefault: () => (prevented = true) });
  return prevented;
}

test('native select retains all arrow keys for changing sort order', () => {
  for (const arrow of ['ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight']) {
    assert.equal(key({ tagName: 'SELECT' }, arrow), false);
  }
});

test('text fields keep editing keys while search still supports moving down', () => {
  assert.equal(key({ tagName: 'INPUT', type: 'text' }, 'ArrowLeft'), false);
  assert.equal(key({ tagName: 'INPUT', type: 'text' }, 'ArrowDown'), true);
  assert.equal(key({ tagName: 'TEXTAREA' }, 'ArrowDown'), false);
  assert.equal(key({ tagName: 'DIV', isContentEditable: true }, 'ArrowUp'), false);
  key({ tagName: 'TEXTAREA' }, 'Backspace');
  assert.equal(backCalls, 0);
});

test('buttons still use spatial navigation and Escape still goes back', () => {
  assert.equal(key({ tagName: 'BUTTON' }, 'ArrowDown'), true);
  key({ tagName: 'BUTTON' }, 'Escape');
  assert.equal(backCalls, 1);
});

test('held A emits one click with a second idle controller in either order', () => {
  pads = [pad(0, true), pad(1)];
  frame(); frame(); frame();
  assert.equal(clicks, 1);
  pads = [pad(1), pad(0, true)];
  frame(); frame();
  assert.equal(clicks, 1);
  pads[1].buttons[0].pressed = false;
  frame();
  pads[1].buttons[0].pressed = true;
  frame();
  assert.equal(clicks, 2);
});

test('controllers have independent press edges', () => {
  pads = [pad(0, true), pad(1, true)];
  frame(); frame();
  assert.equal(clicks, 2);
});

test('disconnect clears held state before a controller reconnects', () => {
  pads = [pad(0, true)];
  frame();
  pads = [null];
  frame();
  pads = [pad(0, true)];
  frame();
  assert.equal(clicks, 2);
});
