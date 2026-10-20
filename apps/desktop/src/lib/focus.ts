// Spatial focus engine: arrow keys / D-pad move focus to the nearest
// [data-nav] element in that direction. The Tauri build will feed gilrs
// gamepad events into `move` and `press`; in a browser the Gamepad API does.

type Dir = 'up' | 'down' | 'left' | 'right';

function candidates(): HTMLElement[] {
  return [...document.querySelectorAll<HTMLElement>('[data-nav]')].filter((el) => {
    if (el.closest('[inert]')) return false;
    const r = el.getBoundingClientRect();
    return r.width > 0 && r.height > 0;
  });
}

export function move(dir: Dir) {
  const all = candidates();
  const active = document.activeElement as HTMLElement | null;
  if (!active || !all.includes(active)) {
    all[0]?.focus();
    return;
  }
  const a = active.getBoundingClientRect();
  const ax = a.left + a.width / 2;
  const ay = a.top + a.height / 2;
  const horizontal = dir === 'left' || dir === 'right';
  // Stay within the same row/group when moving sideways, if one is marked.
  const group = horizontal ? active.closest('[data-nav-row]') : null;

  const find = (within: Element | null) => {
    let best: HTMLElement | null = null;
    let bestScore = Infinity;
    for (const el of all) {
      if (el === active || (within && !within.contains(el))) continue;
      const r = el.getBoundingClientRect();
      const dx = r.left + r.width / 2 - ax;
      const dy = r.top + r.height / 2 - ay;
      const primary = { left: -dx, right: dx, up: -dy, down: dy }[dir];
      if (primary <= 4) continue;
      const cross = horizontal ? Math.abs(dy) : Math.abs(dx);
      const score = primary + cross * (horizontal ? 3 : 1.5);
      if (score < bestScore) {
        bestScore = score;
        best = el;
      }
    }
    return best;
  };
  // At the end of a row, fall through to the whole screen (e.g. the sidebar).
  const best = (group && find(group)) || find(null);
  if (best) {
    best.focus({ preventScroll: true });
    best.scrollIntoView({ block: 'nearest', inline: 'center', behavior: matchMedia('(prefers-reduced-motion: reduce)').matches ? 'auto' : 'smooth' });
  }
}

const keyDir: Record<string, Dir> = { ArrowUp: 'up', ArrowDown: 'down', ArrowLeft: 'left', ArrowRight: 'right' };

export function startFocusEngine(opts: { back: () => void; onInput: (label: string) => void }) {
  const onKey = (e: KeyboardEvent) => {
    if (e.defaultPrevented || e.isComposing) return;
    const t = e.target as HTMLElement;
    const typing = t.tagName === 'INPUT' && (t as HTMLInputElement).type !== 'checkbox';
    const multiline = t.tagName === 'TEXTAREA' || t.isContentEditable;
    const nativeArrows = t.tagName === 'SELECT' || multiline ||
      (typing && ['number', 'range', 'date', 'time', 'datetime-local', 'month', 'week'].includes((t as HTMLInputElement).type));
    const dir = keyDir[e.key];
    if (dir && !(nativeArrows || (typing && horizontalKeyInText(e)))) {
      e.preventDefault();
      opts.onInput(`Key ${e.key}`);
      move(dir);
    } else if (e.key === 'Escape' || (e.key === 'Backspace' && !typing && !multiline && t.tagName !== 'SELECT')) {
      opts.onInput(`Key ${e.key}`);
      opts.back();
    }
  };
  window.addEventListener('keydown', onKey);

  // Browser Gamepad API: standard mapping, A=0 B=1 X=2 Y=3, D-pad 12-15.
  const heldByPad = new Map<number, Set<string>>();
  let raf = 0;
  const poll = () => {
    const connected = new Set<number>();
    for (const pad of navigator.getGamepads?.() ?? []) {
      if (!pad) continue;
      connected.add(pad.index);
      let held = heldByPad.get(pad.index);
      if (!held) heldByPad.set(pad.index, (held = new Set()));
      const pressed = (i: number) => pad.buttons[i]?.pressed;
      const ax = pad.axes[0] ?? 0;
      const ay = pad.axes[1] ?? 0;
      const state: Record<string, boolean> = {
        up: pressed(12) || ay < -0.6,
        down: pressed(13) || ay > 0.6,
        left: pressed(14) || ax < -0.6,
        right: pressed(15) || ax > 0.6,
        a: pressed(0),
        b: pressed(1),
      };
      for (const [k, on] of Object.entries(state)) {
        if (on && !held.has(k)) {
          held.add(k);
          opts.onInput(`Pad ${k.toUpperCase()}`);
          if (k === 'a') (document.activeElement as HTMLElement | null)?.click();
          else if (k === 'b') opts.back();
          else move(k as Dir);
        } else if (!on) held.delete(k);
      }
    }
    for (const index of heldByPad.keys()) {
      if (!connected.has(index)) heldByPad.delete(index);
    }
    // No controller connected: stop polling (no 60 fps wake-ups) until one appears.
    raf = connected.size > 0 ? requestAnimationFrame(poll) : 0;
  };
  const startPolling = () => {
    if (!raf) raf = requestAnimationFrame(poll);
  };
  window.addEventListener('gamepadconnected', startPolling);
  if ((navigator.getGamepads?.() ?? []).some(Boolean)) startPolling();

  return () => {
    window.removeEventListener('keydown', onKey);
    window.removeEventListener('gamepadconnected', startPolling);
    cancelAnimationFrame(raf);
  };
}

function horizontalKeyInText(e: KeyboardEvent) {
  return e.key === 'ArrowLeft' || e.key === 'ArrowRight';
}
