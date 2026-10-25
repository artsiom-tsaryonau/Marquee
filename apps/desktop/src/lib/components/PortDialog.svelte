<script lang="ts">
  import { app } from '../state.svelte';
  import { modal } from '../modal';
  import { portAction, portStatusLabel } from '../presentation';
  import { platformLabel } from '../labels';
  import Artwork from './Artwork.svelte';
  import Icon from './Icon.svelte';
  const port = $derived(app.ports.find((p) => p.id === app.portDialogId));
</script>

{#if port}
  <div class="scrim" data-modal-dismiss role="presentation" onclick={() => (app.portDialogId = null)}></div>
  <div class="dialog glass-panel" role="dialog" aria-modal="true" aria-labelledby="port-title" use:modal={() => (app.portDialogId = null)}>
    <header><div class="art"><Artwork art={port.art} /></div><div class="title"><span>Native port · {platformLabel[port.platform]}</span><h2 id="port-title">{port.name}</h2><p>{port.game}</p></div><button class="btn glass small close" data-nav aria-label="Close port settings" onclick={() => (app.portDialogId = null)}><Icon name="x" /></button></header>
    <div class="body">
      <p class="status">{portStatusLabel(port)}</p>
      <h3>Game files and setup</h3><p>{port.requirement}</p>
      <p>Installing a port installs its engine. Original game files are separate. Marquee has no complete game-data validator for these ports; opening the port may enter its own setup window.</p>
      {#if app.portErrors[port.id]}<p class="error" role="alert">{app.portErrors[port.id]}</p>{/if}
      {#if app.portBusy[port.id]}<p role="status">{app.portBusy[port.id]}</p>{/if}
      <div class="actions">
        {#if port.id in app.portBusy}<button class="btn glass" data-nav onclick={() => app.cancelPortInstall(port)}>Cancel installation</button>{/if}
        {#if port.job && ['failed', 'interrupted', 'cancelled'].includes(port.job.state) && port.status !== 'not-installed'}<button class="btn glass" data-nav onclick={() => app.installPort(port)}>Retry engine installation</button>{/if}
        <button class="btn primary" data-nav disabled={port.id in app.portBusy} onclick={() => app.runPortAction(port)}>{port.id in app.portBusy ? 'Working…' : portAction(port).label}</button>
        {#if port.status === 'update'}<button class="btn glass" data-nav disabled={port.id in app.portBusy} onclick={() => app.launchPort(port)}>Open port</button>{/if}
        {#if port.status !== 'not-installed' && portAction(port).operation !== 'folder'}<button class="btn glass" data-nav onclick={() => app.openPortFolder(port)}><Icon name="folder" />Open game files folder</button>{/if}
        <button class="btn glass" data-nav onclick={() => app.loadPorts()}><Icon name="refresh" />Refresh status</button>
      </div>
      <details><summary data-nav>Installation details</summary><dl><dt>Version</dt><dd>{port.status === 'not-installed' ? 'Not installed' : port.version || 'Unknown'}</dd><dt>Upstream</dt><dd>{port.upstream}</dd></dl></details>
      <p>Accepted engine installations continue when Marquee closes. Cancellation takes effect at the next download or preparation checkpoint; publication finishes once started.</p>
      <p>Installed ports also appear in Library. Saves and settings stay in the port’s managed folder when the engine is updated.</p>
    </div>
  </div>
{/if}

<style>
  .scrim { position:fixed; inset:0; z-index:40; background:rgb(0 0 0 / .65); backdrop-filter:var(--overlay-filter); }
  .dialog { position:fixed; top:6vh; left:50%; transform:translateX(-50%); width:min(760px,calc(100% - 32px)); max-height:88vh; overflow:auto; z-index:41; border-radius:20px; background:var(--surface-strong); }
  header { position:relative; overflow:hidden; min-height:160px; display:flex; align-items:end; padding:24px; }
  .art { position:absolute; inset:0; filter:saturate(.5) brightness(.35); }
  .title { position:relative; padding-right:44px; } .title span { font-size:12px; color:#ddd; } .title p { color:#ddd; } h2 { font-size:28px; color:#fff; margin:8px 0; }
  .close { position:absolute; top:16px; right:16px; } .body { padding:24px; display:grid; gap:16px; }
  p { margin:0; color:var(--text-soft); line-height:1.6; font-size:14px; } h3 { margin:0; font-size:18px; } .status { color:var(--warn); }
  .actions { display:flex; flex-wrap:wrap; gap:10px; } summary { cursor:pointer; color:var(--text-soft); padding:8px 0; } details { font-size:13px; border-top:1px solid var(--line); padding-top:12px; }
  dl { display:grid; grid-template-columns:100px 1fr; gap:12px; } dt { color:var(--text-dim); } dd { margin:0; overflow-wrap:anywhere; } .error { color:var(--bad); }
</style>
