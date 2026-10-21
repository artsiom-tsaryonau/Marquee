<script lang="ts">
  // Typographic stand-in for a game's transparent logo image.
  import { cleanTitle } from '../format';
  import type { GameEntry } from '../types';

  let { game, size = 'md' }: { game: Pick<GameEntry, 'title' | 'art'>; size?: 'sm' | 'md' | 'lg' | 'xl' } = $props();
</script>

<div class="logo {size}" style="--h:{game.art.hue2}">
  {cleanTitle(game.title)}
</div>

<style>
  .logo {
    font-weight: 800;
    line-height: 0.95;
    letter-spacing: -0.02em;
    color: #fff;
    text-wrap: balance;
    background: linear-gradient(180deg, #fff 30%, hsl(var(--h) 90% 82%));
    -webkit-background-clip: text;
    background-clip: text;
    -webkit-text-fill-color: transparent;
  }
  :global(:root[data-effects='full']) .logo {
    filter: drop-shadow(0 4px 24px hsl(var(--h) 90% 40% / 0.55)) drop-shadow(0 2px 2px rgb(0 0 0 / 0.5));
  }
  .sm {
    font-size: 15px;
  }
  .md {
    font-size: 22px;
  }
  .lg {
    font-size: clamp(36px, 5vw, 64px);
  }
  .xl {
    font-size: clamp(44px, 7vw, 96px);
  }
</style>
