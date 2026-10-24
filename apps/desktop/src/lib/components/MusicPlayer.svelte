<script lang="ts">
  import { onMount } from 'svelte';
  import { app } from '../state.svelte';
  import { music } from '../music.svelte';
  let audio: HTMLAudioElement;
  onMount(() => {
    music.attach(audio);
    try {
      const volume = Number(localStorage.getItem('marquee.music.volume') ?? '0.65');
      audio.volume = Number.isFinite(volume) ? Math.min(1, Math.max(0, volume)) : 0.65;
      music.pauseOnGame = localStorage.getItem('marquee.music.pauseOnGame') !== 'false';
    } catch { /* storage unavailable; keep defaults */ }
    const stop = app.api.subscribe(event => { if (event.type === 'session_started') music.pauseForGame(); });
    return () => { stop(); music.detach(); };
  });
  function saveVolume() { try { localStorage.setItem('marquee.music.volume', String(audio.volume)); } catch { /* optional */ } }
  function savePreference() { try { localStorage.setItem('marquee.music.pauseOnGame', String(music.pauseOnGame)); } catch { /* optional */ } }
</script>

<section class="music-player" class:visible={!!music.current} aria-label="Soundtrack player">
  <div class="playing"><span class="record" aria-hidden="true">♫</span><div><strong>{music.current?.title ?? 'Soundtrack'}</strong><span>{music.current?.album}{music.current?.artist ? ` · ${music.current.artist}` : ''}</span></div></div>
  <div class="transport">
    <button class="btn glass small" data-nav aria-label="Previous track" disabled={music.loading || music.index === 0} onclick={() => music.next(app.api, -1)}>Previous</button>
    <audio bind:this={audio} controls preload="metadata" aria-label="Soundtrack playback, seek and volume" onended={() => music.next(app.api)} onplay={() => { music.paused = false; music.error = ''; music.note = ''; }} onpause={() => music.paused = true} onvolumechange={saveVolume} onerror={() => { music.paused = true; music.error = 'This audio could not be decoded or is no longer available. Try another format or re-add the file.'; }}></audio>
    <button class="btn glass small" data-nav aria-label="Next track" disabled={music.loading || music.index >= music.queue.length - 1} onclick={() => music.next(app.api)}>Next</button>
  </div>
  <label class="preference"><input data-nav type="checkbox" bind:checked={music.pauseOnGame} onchange={savePreference} /> Pause when a game starts</label>
  <button class="btn glass small" data-nav aria-label="Close soundtrack player" onclick={() => { music.pause(); music.queue = []; audio.removeAttribute('src'); audio.load(); }}>Close</button>
  {#if music.loading || music.error || music.note}<p class:error={!!music.error} role="status">{music.error || music.note || 'Loading audio…'}</p>{/if}
</section>

<style>
  .music-player { display: none; position: fixed; bottom: 14px; left: 74px; right: 22px; z-index: 35; padding: 16px 20px; border: 1px solid var(--line); border-radius: 18px; background: var(--surface-strong); box-shadow: 0 8px 40px #0005; align-items: center; gap: 16px; flex-wrap: wrap; }
  .visible { display: flex; } .playing { display: flex; align-items: center; gap: 12px; min-width: 140px; max-width: 320px; flex: 1; }
  .playing div { min-width: 0; } strong, span:not(.record) { display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .playing span:not(.record) { font-size: 12px; color: var(--text-dim); margin-top: 4px; }
  .record { font-size: 30px; color: var(--accent); } .transport { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
  audio { width: min(310px, 65vw); height: 40px; } .preference { font-size: 12px; display: flex; align-items: center; gap: 6px; }
  p { flex-basis: 100%; font-size: 12px; color: var(--text-dim); margin: 0; } .error { color: var(--bad); }
  @media(max-width:760px) { .music-player { left: 58px; right: 10px; padding: 12px; } }
  :global(body:has(.music-player.visible) .screen) { padding-bottom: 160px; }
</style>
