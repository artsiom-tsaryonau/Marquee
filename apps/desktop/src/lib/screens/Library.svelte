<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import { flip } from 'svelte/animate';
  import { scale } from 'svelte/transition';
  import { app } from '../state.svelte';
  import { platformLabel, sourceLabel } from '../labels';
  import Mark, { hasMark } from '../components/Mark.svelte';
  import { fuzzy } from '../format';
  import { libraryView, PAGE } from '../libraryView.svelte';
  import { groupGames, launchMethod, matchesVariant, methodLabel } from '../unified';
  import type { Platform } from '../types';
  import Cover from '../components/Cover.svelte';
  import GhostCover from '../components/GhostCover.svelte';
  import Icon from '../components/Icon.svelte';
  import InstallMenu from '../components/InstallMenu.svelte';
  import CatalogueImport from '../components/CatalogueImport.svelte';
  import { metadataMatches, playerModeLabel, regionValues, versions, providerScore } from '../metadata';

  let { collection = null }: { collection?: 'ps2' | null } = $props();
  // Search, filters, sort and scroll position survive opening a game and coming back.
  // svelte-ignore state_referenced_locally
  const view = libraryView(collection ?? 'all');
  const regions = $derived([...new Set(app.games.flatMap((g) => regionValues(g.region)))].sort());
  const languages = $derived([...new Set(app.games.flatMap((g) => g.languages ?? []))].sort());
  const revisions = $derived([...new Set(app.games.flatMap(versions))].sort());
  const playerModes = $derived((['single', 'multi', 'coop'] as const).filter((mode) => app.games.some((g) => {
    const n = g.players ?? g.metadata?.players;
    if (n === undefined || n === null) return false;
    return mode === 'single' ? n <= 1 : n >= 2;
  })));
  const inScope = $derived(app.games.filter((g) => !collection || g.platform === collection));
  const counts = $derived.by(() => { const m = new Map<Platform, number>(); for (const g of inScope) m.set(g.platform, (m.get(g.platform) ?? 0) + 1); return m; });
  // Biggest first, "Other" (store games, unrecognised files) last: it is the catch-all, not a platform.
  const platforms = $derived([...counts.keys()].sort((a, b) => (a === 'unknown' ? 1 : b === 'unknown' ? -1 : (counts.get(b) ?? 0) - (counts.get(a) ?? 0) || platformLabel[a].localeCompare(platformLabel[b]))));
  const chipLabel = (p: Platform) => (p === 'unknown' ? 'Other' : platformLabel[p]);
  // Only offer sorts that can do something with this library.
  const canSort = $derived({
    critic: inScope.some((g) => g.ratings.critic !== null),
    short: inScope.some((g) => g.timeToBeat),
    source: inScope.some((g) => providerScore(g.metadata) !== null),
    recent: inScope.some((g) => g.lastPlayed),
    playtime: inScope.some((g) => g.playtimeMin > 0),
  });
  let search: HTMLInputElement;
  let addMenu: HTMLDetailsElement;

  const allSources = $derived([...new Set(app.games.map((g) => g.source))]);
  const secondaryActive = $derived([view.sourceFilter, view.region, view.language, view.revision, view.playerMode].filter(Boolean).length);

  const shown = $derived.by(() => {
    let list = app.games.filter(
      (g) =>
        (!collection || g.platform === collection) &&
        matchesVariant(g, view.platform, '') &&
        metadataMatches(g, view.region, view.language, view.revision, view.playerMode) &&
        (!view.sourceFilter || g.source === view.sourceFilter) &&
        (!view.installedOnly || g.installed) &&
        (!view.favOnly || g.favorite),
    );
    const query = view.query;
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
    }[view.sort];
    return [...list].sort(by);
  });

  // Typing anywhere jumps into search, like a console launcher.
  function onKey(e: KeyboardEvent) {
    if (e.key.length === 1 && !e.ctrlKey && !e.metaKey && !e.altKey && document.activeElement !== search) {
      if (!e.key.trim() || (e.target instanceof HTMLElement && e.target.closest('input, textarea, select, [contenteditable=true]'))) return;
      search.focus();
    }
  }
  function onSearchKey(e: KeyboardEvent) {
    if (e.key === 'Escape' && view.query) { view.query = ''; e.stopPropagation(); }
  }

  // A changed search or filter starts again at the top; a restored one must not.
  $effect(() => {
    const signature = JSON.stringify([view.query, view.platform, view.sourceFilter, view.region, view.language, view.revision, view.playerMode, view.installedOnly, view.favOnly, view.sort, collection]);
    untrack(() => { if (view.signature !== signature) { view.signature = signature; view.visibleCount = PAGE; } });
  });
  const grouped = $derived(groupGames(shown, app.variantPreferences));

  const filtering = $derived(!!view.query || !!view.platform || !!view.sourceFilter || !!view.region || !!view.language || !!view.revision || !!view.playerMode || view.installedOnly || view.favOnly);
  function clearFilters() { Object.assign(view, { query: '', platform: '', sourceFilter: '', region: '', language: '', revision: '', playerMode: '', installedOnly: false, favOnly: false }); }
  const dur = $derived(app.motionReduced || app.visualEffects === 'balanced' ? 0 : 260);

  // Cards load as the end of the list comes near; the button stays as a fallback.
  function nearEnd(node: HTMLElement) {
    if (typeof IntersectionObserver === 'undefined') return;
    const io = new IntersectionObserver((entries) => { if (entries.some((e) => e.isIntersecting)) view.visibleCount += PAGE; }, { rootMargin: '900px' });
    io.observe(node);
    return { destroy: () => io.disconnect() };
  }

  let frame = 0;
  function onScroll() {
    if (frame) return;
    frame = requestAnimationFrame(() => { frame = 0; view.scrollY = window.scrollY; });
  }
  onMount(() => {
    if (view.scrollY > 0) requestAnimationFrame(() => window.scrollTo(0, view.scrollY));
    const close = (e: MouseEvent) => { if (addMenu?.open && !addMenu.contains(e.target as Node)) addMenu.open = false; };
    window.addEventListener('click', close);
    return () => window.removeEventListener('click', close);
  });

  const addTargets: { id: string; label: string }[] = [
    { id: 'ps2', label: 'PlayStation 2' }, { id: 'ps1', label: 'PlayStation' }, { id: 'genesis', label: 'Sega Genesis' },
    { id: 'ds', label: 'Nintendo DS' }, { id: 'neogeo', label: 'Neo Geo' },
  ];
  function addFolder(system: string) { addMenu.open = false; void app.addSystemFolder(system); }
</script>

<svelte:window onkeydown={onKey} onscroll={onScroll} />

<div class="library">
  <header class="library-heading">
    <div><span class="eyebrow">Your collection</span><h1>{collection === 'ps2' ? 'PlayStation 2' : 'Library'}</h1><p>Choose a game, then choose how you play.</p></div>
    <div class="heading-actions">
      <button class="btn glass small" data-nav onclick={() => app.go('tools')}><Icon name="controller" size={16} /> Tools & launchers</button>
      <details class="add-menu" bind:this={addMenu}>
        <summary class="btn primary small" data-nav><Icon name="plus" size={16} /> Add games</summary>
        <div class="add-list glass-panel" role="menu" aria-label="Add a folder of games">
          <span class="add-hint">Pick the system, then the folder that holds its games.</span>
          {#each addTargets as t (t.id)}<button class="add-item" role="menuitem" data-nav onclick={() => addFolder(t.id)}>{t.label}</button>{/each}
        </div>
      </details>
    </div>
  </header>
  <div class="categories" data-nav-row aria-label="Game platforms">
    <button class="chip" class:on={!view.platform} data-nav aria-pressed={!view.platform} onclick={() => view.platform = ''}>All platforms <span>{inScope.length}</span></button>
    {#each platforms as p}<button class="chip" class:on={view.platform === p} data-nav aria-pressed={view.platform === p} onclick={() => view.platform = p}>{#if hasMark(p)}<Mark id={p} size={16} />{/if}<b>{chipLabel(p)}</b> <span>{counts.get(p)}</span></button>{/each}
  </div>
  <div class="bar glass-panel">
    <label class="search">
      <Icon name="search" size={18} />
      <input bind:this={search} bind:value={view.query} onkeydown={onSearchKey} data-nav placeholder="Search {collection === 'ps2' ? 'PS2' : app.games.length} games…" spellcheck="false" />
      {#if view.query}<button class="clear" onclick={() => (view.query = '')} aria-label="Clear search"><Icon name="x" size={16} /></button>{/if}
    </label>
    <div class="chips" data-nav-row>
      <button class="chip" class:on={view.installedOnly} data-nav aria-pressed={view.installedOnly} onclick={() => (view.installedOnly = !view.installedOnly)}>Installed</button>
      <button class="chip" class:on={view.favOnly} data-nav aria-pressed={view.favOnly} onclick={() => (view.favOnly = !view.favOnly)}><Icon name="star" size={14} filled={view.favOnly} /> Favourites</button>
      <button class="chip" class:on={secondaryActive > 0} data-nav aria-expanded={view.filtersOpen} aria-controls="more-filters" onclick={() => (view.filtersOpen = !view.filtersOpen)}>Filters{#if secondaryActive}<span class="badge">{secondaryActive}</span>{/if}</button>
      <select class="ui-select" bind:value={view.sort} data-nav aria-label="Sort by">
        <option value="title">Sort: A → Z</option>
        <option value="added">Sort: Recently added</option>
        {#if canSort.recent}<option value="recent">Sort: Recently played</option>{/if}
        {#if canSort.playtime}<option value="playtime">Sort: Most played</option>{/if}
        {#if canSort.critic}<option value="critic">Sort: Critic score</option>{/if}
        {#if canSort.short}<option value="short">Sort: Shortest to beat</option>{/if}
        {#if canSort.source}<option value="source">Sort: Source rating · Vimm</option>{/if}
      </select>
    </div>
    {#if view.filtersOpen}
    <div class="filters" id="more-filters" data-nav-row>
      <select class="ui-select" bind:value={view.sourceFilter} data-nav aria-label="Source or launcher">
        <option value="">All sources</option>
        {#each allSources as s (s)}<option value={s}>{sourceLabel[s]}</option>{/each}
      </select>
      {#if regions.length}<select class="ui-select" bind:value={view.region} data-nav aria-label="Region"><option value="">All regions</option>{#each regions as value}<option value={value}>{value}</option>{/each}</select>{/if}
      {#if languages.length}<select class="ui-select" bind:value={view.language} data-nav aria-label="Language"><option value="">All languages</option>{#each languages as value}<option value={value}>{value}</option>{/each}</select>{/if}
      {#if revisions.length}<select class="ui-select" bind:value={view.revision} data-nav aria-label="ROM revision"><option value="">All ROM revisions</option>{#each revisions as value}<option value={value}>{value}</option>{/each}</select>{/if}
      {#if playerModes.length}<select class="ui-select" bind:value={view.playerMode} data-nav aria-label="Player mode"><option value="">Any player mode</option>{#each playerModes as mode}<option value={mode}>{playerModeLabel[mode]}</option>{/each}</select>{/if}
    </div>
    {/if}
  </div>

  <div class="results"><p class="count" aria-live="polite">{grouped.length} {grouped.length === 1 ? 'game' : 'games'}{shown.length !== grouped.length ? ` · ${shown.length} play options` : ''}</p>{#if filtering}<button class="reset" data-nav onclick={clearFilters}>Clear filters</button>{/if}</div>

  {#if !shown.length && (app.libraryStatus === 'loading' || app.libraryStatus === 'failed' || view.query || app.games.length)}
    <div class="empty-banner">
      {#if app.libraryStatus === 'loading'}
        <p>Loading library…</p>
      {:else if app.libraryStatus === 'failed'}
        <p>{app.loadError ?? 'Could not open the library.'}</p>
      {:else}
        <p>{view.query ? `No games match “${view.query}” and these filters.` : 'No games match these filters.'}</p><button class="btn glass small" data-nav onclick={clearFilters}>Clear filters</button>
      {/if}
    </div>
  {:else if !shown.length && !view.query && app.games.length === 0}
    <div class="empty-banner actions">
      <p>No games yet. Add a folder of games, or connect a launcher in Tools.</p>
      {#each addTargets as t (t.id)}<button class="btn {t.id === 'ps2' ? 'primary' : 'glass'}" data-nav onclick={() => app.addSystemFolder(t.id)}>{t.label}</button>{/each}
      <InstallMenu />
      {#if app.lastScan?.skipped_unsupported}
        <p>Skipped {app.lastScan.skipped_unsupported} companion or unsupported files. PS1 accepts ISO, CHD and CUE sets; PS2 accepts ISO.</p>
      {/if}
    </div>
  {/if}

  {#if app.lutrisError}<div class="empty-banner"><p>Lutris refresh failed. Previously imported entries may be out of date.</p><button class="btn glass small" data-nav onclick={() => app.go('tools')}>Check connection</button></div>{/if}
  <div class="grid">
    {#each grouped.slice(0, view.visibleCount) as group (group.game.id)}
      {@const platformTags = [...new Set(group.entries.map((e) => e.platform))]}
      <div class="grid-cell" animate:flip={{ duration: dur }} in:scale={{ start: 0.92, duration: dur }}>
        <Cover game={group.game} showTitle />
        {#if platformTags.length > 1}
          <div class="platform-tags" aria-label="Platforms">
            {#each platformTags as p}<span class="tag">{platformLabel[p]}</span>{/each}
          </div>
        {/if}
        {#if group.entries.length > 1}<span class="variants">{group.entries.length} play options</span>{/if}
      </div>
    {:else}
      {#if !view.query && app.games.length === 0 && app.libraryStatus !== 'loading' && app.libraryStatus !== 'failed'}
        {#each Array(12) as _, i (i)}
          <GhostCover onclick={() => app.addPs2Folder()} />
        {/each}
      {/if}
    {/each}
  </div>
  {#if grouped.length > view.visibleCount}
    {#key view.visibleCount}<div class="load-more" use:nearEnd><button class="btn glass" data-nav onclick={() => (view.visibleCount += PAGE)}>Show more · {view.visibleCount} of {grouped.length}</button></div>{/key}
  {/if}
  <CatalogueImport />
</div>

<style>
  .categories { display:flex; gap:8px; flex-wrap:wrap; margin-bottom:18px; }
  .categories .chip { display:inline-flex; align-items:center; gap:6px; }
  .categories b { font-weight:inherit; }
  .categories span, .badge { opacity:.65; font-variant-numeric:tabular-nums; }
  .badge { display:inline-grid; place-items:center; min-width:18px; height:18px; margin-left:6px; border-radius:9px; background:var(--accent); color:#fff; font-size:11px; opacity:1; padding:0 5px; }
  .add-menu { position:relative; }
  .add-menu summary { list-style:none; display:inline-flex; align-items:center; gap:6px; cursor:pointer; }
  .add-menu summary::-webkit-details-marker { display:none; }
  .add-list { position:absolute; right:0; top:calc(100% + 8px); z-index:30; display:grid; gap:2px; min-width:230px; padding:8px; border-radius:14px; background:#16131f; border:1px solid var(--line); box-shadow:0 18px 40px -12px rgb(0 0 0 / .8); }
  .add-hint { font-size:12px; color:var(--text-dim); padding:6px 10px 8px; }
  .add-item { text-align:left; border:0; background:none; color:var(--text); font:inherit; padding:10px; border-radius:8px; cursor:pointer; }
  .add-item:hover, .add-item:focus-visible { background:var(--field); outline:none; }
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
  .filters {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    align-items: center;
    flex: 1 1 100%;
    padding-top: 6px;
    border-top: 1px solid var(--line);
  }
  .grid-cell { display: flex; flex-direction: column; gap: 6px; min-width: 0; }
  .platform-tags { display: flex; flex-wrap: wrap; gap: 6px; }
  .platform-tags .tag {
    font-size: 11px;
    padding: 3px 8px;
    border-radius: 999px;
    background: var(--field);
    color: var(--text-soft);
    border: 1px solid var(--line);
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
