<script lang="ts">
  import { onMount } from 'svelte';
  import { app } from '../state.svelte';
  import { systemLabel } from '../labels';
  import type { TorrentDownload, ManagerStatus, BrowserJob, PreparationJob, PreparationState } from '../api/contracts';
  let { paths }: { paths: string[] } = $props();
  let systems = $state<{id: string; name: string}[]>([]);
  let manager = $state<ManagerStatus | null>(null);
  let system = $state('');
  let jobs = $state<PreparationJob[]>([]);
  let torrentDownloads = $state<TorrentDownload[]>([]);
  let torrentSystems = $state<Record<string,string>>({});
  let browserJobs = $state<BrowserJob[]>([]);
  let browserSystems = $state<Record<string, string>>({});
  let inspecting = $state(false);
  let error = $state('');
  let refreshError = $state('');
  let mounted = false;
  let refreshing = false;
  let lastRefresh = 0;
  const labels: Record<PreparationState, string> = {
    awaiting_review: 'Ready for review', preparing: 'Preparing game files',
    interrupted: 'Interrupted · retry available', awaiting_library: 'Prepared · library registration pending',
    registered: 'Added to library', failed: 'Needs attention', cancelled: 'Cancelled',
  };
  const size = (bytes: number) => bytes >= 1073741824 ? `${(bytes / 1073741824).toFixed(1)} GiB` : bytes >= 1048576 ? `${(bytes / 1048576).toFixed(1)} MiB` : bytes >= 1024 ? `${(bytes / 1024).toFixed(1)} KiB` : `${bytes} bytes`;
  async function refresh() {
    if (!mounted || refreshing) return;
    refreshing = true; lastRefresh = Date.now();
    try {
      manager = await app.api.managerStatus();
      const next = await app.api.preparationJobs();
      browserJobs = await app.api.browserJobs();
      torrentDownloads = await app.api.torrentDownloads();
      if (!mounted) return;
      const changed = next.some(j => j.state === 'registered' && jobs.find(old => old.id === j.id)?.state !== 'registered');
      jobs = next; refreshError = '';
      if (changed) app.applySnapshot(await app.api.snapshot());
    } catch (err) { if (mounted) refreshError = (err as Error).message ?? 'Could not load preparation history'; }
    finally { refreshing = false; }
  }
  async function inspect() {
    if (!system || inspecting || app.intakeBusy) return;
    inspecting = true; error = '';
    const failures: string[] = [];
    for (const path of [...paths]) {
      try { await app.api.inspectPreparation(path, system); }
      catch (err) { failures.push(`${path.split('/').at(-1)}: ${(err as Error).message}`); }
    }
    if (mounted) { error = failures.join('\n'); await refresh(); inspecting = false; }
  }
  async function run(job: PreparationJob) {
    error = '';
    try {
      const next = await app.api.startPreparation(job.id, job.review_token);
      if (mounted) jobs = jobs.map(j => j.id === next.id ? next : j);
    } catch (err) { if (mounted) error = (err as Error).message; }
  }
  async function cancel(job: PreparationJob) {
    try { await app.api.cancelPreparation(job.id); await refresh(); }
    catch (err) { if (mounted) error = (err as Error).message; }
  }
  async function controlManager(start: boolean) {
    error = '';
    try {
      manager = start ? await app.api.startManager() : await app.api.stopManager();
      if (start) await refresh();
    } catch (err) { if (mounted) error = (err as Error).message; }
  }
  async function cancelBrowser(job: BrowserJob) {
    try { await app.api.cancelBrowserJob(job.job_id); await refresh(); }
    catch (err) { if (mounted) error = (err as Error).message; }
  }
  async function inspectBrowser(job: BrowserJob, selectedSystem = browserSystems[job.job_id]) {
    if (inspecting || !selectedSystem) return;
    inspecting = true; error = '';
    try { await app.api.inspectBrowserPreparation(job.job_id, selectedSystem); await refresh(); }
    catch (err) { if (mounted) error = (err as Error).message; }
    finally { inspecting = false; }
  }
  async function inspectTorrent(download: TorrentDownload, path: string, selectedSystem = torrentSystems[`${download.info_hash}/${path}`]) {
    if (inspecting || !selectedSystem) return;
    inspecting = true; error = '';
    try { await app.api.inspectTorrentPreparation(download.info_hash, path, selectedSystem); await refresh(); }
    catch (err) { if (mounted) error = (err as Error).message; }
    finally { inspecting = false; }
  }
  onMount(() => {
    mounted = true;
    void app.api.preparationSystems().then(value => { if (mounted) systems = value; }).catch(err => { if (mounted) error = err.message; });
    void refresh();
    const resume = () => { if (document.visibilityState === 'visible') void refresh(); };
    document.addEventListener('visibilitychange', resume);
    const timer = setInterval(() => {
      if (document.visibilityState !== 'visible') return;
      if (jobs.some(j => j.state === 'preparing') || Date.now() - lastRefresh >= 10000) void refresh();
    }, 1500);
    return () => { mounted = false; clearInterval(timer); document.removeEventListener('visibilitychange', resume); };
  });
</script>

<section aria-label="Saved archive preparation">
  <h3>Prepare game files</h3>
  <p>Choose the platform, then inspect the selected archives. Originals are always kept. Firmware, installers, updates and unsupported console packages need their own setup workflow.</p>
  {#if paths.length}
    <label>Platform
      <select class="ui-select" data-nav bind:value={system} disabled={inspecting || app.intakeBusy}>
        <option value="">Choose a supported platform…</option>
        {#each systems as option}<option value={option.id}>{option.name}</option>{/each}
      </select>
    </label>
    <button class="btn primary" data-nav disabled={!system || inspecting || app.intakeBusy || jobs.some(j => j.state === 'preparing')} onclick={inspect}>{inspecting ? 'Inspecting archives…' : 'Inspect selected archives'}</button>
  {/if}
  {#if error}<p class="error" role="alert">{error}</p>{/if}
  {#if refreshError}<p class="error" role="alert">{refreshError}</p>{/if}
  <p>Accepted preparation continues after Marquee closes. Reopen Marquee to add completed games to the library. Interrupted work can be retried; byte progress and cancellation during extraction are not available yet.</p>
  <div class="actions">
    <span>{manager?.stopping ? 'Manager stopping after preparation' : manager?.running ? 'Background manager running' : 'Background manager offline'}</span>
    {#if manager?.running}<button class="btn glass small" data-nav disabled={manager.stopping} onclick={() => controlManager(false)}>Stop manager</button>
    {:else}<button class="btn glass small" data-nav onclick={() => controlManager(true)}>Start manager</button>{/if}
  </div>
  <p>Stopping the manager saves torrent progress and stops downloads and seeding. Accepted archive preparation finishes before the manager exits. Start it again to resume eligible torrents.</p>
  {#if browserJobs.length}<h3>Browser downloads · {browserJobs.length}</h3>
    <p>The companion uses your existing browser session. Open it and refresh to report progress or apply a cancellation request. Downloaded files need a preparation review before they can enter the library.</p>
  {/if}
  {#each browserJobs as job (job.job_id)}
    {@const linked = jobs.find(preparation => preparation.id === job.preparation_id)}
    <article>
      <h4>{job.selection.title}</h4>
      <p>{job.selection.provider === 'vimm' ? 'Vimm' : 'Axekin'} · {linked ? labels[linked.state] : job.cancel_requested ? 'Cancellation requested · waiting for browser' : job.state === 'completed' ? 'Downloaded · preparation needed' : job.state === 'waiting' ? 'Waiting for browser download' : job.state === 'downloading' ? 'Downloading in browser' : job.state === 'interrupted' ? 'Interrupted · review the source to retry' : 'Cancelled'}</p>
      <p>{size(job.bytes)}{job.total ? ` of ${size(job.total)}` : ''} · Last reported by the browser</p>
      {#if job.error}<p class="error">{job.error}</p>{/if}
      {#if job.result}<p class="path">Source: {job.result.filename}</p><p>Local SHA-256 recorded. Game identity and compatibility remain unverified.</p>
        {#if job.source_available === false}<p>Source archive is missing, unavailable or changed in size. Download and preparation history are retained.</p>{/if}
        {#if linked}<p>Linked preparation · {systemLabel(linked.plan.system)} · {linked.state === 'registered' ? `${linked.edition_ids.length} library entries` : 'Review and controls in Preparation history below'}</p>
          {#if linked.state === 'cancelled'}<button class="btn glass small" data-nav disabled={inspecting || job.source_available === false} onclick={() => inspectBrowser(job, linked.plan.system)}>Review cancelled preparation</button>{/if}
        {:else if job.preparation_id}<p class="error">Linked preparation could not be loaded. Retry refreshing; no new job will be created.</p>
        {:else if job.selection.link_type && job.selection.link_type !== 'base'}<p>Updates and DLC need their own supported setup workflow.</p>
        {:else if /\.(zip|7z|rar)$/i.test(job.result.filename)}
          <label>Platform for {job.selection.title}
            <select class="ui-select" data-nav bind:value={browserSystems[job.job_id]} disabled={inspecting || job.source_available === false}>
              <option value="">Choose a supported platform…</option>
              {#each systems as option}<option value={option.id}>{option.name}</option>{/each}
            </select>
          </label>
          <button class="btn glass small" data-nav disabled={inspecting || app.intakeBusy || !browserSystems[job.job_id] || job.source_available === false} onclick={() => inspectBrowser(job)}>{inspecting ? 'Inspecting archive…' : 'Inspect downloaded archive'}</button>
        {:else}<p>Add the downloaded game folder through Library to scan supported media.</p>{/if}
      {:else if ['waiting', 'downloading'].includes(job.state)}
        <button class="btn glass small" data-nav disabled={job.cancel_requested} onclick={() => cancelBrowser(job)}>Request cancellation</button>
      {/if}
    </article>
  {/each}
  {#if torrentDownloads.length}<h3>Completed torrent downloads · {torrentDownloads.length}</h3>
    <p>Choose a supported archive and platform to review preparation. Original files stay in their download folders for seeding; packages, patches and DLC need their own setup workflow.</p>
  {/if}
  {#each torrentDownloads as download (download.info_hash)}
    <article>
      <h4>{download.name}</h4>
      {#if download.error}<p class="error">{download.error}</p>{/if}
      {#each download.files.slice(0, 50) as file (file.path)}
        {@const linked = jobs.find(job => job.id === file.preparation_id)}
        <p class="path">{file.path} · {size(file.size)}</p>
        {#if linked}<p>Linked preparation · {systemLabel(linked.plan.system)} · {labels[linked.state]}</p>
          {#if linked.state === 'cancelled'}<button class="btn glass small" data-nav disabled={inspecting || !download.available} onclick={() => inspectTorrent(download, file.path, linked.plan.system)}>Review cancelled preparation</button>{/if}
        {:else if file.preparation_id}<p class="error">Linked preparation could not be loaded. Refresh to retry; no duplicate job will be created.</p>
        {:else if !file.supported}<p>{file.reason}</p>
        {:else}
          <label>Platform for {file.path}
            <select class="ui-select" data-nav aria-label={`Platform for ${file.path}`} bind:value={torrentSystems[`${download.info_hash}/${file.path}`]} disabled={inspecting || !download.available}>
              <option value="">Choose a supported platform…</option>
              {#each systems as option}<option value={option.id}>{option.name}</option>{/each}
            </select>
          </label>
          <button class="btn glass small" data-nav disabled={inspecting || app.intakeBusy || !download.available || !torrentSystems[`${download.info_hash}/${file.path}`]} onclick={() => inspectTorrent(download, file.path)}>{inspecting ? 'Verifying and inspecting…' : 'Inspect torrent archive'}</button>
        {/if}
      {/each}
      {#if download.files.length > 50}<p>Showing the first 50 files. Use the CLI to review another archive.</p>{/if}
    </article>
  {/each}
  {#if jobs.length}<h3>Preparation history · {jobs.length}</h3>{/if}
  {#each jobs as job (job.id)}
    <article>
      <h4>{job.source.split('/').at(-1)}</h4>
      <p><strong>{labels[job.state]}</strong> · {systemLabel(job.plan.system)}</p>
      <p>{job.plan.keep_archive ? 'Keep arcade ZIP intact' : 'Extract game files'} · {job.plan.files.length} {job.plan.files.length === 1 ? 'file' : 'files'} · {size(job.source_size)} source · {size(job.plan.required_bytes)} free space required</p>
      <p class="path">Destination: {job.destination}</p>
      {#if job.torrent_source}<p>Torrent · {job.torrent_source.name} · {job.torrent_source.relative_path}. Download integrity was checked; game authenticity and compatibility remain unverified.</p>{/if}
      {#if job.browser_source}<p>Downloaded from {job.browser_source.selection.provider === 'vimm' ? 'Vimm' : 'Axekin'} · {job.browser_source.selection.title}. Download provenance is retained with this review and receipt.</p>{/if}
      <details><summary data-nav>Review files</summary><ul>{#each job.plan.files.slice(0, 100) as file}<li>{file.path} · {size(file.size)}</li>{/each}</ul>{#if job.plan.files.length > 100}<p>Showing the first 100 files.</p>{/if}</details>
      {#if job.error}<p class="error">{job.error}</p>{/if}
      {#if job.state === 'registered'}<p>Preparation did not delete the source archive. Game launch and compatibility have not been tested by this operation.</p>
      {:else if !['preparing', 'cancelled'].includes(job.state)}
        <div class="actions">
          <button class="btn primary small" data-nav disabled={inspecting || app.intakeBusy || jobs.some(j => j.state === 'preparing')} onclick={() => run(job)}>{job.state === 'awaiting_review' ? 'Approve and prepare' : job.receipt ? 'Retry registration' : 'Retry preparation'}</button>
          {#if !job.receipt}<button class="btn glass small" data-nav disabled={inspecting || jobs.some(j => j.state === 'preparing')} onclick={() => cancel(job)}>Cancel job</button>{/if}
        </div>
      {/if}
    </article>
  {/each}
</section>

<style>
  section { display:grid; gap:14px; }
  h3, h4, p { margin:0; } h3 { font-size:16px; } h4 { font-size:14px; overflow-wrap:anywhere; }
  p, li { color:var(--text-soft); font-size:13px; line-height:1.6; }
  label { display:grid; gap:8px; font-size:14px; } select { width:100%; }
  article { display:grid; gap:10px; border:1px solid var(--line); border-radius:12px; padding:16px; }
  details { font-size:13px; } summary { cursor:pointer; padding:8px 0; }
  .path, li { overflow-wrap:anywhere; } .error { color:var(--bad); white-space:pre-line; }
  .actions { display:flex; flex-wrap:wrap; gap:8px; }
</style>
