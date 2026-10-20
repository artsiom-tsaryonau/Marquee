<script lang="ts">
  // Mock of the "Add a Windows app" flow from the roadmap: pick a recipe,
  // Marquee prepares the Proton prefix, you click through the installer once.
  import { fade, fly } from 'svelte/transition';
  import { app } from '../state.svelte';
  import Icon from './Icon.svelte';
  import Artwork from './Artwork.svelte';
  import type { Art } from '../types';

  let { onclose }: { onclose: () => void } = $props();

  const recipes: { id: string; name: string; note: string; art: Art }[] = [
    { id: 'purple', name: 'NCSOFT Purple', note: 'corefonts · vcrun2022', art: { hue: 270, hue2: 300, motif: 'shards', logo: 'heavy' } },
    { id: 'ea', name: 'EA App', note: 'Lutris script available', art: { hue: 10, hue2: 350, motif: 'rings', logo: 'wide' } },
    { id: 'bnet', name: 'Battle.net', note: 'vcrun2022 · dxvk', art: { hue: 205, hue2: 190, motif: 'orbit', logo: 'wide' } },
    { id: 'ubi', name: 'Ubisoft Connect', note: 'Lutris script available', art: { hue: 220, hue2: 250, motif: 'waves', logo: 'heavy' } },
    { id: 'rockstar', name: 'Rockstar Launcher', note: 'vcrun2019', art: { hue: 40, hue2: 25, motif: 'sun', logo: 'heavy' } },
    { id: 'custom', name: 'Any setup.exe', note: 'Choose a file', art: { hue: 160, hue2: 140, motif: 'grid', logo: 'mono' } },
  ];

  const steps = [
    'Creating prefix in ~/Games/<id>/',
    'Selecting GE-Proton 10-20 (umu-launcher)',
    'Installing winetricks packages',
    'Running installer — click through its window',
    'Reading Start Menu shortcuts',
    'Fetching artwork and icon',
  ];

  let picked = $state<(typeof recipes)[number] | null>(null);
  let step = $state(-1);

  function start(r: (typeof recipes)[number]) {
    picked = r;
    step = 0;
    const tick = () => {
      if (step >= steps.length) return;
      step++;
      if (step < steps.length) setTimeout(tick, step === 3 ? 1600 : 700);
    };
    setTimeout(tick, 700);
  }

  function finish() {
    if (!picked) return;
    const p = picked;
    const id = `lutris:${p.id}`;
    if (!app.games.some((g) => g.id === id)) {
      app.games.unshift({
        id,
        title: p.name,
        tagline: 'Installed by Marquee. No Wine settings touched.',
        platform: 'pc-windows',
        source: 'lutris',
        installed: true,
        genre: 'Launcher',
        year: 2026,
        playtimeMin: 0,
        lastPlayed: null,
        addedAt: new Date().toISOString(),
        favorite: false,
        ratings: { critic: null, criticCount: 0, user: null, steamReviews: null },
        timeToBeat: null,
        achievements: null,
        art: p.art,
      });
    }
    app.notify(`${p.name} added to your library`);
    onclose();
    const g = app.games.find((x) => x.id === id);
    if (g) app.open(g);
  }

  const dur = $derived(app.reduceMotion ? 0 : 240);
</script>

<div class="scrim" transition:fade={{ duration: dur }} onclick={onclose} role="presentation"></div>
<div class="dialog glass-panel" role="dialog" aria-modal="true" aria-label="Add a Windows app" transition:fly={{ y: 24, duration: dur }}>
  <header>
    <div>
      <h2>Add a Windows app</h2>
      <p>Pick a launcher. Marquee sets up Proton, the prefix and the fixes. You only click through the installer.</p>
    </div>
    <button class="close" data-nav onclick={onclose} aria-label="Close"><Icon name="x" /></button>
  </header>

  {#if !picked}
    <div class="recipes">
      {#each recipes as r (r.id)}
        <button class="recipe" data-nav style="--accent-h:{r.art.hue2}" onclick={() => start(r)}>
          <div class="art"><Artwork art={r.art} grain={false} /></div>
          <strong>{r.name}</strong>
          <span>{r.note}</span>
        </button>
      {/each}
    </div>
  {:else}
    <ol class="steps">
      {#each steps as s, i (s)}
        <li class:done={i < step} class:now={i === step}>
          <span class="mark">
            {#if i < step}<Icon name="check" size={16} />{:else if i === step}<span class="spin"></span>{:else}{i + 1}{/if}
          </span>
          {s.replace('<id>', picked.id)}
        </li>
      {/each}
    </ol>
    <footer>
      {#if step >= steps.length}
        <p class="ok"><Icon name="sparkle" /> {picked.name} is ready. Found shortcut “{picked.name}”.</p>
        <button class="btn primary" data-nav onclick={finish}><Icon name="plus" /> Add to library</button>
      {:else}
        <p>Working… nothing to configure.</p>
      {/if}
    </footer>
  {/if}
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 40;
    background: rgb(0 0 0 / 0.55);
    backdrop-filter: blur(4px);
  }
  .dialog {
    position: fixed;
    z-index: 41;
    left: 50%;
    top: 50%;
    translate: -50% -50%;
    width: min(760px, calc(100vw - 32px));
    max-height: calc(100vh - 32px);
    overflow: auto;
    border-radius: 24px;
    padding: 24px;
    background: var(--surface-strong);
  }
  header {
    display: flex;
    justify-content: space-between;
    gap: 16px;
    margin-bottom: 18px;
  }
  h2 {
    margin: 0 0 6px;
    font-size: 22px;
  }
  header p {
    margin: 0;
    color: var(--text-dim);
    font-size: 14px;
  }
  .close {
    flex: none;
    width: 36px;
    height: 36px;
    display: grid;
    place-items: center;
    border-radius: 50%;
    border: 1px solid var(--line);
    background: var(--field);
    color: var(--text);
    cursor: pointer;
  }
  .recipes {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
    gap: 12px;
  }
  .recipe {
    text-align: left;
    display: grid;
    gap: 4px;
    padding: 10px 10px 14px;
    border-radius: 16px;
    border: 1px solid var(--line);
    background: var(--field);
    color: var(--text);
    font: inherit;
    cursor: pointer;
    transition: transform 180ms var(--ease), box-shadow 180ms;
    outline: none;
  }
  .recipe:hover,
  .recipe:focus-visible {
    transform: translateY(-3px);
    box-shadow: 0 0 0 2px hsl(var(--accent-h) 90% 70%), 0 14px 30px -12px hsl(var(--accent-h) 90% 50% / 0.6);
  }
  .recipe .art {
    height: 84px;
    border-radius: 10px;
    overflow: hidden;
    margin-bottom: 6px;
  }
  .recipe span {
    font-size: 12px;
    color: var(--text-dim);
  }
  .steps {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 8px;
  }
  .steps li {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 14px;
    border-radius: 12px;
    color: var(--text-dim);
    background: var(--field);
    transition: color 200ms, background 200ms;
  }
  .steps li.now {
    color: var(--text);
    background: color-mix(in oklab, var(--accent) 18%, var(--field));
  }
  .steps li.done {
    color: var(--text-soft);
  }
  .mark {
    flex: none;
    width: 26px;
    height: 26px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    font-size: 12px;
    font-weight: 700;
    background: var(--field-strong);
  }
  .done .mark {
    background: var(--ok);
    color: #04261a;
  }
  .spin {
    width: 14px;
    height: 14px;
    border-radius: 50%;
    border: 2px solid var(--accent);
    border-right-color: transparent;
    animation: spin 0.7s linear infinite;
  }
  @keyframes spin {
    to {
      rotate: 360deg;
    }
  }
  footer {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 12px;
    margin-top: 18px;
    min-height: 44px;
  }
  footer p {
    margin: 0;
    color: var(--text-dim);
    display: flex;
    align-items: center;
    gap: 8px;
  }
  footer .ok {
    color: var(--ok);
  }
</style>
