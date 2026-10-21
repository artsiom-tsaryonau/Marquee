<script lang="ts">
  import { onMount } from 'svelte';
  import { app } from '../state.svelte';
  import type { KeyInfo } from '../api/contracts';
  import Icon from '../components/Icon.svelte';
  import InstallMenu from '../components/InstallMenu.svelte';

  const sources = $state([
    { name: 'Lutris', detail: '~/.local/share/lutris/pga.db · read-only', on: true },
    { name: 'Steam', detail: 'Native + Flatpak library folders', on: true },
    { name: 'GOG', detail: 'Through Lutris', on: true },
    { name: 'Epic Games', detail: 'Legendary · not signed in', on: false },
    { name: 'Heroic', detail: 'Optional import', on: false },
    { name: 'ROM folders', detail: '~/ROMs/<system>/', on: true },
  ]);

  let drafts = $state<Record<string, string>>({});
  onMount(() => void app.loadKeys());

  const sourceLabel = (k: KeyInfo) =>
    ({
      app: `Saved in Marquee · ${k.preview}`,
      env: `From the environment · ${k.preview} · saving here overrides it`,
      dot_env: `From a .env file · ${k.preview} · saving here overrides it`,
      none: 'Not set',
    })[k.source];

  async function save(k: KeyInfo) {
    const value = drafts[k.id]?.trim();
    if (!value) return;
    await app.saveKey(k.id, value);
    drafts[k.id] = '';
  }
</script>

<div class="settings">
  <h1>Settings</h1>

  {#each [{ id: 'ps1', name: 'PlayStation', emulator: 'DuckStation', kind: 'duckstation' }, { id: 'ps2', name: 'PlayStation 2', emulator: 'PCSX2', kind: 'pcsx2' }] as system}
    {@const runner = app.runnerFor(system.id)}
    {@const choices = app.runners.filter((r) => r.kind === system.kind)}
    <section class="glass-panel">
      <h2>{system.name}</h2>
      <div class="item"><div><strong>Disc folders</strong>
        {#each app.roots.filter((r) => r.system === system.id) as root}<span>{root.path} · {root.status}</span>{:else}<span>No folder selected</span>{/each}
        <span>{app.games.filter((g) => g.source === 'rom' && g.platform === system.id).length} disc/image entries · {system.id === 'ps1' ? 'ISO, CHD, CUE with its tracks' : 'ISO'}</span>
      </div><button class="btn glass small" data-nav onclick={() => app.addSystemFolder(system.id)}><Icon name="folder" size={16} /> Add folder</button></div>
      <div class="item"><div><strong>{system.emulator}</strong><span>{runner ? `${runner.display_name} · ${runner.bios_state === 'launch_tested' ? 'previously launched' : runner.bios_state}` : 'No available emulator detected'}</span>
        {#if runner?.bios_hint}<span>{runner.bios_hint}</span>{/if}
        {#if system.id === 'ps2' && app.installingPcsx2}<span>{app.installProgress || 'Installing…'}</span>{/if}
        {#if !runner && system.id === 'ps1'}<span>Install DuckStation, then refresh detection. Native commands, AppImages and Flatpak are supported.</span>{/if}
      </div><div class="row-actions">
        {#if system.id === 'ps2'}<InstallMenu tone="primary" size="small" />{:else if !runner}<a class="btn glass small" data-nav href="https://github.com/stenzek/duckstation" target="_blank" rel="noreferrer">Get DuckStation ↗</a>{/if}
        <button class="btn glass small" data-nav onclick={() => app.openSetup(runner?.id, system.id)} disabled={!runner}>Open setup</button>
      </div></div>
      {#if choices.length > 1}<div class="item"><div><strong>Default runner</strong><span>Saved independently for {system.name}</span></div><select data-nav aria-label={`${system.name} default runner`} value={runner?.id || ''} onchange={(e) => app.chooseRunner(e.currentTarget.value)}>{#each choices as choice}<option value={choice.id} disabled={!choice.available}>{choice.display_name}{choice.available ? '' : ' · unavailable'}</option>{/each}</select></div>{/if}
    </section>
  {/each}
  <section class="glass-panel">
    <h2>Disc library maintenance</h2>
    <div class="item"><div><strong>Last import</strong><span>{app.lastScan ? `${app.lastScan.added} added · ${app.lastScan.skipped_unsupported} companion or unsupported files` : 'Not scanned in this window'}</span></div><button class="btn glass small" data-nav onclick={() => app.rescan()} disabled={app.scanning}>{app.scanning ? 'Scanning…' : 'Rescan folders'}</button></div>
    <div class="item"><div><strong>Emulator detection</strong><span>Refresh after installing or moving an emulator.</span></div><button class="btn glass small" data-nav onclick={() => app.refreshRunners()}>Refresh emulators</button></div>
    <div class="item"><div><strong>Box art</strong><span>Looks up PS1 and PS2 covers by disc serial. Missing artwork never blocks Play.</span></div><button class="btn glass small" data-nav onclick={() => app.refreshArtwork()} disabled={app.fetchingArt || !app.games.length}>{app.fetchingArt ? 'Fetching…' : 'Fetch covers'}</button></div>
  </section>

  <section class="glass-panel">
    <h2>API keys</h2>
    <p class="hint">
      Saved on this computer in a file only you can read. After you save a secret, Marquee shows only where it comes from and its last four characters, never the value. A value saved here beats the environment and any .env file.
    </p>
    {#each app.keys as k (k.id)}
      <div class="item key">
        <div>
          <strong>{k.label} <span class="pill">{k.used_by}</span></strong>
          <span>{k.description}</span>
          <span>{k.help}</span>
          <span class="status" class:set={k.set}>{sourceLabel(k)}</span>
        </div>
        <form class="key-edit" onsubmit={(e) => { e.preventDefault(); void save(k); }}>
          <input
            data-nav
            type={k.secret ? 'password' : 'text'}
            autocomplete="off"
            spellcheck="false"
            placeholder={k.set ? 'Replace value…' : 'Paste value'}
            aria-label={k.label}
            bind:value={drafts[k.id]}
          />
          <button class="btn small primary" data-nav type="submit" disabled={!drafts[k.id]?.trim()}>Save</button>
          {#if k.source === 'app'}
            <button class="btn glass small" data-nav type="button" onclick={() => app.clearKey(k.id)}>Clear</button>
          {/if}
        </form>
      </div>
    {/each}
  </section>

  {#if app.webUrl}
  <section class="glass-panel">
    <h2>Web access</h2>
    <div class="item">
      <div>
        <strong>Open this library in a browser</strong>
        <span>Same games, same data, same launches. Only this computer can connect, and only with the key in the address.</span>
      </div>
      <div class="row-actions">
        <button class="btn glass small" data-nav onclick={() => app.copyWebUrl()}>Copy address</button>
      </div>
    </div>
  </section>
  {/if}

  {#if app.demo}
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
    <button class="btn glass small" data-nav onclick={() => app.rescan()}><Icon name="refresh" size={16} /> Rescan now</button>
  </section>
  {/if}

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
      <div><strong>Visual effects</strong><span>Balanced reduces GPU work. Full enables backdrop blur, grain and pointer tilt.</span></div>
      <div class="seg" data-nav-row>
        <button data-nav class:on={app.visualEffects === 'balanced'} onclick={() => (app.visualEffects = 'balanced')}>Balanced</button>
        <button data-nav class:on={app.visualEffects === 'full'} onclick={() => (app.visualEffects = 'full')}>Full</button>
      </div>
    </div>
    <div class="item">
      <div><strong>Reduce motion</strong><span>Turns off tilt and cross-fades</span></div>
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

  {#if app.demo}
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
  {/if}
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
  .row-actions {
    display: flex;
    gap: 8px;
    flex: none;
  }
  .item > div:first-child {
    display: grid;
    gap: 2px;
  }
  .item span {
    color: var(--text-dim);
    font-size: 13px;
  }
  .hint {
    margin: 0 0 6px;
    color: var(--text-dim);
    font-size: 13px;
    max-width: 62ch;
  }
  .key {
    flex-wrap: wrap;
    align-items: flex-start;
  }
  .key > div:first-child {
    flex: 1 1 320px;
    min-width: 0;
  }
  .key .status {
    font-weight: 600;
  }
  .key .status.set {
    color: var(--ok);
  }
  .key-edit {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
    align-items: center;
  }
  .key-edit input {
    width: 230px;
    padding: 8px 12px;
    border-radius: 10px;
    border: 1px solid var(--line);
    background: var(--field);
    color: var(--text);
    font: inherit;
    font-size: 13px;
  }
  .key-edit input:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
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
