<script lang="ts">
  import { app } from '../state.svelte';
  import { bytes, transferLine } from '../format';
  import type { TorrentReview } from '../api/contracts';

  let link = $state('');
  let path = $state('');
  let review = $state<TorrentReview | null>(null);
  let peers = $state('');
  let busy = $state(false);
  let error = $state('');
  let visibleFiles = $state(50);
  $effect(() => { if (app.torrentsOn) return app.watchTransfers(); });

  async function receive() {
    if (!link.trim() || app.transferLinkBusy) return;
    if (await app.getFromLink(link)) link = '';
  }
  async function choose() {
    if (busy) return;
    busy = true; error = '';
    try {
      const selected = await app.api.pickTorrent();
      if (selected) { path = selected; review = await app.api.transferReview(selected); visibleFiles = 50; }
    } catch (e) { error = (e as Error).message; }
    finally { busy = false; }
  }
  async function download() {
    if (!review || busy) return;
    busy = true; error = '';
    try {
      await app.api.transferAddFile(path, review.review_token, peers.split(/[,\s]+/).filter(Boolean));
      review = null; peers = '';
      await app.refreshTransfers();
    } catch (e) { error = (e as Error).message; }
    finally { busy = false; }
  }
</script>

<section aria-labelledby="local-transfers-title">
  <h3 id="local-transfers-title">Local device transfers</h3>
  <p>Receive games from your other Marquee devices on a private network. Public peers, trackers and DHT are disabled.</p>
  {#if !app.torrentsOn}
    <button class="btn primary small" data-nav disabled={app.torrentSettingsBusy} onclick={() => app.setTorrents({enabled:true, trackers:false, dht:false, proxy:null})}>Enable local transfers</button>
  {:else}
    <form onsubmit={(event) => { event.preventDefault(); void receive(); }}>
      <label>Device share link<input data-nav aria-label="Device share link" aria-describedby={app.transferLinkError ? 'device-link-error' : undefined} aria-invalid={!!app.transferLinkError} placeholder="Paste the link from Share → Copy link" bind:value={link} disabled={app.transferLinkBusy} /></label>
      <button class="btn primary small" data-nav disabled={!link.trim() || app.transferLinkBusy}>{app.transferLinkBusy ? 'Connecting…' : 'Receive files'}</button>
    </form>
    {#if app.transferLinkError}<p class="error" id="device-link-error" role="alert">{app.transferLinkError}</p>{/if}
    <button class="btn glass small" data-nav disabled={busy} onclick={choose}>Choose .torrent…</button>
    {#if error}<p class="error" role="alert">{error}</p>{/if}
    {#if review}
      <article>
        <h4>{review.name}</h4>
        <p>{review.files.length} files · {bytes(review.total)} · Local peers only</p>
        <p>Tracker addresses in this file are ignored. All files download before archive preparation.</p>
        <details><summary data-nav>Review files and destination</summary>
          <p>Save to: {review.directory}</p>
          <ul>{#each review.files.slice(0, visibleFiles) as file}<li>{file.path} · {bytes(file.size)}</li>{/each}</ul>
          {#if review.files.length > visibleFiles}<button class="btn glass small" data-nav onclick={() => visibleFiles += 50}>Show more files · {visibleFiles} of {review.files.length}</button>{/if}
        </details>
        <label>Local device addresses · required<input data-nav aria-label="Torrent direct peers" placeholder="192.168.1.20:51413" bind:value={peers} disabled={busy} /></label>
        <div class="actions"><button class="btn primary small" data-nav disabled={busy || !peers.trim()} onclick={download}>{busy ? 'Starting…' : 'Download files'}</button><button class="btn glass small" data-nav disabled={busy} onclick={() => { review = null; error = ''; }}>Cancel</button></div>
      </article>
    {/if}
  {/if}
  {#if app.transferError}<p class="error" role="alert">{app.transferError}</p>{/if}
  {#each app.transfers?.items ?? [] as transfer (transfer.info_hash || transfer.name)}
    <article>
      <h4>{transfer.name}</h4>
      <p class:error={transfer.state === 'error'}>{transfer.kind === 'get' ? 'Receive' : 'Share'} · {transferLine(transfer)}</p>
      {#if transfer.error && transfer.state !== 'error'}<p class="error" role="alert">{transfer.error}</p>{/if}
      {#if transfer.state === 'downloading'}
        <progress max={transfer.total || 1} value={transfer.downloaded} aria-label={`Download progress for ${transfer.name}`}></progress>
        <p>{transfer.peers ?? 0} connected {(transfer.peers ?? 0) === 1 ? 'device' : 'devices'} · {bytes(transfer.download_bps ?? 0)}/s{#if !transfer.peers} · Waiting for the other device{/if}{#if transfer.download_bps && transfer.total > transfer.downloaded} · About {Math.max(1, Math.ceil((transfer.total - transfer.downloaded) / transfer.download_bps / 60))} min remaining{/if}</p>
      {:else if transfer.state === 'sharing' || transfer.state === 'seeding'}
        <p>{transfer.peers ?? 0} connected {(transfer.peers ?? 0) === 1 ? 'device' : 'devices'} · {bytes(transfer.upload_bps ?? 0)}/s uploaded</p>
      {:else if transfer.state === 'starting'}<p>Connecting to the local device and retrieving its file list…</p>{/if}
      <div class="actions">
        {#if transfer.link}<button class="btn glass small" data-nav onclick={() => app.copyTransferLink(transfer)}>Copy link</button>{/if}
        {#if transfer.info_hash}
          {#if !['complete','seeding','sharing'].includes(transfer.state)}<button class="btn glass small" data-nav onclick={() => app.pauseTransfer(transfer, !['paused','error','source_missing'].includes(transfer.state))}>{transfer.state === 'error' ? 'Retry' : ['paused','source_missing'].includes(transfer.state) ? 'Resume' : 'Pause'}</button>{/if}
          <button class="btn glass small" data-nav onclick={() => app.stopTransfer(transfer)}>{['sharing','seeding'].includes(transfer.state) ? 'Stop sharing' : 'Stop'}</button>
        {/if}
      </div>
      {#if transfer.directory}<details><summary data-nav>File location</summary><p>{transfer.directory}</p></details>{/if}
    </article>
  {/each}
</section>

<style>
  section, article { display:grid; gap:12px; min-width:0; }
  article { border:1px solid var(--line); padding:16px; border-radius:12px; }
  h3, h4, p { margin:0; overflow-wrap:anywhere; }
  h3 { font-size:18px; } h4 { font-size:15px; }
  p, li { font-size:13px; line-height:1.6; color:var(--text-soft); overflow-wrap:anywhere; }
  form { display:flex; flex-wrap:wrap; gap:10px; align-items:end; }
  form label { flex:1 1 280px; }
  label { display:grid; gap:8px; font-size:14px; min-width:0; }
  input { min-width:0; width:100%; border:1px solid var(--line); border-radius:8px; background:var(--field); color:var(--text); padding:12px; font:inherit; }
  .actions { display:flex; flex-wrap:wrap; gap:8px; }
  .error { color:var(--bad); }
  progress { width:100%; accent-color:var(--accent); }
  details { min-width:0; font-size:13px; } summary { cursor:pointer; padding:8px 0; }
  ul { max-height:240px; overflow:auto; padding-left:20px; }
</style>
