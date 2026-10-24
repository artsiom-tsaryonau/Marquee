<script lang="ts">
  import { onMount } from 'svelte';
  import { music, musicTime } from '../music.svelte';
  import { app } from '../state.svelte';
  import Icon from './Icon.svelte';
  let close: HTMLButtonElement;
  let panel: HTMLElement;
  onMount(() => {
    const previous = document.activeElement;
    close.focus();
    const outside = (event: PointerEvent) => { if (event.target instanceof Element && !panel.contains(event.target) && !event.target.closest('[data-queue-toggle]')) music.queueOpen = false; };
    document.addEventListener('pointerdown', outside);
    return () => { document.removeEventListener('pointerdown', outside); if (previous instanceof HTMLElement && previous.isConnected) previous.focus(); };
  });
</script>

<aside bind:this={panel} class="queue-panel" aria-label="Playback queue">
  <header><h2>Queue</h2><button bind:this={close} data-nav aria-label="Close queue" onclick={() => music.queueOpen = false}><Icon name="x" size={20} /></button></header>
  <p class="hint">{music.queue.length - music.index - 1} tracks coming up · This device</p>
  <ol>{#each music.queue as track, index}
    {#if index >= music.index}<li class:current={index === music.index}>
      <button class="queued-track" data-nav aria-current={index === music.index ? 'true' : undefined} onclick={() => music.selectQueued(app.api, index)}><span class="label">{index === music.index ? 'Now selected' : 'Up next'}</span><strong>{track.title}</strong><span>{track.artist || track.album}{track.duration_seconds ? ` · ${musicTime(track.duration_seconds)}` : ''}</span></button>
      {#if index > music.index}<div class="queue-actions"><button data-nav aria-label={`Move ${track.title} earlier`} disabled={index === music.index + 1} onclick={() => music.moveQueued(index, -1)}>↑</button><button data-nav aria-label={`Move ${track.title} later`} disabled={index === music.queue.length - 1} onclick={() => music.moveQueued(index, 1)}>↓</button><button data-nav aria-label={`Remove ${track.title} from queue`} onclick={() => music.removeQueued(index)}><Icon name="x" size={15} /></button></div>{/if}
    </li>{/if}
  {/each}</ol>
  {#if music.index === music.queue.length - 1}<p class="hint">Nothing else queued. Use a track’s options to play next or add to queue.</p>{/if}
</aside>

<style>
  .queue-panel { position: fixed; z-index: 34; top: auto; max-height: calc(100dvh - var(--player-height,0px) - 84px); bottom: calc(var(--player-height,0px) + 12px); right: 12px; width: min(360px,calc(100vw - 112px)); overflow: auto; padding: 20px; box-sizing: border-box; background: var(--surface-strong); border: 1px solid var(--line); border-radius: 14px; box-shadow: 0 8px 30px #0005; }
  header { display: flex; align-items: center; justify-content: space-between; position: sticky; top: -20px; margin: -20px -20px 0; padding: 16px 20px; background: var(--surface-strong); z-index: 1; } h2 { margin: 0; font-size: 20px; }
  button { color: var(--text-soft); background: transparent; border: 0; cursor: pointer; font: inherit; border-radius: 6px; } button:hover { background: var(--field); } button:disabled { opacity: .3; cursor: default; }
  header button, .queue-actions button { display: grid; place-items: center; min-width: 36px; min-height: 36px; }
  .hint, .queued-track span { color: var(--text-dim); font-size: 12px; line-height: 1.6; }
  ol { padding: 0; list-style: none; } li { padding: 8px 0; border-bottom: 1px solid var(--line); }
  .queued-track { width: 100%; display: grid; gap: 4px; text-align: left; padding: 8px; } strong { font-size: 13px; } .current strong { color: var(--music-text); }
  .queued-track .label { font-size: 10px; text-transform: uppercase; letter-spacing: .06em; }
  .queue-actions { display: flex; justify-content: end; }
  :global([data-mode='big']) .queue-panel { bottom: calc(var(--player-height,0px) + 64px); }
  @media(max-width:640px) { .queue-panel { width: calc(100vw - 24px); bottom: calc(var(--player-height,0px) + 84px); } }
</style>
