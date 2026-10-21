<script lang="ts">
  import { flip } from 'svelte/animate';
  import { scale } from 'svelte/transition';
  import { app } from '../state.svelte';
  import { platformLabel, sourceLabel } from '../labels';
  import Mark, { hasMark } from '../components/Mark.svelte';
  import { fuzzy } from '../format';
  import { groupGames, launchMethod, matchesVariant, methodLabel, type LaunchMethod } from '../unified';
  import type { Source, Platform } from '../types';
  import Cover from '../components/Cover.svelte';
  import GhostCover from '../components/GhostCover.svelte';
  import Icon from '../components/Icon.svelte';
  import InstallMenu from '../components/InstallMenu.svelte';
  import CatalogueImport from '../components/CatalogueImport.svelte';
  import { metadataMatches, regionValues, versions, providerScore } from '../metadata';

  let { collection = null }: { collection?: 'ps2' | null } = $props();
  let visibleCount = $state(48);
  let query = $state('');
  let platform = $state<Platform | ''>('');
  let method = $state<LaunchMethod | ''>('');
  let region = $state('');
  let language = $state('');
  let revision = $state('');
  let players = $state('');
  const regions = $derived([...new Set(app.games.flatMap((g) => regionValues(g.region)))].sort());
  const languages = $derived([...new Set(app.games.flatMap((g) => g.languages ?? []))].sort());
  const revisions = $derived([...new Set(app.games.flatMap(versions))].sort());
  const playerCounts = $derived([...new Set(app.games.map((g) => g.players).filter((n) => n !== undefined))].sort((a,b) => a! - b!));
  const platforms = $derived([...new Set(app.games.filter((g) => !collection || g.platform === collection).map((g) => g.platform))].sort((a, b) => platformLabel[a].localeCompare(platformLabel[b])));
  let sources = $state<Set<Source>>(new Set());
  let installedOnly = $state(false);
  let favOnly = $state(false);
  let sort = $state<'title' | 'recent' | 'playtime' | 'added' | 'critic' | 'short' | 'source'>('title');
  let search: HTMLInputElement;

  const allSources = $derived([...new Set(app.games.map((g) => g.source))]);

  const shown = $derived.by(() => {
    let list = app.games.filter(
      (g) =>
        (!collection || g.platform === collection) &&
        matchesVariant(g, platform, method) &&
        metadataMatches(g, region, language, revision, players) &&
        (!sources.size || sources.has(g.source)) &&
        (!installedOnly || g.installed) &&
        (!favOnly || g.favorite),
    );
    if (query.trim()) {
      return list
        .map((g) => ({ g, s: Math.max(fuzzy(query, g.title), fuzzy(query, `${g.genre} ${g.editionLabel ?? ""} ${platformLabel[g.platform]} ${methodLabel[launchMethod(g)]} ${g.region ?? ''} ${g.languages?.join(' ') ?? ''} ${versions(g).join(' ')} ${g.serial ?? ''}`) - 5) }))
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
      source: (a: typeof list[0], b: typeof list[0]) => (providerScore(b.metadata) ?? -1) - (providerScore(a.metadata) ?? -1),
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
      if (!e.key.trim() || (e.target instanceof HTMLElement && e.target.closest('input, textarea, select, [contenteditable=true]'))) return;
      search.focus();
    }
  }

  $effect(() => { JSON.stringify([query, platform, method, region, language, revision, players, [...sources], installedOnly, favOnly, sort, collection]); visibleCount = 48; });
  const grouped = $derived(groupGames(shown, app.variantPreferences));

  const filtering = $derived(!!query || !!platform || !!method || !!region || !!language || !!revision || !!players || sources.size > 0 || installedOnly || favOnly);
  function clearFilters() { query = ''; platform = ''; method = ''; region = ''; language = ''; revision = ''; players = ''; sources = new Set(); installedOnly = false; favOnly = false; }
  const dur = $derived(app.motionReduced || app.visualEffects === 'balanced' ? 0 : 260);
</script>

<svelte:window onkeydown={onKey} />

<div class="library">
  <header class="library-heading"><div><span class="eyebrow">Your collection</span><h1>{collection === 'ps2' ? 'PlayStation 2' : 'Library'}</h1><p>Choose a game, then choose how you play.</p></div><div class="heading-actions"><button class="btn glass small" data-nav onclick={() => app.go('tools')}><Icon name="controller" size={16} /> Tools & launchers</button><button class="btn glass small" data-nav onclick={() => app.addSystemFolder('ps1')}><Icon name="plus" size={16} /> Add PS1 folder</button><button class="btn primary small" data-nav onclick={() => app.addPs2Folder()}><Icon name="plus" size={16} /> Add PS2 folder</button></div></header>
  <div class="heading-actions" style="margin-bottom:18px">
    <button class="btn glass small" data-nav onclick={() => app.addSystemFolder('genesis')}><Icon name="plus" size={16} /> Add Genesis folder</button>
    <button class="btn glass small" data-nav onclick={() => app.addSystemFolder('ds')}><Icon name="plus" size={16} /> Add Nintendo DS folder</button>
    <button class="btn glass small" data-nav onclick={() => app.addSystemFolder('neogeo')}><Icon name="plus" size={16} /> Add Neo Geo folder</button>
  </div>
  <div class="categories" data-nav-row aria-label="Game platforms">
    <button class="chip" class:on={!platform} data-nav aria-pressed={!platform} onclick={() => platform = ''}>All platforms</button>
    {#each platforms as p}<button class="chip" class:on={platform === p} data-nav aria-pressed={platform === p} title={platformLabel[p]} onclick={() => platform = p}>{#if hasMark(p)}<Mark id={p} size={18} /><span class="sr">{platformLabel[p]}</span>{:else}{platformLabel[p]}{/if} <span>{app.games.filter((g) => g.platform === p).length}</span></button>{/each}
  </div>
  <CatalogueImport />
  <div class="bar glass-panel">
    <label class="search">
      <Icon name="search" size={18} />
      <input bind:this={search} bind:value={query} data-nav placeholder="Search {collection === 'ps2' ? 'PS2' : app.games.length} games…" spellcheck="false" />
      {#if query}<button class="clear" onclick={() => (query = '')} aria-label="Clear"><Icon name="x" size={16} /></button>{/if}
    </label>
    <div class="chips" data-nav-row>
      {#each allSources as s (s)}
        <button class="chip" class:on={sources.has(s)} data-nav title={sourceLabel[s]} onclick={() => toggle(s)}>{#if hasMark(s)}<Mark id={s} size={18} /><span class="sr">{sourceLabel[s]}</span>{:else}{sourceLabel[s]}{/if}</button>
      {/each}
      <span class="sep"></span>
      <button class="chip" class:on={installedOnly} data-nav onclick={() => (installedOnly = !installedOnly)}>Installed</button>
      <button class="chip" class:on={favOnly} data-nav onclick={() => (favOnly = !favOnly)}><Icon name="star" size={14} filled={favOnly} /> Favourites</button>
    </div>
    <select bind:value={platform} data-nav aria-label="Platform">
      <option value="">All platforms</option>
      {#each platforms as p}<option value={p}>{platformLabel[p]}</option>{/each}
    </select>
    <select bind:value={method} data-nav aria-label="Run method">
      <option value="">All run methods</option>
      {#each Object.entries(methodLabel) as [key, label]}<option value={key}>{label}</option>{/each}
    </select>
    <select bind:value={sort} data-nav aria-label="Sort">
      <option value="title">A → Z</option>
      <option value="recent">Recently played</option>
      <option value="playtime">Most played</option>
      <option value="added">Recently added</option>
      <option value="critic">Critic score</option>
      <option value="short">Shortest to beat</option>
      <option value="source">Source rating · Vimm</option>
    </select>
    {#if regions.length}<select bind:value={region} data-nav aria-label="Region"><option value="">All regions</option>{#each regions as value}<option value={value}>{value}</option>{/each}</select>{/if}
    {#if languages.length}<select bind:value={language} data-nav aria-label="Language"><option value="">All languages</option>{#each languages as value}<option value={value}>{value}</option>{/each}</select>{/if}
    {#if revisions.length}<select bind:value={revision} data-nav aria-label="ROM revision"><option value="">All ROM revisions</option>{#each revisions as value}<option value={value}>{value}</option>{/each}</select>{/if}
    {#if playerCounts.length}<select bind:value={players} data-nav aria-label="Player count"><option value="">Any player count</option>{#each playerCounts as value}<option value={String(value)}>{value} player{value === 1 ? '' : 's'}</option>{/each}</select>{/if}
  </div>

  <div class="results"><p class="count" aria-live="polite">{grouped.length} {grouped.length === 1 ? 'game' : 'games'}{shown.length !== grouped.length ? ` · ${shown.length} play options` : ''}</p>{#if filtering}<button class="reset" data-nav onclick={clearFilters}>Clear filters</button>{/if}</div>

  {#if !shown.length && (app.libraryStatus === 'loading' || app.libraryStatus === 'failed' || query || app.games.length)}
    <div class="empty-banner">
      {#if app.libraryStatus === 'loading'}
        <p>Loading library…</p>
      {:else if app.libraryStatus === 'failed'}
        <p>{app.loadError ?? 'Could not open the library.'}</p>
      {:else}
        <p>{query ? `No games match “${query}” and these filters.` : 'No games match these filters.'}</p><button class="btn glass small" data-nav onclick={clearFilters}>Clear filters</button>
      {/if}
    </div>
  {:else if !shown.length && !query && app.games.length === 0}
    <div class="empty-banner actions">
      <p>No game discs yet.</p>
      <button class="btn glass" data-nav onclick={() => app.addSystemFolder('ps1')}>Add PS1 folder</button><button class="btn primary" data-nav onclick={() => app.addPs2Folder()}>Add PS2 folder</button>
      <InstallMenu />
      {#if app.lastScan?.skipped_unsupported}
        <p>Skipped {app.lastScan.skipped_unsupported} companion or unsupported files. PS1 accepts ISO, CHD and CUE sets; PS2 accepts ISO.</p>
      {/if}
    </div>
  {/if}

  {#if app.lutrisError}<div class="empty-banner"><p>Lutris refresh failed. Previously imported entries may be out of date.</p><button class="btn glass small" data-nav onclick={() => app.go('tools')}>Check connection</button></div>{/if}
  <div class="grid">
    {#each grouped.slice(0, visibleCount) as group (group.game.id)}
      <div animate:flip={{ duration: dur }} in:scale={{ start: 0.92, duration: dur }}>
        <Cover game={group.game} showTitle />
        {#if group.entries.length > 1}<span class="variants">{group.entries.length} play options</span>{/if}
      </div>
    {:else}
      {#if !query && app.games.length === 0 && app.libraryStatus !== 'loading' && app.libraryStatus !== 'failed'}
        {#each Array(12) as _, i (i)}
          <GhostCover onclick={() => app.addPs2Folder()} />
        {/each}
      {/if}
    {/each}
  </div>
  {#if grouped.length > visibleCount}<div class="load-more"><button class="btn glass" data-nav onclick={() => (visibleCount += 48)}>Show more games · {Math.min(visibleCount, grouped.length)} of {grouped.length}</button></div>{/if}
</div>

<style>
  .categories { display:flex; gap:8px; flex-wrap:wrap; margin-bottom:18px; } .categories span { margin-left:6px; opacity:.65; font-variant-numeric:tabular-nums; }
  .load-more { display:flex; justify-content:center; margin-top:28px; }
  .library-heading { display:flex; flex-wrap:wrap; justify-content:space-between; align-items:center; gap:18px; margin:4px 0 26px; }
  .library-heading h1 { font-size:34px; margin:5px 0 8px; letter-spacing:-.8px; }
  .library-heading p { margin:0; font-size:14px; color:var(--text-dim); }
  .heading-actions { display:flex; flex-wrap:wrap; gap:8px; }
  .results { display:flex; justify-content:space-between; align-items:center; margin-top:10px; }
  .reset { color:var(--text-soft); border:0; background:none; padding:10px; font:inherit; font-size:12px; cursor:pointer; }
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
  .variants { font-size: 12px; color: var(--text-dim); }
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
  .empty-banner {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 12px;
    margin: 8px 0 4px;
    color: var(--text-soft);
  }
  .empty-banner.actions {
    margin-top: 18px;
  }
  .empty-banner p {
    margin: 0;
  }
</style>
