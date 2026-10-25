<script lang="ts">
  import { onMount } from 'svelte';
  import { app } from '../state.svelte';
  import type { StorageMoveStatus } from '../api/contracts';
  import { asError } from '../api/core';
  import { bytes } from '../format';
  import Icon from './Icon.svelte';
  let { onchange = () => {} }: { onchange?: () => void } = $props();
  let moveState = $state<StorageMoveStatus | null>(null);
  let busy = $state(false);
  let error = $state('');
  let confirmation = $state<'apply' | 'rollback' | null>(null);
  let disposed = false;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let polling = false;
  async function refresh() {
    if (disposed || polling) return;
    polling = true; clearTimeout(timer);
    try {
      const result = await app.api.storageMoveStatus();
      if (disposed) return;
      if (moveState && ((moveState.running && !result.running) || moveState.plan?.stage !== result.plan?.stage || moveState.plan?.id !== result.plan?.id)) onchange();
      if (moveState?.plan?.id !== result.plan?.id) confirmation = null;
      moveState = result;
      if (result.running || result.manager_stopping) timer = setTimeout(() => void refresh(), 800);
    } catch(e) { if (!disposed) error = asError(e).message; }
    finally { polling = false; }
  }
  async function action(run: () => Promise<unknown>) {
    if (busy || moveState?.running) return;
    busy = true; error = ''; confirmation = null;
    try { await run(); await refresh(); }
    catch(e) { if (!disposed) error = asError(e).message; }
    finally { if (!disposed) busy = false; }
  }
  async function review() {
    await action(async () => {
      const parent = await app.api.pickRoot('Choose the destination parent for managed installations');
      if (parent) await app.api.storageMoveReview(parent);
    });
  }
  function confirm() {
    const selected = confirmation;
    const id = moveState?.plan?.id;
    if (!id || !selected) return;
    void action(() => selected === 'apply' ? app.api.storageMoveApply(id) : app.api.storageMoveRollback(id));
  }
  onMount(() => { void refresh(); return () => { disposed = true; clearTimeout(timer); }; });
  const stageLabel = (stage: string) => ({ reviewed:'Ready for review',moving:'Move needs recovery',verifying:'Verification needs recovery',complete:'Move complete',rolling_back:'Rollback needs recovery',rolled_back:'Rollback complete' })[stage] || stage;
</script>

<section class="installation-move glass-panel" aria-label="Move managed installations">
  <header><div><h3>Move managed installations</h3><p>Relocate prepared games on the same filesystem. Review the destination, size and affected games first.</p></div><Icon name="folder" size={21} /></header>
  <p>Close games and stop the streaming host. Disable torrents, finish or cancel pending preparation and browser jobs, then stop the manager. Firmware, saves and source downloads stay in place.</p>
  {#if error || moveState?.error}<p class="error" role="alert">{error || moveState?.error?.message}</p>{/if}
  {#if !moveState}<button class="btn glass small" data-nav disabled={busy} onclick={refresh}>Load move status</button>
  {:else}
    <div class="manager-status"><span>{moveState.manager_stopping ? 'Manager is draining accepted work…' : moveState.manager_running ? 'Manager is running' : 'Manager is stopped'}</span>
      {#if moveState.manager_running}<button class="btn glass small" data-nav disabled={busy || moveState.running || moveState.manager_stopping} onclick={() => action(() => app.api.stopManager())}>Stop manager for maintenance</button>
      {:else if !moveState.running}<button class="btn glass small" data-nav disabled={busy || (!!moveState.plan && !['reviewed','complete','rolled_back'].includes(moveState.plan.stage))} onclick={() => action(() => app.api.startManager())}>Start manager</button>{/if}
    </div>
    {#if moveState.running}<p role="status">{moveState.action === 'review' ? 'Hashing installations and checking receipts…' : moveState.action === 'rollback' ? 'Verifying and restoring the previous location…' : 'Moving and verifying installations…'} Keep Marquee open. An interrupted move can be recovered here.</p>{/if}
    {#if moveState.plan}
      <div class="move-review">
        <strong>{stageLabel(moveState.plan.stage)}</strong>
        <dl><dt>From</dt><dd><code>{moveState.plan.source}</code></dd><dt>To</dt><dd><code>{moveState.plan.destination}</code></dd><dt>Size</dt><dd>{bytes(moveState.plan.bytes)} · {moveState.plan.files} {moveState.plan.files === 1 ? 'file' : 'files'} · {moveState.plan.receipt_ids.length} {moveState.plan.receipt_ids.length === 1 ? 'installation' : 'installations'}</dd>{#if moveState.plan.free_bytes !== null}<dt>Free</dt><dd>{bytes(moveState.plan.free_bytes)} on the destination volume</dd>{/if}</dl>
        <p>The existing path is retained as a compatibility link. Receipts and game IDs are preserved. A same-filesystem rename does not need a second payload copy.</p>
        {#if moveState.games.length}<details><summary>Affected games ({moveState.games.length})</summary><ul>{#each moveState.games as game (game.id)}<li>{game.title}</li>{/each}</ul></details>{/if}
        <div class="actions">
          {#if !['complete','rolled_back','rolling_back'].includes(moveState.plan.stage)}<button class="btn primary small" data-nav disabled={busy || moveState.running || moveState.manager_running} onclick={() => confirmation = 'apply'}>{moveState.plan.stage === 'reviewed' ? 'Move installations…' : 'Resume move…'}</button>{/if}
          {#if moveState.plan.stage !== 'rolled_back'}<button class="btn glass small" data-nav disabled={busy || moveState.running || moveState.manager_running} onclick={() => confirmation = 'rollback'}>{moveState.plan.stage === 'reviewed' ? 'Cancel review…' : 'Roll back…'}</button>{/if}
        </div>
        {#if confirmation}
          <div class="confirmation" role="group" aria-label="Confirm installation relocation">
            <p>{confirmation === 'apply' ? 'Move this reviewed installation tree and verify its contents at the destination?' : 'Restore the previous location? Rollback is refused if installations or registered jobs have changed.'}</p>
            <div class="actions"><button class="btn primary small" data-nav disabled={busy || moveState.running || moveState.manager_running} onclick={confirm}>Confirm {confirmation === 'apply' ? 'move' : 'rollback'}</button><button class="btn glass small" data-nav onclick={() => confirmation = null}>Keep current location</button></div>
          </div>
        {/if}
      </div>
    {/if}
    <button class="btn glass small" data-nav disabled={busy || moveState.running || moveState.manager_running || (!!moveState.plan && !['reviewed','complete','rolled_back'].includes(moveState.plan.stage))} onclick={review}><Icon name="folder" size={15} />Choose destination and review</button>
  {/if}
</section>

<style>
  section { padding:20px; border-radius:12px; display:grid; gap:14px; min-width:0; }
  header { display:flex; justify-content:space-between; gap:16px; }
  h3 { margin:0; font-size:14px; }
  p { color:var(--text-dim); font-size:12px; line-height:1.6; margin:4px 0 0; }
  .manager-status { display:flex; justify-content:space-between; gap:12px; align-items:center; flex-wrap:wrap; font-size:12px; }
  .move-review { padding:16px; border:1px solid var(--line); border-radius:9px; display:grid; gap:12px; min-width:0; }
  strong { font-size:13px; }
  dl { display:grid; grid-template-columns:42px minmax(0,1fr); gap:8px 12px; margin:0; font-size:12px; }
  dt { color:var(--text-dim); } dd { margin:0; overflow-wrap:anywhere; }
  code { font-size:11px; color:var(--text-soft); }
  .actions { display:flex; flex-wrap:wrap; gap:8px; }
  details { font-size:12px; color:var(--text-soft); } summary { cursor:pointer; } li { margin-top:6px; }
  .confirmation { border-top:1px solid var(--line); padding-top:12px; display:grid; gap:10px; }
  .error { color:var(--danger); }
  section > button { justify-self:start; }
</style>
