import assert from 'node:assert/strict';
import { test } from 'node:test';
import { modal } from '../src/lib/modal';

test('dialogs isolate the page, contain Tab and Escape, and restore prior focus and inert state', async () => {
  const previous = new Map(['document', 'HTMLElement'].map((key) => [key, Object.getOwnPropertyDescriptor(globalThis, key)]));
  class Element {
    children = []; parentElement = null; inert = false; tabIndex = 0; isConnected = true; listeners = new Map();
    append(child) { this.children.push(child); child.parentElement = this; return child; }
    focus() { document.activeElement = this; }
    closest() { for (let node = this; node; node = node.parentElement) if (node.inert) return node; return null; }
    getClientRects() { return [1]; }
    matches() { return false; }
    hasAttribute(name) { return name === 'data-modal-dismiss' && !!this.dismiss; }
    querySelectorAll() { return this.children; }
    addEventListener(name, fn) { this.listeners.set(name, fn); }
    removeEventListener(name) { this.listeners.delete(name); }
  }
  const body = new Element();
  const page = body.append(new Element());
  const sidebar = page.append(new Element());
  const content = page.append(new Element());
  const opener = content.append(new Element());
  const alreadyInert = content.append(new Element()); alreadyInert.inert = true;
  const externallyManaged = content.append(new Element()); externallyManaged.inert = true;
  const scrim = content.append(new Element()); scrim.dismiss = true;
  const dialog = content.append(new Element());
  const first = dialog.append(new Element());
  const last = dialog.append(new Element());
  Object.defineProperty(globalThis, 'HTMLElement', { configurable: true, value: Element });
  Object.defineProperty(globalThis, 'document', { configurable: true, value: { body, activeElement: opener } });
  let closes = 0;
  let action;
  try {
    action = modal(dialog, () => closes++);
    assert.equal(sidebar.inert, true); assert.equal(opener.inert, true); assert.equal(dialog.inert, false);
    assert.equal(scrim.inert, false, 'the dialog dismissal backdrop must remain clickable');
    await Promise.resolve(); assert.equal(document.activeElement, first);
    const event = (key, shiftKey = false) => ({ key, shiftKey, prevented: false, stopped: false,
      preventDefault() { this.prevented = true; }, stopPropagation() { this.stopped = true; } });
    last.focus(); const tab = event('Tab'); dialog.listeners.get('keydown')(tab);
    assert.equal(document.activeElement, first); assert.equal(tab.prevented, true);
    const backTab = event('Tab', true); dialog.listeners.get('keydown')(backTab);
    assert.equal(document.activeElement, last);
    const escape = event('Escape'); dialog.listeners.get('keydown')(escape);
    assert.equal(closes, 1); assert.equal(escape.stopped, true, 'global Back must not run a second time');
    externallyManaged.inert = false; // The shell releases its own lock while the dialog closes.
    action.destroy(); action = null;
    assert.equal(externallyManaged.inert, false, 'dialog cleanup must not re-lock a shell that already released its own lock');
    assert.equal(sidebar.inert, false); assert.equal(opener.inert, false); assert.equal(alreadyInert.inert, true);
    assert.equal(document.activeElement, opener);
  } finally {
    action?.destroy();
    for (const [key, descriptor] of previous) {
      if (descriptor) Object.defineProperty(globalThis, key, descriptor); else delete globalThis[key];
    }
  }
});
