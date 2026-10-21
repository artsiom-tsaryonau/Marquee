<script lang="ts">
  // The focused game's art, blurred behind everything; cross-fades on change.
  import { fade } from 'svelte/transition';
  import { app } from '../state.svelte';
  import { homeAtmosphere } from '../art';
  import Artwork from './Artwork.svelte';

  let { blur = true }: { blur?: boolean } = $props();
  const dur = $derived(app.motionReduced ? 0 : 450);
  const art = $derived(app.hero?.art ?? homeAtmosphere);
</script>

<div class="backdrop" class:sharp={!blur}>
  {#if app.visualEffects === 'balanced'}
    <div class="atmosphere" style="--h:{art.hue};--h2:{art.hue2}"></div>
  {:else}
  {#key app.hero?.id ?? 'atmosphere'}
    <div class="layer" transition:fade={{ duration: dur }}>
      <Artwork {art} grain={false} />
    </div>
  {/key}
  {/if}
  <div class="tint" style="--h:{art.hue}"></div>
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
  .atmosphere {
    position: absolute;
    inset: 0;
    opacity: var(--backdrop-opacity);
    background: radial-gradient(ellipse at 70% 15%, hsl(var(--h2) 50% 22%), transparent 65%),
      linear-gradient(145deg, hsl(var(--h) 40% 12%), var(--bg) 85%);
  }
  .sharp .layer {
    filter: saturate(1.1);
  }
  .tint {
    position: absolute;
    inset: 0;
    background:
      radial-gradient(120% 80% at 70% 0%, transparent, var(--bg) 85%),
      linear-gradient(180deg, color-mix(in srgb, var(--bg) 20%, transparent), var(--bg) 92%);
  }
</style>
