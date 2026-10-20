<script lang="ts">
  import { app } from '../state.svelte';
  import Icon from '../components/Icon.svelte';

  const sources = $state([
    { name: 'Lutris', detail: '~/.local/share/lutris/pga.db · read-only', on: true },
    { name: 'Steam', detail: 'Native + Flatpak library folders', on: true },
    { name: 'GOG', detail: 'Through Lutris', on: true },
    { name: 'Epic Games', detail: 'Legendary · not signed in', on: false },
    { name: 'Heroic', detail: 'Optional import', on: false },
    { name: 'ROM folders', detail: '~/ROMs/<system>/', on: true },
  ]);
</script>

<div class="settings">
  <h1>Settings</h1>

  <section class="glass-panel">
    <h2>Sources</h2>
    {#each sources as s (s.name)}
      <div class="item">
        <div>
          <strong>{s.name}</strong>
          <span>{s.detail}</span>
        </div>
        <button class="switch" class:on={s.on} data-nav role="switch" aria-checked={s.on} aria-label={s.name} onclick={() => (s.on = !s.on)}><i></i></button>
      </div>
    {/each}
    <button class="btn glass small" data-nav onclick={() => app.notify('Rescanning all sources…')}><Icon name="refresh" size={16} /> Rescan now</button>
  </section>

  <section class="glass-panel">
    <h2>Look and feel</h2>
    <div class="item">
      <div><strong>Mode</strong><span>Big Screen is fullscreen, controller-first, larger everything</span></div>
      <div class="seg" data-nav-row>
        <button data-nav class:on={app.mode === 'desktop'} onclick={() => (app.mode = 'desktop')}><Icon name="monitor" size={16} /> Desktop</button>
        <button data-nav class:on={app.mode === 'big'} onclick={() => (app.mode = 'big')}><Icon name="tv" size={16} /> Big Screen</button>
      </div>
    </div>
    <div class="item">
      <div><strong>Theme</strong><span>Dark is the default</span></div>
      <div class="seg" data-nav-row>
        <button data-nav class:on={app.theme === 'dark'} onclick={() => (app.theme = 'dark')}><Icon name="moon" size={16} /> Dark</button>
        <button data-nav class:on={app.theme === 'light'} onclick={() => (app.theme = 'light')}><Icon name="sun" size={16} /> Light</button>
      </div>
    </div>
    <div class="item">
      <div><strong>Reduce motion</strong><span>Turns off tilt, cross-fades and pulses</span></div>
      <button class="switch" class:on={app.reduceMotion} data-nav role="switch" aria-checked={app.reduceMotion} aria-label="Reduce motion" onclick={() => (app.reduceMotion = !app.reduceMotion)}><i></i></button>
    </div>
    <div class="item">
      <div><strong>UI sounds</strong><span>Move, select and back</span></div>
      <button class="switch" class:on={app.sounds} data-nav role="switch" aria-checked={app.sounds} aria-label="UI sounds" onclick={() => (app.sounds = !app.sounds)}><i></i></button>
    </div>
  </section>

  <section class="glass-panel">
    <h2>Controller</h2>
    <div class="item">
      <div><strong>Last input</strong><span>Arrow keys, Enter and Esc work like a D-pad, A and B</span></div>
      <code class="input"><Icon name="controller" size={18} /> {app.lastInput}</code>
    </div>
  </section>

  <section class="glass-panel">
    <h2>Windows games</h2>
    <div class="item">
      <div><strong>Proton</strong><span>GE-Proton, updated in the background. You never pick Wine versions.</span></div>
      <span class="pill">GE-Proton 10-20</span>
    </div>
    <div class="item">
      <div><strong>Prefixes</strong><span>One per game under ~/Games/&lt;id&gt;/</span></div>
      <span class="pill">7 prefixes</span>
    </div>
  </section>
</div>

<style>
  .settings {
    padding: 36px var(--gutter);
    max-width: 860px;
    display: grid;
    gap: 18px;
  }
  h1 {
    font-size: 32px;
    margin: 0 0 4px;
  }
  section {
    border-radius: 18px;
    padding: 18px 20px;
    display: grid;
    gap: 4px;
    justify-items: start;
  }
  h2 {
    margin: 0 0 8px;
    font-size: 15px;
    color: var(--text-dim);
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }
  .item {
    width: 100%;
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 16px;
    padding: 10px 0;
    border-bottom: 1px solid var(--line);
  }
  .item:last-of-type {
    border-bottom: 0;
  }
  .item > div:first-child {
    display: grid;
    gap: 2px;
  }
  .item span {
    color: var(--text-dim);
    font-size: 13px;
  }
  section > .btn {
    margin-top: 8px;
  }
  .switch {
    flex: none;
    width: 46px;
    height: 26px;
    border-radius: 999px;
    border: 0;
    background: var(--field-strong);
    position: relative;
    cursor: pointer;
    transition: background 200ms;
  }
  .switch i {
    position: absolute;
    top: 3px;
    left: 3px;
    width: 20px;
    height: 20px;
    border-radius: 50%;
    background: #fff;
    box-shadow: 0 2px 6px rgb(0 0 0 / 0.3);
    transition: transform 220ms var(--ease);
  }
  .switch.on {
    background: var(--accent);
  }
  .switch.on i {
    transform: translateX(20px);
  }
  .seg {
    display: flex;
    background: var(--field);
    border-radius: 12px;
    padding: 3px;
  }
  .seg button {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    border: 0;
    background: none;
    color: var(--text-dim);
    font: inherit;
    font-size: 13px;
    font-weight: 600;
    padding: 7px 12px;
    border-radius: 9px;
    cursor: pointer;
  }
  .seg button.on {
    background: var(--surface-strong);
    color: var(--text);
    box-shadow: 0 2px 8px rgb(0 0 0 / 0.2);
  }
  .input {
    display: inline-flex;
    gap: 8px;
    align-items: center;
    font-size: 13px;
    background: var(--field);
    padding: 6px 10px;
    border-radius: 8px;
  }
</style>
