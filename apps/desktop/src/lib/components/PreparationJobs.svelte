<script lang="ts">
  import { onMount } from 'svelte';
  import { app } from '../state.svelte';
  import { systemLabel } from '../labels';
  import type { ManagerStatus, PreparationJob, PreparationState } from '../api/contracts';
  let { paths }: { paths: string[] } = $props();
  let systems = $state<{id: string; name: string}[]>([]);
  let manager = $state<ManagerStatus | null>(null);
  let system = $state('');
  let jobs = $state<PreparationJob[]>([]);
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
  const size = (bytes: number) => bytes >= 1073741824 ? `${(bytes / 1073741824).toFixed(1)} GiB` : `${(bytes / 1048576).toFixed(1)} MiB`;
  async function refresh() {
    if (!mounted || refreshing) return;
    refreshing = true; lastRefresh = Date.now();
    try {
      manager = await app.api.managerStatus();
      const next = await app.api.preparationJobs();
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
    <span>{manager?.stopping ? 'Manager finishing work before stopping' : manager?.running ? 'Background manager running' : 'Background manager offline'}</span>
    {#if manager?.running}<button class="btn glass small" data-nav disabled={manager.stopping} onclick={() => controlManager(false)}>Stop manager after current work</button>
    {:else}<button class="btn glass small" data-nav onclick={() => controlManager(true)}>Start manager</button>{/if}
  </div>
  {#if jobs.length}<h3>Preparation history · {jobs.length}</h3>{/if}
  {#each jobs as job (job.id)}
    <article>
      <h4>{job.source.split('/').at(-1)}</h4>
      <p><strong>{labels[job.state]}</strong> · {systemLabel(job.plan.system)}</p>
      <p>{job.plan.keep_archive ? 'Keep arcade ZIP intact' : 'Extract game files'} · {job.plan.files.length} files · {size(job.source_size)} source · {size(job.plan.required_bytes)} free space required</p>
      <p class="path">Destination: {job.destination}</p>
      <details><summary data-nav>Review files</summary><ul>{#each job.plan.files.slice(0, 100) as file}<li>{file.path} · {size(file.size)}</li>{/each}</ul>{#if job.plan.files.length > 100}<p>Showing the first 100 files.</p>{/if}</details>
      {#if job.error}<p class="error">{job.error}</p>{/if}
      {#if job.state === 'registered'}<p>Original archive retained. Game launch and compatibility have not been tested by this operation.</p>
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
