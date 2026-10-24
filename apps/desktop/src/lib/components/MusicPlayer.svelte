<script lang="ts">
  import { onMount } from 'svelte';
  import { app } from '../state.svelte';
  import { music, musicTime } from '../music.svelte';
  import Icon from './Icon.svelte';
  import MusicMenu from './MusicMenu.svelte';
  import MusicQueue from './MusicQueue.svelte';
  let audio: HTMLAudioElement;
  function measurePlayer(node: HTMLElement) {
    const update = () => document.documentElement.style.setProperty('--player-height', `${node.getBoundingClientRect().height}px`);
    update();
    const observer = new ResizeObserver(update); observer.observe(node);
    return { destroy() { observer.disconnect(); document.documentElement.style.setProperty('--player-height', '0px'); } };
  }
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
<section class="music-player" use:measurePlayer aria-label="Soundtrack player">
  <div class="playing">
    <span class="record" title={music.current.artworkSource || undefined} aria-hidden="true">{#if music.current.artwork}<img src={music.current.artwork} alt="" />{:else}<Icon name="music" size={24} />{/if}</span>
    <div class="metadata"><strong title={music.current.title}>{music.current.title}</strong><span title={music.current.album}>{music.current.artist || music.current.album}</span></div>
  </div>
  <div class="center">
    <div class="transport">
      <button class="control" data-nav aria-label="Previous track or restart" title="Previous track · restart after 3 seconds" disabled={music.loading} onclick={() => music.previous(app.api)}><Icon name="previous" size={18} filled /></button>
      <button class="control primary-play" class:loading={music.loading} data-nav aria-label={music.loading ? 'Cancel loading' : music.paused ? 'Play soundtrack' : 'Pause soundtrack'} title={music.loading ? 'Cancel loading' : music.paused ? 'Play' : 'Pause'} onclick={() => music.toggle(app.api)}><Icon name={music.loading || !music.paused ? 'pause' : 'play'} size={18} filled /></button>
      <button class="control" data-nav aria-label="Next track" title="Next track" disabled={music.loading || music.index >= music.queue.length - 1} onclick={() => music.next(app.api)}><Icon name="next" size={18} filled /></button>
    </div>
    <div class="timeline">
      <span aria-hidden="true">{musicTime(music.position)}</span>
      <input class="seek" data-nav type="range" aria-label="Seek in soundtrack" aria-valuetext={`${musicTime(music.position)} of ${musicTime(music.duration)}`} min="0" max={music.duration || 0} step="1" value={music.position} disabled={!music.duration || music.loading} style={`--filled:${progress}%`} oninput={(event) => music.seek(Number(event.currentTarget.value))} />
      <span aria-hidden="true">{music.duration ? musicTime(music.duration) : '--:--'}</span>
    </div>
  </div>
  <div class="right" aria-label="Playback utilities">
    <button class="control" class:active={music.queueOpen} data-nav data-queue-toggle aria-label="Show playback queue" title="Queue" aria-pressed={music.queueOpen} onclick={() => music.queueOpen = !music.queueOpen}><Icon name="queue" size={19} /></button>
    <MusicMenu label="Soundtrack volume" icon={music.muted || music.volume === 0 ? 'muted' : 'volume'} closeOnAction={false}>
      <div class="volume-popup"><strong>Volume <span>{music.muted ? 'Muted' : `${Math.round(music.volume * 100)}%`}</span></strong>
        <input class="volume" data-nav type="range" aria-label="Soundtrack volume" aria-valuetext={music.muted ? 'Muted' : `${Math.round(music.volume * 100)} percent`} min="0" max="1" step="0.01" value={music.muted ? 0 : music.volume} style={`--filled:${music.muted ? 0 : music.volume * 100}%`} oninput={(event) => music.setVolume(Number(event.currentTarget.value))} />
      </div>
      <button data-nav onclick={() => music.toggleMute()}><Icon name={music.muted || music.volume === 0 ? 'volume' : 'muted'} size={17} />{music.muted || music.volume === 0 ? 'Unmute' : 'Mute'}</button>
    </MusicMenu>
    <MusicMenu label="Playback device" icon="monitor" text="This device"><div class="device-popup"><strong>This device</strong><p>Music plays on this computer. No remote music device is connected.</p></div></MusicMenu>
    <MusicMenu label="Playback options"><label><input data-nav type="checkbox" bind:checked={music.pauseOnGame} onchange={savePreference} /> Pause when a game starts</label><button data-nav onclick={() => { music.pause(); music.queue = []; audio.removeAttribute('src'); audio.load(); }}>Close player</button></MusicMenu>
  </div>
  {#if music.loading || music.error || music.note}<p class="message" class:error={!!music.error} role="status">{music.error || music.note || 'Loading audio…'}</p>{/if}
</section>
{#if music.queueOpen}<MusicQueue />{/if}
{/if}

<style>
  .record img { width: 100%; height: 100%; object-fit: contain; border-radius: inherit; }
  .music-player { position: fixed; bottom: 0; left: 0; right: 0; z-index: 35; display: grid; grid-template-columns: minmax(150px, 1fr) minmax(250px, 1.4fr) max-content; gap: 16px; align-items: center; padding: 8px 16px; border-top: 1px solid var(--line); background: var(--surface-strong); }
  .control.active { color: var(--music-text); }
  .playing { display: flex; align-items: center; gap: 12px; min-width: 0; } .metadata { min-width: 0; }
  strong, .metadata span { display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; } strong { font-size: 13px; font-weight: 650; }
  .metadata span { font-size: 12px; color: var(--text-dim); margin-top: 5px; }
  .record { flex: 0 0 auto; display: grid; place-items: center; width: 48px; height: 48px; border: 1px solid var(--line); border-radius: 10px; color: var(--text-soft); background: radial-gradient(circle at 35% 25%, color-mix(in srgb, var(--music-accent) 35%, transparent), var(--surface)); }
  .center { display: grid; gap: 2px; min-width: 0; } .transport { display: flex; align-items: center; justify-content: center; gap: 18px; }
  .control { flex: 0 0 auto; display: grid; place-items: center; width: var(--music-control-size); height: var(--music-control-size); padding: 0; border: 0; border-radius: 50%; background: transparent; color: var(--text-dim); cursor: pointer; }
  .control:hover:not(:disabled) { color: var(--text); background: var(--field); } .control:disabled { opacity: .3; cursor: default; }
  .control:focus-visible { outline: 2px solid var(--music-text); outline-offset: 3px; } .primary-play { width: 32px; height: 32px; color: var(--music-ink); background: var(--music-accent); }
  .transport { min-height: 40px; gap: 16px; }
  :global(:root[data-mode='big']) .transport .control { width: 48px; height: 48px; }
  .primary-play:hover:not(:disabled) { color: var(--music-ink); background: var(--music-accent); transform: scale(1.05); } .loading { box-shadow: 0 0 0 3px color-mix(in srgb, var(--music-accent) 60%, transparent); }
  .timeline { display: flex; align-items: center; gap: 10px; } .timeline span { min-width: 34px; color: var(--text-dim); font-size: 11px; font-variant-numeric: tabular-nums; text-align: center; }
  input[type='range'] { appearance: none; -webkit-appearance: none; min-width: 0; height: 24px; padding: 0; margin: 0; background: transparent; cursor: pointer; }
  input[type='range']::-webkit-slider-runnable-track { height: 4px; border-radius: 4px; background: linear-gradient(to right, var(--text-soft) 0 var(--filled), var(--field-strong) var(--filled) 100%); }
  input[type='range']::-webkit-slider-thumb { -webkit-appearance: none; width: 10px; height: 10px; margin-top: -3px; border-radius: 50%; background: var(--text); }
  input[type='range']::-moz-range-track { height: 4px; border-radius: 4px; background: var(--field-strong); } input[type='range']::-moz-range-progress { height: 4px; background: var(--text-soft); }
  input[type='range']::-moz-range-thumb { width: 10px; height: 10px; border: 0; border-radius: 50%; background: var(--text); }
  input[type='range']:focus-visible { outline: 2px solid var(--music-text); outline-offset: 3px; border-radius: 4px; } input:disabled { opacity: .35; cursor: default; }
  .seek { width: 100%; } .right { display: flex; align-items: center; justify-content: end; gap: 4px; }
  .volume-popup, .device-popup { padding: 10px 12px; } .volume-popup strong { display: flex; justify-content: space-between; gap: 20px; } .volume-popup span { color: var(--text-dim); }
  .volume-popup .volume { width: 100%; margin-top: 8px; } .device-popup p { font-size: 12px; color: var(--text-dim); line-height: 1.6; margin: 8px 0 0; max-width: 230px; }
  .message { grid-column: 1 / -1; margin: 0; font-size: 12px; color: var(--text-dim); } .error { color: var(--bad); }
  @media(max-width:900px) { .music-player { grid-template-columns: minmax(110px,1fr) minmax(190px,1.4fr) max-content; gap: 10px; } }
  @media(max-width:640px) { .music-player { grid-template-columns: minmax(0,1fr) max-content; gap: 8px 12px; padding: 8px 12px; } .center { grid-row: 2; grid-column: 1 / -1; } .right { grid-column: 2; grid-row: 1; } .record { width: 36px; height: 36px; } .playing { gap: 8px; } }
  @media(pointer:coarse) { .control { width: 44px; height: 44px; } input[type='range'] { height: 36px; } }
  @media(prefers-reduced-motion:reduce) { .primary-play:hover:not(:disabled) { transform: none; } }
  :global(:root[data-motion='reduce']) .primary-play:hover:not(:disabled) { transform: none; }
</style>
