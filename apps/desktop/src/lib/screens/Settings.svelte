<script lang="ts">
  import { onMount } from 'svelte';
  import { transferLine, bytes } from '../format';
  import { app } from '../state.svelte';
  import type { KeyInfo, TorrentReview } from '../api/contracts';
  import Icon from '../components/Icon.svelte';
  import InstallMenu from '../components/InstallMenu.svelte';
  import { musicAccents } from '../musicAppearance';

  const categories = [['appearance', 'Appearance'], ['games', 'Games and controls'], ['library', 'Library locations'], ['providers', 'Artwork and API keys'], ['devices', 'Torrents and devices']] as const;
  let category = $state<string>('appearance');
  let drafts = $state<Record<string, string>>({});
  let link = $state('');
  let torrentPath = $state('');
  let torrentReview = $state<TorrentReview | null>(null);
  let torrentBusy = $state(false);
  let torrentError = $state('');
  let peers = $state('');
  $effect(() => { if (app.torrentsOn) return app.watchTransfers(); });
  onMount(() => void app.loadKeys());

  async function reviewTorrent() {
    torrentBusy = true; torrentError = ''; torrentReview = null;
    try {
      const path = await app.api.pickTorrent();
      if (path) { torrentPath = path; torrentReview = await app.api.transferReview(path); }
    } catch (err) { torrentError = (err as Error).message; }
    finally { torrentBusy = false; }
  }

  async function downloadTorrent() {
    if (!torrentReview || torrentBusy) return;
    torrentBusy = true; torrentError = '';
    try {
      await app.api.transferAddFile(torrentPath, torrentReview.review_token, peers.split(/[,\s]+/).filter(Boolean));
      torrentReview = null; peers = '';
      await app.refreshTransfers();
    } catch (err) { torrentError = (err as Error).message; }
    finally { torrentBusy = false; }
  }

  const sourceLabel = (k: KeyInfo) =>
    ({
      app: `Saved in Marquee · ${k.preview}`,
      env: `From the environment · ${k.preview} · save here to use a different key`,
      dot_env: `From a .env file · ${k.preview} · save here to use a different key`,
      none: 'Not set',
    })[k.source];

  async function save(k: KeyInfo) {
    const value = drafts[k.id]?.trim();
    if (!value) return;
    await app.saveKey(k.id, value);
    drafts[k.id] = '';
  }
</script>

<div class="settings">
  <h1>Settings</h1>
  <div class="settings-layout">
    <nav class="settings-nav" aria-label="Settings categories">
      {#each categories as [id, label]}<button data-nav class:on={category === id} aria-pressed={category === id} onclick={() => (category = id)}>{label}</button>{/each}
    </nav>
    <div class="settings-content">

  {#each [{ id: 'ps1', name: 'PlayStation', emulator: 'DuckStation', kind: 'duckstation' }, { id: 'ps2', name: 'PlayStation 2', emulator: 'PCSX2', kind: 'pcsx2' }] as system}
    {@const runner = app.runnerFor(system.id)}
    {@const choices = app.runners.filter((r) => r.kind === system.kind)}
    <section class="glass-panel" hidden={category !== 'library'}>
      <h2>{system.name}</h2>
      <div class="item"><div><strong>Disc folders</strong>
        {#each app.roots.filter((r) => r.system === system.id) as root}<span>{root.path} · {root.status}</span>{:else}<span>No folder selected</span>{/each}
        <span>{app.games.filter((g) => g.source === 'rom' && g.platform === system.id).length} games found · {system.id === 'ps1' ? 'ISO, CHD, CUE with its tracks' : 'ISO'}</span>
      </div><button class="btn glass small" data-nav onclick={() => app.addSystemFolder(system.id)}><Icon name="folder" size={16} /> Add folder</button></div>
      <div class="item"><div><strong>{system.emulator}</strong><span>{runner ? `${runner.display_name} · ${runner.bios_state === 'launch_tested' ? 'previously started' : runner.bios_state === 'unverified' ? 'firmware not yet checked' : runner.bios_state === 'missing' ? 'firmware required' : runner.bios_state}` : 'No available emulator detected'}</span>
        {#if runner?.bios_hint}<span>{runner.bios_hint}</span>{/if}
        {#if system.id === 'ps2' && app.installingPcsx2}<span>{app.installProgress || 'Installing…'}</span>{/if}
        {#if !runner && system.id === 'ps1'}<span>Install DuckStation from Tools; Marquee downloads and configures it for you.</span>{/if}
      </div><div class="row-actions">
        {#if system.id === 'ps2'}<InstallMenu tone="primary" size="small" />{:else if !runner}<button class="btn glass small" data-nav onclick={() => app.go('tools')}>Set up in Tools</button>{/if}
      </div></div>
      {#if choices.length > 1}<div class="item"><div><strong>Default emulator</strong><span>Saved independently for {system.name}</span></div><select data-nav aria-label={`${system.name} default emulator`} value={runner?.id || ''} onchange={(e) => app.chooseRunner(e.currentTarget.value)}>{#each choices as choice}<option value={choice.id} disabled={!choice.available}>{choice.display_name}{choice.available ? '' : ' · unavailable'}</option>{/each}</select></div>{/if}
    </section>
  {/each}
  <section class="glass-panel" hidden={category !== 'library'}>
    <h2>Disc library maintenance</h2>
    <div class="item"><div><strong>Last import</strong><span>{app.lastScan ? `${app.lastScan.added} added · ${app.lastScan.skipped_unsupported} companion or unsupported files` : 'No scan results available in this session'}</span></div><button class="btn glass small" data-nav onclick={() => app.rescan()} disabled={app.scanning}>{app.scanning ? 'Scanning…' : 'Rescan folders'}</button></div>
    <div class="item"><div><strong>Emulator detection</strong><span>Refresh after installing or moving an emulator.</span></div><button class="btn glass small" data-nav onclick={() => app.refreshRunners()}>Refresh emulators</button></div>
    <div class="item"><div><strong>Box art</strong><span>Looks up PS1 and PS2 covers by disc serial. Missing artwork never blocks Play.</span></div><button class="btn glass small" data-nav onclick={() => app.refreshArtwork()} disabled={app.fetchingArt || !app.games.length}>{app.fetchingArt ? 'Fetching…' : 'Fetch covers'}</button></div>
  </section>

  <section class="glass-panel" hidden={category !== 'providers'}>
    <h2>API keys</h2>
    <p class="hint">
      Optional provider keys enable artwork and supported library features. Secrets stay on this computer; only their last four characters are shown after saving. Installed games do not require an artwork key.
    </p>
    {#each app.keys as k (k.id)}
      <div class="item key">
        <div>
          <strong>{k.label} <span class="pill">{k.used_by}</span></strong>
          <span>{k.description}</span>
          <span>{k.help}</span>
          <span class="status" class:set={k.set}>{sourceLabel(k)}</span>
        </div>
        <form class="key-edit" onsubmit={(e) => { e.preventDefault(); void save(k); }}>
          <input
            data-nav
            type={k.secret ? 'password' : 'text'}
            autocomplete="off"
            spellcheck="false"
            placeholder={k.set ? 'Replace value…' : 'Paste value'}
            aria-label={k.label}
            bind:value={drafts[k.id]}
          />
          <button class="btn small primary" data-nav type="submit" disabled={!drafts[k.id]?.trim()}>Save</button>
          {#if k.source === 'app'}
            <button class="btn glass small" data-nav type="button" onclick={() => app.clearKey(k.id)}>Clear</button>
          {/if}
        </form>
      </div>
    {/each}
  </section>

  <section class="glass-panel" hidden={category !== 'games'}>
    <h2>Games</h2>
    <div class="item">
      <div>
        <strong>Open games in</strong>
        <span>Applies to every emulator Marquee starts, the next time you press Play.</span>
      </div>
      <select data-nav aria-label="How games open" value={app.runnerSettings.display_mode ?? 'window'} onchange={(e) => app.setDisplayMode(e.currentTarget.value as 'window' | 'fullscreen')}>
        <option value="window">A window</option>
        <option value="fullscreen">Fullscreen</option>
      </select>
    </div>
  </section>

  <section class="glass-panel" hidden={category !== 'devices'}>
    <h2>Torrents and device transfers</h2>
    <div class="item">
      <div>
        <strong>Enable BitTorrent transfers</strong>
        <span>Off by default. Private device links use direct peers. Torrent files and public magnets use only the discovery options you enable below.</span>
      </div>
      <label class="pin-choice"><input type="checkbox" data-nav aria-label="Enable BitTorrent transfers" disabled={app.torrentSettingsBusy} checked={app.torrentSettings.enabled} onchange={(e) => app.setTorrents({ enabled: e.currentTarget.checked })} /> {app.torrentSettings.enabled ? 'On' : 'Off'}</label>
    </div>
    {#if app.torrentSettings.enabled}
      <div class="item"><div><strong>Tracker discovery for torrent downloads</strong><span>Contact trackers listed in imported torrents or public magnets. Your address is visible to those trackers and peers. Private device shares stay separate.</span></div><label class="pin-choice"><input type="checkbox" data-nav aria-label="Tracker discovery" disabled={app.torrentSettingsBusy} checked={!!app.torrentSettings.trackers} onchange={(e) => app.setTorrents({ trackers:e.currentTarget.checked })} /> Trackers</label></div>
      <div class="item"><div><strong>DHT discovery for public torrents</strong><span>Find peers through the public DHT network. Disabled for private device shares and torrents marked private.</span></div><label class="pin-choice"><input type="checkbox" data-nav aria-label="DHT discovery" disabled={app.torrentSettingsBusy} checked={!!app.torrentSettings.dht} onchange={(e) => app.setTorrents({ dht:e.currentTarget.checked })} /> DHT</label></div>
      <div class="item">
        <div>
          <strong>Allow uploads and seeding</strong>
          <span>Applies to private sharing and public torrents. Turn off to receive only. Original downloaded files are required to seed their torrent.</span>
        </div>
        <label class="pin-choice"><input type="checkbox" data-nav aria-label="Allow uploads and seeding" disabled={app.torrentSettingsBusy} checked={app.torrentSettings.seeding} onchange={(e) => app.setTorrents({ seeding: e.currentTarget.checked })} /> {app.torrentSettings.seeding ? 'Allowed' : 'Receive only'}</label>
      </div>
      <div class="item">
        <div>
          <strong>Upload speed limit</strong>
          <span>Applies to everything this device shares.</span>
        </div>
        <select data-nav aria-label="Upload speed limit" disabled={app.torrentSettingsBusy} value={String(app.torrentSettings.upload_kbps)} onchange={(e) => app.setTorrents({ upload_kbps: Number(e.currentTarget.value) })}>
          <option value="0">No limit</option>
          <option value="1024">1 MB/s</option>
          <option value="5120">5 MB/s</option>
          <option value="20480">20 MB/s</option>
        </select>
      </div>
      <div class="item"><div><strong>Download speed limit</strong><span>Applies to all downloads.</span></div><select data-nav aria-label="Download speed limit" disabled={app.torrentSettingsBusy} value={String(app.torrentSettings.download_kbps)} onchange={(e) => app.setTorrents({ download_kbps:Number(e.currentTarget.value) })}><option value="0">No limit</option><option value="1024">1 MB/s</option><option value="5120">5 MB/s</option><option value="20480">20 MB/s</option></select></div>
      <div class="item"><div><strong>Import a torrent file</strong><span>Review filenames, size and destination before starting. Downloading packages does not install a game. Files stay separate from existing installations.</span></div><button class="btn glass small torrent-choose" data-nav disabled={torrentBusy} onclick={reviewTorrent}>Choose .torrent…</button></div>
      {#if torrentError}<p role="alert">{torrentError}</p>{/if}
      {#if torrentReview}
        <div class="item torrent-review"><div><strong>{torrentReview.name}</strong><span>{torrentReview.files.length} files · {bytes(torrentReview.total)} · {torrentReview.tracker_count} trackers · {torrentReview.private ? 'Private torrent' : 'Public torrent'}</span><span>Save to: {torrentReview.directory}</span><span>All listed files will download. No installation or executable launch is performed.</span><details><summary data-nav>Review files</summary><ul class="torrent-files">{#each torrentReview.files.slice(0, 50) as file}<li>{file.path} · {bytes(file.size)}{#if torrentReview.preparation_files?.find(f => f.path === file.path)?.reason}<span>{torrentReview.preparation_files.find(f => f.path === file.path)?.reason}</span>{/if}</li>{/each}</ul>{#if torrentReview.files.length > 50}<p>Showing the first 50 files.</p>{/if}</details><label>Direct peers (optional)<input class="transfer-input" data-nav aria-label="Torrent direct peers" placeholder="192.168.1.20:51413" bind:value={peers} /></label></div><div class="row-actions"><button class="btn primary small" data-nav disabled={torrentBusy} onclick={downloadTorrent}>{torrentBusy ? 'Starting…' : 'Download files'}</button><button class="btn glass small" data-nav disabled={torrentBusy} onclick={() => { torrentReview = null; torrentError = ''; }}>Cancel</button></div></div>
      {/if}
      <div class="item">
        <div>
          <strong>Add a magnet or device link</strong>
          <span>Paste a magnet link or use Share → Copy link on your other Marquee device. Public magnets need tracker or DHT discovery.</span>
        </div>
        <form class="row-actions" onsubmit={(e) => { e.preventDefault(); if (link.trim()) { void app.getFromLink(link); link = ''; } }}>
          <input class="transfer-input" data-nav type="text" placeholder="magnet:?xt=…" aria-label="Game link" bind:value={link} />
          <button class="btn glass small" data-nav type="submit" disabled={!link.trim()}>Get</button>
        </form>
      </div>
      {#if app.transferError}<p role="alert">{app.transferError}</p>{/if}
      {#each app.transfers?.items ?? [] as t (t.info_hash || t.name)}
        <div class="item">
          <div>
            <strong>{t.name}</strong>
            <span>{t.kind === 'get' ? 'Download' : 'Share'} · {transferLine(t)}</span>
            {#if t.directory}<span>Files: {t.directory}</span>{/if}
            {#if t.error && t.state !== 'error'}<span role="alert">{t.error}</span>{/if}
            {#if t.state === 'downloading' && t.total}<progress max={t.total} value={t.downloaded} aria-label="Download progress"></progress>{/if}
          </div>
          <div class="row-actions">
            {#if t.link}<button class="btn glass small" data-nav onclick={() => app.copyTransferLink(t)}>Copy link</button>{/if}
            {#if t.info_hash}<button class="btn glass small" data-nav onclick={() => app.pauseTransfer(t, !['paused','error','source_missing'].includes(t.state))}>{t.state === 'error' ? 'Retry' : ['paused','source_missing'].includes(t.state) ? 'Resume' : 'Pause'}</button>{/if}
            {#if t.info_hash}<button class="btn glass small" data-nav onclick={() => app.stopTransfer(t)}>Stop</button>{/if}
          </div>
        </div>
      {:else}
        <div class="item"><div><span>Nothing shared or received yet.</span></div></div>
      {/each}
    {/if}
  </section>

  {#if app.webUrl}
  <section class="glass-panel" hidden={category !== 'devices'}>
    <h2>Web access</h2>
    <div class="item">
      <div>
        <strong>Open this library in a browser</strong>
        <span>Same games, same data, same launches. Only this computer can connect, and only with the key in the address.</span>
      </div>
      <div class="row-actions">
        <button class="btn glass small" data-nav onclick={() => app.copyWebUrl()}>Copy address</button>
      </div>
    </div>
  </section>
  {/if}


  <section class="glass-panel" hidden={category !== 'appearance'}>
    <h2>Look and feel</h2>
    <div class="item">
      <div><strong>Mode</strong><span>Big Screen uses larger cards and controller navigation. Native controller support depends on your environment.</span></div>
      <div class="seg" data-nav-row>
        <button data-nav aria-pressed={app.mode === 'desktop'} class:on={app.mode === 'desktop'} onclick={() => (app.mode = 'desktop')}><Icon name="monitor" size={16} /> Desktop</button>
        <button data-nav aria-pressed={app.mode === 'big'} class:on={app.mode === 'big'} onclick={() => (app.mode = 'big')}><Icon name="tv" size={16} /> Big Screen</button>
      </div>
    </div>
    <div class="item"><div><strong>Keep navigation expanded</strong><span>Show sidebar labels in desktop mode.</span></div><button class="switch" class:on={app.sidebarExpanded} data-nav role="switch" aria-checked={app.sidebarExpanded} aria-label="Keep navigation expanded" onclick={() => (app.sidebarExpanded = !app.sidebarExpanded)}><i></i></button></div>
    <div class="item">
      <div><strong>Theme</strong><span>Dark is the default</span></div>
      <div class="seg" data-nav-row>
        <button data-nav aria-pressed={app.theme === 'dark'} class:on={app.theme === 'dark'} onclick={() => (app.theme = 'dark')}><Icon name="moon" size={16} /> Dark</button>
        <button data-nav aria-pressed={app.theme === 'light'} class:on={app.theme === 'light'} onclick={() => (app.theme = 'light')}><Icon name="sun" size={16} /> Light</button>
      </div>
    </div>
    <div class="item music-appearance">
      <div><strong>Music accent</strong><span>Music uses round play buttons and its own color, separate from game Play.</span></div>
      <div class="music-colors" role="group" aria-label="Music accent color">
        {#each musicAccents as accent}<button class="music-swatch" data-nav style={`--swatch:${accent.value}`} aria-label={`${accent.name} music accent`} aria-pressed={app.musicAccent === accent.value} title={accent.name} onclick={() => app.musicAccent = accent.value}>{#if app.musicAccent === accent.value}<Icon name="check" size={17} />{/if}</button>{/each}
        <label class="custom-music-color">Custom<input type="color" data-nav aria-label="Custom music accent color" bind:value={app.musicAccent} /></label>
      </div>
    </div>
    <div class="item">
      <div><strong>Visual effects</strong><span>Balanced reduces GPU work. Full enables backdrop blur, grain and pointer tilt.</span></div>
      <div class="seg" data-nav-row>
        <button data-nav aria-pressed={app.visualEffects === 'balanced'} class:on={app.visualEffects === 'balanced'} onclick={() => (app.visualEffects = 'balanced')}>Balanced</button>
        <button data-nav aria-pressed={app.visualEffects === 'full'} class:on={app.visualEffects === 'full'} onclick={() => (app.visualEffects = 'full')}>Full</button>
      </div>
    </div>
    <div class="item">
      <div><strong>Reduce motion</strong><span>Turns off tilt and cross-fades</span></div>
      <button class="switch" class:on={app.reduceMotion} data-nav role="switch" aria-checked={app.reduceMotion} aria-label="Reduce motion" onclick={() => (app.reduceMotion = !app.reduceMotion)}><i></i></button>
    </div>
    <div class="item">
      <div><strong>UI sounds</strong><span>Move, select and back</span></div>
      <button class="switch" class:on={app.sounds} data-nav role="switch" aria-checked={app.sounds} aria-label="UI sounds" onclick={() => (app.sounds = !app.sounds)}><i></i></button>
    </div>
  </section>

  <section class="glass-panel" hidden={category !== 'games'}>
    <h2>Controller</h2>
    <div class="item">
      <div><strong>Last input</strong><span>Arrow keys, Enter and Esc work like a D-pad, A and B</span></div>
      <code class="input"><Icon name="controller" size={18} /> {app.lastInput}</code>
    </div>
  </section>

    </div>
  </div>
</div>

<style>
  .music-colors { display: flex; gap: 10px; flex-wrap: wrap; align-items: center; }
  .music-swatch { width: 36px; height: 36px; border: 3px solid transparent; border-radius: 50%; background: var(--swatch); color: #08120f; cursor: pointer; display: grid; place-items: center; }
  .music-swatch[aria-pressed='true'] { outline: 2px solid var(--text); outline-offset: 2px; }
  .custom-music-color { display: flex; align-items: center; gap: 8px; font-size: 12px; color: var(--text-soft); }
  .custom-music-color input { width: 38px; height: 36px; padding: 3px; border: 1px solid var(--line); border-radius: 8px; background: var(--field); cursor: pointer; }
  .settings {
    padding: 36px var(--gutter);
    max-width: 1400px;
    width:100%;
    margin:auto;
    display: grid;
    gap: 18px;
  }
  .settings-layout { display:grid; grid-template-columns:220px minmax(0,860px); gap:32px; align-items:start; }
  .settings-nav { display:grid; gap:6px; position:sticky; top:24px; }
  .settings-nav button { text-align:left; border:0; border-radius:10px; padding:14px 16px; background:none; color:var(--text-soft); font:inherit; cursor:pointer; }
  .settings-nav button.on { background:var(--field-strong); color:var(--text); font-weight:700; }
  .settings-content { display:grid; gap:18px; min-width:0; }
  section[hidden] { display:none; }
  @media(max-width:900px) { .settings-layout { grid-template-columns:1fr; gap:18px; } .settings-nav { display:flex; flex-wrap:wrap; position:static; } }
  h1 {
    font-size: 32px;
    margin: 0 0 4px;
  }
  section {
    border-radius: 18px;
    padding: 18px 20px;
    display: grid;
    gap: 4px;
    justify-items: start;
  }
  h2 {
    margin: 0 0 8px;
    font-size: 15px;
    color: var(--text-dim);
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }
  .item {
    width: 100%;
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 16px;
    padding: 10px 0;
    border-bottom: 1px solid var(--line);
  }
  .item:last-of-type {
    border-bottom: 0;
  }
  .row-actions {
    display: flex;
    gap: 8px;
    flex: none;
  }
  .item > div:first-child {
    display: grid;
    gap: 2px;
  }
  .item span {
    color: var(--text-dim);
    font-size: 13px;
  }
  .hint {
    margin: 0 0 6px;
    color: var(--text-dim);
    font-size: 13px;
    max-width: 62ch;
  }
  .key {
    flex-wrap: wrap;
    align-items: flex-start;
  }
  .key > div:first-child {
    flex: 1 1 320px;
    min-width: 0;
  }
  .key .status {
    font-weight: 600;
  }
  .key .status.set {
    color: var(--ok);
  }
  .key-edit {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
    align-items: center;
  }
  .key-edit input {
    width: 230px;
    padding: 8px 12px;
    border-radius: 10px;
    border: 1px solid var(--line);
    background: var(--field);
    color: var(--text);
    font: inherit;
    font-size: 13px;
  }
  .key-edit input:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }
  .switch {
    flex: none;
    width: 46px;
    height: 26px;
    border-radius: 999px;
    border: 0;
    background: var(--field-strong);
    position: relative;
    cursor: pointer;
    transition: background 200ms;
  }
  .switch i {
    position: absolute;
    top: 3px;
    left: 3px;
    width: 20px;
    height: 20px;
    border-radius: 50%;
    background: #fff;
    box-shadow: 0 2px 6px rgb(0 0 0 / 0.3);
    transition: transform 220ms var(--ease);
  }
  .switch.on {
    background: var(--accent);
  }
  .switch.on i {
    transform: translateX(20px);
  }
  .seg {
    display: flex;
    background: var(--field);
    border-radius: 12px;
    padding: 3px;
  }
  .seg button {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    border: 0;
    background: none;
    color: var(--text-dim);
    font: inherit;
    font-size: 13px;
    font-weight: 600;
    padding: 7px 12px;
    border-radius: 9px;
    cursor: pointer;
  }
  .seg button.on {
    background: var(--surface-strong);
    color: var(--text);
    box-shadow: 0 2px 8px rgb(0 0 0 / 0.2);
  }
  .input {
    display: inline-flex;
    gap: 8px;
    align-items: center;
    font-size: 13px;
    background: var(--field);
    padding: 6px 10px;
    border-radius: 8px;
  }
  .torrent-choose { white-space: nowrap; flex-shrink: 0; }
  .torrent-review { align-items: flex-start; }
  .torrent-files { max-height: 14rem; overflow: auto; overflow-wrap: anywhere; padding-left: 1.2rem; }
  .transfer-input { min-width: 0; max-width: 100%; border: 1px solid var(--border); background: var(--field); color: var(--text); padding: 8px 10px; border-radius: 8px; font: inherit; }
</style>
