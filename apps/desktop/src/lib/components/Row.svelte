<script lang="ts">
  import type { GameEntry } from '../types';
  import Cover from './Cover.svelte';

  let { title, games, shape = 'cover' }: { title: string; games: GameEntry[]; shape?: 'cover' | 'wide' } = $props();
</script>

{#if games.length}
  <section class="row">
    <h2>{title} <span>{games.length}</span></h2>
    <div class="scroller" data-nav-row>
      {#each games as game (game.id)}
        <Cover {game} {shape} />
      {/each}
    </div>
  </section>
{/if}

<style>
  .row {
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
    mask-image: linear-gradient(90deg, transparent 0, #000 calc(var(--gutter) * 0.6), #000 calc(100% - var(--gutter) * 0.6), transparent);
  }
  .scroller::-webkit-scrollbar {
    display: none;
  }
</style>
