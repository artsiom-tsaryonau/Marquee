<script lang="ts" module>
  import type { IconName } from './Icon.svelte';

  const consoleDigit: Record<string, string> = { ps1: '1', ps2: '2', ps3: '3' };
  const brandFor: Record<string, string> = {
    steam: 'steam', gog: 'gog', epic: 'epic', lutris: 'lutris',
    ps1: 'playstation', ps2: 'playstation', ps3: 'playstation', psp: 'psp', dreamcast: 'sega',
    'pc-linux': 'linux', pc: 'linux',
  };
  const glyphFor: Record<string, IconName> = { rom: 'controller', port: 'puzzle', 'pc-windows': 'monitor' };

  /** Whether a store/platform id has a recognisable mark; callers fall back to text when not. */
  export const hasMark = (id: string): boolean => id in brandFor || id in glyphFor;
</script>

<script lang="ts">
  import { brands } from '../brands';
  import Icon from './Icon.svelte';

  let { id, size = 20 }: { id: string; size?: number } = $props();
  const brand = $derived(brandFor[id]);
  const digit = $derived(consoleDigit[id]);
  // The PSP mark is a wide wordmark; everything else sits on a square 24px grid.
  const box = $derived(digit ? '0 0 35 24' : brand === 'psp' ? '0 7 24 10' : '0 0 24 24');
</script>

{#if brand}
  <svg class="mark" height={size} width={digit ? size * 1.45 : brand === 'psp' ? size * 2.2 : size} viewBox={box} fill="currentColor" aria-hidden="true">
    <path d={brands[brand]} />
    {#if digit}<text x="24.5" y="23" font-size="18" font-weight="800" font-family="system-ui, sans-serif">{digit}</text>{/if}
  </svg>
{:else if glyphFor[id]}
  <Icon name={glyphFor[id]} size={size} />
{/if}

<style>
  .mark { display: block; flex-shrink: 0; }
  :global(.sr) { position: absolute; width: 1px; height: 1px; overflow: hidden; clip-path: inset(50%); white-space: nowrap; }
</style>
