<script lang="ts">
  import { app } from '../state.svelte';
  import { platformLabel } from '../labels';
  import { portAction, portStatusLabel } from '../presentation';
  import Artwork from '../components/Artwork.svelte';
  import Icon from '../components/Icon.svelte';

  let filter = $state<'all' | 'setup' | 'installed' | 'update' | 'not-installed'>('all');
  let refreshing = $state(false);
  const filters = [ ['all', 'All ports'], ['installed', 'Installed'], ['setup', 'Needs setup'], ['update', 'Updates'], ['not-installed', 'Not installed'] ] as const;
  const shown = $derived(app.ports.filter((p) => filter === 'all' || (filter === 'setup' ? p.status === 'installed' || p.status === 'needs-rom' || p.status === 'needs-data' : filter === 'installed' ? p.status !== 'not-installed' : p.status === filter)));
  async function refresh() {
    refreshing = true;
    try { await app.loadPorts(); } finally { refreshing = false; }
  }
</script>

<div class="ports">
  <header class="heading">
    <div><span class="eyebrow">Discover ways to play</span><h1>Native ports</h1>
      <p>Set up native versions of classic games. Installed ports appear in your Library.</p></div>
    <button class="btn glass" data-nav disabled={refreshing} onclick={refresh}><Icon name="refresh" />{refreshing ? 'Checking…' : 'Refresh status'}</button>
  </header>
  <div class="intro glass-panel">
    <Icon name="info" size={24} />
    <p>Install a port, add the compatible game files it requires, then play from your Library. Installing a port does not include the original game.</p>
  </div>
  <div class="filters" data-nav-row aria-label="Port availability">
    {#each filters as [value, label]}
      <button class="chip" class:on={filter === value} aria-pressed={filter === value} data-nav onclick={() => (filter = value)}>{label}</button>
    {/each}
  </div>
  <p class="count" aria-live="polite">{shown.length} {shown.length === 1 ? 'port' : 'ports'}</p>
  <div class="grid">
    {#each shown as p (p.id)}
      {@const needsFiles = p.status === 'needs-rom' || p.status === 'needs-data'}
      <article class="port glass-panel">
        <div class="thumb"><Artwork art={p.art} /><span>{platformLabel[p.platform]}</span></div>
        <div class="text">
          <span class="status" class:ready={p.status === 'ready'} class:needs-files={needsFiles}>{portStatusLabel(p)}</span>
          <h2>{p.name}</h2><p class="game">{p.game}</p>
          <p class="requirement">{p.requirement}</p>
          {#if p.status === 'installed' || p.status === 'update'}<p class="setup">The port is installed. Marquee has not verified its game files; open it to finish setup.</p>{/if}
          {#if needsFiles}<p class="setup">Add the required files to the port folder, then open the port to finish setup. Some ports finish setup in their own window.</p>{/if}
          {#if app.portErrors[p.id]}<p class="error" role="alert">{app.portErrors[p.id]}</p>{/if}
          {#if app.portBusy[p.id]}<p class="progress" role="status">{app.portBusy[p.id]}</p>{/if}
          <div class="actions">
            <button class="btn primary small" data-nav disabled={p.id in app.portBusy} onclick={() => app.runPortAction(p)}><Icon name={portAction(p).operation === 'folder' ? 'folder' : p.status === 'ready' ? 'play' : 'download'} size={16} />{p.id in app.portBusy ? 'Working…' : portAction(p).label}</button>
            {#if p.status === 'update'}<button class="btn glass small" data-nav disabled={p.id in app.portBusy} onclick={() => app.launchPort(p)}><Icon name="play" size={16} />Open port</button>{/if}
            {#if p.status === 'ready'}<button class="btn glass small" data-nav onclick={() => app.go('library')}>View library</button>{/if}
          </div>
          <details><summary data-nav>Installation details</summary><p>{p.version ? `Version ${p.version}` : 'No version installed'}</p><p class="upstream">{p.upstream}</p>
            {#if p.status !== 'not-installed' && !needsFiles}<button class="btn glass small" data-nav onclick={() => app.openPortFolder(p)}>Open folder</button>{/if}
          </details>
        </div>
      </article>
    {:else}
      <div class="empty glass-panel"><h2>No ports to show</h2><p>{app.ports.length ? 'Try another availability filter.' : 'Check again to load available ports.'}</p><button class="btn glass" data-nav onclick={() => { filter = 'all'; void refresh(); }}>Show all ports</button></div>
    {/each}
  </div>
</div>

<style>
  .ports { padding:32px var(--gutter) 64px; max-width:1600px; margin:auto; }
  .heading { display:flex; flex-wrap:wrap; align-items:center; justify-content:space-between; gap:20px; margin-bottom:24px; }
  .eyebrow { color:var(--text-dim); font-size:13px; }
  h1 { margin:6px 0 10px; font-size:clamp(30px,3vw,40px); letter-spacing:-.03em; }
  p { color:var(--text-soft); line-height:1.6; margin:8px 0; }
  .intro { display:flex; align-items:center; gap:16px; padding:16px 20px; border-radius:16px; }
  .intro :global(svg) { flex:none; color:var(--accent); }
  .filters { display:flex; flex-wrap:wrap; gap:8px; margin:24px 0 12px; }
  .count { font-size:13px; color:var(--text-dim); }
  .grid { display:grid; grid-template-columns:repeat(auto-fill,minmax(min(100%,340px),1fr)); gap:20px; }
  .port { overflow:hidden; border-radius:18px; display:flex; flex-direction:column; min-width:0; }
  .thumb { height:110px; position:relative; overflow:hidden; filter:saturate(.55); }
  .thumb span { position:absolute; left:20px; bottom:12px; background:rgb(0 0 0 / .6); color:#fff; padding:4px 8px; border-radius:6px; font-size:12px; }
  .text { padding:20px; display:flex; flex:1; flex-direction:column; align-items:start; gap:6px; }
  h2 { font-size:21px; margin:4px 0 0; }
  .status { font-size:12px; color:var(--text-soft); }
  .status.ready { color:var(--ok); } .status.needs-files { color:var(--warn); }
  .game { margin:0 0 4px; font-size:15px; }
  .requirement, .setup { font-size:14px; } .setup { color:var(--warn); }
  .error { color:var(--bad); }
  .progress { font-size:14px; color:var(--accent); }
  .actions { display:flex; flex-wrap:wrap; gap:8px; margin-top:auto; padding-top:16px; }
  details { border-top:1px solid var(--line); margin-top:16px; padding-top:14px; width:100%; font-size:13px; }
  summary { cursor:pointer; color:var(--text-dim); padding:4px 0; }
  .upstream { overflow-wrap:anywhere; }
  .empty { border-radius:16px; padding:24px; grid-column:1/-1; }
</style>
