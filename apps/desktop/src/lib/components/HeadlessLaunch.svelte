<script lang="ts">
  import { app } from '../state.svelte';
  import { music } from '../music.svelte';
  import { asError } from '../api/core';
  import type { HeadlessStatus } from '../api/contracts';
  import Icon from './Icon.svelte';
  let { editionId, runnerId, disabled = false, onactive }: { editionId: string; runnerId: string; disabled?: boolean; onactive: (active:boolean)=>void } = $props();
  let status = $state<HeadlessStatus | null>(null);
  let busy = $state(false);
  let error = $state('');
  let confirmStop = $state(false);
  const activeHere = $derived(status?.active_edition_id === editionId && status.game_running);
  $effect(() => { onactive(!!status?.game_running || !!status?.busy || busy); });
  $effect(() => {
    const id = editionId; void runnerId;
    let stopped = false; let timer: ReturnType<typeof setTimeout>;
    status = null; error = ''; confirmStop = false;
    async function poll() {
      try { const next = await app.api.headlessStatus(id); if (!stopped) status = next; }
      catch (e) { if (!stopped) error = asError(e).message; }
      if (!stopped) timer = setTimeout(poll, 2500);
    }
    void poll();
    return () => { stopped = true; clearTimeout(timer); };
  });
  async function act(action:()=>Promise<unknown>) {
    if (busy) return;
    busy = true; error = '';
    const id = editionId;
    try { await action(); const next = await app.api.headlessStatus(id); if (id === editionId) status = next; }
    catch(e) { if (id === editionId) error = asError(e).message; }
    finally { busy = false; }
  }
  function stream() {
    if (!status?.ready) { app.go('streaming'); return; }
    music.pauseForGame(); void act(()=>app.api.headlessLaunch(editionId));
  }
</script>

{#if status?.supported}
  <div class="headless-launch">
    <button class="btn glass" data-nav disabled={disabled || busy || status.busy || (!!status.game_running && !activeHere)} title={status.game_running && !activeHere ? 'Another streamed game is running. Stop it in Streaming before launching this game.' : status.reason ?? 'Private virtual display with separate persistent streaming saves. Closing the viewer keeps the game running.'} onclick={stream}>
      <Icon name="monitor" size={18}/>{busy || status.busy ? 'Preparing stream…' : activeHere ? 'Reconnect stream' : status.ready ? 'Stream' : 'Streaming setup'}
    </button>
    {#if activeHere}<button class="btn glass" data-nav disabled={busy} onclick={() => { if (confirmStop) { confirmStop=false; void act(()=>app.api.headlessStop()); } else confirmStop=true; }}>{confirmStop ? 'Confirm stop game' : 'Stop stream'}</button>{#if confirmStop}<button class="btn glass" data-nav onclick={() => confirmStop=false}>Cancel</button>{/if}{/if}
    {#if error || status.error || status.host_error}<p class="error" role="alert">{error || status.error?.message || status.host_error}</p>{/if}
  </div>
{/if}

<style>
  .headless-launch { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; }
  .error { flex-basis: 100%; max-width: 480px; margin: 0; font-size: 12px; color: var(--danger); }
</style>
