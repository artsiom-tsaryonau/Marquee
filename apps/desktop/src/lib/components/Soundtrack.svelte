<script lang="ts">
  import { onMount } from 'svelte';
  import { app } from '../state.svelte';
  import { music } from '../music.svelte';
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
    <div class="actions"><button class="btn primary" data-nav disabled={!album.tracks.some(t => t.available)} onclick={() => { const first = album.tracks.find(t => t.available); if (first) void music.play(app.api, album, first.id); }}>Play album</button><button class="btn glass" data-nav disabled={busy} onclick={() => addFiles(album)}>Choose local files</button><button class="btn glass small" data-nav disabled={busy} onclick={() => edit(album)}>Edit details</button><button class="btn glass small" data-nav disabled={busy} onclick={() => run(() => app.api.musicLink(gameKey, album.id, false))}>Unlink from game</button></div>
    {#if album.tracks.length}<ol class="tracks">{#each album.tracks as track (track.id)}<li class:current={music.current?.id === track.id}><button class="track" data-nav disabled={!track.available} onclick={() => music.play(app.api, album, track.id)}><span>{track.title}</span><span class="hint">{track.available ? music.current?.id === track.id ? music.paused ? 'Paused' : 'Playing' : 'Play' : 'File unavailable'}</span></button><button class="btn glass small" data-nav disabled={busy} aria-label={`Remove ${track.title} from this album`} onclick={() => run(async () => { await app.api.musicRemoveTrack(album.id, track.id); music.removed(track.id); })}>Remove</button></li>{/each}</ol>{:else}<p class="hint">No audio attached. Choose local files to build this album’s track list.</p>{/if}
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
  .empty { text-align: center; padding: 44px 24px; } .disc { font-size: 48px; color: var(--accent); }
  .album-art { width: 108px; height: 108px; border-radius: 16px; background: radial-gradient(circle, var(--bg) 9%, var(--accent) 10%, var(--surface) 65%); border: 1px solid var(--line); display: grid; place-items: center; font-size: 32px; }
  .album-head { margin-bottom: 24px; } .album-info { flex: 1; min-width: 180px; } .album-info h3 { font-size: 22px; } a { color: var(--text-soft); }
  .tracks { padding-left: 28px; margin-bottom: 0; } li { padding: 8px 0; border-top: 1px solid var(--line); } li::marker { font-size: 12px; color: var(--text-dim); }
  .track { display: inline-flex; align-items: center; justify-content: space-between; gap: 18px; width: calc(100% - 100px); text-align: left; color: inherit; background: none; border: 0; padding: 12px 10px; cursor: pointer; vertical-align: middle; }
  .track > span:first-child { overflow-wrap: anywhere; } .current .track { color: var(--accent); } .track:disabled { opacity: .5; cursor: default; }
  .hint { color: var(--text-dim); font-size: 12px; } .error { color: var(--bad); } .reuse { display: flex; gap: 14px; flex-wrap: wrap; align-items: end; }
</style>
