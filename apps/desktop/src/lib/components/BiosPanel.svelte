<script lang="ts">
  import { onMount } from 'svelte';
  import { app } from '../state.svelte';
  import { systemLabel } from '../labels';
  import Icon from './Icon.svelte';

  let { platform = '' }: { platform?: string } = $props();
  /** `bios:<system>` while an irreversible removal waits for a second click. */
  let confirm = $state<string | null>(null);

  onMount(() => void app.loadBios());

  const systems = $derived([...new Set(app.bios.map((b) => b.system))].filter((s) => !platform || s === platform));

  async function drop(system: string) {
    if (confirm !== `bios:${system}`) {
      confirm = `bios:${system}`;
      return;
    }
    confirm = null;
    await app.removeBios(system);
  }
</script>

<section aria-labelledby="bios-title">
  <div class="heading">
    <div>
      <h2 id="bios-title">BIOS files</h2>
      <p>Firmware should be fetched automatically when a source is configured. Until then, import a folder — Marquee detects regions and stores copies locally.</p>
    </div>
    <button class="btn primary small" data-nav disabled={app.biosBusy} onclick={() => app.importBios()}><Icon name="folder" size={15} />{app.biosBusy ? 'Working…' : 'Import folder (fallback)…'}</button>
  </div>
  {#if systems.length}
    <div class="grid">
      {#each systems as system}
        {@const files = app.bios.filter((b) => b.system === system)}
        <article class="card glass-panel">
          <div class="head">
            <span class="mark"><Icon name="monitor" size={22} /></span>
            <div class="title"><h3>{systemLabel(system)}</h3><span class="badge">{files.length} file{files.length === 1 ? '' : 's'}</span></div>
          </div>
          <ul>
            {#each files as b (b.file)}
              <li><span class="region">{b.region}</span><span class="file" title={b.file}>{b.file}</span><span class="ver">v{b.version}</span></li>
            {/each}
          </ul>
          <p class="detail">{system === 'ps2' ? 'For each game, Marquee picks the newest BIOS of its region.' : 'DuckStation picks the one matching each disc.'}</p>
          <button class="text-button" data-nav disabled={app.biosBusy} onclick={() => drop(system)} onblur={() => (confirm = null)}>
            {confirm === `bios:${system}` ? 'Click again to delete these files' : `Remove ${system.toUpperCase()} BIOS…`}
          </button>
        </article>
      {/each}
    </div>
  {:else if !platform}
    <div class="empty glass-panel">
      <Icon name="folder" size={28} />
      <h3>No BIOS files yet</h3>
      <p>Fallback: pick a folder or archive. Marquee copies only recognised firmware files and ignores the rest.</p>
    </div>
  {:else}
    <p class="detail">No BIOS files for this platform.</p>
  {/if}
</section>

<style>
  .heading { display: flex; align-items: center; justify-content: space-between; gap: 16px; margin-bottom: 18px; }
  h2 { margin: 0; font-size: 21px; letter-spacing: -.4px; }
  p { color: var(--text-soft); margin: 5px 0; line-height: 1.6; font-size: 13px; }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(290px, 1fr)); gap: 16px; }
  .card { padding: 22px; border-radius: 18px; display: flex; flex-direction: column; min-width: 0; }
  .head { display: flex; gap: 14px; align-items: center; margin-bottom: 14px; }
  .title { flex: 1; min-width: 0; }
  h3 { margin: 0 0 4px; font-size: 17px; }
  .mark { display: grid; place-items: center; width: 48px; height: 48px; flex-shrink: 0; border-radius: 14px; background: var(--field); color: var(--text-soft); }
  .badge { display: inline-block; padding: 4px 8px; border-radius: 6px; font-size: 11px; color: var(--ok); background: rgb(52 211 153 / .08); }
  .detail { font-size: 12px; color: var(--text-dim); }
  ul { list-style: none; margin: 0 0 8px; padding: 0; display: grid; gap: 6px; }
  li { display: grid; grid-template-columns: 64px 1fr auto; gap: 10px; align-items: center; font-size: 12px; }
  .region { color: var(--accent); font-weight: 600; }
  .file { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--text-soft); }
  .ver { color: var(--text-dim); }
  .text-button { align-self: flex-start; margin-top: auto; border: 0; background: none; color: var(--text-dim); padding: 8px 0; cursor: pointer; font: inherit; font-size: 12px; }
  .text-button:hover, .text-button:focus-visible { color: var(--bad); }
  .empty { text-align: center; padding: 36px; border-radius: 18px; color: var(--text-dim); display: grid; gap: 12px; justify-items: center; }
  .empty h3, .empty p { margin: 0; }
</style>
