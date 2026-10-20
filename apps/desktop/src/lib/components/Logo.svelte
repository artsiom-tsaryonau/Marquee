<script lang="ts">
  // Typographic stand-in for a game's transparent logo image.
  import type { GameEntry } from '../types';

  let { game, size = 'md' }: { game: Pick<GameEntry, 'title' | 'art'>; size?: 'sm' | 'md' | 'lg' | 'xl' } = $props();
</script>

<div class="logo {game.art.logo} {size}" style="--h:{game.art.hue2}">
  {game.title}
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
    filter: drop-shadow(0 4px 24px hsl(var(--h) 90% 40% / 0.55)) drop-shadow(0 2px 2px rgb(0 0 0 / 0.5));
  }
  .wide {
    text-transform: uppercase;
    letter-spacing: 0.14em;
    font-weight: 700;
  }
  .serif {
    font-family: 'Iowan Old Style', 'Palatino Linotype', Palatino, Georgia, serif;
    font-weight: 600;
    font-style: italic;
    letter-spacing: 0;
  }
  .mono {
    font-family: ui-monospace, 'JetBrains Mono', 'Cascadia Code', monospace;
    text-transform: uppercase;
    font-weight: 700;
    letter-spacing: 0.04em;
  }
  .heavy {
    font-weight: 800;
    text-transform: uppercase;
    letter-spacing: -0.03em;
  }
  .sm {
    font-size: 15px;
  }
  .wide.sm,
  .mono.sm {
    font-size: 12.5px;
    letter-spacing: 0.06em;
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
  .wide.lg,
  .wide.xl {
    font-size: clamp(30px, 4.2vw, 60px);
  }
</style>
