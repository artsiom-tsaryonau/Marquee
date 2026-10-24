<script lang="ts">
  import { fly } from 'svelte/transition';
  import { app } from '../state.svelte';
  import { platformLabel, sourceLabel } from '../labels';
  import { canonicalId, groupGames, editionKey, editionLabel, launchMethod, methodLabel } from '../unified';
  import { canPlay, emulatorName, gameStatus, portAction } from '../presentation';
  import { cleanTitle, ago, hours, transferLine } from '../format';
  import Artwork from '../components/Artwork.svelte';
  import Icon from '../components/Icon.svelte';
  import Row from '../components/Row.svelte';
  import InstallMenu from '../components/InstallMenu.svelte';
  import CatalogueDetails from '../components/CatalogueDetails.svelte';
  import Soundtrack from '../components/Soundtrack.svelte';
  let tab = $state<'overview' | 'ost'>('overview');

  const g = $derived(app.current ?? app.hero);
  const variants = $derived(g ? app.games.filter((x) => canonicalId(x) === canonicalId(g)) : []);
  const editions = $derived([...new Map(variants.map((x) => [editionKey(x), x])).values()]);
  const choices = $derived(g ? variants.filter((x) => editionKey(x) === editionKey(g)) : []);
  function choose(id: string) { app.chooseVariant(id); }
  const port = $derived(g?.source === 'port' ? app.ports.find((p) => `port:${p.id}` === g.id) : undefined);
  const similar = $derived(g?.genre ? groupGames(app.games.filter((x) => canonicalId(x) !== canonicalId(g) && x.genre === g.genre && canPlay(x))).map((group) => group.game).slice(0, 10) : []);
  const dur = $derived(app.motionReduced ? 0 : 360);
  const offline = $derived(!!g && g.source === 'rom' && g.platform !== 'unknown');
  const emulator = $derived(g ? emulatorName(g) : 'emulator');
  const status = $derived(g ? gameStatus(g) : null);
  const share = $derived(g && offline ? app.transferFor(g.id) : undefined);
  $effect(() => { if (app.torrentsOn) return app.watchTransfers(); });
  let editing = $state(false);
  let titleDraft = $state('');
  let identification = $state<HTMLDetailsElement>();
  function showIdentification() { if (identification) { identification.open = true; identification.scrollIntoView({ block: 'center', behavior: app.motionReduced ? 'auto' : 'smooth' }); identification.querySelector('summary')?.focus(); } }

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

  const playReady = $derived(!!g && canPlay(g));
  const playLabel = $derived(!g ? 'Play' : g.rootOffline || g.mediaMissing ? 'Review library files' :
    g.setupState === 'missing' ? 'Add firmware' : port ? port.status === 'update' ? 'Open port' : portAction(port).label :
    g.lutris && !g.installed ? 'Install in Lutris' : playReady ? 'Play' : 'View setup in Tools');
  function primary() {
    if (!g) return;
    if (g.rootOffline || g.mediaMissing) { app.go('library'); return; }
    if (g.setupState === 'missing') { void app.importBios(); return; }
    if (port && !playReady) {
      if (portAction(port).operation === 'launch' || port.status === 'update') void app.launchPort(port);
      else if (portAction(port).operation === 'folder') void app.openPortFolder(port);
      else void app.installPort(port);
      return;
    }
    if (g.lutris) { void app.openLutrisGame(g); return; }
    if (playReady) app.play(g);
    else app.go('tools');
  }
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
  <div class="banner" class:cover-backdrop={!g.heroUrl}>
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
      <label class="rename-label">Game title<input class="rename" data-nav bind:value={titleDraft} onkeydown={(e) => { if (e.key === 'Enter') { void app.rename(g, titleDraft); editing = false; } if (e.key === 'Escape') { editing = false; e.stopPropagation(); } }} /></label>
      <div class="rename-actions"><button class="btn glass small" data-nav onclick={() => { void app.rename(g, titleDraft); editing = false; }}>Save title</button><button class="btn glass small" data-nav onclick={() => (editing = false)}>Cancel</button></div>
    {:else}
      <h1 title={g.title}>{cleanTitle(g.title)}</h1>
    {/if}
    {#if g.coverUrl}<img class="detail-poster" src={g.coverUrl} alt="{cleanTitle(g.title)} cover" />{/if}
    <p class="tagline">{platformLabel[g.platform]}{g.region ? ` · ${g.region}` : ''}{g.revision ? ` · Revision ${g.revision}` : ''}</p>
    {#if status}<p class="availability" role="status">{status.label}</p>{/if}
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
      {#if g.wanted}
        <button class="btn primary big" data-nav onclick={() => app.addGames()}><Icon name="folder" /> Add game files</button>
        <button class="btn glass" data-nav onclick={() => app.setWantedStatus(g, g.wanted === 'wishlist' ? 'owned' : 'wishlist')}>{g.wanted === 'wishlist' ? 'I own this' : 'Move to wishlist'}</button>
        <button class="btn glass" data-nav onclick={() => app.removeWantedGame(g)}><Icon name="x" /> Remove</button>
      {:else if g.source === 'catalogue'}
        <p class="catalogue-note" role="status">Not in your library — catalogue discovery only. Open the source page from the facts panel when you want to browse Vimm’s Lair.</p>
      {:else if offline && g.setupState === 'runner_missing'}
        {#if g.platform === 'ps2'}<InstallMenu tone="primary" size="big" />
        {:else}<button class="btn primary big" data-nav onclick={() => app.go('tools')}><Icon name="settings" /> Set up {g.platform === 'ps1' ? 'DuckStation' : g.platform === 'genesis' ? 'BlastEm' : g.platform === 'ds' ? 'melonDS' : g.platform === 'neogeo' ? 'FinalBurn Neo' : 'emulator'}</button>{/if}
      {:else}
        <button class="btn primary big" data-nav onclick={primary}>
          <Icon name={playReady ? 'play' : 'folder'} size={24} /> {playLabel}
        </button>
      {/if}
      {#if offline && !g.coverUrl}
        <button class="btn glass" data-nav onclick={() => app.refreshArtwork(g.id)} disabled={app.fetchingArt}>
          <Icon name="image" /> {app.fetchingArt ? 'Finding artwork…' : 'Find artwork'}
        </button>
      {/if}
      {#if g.portStatus === 'update'}
        <button class="btn glass" data-nav onclick={() => port && app.installPort(port)}><Icon name="refresh" /> Update</button>
      {/if}
      <button class="btn glass icon" data-nav aria-label="Favourite" aria-pressed={g.favorite} disabled={g.source === 'port' || g.source === 'catalogue' || !!g.lutris} onclick={() => app.toggleFavorite(g)}>
        <Icon name="star" filled={g.favorite} />
      </button>
      {#if offline}
        <details class="manage-menu"><summary class="btn glass" data-nav>Manage game <Icon name="chevron" size={16} /></summary>
          <div class="manage-actions glass-panel">
            <button class="btn glass small" data-nav onclick={() => { titleDraft = g.title; editing = true; }}>Edit title</button>
            <button class="btn glass small" data-nav onclick={() => app.go('tools')}>Emulator settings</button>
          </div>
        </details>
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
    {#if g.wanted}<p class="tagline" role="status">{g.wanted === 'wishlist' ? 'On your wishlist.' : 'You own this game.'} Play unlocks when a file for it is added: scan a folder that contains it, or use archive intake. It attaches to this entry automatically.</p>{/if}
    {#if offline && share}<p class="tagline" role="status">{transferLine(share)}</p>{/if}
    {#if offline && (g.mediaIssue || g.setupState === 'runner_missing')}<p class="tagline" role="status">{g.mediaIssue || `${emulator} is not installed or available for this platform. Open Tools to set it up.`}</p>{/if}
  </div>

  <div class="play-strip" aria-label="Game activity">
    <div><Icon name="clock" size={19} /><span><small>PLAYTIME</small><strong>{g.source === 'catalogue' || port ? 'Not tracked' : g.playtimeMin ? hours(g.playtimeMin) : 'Not played yet'}</strong></span></div>
    <div><span><small>LAST PLAYED</small><strong>{g.lastPlayed ? ago(g.lastPlayed) : '—'}</strong></span></div>
    <div><Icon name="controller" size={19} /><span><small>PLAY WITH</small><strong>{launchVia}</strong></span></div>
  </div>

  <div class="game-tabs" role="tablist" tabindex="-1" aria-label="Game details" onkeydown={(event) => {
    if (['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) {
      event.preventDefault(); tab = event.key === 'Home' ? 'overview' : event.key === 'End' ? 'ost' : tab === 'overview' ? 'ost' : 'overview';
      (event.currentTarget.querySelector(`[data-tab="${tab}"]`) as HTMLButtonElement)?.focus();
    }
  }}>
    <button id="overview-tab" data-tab="overview" role="tab" aria-selected={tab === 'overview'} aria-controls="overview-panel" tabindex={tab === 'overview' ? 0 : -1} data-nav onclick={() => tab = 'overview'}>Overview</button>
    <button id="ost-tab" data-tab="ost" role="tab" aria-selected={tab === 'ost'} aria-controls="ost-panel" tabindex={tab === 'ost' ? 0 : -1} data-nav onclick={() => tab = 'ost'}>OST</button>
  </div>
  {#if tab === 'ost'}
    <div id="ost-panel" role="tabpanel" aria-labelledby="ost-tab" tabindex="0">{#key canonicalId(g)}<Soundtrack gameKey={canonicalId(g)} gameTitle={cleanTitle(g.title)} />{/key}</div>
  {:else}
  <div id="overview-panel" role="tabpanel" aria-labelledby="overview-tab" tabindex="0">
  <div class="body">
    <div class="main">
      {#if g.description}
        <h3>About</h3>
        <p class="about">{g.description}</p>
        {#if g.descriptionSource}<p class="media-credit">Description source: {g.descriptionSource}</p>{/if}
      {/if}

      {#if g.screenshots?.length}
      <h3>Screenshots</h3>
      <div class="shots" data-nav-row>
        {#each g.screenshots as shot}<a class="shot" href={shot.url} target="_blank" rel="noreferrer" data-nav aria-label={shot.caption}><img src={shot.url} alt={shot.caption} loading="lazy" /></a>{/each}
      </div>
      <p class="media-credit">{g.mediaCredit} <a href={g.mediaSource} target="_blank" rel="noreferrer" data-nav>Source gallery</a></p>
      {/if}
      {#if !g.description && !g.screenshots?.length}
        <section class="library-summary glass-panel"><h3>Your copy</h3><p>{platformLabel[g.platform]}{g.region ? ` · ${g.region}` : ''}{g.year ? ` · ${g.year}` : ''}</p><p class="summary-note">{g.coverUrl ? 'Artwork is available. Release details and screenshots have not been added yet.' : 'Add artwork to make this game easier to find in your library.'}</p><div class="summary-actions">{#if offline && !g.coverUrl}<button class="btn glass small" data-nav disabled={app.fetchingArt} onclick={() => app.refreshArtwork(g.id)}><Icon name="image" size={17} />{app.fetchingArt ? 'Finding artwork…' : 'Find artwork'}</button>{/if}{#if offline || g.metadata}<button class="btn glass small" data-nav onclick={showIdentification}>Review game metadata</button>{/if}<button class="btn glass small" data-nav onclick={() => tab = 'ost'}><Icon name="music" size={17} />Soundtracks</button></div></section>
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
      {#if g.metadata || offline}<details class="technical" bind:this={identification}><summary data-nav>Identification and source metadata</summary><CatalogueDetails game={g} /></details>{/if}
    </div>

    <aside class="facts glass-panel">
      <h3>Game details</h3>
      <dl>
        <dt>{g.source === 'rom' || g.source === 'port' ? 'Run method' : 'Library source'}</dt>
        <dd><span class="pill">{g.source === 'rom' || g.source === 'port' ? methodLabel[launchMethod(g)] : sourceLabel[g.source]}</span></dd>
        {#if g.metadata?.url}
          <dt>Source page</dt>
          <dd><button type="button" class="facts-link" data-nav onclick={() => app.openCataloguePage(g.metadata!.id)}>Open in browser</button></dd>
        {/if}
        <dt>Platform</dt>
        <dd>{platformLabel[g.platform]}{g.platform === 'unknown' && g.metadata ? ` · ${g.metadata.system}` : ''}</dd>
        {#if g.region}<dt>Region</dt><dd>{g.region}</dd>{/if}
        {#if g.languages?.length}<dt>Languages</dt><dd>{g.languages.join(', ')}</dd>{/if}
        {#if g.revision}<dt>ROM revision</dt><dd>{g.revision}</dd>{/if}
        {#if g.players}<dt>Players</dt><dd>{g.players}</dd>{/if}
        <dt>Launches with</dt>
        <dd>{launchVia}</dd>

        {#if g.lutris}<dt>Library provider</dt><dd>Lutris · {g.installed ? 'configured installation' : 'cached store listing'}</dd>{/if}
        {#if g.genre}<dt>Genre</dt><dd>{g.genre}</dd>{/if}
        {#if g.year}<dt>Released</dt><dd>{g.year}</dd>{/if}
        {#if g.developer}<dt>Developer</dt><dd>{g.developer}</dd>{/if}
        {#if g.publisher && g.publisher !== g.developer}<dt>Publisher</dt><dd>{g.publisher}</dd>{/if}
        {#if g.serial}
          <dt>Serial</dt>
          <dd>{g.serial}{g.region ? ` · ${g.region}` : ''}</dd>
        {/if}
        {#if g.setupState && (g.setupState !== 'unverified' || ['ps1', 'ps2', 'ds', 'neogeo'].includes(g.platform))}
          <dt>{emulator}</dt>
          <dd>{{ runner_missing: 'Not installed', missing: 'Firmware required', unverified: 'Firmware not yet checked', launch_tested: 'Previously started' }[g.setupState]}</dd>
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
          <dd>Unknown · no verified report</dd>
        {/if}
      </dl>
    </aside>
  </div>

  <Row title="More in this genre" games={similar} />
  </div>
  {/if}
</article>
{/if}

<style>
  .play-strip { display: flex; align-items: center; flex-wrap: wrap; gap: 20px 40px; padding: 20px var(--gutter); background: var(--surface); border-block: 1px solid var(--line); }
  .play-strip > div { display: flex; align-items: center; gap: 10px; min-width: 110px; color: var(--text-dim); }
  .play-strip span { display: grid; gap: 6px; } .play-strip small { font-size: 10px; letter-spacing: .09em; } .play-strip strong { font-size: 13px; font-weight: 600; color: var(--text-soft); }
  .library-summary { padding: 24px; border-radius: 14px; margin-bottom: 24px; }
  .library-summary p { color: var(--text-soft); line-height: 1.6; } .library-summary .summary-note { color: var(--text-dim); font-size: 13px; }
  .summary-actions { display: flex; gap: 10px; flex-wrap: wrap; margin-top: 20px; }

  .game-tabs { display: flex; gap: 24px; padding: 0 var(--gutter); margin: 12px 0 0; border-bottom: 1px solid var(--line); }
  .game-tabs button { border: 0; border-bottom: 2px solid transparent; padding: 14px 4px; background: none; color: var(--text-dim); cursor: pointer; font: inherit; }
  .game-tabs button[aria-selected='true'] { border-color: var(--accent); color: var(--text); }
  .about { white-space:pre-line; line-height:1.6; max-width:70ch; color:var(--text-soft); }
  .variant-picker { display:flex; flex-wrap:wrap; gap:18px; padding:16px; border-radius:14px; margin:16px 0; align-items:end; }
  .variant-picker label { display:grid; gap:8px; font-size:12px; color:var(--text-soft); }
  .variant-picker select { max-width:100%; min-height:42px; }
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
    height: clamp(320px, 48vh, 480px);
    z-index: -1;
    overflow: hidden;
  }
  .cover-backdrop .banner-cover { filter:blur(32px) saturate(.55) brightness(.45); transform:scale(1.15); }
  h1 { margin:0; font-size:clamp(30px, 4vw, 54px); line-height:1.12; letter-spacing:-.03em; text-wrap:balance; max-width:22ch; }
  .detail-poster { position:absolute; right:var(--gutter); bottom:28px; width:clamp(120px,16vw,190px); max-height:260px; object-fit:contain; border-radius:8px; filter:drop-shadow(0 16px 25px #0006); }
  @media(min-width:850px) { .head:has(.detail-poster) { padding-right:calc(var(--gutter) + 230px); } }
  @media(max-width:849px) { .detail-poster { display:none; } }
  .head { position:relative; }
  .availability { margin:0; color:var(--warn); font-size:14px; }
  .technical { margin-top:24px; }
  .technical > summary { cursor:pointer; padding:14px 0; color:var(--text-soft); }
  .manage-menu { position:relative; }
  .manage-menu summary { list-style:none; cursor:pointer; }
  .manage-actions { position:absolute; top:calc(100% + 8px); left:0; z-index:10; display:grid; gap:8px; min-width:210px; border-radius:12px; padding:10px; background:var(--surface-strong); }
  .rename-label { display:grid; gap:8px; }
  .rename-actions { display:flex; gap:8px; }
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
    min-height: clamp(280px, 42vh, 430px);
    padding: 84px var(--gutter) 28px;
    display: flex;
    flex-direction: column;
    justify-content: flex-end;
    gap: 14px;
    max-width: 1400px;
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
    width:min(100%, 640px);
  }
  .actions {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 10px;
    margin-top: 4px;
  }
  .catalogue-note {
    margin: 0;
    padding: 12px 16px;
    border-radius: 12px;
    background: var(--field);
    border: 1px solid var(--line);
    font-size: 14px;
    color: var(--text-soft);
    max-width: 52ch;
  }
  .facts-link {
    padding: 0;
    border: 0;
    background: none;
    color: var(--accent);
    font: inherit;
    font-size: inherit;
    text-decoration: underline;
    text-underline-offset: 3px;
    cursor: pointer;
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
