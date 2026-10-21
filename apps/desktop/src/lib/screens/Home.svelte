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
  const addFolder = () => app.addPs2Folder();
  const playReady = $derived(
    !!g && g.installed && !g.mediaMissing && g.setupState !== 'missing',
  );
</script>

<div class="home">
  <div class="hero-art" aria-hidden="true">
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
        <p class="tagline">Add PS1 ISO, CHD or CUE sets, or PS2 ISO files. Each platform uses its own emulator setup.</p>
        {#if app.libraryStatus === 'failed'}
          <p class="tagline">{app.loadError}</p>
        {/if}
        {#if app.installProgress}
          <p class="tagline">{app.installProgress}</p>
        {/if}
        <div class="actions">
          <button class="btn glass" data-nav onclick={() => app.addSystemFolder('ps1')}><Icon name="plus" /> Add PS1 folder</button>
          <button class="btn primary" data-nav onclick={addFolder}>
            <Icon name="plus" /> Add PS2 folder
          </button>
          <InstallMenu />
          <button class="btn glass" data-nav onclick={() => app.go('settings')}>Settings</button>
        </div>
      </div>
    {:else if g}
      <div class="info">
        <div class="hero-title" title={g.title}>
          <div class="eyebrow">
            <span class="pill">{sourceLabel[g.source]}</span>
            <span>{platformLabel[g.platform]}</span>
            {#if g.genre}<span class="dot">•</span><span>{g.genre}</span>{/if}
            {#if g.year}<span class="dot">•</span><span>{g.year}</span>{/if}
          </div>
          <Logo game={g} size="lg" />
        </div>
        <p class="tagline">{g.tagline}</p>
        <div class="meta">
          {#if g.playtimeMin}<span><Icon name="clock" size={16} /> {hours(g.playtimeMin)}</span>{/if}
          {#if g.lastPlayed}<span>Last played {ago(g.lastPlayed)}</span>{/if}
        </div>
        <div class="actions">
          {#if g.setupState === 'runner_missing'}
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
    {/if}
  </header>

  <Row title="Continue playing" games={continuePlaying} shape="wide" placeholders={empty ? 4 : 0} onPlaceholder={addFolder} />
  <Row title="Favourites" games={favorites} placeholders={empty ? 6 : 0} />
  <Row title="Recently added" games={recentlyAdded} placeholders={empty ? 8 : 0} onPlaceholder={addFolder} />
  <Row title="PlayStation 2" games={emulated} placeholders={empty ? 8 : 0} onPlaceholder={addFolder} />
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
  .hero-cover {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }
  .hero {
    position: relative;
    z-index: 1;
    min-height: max(var(--hero-h), 440px);
    display: flex;
    align-items: flex-end;
    padding: 0 var(--gutter) 28px;
  }
  .info {
    width: min(720px, 100%);
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .hero-title {
    height: calc(clamp(36px, 5vw, 64px) * 2.85 + 36px);
    display: flex;
    flex-direction: column;
    justify-content: flex-end;
    align-items: flex-start;
    gap: 14px;
  }
  .hero-title :global(.logo) {
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    overflow: hidden;
  }
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
