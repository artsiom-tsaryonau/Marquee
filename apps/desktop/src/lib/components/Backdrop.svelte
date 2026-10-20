<script lang="ts">
  // The focused game's art, blurred behind everything; cross-fades on change.
  import { fade } from 'svelte/transition';
  import { app } from '../state.svelte';
  import Artwork from './Artwork.svelte';

  let { blur = true }: { blur?: boolean } = $props();
  const dur = $derived(app.reduceMotion ? 0 : 450);
</script>

<div class="backdrop" class:sharp={!blur}>
  {#if app.hero}
  {#key app.hero.id}
    <div class="layer" transition:fade={{ duration: dur }}>
      <Artwork art={app.hero.art} grain={false} />
    </div>
  {/key}
  <div class="tint" style="--h:{app.hero.art.hue}"></div>
  {:else}
  <div class="tint" style="--h:260"></div>
  {/if}
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: -1;
    overflow: hidden;
    background: var(--bg);
  }
  .layer {
    position: absolute;
    inset: -8%;
    filter: blur(var(--backdrop-blur)) saturate(1.3);
    transform: scale(1.1);
    opacity: var(--backdrop-opacity);
  }
  .sharp .layer {
    filter: saturate(1.1);
  }
  .tint {
    position: absolute;
    inset: 0;
    background:
      radial-gradient(120% 80% at 70% 0%, transparent, var(--bg) 85%),
      linear-gradient(180deg, color-mix(in oklab, var(--bg) 20%, transparent), var(--bg) 92%);
  }
</style>
