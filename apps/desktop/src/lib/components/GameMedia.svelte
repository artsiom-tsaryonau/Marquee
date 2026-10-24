<script lang="ts">
  import type { GameEntry } from '../types';
  import { gameMedia } from '../gameMedia';
  import Artwork from './Artwork.svelte';
  import Icon from './Icon.svelte';
  let { game }: { game: GameEntry } = $props();
  const items = $derived(gameMedia(game));
  let index = $state(0);
  let failed = $state<string[]>([]);
  const selected = $derived(items[index] ?? items[0]);
</script>

<section class="gallery" aria-label="Game media">
  <div class="stage" class:box-cover={selected?.kind === 'cover'}>
    {#if selected && !failed.includes(selected.url)}<a href={selected.url} target="_blank" rel="noreferrer" data-nav aria-label={`Open ${selected.caption}`}><img src={selected.url} alt={selected.caption} onerror={() => failed = [...failed, selected.url]} /></a>
    {:else}<Artwork art={game.art} grain={false} /><p>{selected ? 'Image unavailable' : 'No images added yet'}</p>{/if}
  </div>
  {#if items.length > 1}<div class="thumbnails" aria-label="Choose game image" data-nav-row>
    {#each items as item, i}<button data-nav class:active={selected?.url === item.url} aria-label={`Show ${item.caption}`} aria-pressed={selected?.url === item.url} disabled={failed.includes(item.url)} onclick={() => index = i}><img src={item.url} alt="" loading="lazy" /><span>{item.kind === 'cover' ? 'Cover' : item.kind === 'hero' ? 'Artwork' : `Image ${i + 1}`}</span></button>{/each}
  </div>{/if}
  <p class="caption">{selected?.kind === 'cover' ? 'Box cover' : selected?.caption || 'Artwork will appear here when available.'}{#if selected?.kind === 'screenshot' && game.mediaCredit}<span>{game.mediaCredit}</span>{/if}</p>
</section>

<style>
  .gallery { min-width: 0; }
  .stage { aspect-ratio: 16/9; position: relative; overflow: hidden; background: color-mix(in srgb,var(--bg) 85%,black); border: 1px solid var(--line); border-radius: 8px; }
  .stage a { display: block; width: 100%; height: 100%; } .stage img { width: 100%; height: 100%; object-fit: contain; display: block; }
  .stage.box-cover img { padding: 16px; box-sizing: border-box; }
  .stage > :global(svg) { width: 100%; height: 100%; opacity: .35; } .stage > p { position: absolute; bottom: 16px; left: 20px; color: var(--text-soft); font-size: 13px; }
  .thumbnails { display: flex; gap: 8px; overflow-x: auto; padding: 10px 2px 4px; }
  .thumbnails button { flex: 0 0 100px; height: 64px; position: relative; border: 2px solid transparent; padding: 0; background: var(--field); border-radius: 4px; overflow: hidden; cursor: pointer; }
  .thumbnails button.active { border-color: var(--text-soft); } .thumbnails img { width: 100%; height: 100%; object-fit: contain; } .thumbnails button:disabled { opacity: .3; }
  .thumbnails span { position: absolute; bottom: 0; left: 0; right: 0; padding: 2px; background: #000b; color: white; font-size: 9px; }
  .caption { font-size: 11px; color: var(--text-dim); margin: 10px 0 0; line-height: 1.5; } .caption span { display: block; }
</style>
