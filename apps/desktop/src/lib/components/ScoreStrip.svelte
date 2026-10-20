<script lang="ts">
  // Scores and facts from rating sites; each tile names its source.
  import type { GameEntry } from '../types';
  import { app } from '../state.svelte';
  import Icon from './Icon.svelte';

  let { game }: { game: GameEntry } = $props();

  const r = $derived(game.ratings);

  function steamLabel(p: number, n: number) {
    if (p >= 95 && n >= 500) return 'Overwhelmingly Positive';
    if (p >= 80) return 'Very Positive';
    if (p >= 70) return 'Mostly Positive';
    if (p >= 40) return 'Mixed';
    return 'Mostly Negative';
  }
  const compact = (n: number) => (n >= 1000 ? `${Math.round(n / 100) / 10}k` : `${n}`);
  const tone = (v: number) => (v >= 85 ? 'great' : v >= 70 ? 'good' : 'meh');

  const links = $derived(
    [
      game.source === 'steam' && 'SteamDB',
      'Metacritic',
      'HowLongToBeat',
      game.platform.startsWith('pc') && 'PCGamingWiki',
      game.source === 'rom' && 'RetroAchievements',
    ].filter(Boolean) as string[],
  );

  // Gauge ring: circumference of r=26.
  const C = 2 * Math.PI * 26;
</script>

<div class="strip">
  {#if r.critic !== null}
    <div class="tile glass-panel">
      <svg class="ring {tone(r.critic)}" viewBox="0 0 60 60" aria-hidden="true">
        <circle cx="30" cy="30" r="26" class="track" />
        <circle cx="30" cy="30" r="26" class="value" stroke-dasharray="{(r.critic / 100) * C} {C}" />
        <text x="30" y="35">{r.critic}</text>
      </svg>
      <div><strong>Critics</strong><span>IGDB · {r.criticCount} reviews</span></div>
    </div>
  {/if}
  {#if r.user !== null}
    <div class="tile glass-panel">
      <svg class="ring {tone(r.user)}" viewBox="0 0 60 60" aria-hidden="true">
        <circle cx="30" cy="30" r="26" class="track" />
        <circle cx="30" cy="30" r="26" class="value" stroke-dasharray="{(r.user / 100) * C} {C}" />
        <text x="30" y="35">{r.user}</text>
      </svg>
      <div><strong>Players</strong><span>IGDB user rating</span></div>
    </div>
  {/if}
  {#if r.steamReviews}
    <div class="tile glass-panel wide">
      <div class="bar"><i class={tone(r.steamReviews.percent)} style="--h:{r.steamReviews.percent}%"></i></div>
      <div>
        <strong class="steam {tone(r.steamReviews.percent)}">{steamLabel(r.steamReviews.percent, r.steamReviews.count)}</strong>
        <span>Steam · {r.steamReviews.percent}% of {compact(r.steamReviews.count)}</span>
      </div>
    </div>
  {/if}
  {#if game.timeToBeat}
    <div class="tile glass-panel wide">
      <Icon name="clock" size={22} />
      <div class="ttb">
        <span><b>{game.timeToBeat[0]} h</b> main</span>
        <span><b>{game.timeToBeat[1]} h</b> + extras</span>
        <span><b>{game.timeToBeat[2]} h</b> 100%</span>
      </div>
    </div>
  {/if}
  {#if game.achievements}
    <div class="tile glass-panel">
      <Icon name="star" size={22} />
      <div>
        <strong>{game.achievements.unlocked} / {game.achievements.total}</strong>
        <span>{game.source === 'rom' ? 'RetroAchievements' : 'Steam achievements'}</span>
      </div>
    </div>
  {/if}
</div>

<div class="links">
  {#each links as l (l)}
    <button class="chip" data-nav onclick={() => app.notify(`Opens ${l} in your browser`)}>{l} ↗</button>
  {/each}
  <button class="chip" data-nav onclick={() => app.notify('Pick the right game from IGDB matches')}>Wrong game?</button>
</div>

<style>
  .strip {
    display: flex;
    flex-wrap: wrap;
    gap: 12px;
  }
  .tile {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 16px 10px 10px;
    border-radius: 16px;
    min-height: 72px;
  }
  .tile.wide {
    padding-left: 16px;
  }
  .tile div {
    display: grid;
    gap: 2px;
  }
  .tile strong {
    font-size: 15px;
  }
  .tile span {
    font-size: 12px;
    color: var(--text-dim);
  }
  .ring {
    width: 52px;
    height: 52px;
    rotate: -90deg;
  }
  .ring circle {
    fill: none;
    stroke-width: 5;
  }
  .ring .track {
    stroke: var(--field-strong);
  }
  .ring .value {
    stroke-linecap: round;
    stroke: currentColor;
    filter: drop-shadow(0 0 4px currentColor);
  }
  .ring text {
    rotate: 90deg;
    transform-origin: 30px 30px;
    text-anchor: middle;
    font-size: 17px;
    font-weight: 800;
    fill: var(--text);
  }
  .great {
    color: #34d399;
  }
  .good {
    color: #a3e635;
  }
  .meh {
    color: #fbbf24;
  }
  .steam {
    color: inherit;
  }
  .steam.great,
  .steam.good {
    color: #66c0f4;
  }
  .bar {
    width: 6px;
    height: 44px;
    border-radius: 3px;
    background: var(--field-strong);
    display: flex !important;
    align-items: flex-end;
    overflow: hidden;
  }
  .bar i {
    display: block;
    width: 100%;
    background: currentColor;
    height: var(--h, 100%);
  }
  .ttb {
    grid-auto-flow: column;
    gap: 14px !important;
  }
  .ttb span {
    color: var(--text-dim);
  }
  .ttb b {
    color: var(--text);
    font-size: 15px;
  }
  .links {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: 12px;
  }
</style>
