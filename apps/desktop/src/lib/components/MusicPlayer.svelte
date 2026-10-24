<script lang="ts">
  import { onMount } from 'svelte';
  import { app } from '../state.svelte';
  import { music, musicTime } from '../music.svelte';
  import Icon from './Icon.svelte';
  import MusicMenu from './MusicMenu.svelte';
  let audio: HTMLAudioElement;
  const progress = $derived(music.duration > 0 ? Math.min(100, music.position / music.duration * 100) : 0);
  onMount(() => {
    try {
      const volume = Number(localStorage.getItem('marquee.music.volume') ?? '0.65');
      music.volume = Number.isFinite(volume) ? Math.min(1, Math.max(0, volume)) : 0.65;
      music.pauseOnGame = localStorage.getItem('marquee.music.pauseOnGame') !== 'false';
    } catch { /* keep defaults when storage is unavailable */ }
    music.attach(audio);
    const stop = app.api.subscribe(event => { if (event.type === 'session_started') music.pauseForGame(); });
    return () => { stop(); music.detach(); };
  });
  function saveVolume() {
    music.volume = audio.volume; music.muted = audio.muted;
    try { localStorage.setItem('marquee.music.volume', String(audio.volume)); } catch { /* optional */ }
  }
  function savePreference() { try { localStorage.setItem('marquee.music.pauseOnGame', String(music.pauseOnGame)); } catch { /* optional */ } }
</script>

<audio bind:this={audio} hidden preload="metadata" onended={() => music.next(app.api)} ontimeupdate={() => music.syncTime()} ondurationchange={() => music.syncTime()} onloadedmetadata={() => music.syncTime()} onplay={() => { music.paused = false; music.error = ''; music.note = ''; }} onpause={() => music.paused = true} onvolumechange={saveVolume} onerror={() => { music.paused = true; music.error = 'This audio could not be decoded or is no longer available. Try another format or re-add the file.'; }}></audio>

{#if music.current}
<section class="music-player" aria-label="Soundtrack player">
  <div class="playing">
    <span class="record" aria-hidden="true"><Icon name="music" size={24} /></span>
    <div class="metadata"><strong title={music.current.title}>{music.current.title}</strong><span title={music.current.album}>{music.current.artist || music.current.album}</span></div>
  </div>
  <div class="center">
    <div class="transport">
      <button class="control" data-nav aria-label="Previous track or restart" title="Previous track · restart after 3 seconds" disabled={music.loading} onclick={() => music.previous(app.api)}><Icon name="previous" size={20} /></button>
      <button class="control primary-play" class:loading={music.loading} data-nav aria-label={music.loading ? 'Cancel loading' : music.paused ? 'Play soundtrack' : 'Pause soundtrack'} title={music.loading ? 'Cancel loading' : music.paused ? 'Play' : 'Pause'} onclick={() => music.toggle(app.api)}><Icon name={music.loading || !music.paused ? 'pause' : 'play'} size={21} filled /></button>
      <button class="control" data-nav aria-label="Next track" title="Next track" disabled={music.loading || music.index >= music.queue.length - 1} onclick={() => music.next(app.api)}><Icon name="next" size={20} /></button>
    </div>
    <div class="timeline">
      <span aria-hidden="true">{musicTime(music.position)}</span>
      <input class="seek" data-nav type="range" aria-label="Seek in soundtrack" aria-valuetext={`${musicTime(music.position)} of ${musicTime(music.duration)}`} min="0" max={music.duration || 0} step="1" value={music.position} disabled={!music.duration || music.loading} style={`--filled:${progress}%`} oninput={(event) => music.seek(Number(event.currentTarget.value))} />
      <span aria-hidden="true">{music.duration ? musicTime(music.duration) : '--:--'}</span>
    </div>
  </div>
  <div class="right">
    <div class="output" title="Music plays on this device. It is not automatically included in a Wolf game stream."><Icon name="monitor" size={14} /><span>This device</span></div>
    <div class="utilities">
      <button class="control volume-button" data-nav aria-label={music.muted || music.volume === 0 ? 'Unmute soundtrack' : 'Mute soundtrack'} title={music.muted || music.volume === 0 ? 'Unmute' : 'Mute'} aria-pressed={music.muted || music.volume === 0} onclick={() => music.toggleMute()}><Icon name={music.muted || music.volume === 0 ? 'muted' : 'volume'} size={18} /></button>
      <input class="volume" data-nav type="range" aria-label="Soundtrack volume" aria-valuetext={music.muted ? 'Muted' : `${Math.round(music.volume * 100)} percent`} min="0" max="1" step="0.01" value={music.muted ? 0 : music.volume} style={`--filled:${music.muted ? 0 : music.volume * 100}%`} oninput={(event) => music.setVolume(Number(event.currentTarget.value))} />
      <MusicMenu label="Playback options"><label><input data-nav type="checkbox" bind:checked={music.pauseOnGame} onchange={savePreference} /> Pause when a game starts</label><button data-nav onclick={() => { music.pause(); music.queue = []; audio.removeAttribute('src'); audio.load(); }}>Close player</button></MusicMenu>
    </div>
  </div>
  {#if music.loading || music.error || music.note}<p class="message" class:error={!!music.error} role="status">{music.error || music.note || 'Loading audio…'}</p>{/if}
</section>
{/if}

<style>
  .music-player { position: fixed; bottom: 14px; left: 68px; right: 18px; z-index: 35; display: grid; grid-template-columns: minmax(150px, 1fr) minmax(250px, 1.4fr) minmax(170px, 1fr); gap: 18px; align-items: center; padding: 14px 20px; border: 1px solid var(--line); border-radius: 18px; background: var(--surface-strong); box-shadow: 0 8px 40px #0004; }
  .playing { display: flex; align-items: center; gap: 12px; min-width: 0; } .metadata { min-width: 0; }
  strong, .metadata span { display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; } strong { font-size: 13px; font-weight: 650; }
  .metadata span { font-size: 12px; color: var(--text-dim); margin-top: 5px; }
  .record { flex: 0 0 auto; display: grid; place-items: center; width: 48px; height: 48px; border: 1px solid var(--line); border-radius: 10px; color: var(--text-soft); background: radial-gradient(circle at 35% 25%, color-mix(in srgb, var(--accent) 35%, transparent), var(--surface)); }
  .center { display: grid; gap: 8px; min-width: 0; } .transport { display: flex; align-items: center; justify-content: center; gap: 18px; }
  .control { flex: 0 0 auto; display: grid; place-items: center; width: 36px; height: 36px; padding: 0; border: 0; border-radius: 50%; background: transparent; color: var(--text-dim); cursor: pointer; }
  .control:hover:not(:disabled) { color: var(--text); background: var(--field); } .control:disabled { opacity: .3; cursor: default; }
  .control:focus-visible { outline: 2px solid var(--accent); outline-offset: 3px; } .primary-play { width: 40px; height: 40px; color: var(--bg); background: var(--text); }
  .primary-play:hover:not(:disabled) { color: var(--bg); background: var(--text); transform: scale(1.05); } .loading { box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 60%, transparent); }
  .timeline { display: flex; align-items: center; gap: 10px; } .timeline span { min-width: 34px; color: var(--text-dim); font-size: 11px; font-variant-numeric: tabular-nums; text-align: center; }
  input[type='range'] { appearance: none; -webkit-appearance: none; min-width: 0; height: 24px; padding: 0; margin: 0; background: transparent; cursor: pointer; }
  input[type='range']::-webkit-slider-runnable-track { height: 4px; border-radius: 4px; background: linear-gradient(to right, var(--text-soft) 0 var(--filled), var(--field-strong) var(--filled) 100%); }
  input[type='range']::-webkit-slider-thumb { -webkit-appearance: none; width: 10px; height: 10px; margin-top: -3px; border-radius: 50%; background: var(--text); }
  input[type='range']::-moz-range-track { height: 4px; border-radius: 4px; background: var(--field-strong); } input[type='range']::-moz-range-progress { height: 4px; background: var(--text-soft); }
  input[type='range']::-moz-range-thumb { width: 10px; height: 10px; border: 0; border-radius: 50%; background: var(--text); }
  input[type='range']:focus-visible { outline: 2px solid var(--accent); outline-offset: 3px; border-radius: 4px; } input:disabled { opacity: .35; cursor: default; }
  .seek { width: 100%; } .right { display: grid; gap: 2px; justify-items: end; } .output { display: flex; gap: 6px; align-items: center; font-size: 12px; color: var(--text-dim); }
  .utilities { display: flex; align-items: center; gap: 5px; } .volume { width: 80px; } .message { grid-column: 1 / -1; margin: 0; font-size: 12px; color: var(--text-dim); } .error { color: var(--bad); }
  @media(max-width:900px) { .music-player { grid-template-columns: minmax(110px, 1fr) minmax(210px, 1.4fr); } .right { grid-column: 1 / -1; display: flex; align-items: center; justify-content: space-between; width: 100%; } }
  @media(max-width:560px) { .music-player { left: 58px; right: 8px; padding: 12px; gap: 12px; grid-template-columns: 1fr; } .record { width: 36px; height: 36px; } }
  :global(body:has(.music-player) .screen) { padding-bottom: 170px; }
  :global(:root[data-mode='big']) .music-player { bottom: 52px; left: 24px; right: 24px; }
  @media(pointer:coarse) { .control { width: 44px; height: 44px; } input[type='range'] { height: 36px; } }
  @media(prefers-reduced-motion:reduce) { .primary-play:hover:not(:disabled) { transform: none; } }
  :global(:root[data-motion='reduce']) .primary-play:hover:not(:disabled) { transform: none; }
</style>
