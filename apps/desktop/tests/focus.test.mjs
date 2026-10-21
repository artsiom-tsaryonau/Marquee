import assert from 'node:assert/strict';
import { afterEach, beforeEach, test } from 'node:test';
import { startFocusEngine, move } from '../src/lib/focus';

let keydown, frame, pads, clicks, backCalls, stop, listeners;
const originals = new Map(['window', 'document', 'navigator', 'requestAnimationFrame', 'cancelAnimationFrame']
  .map((key) => [key, Object.getOwnPropertyDescriptor(globalThis, key)]));

beforeEach(() => {
  clicks = backCalls = 0;
  pads = [];
  listeners = {};
  frame = undefined;
  const globals = {
    window: {
      addEventListener: (type, fn) => (type === 'keydown' ? (keydown = fn) : (listeners[type] = fn)),
      removeEventListener() {},
    },
    document: { hidden: false, hasFocus: () => true,
      addEventListener: (type, fn) => (listeners[type] = fn), removeEventListener() {},
      activeElement: { click: () => clicks++ }, querySelectorAll: () => [] },
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

// A browser fires this when a controller first reports input; polling only runs from then on.
function connect() {
  listeners.gamepadconnected();
}

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
  connect();
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
  connect();
  frame(); frame();
  assert.equal(clicks, 2);
});

test('disconnect clears held state before a controller reconnects', () => {
  pads = [pad(0, true)];
  connect();
  frame();
  pads = [null];
  frame();
  pads = [pad(0, true)];
  connect();
  frame();
  assert.equal(clicks, 2);
});

test('no controller means no polling loop at all', () => {
  assert.equal(frame, undefined, 'nothing is scheduled before a controller connects');
  pads = [pad(0)];
  connect();
  const poll = frame;
  frame = undefined;
  poll();
  assert.notEqual(frame, undefined, 'polling continues while a controller is present');
  const next = frame;
  frame = undefined;
  pads = [null];
  next();
  assert.equal(frame, undefined, 'polling stops once the controller is gone');
});

test('background controllers cannot activate the launcher and resume on focus', () => {
  pads = [pad(0, true)];
  document.hidden = true;
  connect();
  assert.equal(frame, undefined);
  document.hidden = false;
  listeners.visibilitychange();
  assert.ok(frame);
  const pending = frame;
  frame = undefined;
  document.hasFocus = () => false;
  pending();
  assert.equal(clicks, 0);
  assert.equal(frame, undefined);
  document.hasFocus = () => true;
  listeners.focus();
  frame();
  assert.equal(clicks, 1);
});

test('disposed controller engine cannot schedule work from an already queued frame', () => {
  pads = [pad(0, true)];
  connect();
  const pending = frame;
  frame = undefined;
  stop();
  pending();
  assert.equal(clicks, 0);
  assert.equal(frame, undefined);
});


test('spatial navigation skips disabled launcher controls', () => {
  let focused = '';
  const button = (id, disabled) => ({
    closest: () => null,
    matches: () => disabled,
    getAttribute: () => null,
    getBoundingClientRect: () => ({ width: 100, height: 40 }),
    focus: () => { focused = id; },
  });
  document.querySelectorAll = () => [button('disabled', true), button('enabled', false)];
  move('down');
  assert.equal(focused, 'enabled');
});

test('holding a direction repeats after a delay while A remains a single activation', () => {
  const previousNow = Date.now;
  let now = 1000, focusMoves = 0;
  Date.now = () => now;
  document.querySelectorAll = () => [{
    closest: () => null, matches: () => false, getAttribute: () => null,
    getBoundingClientRect: () => ({ width: 100, height: 40 }), focus: () => focusMoves++,
  }];
  try {
    pads = [pad(0, true)]; pads[0].buttons[15].pressed = true;
    connect(); frame();
    assert.equal(focusMoves, 1); assert.equal(clicks, 1);
    now = 1399; frame(); assert.equal(focusMoves, 1);
    now = 1400; frame(); assert.equal(focusMoves, 2);
    now = 1520; frame(); assert.equal(focusMoves, 3); assert.equal(clicks, 1);
    pads[0].buttons[15].pressed = false; frame();
    pads[0].buttons[15].pressed = true; frame(); assert.equal(focusMoves, 4);
  } finally { Date.now = previousNow; }
});
