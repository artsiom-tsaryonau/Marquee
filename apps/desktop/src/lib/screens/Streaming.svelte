<script lang="ts">
  import { onMount } from 'svelte';
  import { app } from '../state.svelte';
  import { StreamingControls } from '../streaming.svelte';
  const controls = new StreamingControls(app.api);
  const snapshot = $derived(controls.state);
  const occupied = $derived(controls.busy || !!snapshot?.operation || !!snapshot?.viewer.running);
  let folder = $state('');
  let socket = $state('');
  let pin = $state('');
  let client = $state('');
  onMount(() => {
    let stopped = false;
    let timer: ReturnType<typeof setTimeout>;
    async function poll() {
      await controls.refresh();
      if (stopped) return;
      if (!folder && controls.state) { folder = controls.state.connection.data_dir; socket = controls.state.connection.socket; }
      timer = setTimeout(poll, 3000);
    }
    void poll();
    return () => { stopped = true; clearTimeout(timer); controls.dispose(); };
  });
  function submitPin(event: SubmitEvent) {
    event.preventDefault();
    const value = pin; pin = '';
    void controls.act(() => app.api.streamingPair(client, value));
  }
</script>

<div class="streaming">
  <header><div><h1>Streaming</h1><p>Play games from your local host in a separate viewer.</p></div><button class="btn glass" data-nav disabled={controls.busy} onclick={() => controls.refresh()}>Refresh</button></header>
  {#if controls.refreshError}<p class="error" role="alert">{controls.refreshError}</p>{/if}
  {#if controls.error}<p class="error" role="alert">{controls.error}</p>{/if}
  {#if !snapshot}<p aria-live="polite">{controls.busy ? 'Checking streaming…' : 'Streaming status unavailable. Try Refresh.'}</p>
  {:else}
    <section class="glass-panel panel">
      <h2>{snapshot.host ? 'Local host connected' : 'Local host offline'}</h2>
      <p>{snapshot.host ? 'Choose a game offered by this host. A connected host does not confirm playback.' : 'Start your configured Wolf host, then refresh. Host installation and startup are still managed outside this screen.'}</p>
      {#if snapshot.host_error}<p class="hint">{snapshot.host_error.message}</p>{/if}
      <details><summary data-nav>Advanced connection</summary>
        <form onsubmit={(e) => { e.preventDefault(); void controls.act(() => app.api.streamingSaveConnection({ data_dir: folder.trim(), socket: socket.trim() })); }}>
          <label>Streaming data folder<input data-nav bind:value={folder} disabled={occupied} onchange={() => { socket = `${folder.replace(/\/$/, '')}/streaming/wolf/snapshot/runtime/wolf.sock`; }} /></label>
          <label>Wolf local API socket<input data-nav bind:value={socket} disabled={occupied} /></label>
          <p class="hint">Use the folder containing your host identity and isolated viewer preferences. Close the viewer before changing this connection.</p>
          <button class="btn glass" data-nav disabled={occupied || !folder || !socket}>Save connection</button>
        </form>
      </details>
    </section>
    <div class="columns">
      <section class="glass-panel panel">
        <h2>Viewer</h2>
        <p aria-live="polite">{snapshot.operation === 'install' ? 'Installing reference viewer…' : snapshot.viewer.running ? (snapshot.viewer.app_id === 'pairing' ? 'Pairing window open' : 'Viewer open') : snapshot.operation ? 'Opening viewer…' : 'Viewer closed'}</p>
        <p class="hint">The Moonlight reference viewer opens on the computer running Marquee. Closing it leaves the host game running.</p>
        {#if snapshot.viewer_error}<p class="error">{snapshot.viewer_error.message}</p>{/if}
        {#if snapshot.receiver_error}<p class="error">{snapshot.receiver_error.message}</p>{/if}
        {#if snapshot.last_error}<p class="error" role="status">{snapshot.last_error.message}</p>{/if}
        <div class="actions">
          {#if !snapshot.receiver_installed}<button class="btn primary" data-nav disabled={occupied} onclick={() => controls.act(() => app.api.streamingInstallViewer())}>Install reference viewer</button>
          {:else}<button class="btn glass" data-nav disabled={occupied || !snapshot.host} onclick={() => controls.act(() => app.api.streamingPairViewer())}>Pair this viewer</button>{/if}
          <button class="btn glass" data-nav disabled={controls.busy || !snapshot.viewer.running} onclick={() => controls.act(() => app.api.streamingDisconnectViewer())}>Close viewer</button>
        </div>
        <p class="hint">Alt+Tab switches windows. Ctrl+Alt+Shift+Z releases the mouse. Ctrl+Alt+Shift+Q disconnects.</p>
      </section>
      <section class="glass-panel panel">
        <h2>Pair a device</h2>
        <p class="hint">Open pairing in Moonlight, then enter its four-digit PIN here. For this computer, use “Pair this viewer.” Close the pairing window before opening a game.</p>
        {#if snapshot.pending.length}
          <form onsubmit={submitPin}>
            <label>Waiting device<select data-nav bind:value={client}><option value="">Choose device</option>{#each snapshot.pending as pending}<option value={pending.client_ip}>{pending.client_ip}</option>{/each}</select></label>
            <label>Pairing PIN<input data-nav bind:value={pin} inputmode="numeric" pattern="[0-9]{4}" maxlength="4" autocomplete="off" required /></label>
            <button class="btn primary" data-nav disabled={controls.busy || !client || !/^\d{4}$/.test(pin)}>Pair device</button>
          </form>
        {:else}<p>No devices waiting to pair.</p>{/if}
      </section>
    </div>
    <section>
      <h2>Games offered by host</h2>
      <div class="games">{#each snapshot.host?.apps ?? [] as game (game.id)}
        <article class="glass-panel panel"><h3>{game.title}</h3><button class="btn primary" data-nav disabled={occupied || !snapshot.receiver_installed} onclick={() => controls.act(() => app.api.streamingOpenViewer(game.id))}>Open viewer</button></article>
      {:else}<p class="hint">{snapshot.host ? 'No games are configured on this host.' : 'Connect a local host to see its games.'}</p>{/each}</div>
      <p class="hint">Only configured host games appear here. Library-wide streaming and host setup are next. Physical controller testing awaits a controller.</p>
    </section>
    <section class="glass-panel panel"><h2>Host sessions</h2><p class="hint">End game stops its host session and may lose unsaved progress. Use Close viewer to disconnect instead.</p>
      {#each snapshot.host?.sessions ?? [] as session (session.session_id)}
        <div class="row"><div><strong>{snapshot.host?.apps.find(a => a.id === session.app_id)?.title ?? 'Host session'}</strong><p class="hint">{session.client_ip} · Requested {session.video_width} × {session.video_height} at {session.video_refresh_rate} FPS</p></div><div class="actions"><button class="btn glass" data-nav disabled={controls.busy} onclick={() => controls.confirm('end', session.session_id)}>{controls.confirmation === `end:${session.session_id}` ? 'Confirm end game' : 'End game'}</button>{#if controls.confirmation === `end:${session.session_id}`}<button class="btn glass" data-nav onclick={() => controls.confirmation = ''}>Cancel</button>{/if}</div></div>
      {:else}<p>No active host sessions reported.</p>{/each}
    </section>
    <section class="glass-panel panel"><h2>Paired devices</h2><p class="hint">Revoking a device removes permission to connect. It will need to pair again.</p>
      {#each snapshot.host?.clients ?? [] as device (device.client_id)}
        <div class="row"><span class="device">Device {device.client_id}</span><div class="actions"><button class="btn glass" data-nav disabled={controls.busy} onclick={() => controls.confirm('revoke', device.client_id)}>{controls.confirmation === `revoke:${device.client_id}` ? 'Confirm revoke' : 'Revoke'}</button>{#if controls.confirmation === `revoke:${device.client_id}`}<button class="btn glass" data-nav onclick={() => controls.confirmation = ''}>Cancel</button>{/if}</div></div>
      {:else}<p>No paired devices reported.</p>{/each}
    </section>
  {/if}
</div>

<style>
  .streaming { max-width: 1120px; margin: 0 auto; padding: 28px 32px 64px; display: grid; gap: 22px; }
  header, .row { display: flex; align-items: center; justify-content: space-between; gap: 16px; }
  h1 { margin: 0 0 8px; } h2 { font-size: 18px; margin: 0 0 12px; } h3 { margin: 0 0 18px; }
  p { line-height: 1.6; margin: 8px 0 14px; } header p, .hint { color: var(--text-dim); font-size: 13px; }
  .panel { padding: 22px; border-radius: 16px; } .columns { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 18px; }
  .games { display: grid; grid-template-columns: repeat(auto-fit, minmax(240px, 1fr)); gap: 16px; }
  .actions { display: flex; flex-wrap: wrap; gap: 8px; } .error { color: var(--bad); overflow-wrap: anywhere; }
  summary { cursor: pointer; padding: 12px 0; } form { display: grid; gap: 14px; } label { display: grid; gap: 8px; font-size: 13px; }
  input, select { width: 100%; padding: 10px; background: var(--field); color: inherit; border: 1px solid var(--line); border-radius: 8px; box-sizing: border-box; }
  .row { padding: 12px 0; border-top: 1px solid var(--line); } .device { overflow-wrap: anywhere; min-width: 0; }
  @media (max-width: 760px) { .streaming { padding: 20px 16px; } .columns { grid-template-columns: 1fr; } .row { flex-wrap: wrap; } }
</style>
