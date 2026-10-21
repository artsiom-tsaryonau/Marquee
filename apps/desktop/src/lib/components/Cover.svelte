<script lang="ts">
  import type { GameEntry } from '../types';
  import { sourceLabel } from '../labels';
  import { app } from '../state.svelte';
  import Artwork from './Artwork.svelte';
  import Logo from './Logo.svelte';
  import Icon from './Icon.svelte';
  import Mark, { hasMark } from './Mark.svelte';

  let {
    game,
    shape = 'cover',
    showTitle = false,
  }: { game: GameEntry; shape?: 'cover' | 'wide'; showTitle?: boolean } = $props();

  const status = $derived(
    game.portStatus === 'update'
      ? { label: 'Update', tone: 'warn' }
      : game.source === 'port'
        ? { label: game.installed ? 'Native port' : 'Not installed', tone: game.installed ? 'ok' : 'dim' }
        : game.rootOffline
          ? { label: 'Drive offline', tone: 'dim' }
          : game.mediaMissing
            ? { label: 'Missing', tone: 'warn' }
            : game.setupState === 'runner_missing'
              ? { label: game.platform === 'ps1' ? 'Needs DuckStation' : 'Needs PCSX2', tone: 'warn' }
              : game.setupState === 'missing'
                ? { label: 'Needs BIOS', tone: 'warn' }
                : !game.installed
                  ? { label: 'Not installed', tone: 'dim' }
                  : null,
  );
</script>

<div class="wrap {shape}">
  <button
    class="card"
    data-nav
    style="--accent-h:{game.art.hue2}"
    onfocus={() => app.preview(game.id)}
    onpointerenter={() => app.preview(game.id)}
    onclick={() => app.open(game)}
    aria-label={game.title}
  >
    <div class="art">
      {#if game.coverUrl}
        <img class="cover-img" src={game.coverUrl} alt="" loading="lazy" decoding="async" />
      {:else}
        <Artwork art={game.art} grain={shape === 'wide'} />
      {/if}
    </div>
    <div class="shade"></div>
    {#if !game.coverUrl}
      <div class="logo"><Logo {game} size={shape === 'wide' ? 'md' : 'sm'} /></div>
    {/if}
    <span class="source" title={sourceLabel[game.source]}>{#if hasMark(game.source)}<Mark id={game.source} size={14} /><span class="sr">{sourceLabel[game.source]}</span>{:else}{sourceLabel[game.source]}{/if}</span>
    {#if game.favorite}<span class="fav"><Icon name="star" size={14} filled /></span>{/if}
    {#if status?.tone === 'dim'}
      <span class="status dim icon" title="Not installed"><Icon name="download" size={13} /></span>
    {:else if status}
      <span class="status {status.tone}">{status.label}</span>
    {/if}
  </button>
  {#if showTitle}
    <div class="caption">
      <span class="t">{game.title}</span>
      <span class="m">{game.playtimeMin ? `${Math.round(game.playtimeMin / 60)} h played` : [game.source === 'catalogue' ? 'Catalogue only' : game.genre, game.region, game.revision ? `rev ${game.revision}` : ''].filter(Boolean).join(' · ')}</span>
    </div>
  {/if}
</div>

<style>
  .wrap {
    flex: none;
    width: var(--cover-w);
  }
  .wrap.wide {
    width: calc(var(--cover-w) * 2.1);
  }
  .card {
    position: relative;
    display: block;
    width: 100%;
    aspect-ratio: 2 / 3;
    border: 0;
    padding: 0;
    border-radius: var(--radius);
    overflow: hidden;
    cursor: pointer;
    background: hsl(var(--accent-h) 30% 15%);
    box-shadow: 0 10px 30px -12px rgb(0 0 0 / 0.7);
    transition: box-shadow 180ms var(--ease);
    outline: none;
  }
  .wide .card {
    aspect-ratio: 16 / 9.4;
  }
  .card:hover,
  .card:focus-visible {
    z-index: 2;
    box-shadow:
      0 0 0 2px hsl(var(--accent-h) 95% 72%),
      0 0 32px 4px hsl(var(--accent-h) 95% 60% / 0.55),
      0 24px 50px -16px rgb(0 0 0 / 0.9);
  }
  .art {
    position: absolute;
    inset: 0;
  }
  .cover-img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }
  .shade {
    position: absolute;
    inset: 0;
    background: linear-gradient(180deg, transparent 45%, rgb(0 0 0 / 0.65));
  }
  .logo {
    position: absolute;
    left: 12px;
    right: 12px;
    bottom: 12px;
    text-align: left;
  }
  .source,
  .status {
    position: absolute;
    top: 10px;
    font-size: 10.5px;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    padding: 4px 8px;
    border-radius: 999px;
    backdrop-filter: var(--badge-filter);
    -webkit-backdrop-filter: var(--badge-filter);
  }
  .source {
    left: 10px;
    color: #fff;
    background: rgb(0 0 0 / 0.35);
    border: 1px solid rgb(255 255 255 / 0.18);
  }
  .status {
    right: 10px;
  }
  .status.ok {
    background: rgb(52 211 153 / 0.85);
    color: #04261a;
  }
  .status.warn {
    background: rgb(251 191 36 / 0.92);
    color: #2b1b00;
  }
  .status.icon {
    padding: 4px;
    display: grid;
  }
  .status.dim {
    background: rgb(0 0 0 / 0.45);
    color: rgb(255 255 255 / 0.75);
  }
  .fav {
    position: absolute;
    right: 10px;
    bottom: 10px;
    color: #fde68a;
  }
  :global(:root[data-effects='full']) .fav { filter: drop-shadow(0 0 6px rgb(253 230 138 / 0.8)); }
  .caption {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 10px 2px 0;
  }
  .caption .t {
    font-weight: 650;
    font-size: 14px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .caption .m {
    color: var(--text-dim);
    font-size: 12px;
  }
</style>
