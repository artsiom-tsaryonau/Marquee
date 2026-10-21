<script lang="ts">
  import { onMount } from 'svelte';
  import { app } from '../state.svelte';
  import type { BiosFile, EmulatorInfo } from '../api/contracts';
  import { artFromSeed } from '../art';
  import { coverSrc } from '../api/map';
  import { systemLabel } from '../labels';
  import Artwork from './Artwork.svelte';
  import EmulatorDialog from './EmulatorDialog.svelte';
  import Icon from './Icon.svelte';

  let { query = '', onlyReady = false, platform = '', show = 'all' }: { query?: string; onlyReady?: boolean; platform?: string; show?: 'all' | 'emulators' | 'bios' } = $props();
  /** `<emulator id>:purge` or `bios:<system>` while an irreversible action waits for a second click. */
  let confirm = $state<string | null>(null);

  onMount(() => {
    void app.loadEmulators();
    void app.loadBios();
  });

  const shown = $derived(
    app.emulators.filter(
      (e) =>
        (!onlyReady || e.status !== 'not_installed') &&
        (!platform || e.systems.includes(platform)) &&
        `${e.name} ${e.systems.map(systemLabel).join(' ')}`.toLowerCase().includes(query.toLowerCase()),
    ),
  );
  const systemsWithBios = $derived([...new Set(app.bios.map((b) => b.system))].filter((s) => !platform || s === platform));

  function version(v: string | null) {
    if (!v) return '';
    return v.length > 18 ? `${v.slice(0, 14)}…` : v;
  }
  function byRegion(files: BiosFile[], system: string) {
    return files.filter((b) => b.system === system);
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

{#if show !== 'bios'}
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
      {@const icon = coverSrc(e.icon)}
      <article class="card glass-panel" class:ready={e.status !== 'not_installed'}>
        <button class="tile" data-nav onclick={() => (app.emulatorDialogId = e.id)} aria-label={`${e.name}: settings`}>
          <Artwork art={artFromSeed(`emulator:${e.id}`)} />
          <span class="shade"></span>
          {#if icon}<img class="pic" src={icon} alt="" loading="lazy" />{:else}<span class="pic fallback"><Icon name="controller" size={30} /></span>{/if}
          <span class="badge" class:ok={e.status === 'ready'} class:warn={e.status === 'update'}>
            {e.status === 'not_installed' ? 'Not installed' : e.status === 'update' ? 'Update available' : 'Ready'}
          </span>
        </button>
        <div class="info">
          <h3>{e.name}</h3>
          <p class="systems">{e.systems.map(systemLabel).join(' · ')}</p>
          <p class="detail">{e.integrated ? 'Launches your library games' : 'Opens on its own for now'}{e.installed_version ? ` · ${version(e.installed_version)}` : ''}</p>
          {#if busy}<p class="busy" role="status">{busy}</p>{/if}
          <div class="actions">
            {#if e.status === 'not_installed'}
              <button class="btn primary small" data-nav disabled={!!busy} onclick={() => app.installEmulator(e)}><Icon name="download" size={15} />{busy ? 'Installing…' : 'Install'}</button>
            {:else if e.status === 'update'}
              <button class="btn primary small" data-nav disabled={!!busy} onclick={() => app.installEmulator(e)}>Update</button>
            {:else}
              <button class="btn primary small" data-nav disabled={!!busy} onclick={() => app.openEmulator(e)}><Icon name="play" size={15} />Open</button>
            {/if}
            <button class="btn glass small" data-nav onclick={() => (app.emulatorDialogId = e.id)}><Icon name="settings" size={15} />Configure</button>
          </div>
        </div>
      </article>
    {:else}
      <div class="empty glass-panel"><p>No emulators match these filters.</p></div>
    {/each}
  </div>
</section>

{/if}
<EmulatorDialog />

{#if show !== 'emulators'}
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
        <article class="card bios-card glass-panel">
          <div class="head">
            <span class="mark"><Icon name="monitor" size={22} /></span>
            <div class="title"><h3>{systemLabel(system)}</h3><span class="badge ok">{byRegion(app.bios, system).length} file{byRegion(app.bios, system).length === 1 ? '' : 's'}</span></div>
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
{/if}

<style>
  .managed { display: grid; gap: 0; }
  .section-heading { display: flex; align-items: center; justify-content: space-between; gap: 16px; margin-bottom: 18px; }
  .section-heading h2 { margin: 0; font-size: 21px; letter-spacing: -.4px; }
  .section-heading p, p { color: var(--text-soft); margin: 5px 0; line-height: 1.6; font-size: 13px; }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(290px, 1fr)); gap: 16px; }
  .card { border-radius: 18px; display: flex; flex-direction: column; min-width: 0; overflow: hidden; transition: border-color .2s, transform .2s; }
  .card:hover { border-color: rgb(139 92 246 / .45); }
  .card:not(.ready) .tile { filter: saturate(.55); }
  .bios-card { padding: 22px; }
  .tile { position: relative; display: block; width: 100%; height: 150px; padding: 0; border: 0; overflow: hidden; cursor: pointer; background: none; color: inherit; }
  .tile :global(svg) { position: absolute; inset: 0; width: 100%; height: 100%; }
  .shade { position: absolute; inset: 0; background: linear-gradient(to top, rgb(8 8 14 / .7), transparent 65%); }
  .pic { position: absolute; left: 50%; top: 50%; width: 76px; height: 76px; transform: translate(-50%, -52%); object-fit: contain; filter: drop-shadow(0 8px 16px rgb(0 0 0 / .5)); transition: transform .25s; }
  .tile:hover .pic { transform: translate(-50%, -52%) scale(1.08); }
  .pic.fallback { display: grid; place-items: center; border-radius: 20px; background: rgb(0 0 0 / .3); color: #fff; }
  .tile .badge { position: absolute; left: 12px; bottom: 10px; }
  .info { padding: 16px 18px 18px; display: flex; flex-direction: column; flex: 1; }
  .head { display: flex; gap: 14px; align-items: center; margin-bottom: 14px; }
  .title { flex: 1; min-width: 0; }
  h3 { margin: 0 0 4px; font-size: 17px; }
  .mark { display: grid; place-items: center; width: 48px; height: 48px; flex-shrink: 0; border-radius: 14px; background: var(--field); color: var(--text-soft); }
  .badge { display: inline-block; padding: 4px 8px; border-radius: 6px; font-size: 11px; color: var(--text-soft); background: rgb(0 0 0 / .45); backdrop-filter: blur(4px); }
  .badge.ok { color: var(--ok); }
  .badge.warn { color: var(--warn); }
  .systems { font-size: 13px; }
  .detail { font-size: 12px; color: var(--text-dim); }
  .busy { font-size: 12px; color: var(--accent); }
  .actions { display: flex; flex-wrap: wrap; gap: 8px; align-items: center; margin-top: auto; padding-top: 14px; }
  .text-button { border: 0; background: none; color: var(--text-dim); padding: 8px; cursor: pointer; font: inherit; font-size: 12px; }
  .text-button.danger:hover, .text-button.danger:focus-visible { color: var(--bad); }
  .bios-list { list-style: none; margin: 0 0 8px; padding: 0; display: grid; gap: 6px; }
  .bios-list li { display: grid; grid-template-columns: 64px 1fr auto; gap: 10px; align-items: center; font-size: 12px; }
  .region { color: var(--accent); font-weight: 600; }
  .file { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--text-soft); }
  .ver { color: var(--text-dim); }
  .empty { grid-column: 1 / -1; text-align: center; padding: 36px; border-radius: 18px; color: var(--text-dim); display: grid; gap: 12px; justify-items: center; }
  .empty h3, .empty p { margin: 0; }
</style>
