<script lang="ts">
  import { fade, fly } from 'svelte/transition';
  import { app } from '../state.svelte';
  import { platformLabel, sourceLabel } from '../mock';
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
  const notInstalled = $derived(app.games.filter((g) => !g.installed));

  const g = $derived(app.hero);
  const dur = $derived(app.reduceMotion ? 0 : 320);
</script>

<div class="home">
  <div class="hero-art" aria-hidden="true">
    {#key g.id}
      <div class="hero-layer" transition:fade={{ duration: dur * 1.4 }}><Artwork art={g.art} /></div>
    {/key}
  </div>
  <header class="hero">
    {#key g.id}
      <div class="info" in:fly={{ y: 14, duration: dur, opacity: 0 }}>
        <div class="eyebrow">
          <span class="pill">{sourceLabel[g.source]}</span>
          <span>{platformLabel[g.platform]}</span>
          <span class="dot">•</span>
          <span>{g.genre}</span>
          <span class="dot">•</span>
          <span>{g.year}</span>
        </div>
        <Logo game={g} size="xl" />
        <p class="tagline">{g.tagline}</p>
        <div class="meta">
          {#if g.playtimeMin}<span><Icon name="clock" size={16} /> {hours(g.playtimeMin)}</span>{/if}
          {#if g.lastPlayed}<span>Last played {ago(g.lastPlayed)}</span>{/if}
        </div>
        <div class="actions">
          <button class="btn primary" data-nav style="--accent-h:{g.art.hue2}" onclick={() => (g.installed ? app.play(g) : app.install(g.title))}>
            <Icon name={g.installed ? 'play' : 'download'} />
            {g.installed ? 'Play' : 'Install'}
          </button>
          <button class="btn glass" data-nav onclick={() => app.open(g)}>Details</button>
          <button class="btn glass icon" data-nav aria-label="Favourite" onclick={() => app.toggleFavorite(g)}>
            <Icon name="star" filled={g.favorite} />
          </button>
        </div>
      </div>
    {/key}
  </header>

  <Row title="Continue playing" games={continuePlaying} shape="wide" />
  <Row title="Ports ready to play" games={portsReady} />
  <Row title="Favourites" games={favorites} />
  <Row title="Recently added" games={recentlyAdded} />
  <Row title="Emulated classics" games={emulated} />
  <Row title="In your library, not installed" games={notInstalled} />
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
</style>
