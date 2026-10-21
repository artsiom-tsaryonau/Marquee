<script lang="ts">
  import type { Snippet } from 'svelte';
  import { artFromSeed } from '../art';
  import Artwork from './Artwork.svelte';
  import Icon, { type IconName } from './Icon.svelte';

  let {
    seed,
    kind,
    title,
    subtitle = '',
    detail = '',
    mono = '',
    badge,
    icon = undefined,
    glyph = 'controller',
    ready = true,
    busy = '',
    onopen = undefined,
    actions,
  }: {
    seed: string;
    kind: string;
    title: string;
    subtitle?: string;
    detail?: string;
    mono?: string;
    badge: { text: string; tone?: 'ok' | 'warn' | '' };
    icon?: string | undefined;
    glyph?: IconName;
    ready?: boolean;
    busy?: string;
    onopen?: (() => void) | undefined;
    actions: Snippet;
  } = $props();
</script>

<article class="card glass-panel" class:dim={!ready}>
  {#if onopen}
    <button class="tile" data-nav onclick={onopen} aria-label={`${title}: details`}>
    <Artwork art={artFromSeed(seed)} />
    <span class="shade"></span>
    {#if icon}<img class="pic" src={icon} alt="" loading="lazy" />{:else}<span class="pic fallback"><Icon name={glyph} size={30} /></span>{/if}
    <span class="kind">{kind}</span>
    <span class="badge" class:ok={badge.tone === 'ok'} class:warn={badge.tone === 'warn'}>{badge.text}</span>
    </button>
  {:else}
    <div class="tile">
    <Artwork art={artFromSeed(seed)} />
    <span class="shade"></span>
    {#if icon}<img class="pic" src={icon} alt="" loading="lazy" />{:else}<span class="pic fallback"><Icon name={glyph} size={30} /></span>{/if}
    <span class="kind">{kind}</span>
    <span class="badge" class:ok={badge.tone === 'ok'} class:warn={badge.tone === 'warn'}>{badge.text}</span>
    </div>
  {/if}
  <div class="info">
    <h3>{title}</h3>
    {#if subtitle}<p class="sub">{subtitle}</p>{/if}
    {#if detail}<p class="detail">{detail}</p>{/if}
    {#if mono}<p class="mono" title={mono}>{mono}</p>{/if}
    {#if busy}<p class="busy" role="status">{busy}</p>{/if}
    <div class="actions">{@render actions()}</div>
  </div>
</article>

<style>
  .card { padding: 0; border-radius: 18px; display: flex; flex-direction: column; min-width: 0; overflow: hidden; transition: border-color .2s; }
  .card:hover { border-color: rgb(139 92 246 / .45); }
  :global(:root[data-effects='full']) .card.dim .tile { filter: saturate(.55); }
  .tile { position: relative; display: block; width: 100%; height: 140px; padding: 0; border: 0; overflow: hidden; background: none; color: inherit; font: inherit; }
  button.tile { cursor: pointer; }
  .tile :global(svg) { position: absolute; inset: 0; width: 100%; height: 100%; }
  .shade { position: absolute; inset: 0; background: linear-gradient(to top, rgb(8 8 14 / .7), transparent 65%); }
  .pic { position: absolute; left: 50%; top: 50%; width: 72px; height: 72px; transform: translate(-50%, -52%); object-fit: contain; transition: transform .25s; }
  :global(:root[data-effects='full']) .pic { filter: drop-shadow(0 8px 16px rgb(0 0 0 / .5)); }
  button.tile:hover .pic { transform: translate(-50%, -52%) scale(1.08); }
  .pic.fallback { display: grid; place-items: center; border-radius: 20px; background: rgb(0 0 0 / .3); color: #fff; }
  .kind, .badge { position: absolute; bottom: 10px; padding: 4px 8px; border-radius: 6px; font-size: 11px; color: var(--text-soft); background: rgb(0 0 0 / .45); backdrop-filter: var(--overlay-filter); }
  .kind { left: 12px; top: 10px; bottom: auto; letter-spacing: .4px; text-transform: uppercase; font-size: 10px; }
  .badge { left: 12px; }
  .badge.ok { color: var(--ok); }
  .badge.warn { color: var(--warn); }
  .info { padding: 16px 18px 18px; display: flex; flex-direction: column; flex: 1; min-width: 0; }
  h3 { margin: 0 0 4px; font-size: 17px; }
  p { margin: 4px 0; font-size: 13px; line-height: 1.5; color: var(--text-soft); }
  .detail { font-size: 12px; color: var(--text-dim); }
  .mono { font: 11px ui-monospace, monospace; color: var(--text-dim); overflow-wrap: anywhere; }
  .busy { font-size: 12px; color: var(--accent); }
  .actions { display: flex; flex-wrap: wrap; gap: 8px; align-items: center; margin-top: auto; padding-top: 14px; }
  .actions :global(.text-button) { border: 0; background: none; color: var(--text-dim); padding: 8px; cursor: pointer; font: inherit; font-size: 12px; }
  .actions :global(.pin) { margin-left: auto; border: 0; background: none; color: var(--text-dim); padding: 6px; cursor: pointer; }
  .actions :global(.pin.pinned) { color: var(--warn); }
</style>
