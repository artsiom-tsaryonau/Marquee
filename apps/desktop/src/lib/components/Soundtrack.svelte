<script lang="ts">
  import { onMount } from 'svelte';
  import { app } from '../state.svelte';
  import { music, trackTitle, musicTime } from '../music.svelte';
  import Icon from './Icon.svelte';
  import MusicMenu from './MusicMenu.svelte';
  import { asError } from '../api/core';
  import type { MusicAlbum } from '../api/contracts';
  let { gameKey, gameTitle }: { gameKey: string; gameTitle: string } = $props();
  let albums = $state<MusicAlbum[]>([]);
  let all = $state<MusicAlbum[]>([]);
  let busy = $state(false);
  let error = $state('');
  let note = $state('');
  let editing = $state(false);
  let albumId = $state<string | null>(null);
  let title = $state('');
  let artist = $state('');
  let source = $state('');
  let existing = $state('');
  const unlinked = $derived(all.filter(a => !albums.some(link => link.id === a.id)));
  const findUrl = $derived(`https://www.youtube.com/results?search_query=${encodeURIComponent(`${gameTitle} official soundtrack OST`)}`);
  async function refresh() { [albums, all] = await Promise.all([app.api.musicAlbums(gameKey), app.api.musicAlbums()]); }
  async function run(action: () => Promise<unknown>) {
    if (busy) return;
    busy = true; error = ''; note = '';
    try { await action(); await refresh(); } catch (err) { error = asError(err).message; } finally { busy = false; }
  }
  onMount(() => { void run(async () => {}); });
  function edit(album?: MusicAlbum) { albumId = album?.id ?? null; title = album?.title ?? `${gameTitle} — Original soundtrack`; artist = album?.artist ?? ''; source = album?.source_url ?? ''; editing = true; }
  function openSearch(event: MouseEvent) {
    if (app.api.kind === 'desktop') { event.preventDefault(); void run(() => app.api.musicFind(gameTitle)); }
  }
  function openSource(event: MouseEvent, album: MusicAlbum) {
    if (app.api.kind === 'desktop') { event.preventDefault(); void run(() => app.api.musicOpenSource(album.id)); }
  }
  async function toggleAlbum(album: MusicAlbum) {
    if (music.current?.albumId === album.id) await music.toggle(app.api);
    else { const first = album.tracks.find(t => t.available); if (first) await music.play(app.api, album, first.id); }
  }
  async function save(event: SubmitEvent) {
    event.preventDefault();
    await run(async () => { await app.api.musicSaveAlbum({ game_key: gameKey, album_id: albumId, title, artist, source_url: source.trim() }); editing = false; });
  }
  async function addFiles(album: MusicAlbum) {
    await run(async () => {
      const paths = await app.api.pickMusic(); if (!paths.length) return;
      paths.sort((a,b) => a.localeCompare(b, undefined, { numeric: true }));
      const added = await app.api.musicImport(album.id, paths);
      note = added ? `Added ${added} ${added === 1 ? 'track' : 'tracks'}.` : 'These tracks are already in this soundtrack.';
    });
  }
</script>

<section class="soundtrack" aria-label="Game original soundtracks">
  <header><div><span class="eyebrow">MUSIC FOR THIS GAME</span><h2>Soundtracks</h2><p>Original, remastered or arranged albums, linked to this game.</p></div><div class="actions"><a class="btn glass" data-nav href={findUrl} onclick={openSearch} target="_blank" rel="noreferrer">Find OST on YouTube ↗</a><button class="btn primary" data-nav disabled={busy} onclick={() => edit()}>Add soundtrack</button></div></header>
  {#if error}<p class="error" role="alert">{error}</p><button class="btn glass small" data-nav disabled={busy} onclick={() => run(async () => {})}>Retry</button>{/if}
  {#if note}<p role="status">{note}</p>{/if}
  {#if busy}<p class="hint" role="status">Updating soundtracks…</p>{/if}
  {#if editing}<form class="glass-panel editor" onsubmit={save}>
    <h3>{albumId ? 'Edit soundtrack details' : 'Add a soundtrack'}</h3>
    <label>Album title<input data-nav bind:value={title} maxlength="300" required disabled={busy} /></label>
    <label>Composer / artist<input data-nav bind:value={artist} maxlength="300" disabled={busy} /></label>
    <label>Source page (optional)<input data-nav type="url" bind:value={source} maxlength="2048" placeholder="https://…" disabled={busy} /></label>
    <p class="hint">A source link records attribution; it does not download or play that page. Shared albums keep the same details across linked games.</p>
    <div class="actions"><button class="btn primary" data-nav disabled={busy}>Save soundtrack</button><button type="button" class="btn glass" data-nav disabled={busy} onclick={() => editing = false}>Cancel</button></div>
  </form>{/if}
  {#if !albums.length && !busy}<div class="glass-panel empty"><span class="disc" aria-hidden="true">♫</span><h3>Give this game a soundtrack</h3><p>Add an album, then choose its local audio files. Music keeps playing while you browse Marquee.</p><p class="hint">MP3, FLAC, OGG and WAV are accepted for import; playback depends on the decoder available in your browser or desktop runtime.</p></div>{/if}
  {#each albums as album (album.id)}<article class="glass-panel album">
    <div class="album-head"><div class="album-art" aria-hidden="true">♫</div><div class="album-info"><span class="eyebrow">SOUNDTRACK · {album.tracks.length} TRACKS</span><h3>{album.title}</h3>{#if album.artist}<p>{album.artist}</p>{/if}{#if album.source_url}<a data-nav href={album.source_url} onclick={(event) => openSource(event, album)} target="_blank" rel="noreferrer">View source ↗</a>{/if}</div></div>
    <div class="album-actions">
      <button class="album-play" data-nav aria-label={music.current?.albumId === album.id && (!music.paused || music.loading) ? 'Pause album' : 'Play album'} title={music.current?.albumId === album.id && (!music.paused || music.loading) ? 'Pause album' : 'Play album'} disabled={!album.tracks.some(t => t.available)} onclick={() => toggleAlbum(album)}><Icon name={music.current?.albumId === album.id && (!music.paused || music.loading) ? 'pause' : 'play'} size={25} filled /></button>
      <button class="add-tracks" data-nav disabled={busy} title="Choose local audio files" onclick={() => addFiles(album)}><Icon name="plus" size={18} />Add tracks</button>
      <MusicMenu label={`Album options for ${album.title}`}><button data-nav disabled={busy} onclick={() => edit(album)}><Icon name="settings" size={16} />Edit album details</button><button data-nav disabled={busy} onclick={() => run(() => app.api.musicLink(gameKey, album.id, false))}>Unlink from this game</button></MusicMenu>
    </div>
    {#if album.tracks.length}
      <div class="list-heading" aria-hidden="true"><span>#</span><span>Title</span><span class="album-column">Album</span><span class="duration-column"><Icon name="clock" size={14} /></span></div>
      <ol class="tracks">{#each album.tracks as track, index (track.id)}
        {@const current = music.current?.id === track.id}
        {@const title = trackTitle(track.title, index + 1)}
        {@const duration = current && music.duration > 0 ? music.duration : track.duration_seconds}
        <li class:current aria-current={current ? 'true' : undefined}>
          <button class="track" data-nav disabled={!track.available} aria-label={`${current && (!music.paused || music.loading) ? 'Pause' : 'Play'} ${title}`} onclick={() => music.select(app.api, album, track.id)}>
            <span class="track-control" aria-hidden="true"><span class="ordinal">{index + 1}</span>{#if current && !music.paused && !music.loading}<span class="equalizer"><i></i><i></i><i></i></span>{/if}<span class="row-icon"><Icon name={current && (!music.paused || music.loading) ? 'pause' : 'play'} size={17} filled /></span></span>
            <span class="track-name"><span class="track-title" title={title}>{title}</span>{#if album.artist}<span class="track-artist">{album.artist}</span>{:else if !track.available}<span class="track-artist">File unavailable</span>{/if}</span>
            <span class="album-column" title={album.title}>{album.title}</span>
            <span class="duration-column">{duration && duration > 0 ? musicTime(duration) : '—'}</span>
          </button>
          <MusicMenu label={`Options for ${title}`}><button class="danger" data-nav disabled={busy} onclick={() => run(async () => { await app.api.musicRemoveTrack(album.id, track.id); music.removed(track.id); })}>Remove from album</button></MusicMenu>
        </li>
      {/each}</ol>
    {:else}<p class="hint">No audio attached. Add tracks to build this album’s track list.</p>{/if}
  </article>{/each}
  {#if unlinked.length}<form class="reuse" onsubmit={(e) => { e.preventDefault(); if (existing) void run(async () => { await app.api.musicLink(gameKey, existing, true); existing = ''; }); }}><label>Link an existing soundtrack<select data-nav bind:value={existing} disabled={busy}><option value="">Choose album</option>{#each unlinked as album}<option value={album.id}>{album.title}{album.artist ? ` · ${album.artist}` : ''}</option>{/each}</select></label><button class="btn glass" data-nav disabled={busy || !existing}>Link album</button></form>{/if}
  <p class="hint">Original files are kept intact. Removing a track or unlinking an album leaves the copied audio on disk. yt-dlp downloads, Jellyfin and automatic release matching are planned separately; source links open in your browser.</p>
</section>

<style>
  .soundtrack { display: grid; gap: 22px; max-width: 1100px; margin: 0 auto; padding: 24px var(--gutter) 40px; }
  header, .album-head { display: flex; align-items: center; gap: 22px; flex-wrap: wrap; } header { justify-content: space-between; }
  h2 { margin: 8px 0; font-size: 28px; } h3 { margin: 6px 0 12px; } p { line-height: 1.65; margin: 8px 0; color: var(--text-soft); }
  .eyebrow { color: var(--text-dim); font-size: 11px; letter-spacing: .1em; } .actions { display: flex; gap: 10px; flex-wrap: wrap; }
  .editor, .album, .empty { border-radius: 18px; padding: 24px; } .editor { display: grid; gap: 16px; max-width: 680px; }
  label { display: grid; gap: 8px; font-size: 13px; } input, select { padding: 11px; color: var(--text); background: var(--field); border: 1px solid var(--line); border-radius: 8px; min-width: 0; }
  .empty { text-align: center; padding: 44px 24px; } .disc { font-size: 48px; color: var(--music-text); }
  .album-art { width: 108px; height: 108px; border-radius: 16px; background: radial-gradient(circle, var(--bg) 9%, var(--music-accent) 10%, var(--surface) 65%); border: 1px solid var(--line); display: grid; place-items: center; font-size: 32px; }
  .album-head { margin-bottom: 24px; } .album-info { flex: 1; min-width: 180px; } .album-info h3 { font-size: 22px; } a { color: var(--text-soft); }
  .album-actions { display: flex; align-items: center; gap: 18px; }
  .album-play { width: var(--music-play-size); height: var(--music-play-size); display: grid; place-items: center; flex: 0 0 auto; border: 0; border-radius: 50%; color: var(--music-ink); background: var(--music-accent); cursor: pointer; }
  .album-play:hover:not(:disabled) { filter: brightness(1.12); transform: scale(1.04); } .album-play:disabled { opacity: .35; cursor: default; }
  .album-play:focus-visible, .add-tracks:focus-visible, .track:focus-visible { outline: 2px solid var(--music-text); outline-offset: 3px; }
  .add-tracks { display: flex; align-items: center; gap: 8px; height: var(--music-control-size); box-sizing: border-box; padding: 0 16px; border-radius: 24px; border: 1px solid var(--line); background: transparent; color: var(--text-soft); font: inherit; font-size: 13px; cursor: pointer; }
  .add-tracks:hover { color: var(--text); border-color: var(--text-dim); } .add-tracks:disabled { opacity: .4; cursor: default; }
  .list-heading { display: grid; grid-template-columns: 36px minmax(0,1fr) minmax(0,.7fr) 56px; gap: 12px; padding: 22px 58px 10px 10px; color: var(--text-dim); font-size: 12px; border-bottom: 1px solid var(--line); }
  .list-heading span:first-child { text-align: center; } .tracks { padding: 6px 0 0; margin: 0; list-style: none; }
  .tracks li { display: flex; align-items: center; gap: 8px; padding: 2px 6px; border-radius: 7px; min-height: 52px; }
  .tracks li:hover, .tracks li:focus-within { background: var(--field); } .tracks li.current { background: color-mix(in srgb, var(--music-accent) 8%, transparent); }
  .track { flex: 1; display: grid; grid-template-columns: 36px minmax(0, 1fr) minmax(0,.7fr) 56px; align-items: center; gap: 12px; min-width: 0; min-height: 48px; text-align: left; color: var(--text-soft); background: none; border: 0; padding: 4px; cursor: pointer; border-radius: 4px; font: inherit; }
  .track-control { position: relative; display: grid; place-items: center; width: 36px; height: 30px; color: var(--text-dim); font-size: 12px; font-variant-numeric: tabular-nums; }
  .row-icon { position: absolute; display: none; } .track-title { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 13px; }
  .track-name { display: grid; gap: 3px; min-width: 0; } .track-artist { font-size: 12px; color: var(--text-dim); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .album-column { color: var(--text-dim); font-size: 12px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .duration-column { display: flex; justify-content: end; color: var(--text-dim); font-size: 12px; font-variant-numeric: tabular-nums; }
  .current .track-title, .current .track-control { color: var(--music-text); }
  .equalizer { position: absolute; display: flex; align-items: end; gap: 2px; height: 15px; }
  .equalizer i { width: 3px; height: 13px; background: currentColor; transform-origin: bottom; animation: level .8s ease-in-out infinite alternate; }
  .equalizer i:nth-child(2) { height: 9px; animation-delay: -.4s; } .equalizer i:nth-child(3) { height: 15px; animation-delay: -.2s; }
  .track-control:has(.equalizer) .row-icon { display: none; }
  .tracks li:hover .equalizer, .tracks li:focus-within .equalizer { display: none; }
  .tracks li:hover .row-icon, .tracks li:focus-within .row-icon { display: flex; }
  @keyframes level { to { transform: scaleY(.35); } }
  .tracks li:hover .row-icon, .tracks li:focus-within .row-icon, .current .row-icon { display: flex; }
  .tracks li:hover .ordinal, .tracks li:focus-within .ordinal, .current .ordinal { visibility: hidden; }
  .tracks li :global(.music-menu > summary) { opacity: 0; }
  .tracks li:hover :global(.music-menu > summary), .tracks li:focus-within :global(.music-menu > summary), .tracks li :global(.music-menu[open] > summary) { opacity: 1; }
  .track:disabled { opacity: .45; cursor: default; }
  @media(hover:none) { .row-icon { display: flex; } .ordinal { visibility: hidden; } .tracks li :global(.music-menu > summary) { opacity: 1; } }
  @media(max-width:700px) { .album-column { display: none; } .track, .list-heading { grid-template-columns: 36px minmax(0,1fr) 56px; } }
  @media(prefers-reduced-motion:reduce) { .album-play:hover:not(:disabled) { transform: none; } }
  :global(:root[data-motion='reduce']) .album-play:hover:not(:disabled) { transform: none; }
  @media(prefers-reduced-motion:reduce) { .equalizer i { animation: none; } }
  :global(:root[data-motion='reduce']) .equalizer i { animation: none; }
  :global(:root[data-idle='yes']) .equalizer i { animation-play-state: paused; }
  .actions .btn { height: var(--music-control-size); box-sizing: border-box; }
  .actions .btn.primary { color: var(--music-ink); background: var(--music-accent); box-shadow: none; border-radius: 24px; }
  .hint { color: var(--text-dim); font-size: 12px; } .error { color: var(--bad); } .reuse { display: flex; gap: 14px; flex-wrap: wrap; align-items: end; }
</style>
