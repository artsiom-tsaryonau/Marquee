<script lang="ts">
  import { onMount } from 'svelte';
  import { app } from '../state.svelte';
  import { pollWhileActive } from '../activity';
  import { bytes } from '../format';
  import type { Ps3FirmwareStatus } from '../api/contracts';
  import Icon from './Icon.svelte';
  let { installed }: {installed: boolean} = $props();
  let status = $state<Ps3FirmwareStatus | null>(null);
  let reviewing = $state(false);
  let starting = $state(false);
  let error = $state('');
  let generation = 0;
  let disposed = false;
  const active = $derived(status?.job?.state === 'running' || status?.job?.state === 'promoting');
  async function refresh() {
    if (reviewing || disposed) return;
    const request = generation;
    try { const next = await app.api.ps3FirmwareStatus(); if (!disposed && request === generation) status = next; }
    catch (err) { if (!disposed && request === generation) error = (err as Error).message; }
  }
  onMount(() => {
    void refresh();
    const stop = pollWhileActive(refresh, () => active || starting ? 2000 : 10000);
    return () => { disposed = true; generation++; stop(); };
  });
  async function review() {
    reviewing = true; error = ''; generation++;
    try { const path = await app.api.pickPs3Firmware(); if (path) status = await app.api.reviewPs3Firmware(path); }
    catch (err) { error = (err as Error).message; }
    finally { generation++; reviewing = false; }
  }
  async function install() {
    if (!status?.job || active || starting) return;
    const token = status.job.review_token;
    starting = true; error = ''; generation++;
    try { status = await app.api.installPs3Firmware(token); await app.loadEmulators(); }
    catch (err) { error = (err as Error).message; }
    finally { generation++; starting = false; await refresh(); }
  }
</script>

<section class="firmware-setup">
  <h3>PS3 firmware setup</h3>
  <p class="readiness">{status?.installed_version ? `Installed firmware files: ${status.installed_version}` : 'Firmware is not installed'}</p>
  {#if status?.installed_version}<p>{status.managed_receipt ? 'Marquee installation receipt recorded.' : 'Existing files detected; no Marquee installation receipt.'} Existing firmware is kept; automatic replacement is not supported yet. Game compatibility is tested separately.</p>
  {:else}<p>Choose your PS3UPDAT.PUP, review its version, then finish the installer in RPCS3. Installation uses a temporary profile and continues if Marquee closes.</p>{/if}
  {#if !installed}<p>Install RPCS3 first using the button above.</p>{/if}
  <div class="actions">
    <button class="btn glass small" data-nav disabled={!installed || reviewing || starting || active || !!status?.installed_version} onclick={review}><Icon name="folder" size={16} />{reviewing ? 'Checking package…' : 'Choose firmware file'}</button>
    {#if status?.job?.state === 'reviewed' && !status.installed_version}<button class="btn primary small" data-nav disabled={reviewing || starting || active} onclick={install}><Icon name="download" size={16} />{starting ? 'Starting installer…' : 'Install firmware'}</button>{/if}
    <button class="btn glass small" data-nav disabled={reviewing} onclick={refresh}><Icon name="refresh" size={16} />Check status</button>
  </div>
  {#if status?.job}
    {@const job = status.job}
    <div class="package">
      <strong>Reviewed package · {job.package.version}</strong>
      <p class="path">{job.package.source}</p>
      <p>{bytes(job.package.bytes)} · RPCS3 {job.runner_version}</p>
      <p role="status">{job.message}</p>
      {#if active || starting}<p>Confirm the firmware installation in RPCS3, wait for completion, then close its window normally. The informational welcome is hidden during setup; existing saves and firmware are kept.</p>{/if}
      {#if job.output}<p>{job.output.files.toLocaleString()} firmware files · {bytes(job.output.bytes)}</p>{/if}
      <details><summary data-nav>Verification details</summary><dl><dt>PUP SHA-256</dt><dd>{job.package.sha256}</dd><dt>RPCS3 program fingerprint</dt><dd>{job.program_sha256}</dd>{#if job.output}<dt>Firmware output fingerprint</dt><dd>{job.output.sha256}</dd>{/if}{#if job.exit_code !== null}<dt>Installer exit</dt><dd>{job.exit_code}</dd>{/if}{#if job.log_path}<dt>Private installer log</dt><dd>{job.log_path}</dd>{/if}</dl><p>The package check validates structure and records a fingerprint. RPCS3 verifies package contents during installation. A stored PUP alone does not establish readiness.</p></details>
    </div>
  {/if}
  {#if error}<p class="error" role="alert">{error}</p>{/if}
</section>

<style>
  .firmware-setup { border-top: 1px solid var(--line); padding-top: 18px; margin-top: 18px; }
  h3 { margin: 0 0 8px; font-size: 15px; }
  p { margin: 8px 0; font-size: 13px; line-height: 1.6; color: var(--text-soft); }
  .readiness { font-weight: 600; color: var(--text); }
  .actions { display: flex; flex-wrap: wrap; gap: 8px; }
  .package { padding: 14px; margin-top: 14px; background: var(--field); border: 1px solid var(--line); border-radius: 10px; }
  strong { font-size: 14px; } .path { overflow-wrap: anywhere; color: var(--text-dim); font-size: 12px; }
  summary { cursor: pointer; padding-top: 8px; font-size: 13px; color: var(--text-soft); }
  dl { font-size: 12px; } dt { margin-top: 10px; color: var(--text-dim); } dd { margin: 4px 0; overflow-wrap: anywhere; }
  .error { color: var(--danger); }
</style>
