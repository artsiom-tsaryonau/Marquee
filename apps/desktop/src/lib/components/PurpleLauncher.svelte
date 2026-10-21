<script lang="ts">
  import { onMount } from 'svelte';
  import { app } from '../state.svelte';
  import { asError } from '../api/core';
  import type { PurpleStatus } from '../api/contracts';
  import { brandIcon } from '../brandIcons';
  import Icon from './Icon.svelte';

  const purpleMark = brandIcon('purple');
  import { pollWhileActive } from '../activity';

  let status = $state<PurpleStatus | null>(null);
  let error = $state<string | null>(null);
  let connectionError = $state<string | null>(null);
  let pending = $state(false);
  let details = $state(false);
  const busy = $derived(pending || !!status?.busy || !!status?.running);
  onMount(() => {
    let alive = true;
    let polling = false;
    async function refresh() {
      if (polling) return;
      polling = true;
      try { const next = await app.api.purpleStatus(); if (alive) { if (JSON.stringify(next) !== JSON.stringify(status)) status = next; connectionError = null; } }
      catch (err) { if (alive) connectionError = asError(err).message; }
      finally { polling = false; }
    }
    const stop = pollWhileActive(refresh, () => status?.busy || status?.running ? 1500 : 15000);
    return () => { alive = false; stop(); };
  });
  async function action(kind: 'install' | 'open' | 'cancel') {
    if (pending) return;
    pending = true; error = null;
    try {
      if (kind === 'open') await app.api.openPurple();
      else if (kind === 'cancel') status = await app.api.cancelPurple();
      else status = await app.api.installPurple();
      status = await app.api.purpleStatus();
    } catch (err) { error = asError(err).message; }
    finally { pending = false; }
  }
</script>

<section class="purple glass-panel" aria-labelledby="purple-title" aria-busy={!!status?.busy}>
  <div class="heading"><span class="mark"><img src={purpleMark.src} width="36" height="36" alt="" /></span><div><span class="eyebrow">Managed Windows launcher</span><h2 id="purple-title">Purple for Aion 2</h2></div><span class="badge">{status?.busy ? 'Setting up' : status?.running ? 'Purple open' : status?.ready ? 'Launcher installed' : 'Setup available'}</span></div>
  <p>Marquee prepares the compatibility runtime and a separate Purple installation for you. Sign in and select Aion 2 in Purple’s Store.</p>
  <p class="compatibility">Aion 2 on Linux is unverified. NCSOFT lists Windows as its supported PC platform; installing Purple does not establish game compatibility.</p>
  {#if status?.busy || status?.error}<p class="progress" role="status" aria-live="polite">{status.message}</p>{/if}
  {#if error || connectionError || status?.error}<p class="error" role="alert">{error || connectionError || status?.error}</p>{/if}
  <div class="actions" data-nav-row>
    {#if status?.ready}
      <button class="btn primary" data-nav disabled={busy} onclick={() => action('open')}><Icon name="play" size={16} />{status.running ? 'Purple is open' : 'Open Purple for Aion 2'}</button>
    {/if}
    {#if !status?.ready || status?.error}
      <button class="btn primary" data-nav disabled={busy || !status} onclick={() => action('install')}>{status?.busy ? 'Setting up…' : status?.error || status?.installed ? 'Retry setup' : 'Install Purple'}</button>
    {/if}
    {#if status?.busy}<button class="btn glass" data-nav disabled={pending} onclick={() => action('cancel')}>Stop setup</button>{/if}
    <button class="btn glass" data-nav aria-expanded={details} onclick={() => details = !details}>Setup details</button>
    <a class="btn glass" data-nav href="https://purple.plaync.com/game/aion2global?locale=en-US" target="_blank" rel="noreferrer">Aion 2 on Purple ↗</a>
  </div>
  {#if details}
    <div class="details">
      <p>First setup downloads GE-Proton, Steam runtime, dependencies, and the verified NCSOFT installer (388 MiB). Complete any installer windows that appear and keep Purple’s default folder. No Lutris configuration is needed.</p>
      <p>You can leave this page while setup continues. Stop ends the active setup process; downloads finish before stopping. Retry keeps the prefix and resumes completed dependencies.</p>
      <p>Purple manages your account, region, game downloads and updates. Marquee opens the launcher; game discovery and Aion 2 playtime tracking are not connected yet.</p>
      {#if status?.prefix}<dl><dt>Prefix</dt><dd>{status.prefix}</dd><dt>Runtime</dt><dd>{status.runtime || 'Selected automatically on first setup'}</dd><dt>Local setup log</dt><dd>{status.log_path}</dd></dl>{/if}
    </div>
  {/if}
</section>

<style>
  .purple { padding:26px; border-radius:20px; background:radial-gradient(ellipse at 0 0,rgb(139 92 246 / .13),transparent 75%),var(--surface); }
  .heading { display:flex; align-items:center; gap:16px; flex-wrap:wrap; } .heading>div { flex:1; } .mark { width:54px; height:54px; display:grid; place-items:center; border-radius:15px; color:var(--accent); background:rgb(139 92 246 / .12); } .mark img { width:36px; height:36px; object-fit:contain; }
  h2 { margin:4px 0; font-size:23px; } p { margin:16px 0 0; line-height:1.65; color:var(--text-soft); font-size:14px; } .badge { padding:6px 10px; border-radius:8px; background:var(--field); color:var(--text-dim); font-size:12px; }
  .compatibility { color:var(--text-dim); font-size:13px; } .actions { margin-top:20px; display:flex; gap:10px; flex-wrap:wrap; } .actions a { text-decoration:none; } .progress { color:var(--accent); } .error { color:var(--bad); } .details { border-top:1px solid var(--line); margin-top:20px; } dl { display:grid; gap:8px; margin:18px 0 0; font-size:12px; } dt { color:var(--text-dim); } dd { margin:0 0 8px; overflow-wrap:anywhere; font-family:ui-monospace,monospace; }
</style>
