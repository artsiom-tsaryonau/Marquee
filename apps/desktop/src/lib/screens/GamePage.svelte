<script lang="ts">
  import { fly } from 'svelte/transition';
  import { app } from '../state.svelte';
  import { platformLabel, sourceLabel } from '../labels';
  import { ago, hours } from '../format';
  import Artwork from '../components/Artwork.svelte';
  import Logo from '../components/Logo.svelte';
  import Icon from '../components/Icon.svelte';
  import Row from '../components/Row.svelte';
  import ScoreStrip from '../components/ScoreStrip.svelte';

  const g = $derived(app.current ?? app.hero);
  const port = $derived(g?.source === 'port' ? app.ports.find((p) => `port:${p.id}` === g.id) : undefined);
  const shots = $derived(g ? [0, 1, 2, 3].map((i) => ({ ...g.art, hue: (g.art.hue + i * 24) % 360, motif: (['peaks', 'waves', 'grid', 'orbit'] as const)[(i + g.art.hue) % 4] })) : []);
  const similar = $derived(g ? app.games.filter((x) => x.id !== g.id && (x.genre === g.genre || x.source === g.source)).slice(0, 10) : []);
  const dur = $derived(app.reduceMotion ? 0 : 360);
  const offline = $derived(!!g && !app.demo && g.platform === 'ps2');
  let editing = $state(false);
  let titleDraft = $state('');

  const launchVia = $derived(
    g
      ? {
          steam: 'Steam · steam://rungameid',
          gog: 'Lutris · GOG',
          epic: 'Legendary',
          lutris: 'Lutris · umu-launcher + GE-Proton',
          port: 'Native port binary',
          rom: offline ? (app.selectedRunner?.display_name ?? 'PCSX2') : 'Emulator (Flatpak)',
        }[g.source]
      : '',
  );

  const playLabel = $derived(
    !g ? 'Play' : g.setupState === 'missing' ? 'Open PCSX2 setup' : g.mediaMissing ? 'Disc missing' : g.installed ? 'Play' : app.demo ? 'Install' : 'Reconnect',
  );
</script>

{#if !g}
  <article class="page">
    <div class="head">
      <button class="back" data-nav onclick={() => app.back()}><Icon name="back" size={18} /> Back</button>
      <p class="tagline">Select a game from the library.</p>
    </div>
  </article>
{:else}
<article class="page" style="--accent-h:{g.art.hue2}">
  <div class="banner">
    <Artwork art={g.art} />
    <div class="fade"></div>
  </div>

  <div class="head" in:fly={{ y: 20, duration: dur }}>
    <button class="back" data-nav onclick={() => app.back()}><Icon name="back" size={18} /> Back</button>
    {#if editing && offline}
      <input class="rename" data-nav bind:value={titleDraft} onkeydown={(e) => { if (e.key === 'Enter') { void app.rename(g, titleDraft); editing = false; } if (e.key === 'Escape') editing = false; }} />
    {:else}
      <Logo game={g} size="lg" />
    {/if}
    <p class="tagline">{g.tagline}</p>
    <div class="actions">
      <button class="btn primary big" data-nav onclick={() => {
        if (offline && g.setupState === 'missing') app.openSetup();
        else if (g.installed) app.play(g);
        else if (app.demo) app.install(g.title, { kind: 'game', id: g.id });
      }}>
        <Icon name={g.installed && g.setupState !== 'missing' ? 'play' : 'download'} />
        {playLabel}
      </button>
      {#if offline && (g.setupState === 'unverified' || g.setupState === 'runner_missing' || g.setupState === 'missing')}
        <button class="btn glass" data-nav onclick={() => app.openSetup()}><Icon name="settings" /> Open PCSX2 setup</button>
      {/if}
      {#if g.portStatus === 'update'}
        <button class="btn glass" data-nav onclick={() => port && app.install(`${g.title} update`, { kind: 'port', id: port.id })}><Icon name="refresh" /> Update</button>
      {/if}
      <button class="btn glass icon" data-nav aria-label="Favourite" onclick={() => app.toggleFavorite(g)}>
        <Icon name="star" filled={g.favorite} />
      </button>
      {#if offline}
        <button class="btn glass" data-nav onclick={() => { titleDraft = g.title; editing = true; }}><Icon name="image" /> Rename</button>
      {:else}
        <button class="btn glass" data-nav onclick={() => app.notify('Opened install folder')}><Icon name="folder" /> Folder</button>
        <button class="btn glass" data-nav onclick={() => app.notify('Drop an image to replace the artwork')}><Icon name="image" /> Artwork</button>
        <button class="btn glass" data-nav onclick={() => app.notify(`${g.title} added to Steam (restart Steam)`)}><Icon name="plus" /> Add to Steam</button>
      {/if}
    </div>
  </div>

  <div class="body">
    <div class="main">
      {#if !offline}
      <h3>Ratings and facts</h3>
      <div class="scores"><ScoreStrip game={g} /></div>
      <h3>Screenshots</h3>
      <div class="shots" data-nav-row>
        {#each shots as art, i (i)}
          <button class="shot" data-nav aria-label="Screenshot {i + 1}"><Artwork {art} /></button>
        {/each}
      </div>

      {/if}
      {#if port}
        <h3>Requirements</h3>
        <ul class="reqs glass-panel">
          <li class="ok"><Icon name="check" size={18} /> {port.name} {port.version} installed from {port.upstream}</li>
          <li class="ok"><Icon name="check" size={18} /> {port.requirement}</li>
          <li class="ok"><Icon name="check" size={18} /> Controller profile applied</li>
        </ul>
      {/if}
    </div>

    <aside class="facts glass-panel">
      <dl>
        <dt>Source</dt>
        <dd><span class="pill">{sourceLabel[g.source]}</span></dd>
        <dt>Platform</dt>
        <dd>{platformLabel[g.platform]}</dd>
        <dt>Launches with</dt>
        <dd>{launchVia}</dd>
        <dt>Playtime</dt>
        <dd>{g.playtimeMin ? hours(g.playtimeMin) : 'Not played yet'}</dd>
        <dt>Last played</dt>
        <dd>{g.lastPlayed ? ago(g.lastPlayed) : '—'}</dd>
        <dt>Genre</dt>
        <dd>{g.genre}{g.year ? ` · ${g.year}` : ''}</dd>
        {#if g.serial}
          <dt>Serial</dt>
          <dd>{g.serial}{g.region ? ` · ${g.region}` : ''}</dd>
        {/if}
        {#if g.setupState}
          <dt>PCSX2</dt>
          <dd>{g.setupState.replace('_', ' ')}</dd>
        {/if}
        {#if g.steam && !offline}
          <dt>Players now</dt>
          <dd><span class="live-dot"></span>{g.steam.playersNow.toLocaleString()}</dd>
          <dt>Last update</dt>
          <dd>{g.steam.updatedDaysAgo === 0 ? 'Today' : `${g.steam.updatedDaysAgo} d ago`} · <span class="patch">{g.steam.patchTitle}</span></dd>
          <dt>Steam Deck</dt>
          <dd><span class="badge deck-{g.steam.deck}">{g.steam.deck[0].toUpperCase() + g.steam.deck.slice(1)}</span></dd>
        {/if}
        {#if !offline && g.platform === 'pc-windows' && g.source !== 'steam'}
          <dt>Compatibility</dt>
          <dd><span class="badge gold">ProtonDB Gold</span> <span class="badge ok">No anti-cheat</span></dd>
        {/if}
      </dl>
    </aside>
  </div>

  <Row title="More like this" games={similar} />
</article>
{/if}

<style>
  .page {
    position: relative;
    padding-bottom: 40px;
  }
  .banner {
    position: absolute;
    inset: 0 0 auto;
    height: min(62vh, 620px);
    z-index: -1;
    overflow: hidden;
  }
  .fade {
    position: absolute;
    inset: 0;
    background:
      linear-gradient(90deg, var(--bg) 0%, color-mix(in oklab, var(--bg) 40%, transparent) 45%, transparent 75%),
      linear-gradient(180deg, transparent 40%, var(--bg));
  }
  .head {
    min-height: min(62vh, 620px);
    padding: 24px var(--gutter) 28px;
    display: flex;
    flex-direction: column;
    justify-content: flex-end;
    gap: 14px;
    max-width: 900px;
  }
  .back {
    position: absolute;
    top: 24px;
    left: var(--gutter);
    display: inline-flex;
    align-items: center;
    gap: 6px;
    background: rgb(0 0 0 / 0.35);
    color: #fff;
    border: 1px solid rgb(255 255 255 / 0.15);
    border-radius: 999px;
    padding: 8px 14px 8px 10px;
    font: inherit;
    font-weight: 600;
    cursor: pointer;
    backdrop-filter: blur(12px);
  }
  .tagline {
    margin: 0;
    font-size: var(--tagline);
    color: var(--text-soft);
  }
  .rename {
    font: inherit;
    font-size: clamp(28px, 4vw, 48px);
    font-weight: 800;
    background: var(--field);
    color: var(--text);
    border: 1px solid var(--line);
    border-radius: 12px;
    padding: 8px 12px;
    max-width: 16ch;
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
    margin-top: 4px;
  }
  .body {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 320px;
    gap: 28px;
    padding: 8px var(--gutter) 28px;
  }
  @media (max-width: 900px) {
    .body {
      grid-template-columns: 1fr;
    }
  }
  h3 {
    margin: 0 0 12px;
    font-size: 17px;
  }
  .scores {
    margin-bottom: 28px;
  }
  .shots {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
    gap: 14px;
    margin-bottom: 28px;
  }
  .shot {
    aspect-ratio: 16 / 9;
    border: 0;
    padding: 0;
    border-radius: 12px;
    overflow: hidden;
    cursor: zoom-in;
    transition: transform 200ms var(--ease), box-shadow 200ms;
    outline: none;
  }
  .shot:hover,
  .shot:focus-visible {
    transform: scale(1.04);
    box-shadow: 0 0 0 2px hsl(var(--accent-h) 95% 72%), 0 0 28px hsl(var(--accent-h) 95% 60% / 0.5);
  }
  .reqs {
    list-style: none;
    margin: 0;
    padding: 14px 18px;
    border-radius: 16px;
    display: grid;
    gap: 10px;
  }
  .reqs li {
    display: flex;
    gap: 10px;
    align-items: center;
  }
  .reqs .ok :global(svg) {
    color: var(--ok);
  }
  .facts {
    border-radius: 18px;
    padding: 18px 20px;
    align-self: start;
  }
  dl {
    margin: 0;
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 12px 16px;
    font-size: 14px;
  }
  dt {
    color: var(--text-dim);
  }
  dd {
    margin: 0;
    font-weight: 600;
  }
  .badge {
    display: inline-block;
    font-size: 11px;
    font-weight: 700;
    padding: 3px 8px;
    border-radius: 999px;
    margin: 0 4px 4px 0;
  }
  .live-dot {
    display: inline-block;
    width: 8px;
    height: 8px;
    margin-right: 8px;
    border-radius: 50%;
    background: #22c55e;
    box-shadow: 0 0 8px #22c55e;
  }
  .patch {
    color: var(--text-dim);
    font-weight: 500;
  }
  .badge.deck-verified {
    background: rgb(52 211 153 / 0.18);
    color: var(--ok);
  }
  .badge.deck-playable {
    background: rgb(250 204 21 / 0.2);
    color: #fde047;
  }
  .badge.deck-unsupported,
  .badge.deck-unknown {
    background: var(--field);
    color: var(--text-dim);
  }
  .badge.gold {
    background: rgb(250 204 21 / 0.2);
    color: #fde047;
  }
  .badge.ok {
    background: rgb(52 211 153 / 0.18);
    color: var(--ok);
  }
</style>
