<script lang="ts">
  import { flip } from 'svelte/animate';
  import { scale } from 'svelte/transition';
  import { app } from '../state.svelte';
  import { platformLabel, sourceLabel } from '../labels';
  import { fuzzy } from '../format';
  import type { Source } from '../types';
  import Cover from '../components/Cover.svelte';
  import Icon from '../components/Icon.svelte';

  let { collection = null }: { collection?: 'ps2' | null } = $props();
  let query = $state('');
  let sources = $state<Set<Source>>(new Set());
  let installedOnly = $state(false);
  let favOnly = $state(false);
  let sort = $state<'title' | 'recent' | 'playtime' | 'added' | 'critic' | 'short'>('title');
  let search: HTMLInputElement;

  const allSources = Object.keys(sourceLabel) as Source[];

  const shown = $derived.by(() => {
    let list = app.games.filter(
      (g) =>
        (!collection || g.platform === collection) &&
        (!sources.size || sources.has(g.source)) &&
        (!installedOnly || g.installed) &&
        (!favOnly || g.favorite),
    );
    if (query.trim()) {
      return list
        .map((g) => ({ g, s: Math.max(fuzzy(query, g.title), fuzzy(query, `${g.genre} ${platformLabel[g.platform]}`) - 5) }))
        .filter((x) => x.s >= 0)
        .sort((a, b) => b.s - a.s)
        .map((x) => x.g);
    }
    const by = {
      title: (a: typeof list[0], b: typeof list[0]) => a.title.localeCompare(b.title),
      recent: (a: typeof list[0], b: typeof list[0]) => (b.lastPlayed ?? '').localeCompare(a.lastPlayed ?? ''),
      playtime: (a: typeof list[0], b: typeof list[0]) => b.playtimeMin - a.playtimeMin,
      added: (a: typeof list[0], b: typeof list[0]) => b.addedAt.localeCompare(a.addedAt),
      critic: (a: typeof list[0], b: typeof list[0]) => (b.ratings.critic ?? -1) - (a.ratings.critic ?? -1),
      short: (a: typeof list[0], b: typeof list[0]) => (a.timeToBeat?.[0] ?? 999) - (b.timeToBeat?.[0] ?? 999),
    }[sort];
    return [...list].sort(by);
  });

  function toggle(s: Source) {
    const next = new Set(sources);
    if (next.has(s)) next.delete(s);
    else next.add(s);
    sources = next;
  }

  // Typing anywhere jumps into search, like a console launcher.
  function onKey(e: KeyboardEvent) {
    if (e.key.length === 1 && !e.ctrlKey && !e.metaKey && !e.altKey && document.activeElement !== search) {
      if ((document.activeElement as HTMLElement)?.tagName === 'INPUT') return;
      search.focus();
    }
  }

  const dur = $derived(app.reduceMotion ? 0 : 260);
</script>

<svelte:window onkeydown={onKey} />

<div class="library">
  <div class="bar glass-panel">
    <label class="search">
      <Icon name="search" size={18} />
      <input bind:this={search} bind:value={query} data-nav placeholder="Search {collection === 'ps2' ? 'PS2' : app.games.length} games…" spellcheck="false" />
      {#if query}<button class="clear" onclick={() => (query = '')} aria-label="Clear"><Icon name="x" size={16} /></button>{/if}
    </label>
    <div class="chips" data-nav-row>
      {#each allSources as s (s)}
        <button class="chip" class:on={sources.has(s)} data-nav onclick={() => toggle(s)}>{sourceLabel[s]}</button>
      {/each}
      <span class="sep"></span>
      <button class="chip" class:on={installedOnly} data-nav onclick={() => (installedOnly = !installedOnly)}>Installed</button>
      <button class="chip" class:on={favOnly} data-nav onclick={() => (favOnly = !favOnly)}><Icon name="star" size={14} filled={favOnly} /> Favourites</button>
    </div>
    <select bind:value={sort} data-nav aria-label="Sort">
      <option value="title">A → Z</option>
      <option value="recent">Recently played</option>
      <option value="playtime">Most played</option>
      <option value="added">Recently added</option>
      <option value="critic">Critic score</option>
      <option value="short">Shortest to beat</option>
    </select>
  </div>

  <p class="count">{shown.length} {shown.length === 1 ? 'game' : 'games'}</p>

  <div class="grid">
    {#each shown as game (game.id)}
      <div animate:flip={{ duration: dur }} in:scale={{ start: 0.92, duration: dur }}>
        <Cover {game} showTitle />
      </div>
    {:else}
      <div class="empty">
        <Icon name="sparkle" size={36} />
        {#if app.libraryStatus === 'loading'}
          <p>Loading library…</p>
        {:else if app.libraryStatus === 'failed'}
          <p>{app.loadError ?? 'Could not open the library.'}</p>
        {:else if !query && app.games.length === 0}
          <p>No PS2 games yet.</p>
          <button class="btn primary" data-nav onclick={() => app.addPs2Folder()}>Add PS2 folder</button>
          {#if app.lastScan?.skipped_unsupported}
            <p>Skipped {app.lastScan.skipped_unsupported} unsupported files. Import .iso files for this milestone.</p>
          {/if}
        {:else}
          <p>Nothing matches “{query}”.</p>
        {/if}
      </div>
    {/each}
  </div>
</div>

<style>
  .library {
    padding: 28px var(--gutter) 40px;
  }
  .bar {
    position: sticky;
    top: 16px;
    z-index: 5;
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 12px;
    padding: 10px 12px;
    border-radius: 18px;
  }
  .search {
    flex: 1 1 260px;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 12px;
    height: 42px;
    border-radius: 12px;
    background: var(--field);
    color: var(--text-dim);
  }
  .search:focus-within {
    box-shadow: 0 0 0 2px var(--accent);
  }
  input {
    flex: 1;
    min-width: 0;
    background: none;
    border: 0;
    outline: none;
    color: var(--text);
    font: inherit;
    font-size: 15px;
  }
  .clear {
    background: none;
    border: 0;
    color: inherit;
    cursor: pointer;
    display: grid;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    align-items: center;
  }
  .sep {
    width: 1px;
    height: 22px;
    background: var(--line);
    margin: 0 4px;
  }
  select {
    height: 38px;
    border-radius: 10px;
    border: 1px solid var(--line);
    background: var(--field);
    color: var(--text);
    font: inherit;
    padding: 0 10px;
  }
  .count {
    margin: 18px 2px 4px;
    color: var(--text-dim);
    font-size: 13px;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(var(--cover-w), 1fr));
    gap: 26px var(--card-gap);
    padding-top: 12px;
  }
  .grid :global(.wrap) {
    width: 100%;
  }
  .empty {
    grid-column: 1 / -1;
    display: grid;
    place-items: center;
    padding: 80px 0;
    color: var(--text-dim);
  }
</style>
