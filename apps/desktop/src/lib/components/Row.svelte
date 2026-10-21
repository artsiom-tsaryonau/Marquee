<script lang="ts">
  import type { GameEntry } from '../types';
  import Cover from './Cover.svelte';
  import GhostCover from './GhostCover.svelte';

  let {
    title,
    games,
    shape = 'cover',
    placeholders = 0,
    onPlaceholder,
  }: {
    title: string;
    games: GameEntry[];
    shape?: 'cover' | 'wide';
    placeholders?: number;
    onPlaceholder?: () => void;
  } = $props();
  let visibleCount = $state(24);
</script>

{#if games.length || placeholders}
  <section class="row">
    <h2>{title} {#if games.length}<span>{games.length}</span>{/if}</h2>
    <div class="scroller" data-nav-row>
      {#each games.slice(0, visibleCount) as game (game.id)}
        <Cover {game} {shape} />
      {/each}
      {#if games.length > visibleCount}
        <button class="more btn glass" data-nav onclick={() => (visibleCount += 24)}>Show more · {visibleCount} of {games.length}</button>
      {/if}
      {#each Array(placeholders) as _, i (i)}
        <GhostCover {shape} onclick={i === 0 ? onPlaceholder : undefined} />
      {/each}
    </div>
  </section>
{/if}

<style>
  .row {
    position: relative;
    z-index: 1;
    margin-bottom: 18px;
  }
  h2 {
    margin: 0 0 4px;
    padding: 0 var(--gutter);
    font-size: var(--row-title);
    font-weight: 700;
    letter-spacing: -0.01em;
  }
  h2 span {
    color: var(--text-dim);
    font-weight: 500;
    font-size: 0.8em;
    margin-left: 6px;
  }
  .scroller {
    display: flex;
    gap: var(--card-gap);
    overflow-x: auto;
    padding: 16px var(--gutter) 22px;
    scroll-padding: 0 var(--gutter);
    scrollbar-width: none;
    -webkit-mask-image: linear-gradient(90deg, transparent 0, #000 calc(var(--gutter) * 0.6), #000 calc(100% - var(--gutter) * 0.6), transparent);
    mask-image: linear-gradient(90deg, transparent 0, #000 calc(var(--gutter) * 0.6), #000 calc(100% - var(--gutter) * 0.6), transparent);
  }
  .scroller::-webkit-scrollbar {
    display: none;
  }
  .more { flex: none; align-self: center; }
</style>
