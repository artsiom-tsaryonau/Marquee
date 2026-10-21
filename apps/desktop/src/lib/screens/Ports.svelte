<script lang="ts">
  import { app } from '../state.svelte';
  import { platformLabel } from '../labels';
  import type { PortEntry, PortStatus } from '../types';
  import Artwork from '../components/Artwork.svelte';
  import Logo from '../components/Logo.svelte';
  import Icon from '../components/Icon.svelte';
  import { portsAtmosphere } from '../art';

  const statusLabel: Record<PortStatus, string> = {
    ready: 'Ready',
    update: 'Update available',
    'needs-rom': 'Needs your ROM',
    'needs-data': 'Needs game data',
    'not-installed': 'Not installed',
  };

  const shortLabel: Record<PortStatus, string> = {
    ready: 'Ready',
    update: 'Update',
    'needs-rom': 'Needs ROM',
    'needs-data': 'Needs data',
    'not-installed': 'Get',
  };

  let filter = $state<'all' | PortStatus>('all');
  const shown = $derived(app.ports.filter((p) => filter === 'all' || p.status === filter));
  const featured = $derived(app.ports.find((p) => p.status === 'update') ?? app.ports[0]);

  function primary(p: PortEntry) {
    if (p.id in app.portBusy) return;
    if (p.status === 'ready') void app.launchPort(p);
    else void app.installPort(p);
  }

  const action = (p: PortEntry) =>
    p.id in app.portBusy
      ? 'Installing…'
      : ({ ready: 'Play', update: 'Update', 'needs-rom': 'Choose ROM', 'needs-data': 'Find data', 'not-installed': 'Install' })[p.status];
  const filters = ['all', 'ready', 'update', 'not-installed'] as const;
</script>

<div class="ports">
  {#if featured}
  <section class="feature" style="--accent-h:{featured.art.hue2}">
    <div class="feature-art"><Artwork art={featured.art} /></div>
    <div class="feature-fade"></div>
    <div class="feature-text">
      <span class="eyebrow">Featured port · {featured.family}</span>
      <Logo game={{ title: featured.name, art: featured.art }} size="lg" />
      <p>Native {platformLabel[featured.platform]}-era classic rebuilt for modern PCs: {featured.game}, widescreen, high frame rate, mods.</p>
      <button class="btn primary" data-nav onclick={() => primary(featured)}>
        <Icon name={featured.status === 'ready' ? 'play' : 'download'} />
        {action(featured)}
      </button>
    </div>
  </section>
  {:else}
  <section class="feature" style="--accent-h:{portsAtmosphere.hue2}">
    <div class="feature-art"><Artwork art={portsAtmosphere} /></div>
    <div class="feature-fade"></div>
    <div class="feature-text">
      <span class="eyebrow">Native ports</span>
      <Logo game={{ title: 'Ports', art: portsAtmosphere }} size="lg" />
      <p>No ports could be loaded. PS2 games stay in Library. You supply your own files.</p>
      <button class="btn primary" data-nav onclick={() => app.go('library')}>
        <Icon name="grid" /> Open library
      </button>
    </div>
  </section>
  {/if}

  <div class="filters" data-nav-row>
    {#each filters as f (f)}
      <button class="chip" class:on={filter === f} data-nav onclick={() => (filter = f)}>{f === 'all' ? 'All ports' : statusLabel[f]}</button>
    {/each}
  </div>

  <div class="grid">
    {#each shown as p (p.id)}
      <article class="port glass-panel" style="--accent-h:{p.art.hue2}">
        <div class="thumb"><Artwork art={p.art} /><Logo game={{ title: p.name, art: p.art }} size="sm" /></div>
        <div class="text">
          <div class="top">
            <h3>{p.name}</h3>
            <span class="st {p.status}" title={statusLabel[p.status]}>{shortLabel[p.status]}</span>
          </div>
          <p class="game">{p.game} · {platformLabel[p.platform]}</p>
          <p class="req">
            <Icon name={p.status === 'needs-rom' || p.status === 'needs-data' ? 'alert' : 'check'} size={15} />
            {p.requirement}
          </p>
          <div class="foot">
            <code>{app.portBusy[p.id] ?? p.upstream}</code>
            {#if p.status !== 'not-installed'}
              <button class="btn small" data-nav onclick={() => app.openPortFolder(p)} title="Open the port folder to add your own ROM or game files">Open folder</button>
            {/if}
            <button class="btn small" disabled={p.id in app.portBusy} class:primary={p.status !== 'ready'} data-nav onclick={() => primary(p)}>{action(p)}</button>
          </div>
        </div>
      </article>
    {/each}
  </div>
  <p class="note">The catalogue links only to official port releases on GitHub. You supply your own game files; updates keep them.</p>
</div>

<style>
  .ports {
    padding: 28px var(--gutter) 40px;
  }
  .feature {
    position: relative;
    border-radius: 24px;
    overflow: hidden;
    min-height: 340px;
    display: flex;
    align-items: flex-end;
    box-shadow: 0 30px 80px -30px hsl(var(--accent-h) 80% 40% / 0.6);
  }
  .feature-art {
    position: absolute;
    inset: 0;
  }
  .feature-fade {
    position: absolute;
    inset: 0;
    background: linear-gradient(90deg, rgb(0 0 0 / 0.75), transparent 70%);
  }
  .feature-text {
    position: relative;
    padding: 32px;
    max-width: 560px;
    display: grid;
    gap: 12px;
    justify-items: start;
    color: #fff;
  }
  .feature-text p {
    margin: 0;
    color: rgb(255 255 255 / 0.8);
  }
  .eyebrow {
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: hsl(var(--accent-h) 90% 80%);
  }
  .filters {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin: 26px 0 18px;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(340px, 1fr));
    gap: 16px;
  }
  .port {
    display: grid;
    grid-template-columns: 120px 1fr;
    border-radius: 18px;
    overflow: hidden;
    transition: transform 200ms var(--ease), box-shadow 200ms;
  }
  .port:focus-within,
  .port:hover {
    transform: translateY(-3px);
    box-shadow: 0 0 0 1.5px hsl(var(--accent-h) 90% 70% / 0.8), 0 16px 40px -14px hsl(var(--accent-h) 90% 50% / 0.6);
  }
  .thumb {
    position: relative;
    min-height: 160px;
  }
  .thumb :global(.logo) {
    position: absolute;
    left: 10px;
    right: 10px;
    bottom: 10px;
  }
  .text {
    padding: 14px 16px;
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
  }
  .top {
    display: flex;
    justify-content: space-between;
    gap: 8px;
    align-items: start;
  }
  h3 {
    margin: 0;
    min-width: 0;
    font-size: 16px;
    overflow-wrap: anywhere;
  }
  .st {
    flex: none;
    font-size: 10.5px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    padding: 3px 8px;
    border-radius: 999px;
    background: var(--field);
    color: var(--text-dim);
  }
  .st.ready {
    background: rgb(52 211 153 / 0.18);
    color: var(--ok);
  }
  .st.update {
    background: rgb(251 191 36 / 0.18);
    color: var(--warn);
  }
  .st.needs-rom,
  .st.needs-data {
    background: rgb(248 113 113 / 0.16);
    color: var(--bad);
  }
  .game {
    margin: 0;
    color: var(--text-dim);
    font-size: 13px;
  }
  .req {
    margin: 0;
    display: flex;
    gap: 6px;
    align-items: center;
    font-size: 13px;
    color: var(--text-soft);
  }
  .foot {
    margin-top: auto;
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 8px;
  }
  .foot .btn {
    flex: none;
    white-space: nowrap;
  }
  code {
    font-size: 11.5px;
    color: var(--text-dim);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .note {
    margin-top: 24px;
    color: var(--text-dim);
    font-size: 13px;
  }
</style>
