<script lang="ts">
  import { fade, fly } from 'svelte/transition';
  import { app } from '../state.svelte';
  import { platformLabel, sourceLabel } from '../labels';
  import { ago, hours } from '../format';
  import Row from '../components/Row.svelte';
  import Logo from '../components/Logo.svelte';
  import Icon from '../components/Icon.svelte';
  import Artwork from '../components/Artwork.svelte';

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
</script>

<div class="home">
  {#if g}
  <div class="hero-art" aria-hidden="true">
    {#key g.id}
      <div class="hero-layer" transition:fade={{ duration: dur * 1.4 }}><Artwork art={g.art} /></div>
    {/key}
  </div>
  {/if}
  <header class="hero">
    {#if empty}
      <div class="info">
        <p class="eyebrow"><span class="pill">PlayStation 2</span></p>
        <h1 class="empty-title">Your library is empty</h1>
        <p class="tagline">Choose a folder of PS2 .iso files. Marquee will not download games or BIOS files.</p>
        {#if app.libraryStatus === 'failed'}
          <p class="tagline">{app.loadError}</p>
        {/if}
        <div class="actions">
          <button class="btn primary" data-nav onclick={() => app.addPs2Folder()}>
            <Icon name="plus" /> Add PS2 folder
          </button>
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
          <button class="btn primary" data-nav style="--accent-h:{g.art.hue2}" onclick={() => (g.installed ? app.play(g) : app.demo ? app.install(g.title, { kind: 'game', id: g.id }) : app.addPs2Folder())}>
            <Icon name={g.installed ? 'play' : 'download'} />
            {g.installed ? 'Play' : app.demo ? 'Install' : 'Reconnect'}
          </button>
          <button class="btn glass" data-nav onclick={() => app.open(g)}>Details</button>
          <button class="btn glass icon" data-nav aria-label="Favourite" onclick={() => app.toggleFavorite(g)}>
            <Icon name="star" filled={g.favorite} />
          </button>
        </div>
      </div>
    {/key}
    {/if}
  </header>

  <Row title="Continue playing" games={continuePlaying} shape="wide" />
  {#if app.demo}
    <Row title="Ports ready to play" games={portsReady} />
    <Row title="Hidden gems in your backlog" games={hiddenGems} />
  {/if}
  <Row title="Favourites" games={favorites} />
  {#if app.demo}
    <Row title="Quick wins · under 6 hours" games={quickWins} />
  {/if}
  <Row title="Recently added" games={recentlyAdded} />
  <Row title="PlayStation 2" games={emulated} />
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
    mask-image: linear-gradient(180deg, #000 55%, transparent), linear-gradient(90deg, transparent 0, #000 45%);
    mask-composite: intersect;
  }
  .hero-layer {
    position: absolute;
    inset: 0;
    animation: kenburns 24s ease-in-out infinite alternate;
  }
  @keyframes kenburns {
    from {
      transform: scale(1.02);
    }
    to {
      transform: scale(1.12) translate(-1.5%, 1%);
    }
  }
  .hero {
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
  .empty-title {
    margin: 0;
    font-size: clamp(36px, 6vw, 72px);
    letter-spacing: -0.04em;
  }
</style>
