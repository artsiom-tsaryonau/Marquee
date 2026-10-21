<script lang="ts">
  import { gameStatus } from '../presentation';
  import type { GameEntry } from '../types';
  import { platformLabel, sourceLabel } from '../labels';
  import { cleanTitle, hours, titleRegion } from '../format';
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

  const status = $derived(gameStatus(game));
</script>

<div class="wrap {shape}">
  <button
    class="card"
    data-nav
    style="--accent-h:{game.art.hue2}"
    onfocus={() => { if (app.mode === 'big') app.preview(game.id); }}
    onclick={() => app.open(game)}
    aria-label={`${cleanTitle(game.title)}${status ? ` · ${status.label}` : ''}`}
  >
    <div class="art" class:generated={!game.coverUrl}>
      {#if game.coverUrl}
        <img class="cover-img" src={game.coverUrl} alt="" loading="lazy" decoding="async" />
      {:else}
        <Artwork art={game.art} grain={shape === 'wide'} />
      {/if}
    </div>
    <div class="shade"></div>
    {#if !game.coverUrl && !showTitle}
      <div class="logo"><Logo {game} size={shape === 'wide' ? 'md' : 'sm'} /></div>
    {:else if !game.coverUrl && game.platform !== 'unknown'}
      <!-- The title is written under the card; repeating it on generated art is noise. -->
      <span class="plat">{platformLabel[game.platform]}</span>
    {/if}
    <span class="source" title={sourceLabel[game.source]}>{#if hasMark(game.source)}<Mark id={game.source} size={14} /><span class="sr">{sourceLabel[game.source]}</span>{:else}{sourceLabel[game.source]}{/if}</span>
    {#if game.favorite}<span class="fav"><Icon name="star" size={14} filled /></span>{/if}
    {#if status}
      <span class="status {status.tone}">{status.label}</span>
    {/if}
  </button>
  {#if showTitle}
    <div class="caption">
      <span class="t" title={game.title}>{cleanTitle(game.title)}</span>
      <span class="m">{game.playtimeMin ? `${hours(game.playtimeMin)} played` : [game.source === 'catalogue' ? 'Catalogue only' : game.genre, game.region ?? titleRegion(game.title), game.revision ? `rev ${game.revision}` : ''].filter(Boolean).join(' · ')}</span>
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
  .art.generated { filter:saturate(.5) brightness(.8); }
  .wide .cover-img { object-fit:contain; }
  .logo {
    position: absolute;
    left: 12px;
    right: 12px;
    bottom: 42px;
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
    right: 8px;
    top: auto;
    bottom: 8px;
    max-width: calc(100% - 16px);
    font-size: 10px;
    text-transform: none;
    letter-spacing: 0;
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
  .status.note {
    background: rgb(99 102 241 / 0.88);
    color: #fff;
  }
  .status.dim {
    background: rgb(0 0 0 / 0.45);
    color: rgb(255 255 255 / 0.75);
  }
  .fav {
    position: absolute;
    right: 10px;
    bottom: auto;
    top: 10px;
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
    line-height: 1.25;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    overflow-wrap: anywhere;
  }
  .plat {
    position: absolute;
    left: 12px;
    right: 12px;
    bottom: 42px;
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: rgb(255 255 255 / 0.8);
  }
  .caption .m {
    color: var(--text-dim);
    font-size: 12px;
  }
</style>
