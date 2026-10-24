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
    onViewAll,
    initialCount = 24,
  }: {
    title: string;
    games: GameEntry[];
    shape?: 'cover' | 'wide';
    placeholders?: number;
    onPlaceholder?: () => void;
    onViewAll?: () => void;
    initialCount?: number;
  } = $props();
  let visibleCount = $state(24);
  $effect(() => { visibleCount = initialCount; });
  let scroller = $state<HTMLDivElement>();
  function scroll(direction: number) { scroller?.scrollBy({ left: direction * scroller.clientWidth * .8, behavior: matchMedia('(prefers-reduced-motion: reduce)').matches ? 'auto' : 'smooth' }); }
</script>

{#if games.length || placeholders}
  <section class="row">
    <div class="heading"><h2>{title} {#if games.length}<span>{games.length}</span>{/if}</h2><div class="row-actions">{#if onViewAll}<button class="view-all" data-nav onclick={onViewAll}>View all →</button>{/if}<button data-nav aria-label={`Scroll ${title} left`} onclick={() => scroll(-1)}>←</button><button data-nav aria-label={`Scroll ${title} right`} onclick={() => scroll(1)}>→</button></div></div>
    <div bind:this={scroller} class="scroller" data-nav-row>
      {#each games.slice(0, visibleCount) as game (game.id)}
        <Cover {game} {shape} showTitle shelf />
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
  .heading { display:flex; align-items:center; justify-content:space-between; gap:16px; padding-right:var(--gutter); }
  .row-actions { display:flex; gap:6px; }
  .row-actions button { border:1px solid var(--line); background:var(--field); color:var(--text); border-radius:8px; min-width:36px; min-height:32px; cursor:pointer; font:inherit; }
  :global([data-mode='big']) .row-actions button { min-width:48px; min-height:48px; }
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
    scrollbar-width: thin;
    -webkit-mask-image: linear-gradient(90deg, transparent 0, #000 calc(var(--gutter) * 0.6), #000 calc(100% - var(--gutter) * 0.6), transparent);
    mask-image: linear-gradient(90deg, transparent 0, #000 calc(var(--gutter) * 0.6), #000 calc(100% - var(--gutter) * 0.6), transparent);
  }
  .scroller::-webkit-scrollbar {
    height: 6px;
  }
  .more { flex: none; align-self: center; }
</style>
