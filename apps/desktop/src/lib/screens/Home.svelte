<script lang="ts">
  import { fade, fly } from 'svelte/transition';
  import { app } from '../state.svelte';
  import { platformLabel, sourceLabel } from '../labels';
  import { ago, hours } from '../format';
  import Row from '../components/Row.svelte';
  import Logo from '../components/Logo.svelte';
  import Icon from '../components/Icon.svelte';
  import Artwork from '../components/Artwork.svelte';
  import InstallMenu from '../components/InstallMenu.svelte';
  import { homeAtmosphere } from '../art';

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
  const dur = $derived(app.reduceMotion ? 0 : 320);
  const empty = $derived(!g);
  const addFolder = () => app.addPs2Folder();
</script>

<div class="home">
  <div class="hero-art" aria-hidden="true">
    {#key g?.id ?? 'atmosphere'}
      <div class="hero-layer" transition:fade={{ duration: dur * 1.4 }}>
        {#if g?.coverUrl}
          <img class="hero-cover" src={g.coverUrl} alt="" />
        {:else}
          <Artwork art={g?.art ?? homeAtmosphere} />
        {/if}
      </div>
    {/key}
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
    {#key g.id}
      <div class="info" in:fly={{ y: 14, duration: dur, opacity: 0 }}>
        <div class="eyebrow">
          <span class="pill">{sourceLabel[g.source]}</span>
          <span>{platformLabel[g.platform]}</span>
          {#if g.genre}<span class="dot">•</span><span>{g.genre}</span>{/if}
          {#if g.year}<span class="dot">•</span><span>{g.year}</span>{/if}
        </div>
        <Logo game={g} size="xl" />
        <p class="tagline">{g.tagline}</p>
        <div class="meta">
          {#if g.playtimeMin}<span><Icon name="clock" size={16} /> {hours(g.playtimeMin)}</span>{/if}
          {#if g.lastPlayed}<span>Last played {ago(g.lastPlayed)}</span>{/if}
        </div>
        <div class="actions">
          {#if !app.demo && g.setupState === 'runner_missing'}
            {#if g.platform === 'ps1'}<button class="btn primary" data-nav onclick={() => app.go('tools')}>Set up DuckStation</button>{:else}<InstallMenu tone="primary" />{/if}
          {:else}
            <button class="btn primary" data-nav style="--accent-h:{g.art.hue2}" onclick={() => {
              if (!app.demo && g.mediaMissing) app.open(g);
              else if (!app.demo && g.setupState === 'missing') void app.openSetup(g.runnerId, g.platform);
              else if (g.installed) app.play(g);
              else if (app.demo) app.install(g.title, { kind: 'game', id: g.id });
              else app.open(g);
            }}>
              <Icon name={g.installed ? 'play' : 'download'} />
              {!app.demo && g.mediaMissing ? 'Check disc files' : !app.demo && g.setupState === 'missing' ? 'Open emulator setup' : g.installed ? 'Play' : app.demo ? 'Install' : 'Details'}
            </button>
          {/if}
          <button class="btn glass" data-nav onclick={() => app.open(g)}>Details</button>
          <button class="btn glass icon" data-nav aria-label="Favourite" onclick={() => app.toggleFavorite(g)}>
            <Icon name="star" filled={g.favorite} />
          </button>
        </div>
      </div>
    {/key}
    {/if}
  </header>

  <Row title="Continue playing" games={continuePlaying} shape="wide" placeholders={empty ? 4 : 0} onPlaceholder={addFolder} />
  {#if app.demo}
    <Row title="Ports ready to play" games={portsReady} />
    <Row title="Hidden gems in your backlog" games={hiddenGems} />
  {/if}
  <Row title="Favourites" games={favorites} placeholders={empty ? 6 : 0} />
  {#if app.demo}
    <Row title="Quick wins · under 6 hours" games={quickWins} />
  {/if}
  <Row title="Recently added" games={recentlyAdded} placeholders={empty ? 8 : 0} onPlaceholder={addFolder} />
  <Row title="PlayStation 2" games={emulated} placeholders={empty ? 8 : 0} onPlaceholder={addFolder} />
  {#if app.demo}
    <Row title="In your library, not installed" games={notInstalled} />
  {/if}
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
    min-height: var(--hero-h);
    display: flex;
    align-items: flex-end;
    padding: 0 var(--gutter) 28px;
  }
  .info {
    max-width: 720px;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .eyebrow {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    font-size: 13px;
    color: var(--text-dim);
    font-weight: 600;
  }
  .dot {
    opacity: 0.5;
  }
  .tagline {
    margin: 0;
    font-size: var(--tagline);
    color: var(--text-soft);
    max-width: 52ch;
  }
  .meta {
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
    gap: 10px;
    margin-top: 6px;
  }
</style>
