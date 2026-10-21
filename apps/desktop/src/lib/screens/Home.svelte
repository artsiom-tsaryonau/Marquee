<script lang="ts">
  import { app } from '../state.svelte';
  import { platformLabel, sourceLabel } from '../labels';
  import { ago, hours } from '../format';
  import Row from '../components/Row.svelte';
  import Logo from '../components/Logo.svelte';
  import Icon from '../components/Icon.svelte';
  import Artwork from '../components/Artwork.svelte';
  import InstallMenu from '../components/InstallMenu.svelte';
  import { homeAtmosphere } from '../art';
  import type { Art } from '../types';
  import { libraryView } from '../libraryView.svelte';


  const continuePlaying = $derived(
    app.games.filter((g) => g.lastPlayed).sort((a, b) => b.lastPlayed!.localeCompare(a.lastPlayed!)).slice(0, 10),
  );
  const recentlyAdded = $derived([...app.games].sort((a, b) => b.addedAt.localeCompare(a.addedAt)).slice(0, 12));
  const portsReady = $derived(app.games.filter((g) => g.source === 'port'));
  const favorites = $derived(app.games.filter((g) => g.favorite));
  const emulated = $derived(app.games.filter((g) => g.source === 'rom'));
  const hiddenGems = $derived(app.games.filter((g) => !g.playtimeMin && (g.ratings.critic ?? 0) >= 80));
  const quickWins = $derived(
    app.games.filter((g) => g.timeToBeat && g.timeToBeat[0] <= 6).sort((a, b) => a.timeToBeat![0] - b.timeToBeat![0]),
  );
  const notInstalled = $derived(app.games.filter((g) => !g.installed));
  const wanted = $derived(app.games.filter((g) => g.wanted));
  const stats = $derived([
    { n: app.games.length, label: 'games' },
    { n: app.games.filter((g) => g.lastPlayed || g.playtimeMin).length, label: 'played' },
    { n: favorites.length, label: 'favourites' },
    { n: wanted.length, label: 'without files' },
  ].filter((x) => x.n > 0));
  const topPlatforms = $derived.by(() => {
    const m = new Map<string, number>();
    for (const x of app.games) if (x.platform !== 'unknown') m.set(x.platform, (m.get(x.platform) ?? 0) + 1);
    return [...m.entries()].sort((a, b) => b[1] - a[1]).slice(0, 10) as [import('../types').Platform, number][];
  });
  function browse(p: import('../types').Platform) { libraryView('all').platform = p; app.go('library'); }

  const g = $derived(app.hero);
  // Keep the previous image until the latest selection is decoded. Cancelling
  // the effect prevents a slow earlier request from replacing a newer preview.
  let preview = $state<{ art: Art; url?: string }>({ art: homeAtmosphere });
  $effect(() => {
    const art = g?.art ?? homeAtmosphere;
    const url = g?.heroUrl ?? g?.coverUrl;
    let cancelled = false;
    if (url) {
      const image = new Image();
      image.src = url;
      image.decode().then(
        () => { if (!cancelled) preview = { art, url }; },
        () => { if (!cancelled) preview = { art }; },
      );
    } else {
      preview = { art };
    }
    return () => { cancelled = true; };
  });
  const empty = $derived(!g);
  const wide = $derived(!!g?.heroUrl);
  const poster = $derived(!wide ? (preview.url ?? undefined) : undefined);
  const addFolder = () => app.addPs2Folder();
  const playReady = $derived(
    !!g && g.installed && !g.mediaMissing && g.setupState !== 'missing',
  );
</script>

<div class="home">
  <div class="hero-art" class:blurred={!wide} aria-hidden="true">
    <div class="hero-layer">
      {#if preview.url}
        <img class="hero-cover" src={preview.url} alt="" />
      {:else}
        <Artwork art={preview.art} />
      {/if}
    </div>
  </div>
  <header class="hero">
    {#if empty}
      <div class="info">
        <p class="eyebrow"><span class="pill">Your game library</span></p>
        <Logo game={{ title: 'Marquee', art: homeAtmosphere }} size="xl" />
        <p class="tagline">Add a folder of games for each system. Marquee sets up the emulator, finds covers and details, and launches from here.</p>
        {#if app.libraryStatus === 'failed'}
          <p class="tagline">{app.loadError}</p>
        {/if}
        {#if app.installProgress}
          <p class="tagline">{app.installProgress}</p>
        {/if}
        <div class="actions">
          <button class="btn primary" data-nav onclick={addFolder}>
            <Icon name="plus" /> Add PS2 folder
          </button>
          <button class="btn glass" data-nav onclick={() => app.addSystemFolder('ps1')}><Icon name="plus" /> Add PS1 folder</button>
          <button class="btn glass" data-nav onclick={() => app.go('library')}>Other systems…</button>
          <InstallMenu />
          <button class="btn glass" data-nav onclick={() => app.go('settings')}>Settings</button>
        </div>
      </div>
    {:else if g}
      <div class="info">
        <div class="eyebrow">
          <span class="kicker">{g.lastPlayed && g.id === app.defaultHero()?.id ? 'Pick up where you left off' : 'Selected'}</span>
          <span class="pill">{sourceLabel[g.source]}</span>
          {#if g.platform !== 'unknown'}<span>{platformLabel[g.platform]}</span>{/if}
          {#if g.genre}<span class="dot">•</span><span>{g.genre}</span>{/if}
          {#if g.year}<span class="dot">•</span><span>{g.year}</span>{/if}
        </div>
        <h1 class="title" title={g.title}>{g.title}</h1>
        <p class="tagline">{g.description ?? g.tagline}</p>
        <div class="meta">
          {#if g.playtimeMin}<span><Icon name="clock" size={16} /> {hours(g.playtimeMin)}</span>{/if}
          {#if g.lastPlayed}<span>Last played {ago(g.lastPlayed)}</span>{/if}
        </div>
        <div class="actions">
          {#if g.wanted}
            <button class="btn primary" data-nav disabled><Icon name="download" size={24} /> Play · no files yet</button>
          {:else if g.setupState === 'runner_missing'}
            {#if g.platform === 'ps2'}<InstallMenu tone="primary" />{:else}<button class="btn primary" data-nav onclick={() => app.go('tools')}>Set up {g.platform === 'ps1' ? 'DuckStation' : g.platform === 'genesis' ? 'BlastEm' : g.platform === 'ds' ? 'melonDS' : g.platform === 'neogeo' ? 'FinalBurn Neo' : 'emulator'}</button>{/if}
          {:else}
            <button
              class="btn primary"
              class:play={playReady}
              data-nav
              style="--accent-h:{g.art.hue2}"
              aria-label={playReady ? 'Play' : undefined}
              onclick={() => {
                if (g.mediaMissing) app.open(g);
                else if (g.setupState === 'missing') app.explainBios();
                else if (g.installed) app.play(g);
                else app.open(g);
              }}
            >
              <Icon name={g.installed ? 'play' : 'download'} size={playReady ? 28 : 24} />
              {#if !playReady}
                {g.mediaMissing ? 'Check disc files' : g.setupState === 'missing' ? 'BIOS needed' : g.installed ? 'Play' : 'Details'}
              {/if}
            </button>
          {/if}
          <button class="btn glass" data-nav onclick={() => app.open(g)}>Details</button>
          <button class="btn glass icon" data-nav aria-label="Favourite" onclick={() => app.toggleFavorite(g)}>
            <Icon name="star" filled={g.favorite} />
          </button>
        </div>
      </div>
      {#if poster}<img class="poster" src={poster} alt="" />{/if}
    {/if}
  </header>

  {#if !empty}
    <nav class="strip" aria-label="Library overview">
      <div class="stats">
        {#each stats as s (s.label)}<span><b>{s.n.toLocaleString()}</b> {s.label}</span>{/each}
      </div>
      <div class="plats" data-nav-row>
        {#each topPlatforms as [p, n] (p)}<button class="chip" data-nav onclick={() => browse(p)}>{platformLabel[p]} <span>{n}</span></button>{/each}
        <button class="chip all" data-nav onclick={() => app.go('library')}>Browse everything →</button>
      </div>
    </nav>
  {/if}

  <Row title="Continue playing" games={continuePlaying} shape="wide" placeholders={empty ? 4 : 0} onPlaceholder={addFolder} />
  <Row title="Favourites" games={favorites} placeholders={empty ? 6 : 0} />
  <Row title="Wishlist and games without files" games={wanted} />
  <Row title="Recently added" games={recentlyAdded} placeholders={empty ? 8 : 0} onPlaceholder={addFolder} />
  <Row title="Emulated games" games={emulated} placeholders={empty ? 8 : 0} onPlaceholder={addFolder} />
</div>

<style>
  .home {
    position: relative;
  }
  .hero-art {
    position: absolute;
    inset: 0 0 auto 0;
    height: calc(var(--hero-h) + 160px);
    z-index: -1;
    overflow: hidden;
    z-index: 0;
    -webkit-mask-image: linear-gradient(180deg, #000 58%, transparent),
      linear-gradient(90deg, transparent 0, #000 42%);
    mask-image: linear-gradient(180deg, #000 58%, transparent),
      linear-gradient(90deg, transparent 0, #000 42%);
    -webkit-mask-composite: source-in;
    mask-composite: intersect;
  }
  .hero-layer {
    position: absolute;
    inset: 0;
  }
  /* A cover or generated art is not a wide picture: show it as a dim, soft backdrop and
     keep the sharp version for the poster. */
  .hero-art.blurred .hero-layer { filter: blur(36px) saturate(0.7) brightness(0.5); transform: scale(1.25); }
  .hero-art.blurred { -webkit-mask-image: linear-gradient(180deg, #000 50%, transparent); mask-image: linear-gradient(180deg, #000 50%, transparent); }
  .hero-cover {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }
  .hero {
    position: relative;
    z-index: 1;
    min-height: max(calc(var(--hero-h) * 0.62), 320px);
    gap: 32px;
    display: flex;
    align-items: flex-end;
    padding: 56px var(--gutter) 24px;
  }
  .info {
    width: min(720px, 100%);
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .title {
    margin: 0;
    font-size: clamp(30px, 4.2vw, 56px);
    line-height: 1.05;
    letter-spacing: -0.02em;
    font-weight: 800;
    color: var(--text);
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    overflow: hidden;
    text-wrap: balance;
  }
  .kicker { color: var(--text); letter-spacing: 0.06em; text-transform: uppercase; font-size: 11px; }
  .poster {
    flex: none;
    margin-left: auto;
    align-self: flex-end;
    width: clamp(120px, 14vw, 190px);
    aspect-ratio: 2 / 3;
    object-fit: cover;
    border-radius: var(--radius);
    box-shadow: 0 24px 50px -16px rgb(0 0 0 / 0.9), 0 0 0 1px rgb(255 255 255 / 0.08);
  }
  @media (max-width: 760px) { .poster { display: none; } }
  .strip {
    position: relative;
    z-index: 1;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 12px 24px;
    padding: 8px var(--gutter) 14px;
  }
  .stats { display: flex; flex-wrap: wrap; gap: 6px 22px; font-size: 13px; color: var(--text-dim); }
  .stats b { color: var(--text); font-weight: 700; font-variant-numeric: tabular-nums; }
  .plats { display: flex; flex-wrap: wrap; gap: 8px; }
  .plats .chip span { opacity: 0.6; margin-left: 4px; font-variant-numeric: tabular-nums; }
  .plats .chip.all { color: var(--text-soft); }
  .eyebrow {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
    color: var(--text-dim);
    font-weight: 600;
    height: 22px;
    flex-shrink: 0;
    max-width: 100%;
    overflow: hidden;
    white-space: nowrap;
  }
  .dot {
    opacity: 0.5;
  }
  .tagline {
    margin: 0;
    font-size: var(--tagline);
    color: var(--text-soft);
    max-width: 52ch;
    height: 2.8em;
    line-height: 1.4;
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    overflow: hidden;
  }
  .meta {
    min-height: 20px;
    display: flex;
    gap: 18px;
    font-size: 13px;
    color: var(--text-dim);
  }
  .meta span {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .actions {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    min-height: 68px;
    gap: 10px;
    margin-top: 6px;
  }
</style>
