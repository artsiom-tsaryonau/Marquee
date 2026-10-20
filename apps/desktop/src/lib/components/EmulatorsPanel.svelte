<script lang="ts">
  import { onMount } from 'svelte';
  import { app } from '../state.svelte';
  import type { BiosFile, EmulatorInfo } from '../api/contracts';
  import Icon from './Icon.svelte';

  let { query = '', onlyReady = false }: { query?: string; onlyReady?: boolean } = $props();
  /** `<emulator id>:purge` or `bios:<system>` while an irreversible action waits for a second click. */
  let confirm = $state<string | null>(null);
  let copied = $state<string | null>(null);

  onMount(() => {
    void app.loadEmulators();
    void app.loadBios();
  });

  const shown = $derived(
    app.emulators.filter(
      (e) => (!onlyReady || e.status !== 'not_installed') && `${e.name} ${e.systems.join(' ')}`.toLowerCase().includes(query.toLowerCase()),
    ),
  );
  const systemsWithBios = $derived([...new Set(app.bios.map((b) => b.system))]);
  const label: Record<string, string> = { ps1: 'PlayStation', ps2: 'PlayStation 2', ps3: 'PlayStation 3', psp: 'PSP', dreamcast: 'Dreamcast', wiiu: 'Wii U', xbox: 'Xbox', '3ds': 'Nintendo 3DS' };

  function version(v: string | null) {
    if (!v) return '';
    return v.length > 18 ? `${v.slice(0, 14)}…` : v;
  }
  function byRegion(files: BiosFile[], system: string) {
    return files.filter((b) => b.system === system);
  }
  async function copy(text: string) {
    try {
      await navigator.clipboard.writeText(text);
      copied = text;
      setTimeout(() => (copied = copied === text ? null : copied), 1800);
    } catch {
      app.notify(text);
    }
  }
  async function purge(e: EmulatorInfo) {
    if (confirm !== `${e.id}:purge`) {
      confirm = `${e.id}:purge`;
      return;
    }
    confirm = null;
    await app.uninstallEmulator(e, true);
  }
  async function dropBios(system: string) {
    if (confirm !== `bios:${system}`) {
      confirm = `bios:${system}`;
      return;
    }
    confirm = null;
    await app.removeBios(system);
  }
</script>

<section aria-labelledby="managed-title" class="managed">
  <div class="section-heading">
    <div>
      <h2 id="managed-title">Emulators Marquee manages</h2>
      <p>Installed into Marquee's own folder from the official releases, checksum-verified. Removing one never touches your saves unless you ask.</p>
    </div>
    <span class="pill">{shown.length}</span>
  </div>
  <div class="grid">
    {#each shown as e (e.id)}
      {@const busy = app.emulatorBusy[e.id]}
      <article class="card glass-panel" class:ready={e.status !== 'not_installed'}>
        <div class="head">
          <span class="mark"><Icon name="controller" size={22} /></span>
          <div class="title">
            <h3>{e.name}</h3>
            <span class="badge" class:ok={e.status === 'ready'} class:warn={e.status === 'update'}>
              {e.status === 'not_installed' ? 'Not installed' : e.status === 'update' ? 'Update available' : 'Ready'}
            </span>
          </div>
        </div>
        <p class="systems">{e.systems.map((s) => label[s] ?? s.toUpperCase()).join(' · ')}{e.integrated ? ' · launches your library games' : ' · opens on its own'}</p>
        <p class="detail">{e.requirement}</p>
        {#if e.installed_version}
          <p class="detail">Version {version(e.installed_version)}{e.status === 'update' && e.latest_version ? ` → ${version(e.latest_version)}` : ''}</p>
        {/if}
        {#if busy}<p class="busy" role="status">{busy}</p>{/if}
        <div class="actions">
          {#if e.status === 'not_installed'}
            <button class="btn primary small" data-nav disabled={!!busy} onclick={() => app.installEmulator(e)}><Icon name="download" size={15} />{busy ? 'Installing…' : 'Install'}</button>
          {:else}
            <button class="btn primary small" data-nav disabled={!!busy} onclick={() => app.openEmulator(e)}><Icon name="play" size={15} />Open</button>
            {#if e.status === 'update'}<button class="btn glass small" data-nav disabled={!!busy} onclick={() => app.installEmulator(e)}>Update</button>{/if}
            <button class="btn glass small" data-nav disabled={!!busy} onclick={() => app.uninstallEmulator(e, false)}>Remove</button>
            <button class="text-button danger" data-nav disabled={!!busy} onclick={() => purge(e)} onblur={() => (confirm = null)}>
              {confirm === `${e.id}:purge` ? 'Click again: delete saves too' : 'Remove with saves…'}
            </button>
          {/if}
        </div>
        {#if e.status !== 'not_installed'}
          <p class="path" title={e.data_dir}>Saves and settings: {e.data_dir}</p>
        {/if}
        {#each e.alternatives as alt}
          <details class="alt">
            <summary>Advanced: use {alt.manager} instead</summary>
            <p>{alt.note}</p>
            <div class="cmd">
              <code>{alt.command}</code>
              <button class="btn glass small" data-nav onclick={() => copy(alt.command)}>{copied === alt.command ? 'Copied' : 'Copy'}</button>
            </div>
            <p class="detail">Marquee never runs this for you.</p>
          </details>
        {/each}
      </article>
    {:else}
      <div class="empty glass-panel"><p>No emulators match these filters.</p></div>
    {/each}
  </div>
</section>

<section aria-labelledby="bios-title" class="managed">
  <div class="section-heading">
    <div>
      <h2 id="bios-title">BIOS files</h2>
      <p>You supply these; Marquee never downloads them. It reads the region from each file and keeps them in its own folder.</p>
    </div>
    <button class="btn primary small" data-nav disabled={app.biosBusy} onclick={() => app.importBios()}><Icon name="folder" size={15} />{app.biosBusy ? 'Working…' : 'Import a folder…'}</button>
  </div>
  {#if app.bios.length}
    <div class="grid">
      {#each systemsWithBios as system}
        <article class="card glass-panel">
          <div class="head">
            <span class="mark"><Icon name="monitor" size={22} /></span>
            <div class="title"><h3>{label[system] ?? system.toUpperCase()}</h3><span class="badge ok">{byRegion(app.bios, system).length} file{byRegion(app.bios, system).length === 1 ? '' : 's'}</span></div>
          </div>
          <ul class="bios-list">
            {#each byRegion(app.bios, system) as b (b.file)}
              <li><span class="region">{b.region}</span><span class="file" title={b.file}>{b.file}</span><span class="ver">v{b.version}</span></li>
            {/each}
          </ul>
          <p class="detail">{system === 'ps2' ? 'For each game, Marquee picks the newest BIOS of its region.' : 'DuckStation picks the one matching each disc.'}</p>
          <div class="actions">
            <button class="text-button danger" data-nav disabled={app.biosBusy} onclick={() => dropBios(system)} onblur={() => (confirm = null)}>
              {confirm === `bios:${system}` ? 'Click again to delete these files' : `Remove ${system.toUpperCase()} BIOS…`}
            </button>
          </div>
        </article>
      {/each}
    </div>
  {:else}
    <div class="empty glass-panel">
      <Icon name="folder" size={28} />
      <h3>No BIOS files yet</h3>
      <p>Import a folder or archive you extracted from your own console dumps. Marquee copies only the files it recognises and ignores the rest.</p>
    </div>
  {/if}
</section>

<style>
  .managed { display: grid; gap: 0; }
  .section-heading { display: flex; align-items: center; justify-content: space-between; gap: 16px; margin-bottom: 18px; }
  .section-heading h2 { margin: 0; font-size: 21px; letter-spacing: -.4px; }
  .section-heading p, p { color: var(--text-soft); margin: 5px 0; line-height: 1.6; font-size: 13px; }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(290px, 1fr)); gap: 16px; }
  .card { padding: 22px; border-radius: 18px; display: flex; flex-direction: column; min-width: 0; transition: border-color .2s; }
  .card:hover { border-color: rgb(139 92 246 / .45); }
  .head { display: flex; gap: 14px; align-items: center; margin-bottom: 14px; }
  .title { flex: 1; min-width: 0; }
  h3 { margin: 0 0 6px; font-size: 17px; }
  .mark { display: grid; place-items: center; width: 48px; height: 48px; flex-shrink: 0; border-radius: 14px; background: var(--field); color: var(--text-soft); }
  .badge { display: inline-block; padding: 4px 8px; border-radius: 6px; font-size: 11px; color: var(--text-dim); background: var(--field); }
  .badge.ok { color: var(--ok); background: rgb(52 211 153 / .08); }
  .badge.warn { color: var(--warn); background: rgb(251 191 36 / .1); }
  .systems { font-size: 13px; }
  .detail { font-size: 12px; color: var(--text-dim); }
  .busy { font-size: 12px; color: var(--accent); }
  .path { font: 11px ui-monospace, monospace; color: var(--text-dim); overflow-wrap: anywhere; margin: 12px 0 0; }
  .actions { display: flex; flex-wrap: wrap; gap: 8px; align-items: center; margin-top: auto; padding-top: 16px; }
  .text-button { border: 0; background: none; color: var(--text-dim); padding: 8px; cursor: pointer; font: inherit; font-size: 12px; }
  .text-button.danger:hover, .text-button.danger:focus-visible { color: var(--bad); }
  .alt { margin-top: 14px; border-top: 1px solid var(--line); padding-top: 10px; font-size: 12px; color: var(--text-dim); }
  .alt summary { cursor: pointer; }
  .cmd { display: flex; gap: 8px; align-items: center; margin: 8px 0; }
  code { flex: 1; min-width: 0; overflow-wrap: anywhere; font: 11px ui-monospace, monospace; background: var(--field); padding: 8px 10px; border-radius: 8px; color: var(--text-soft); }
  .bios-list { list-style: none; margin: 0 0 8px; padding: 0; display: grid; gap: 6px; }
  .bios-list li { display: grid; grid-template-columns: 64px 1fr auto; gap: 10px; align-items: center; font-size: 12px; }
  .region { color: var(--accent); font-weight: 600; }
  .file { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--text-soft); }
  .ver { color: var(--text-dim); }
  .empty { grid-column: 1 / -1; text-align: center; padding: 36px; border-radius: 18px; color: var(--text-dim); display: grid; gap: 12px; justify-items: center; }
  .empty h3, .empty p { margin: 0; }
</style>
