<script lang="ts">
  import type { GameEntry } from '../types';
  import { sourceLabel } from '../labels';
  import { app } from '../state.svelte';
  import Artwork from './Artwork.svelte';
  import Logo from './Logo.svelte';
  import Icon from './Icon.svelte';

  let {
    game,
    shape = 'cover',
    showTitle = false,
  }: { game: GameEntry; shape?: 'cover' | 'wide'; showTitle?: boolean } = $props();

  let el: HTMLButtonElement;

  // Tilt toward the pointer with a moving glare; skipped when motion is reduced.
  function tilt(e: PointerEvent) {
    if (app.reduceMotion) return;
    const r = el.getBoundingClientRect();
    const x = (e.clientX - r.left) / r.width;
    const y = (e.clientY - r.top) / r.height;
    el.style.setProperty('--rx', `${(0.5 - y) * 10}deg`);
    el.style.setProperty('--ry', `${(x - 0.5) * 12}deg`);
    el.style.setProperty('--gx', `${x * 100}%`);
    el.style.setProperty('--gy', `${y * 100}%`);
  }
  function untilt() {
    el.style.setProperty('--rx', '0deg');
    el.style.setProperty('--ry', '0deg');
  }

  const status = $derived(
    game.portStatus === 'update'
      ? { label: 'Update', tone: 'warn' }
      : game.source === 'port'
        ? { label: 'Ready', tone: 'ok' }
        : game.rootOffline
          ? { label: 'Drive offline', tone: 'dim' }
          : game.mediaMissing
            ? { label: 'Missing', tone: 'warn' }
            : game.setupState === 'runner_missing'
              ? { label: 'Needs PCSX2', tone: 'warn' }
              : game.setupState === 'missing'
                ? { label: 'Needs BIOS', tone: 'warn' }
                : !game.installed
                  ? { label: 'Not installed', tone: 'dim' }
                  : null,
  );
</script>

<div class="wrap {shape}">
  <button
    bind:this={el}
    class="card"
    data-nav
    style="--accent-h:{game.art.hue2}"
    onfocus={() => app.preview(game.id)}
    onpointerenter={() => app.preview(game.id)}
    onpointermove={tilt}
    onpointerleave={untilt}
    onclick={() => app.open(game)}
    aria-label={game.title}
  >
    <div class="art"><Artwork art={game.art} grain={shape === 'wide'} /></div>
    <div class="shade"></div>
    <div class="logo"><Logo {game} size={shape === 'wide' ? 'md' : 'sm'} /></div>
    <span class="source">{sourceLabel[game.source]}</span>
    {#if game.favorite}<span class="fav"><Icon name="star" size={14} filled /></span>{/if}
    {#if status?.tone === 'dim'}
      <span class="status dim icon" title="Not installed"><Icon name="download" size={13} /></span>
    {:else if status}
      <span class="status {status.tone}">{status.label}</span>
    {/if}
    <span class="glare"></span>
  </button>
  {#if showTitle}
    <div class="caption">
      <span class="t">{game.title}</span>
      <span class="m">{game.playtimeMin ? `${Math.round(game.playtimeMin / 60)} h played` : game.genre}</span>
    </div>
  {/if}
</div>

<style>
  .wrap {
    flex: none;
    width: var(--cover-w);
  }
  .wrap.wide {
    width: calc(var(--cover-w) * 2.1);
  }
  .card {
    --rx: 0deg;
    --ry: 0deg;
    --gx: 50%;
    --gy: 0%;
    position: relative;
    display: block;
    width: 100%;
    aspect-ratio: 2 / 3;
    border: 0;
    padding: 0;
    border-radius: var(--radius);
    overflow: hidden;
    cursor: pointer;
    background: hsl(var(--accent-h) 30% 15%);
    box-shadow: 0 10px 30px -12px rgb(0 0 0 / 0.7);
    transform: perspective(900px) rotateX(var(--rx)) rotateY(var(--ry)) scale(var(--s, 1));
    transition:
      transform 220ms var(--ease),
      box-shadow 220ms var(--ease);
    outline: none;
  }
  .wide .card {
    aspect-ratio: 16 / 9.4;
  }
  .card:hover,
  .card:focus-visible {
    --s: 1.06;
    z-index: 2;
    box-shadow:
      0 0 0 2px hsl(var(--accent-h) 95% 72%),
      0 0 32px 4px hsl(var(--accent-h) 95% 60% / 0.55),
      0 24px 50px -16px rgb(0 0 0 / 0.9);
  }
  .card:focus-visible {
    animation: pulse 2.4s ease-in-out infinite;
  }
  @keyframes pulse {
    50% {
      box-shadow:
        0 0 0 2px hsl(var(--accent-h) 95% 80%),
        0 0 44px 8px hsl(var(--accent-h) 95% 60% / 0.7),
        0 24px 50px -16px rgb(0 0 0 / 0.9);
    }
  }
  .art {
    position: absolute;
    inset: 0;
    transition: transform 400ms var(--ease);
  }
  .card:hover .art,
  .card:focus-visible .art {
    transform: scale(1.05);
  }
  .shade {
    position: absolute;
    inset: 0;
    background: linear-gradient(180deg, transparent 45%, rgb(0 0 0 / 0.65));
  }
  .logo {
    position: absolute;
    left: 12px;
    right: 12px;
    bottom: 12px;
    text-align: left;
  }
  .source,
  .status {
    position: absolute;
    top: 10px;
    font-size: 10.5px;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    padding: 4px 8px;
    border-radius: 999px;
    backdrop-filter: blur(10px);
  }
  .source {
    left: 10px;
    color: #fff;
    background: rgb(0 0 0 / 0.35);
    border: 1px solid rgb(255 255 255 / 0.18);
  }
  .status {
    right: 10px;
  }
  .status.ok {
    background: rgb(52 211 153 / 0.85);
    color: #04261a;
  }
  .status.warn {
    background: rgb(251 191 36 / 0.92);
    color: #2b1b00;
  }
  .status.icon {
    padding: 4px;
    display: grid;
  }
  .status.dim {
    background: rgb(0 0 0 / 0.45);
    color: rgb(255 255 255 / 0.75);
  }
  .fav {
    position: absolute;
    right: 10px;
    bottom: 10px;
    color: #fde68a;
    filter: drop-shadow(0 0 6px rgb(253 230 138 / 0.8));
  }
  .glare {
    position: absolute;
    inset: 0;
    pointer-events: none;
    opacity: 0;
    background: radial-gradient(circle at var(--gx) var(--gy), rgb(255 255 255 / 0.28), transparent 45%);
    transition: opacity 200ms;
    mix-blend-mode: overlay;
  }
  .card:hover .glare {
    opacity: 1;
  }
  .caption {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 10px 2px 0;
  }
  .caption .t {
    font-weight: 650;
    font-size: 14px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .caption .m {
    color: var(--text-dim);
    font-size: 12px;
  }
</style>
