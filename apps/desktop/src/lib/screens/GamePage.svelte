<script lang="ts">
  import { fly } from 'svelte/transition';
  import { app } from '../state.svelte';
  import { platformLabel, sourceLabel } from '../labels';
  import { canonicalId, editionKey, editionLabel, launchMethod, methodLabel } from '../unified';
  import { ago, hours, transferLine } from '../format';
  import Artwork from '../components/Artwork.svelte';
  import Logo from '../components/Logo.svelte';
  import Icon from '../components/Icon.svelte';
  import Row from '../components/Row.svelte';
  import InstallMenu from '../components/InstallMenu.svelte';
  import CatalogueDetails from '../components/CatalogueDetails.svelte';

  const g = $derived(app.current ?? app.hero);
  const variants = $derived(g ? app.games.filter((x) => canonicalId(x) === canonicalId(g)) : []);
  const editions = $derived([...new Map(variants.map((x) => [editionKey(x), x])).values()]);
  const choices = $derived(g ? variants.filter((x) => editionKey(x) === editionKey(g)) : []);
  function choose(id: string) { app.chooseVariant(id); }
  const port = $derived(g?.source === 'port' ? app.ports.find((p) => `port:${p.id}` === g.id) : undefined);
  const similar = $derived(g ? app.games.filter((x) => canonicalId(x) !== canonicalId(g) && (x.genre === g.genre || x.source === g.source)).slice(0, 10) : []);
  const dur = $derived(app.motionReduced ? 0 : 360);
  const offline = $derived(!!g && g.source === 'rom' && g.platform !== 'unknown');
  const emulator = $derived(g?.runnerName?.replace(' (Marquee)', '') ?? (g?.platform === 'ps1' ? 'DuckStation' : g?.platform === 'ps2' ? 'PCSX2' : 'its emulator'));
  const share = $derived(g && offline ? app.transferFor(g.id) : undefined);
  $effect(() => { if (app.torrentsOn) return app.watchTransfers(); });
  let editing = $state(false);
  let titleDraft = $state('');

  const launchVia = $derived(
    g?.lutris ? `Lutris · ${g.lutris.runner ?? 'store installer'}` : g
      ? {
          steam: 'Steam · steam://rungameid',
          gog: 'Lutris · GOG',
          epic: 'Legendary',
          lutris: 'Lutris · umu-launcher + GE-Proton',
          port: 'Native port binary',
          rom: offline ? (g.runnerName ?? emulator) : 'Emulator',
          catalogue: 'Catalogue only · no local installation',
        }[g.source]
      : '',
  );

  const playLabel = $derived(
    !g
      ? 'Play'
      : g.lutris ? (g.mediaMissing ? 'Files unavailable' : g.installed ? 'Play via Lutris' : 'Install in Lutris')
      : g.setupState === 'missing'
        ? `Open ${emulator} setup`
        : g.mediaMissing
          ? 'Disc files unavailable'
          : g.installed
            ? 'Play'
            : 'Reconnect',
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
    {#if g.heroUrl || g.coverUrl}
      <img class="banner-cover" src={g.heroUrl ?? g.coverUrl} alt="" />
    {:else}
      <Artwork art={g.art} />
    {/if}
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
    {#if variants.length > 1 || g.editionLabel}
      <div class="variant-picker glass-panel">
        <label>Edition
          <select data-nav value={editionKey(g)} onchange={(e) => { const first = variants.find((x) => editionKey(x) === e.currentTarget.value); if (first) choose(first.id); }}>
            {#each editions as edition}<option value={editionKey(edition)}>{editionLabel(edition)}</option>{/each}
          </select>
        </label>
        <label>Run with
          <select data-nav value={g.id} onchange={(e) => choose(e.currentTarget.value)}>
            {#each choices as choice}<option value={choice.id}>{methodLabel[launchMethod(choice)]} · {choice.source === 'port' ? 'OpenGOAL' : platformLabel[choice.platform]}{choice.installed ? '' : ' · not installed'}</option>{/each}
          </select>
        </label>
        <span>Your choice is remembered. Playtime and saves belong to this installation.</span>
      </div>
    {/if}
    <div class="actions">
      {#if g.source === 'catalogue' && g.metadata}
        <button class="btn primary big" data-nav onclick={() => g.metadata && app.openCataloguePage(g.metadata.id)}>Open source page in browser</button>
      {:else if offline && g.setupState === 'runner_missing'}
        {#if g.platform === 'ps2'}<InstallMenu tone="primary" size="big" />
        {:else}<button class="btn primary big" data-nav onclick={() => app.go('tools')}><Icon name="settings" /> Set up {g.platform === 'ps1' ? 'DuckStation' : g.platform === 'genesis' ? 'BlastEm' : g.platform === 'ds' ? 'melonDS' : g.platform === 'neogeo' ? 'FinalBurn Neo' : 'emulator'}</button>{/if}
      {:else}
        <button class="btn primary big" class:play={g.installed && !g.mediaMissing && g.setupState !== 'missing'} data-nav disabled={!!g.mediaMissing} onclick={() => {
          if (g.lutris) void app.openLutrisGame(g);
          else if (offline && g.setupState === 'missing') app.explainBios();
          else if (g.installed) app.play(g);
        }}>
          <Icon name={g.installed && g.setupState !== 'missing' ? 'play' : 'download'} size={24} />
          {playLabel}
        </button>
      {/if}
      {#if offline && g.serial && !g.coverUrl}
        <button class="btn glass" data-nav onclick={() => app.refreshArtwork(g.id)} disabled={app.fetchingArt}>
          <Icon name="image" /> {app.fetchingArt ? 'Fetching cover…' : 'Fetch cover'}
        </button>
      {/if}
      {#if g.portStatus === 'update'}
        <button class="btn glass" data-nav onclick={() => port && app.installPort(port)}><Icon name="refresh" /> Update</button>
      {/if}
      <button class="btn glass icon" data-nav aria-label="Favourite" disabled={g.source === 'port' || g.source === 'catalogue' || !!g.lutris} onclick={() => app.toggleFavorite(g)}>
        <Icon name="star" filled={g.favorite} />
      </button>
      {#if offline}
        <button class="btn glass" data-nav onclick={() => { titleDraft = g.title; editing = true; }}><Icon name="image" /> Rename</button>
        {#if app.torrentsOn && app.torrentSettings.seeding}
          {#if !share}
            <button class="btn glass" data-nav onclick={() => app.shareGame(g)}><Icon name="share" /> Share</button>
          {:else}
            {#if share.link}<button class="btn glass" data-nav onclick={() => app.copyTransferLink(share)}><Icon name="share" /> Copy link</button>{/if}
            {#if share.info_hash}<button class="btn glass" data-nav onclick={() => app.stopTransfer(share)}><Icon name="x" /> Stop sharing</button>{/if}
          {/if}
        {/if}
      {:else if g.lutris}
        <button class="btn glass" data-nav onclick={() => app.go('tools')}><Icon name="settings" /> Lutris connection</button>
      {:else if port}
        <button class="btn glass" data-nav onclick={() => app.openPortFolder(port)}><Icon name="folder" /> Folder</button>
      {/if}
    </div>
    {#if offline && share}<p class="tagline" role="status">{transferLine(share)}</p>{/if}
    {#if offline && (g.mediaIssue || g.setupState === 'runner_missing')}<p class="tagline" role="status">{g.mediaIssue || `${emulator} is not installed or available for this platform. Open Tools to set it up.`}</p>{/if}
  </div>

  <div class="body">
    <div class="main">
      {#if g.description}
        <h3>About</h3>
        <p class="about">{g.description}</p>
        <p class="media-credit">Description from TheGamesDB.</p>
      {/if}
      <CatalogueDetails game={g} />
      {#if g.screenshots?.length}
      <h3>Screenshots · OpenGOAL</h3>
      <div class="shots" data-nav-row>
        {#each g.screenshots as shot}<a class="shot" href={shot.url} target="_blank" rel="noreferrer" data-nav aria-label={shot.caption}><img src={shot.url} alt={shot.caption} loading="lazy" /></a>{/each}
      </div>
      <p class="media-credit">{g.mediaCredit} <a href={g.mediaSource} target="_blank" rel="noreferrer" data-nav>Source gallery</a></p>
      {/if}
      {#if g.lutris}
        <h3>Managed by Lutris</h3>
        <p>Lutris owns installation, compatibility settings, and playtime. Store listings come from its last local sync. Refresh the connection in Tools after an installation or play session.</p>
      {/if}
      {#if port}
        <h3>Requirements</h3>
        <ul class="reqs glass-panel">
          <li>{port.name}{port.version ? ` · ${port.version}` : ''} · {port.status === 'not-installed' ? 'Not installed' : 'Installed'} · {port.upstream}</li>
          <li>{port.requirement}</li>
        </ul>
      {/if}
    </div>

    <aside class="facts glass-panel">
      <dl>
        <dt>Source</dt>
        <dd><span class="pill">{sourceLabel[g.source]}</span></dd>
        <dt>Platform</dt>
        <dd>{platformLabel[g.platform]}{g.platform === 'unknown' && g.metadata ? ` · ${g.metadata.system}` : ''}</dd>
        {#if g.region}<dt>Region</dt><dd>{g.region}</dd>{/if}
        {#if g.languages?.length}<dt>Languages</dt><dd>{g.languages.join(', ')}</dd>{/if}
        {#if g.revision}<dt>ROM revision</dt><dd>{g.revision}</dd>{/if}
        {#if g.players}<dt>Players</dt><dd>{g.players}</dd>{/if}
        <dt>Launches with</dt>
        <dd>{launchVia}</dd>
        <dt>Playtime</dt>
        <dd>{g.source === 'catalogue' ? 'No local installation' : port ? 'Tracking not available yet' : g.playtimeMin ? hours(g.playtimeMin) : 'Not played yet'}</dd>
        <dt>Last played</dt>
        <dd>{g.lastPlayed ? ago(g.lastPlayed) : '—'}</dd>
        {#if g.lutris}<dt>Library provider</dt><dd>Lutris · {g.installed ? 'configured installation' : 'cached store listing'}</dd>{/if}
        <dt>Genre</dt>
        <dd>{g.genre}{g.year ? ` · ${g.year}` : ''}</dd>
        {#if g.developer}<dt>Developer</dt><dd>{g.developer}</dd>{/if}
        {#if g.publisher && g.publisher !== g.developer}<dt>Publisher</dt><dd>{g.publisher}</dd>{/if}
        {#if g.serial}
          <dt>Serial</dt>
          <dd>{g.serial}{g.region ? ` · ${g.region}` : ''}</dd>
        {/if}
        {#if g.setupState}
          <dt>{emulator}</dt>
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
  .about { white-space:pre-line; line-height:1.6; max-width:70ch; color:var(--text-soft); }
  .variant-picker { display:flex; flex-wrap:wrap; gap:18px; padding:16px; border-radius:14px; margin:16px 0; align-items:end; }
  .variant-picker label { display:grid; gap:8px; font-size:12px; color:var(--text-soft); }
  .variant-picker select { max-width:100%; color:var(--text); background:var(--field); border:1px solid var(--line); border-radius:8px; padding:10px; font:inherit; }
  .variant-picker span, .media-credit { font-size:12px; color:var(--text-dim); }
  .shot img { width:100%; height:100%; object-fit:cover; }
  .page {
    position: relative;
    isolation: isolate;
    padding-bottom: 40px;
  }
  .banner {
    position: absolute;
    inset: 0 0 auto;
    height: min(62vh, 620px);
    z-index: -1;
    overflow: hidden;
  }
  .banner-cover {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }
  .fade {
    position: absolute;
    inset: 0;
    background:
      linear-gradient(90deg, var(--bg) 0%, color-mix(in srgb, var(--bg) 40%, transparent) 45%, transparent 75%),
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
    backdrop-filter: var(--overlay-filter);
    -webkit-backdrop-filter: var(--overlay-filter);
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
    align-items: center;
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
