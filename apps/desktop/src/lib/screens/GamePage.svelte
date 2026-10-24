<script lang="ts">
  import { fly } from 'svelte/transition';
  import { tick } from 'svelte';
  import { gameDetailTabs, nextGameDetailTab, type GameDetailTab } from '../gameDetails';
  import { libraryView } from '../libraryView.svelte';
  import { app } from '../state.svelte';
  import { platformLabel, sourceLabel } from '../labels';
  import { canonicalId, groupGames, editionKey, editionLabel, launchMethod, methodLabel } from '../unified';
  import { canPlay, emulatorName, gameStatus, portAction } from '../presentation';
  import { cleanTitle, ago, hours, transferLine } from '../format';
  import GameMedia from '../components/GameMedia.svelte';
  import Icon from '../components/Icon.svelte';
  import PlatformIcon from '../components/PlatformIcon.svelte';
  import Row from '../components/Row.svelte';
  import InstallMenu from '../components/InstallMenu.svelte';
  import CatalogueDetails from '../components/CatalogueDetails.svelte';
  import Soundtrack from '../components/Soundtrack.svelte';
  let tab = $state<GameDetailTab>('overview');

  const g = $derived(app.current ?? app.hero);
  const variants = $derived(g ? app.games.filter((x) => canonicalId(x) === canonicalId(g)) : []);
  const editions = $derived([...new Map(variants.map((x) => [editionKey(x), x])).values()]);
  const choices = $derived(g ? variants.filter((x) => editionKey(x) === editionKey(g)) : []);
  function choose(id: string) { app.chooseVariant(id); }
  const port = $derived(g?.source === 'port' ? app.ports.find((p) => `port:${p.id}` === g.id) : undefined);
  const similar = $derived(g ? groupGames(app.games.filter((x) => canonicalId(x) !== canonicalId(g) && (g.genre ? x.genre === g.genre : x.platform === g.platform) && canPlay(x))).map((group) => group.game).slice(0, 10) : []);
  const dur = $derived(app.motionReduced ? 0 : 360);
  const offline = $derived(!!g && g.source === 'rom' && g.platform !== 'unknown');
  const emulator = $derived(g ? emulatorName(g) : 'emulator');
  const status = $derived(g ? gameStatus(g) : null);
  const share = $derived(g && offline ? app.transferFor(g.id) : undefined);
  $effect(() => { if (app.torrentsOn) return app.watchTransfers(); });
  let editing = $state(false);
  let titleDraft = $state('');
  let titleInput = $state<HTMLInputElement>();
  async function beginEdit() { if (!g || !offline) return; titleDraft = g.title; editing = true; await tick(); titleInput?.focus(); titleInput?.select(); }
  async function saveTitle(event: SubmitEvent) { event.preventDefault(); if (!g || !titleDraft.trim()) return; await app.rename(g, titleDraft.trim()); editing = false; }
  function browseCatalogue() { libraryView('all').discovery = true; app.go('library'); }


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
  <div class="head" in:fly={{ y: 20, duration: dur }}>
    <button class="back" data-nav onclick={() => app.back()}><Icon name="back" size={18} /> Back</button>
    {#if offline && !editing}<button class="edit-game btn glass small" data-nav onclick={beginEdit}><Icon name="edit" size={16} />Edit</button>{/if}
    {#if editing && offline}
      <form class="title-editor" onsubmit={saveTitle}>
        <label class="rename-label">Game title<input bind:this={titleInput} class="rename" data-nav bind:value={titleDraft} maxlength="300" required onkeydown={(event) => { if (event.key === 'Escape') { editing = false; event.stopPropagation(); } }} /></label>
        <div class="rename-actions"><button class="btn glass small" data-nav disabled={!titleDraft.trim()}>Save title</button><button type="button" class="btn glass small" data-nav onclick={() => editing = false}>Cancel</button></div>
      </form>
    {:else}<h1 title={g.title}>{cleanTitle(g.title)}</h1>{/if}
    <p class="tagline platform-label"><PlatformIcon platform={g.platform} size={24} />{platformLabel[g.platform]}{g.region ? ` · ${g.region}` : ''}{g.revision ? ` · Revision ${g.revision}` : ''}</p>
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
    {#if g.wanted}<p class="tagline" role="status">{g.wanted === 'wishlist' ? 'On your wishlist.' : 'You own this game.'} Play unlocks when a file for it is added: scan a folder that contains it, or use archive intake. It attaches to this entry automatically.</p>{/if}
    {#if offline && share}<p class="tagline" role="status">{transferLine(share)}</p>{/if}
    {#if offline && (g.mediaIssue || g.setupState === 'runner_missing')}<p class="tagline" role="status">{g.mediaIssue || `${emulator} is not installed or available for this platform. Open Tools to set it up.`}</p>{/if}
  </div>

  <div class="feature-layout">
    {#key g.id}<GameMedia game={g} />{/key}
    <aside class="feature-summary">
      {#if g.coverUrl && (g.heroUrl || g.screenshots?.length)}<img class="summary-cover" src={g.coverUrl} alt="{cleanTitle(g.title)} cover" />{/if}
      {#if g.description}<p class="short-description">{g.description.length > 450 ? `${g.description.slice(0,447)}…` : g.description}</p>{:else}<p class="short-description">{cleanTitle(g.title)}{g.year ? ` · ${g.year}` : ''}<br />{platformLabel[g.platform]}</p>{/if}
      <dl><dt>Platform</dt><dd><span class="platform-label"><PlatformIcon platform={g.platform} size={22} />{platformLabel[g.platform]}</span></dd>{#if g.year}<dt>Released</dt><dd>{g.year}</dd>{/if}{#if g.developer}<dt>Developer</dt><dd>{g.developer}</dd>{/if}{#if g.publisher}<dt>Publisher</dt><dd>{g.publisher}</dd>{/if}{#if g.players}<dt>Players</dt><dd>{g.players}</dd>{/if}</dl>
      {#if g.genre}<div class="genre-tag">{g.genre}</div>{/if}
      {#if g.descriptionSource}<p class="media-credit">Details: {g.descriptionSource}</p>{/if}
    </aside>
  </div>
  <div class="launch-toolbar">
    <div class="actions game-actions">
      {#if g.wanted}
        <button class="btn primary" data-nav onclick={() => app.addGames()}><Icon name="folder" /> Add game files</button>
        <button class="btn glass" data-nav onclick={() => app.setWantedStatus(g, g.wanted === 'wishlist' ? 'owned' : 'wishlist')}>{g.wanted === 'wishlist' ? 'I own this' : 'Move to wishlist'}</button>
        <button class="btn glass" data-nav onclick={() => app.removeWantedGame(g)}><Icon name="x" /> Remove</button>
      {:else if g.source === 'catalogue'}
        <p class="catalogue-note" role="status">Not in your library — catalogue discovery only. Open the source page from the facts panel when you want to browse Vimm’s Lair.</p>
      {:else if offline && g.setupState === 'runner_missing'}
        {#if g.platform === 'ps2'}<InstallMenu tone="primary" />
        {:else}<button class="btn primary" data-nav onclick={() => app.go('tools')}><Icon name="settings" /> Set up {g.platform === 'ps1' ? 'DuckStation' : g.platform === 'genesis' ? 'BlastEm' : g.platform === 'ds' ? 'melonDS' : g.platform === 'neogeo' ? 'FinalBurn Neo' : 'emulator'}</button>{/if}
      {:else}
        <button class="btn primary" class:game-play={playLabel === 'Play' || playLabel === 'Open port'} data-nav onclick={primary}>
          <Icon name={playReady ? 'play' : 'folder'} size={22} /> {playLabel}
        </button>
      {/if}
      {#if g.portStatus === 'update'}
        <button class="btn glass" data-nav onclick={() => port && app.installPort(port)}><Icon name="refresh" /> Update</button>
      {/if}
      <button class="btn glass icon" data-nav aria-label="Favourite" aria-pressed={g.favorite} disabled={g.source === 'port' || g.source === 'catalogue' || !!g.lutris} onclick={() => app.toggleFavorite(g)}>
        <Icon name="star" filled={g.favorite} />
      </button>
    </div>
  <div class="play-strip" aria-label="Game activity">
    <div><Icon name="clock" size={19} /><span><small>PLAYTIME</small><strong>{g.source === 'catalogue' || port ? 'Not tracked' : g.playtimeMin ? hours(g.playtimeMin) : 'Not played yet'}</strong></span></div>
    <div><span><small>LAST PLAYED</small><strong>{g.lastPlayed ? ago(g.lastPlayed) : '—'}</strong></span></div>
    <div><Icon name="controller" size={19} /><span><small>PLAY WITH</small><strong>{launchVia}</strong></span></div>
  </div>

  </div>

  <div class="game-tabs" role="tablist" tabindex="-1" aria-label="Game details" onkeydown={(event) => {
    if (['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) {
      event.preventDefault(); tab = nextGameDetailTab(tab, event.key);
      (event.currentTarget.querySelector(`[data-tab="${tab}"]`) as HTMLButtonElement)?.focus();
    }
  }}>
    {#each gameDetailTabs as item}<button id={`${item}-tab`} data-tab={item} role="tab" aria-selected={tab === item} aria-controls={`${item}-panel`} tabindex={tab === item ? 0 : -1} data-nav onclick={() => tab = item}>{item === 'overview' ? 'Overview' : item === 'ost' ? 'OST' : 'Metadata'}</button>{/each}
  </div>
  <div class="body">
    <div class="main" id={`${tab}-panel`} role="tabpanel" aria-labelledby={`${tab}-tab`} tabindex="0">
      {#if tab === 'ost'}
        {#key canonicalId(g)}<Soundtrack gameKey={canonicalId(g)} gameTitle={cleanTitle(g.title)} />{/key}
      {:else if tab === 'metadata'}
        <section class="metadata-intro"><h3>Metadata and artwork</h3><p class="about">Release information and source links for this installation. Matching updates metadata; it does not change your game files, saves or launch method.</p>
          {#if offline}<button class="btn glass small" data-nav disabled={app.fetchingArt} onclick={() => app.refreshArtwork(g.id)}><Icon name="refresh" size={16} />{app.fetchingArt ? 'Refreshing…' : 'Refresh artwork and details'}</button>{/if}
        </section>
        {#if g.metadata || offline && app.catalogue.records.length}<CatalogueDetails game={g} />{:else}<p class="about">No catalogue metadata is linked to this entry.</p><button class="btn glass small" data-nav onclick={browseCatalogue}>Browse catalogue</button>{/if}
        {#if g.descriptionSource}<p class="media-credit">Description source: {g.descriptionSource}</p>{/if}
      {:else}
        <section class="overview-about"><h3>About this game</h3>
          {#if g.description}<p class="about">{g.description}</p>{:else}<p class="about muted">A release description has not been added yet.</p>{/if}
          {#if g.descriptionSource}<p class="media-credit">Description source: {g.descriptionSource}</p>{/if}
        </section>
        {#if g.lutris}<h3>Managed by Lutris</h3><p class="about">Lutris manages installation and compatibility settings. Refresh its connection in Tools after installing or playing.</p>{/if}
        {#if port}<h3>Requirements</h3><ul class="reqs glass-panel"><li>{port.name}{port.version ? ` · ${port.version}` : ''} · {port.status === 'not-installed' ? 'Not installed' : 'Installed'}</li><li>{port.requirement}</li></ul>{/if}
      {/if}
    </div>
    <aside class="facts glass-panel" aria-label="Installation and identification">
      <h3>Installation details</h3>
      <dl>
        <dt>{g.source === 'rom' || g.source === 'port' ? 'Run method' : 'Library source'}</dt>
        <dd><span class="pill">{g.source === 'rom' || g.source === 'port' ? methodLabel[launchMethod(g)] : sourceLabel[g.source]}</span></dd>
        {#if g.metadata?.url}
          <dt>Source page</dt>
          <dd><button type="button" class="facts-link" data-nav onclick={() => app.openCataloguePage(g.metadata!.id)}>Open in browser</button></dd>
        {/if}
        <dt>Platform</dt>
        <dd><span class="platform-label"><PlatformIcon platform={g.platform} size={22} />{platformLabel[g.platform]}</span>{g.platform === 'unknown' && g.metadata ? ` · ${g.metadata.system}` : ''}</dd>
        {#if g.region}<dt>Region</dt><dd>{g.region}</dd>{/if}
        {#if g.languages?.length}<dt>Languages</dt><dd>{g.languages.join(', ')}</dd>{/if}
        {#if g.revision}<dt>ROM revision</dt><dd>{g.revision}</dd>{/if}
        {#if g.players}<dt>Players</dt><dd>{g.players}</dd>{/if}
        <dt>Launches with</dt>
        <dd>{launchVia}</dd>

        {#if g.lutris}<dt>Library provider</dt><dd>Lutris · {g.installed ? 'configured installation' : 'cached store listing'}</dd>{/if}
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
      {#if offline || g.lutris || port}<div class="side-actions">
        {#if offline}<button class="btn glass small" data-nav onclick={() => app.go('tools')}><Icon name="settings" size={16} />Emulator settings</button>{/if}
      {#if offline}
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
      </div>{/if}
      <section class="identity"><h4>Identification</h4><dl>
        {#if g.serial}<dt>Serial</dt><dd>{g.serial}</dd>{/if}
        <dt>Metadata</dt><dd>{g.source === 'catalogue' ? 'Catalogue entry' : g.metadata ? g.metadataMatch === 'sha1' ? 'Exact SHA-1 link' : 'Manually linked' : 'No catalogue link'}</dd>
        {#if tab === 'metadata'}<dt>Library ID</dt><dd class="identifier">{g.id}</dd>{#if canonicalId(g) !== g.id}<dt>Game identity</dt><dd class="identifier">{canonicalId(g)}</dd>{/if}{/if}
      </dl></section>
    </aside>
  </div>
  {#if tab === 'overview'}<Row title={g.genre ? 'More in this genre' : `More for ${platformLabel[g.platform]}`} games={similar} />{/if}

</article>
{/if}

<style>
  .edit-game { position: absolute; top: 18px; right: var(--gutter); }
  .title-editor { display: grid; gap: 12px; } .rename-actions { display: flex; gap: 8px; }
  .side-actions { display: flex; flex-wrap: wrap; gap: 8px; margin-top: 18px; }
  .identity { margin-top: 22px; padding-top: 18px; border-top: 1px solid var(--line); }
  .identity h4 { font-size: 13px; margin: 0 0 14px; } .identifier { overflow-wrap: anywhere; font-family: ui-monospace,monospace; font-size: 11px; }
  .metadata-intro { margin-bottom: 20px; } .overview-about { margin: 12px 0 28px; }
  .muted { color: var(--text-dim); }
  .main { min-width: 0; } .main :global(.soundtrack) { padding: 0; max-width: none; }

  .feature-layout { display: grid; grid-template-columns: minmax(0,2.3fr) minmax(250px,1fr); gap: 24px; padding: 0 var(--gutter) 24px; max-width: 1500px; margin: 0 auto; }
  .feature-summary { min-width: 0; padding: 4px 0; }
  .summary-cover { width: 100%; max-height: 165px; object-fit: contain; object-position: left; margin-bottom: 12px; }
  .short-description { font-size: 14px; line-height: 1.6; color: var(--text-soft); margin: 0 0 20px; }
  .feature-summary dl { display: grid; grid-template-columns: 90px minmax(0,1fr); gap: 10px 12px; margin: 0; font-size: 12px; }
  .feature-summary dt { color: var(--text-dim); } .feature-summary dd { margin: 0; color: var(--text-soft); overflow-wrap: anywhere; }
  .genre-tag { display: inline-block; margin-top: 20px; padding: 5px 9px; border-radius: 4px; background: var(--field); color: var(--text-soft); font-size: 11px; }
  .launch-toolbar { margin: 0 var(--gutter) 12px; padding: 18px 20px; display: flex; align-items: center; flex-wrap: wrap; gap: 20px 32px; background: var(--surface); border: 1px solid var(--line); border-radius: 8px; }
  .launch-toolbar .actions { margin: 0; } .launch-toolbar .play-strip { flex: 1; padding: 0; border: 0; background: transparent; gap: 18px 28px; }
  @media(max-width:850px) { .feature-layout { grid-template-columns: 1fr; } .feature-summary { padding: 4px 0 0; } .summary-cover { display: none; } .feature-summary dl { grid-template-columns: 100px minmax(0,1fr); } }

  .play-strip { display: flex; align-items: center; flex-wrap: wrap; gap: 20px 40px; padding: 20px var(--gutter); background: var(--surface); border-block: 1px solid var(--line); }
  .play-strip > div { display: flex; align-items: center; gap: 10px; min-width: 110px; color: var(--text-dim); }
  .play-strip span { display: grid; gap: 6px; } .play-strip small { font-size: 10px; letter-spacing: .09em; } .play-strip strong { font-size: 13px; font-weight: 600; color: var(--text-soft); }
  .game-tabs { display: flex; gap: 24px; padding: 0 var(--gutter); margin: 12px 0 0; border-bottom: 1px solid var(--line); }
  .game-tabs button { border: 0; border-bottom: 2px solid transparent; padding: 14px 4px; background: none; color: var(--text-dim); cursor: pointer; font: inherit; }
  .game-tabs button[aria-selected='true'] { border-color: var(--accent); color: var(--text); }
  .about { white-space:pre-line; line-height:1.6; max-width:70ch; color:var(--text-soft); }
  .variant-picker { display:flex; flex-wrap:wrap; gap:18px; padding:16px; border-radius:14px; margin:16px 0; align-items:end; }
  .variant-picker label { display:grid; gap:8px; font-size:12px; color:var(--text-soft); }
  .variant-picker select { max-width:100%; min-height:42px; }
  .variant-picker span, .media-credit { font-size:12px; color:var(--text-dim); }
  .page {
    position: relative;
    isolation: isolate;
    padding-bottom: 40px;
    max-width: 1500px;
    margin: 0 auto;
  }
  .availability { margin:0; color:var(--warn); font-size:14px; }
  .rename-label { display:grid; gap:8px; }
  .rename-actions { display:flex; gap:8px; }
  .head { position: relative; padding: 68px var(--gutter) 24px; display: grid; gap: 12px; }
  h1 { margin: 0; font-size: clamp(24px,2.8vw,38px); line-height: 1.2; letter-spacing: -.02em; max-width: 38ch; }
  .back { position: absolute; top: 18px; left: var(--gutter); display: flex; align-items: center; gap: 6px; padding: 6px 0; border: 0; background: transparent; color: var(--text-soft); font: inherit; cursor: pointer; }
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
