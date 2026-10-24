<script lang="ts">
  import { untrack } from 'svelte';
  import { app } from '../state.svelte';
  import { asError } from '../api/core';
  import type { YoutubeSource, YoutubePlaylist } from '../api/contracts';
  import { musicTime } from '../music.svelte';
  import Icon from './Icon.svelte';
  let { gameKey = '', gameTitle = '', onlinked }: { gameKey?: string; gameTitle?: string; onlinked: () => Promise<void> } = $props();
  let query = $state(untrack(() => `${gameTitle} soundtrack`.trim()));
  let kind = $state<'playlist'|'video'>('playlist');
  let results = $state<YoutubeSource[]>([]);
  let preview = $state<YoutubePlaylist | null>(null);
  let busy = $state(false);
  let searched = $state(false);
  let error = $state('');
  let note = $state('');
  async function run(action: () => Promise<void>) {
    if (busy) return;
    busy = true; error = ''; note = '';
    try { await action(); } catch (e) { error = asError(e).message; }
    finally { busy = false; }
  }
  async function search(event: SubmitEvent) {
    event.preventDefault();
    await run(async () => { results = []; preview = null; searched = false; results = await app.api.musicSearchYoutube(query, kind); searched = true; });
  }
  function open(event: MouseEvent, url: string) {
    if (app.api.kind === 'desktop') { event.preventDefault(); void run(() => app.api.musicOpenYoutube(url)); }
  }
  async function link(source: {url:string;title:string}) {
    await run(async () => {
      await app.api.musicLinkYoutube(gameKey, source.url, source.title);
      await onlinked();
      note = 'Soundtrack source linked. Add local audio tracks to play this album in Marquee.';
    });
  }
</script>
<section class="youtube-search glass-panel" aria-label="Find soundtrack sources">
  <h3>Find a soundtrack</h3>
  <p>Search YouTube playlists or full-album videos. Preview a playlist and link its source to this game. Audio downloads and remote playback are not included.</p>
  <form onsubmit={search}>
    <label class="query">Search<input data-nav bind:value={query} minlength="2" maxlength="200" required disabled={busy} /></label>
    <label>Source type<select data-nav bind:value={kind} disabled={busy} onchange={() => { results = []; preview = null; searched = false; }}><option value="playlist">Playlists</option><option value="video">Videos</option></select></label>
    <button class="btn glass" data-nav disabled={busy || query.trim().length < 2}><Icon name="search" size={16}/>{busy ? 'Working…' : 'Search'}</button>
  </form>
  <p class="hint">The first search installs a verified yt-dlp helper. Searches use no browser cookies.</p>
  {#if error}<p class="error" role="alert">{error}</p>{/if}
  {#if note}<p role="status">{note}</p>{/if}
  {#if searched && !results.length}<p>No sources found. Try a shorter title or the YouTube browser link.</p>{/if}
  {#if results.length}<ul>{#each results as result (result.id)}
    <li>
      <span class="source-icon" aria-hidden="true"><Icon name="music" size={22}/></span>
      <div class="source-info"><strong>{result.title}</strong><small>{result.uploader ? `Uploader: ${result.uploader}` : 'Uploader not supplied'}{result.duration ? ` · ${musicTime(result.duration)}` : ''}</small></div>
      <div class="actions">
        <a class="btn glass small" data-nav href={result.url} target="_blank" rel="noreferrer" onclick={(event) => open(event, result.url)}>YouTube ↗</a>
        {#if result.kind === 'playlist'}<button class="btn glass small" data-nav disabled={busy} onclick={() => run(async () => { preview = await app.api.musicInspectYoutube(result.url); })}>Preview tracks</button>{/if}
        <button class="btn glass small" data-nav disabled={busy} onclick={() => link(result)}>Link source</button>
      </div>
    </li>
  {/each}</ul>{/if}
  {#if preview}
    <section class="playlist-preview" aria-label="Playlist preview">
      <header><div><h4>{preview.title}</h4><p>{preview.tracks.length} entries shown{preview.truncated ? ' · preview limited to 50 tracks' : ''}{preview.uploader ? ` · Uploader: ${preview.uploader}` : ''}</p></div><button class="btn glass small" data-nav disabled={busy} onclick={() => preview = null}>Close preview</button></header>
      <p class="hint">Uploader names identify the YouTube account, not necessarily the composer. Track availability and completeness are not verified.</p>
      <ol>{#each preview.tracks as track}<li><span>{track.title}</span><small>{track.duration ? musicTime(track.duration) : '—'}</small></li>{/each}</ol>
    </section>
  {/if}
</section>
<style>
  .youtube-search { padding: 20px; border-radius: 10px; }
  h3 { margin: 0 0 10px; font-size: 18px; } h4 { margin: 0; font-size: 15px; }
  p { color: var(--text-soft); font-size: 13px; line-height: 1.6; }
  .hint, small { color: var(--text-dim); font-size: 11px; }
  form { display: flex; gap: 10px; align-items: end; flex-wrap: wrap; }
  label { display: grid; gap: 6px; font-size: 12px; min-width: 0; } .query { flex: 1; }
  input, select { box-sizing: border-box; width: 100%; min-width: 0; font: inherit; padding: 10px; color: var(--text); background: var(--field); border: 1px solid var(--line); border-radius: 7px; }
  ul { list-style: none; padding: 0; } ul > li { display: flex; align-items: center; gap: 12px; padding: 14px 0; border-top: 1px solid var(--line); }
  .source-info { flex: 1; min-width: 0; display: grid; gap: 5px; } strong { font-size: 13px; overflow-wrap: anywhere; }
  .source-icon { width: 38px; height: 38px; display: grid; place-items: center; background: var(--field); border-radius: 6px; color: var(--music-accent, #63dec1); }
  .actions { display: flex; gap: 6px; flex-wrap: wrap; } .error { color: var(--danger); }
  .playlist-preview { border-top: 1px solid var(--line); padding-top: 16px; }
  header { display: flex; justify-content: space-between; align-items: start; gap: 12px; }
  ol { padding-left: 28px; } ol li { padding: 7px 0; font-size: 12px; } ol li span { display: inline-block; width: calc(100% - 55px); vertical-align: top; overflow-wrap: anywhere; } ol small { display: inline-block; width: 48px; text-align: right; font-variant-numeric: tabular-nums; }
  @media (max-width: 700px) { ul > li { flex-wrap: wrap; } .actions { width: 100%; } .query { flex-basis: 100%; } }
</style>
