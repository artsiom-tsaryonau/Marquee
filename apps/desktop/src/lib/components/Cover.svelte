<script lang="ts">
  import { gameStatus } from '../presentation';
  import type { GameEntry } from '../types';
  import { platformLabel, sourceLabel } from '../labels';
  import { cleanTitle, hours, titleRegion } from '../format';
  import { app } from '../state.svelte';
  import Icon from './Icon.svelte';
  import Mark, { hasMark } from './Mark.svelte';
  import { caseRatio } from '../coverLayout';
  import { gameInitials } from '../platformIcons';
  import PlatformIcon from './PlatformIcon.svelte';

  let {
    game,
    shape = 'cover',
    showTitle = false,
    shelf = false,
    platformContext = false,
  }: { game: GameEntry; shape?: 'cover' | 'wide'; showTitle?: boolean; shelf?: boolean; platformContext?: boolean } = $props();

  const status = $derived(gameStatus(game));
  const imageUrl = $derived(shape === 'wide' ? game.heroUrl ?? game.coverUrl : game.coverUrl);
  let loaded = $state<{ url: string; ratio: number }>();
  let failedUrl = $state<string>();
  const hasImage = $derived(!!imageUrl && failedUrl !== imageUrl);
  // Reserve a plausible case size until the real artwork dimensions arrive.
  const defaultRatio = $derived(shape === 'wide' && game.heroUrl ? 16 / 9 : caseRatio(game.platform));
  const ratio = $derived(hasImage && loaded && loaded.url === imageUrl ? loaded.ratio : defaultRatio);
</script>

<div class="wrap {shape}" class:shelf style="--art-ratio:{ratio}">
  <div class="stage">
  <button
    class="card"
    data-nav
    style="--accent-h:{game.art.hue2}"
    onfocus={() => { if (app.mode === 'big') app.preview(game.id); }}
    onclick={() => app.open(game)}
    aria-label={`${cleanTitle(game.title)}${status ? ` · ${status.label}` : ''}`}
  >
    <div class="art" class:generated={!hasImage}>
      {#if hasImage}
        <img class="cover-img" src={imageUrl} alt="" loading="lazy" decoding="async"
          onload={(event) => {
            const image = event.currentTarget;
            if (image instanceof HTMLImageElement && image.naturalHeight && imageUrl) loaded = { url: imageUrl, ratio: image.naturalWidth / image.naturalHeight };
          }}
          onerror={() => { failedUrl = imageUrl; }} />
      {:else}
        <div class="fallback">
          <div class="fallback-symbols" aria-hidden="true"><PlatformIcon platform={game.platform} size={36} /><span class="game-initials">{gameInitials(cleanTitle(game.title))}</span></div>
          {#if !showTitle}<span class="fallback-title">{cleanTitle(game.title)}</span>{/if}
          {#if !platformContext}<span class="plat">{platformLabel[game.platform]}</span>{/if}
          <span class="missing-label">No cover</span>
        </div>
      {/if}
    </div>
    <span class="source" title={sourceLabel[game.source]}>{#if hasMark(game.source)}<Mark id={game.source} size={14} /><span class="sr">{sourceLabel[game.source]}</span>{:else}{sourceLabel[game.source]}{/if}</span>
    {#if game.favorite}<span class="fav"><Icon name="star" size={14} filled /></span>{/if}
    {#if status}
      <span class="status {status.tone}">{status.label}</span>
    {/if}
  </button>
  </div>
  {#if showTitle}
    <div class="caption">
      <span class="t" title={game.title}>{cleanTitle(game.title)}</span>
      <span class="m">{game.playtimeMin ? `${hours(game.playtimeMin)} played` : [game.source === 'catalogue' ? 'Catalogue only' : game.genre, game.region ?? titleRegion(game.title), game.revision ? `rev ${game.revision}` : ''].filter(Boolean).join(' · ')}</span>
    </div>
  {/if}
</div>

<style>
  .fallback-symbols { display: flex; align-items: center; justify-content: center; gap: 12px; }
  .game-initials { font-size: 25px; font-weight: 750; letter-spacing: -.04em; color: #eaf1ff; }
  .missing-label { font-size: 9px; letter-spacing: .08em; text-transform: uppercase; color: #c4d0e4; }
  .wrap {
    --art-h: calc(var(--cover-w) / var(--slot-ratio, .6666667));
    flex: none;
    width: var(--cover-w);
  }
  .wrap.wide {
    --art-h: calc(var(--cover-w) * 1.25);
    width: calc(var(--art-h) * clamp(0.55, var(--art-ratio), 2.2));
  }
  .stage {
    height: var(--art-h);
    display: flex;
    align-items: flex-end;
    justify-content: center;
  }
  .wrap.shelf { --art-h: calc(var(--cover-w) * 1.35); width: calc(var(--art-h) * clamp(0.55, var(--art-ratio), 2.2)); }
  .shelf .card { width: 100%; height: 100%; aspect-ratio: auto; }
  .card {
    position: relative;
    display: block;
    width: min(100%, calc(var(--art-h) * var(--art-ratio)));
    aspect-ratio: var(--art-ratio);
    max-height: 100%;
    flex: none;
    border: 0;
    padding: 0;
    border-radius: var(--radius);
    overflow: hidden;
    cursor: pointer;
    background: transparent;
    transition: box-shadow 180ms var(--ease);
    outline: none;
  }
  .card:hover,
  .card:focus-visible {
    z-index: 2;
    box-shadow: 0 0 0 2px hsl(var(--accent-h) 95% 72%);
  }
  .card:focus-visible {
    outline: 3px solid var(--text);
    outline-offset: 4px;
  }
  .art { position: absolute; inset: 0; }
  .cover-img { width: 100%; height: 100%; object-fit: contain; display: block; }
  .art.generated {
    background: linear-gradient(150deg, hsl(var(--accent-h) 12% 19%), hsl(var(--accent-h) 10% 10%));
    color: #eee;
  }
  .fallback {
    height: 100%;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 14px;
    padding: 20px 12px;
    text-align: center;
  }
  .fallback-title {
    font-size: 14px;
    font-weight: 700;
    display: -webkit-box;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;
    overflow-wrap: anywhere;
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
    min-height: 2.5em;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    overflow-wrap: anywhere;
  }
  .plat {
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.04em;
    color: rgb(255 255 255 / 0.65);
  }
  .caption .m {
    color: var(--text-dim);
    font-size: 12px;
  }
</style>
